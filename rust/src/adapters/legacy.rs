//! Layout written by core's typed OPC UA support (no schema version attribute). A port of
//! rdt-core's `config-import-opcua-legacy.util.ts` plus the legacy vocabulary mapping.

use std::collections::BTreeMap;

use crate::adapters::v1::{optional_from_raw, pack_v1, root_node_from_raw, sampling_from_raw};
use crate::contract::{Attr, ExtensionTree, Scalar, Value};
use crate::errors::{join_path, validation, Result};
use crate::fields::{
    assert_leaf, assert_no_datasets, check_value, read_fields, require_child, spec,
    FieldSpec, Raw, ReadCtx,
};
use crate::model::{
    AuthMode, CaseSensitivity, ClientConfig, Combinator, Condition, Evaluation, ExtraAttrs,
    MachineProfile, MinIntegrityLevel, OpcuaConfig, PipeConfig, QueuePolicy, Rule, RuleAction,
    SecurityMode, SecurityPolicy, TriggerConfig, UserAccessLevel,
};
use crate::schema_v1::{
    CLIENT_FIELDS, CONDITION_FIELDS, CURRENT_MINOR, PIPE_FIELDS, RULE_FIELDS, TRIGGER_FIELDS,
};
use crate::{Warning, WarningKind};

use std::cell::RefCell;

type Notes = RefCell<Vec<Warning>>;

const CLIENT: &str = "Client";
const PIPE: &str = "Pipe";
const TRIGGERS: &str = "Triggers";

const CLIENT_KNOWN: &[&str] = &[
    "Server_URL",
    "Auth_Mode",
    "Security_Mode",
    "Security_Policy",
    "BFS_Max_Depth",
    "Publish_Interval",
    "Sampling_Interval",
    "Session_Timeout",
    "Keep_Alive_Count",
    "Lifetime_Count",
    "Machine_Profile",
    "Queue_Policy",
    "Queue_Size_Data_Change",
    "Queue_Size_Events",
    "Reconnect_Interval",
    "Root_Node",
    "Sync_Loop_Interval_Initial",
    "Sync_Loop_Interval_Settled",
];
const PIPE_KNOWN: &[&str] = &[
    "Pipe_Enabled",
    "Buffer_Size",
    "Configure_Client",
    "Inbound_Rate_Limit",
    "Max_Inbound_Message_Size",
    "Min_Integrity_Level",
    "Pipe_Name",
    "User_Access_Level",
];
const TRIGGERS_KNOWN: &[&str] = &["Triggers_Enabled", "Trigger_Stop_Ceiling_Layers"];
// ID and Trigger_Label are dropped: rule ids are assigned and the group name is the label.
const TRIGGER_KNOWN: &[&str] = &[
    "ID",
    "Signal",
    "Subsystem",
    "Rule_Enabled",
    "Start_Value",
    "Stop_Value",
    "Case_Sensitivity",
    "Component",
    "Cooldown_Period",
    "Event",
    "Max_Fires_Per_Job",
    "Trigger_Label",
];

/// Typed access to legacy attributes: integers may arrive as any integral numeric dtype.
struct LegacyGroup<'a> {
    attrs: &'a BTreeMap<String, Attr>,
    path: &'a str,
}

impl<'a> LegacyGroup<'a> {
    fn new(attrs: &'a BTreeMap<String, Attr>, path: &'a str) -> Self {
        Self { attrs, path }
    }

    fn at(&self, name: &str) -> String {
        join_path(self.path, &format!("@{name}"))
    }

    fn scalar(&self, name: &str) -> Result<Option<(&'a Attr, &'a Scalar)>> {
        match self.attrs.get(name) {
            None => Ok(None),
            Some(a) => match &a.value {
                Value::Scalar(s) => Ok(Some((a, s))),
                Value::Array(_) => {
                    Err(validation(self.at(name), "expected a scalar, got an array"))
                }
            },
        }
    }

    /// `def` None = required.
    fn str(&self, name: &str, def: Option<&str>) -> Result<String> {
        match self.scalar(name)? {
            None => def
                .map(str::to_string)
                .ok_or_else(|| validation(self.at(name), "missing required attribute")),
            Some((_, Scalar::Utf8(s))) => Ok(s.clone()),
            Some((a, _)) => Err(validation(
                self.at(name),
                format!("expected dtype utf8, got {}", a.dtype.name()),
            )),
        }
    }

