use std::collections::{BTreeMap, HashMap};

use crate::contract::ExtensionTree;
use crate::errors::{join_path, validation, Result};
use crate::fields::{
    assert_leaf, assert_no_datasets, is_label, read_fields, require_child, write_fields, Raw,
    ReadCtx, Values,
};
use crate::model::{
    AuthMode, CaseSensitivity, ClientConfig, Combinator, ComparisonOperator, Condition, Evaluation,
    MachineProfile, MinIntegrityLevel, OpcuaConfig, PipeConfig, QueuePolicy, RootNode, Rule,
    RuleAction, SamplingInterval, SecurityMode, SecurityPolicy, TriggerConfig, UserAccessLevel,
};
use crate::number::{is_numeric_literal, js_number_to_string};
use crate::schema_v1::{
    CLIENT_FIELDS, CONDITION_FIELDS, CURRENT_MINOR, CURRENT_SCHEMA_VERSION, PIPE_FIELDS, RULE_FIELDS,
    SCHEMA_VERSION_ATTR, TRIGGER_FIELDS, VERSION_FIELDS,
};

const CLIENT: &str = "Client";
const PIPE: &str = "Pipe";
const TRIGGER: &str = "Trigger";

// ---- shared disk <-> model scalar mappings (also used by the legacy adapter) ----

pub(crate) fn root_node_from_raw(raw: &str) -> RootNode {
    if raw.is_empty() {
        RootNode::AutoDiscover
    } else if raw.contains('=') {
        RootNode::NodeId {
            value: raw.to_string(),
        }
    } else {
        RootNode::BrowseName {
            value: raw.to_string(),
        }
    }
}

fn root_node_to_raw(node: &RootNode, path: &str) -> Result<String> {
    match node {
        RootNode::AutoDiscover => Ok(String::new()),
        RootNode::NodeId { value } if value.contains('=') => Ok(value.clone()),
        RootNode::NodeId { .. } => Err(validation(path, "NodeId must contain \"=\"")),
        RootNode::BrowseName { value } if !value.is_empty() && !value.contains('=') => {
            Ok(value.clone())
        }
        RootNode::BrowseName { .. } => Err(validation(
            path,
            "BrowseName must be non-empty and must not contain \"=\"",
        )),
    }
}

pub(crate) fn sampling_from_raw(raw: i64, path: &str) -> Result<SamplingInterval> {
    match raw {
        -1 => Ok(SamplingInterval::ServerNegotiated),
        0 => Ok(SamplingInterval::Fastest),
        n if n > 0 => Ok(SamplingInterval::Ms {
            ms: u32::try_from(n).map_err(|_| validation(path, format!("{n} is out of range")))?,
        }),
        n => Err(validation(
            path,
            format!("invalid sampling interval {n}; expected -1, 0 or a positive number of ms"),
        )),
    }
}

fn sampling_to_raw(s: &SamplingInterval, path: &str) -> Result<i32> {
    match s {
        SamplingInterval::ServerNegotiated => Ok(-1),
        SamplingInterval::Fastest => Ok(0),
        SamplingInterval::Ms { ms } => match i32::try_from(*ms) {
            Ok(n) if n >= 1 => Ok(n),
            _ => Err(validation(
                path,
                format!("Ms must be an integer between 1 and {}", i32::MAX),
            )),
        },
    }
}

/// Disk 0 (and -1 where `also_none`) means absent; other negatives are errors.
pub(crate) fn optional_from_raw(raw: i64, path: &str, also_none: bool) -> Result<Option<u32>> {
    if raw == 0 || (also_none && raw == -1) {
        return Ok(None);
    }
    u32::try_from(raw)
        .map(Some)
        .map_err(|_| validation(path, format!("invalid value {raw}")))
}

fn optional_to_raw(n: Option<u32>, path: &str) -> Result<Raw> {
    match n {
        None => Ok(Raw::Int(0)),
        Some(0) => Err(validation(path, "must be a positive integer when present")),
        Some(n) => Ok(Raw::from(n)),
    }
}

fn flag_to_raw(b: bool) -> Raw {
    Raw::Int(i64::from(b))
}

fn attr_path(group: &str, name: &str) -> String {
    join_path(group, &format!("@{name}"))
}