    fn int(&self, name: &str, def: Option<i64>) -> Result<i64> {
        let here = || self.at(name);
        let Some((_, s)) = self.scalar(name)? else {
            return def.ok_or_else(|| validation(here(), "missing required attribute"));
        };
        let out_of_range =
            |v: &dyn std::fmt::Display| validation(here(), format!("{v} is out of range"));
        match s {
            Scalar::Utf8(_) => Err(validation(here(), "expected a numeric attribute, got utf8")),
            Scalar::I8(v) => Ok(i64::from(*v)),
            Scalar::U8(v) => Ok(i64::from(*v)),
            Scalar::I16(v) => Ok(i64::from(*v)),
            Scalar::U16(v) => Ok(i64::from(*v)),
            Scalar::I32(v) => Ok(i64::from(*v)),
            Scalar::U32(v) => Ok(i64::from(*v)),
            Scalar::I64(v) => Ok(*v),
            Scalar::U64(v) => i64::try_from(*v).map_err(|_| out_of_range(v)),
            Scalar::F32(v) => float_to_int(f64::from(*v), here()),
            Scalar::F64(v) => float_to_int(*v, here()),
        }
    }

    fn opt_int(&self, name: &str) -> Result<Option<i64>> {
        if self.attrs.contains_key(name) {
            self.int(name, None).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Validates against the v1 field spec, so legacy data meets the same rules.
    fn checked_str(&self, f: &FieldSpec, value: String) -> Result<String> {
        match check_value(f, Raw::Str(value), self.path)? {
            Raw::Str(s) => Ok(s),
            _ => unreachable!("a string field returns a string"),
        }
    }

    fn checked_int(&self, f: &FieldSpec, value: i64) -> Result<i64> {
        match check_value(f, Raw::Int(value), self.path)? {
            Raw::Int(n) => Ok(n),
            _ => unreachable!("an integer field returns an integer"),
        }
    }
}

fn float_to_int(v: f64, path: String) -> Result<i64> {
    if v.is_finite() && v.fract() == 0.0 {
        Ok(v as i64)
    } else {
        Err(validation(
            path,
            format!("expected an integral value, got {v}"),
        ))
    }
}

fn extra_of(
    attrs: &BTreeMap<String, Attr>,
    known: &[&str],
    path: &str,
    notes: &Notes,
) -> Result<ExtraAttrs> {
    let mut warnings = notes.borrow_mut();
    let mut ctx = ReadCtx {
        file_minor: 0,
        reader_minor: CURRENT_MINOR,
        warnings: &mut warnings,
    };
    Ok(read_fields(attrs, &[], path, known, &mut ctx)?.1)
}

/// A checked value that is not in the model enum means the schema lists and the enums disagree.
fn to_enum<T>(parse: fn(&str) -> Option<T>, value: &str, path: String) -> Result<T> {
    parse(value).ok_or_else(|| validation(path, format!("invalid value {value:?}")))
}

fn unpack_client(group: &ExtensionTree, notes: &Notes) -> Result<ClientConfig> {
    assert_leaf(group, CLIENT)?;
    let g = LegacyGroup::new(&group.attrs, CLIENT);
    let field = |name: &str| spec(CLIENT_FIELDS, name);
    let str = |name: &str, def: Option<&str>| g.checked_str(field(name), g.str(name, def)?);
    let num = |name: &str, def: Option<i64>| g.checked_int(field(name), g.int(name, def)?);
    let u = |name: &str, def: i64| num(name, Some(def)).map(|n| n as u32);

    let profile = g.str("Machine_Profile", Some(""))?;
    let policy = g.str("Security_Policy", None)?;
    let queue = g.str("Queue_Policy", Some("DiscardOldest"))?;

    let machine_profile = if profile.is_empty() {
        None
    } else {
        let canonical = g.checked_str(field("Machine_Profile"), profile)?;
        Some(to_enum(
            MachineProfile::parse,
            &canonical,
            g.at("Machine_Profile"),
        )?)
    };
    let security_policy = if policy.is_empty() {
        SecurityPolicy::Auto
    } else {
        if policy.eq_ignore_ascii_case("none") {
            notes.borrow_mut().push(Warning { 
                kind: WarningKind::Conversion,
                path: g.at("Security_Policy"),
                message: "\"None\" is read as the no-security policy (earlier codec versions read it as Auto)".into(),
            });
        }
        let canonical = g.checked_str(field("Security_Policy"), policy)?;
        to_enum(SecurityPolicy::parse, &canonical, g.at("Security_Policy"))?
    };
    let queue = g.checked_str(field("Queue_Policy"), queue)?;

    Ok(ClientConfig {
        server_url: str("Server_URL", None)?,
        auth_mode: to_enum(AuthMode::parse, &str("Auth_Mode", None)?, g.at("Auth_Mode"))?,
        machine_profile,
        security_mode: to_enum(
            SecurityMode::parse,
            &str("Security_Mode", None)?,
            g.at("Security_Mode"),
        )?,
        security_policy,
        root_node: root_node_from_raw(&str("Root_Node", Some(""))?),
        sampling_interval: sampling_from_raw(
            num("Sampling_Interval", None)?,
            &format!("{CLIENT}/@Sampling_Interval"),
        )?,
        publish_interval: u("Publish_Interval", 250)?,
        bfs_max_depth: u("BFS_Max_Depth", 16)?,
        keep_alive_count: u("Keep_Alive_Count", 240)?,
        lifetime_count: u("Lifetime_Count", 2400)?,
        queue_policy: to_enum(QueuePolicy::parse, &queue, g.at("Queue_Policy"))?,
        queue_size_data_change: u("Queue_Size_Data_Change", 10)?,
        queue_size_events: u("Queue_Size_Events", 10)?,
        reconnect_interval: u("Reconnect_Interval", 1000)?,
        session_timeout: u("Session_Timeout", 60000)?,
        sync_loop_interval_initial: u("Sync_Loop_Interval_Initial", 1000)?,
        sync_loop_interval_settled: u("Sync_Loop_Interval_Settled", 30000)?,
        extra: extra_of(&group.attrs, CLIENT_KNOWN, CLIENT, notes)?,
    })
}

fn unpack_pipe(group: &ExtensionTree, notes: &Notes) -> Result<PipeConfig> {
    assert_leaf(group, PIPE)?;
    let g = LegacyGroup::new(&group.attrs, PIPE);
    let field = |name: &str| spec(PIPE_FIELDS, name);
    let num = |name: &str, def: i64| g.checked_int(field(name), g.int(name, Some(def))?);
    let u = |name: &str, def: i64| num(name, def).map(|n| n as u32);

    let level = g.str("Min_Integrity_Level", Some("ProtectedProcess"))?;
    let rate = optional_from_raw(
        num("Inbound_Rate_Limit", 100)?,
        &format!("{PIPE}/@Inbound_Rate_Limit"),
        true,
    )?;
    let level = g.checked_str(field("Min_Integrity_Level"), level)?;
    let pipe_name = g.checked_str(
        field("Pipe_Name"),
        g.str("Pipe_Name", Some("\\\\.\\pipe\\opc_ua_client_pipe"))?,
    )?;
    let access = g.checked_str(
        field("User_Access_Level"),
        g.str("User_Access_Level", Some("CurrentUserOnly"))?,
    )?;

    Ok(PipeConfig {
        enabled: num("Pipe_Enabled", 0)? == 1,
        configure_client: num("Configure_Client", 0)? == 1,
        buffer_size: u("Buffer_Size", 65536)?,
        inbound_rate_limit_hz: rate,
        max_inbound_message_size: u("Max_Inbound_Message_Size", 4096)?,
        min_integrity_level: to_enum(
            MinIntegrityLevel::parse,
            &level,
            g.at("Min_Integrity_Level"),
        )?,
        pipe_name,
        user_access_level: to_enum(UserAccessLevel::parse, &access, g.at("User_Access_Level"))?,
        extra: extra_of(&group.attrs, PIPE_KNOWN, PIPE, notes)?,
    })
}

fn unpack_triggers(group: Option<&ExtensionTree>, notes: &Notes) -> Result<TriggerConfig> {
    let Some(group) = group else {
        return Ok(TriggerConfig {
            enabled: false,
            stop_ceiling_layers: None,
            rules: Vec::new(),
            extra: ExtraAttrs::new(),
        });
    };
    assert_no_datasets(group, TRIGGERS)?;
    let g = LegacyGroup::new(&group.attrs, TRIGGERS);

    let stop = match g.opt_int("Trigger_Stop_Ceiling_Layers")? {
        None => None,
        Some(n) => {
            let n = g.checked_int(spec(TRIGGER_FIELDS, "Trigger_Stop_Ceiling_Layers"), n)?;
            optional_from_raw(
                n,
                &format!("{TRIGGERS}/@Trigger_Stop_Ceiling_Layers"),
                false,
            )?
        }
    };
    let enabled = g.checked_int(
        spec(TRIGGER_FIELDS, "Triggers_Enabled"),
        g.int("Triggers_Enabled", Some(0))?,
    )? == 1;

    let mut rules: Vec<Rule> = Vec::new();
    let mut next_id = 1;
    for (trigger_label, trigger) in &group.children {
        let path = join_path(TRIGGERS, trigger_label);
        assert_leaf(trigger, &path)?;
        let t = LegacyGroup::new(&trigger.attrs, &path);

        let label = trigger_label.replace('/', "_");
        let signal = t.str("Signal", Some(""))?;
        let rule_enabled = t.checked_int(
            spec(RULE_FIELDS, "Rule_Enabled"),
            t.int("Rule_Enabled", Some(0))?,
        )? == 1;
        let cooldown = t.checked_int(
            spec(RULE_FIELDS, "Cooldown_Period"),
            t.int("Cooldown_Period", Some(0))?,
        )?;
        let max_fires = optional_from_raw(
            t.checked_int(
                spec(RULE_FIELDS, "Max_Fires_Per_Job"),
                t.int("Max_Fires_Per_Job", Some(0))?,
            )?,
            &join_path(&path, "@Max_Fires_Per_Job"),
            false,
        )?;
        let cond = |name: &str, def: &str| {
            t.checked_str(spec(CONDITION_FIELDS, name), t.str(name, Some(def))?)
        };
        let event = cond("Event", "")?;
        let subsystem = cond("Subsystem", "")?;
        let component = cond("Component", "")?;
        let case_sensitivity = to_enum(
            CaseSensitivity::parse,
            &cond("Case_Sensitivity", "Exact")?,
            t.at("Case_Sensitivity"),
        )?;
        let sides = [
            (
                RuleAction::CaptureStart,
                "Start",
                t.str("Start_Value", Some(""))?,
            ),
            (
                RuleAction::CaptureStop,
                "Stop",
                t.str("Stop_Value", Some(""))?,
            ),
        ];

        for (action, suffix, value) in sides {
            let rule_label = format!("{label} - {suffix}");
            if signal.is_empty() || value.is_empty() {
                notes.borrow_mut().push(Warning {
                    kind: WarningKind::Conversion,
                    path: path.clone(),
                    message: format!("skipped rule \"{rule_label}\": legacy trigger has no signal or value for that action")
                });
                continue;
            }
            rules.push(Rule {
                id: next_id,
                label: rule_label,
                description: String::new(),
                action,
                combinator: Some(Combinator::And),
                enabled: rule_enabled,
                priority: 0,
                cooldown_ms: cooldown as u32,
                max_fires_per_job: max_fires,
                conditions: vec![Condition {
                    id: 1,
                    label: signal.replace('/', "_"),
                    enabled: true,
                    event: event.clone(),
                    subsystem: subsystem.clone(),
                    component: component.clone(),
                    signal: signal.clone(),
                    evaluation: Evaluation::String {
                        value,
                        case_sensitivity,
                    },
                    extra: ExtraAttrs::new(),
                }],
                extra: extra_of(&trigger.attrs, TRIGGER_KNOWN, &path, notes)?,
            });
            next_id += 1;
        }
    }

    Ok(TriggerConfig {
        enabled,
        stop_ceiling_layers: stop,
        rules,
        extra: extra_of(&group.attrs, TRIGGERS_KNOWN, TRIGGERS, notes)?,
    })
}

/// Converts the legacy layout to the model; the result is validated the same way `pack` validates.
pub(crate) fn unpack_legacy(tree: &ExtensionTree) -> Result<OpcuaConfig> {
    assert_no_datasets(tree, "")?;
    if let Some(name) = tree
        .children
        .keys()
        .find(|n| !matches!(n.as_str(), CLIENT | PIPE | TRIGGERS))
    {
        return Err(validation(name.as_str(), "unexpected child group"));
    }
    let notes = Notes::default();
    let client = unpack_client(require_child(tree, CLIENT)?, &notes)?;
    let pipe = unpack_pipe(require_child(tree, PIPE)?, &notes)?;
    let triggers = unpack_triggers(tree.children.get(TRIGGERS), &notes)?;
    let extra = extra_of(&tree.attrs, &[], "", &notes)?;
    let mut config = OpcuaConfig {
        client,
        pipe,
        triggers,
        extra,
        warnings: None,
    };
    crate::checks::advise(&config, &mut notes.borrow_mut());
    config.warnings = Some(notes.into_inner());
    pack_v1(&config)?;
    Ok(config)
}