/// The value was checked against the field's enum list, so a failed parse means the schema lists
/// and the model enums disagree.
fn parse_enum<T>(parse: fn(&str) -> Option<T>, v: &Values, name: &str, group: &str) -> Result<T> {
    let s = v.str(name);
    parse(s).ok_or_else(|| validation(attr_path(group, name), format!("invalid value {s:?}")))
}

// ---- unpack ----

fn unpack_client(group: &ExtensionTree, ctx: &mut ReadCtx<'_>) -> Result<ClientConfig> {
    assert_leaf(group, CLIENT)?;
    let (v, extra) = read_fields(&group.attrs, CLIENT_FIELDS, CLIENT, &[], ctx)?;
    let profile = v.str("Machine_Profile");
    let policy = v.str("Security_Policy");
    Ok(ClientConfig {
        server_url: v.str("Server_URL").to_string(),
        auth_mode: parse_enum(AuthMode::parse, &v, "Auth_Mode", CLIENT)?,
        machine_profile: if profile.is_empty() {
            None
        } else {
            Some(parse_enum(
                MachineProfile::parse,
                &v,
                "Machine_Profile",
                CLIENT,
            )?)
        },
        security_mode: parse_enum(SecurityMode::parse, &v, "Security_Mode", CLIENT)?,
        security_policy: if policy.is_empty() {
            SecurityPolicy::Auto
        } else {
            parse_enum(SecurityPolicy::parse, &v, "Security_Policy", CLIENT)?
        },
        root_node: root_node_from_raw(v.str("Root_Node")),
        sampling_interval: sampling_from_raw(
            v.int("Sampling_Interval"),
            &attr_path(CLIENT, "Sampling_Interval"),
        )?,
        publish_interval: v.u32("Publish_Interval"),
        bfs_max_depth: v.u32("BFS_Max_Depth"),
        keep_alive_count: v.u32("Keep_Alive_Count"),
        lifetime_count: v.u32("Lifetime_Count"),
        queue_policy: parse_enum(QueuePolicy::parse, &v, "Queue_Policy", CLIENT)?,
        queue_size_data_change: v.u32("Queue_Size_Data_Change"),
        queue_size_events: v.u32("Queue_Size_Events"),
        reconnect_interval: v.u32("Reconnect_Interval"),
        session_timeout: v.u32("Session_Timeout"),
        sync_loop_interval_initial: v.u32("Sync_Loop_Interval_Initial"),
        sync_loop_interval_settled: v.u32("Sync_Loop_Interval_Settled"),
        extra,
    })
}

fn unpack_pipe(group: &ExtensionTree, ctx: &mut ReadCtx<'_>) -> Result<PipeConfig> {
    assert_leaf(group, PIPE)?;
    let (v, extra) = read_fields(&group.attrs, PIPE_FIELDS, PIPE, &[], ctx)?;
    let rate = optional_from_raw(
        v.int("Inbound_Rate_Limit"),
        &attr_path(PIPE, "Inbound_Rate_Limit"),
        true,
    )?;
    Ok(PipeConfig {
        enabled: v.flag("Pipe_Enabled"),
        configure_client: v.flag("Configure_Client"),
        buffer_size: v.u32("Buffer_Size"),
        inbound_rate_limit_hz: rate,
        max_inbound_message_size: v.u32("Max_Inbound_Message_Size"),
        min_integrity_level: parse_enum(MinIntegrityLevel::parse, &v, "Min_Integrity_Level", PIPE)?,
        pipe_name: v.str("Pipe_Name").to_string(),
        user_access_level: parse_enum(UserAccessLevel::parse, &v, "User_Access_Level", PIPE)?,
        extra,
    })
}

fn unpack_condition(label: &str, group: &ExtensionTree, rule_path: &str, ctx: &mut ReadCtx <'_>) -> Result<Condition> {
    let path = join_path(rule_path, label);
    if !is_label(label) {
        return Err(validation(
            path,
            "label must be non-empty and contain no \"/\" or NUL",
        ));
    }
    assert_leaf(group, &path)?;
    let (v, extra) = read_fields(&group.attrs, CONDITION_FIELDS, &path, &[], ctx)?;
    if v.str("Condition_Label") != label {
        return Err(validation(
            attr_path(&path, "Condition_Label"),
            format!("must equal the group name \"{label}\""),
        ));
    }
    // The field the mode uses must be present, and its blank reads as the default with a warning.
    // The other one is blank by convention and is ignored.
    let mode = v.str("Evaluation_Mode");
    let numeric = mode == "Numeric";
    let (used, default) = if numeric {
        ("Comparison_Operator", "Equal")
    } else {
        ("Case_Sensitivity", "Exact")
    };
    if !group.attrs.contains_key(used) {
        return Err(validation(
            attr_path(&path, used),
            format!("required when Evaluation_Mode is {mode}"),
        ));
    }
    let used_value = match v.str(used) {
        "" => {
            ctx.warn(&attr_path(&path, used), format!("blank, read as {default:?}"));
            default
        }
        s => s,
    };
    let bad_used = || validation(attr_path(&path, used), format!("invalid value {used_value:?}"));
    let evaluation = if numeric {
        let text = v.str("Value");
        let threshold = if is_numeric_literal(text) {
            text.parse::<f64>().ok().filter(|n| n.is_finite())
        } else {
            None
        };
        let Some(threshold) = threshold else {
            return Err(validation(
                attr_path(&path, "Value"),
                format!("Numeric condition value \"{text}\" is not a finite number"),
            ));
        };
        let epsilon = v.f64("Epsilon");
        if epsilon < 0.0 {
            return Err(validation(
                attr_path(&path, "Epsilon"),
                "must be >= 0 (0 = the consumer's default tolerance)",
            ));
        }
        Evaluation::Numeric {
            threshold,
            operator: ComparisonOperator::parse(used_value).ok_or_else(bad_used)?,
            epsilon,
        }
    } else {
        Evaluation::String {
            value: v.str("Value").to_string(),
            case_sensitivity: CaseSensitivity::parse(used_value).ok_or_else(bad_used)?,
        }
    };
    Ok(Condition {
        id: v.u32("Condition_Id"),
        label: label.to_string(),
        enabled: v.flag("Condition_Enabled"),
        event: v.str("Event").to_string(),
        subsystem: v.str("Subsystem").to_string(),
        component: v.str("Component").to_string(),
        signal: v.str("Signal").to_string(),
        evaluation,
        extra,
    })
}

fn unpack_rule(label: &str, group: &ExtensionTree, ctx: &mut ReadCtx<'_>) -> Result<Rule> {
    let path = join_path(TRIGGER, label);
    if !is_label(label) {
        return Err(validation(
            path,
            "label must be non-empty and contain no \"/\" or NUL",
        ));
    }
    assert_no_datasets(group, &path)?;
    let (v, extra) = read_fields(&group.attrs, RULE_FIELDS, &path, &[], ctx)?;
    if v.str("Rule_Label") != label {
        return Err(validation(
            attr_path(&path, "Rule_Label"),
            format!("must equal the group name \"{label}\""),
        ));
    }

    let mut conditions: Vec<Condition> = Vec::new();
    for (name, child) in &group.children {
        let c = unpack_condition(name, child, &path, ctx)?;
        if conditions.iter().any(|o| o.id == c.id) {
            return Err(validation(
                attr_path(&join_path(&path, name), "Condition_Id"),
                format!("duplicate condition id {}", c.id),
            ));
        }
        conditions.push(c);
    }
    conditions.sort_by_key(|c| c.id);

    if conditions.len() > 1 && !group.attrs.contains_key("Combinator") {
        return Err(validation(attr_path(&path, "Combinator"), "required when a rule has more than one condition"));
    }

    let combinator = v.str("Combinator");
     if conditions.len() > 1 {
        if !group.attrs.contains_key("Combinator") {
            return Err(validation(
                attr_path(&path, "Combinator"),
                "required when a rule has more than one condition",
            ));
        }
        if combinator.is_empty() {
            ctx.warn(
                &attr_path(&path, "Combinator"),
                format!("blank with {} conditions: consumers evaluate only one of them", conditions.len()),
            );
        }
    }
    let max_fires = optional_from_raw(
        v.int("Max_Fires_Per_Job"),
        &attr_path(&path, "Max_Fires_Per_Job"),
        false,
    )?;
    Ok(Rule {
        id: v.u32("Rule_Id"),
        label: label.to_string(),
        description: v.str("Description").to_string(),
        action: parse_enum(RuleAction::parse, &v, "Action", &path)?,
        combinator: if combinator.is_empty() {
            None
        } else {
            Some(parse_enum(Combinator::parse, &v, "Combinator", &path)?)
        },
        enabled: v.flag("Rule_Enabled"),
        priority: v.u32("Priority"),
        cooldown_ms: v.u32("Cooldown_Period"),
        max_fires_per_job: max_fires,
        conditions,
        extra,
    })
}

fn unpack_triggers(group: &ExtensionTree, ctx: &mut ReadCtx<'_>) -> Result<TriggerConfig> {
    assert_no_datasets(group, TRIGGER)?;
    let (v, extra) = read_fields(&group.attrs, TRIGGER_FIELDS, TRIGGER, &[], ctx)?;
    let mut rules: Vec<Rule> = Vec::new();
    for (name, child) in &group.children {
        let r = unpack_rule(name, child, ctx)?;
        if rules.iter().any(|o| o.id == r.id) {
            return Err(validation(
                attr_path(&join_path(TRIGGER, name), "Rule_Id"),
                format!("duplicate rule id {}", r.id),
            ));
        }
        rules.push(r);
    }
    rules.sort_by_key(|r| r.id);
    let ceiling = optional_from_raw(
        v.int("Trigger_Stop_Ceiling_Layers"),
        &attr_path(TRIGGER, "Trigger_Stop_Ceiling_Layers"),
        false,
    )?;
    Ok(TriggerConfig {
        enabled: v.flag("Triggers_Enabled"),
        stop_ceiling_layers: ceiling,
        rules,
        extra,
    })
}

/// Unpacks any 1.x tree; unknown attributes go to `extra`, unknown groups and datasets are errors.
pub(crate) fn unpack_v1(tree: &ExtensionTree, file_minor: u32) -> Result<OpcuaConfig> {
    assert_no_datasets(tree, "")?;
    if let Some(name) = tree
        .children
        .keys()
        .find(|n| !matches!(n.as_str(), CLIENT | PIPE | TRIGGER))
    {
        return Err(validation(name.as_str(), "unexpected child group"));
    }
    let mut warnings = Vec::new();
    let mut ctx = ReadCtx { file_minor, reader_minor: CURRENT_MINOR, warnings: &mut warnings };
    let (_, extra) = read_fields(&tree.attrs, &[], "", &[SCHEMA_VERSION_ATTR], &mut ctx)?;
    let client = unpack_client(require_child(tree, CLIENT)?, &mut ctx)?;
    let pipe = unpack_pipe(require_child(tree, PIPE)?, &mut ctx)?;
    let triggers = unpack_triggers(require_child(tree, TRIGGER)?, &mut ctx)?;
    Ok(OpcuaConfig {
        client,
        pipe,
        triggers,
        extra,
        warnings: (!warnings.is_empty()).then_some(warnings),
    })
}

// ---- pack ----

fn pack_client(c: &ClientConfig) -> Result<ExtensionTree> {
    let policy = c.security_policy.to_spec();
    let values: HashMap<&'static str, Raw> = HashMap::from([
        ("Auth_Mode", Raw::from(c.auth_mode.as_str())),
        ("BFS_Max_Depth", Raw::from(c.bfs_max_depth)),
        ("Keep_Alive_Count", Raw::from(c.keep_alive_count)),
        ("Lifetime_Count", Raw::from(c.lifetime_count)),
        (
            "Machine_Profile",
            Raw::from(c.machine_profile.map_or("", MachineProfile::as_str)),
        ),
        ("Publish_Interval", Raw::from(c.publish_interval)),
        ("Queue_Policy", Raw::from(c.queue_policy.as_str())),
        (
            "Queue_Size_Data_Change",
            Raw::from(c.queue_size_data_change),
        ),
        ("Queue_Size_Events", Raw::from(c.queue_size_events)),
        ("Reconnect_Interval", Raw::from(c.reconnect_interval)),
        (
            "Root_Node",
            Raw::from(root_node_to_raw(
                &c.root_node,
                &attr_path(CLIENT, "Root_Node"),
            )?),
        ),
        (
            "Sampling_Interval",
            Raw::from(sampling_to_raw(
                &c.sampling_interval,
                &attr_path(CLIENT, "Sampling_Interval"),
            )?),
        ),
        ("Security_Mode", Raw::from(c.security_mode.as_str())),
        ("Security_Policy", Raw::from(policy)),
        ("Server_URL", Raw::from(c.server_url.as_str())),
        ("Session_Timeout", Raw::from(c.session_timeout)),
        (
            "Sync_Loop_Interval_Initial",
            Raw::from(c.sync_loop_interval_initial),
        ),
        (
            "Sync_Loop_Interval_Settled",
            Raw::from(c.sync_loop_interval_settled),
        ),
    ]);
    Ok(ExtensionTree {
        attrs: write_fields(values, CLIENT_FIELDS, &c.extra, CLIENT)?,
        ..Default::default()
    })
}

fn pack_pipe(c: &PipeConfig) -> Result<ExtensionTree> {
    let values: HashMap<&'static str, Raw> = HashMap::from([
        ("Buffer_Size", Raw::from(c.buffer_size)),
        ("Configure_Client", flag_to_raw(c.configure_client)),
        (
            "Inbound_Rate_Limit",
            optional_to_raw(
                c.inbound_rate_limit_hz,
                &attr_path(PIPE, "Inbound_Rate_Limit"),
            )?,
        ),
        (
            "Max_Inbound_Message_Size",
            Raw::from(c.max_inbound_message_size),
        ),
        (
            "Min_Integrity_Level",
            Raw::from(c.min_integrity_level.as_str()),
        ),
        ("Pipe_Enabled", flag_to_raw(c.enabled)),
        ("Pipe_Name", Raw::from(c.pipe_name.as_str())),
        ("User_Access_Level", Raw::from(c.user_access_level.as_str())),
    ]);
    Ok(ExtensionTree {
        attrs: write_fields(values, PIPE_FIELDS, &c.extra, PIPE)?,
        ..Default::default()
    })
}

fn pack_condition(c: &Condition, rule_path: &str) -> Result<ExtensionTree> {
    let path = join_path(rule_path, &c.label);
    let (mode, value, case, operator, epsilon) = match &c.evaluation {
        Evaluation::Numeric {
            threshold,
            operator,
            epsilon,
        } => {
            if !threshold.is_finite() {
                return Err(validation(
                    attr_path(&path, "Value"),
                    "Numeric threshold must be a finite number",
                ));
            }
            if *epsilon < 0.0 {
                return Err(validation(attr_path(&path, "Epsilon"), "must be >= 0"));
            }
            (
                "Numeric",
                js_number_to_string(*threshold),
                "Exact",
                operator.as_str(),
                *epsilon,
            )
        }
        Evaluation::String {
            value,
            case_sensitivity,
        } => (
            "String",
            value.clone(),
            case_sensitivity.as_str(),
            "Equal",
            0.0,
        ),
    };
    let values: HashMap<&'static str, Raw> = HashMap::from([
        ("Condition_Id", Raw::from(c.id)),
        ("Condition_Label", Raw::from(c.label.as_str())),
        ("Condition_Enabled", flag_to_raw(c.enabled)),
        ("Signal", Raw::from(c.signal.as_str())),
        ("Event", Raw::from(c.event.as_str())),
        ("Subsystem", Raw::from(c.subsystem.as_str())),
        ("Component", Raw::from(c.component.as_str())),
        ("Evaluation_Mode", Raw::from(mode)),
        ("Value", Raw::from(value)),
        ("Case_Sensitivity", Raw::from(case)),
        ("Comparison_Operator", Raw::from(operator)),
        ("Epsilon", Raw::from(epsilon)),
    ]);
    Ok(ExtensionTree {
        attrs: write_fields(values, CONDITION_FIELDS, &c.extra, &path)?,
        ..Default::default()
    })
}

/// Checks labels and ids and returns the items ordered by id.
fn unique_sorted<'a, T>(
    items: &'a [T],
    id: impl Fn(&T) -> u32,
    label: impl Fn(&T) -> &str,
    parent_path: &str,
    id_attr: &str,
    kind: &str,
) -> Result<Vec<&'a T>> {
    let mut labels: Vec<&str> = Vec::new();
    let mut ids: Vec<u32> = Vec::new();
    for it in items {
        let l = label(it);
        let path = join_path(parent_path, l);
        if !is_label(l) {
            return Err(validation(
                path,
                format!("{kind} label must be non-empty and contain no \"/\" or NUL"),
            ));
        }
        if labels.contains(&l) {
            return Err(validation(path, format!("duplicate {kind} label \"{l}\"")));
        }
        labels.push(l);
        if ids.contains(&id(it)) {
            return Err(validation(
                attr_path(&path, id_attr),
                format!("duplicate {kind} id {}", id(it)),
            ));
        }
        ids.push(id(it));
    }
    let mut sorted: Vec<&T> = items.iter().collect();
    sorted.sort_by_key(|it| id(it));
    Ok(sorted)
}

fn pack_rule(r: &Rule) -> Result<ExtensionTree> {
    let path = join_path(TRIGGER, &r.label);
    let values: HashMap<&'static str, Raw> = HashMap::from([
        ("Rule_Id", Raw::from(r.id)),
        ("Rule_Label", Raw::from(r.label.as_str())),
        ("Action", Raw::from(r.action.as_str())),
        ("Rule_Enabled", flag_to_raw(r.enabled)),
        (
            "Combinator",
            Raw::from(r.combinator.map_or("", Combinator::as_str)),
        ),
        ("Cooldown_Period", Raw::from(r.cooldown_ms)),
        ("Description", Raw::from(r.description.as_str())),
        (
            "Max_Fires_Per_Job",
            optional_to_raw(r.max_fires_per_job, &attr_path(&path, "Max_Fires_Per_Job"))?,
        ),
        ("Priority", Raw::from(r.priority)),
    ]);
    let attrs = write_fields(values, RULE_FIELDS, &r.extra, &path)?;
    let mut children = BTreeMap::new();
    for c in unique_sorted(
        &r.conditions,
        |c| c.id,
        |c| &c.label,
        &path,
        "Condition_Id",
        "condition",
    )? {
        children.insert(c.label.clone(), pack_condition(c, &path)?);
    }
    Ok(ExtensionTree {
        attrs,
        children,
        ..Default::default()
    })
}

fn pack_triggers(t: &TriggerConfig) -> Result<ExtensionTree> {
    let values: HashMap<&'static str, Raw> = HashMap::from([
        ("Triggers_Enabled", flag_to_raw(t.enabled)),
        (
            "Trigger_Stop_Ceiling_Layers",
            optional_to_raw(
                t.stop_ceiling_layers,
                &attr_path(TRIGGER, "Trigger_Stop_Ceiling_Layers"),
            )?,
        ),
    ]);
    let attrs = write_fields(values, TRIGGER_FIELDS, &t.extra, TRIGGER)?;
    let mut children = BTreeMap::new();
    for r in unique_sorted(&t.rules, |r| r.id, |r| &r.label, TRIGGER, "Rule_Id", "rule")? {
        children.insert(r.label.clone(), pack_rule(r)?);
    }
    Ok(ExtensionTree {
        attrs,
        children,
        ..Default::default()
    })
}

/// Packs to schema 1.2: every attribute is written with its exact dtype; rules and conditions are
/// written in id order (the on-disk order is not significant).
pub(crate) fn pack_v1(config: &OpcuaConfig) -> Result<ExtensionTree> {
    let version: HashMap<&'static str, Raw> =
        HashMap::from([(SCHEMA_VERSION_ATTR, Raw::from(CURRENT_SCHEMA_VERSION))]);
    let attrs = write_fields(version, VERSION_FIELDS, &config.extra, "")?;
    let children = BTreeMap::from([
        (CLIENT.to_string(), pack_client(&config.client)?),
        (PIPE.to_string(), pack_pipe(&config.pipe)?),
        (TRIGGER.to_string(), pack_triggers(&config.triggers)?),
    ]);
    Ok(ExtensionTree {
        attrs,
        children,
        ..Default::default()
    })
}
