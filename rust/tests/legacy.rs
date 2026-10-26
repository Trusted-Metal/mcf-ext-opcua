mod common;

use common::*;
use mcf_ext_opcua::{
    pack, peek_schema_version, unpack, AuthMode, CaseSensitivity, ClientConfig, Combinator,
    Condition, ErrorCode, Evaluation, ExtensionTree, MachineProfile, MinIntegrityLevel,
    OpcuaConfig, PackOptions, PipeConfig, QueuePolicy, RootNode, RuleAction, SamplingInterval,
    SecurityMode, SecurityPolicy, UserAccessLevel,
};
use serde_json::{json, Map, Value as Json};

const LEGACY: &str = "legacy/reference_config_opcua";

type Attrs = Vec<(&'static str, Json)>;

fn utf8(s: &str) -> Json {
    json!({"dtype": "utf8", "value": s})
}
fn i32v(n: i32) -> Json {
    json!({"dtype": "i32", "value": n})
}
fn i64v(n: i64) -> Json {
    json!({"dtype": "i64", "value": n.to_string()})
}
fn f64v(x: f64) -> Json {
    json!({"dtype": "f64", "value": x})
}

fn group(attrs: Map<String, Json>, children: Map<String, Json>) -> Json {
    json!({"attrs": attrs, "children": children, "datasets": {}})
}

fn merged(defaults: Attrs, overrides: Attrs) -> Map<String, Json> {
    let mut m: Map<String, Json> = defaults
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    for (k, v) in overrides {
        m.insert(k.to_string(), v);
    }
    m
}

fn triggers_json(groups: Vec<(&str, Attrs)>, attrs: Attrs) -> Json {
    let children = groups
        .into_iter()
        .map(|(name, a)| (name.to_string(), group(merged(vec![], a), Map::new())))
        .collect();
    group(merged(vec![], attrs), children)
}

fn legacy_json(client: Attrs, pipe: Attrs, triggers: Option<Json>) -> Json {
    let client_defaults = vec![
        ("Server_URL", utf8("opc.tcp://h:1")),
        ("Auth_Mode", utf8("Anonymous")),
        ("Security_Mode", utf8("None")),
        ("Security_Policy", utf8("None")),
        ("Sampling_Interval", i64v(250)),
    ];
    let mut children = Map::new();
    children.insert(
        "Client".into(),
        group(merged(client_defaults, client), Map::new()),
    );
    children.insert("Pipe".into(), group(merged(vec![], pipe), Map::new()));
    if let Some(t) = triggers {
        children.insert("Triggers".into(), t);
    }
    group(Map::new(), children)
}

fn legacy_tree(client: Attrs, pipe: Attrs, triggers: Option<Json>) -> ExtensionTree {
    tree_from(legacy_json(client, pipe, triggers))
}

fn plain() -> OpcuaConfig {
    unpack_ok(&legacy_tree(vec![], vec![], None))
}

#[test]
fn reference_fixture_is_detected_as_legacy() {
    assert_eq!(peek_schema_version(&fixture_tree(LEGACY)), "legacy");
}

#[test]
fn reference_fixture_unpack_equals_model_json() {
    assert_eq!(unpack_ok(&fixture_tree(LEGACY)), fixture_model(LEGACY));
}

#[test]
fn reference_fixture_pins_the_values_read_from_the_h5() {
    let m = fixture_model(LEGACY);
    assert_eq!(
        m.client.server_url,
        "opc.tcp://172.17.20.240:62541/TM_OPCUA_DevTemplate_V0.1/TelemetryServer"
    );
    assert_eq!(m.client.auth_mode, AuthMode::UsernamePassword);
    let labels: Vec<&str> = m.triggers.rules.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(
        labels,
        [
            "Chamber Oxygen Level - Start",
            "Chamber Oxygen Level - Stop",
            "Laser Emission Interlock - Start",
            "Laser Emission Interlock - Stop",
        ]
    );
    let ids: Vec<u32> = m.triggers.rules.iter().map(|r| r.id).collect();
    assert_eq!(ids, [1, 2, 3, 4]);
}

#[test]
fn reference_fixture_unpack_pack_unpack_is_stable_except_warnings() {
    let first = unpack_ok(&fixture_tree(LEGACY));
    let packed = pack_ok(&first);
    assert_eq!(peek_schema_version(&packed), "1.2");
    let second = unpack_ok(&packed);
    assert_eq!(second.warnings, None);
    let mut expected = first;
    expected.warnings = None;
    assert_eq!(second, expected);
}

#[test]
fn applies_documented_defaults_to_missing_optional_attributes() {
    let c = plain();
    assert_eq!(
        c.client,
        ClientConfig {
            server_url: "opc.tcp://h:1".into(),
            auth_mode: AuthMode::Anonymous,
            machine_profile: None,
            security_mode: SecurityMode::None,
            security_policy: SecurityPolicy::None,
            root_node: RootNode::AutoDiscover,
            sampling_interval: SamplingInterval::Ms { ms: 250 },
            publish_interval: 250,
            bfs_max_depth: 16,
            keep_alive_count: 240,
            lifetime_count: 2400,
            queue_policy: QueuePolicy::DiscardOldest,
            queue_size_data_change: 10,
            queue_size_events: 10,
            reconnect_interval: 1000,
            session_timeout: 60000,
            sync_loop_interval_initial: 1000,
            sync_loop_interval_settled: 30000,
            extra: Default::default(),
        }
    );
    assert_eq!(
        c.pipe,
        PipeConfig {
            enabled: false,
            configure_client: false,
            buffer_size: 65536,
            inbound_rate_limit_hz: Some(100),
            max_inbound_message_size: 4096,
            min_integrity_level: MinIntegrityLevel::ProtectedProcess,
            pipe_name: "\\\\.\\pipe\\opc_ua_client_pipe".into(),
            user_access_level: UserAccessLevel::CurrentUserOnly,
            extra: Default::default(),
        }
    );
    assert!(!c.triggers.enabled);
    assert!(c.triggers.rules.is_empty());
    assert_eq!(
        c.warnings,
        Some(vec![
            "Client/@Security_Policy: \"None\" is read as the no-security policy (earlier codec versions read it as Auto)"
                .to_string()
        ])
    );
}

#[test]
fn accepts_integers_as_any_integral_dtype() {
    let c = unpack_ok(&legacy_tree(
        vec![
            ("BFS_Max_Depth", i64v(8)),
            ("Publish_Interval", f64v(500.0)),
            ("Session_Timeout", i32v(1234)),
        ],
        vec![("Pipe_Enabled", f64v(1.0)), ("Buffer_Size", i64v(2048))],
        Some(triggers_json(
            vec![],
            vec![
                ("Triggers_Enabled", f64v(1.0)),
                ("Trigger_Stop_Ceiling_Layers", i64v(7)),
            ],
        )),
    ));
    assert_eq!(c.client.bfs_max_depth, 8);
    assert_eq!(c.client.publish_interval, 500);
    assert_eq!(c.client.session_timeout, 1234);
    assert!(c.pipe.enabled);
    assert_eq!(c.pipe.buffer_size, 2048);
    assert!(c.triggers.enabled);
    assert_eq!(c.triggers.stop_ceiling_layers, Some(7));
}

#[test]
fn maps_legacy_vocabulary() {
    let client = |attrs: Attrs| unpack_ok(&legacy_tree(attrs, vec![], None)).client;
    let pipe = |attrs: Attrs| unpack_ok(&legacy_tree(vec![], attrs, None)).pipe;

    assert_eq!(
        client(vec![("Machine_Profile", utf8("eos"))]).machine_profile,
        Some(MachineProfile::Eos)
    );
    assert_eq!(
        client(vec![("Machine_Profile", utf8("ACONITY"))]).machine_profile,
        Some(MachineProfile::Aconity)
    );
    assert_eq!(
        client(vec![("Machine_Profile", utf8(""))]).machine_profile,
        None
    );
    assert_eq!(
        client(vec![("Queue_Policy", utf8("DropOldest"))]).queue_policy,
        QueuePolicy::DiscardOldest
    );
    assert_eq!(
        client(vec![("Queue_Policy", utf8("KeepOldest"))]).queue_policy,
        QueuePolicy::KeepOldest
    );
    assert_eq!(
        client(vec![("Security_Policy", utf8(""))]).security_policy,
        SecurityPolicy::Auto
    );
    assert_eq!(
        client(vec![("Security_Policy", utf8("Basic256Sha256"))]).security_policy,
        SecurityPolicy::Basic256Sha256
    );

    let levels = [
        ("0x0", MinIntegrityLevel::Untrusted),
        ("0x0000", MinIntegrityLevel::Untrusted),
        ("0x1000", MinIntegrityLevel::Low),
        ("0x2000", MinIntegrityLevel::Medium),
        ("0x3000", MinIntegrityLevel::High),
        ("0x4000", MinIntegrityLevel::System),
        ("0x5000", MinIntegrityLevel::ProtectedProcess),
        ("low", MinIntegrityLevel::Low),
    ];
    for (hex, level) in levels {
        assert_eq!(
            pipe(vec![("Min_Integrity_Level", utf8(hex))]).min_integrity_level,
            level,
            "{hex}"
        );
    }
}

#[test]
fn inbound_rate_limit_0_and_minus_1_mean_no_cap() {
    let rate = |n| {
        unpack_ok(&legacy_tree(
            vec![],
            vec![("Inbound_Rate_Limit", i32v(n))],
            None,
        ))
        .pipe
        .inbound_rate_limit_hz
    };
    assert_eq!(rate(0), None);
    assert_eq!(rate(-1), None);
    assert_eq!(rate(30), Some(30));
}

#[test]
fn splits_a_trigger_into_start_and_stop_rules_with_sequential_ids_in_label_order() {
    let c = unpack_ok(&legacy_tree(
        vec![],
        vec![],
        Some(triggers_json(
            vec![
                (
                    "B/x",
                    vec![
                        ("Signal", utf8("a/b")),
                        ("Start_Value", utf8("1")),
                        ("Stop_Value", utf8("2")),
                        ("Rule_Enabled", i32v(1)),
                        ("Cooldown_Period", i32v(40)),
                        ("Max_Fires_Per_Job", i32v(2)),
                        ("Case_Sensitivity", utf8("Contains")),
                        ("Event", utf8("E")),
                        ("Subsystem", utf8("S")),
                        ("Component", utf8("C")),
                        ("ID", utf8("t1")),
                        ("Trigger_Label", utf8("B/x")),
                    ],
                ),
                (
                    "A",
                    vec![
                        ("Signal", utf8("s")),
                        ("Start_Value", utf8("go")),
                        ("Stop_Value", utf8("")),
                    ],
                ),
            ],
            vec![],
        )),
    ));

    let summary: Vec<(u32, &str, RuleAction)> = c
        .triggers
        .rules
        .iter()
        .map(|r| (r.id, r.label.as_str(), r.action))
        .collect();
    assert_eq!(
        summary,
        [
            (1, "A - Start", RuleAction::CaptureStart),
            (2, "B_x - Start", RuleAction::CaptureStart),
            (3, "B_x - Stop", RuleAction::CaptureStop),
        ]
    );

    let [a, b_start, b_stop] = &c.triggers.rules[..] else {
        panic!("expected 3 rules")
    };
    assert!(!a.enabled);
    assert_eq!((a.cooldown_ms, a.priority), (0, 0));
    assert_eq!(a.combinator, Some(Combinator::And));
    assert_eq!(a.description, "");
    assert_eq!(a.max_fires_per_job, None);
    assert!(b_start.enabled);
    assert_eq!(
        (b_start.cooldown_ms, b_start.max_fires_per_job),
        (40, Some(2))
    );
    assert_eq!(
        b_stop.conditions,
        [Condition {
            id: 1,
            label: "a_b".into(),
            enabled: true,
            event: "E".into(),
            subsystem: "S".into(),
            component: "C".into(),
            signal: "a/b".into(),
            evaluation: Evaluation::String {
                value: "2".into(),
                case_sensitivity: CaseSensitivity::Contains,
            },
            extra: Default::default(),
        }]
    );
    assert_eq!(
        a.conditions[0].evaluation,
        Evaluation::String {
            value: "go".into(),
            case_sensitivity: CaseSensitivity::Exact,
        }
    );
}

#[test]
fn reports_skipped_sides_as_warnings() {
    let c = unpack_ok(&legacy_tree(
        vec![],
        vec![],
        Some(triggers_json(
            vec![
                (
                    "NoStop",
                    vec![
                        ("Signal", utf8("s")),
                        ("Start_Value", utf8("1")),
                        ("Stop_Value", utf8("")),
                    ],
                ),
                (
                    "NoSignal",
                    vec![
                        ("Signal", utf8("")),
                        ("Start_Value", utf8("1")),
                        ("Stop_Value", utf8("2")),
                    ],
                ),
            ],
            vec![],
        )),
    ));
    let labels: Vec<&str> = c.triggers.rules.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels, ["NoStop - Start"]);
    let skipped: Vec<String> = c
        .warnings
        .unwrap()
        .into_iter()
        .filter(|w| w.contains("skipped rule"))
        .collect();
    assert_eq!(
        skipped,
        [
            "Triggers/NoSignal: skipped rule \"NoSignal - Start\": legacy trigger has no signal or value for that action",
            "Triggers/NoSignal: skipped rule \"NoSignal - Stop\": legacy trigger has no signal or value for that action",
            "Triggers/NoStop: skipped rule \"NoStop - Stop\": legacy trigger has no signal or value for that action",
        ]
    );
}

#[test]
fn none_policy_reads_as_no_security_with_a_warning() {
    let c = plain();
    assert_eq!(c.client.security_policy, SecurityPolicy::None);
    let ws = c.warnings.unwrap();
    assert!(
        ws.iter().any(|w| w
            .starts_with("Client/@Security_Policy: \"None\" is read as the no-security policy")),
        "{ws:#?}"
    );
}

#[test]
fn keeps_unknown_attributes_in_extra_with_trigger_level_ones_on_each_rule() {
    let c = unpack_ok(&legacy_tree(
        vec![("Client_Future", i32v(1))],
        vec![("Pipe_Future", utf8("p"))],
        Some(triggers_json(
            vec![(
                "T",
                vec![
                    ("Signal", utf8("s")),
                    ("Start_Value", utf8("1")),
                    ("Stop_Value", utf8("2")),
                    ("Trig_Future", i64v(5)),
                ],
            )],
            vec![("Triggers_Future", f64v(2.5))],
        )),
    ));
    let json_of = |m: &std::collections::BTreeMap<String, mcf_ext_opcua::Attr>| {
        serde_json::to_value(m).unwrap()
    };
    assert_eq!(json_of(&c.client.extra), json!({"Client_Future": i32v(1)}));
    assert_eq!(json_of(&c.pipe.extra), json!({"Pipe_Future": utf8("p")}));
    assert_eq!(
        json_of(&c.triggers.extra),
        json!({"Triggers_Future": f64v(2.5)})
    );
    for r in &c.triggers.rules {
        assert_eq!(json_of(&r.extra), json!({"Trig_Future": i64v(5)}));
    }

    let packed = pack_ok(&c);
    let attr = &packed.children["Trigger"].children["T - Stop"].attrs["Trig_Future"];
    assert_eq!(serde_json::to_value(attr).unwrap(), i64v(5));
}

#[test]
fn rejects_invalid_legacy_data_with_a_path() {
    let err =
        |tree: ExtensionTree, path: &str| expect_err(unpack(&tree), ErrorCode::Validation, path);

    let mut no_url = legacy_json(vec![], vec![], None);
    remove(&mut no_url, "/children/Client/attrs/Server_URL");
    err(tree_from(no_url), "Client/@Server_URL");

    err(
        legacy_tree(vec![("Auth_Mode", utf8("Nope"))], vec![], None),
        "Client/@Auth_Mode",
    );
    err(
        legacy_tree(vec![("Sampling_Interval", i64v(-5))], vec![], None),
        "Client/@Sampling_Interval",
    );
    err(
        legacy_tree(vec![("Publish_Interval", f64v(1.5))], vec![], None),
        "Client/@Publish_Interval",
    );
    err(
        legacy_tree(vec![("Publish_Interval", utf8("5"))], vec![], None),
        "Client/@Publish_Interval",
    );
    err(
        legacy_tree(vec![("Publish_Interval", i64v(-1))], vec![], None),
        "Client/@Publish_Interval",
    );
    err(
        legacy_tree(vec![], vec![("Pipe_Enabled", i32v(2))], None),
        "Pipe/@Pipe_Enabled",
    );
    err(
        legacy_tree(vec![], vec![("Pipe_Name", utf8("x"))], None),
        "Pipe/@Pipe_Name",
    );
    err(
        legacy_tree(
            vec![],
            vec![],
            Some(triggers_json(vec![], vec![("Triggers_Enabled", f64v(0.5))])),
        ),
        "Triggers/@Triggers_Enabled",
    );
    err(
        legacy_tree(
            vec![],
            vec![],
            Some(triggers_json(
                vec![("T", vec![("Rule_Enabled", i32v(3))])],
                vec![],
            )),
        ),
        "Triggers/T/@Rule_Enabled",
    );
    err(
        legacy_tree(
            vec![],
            vec![],
            Some(triggers_json(
                vec![("T", vec![("Case_Sensitivity", utf8("x"))])],
                vec![],
            )),
        ),
        "Triggers/T/@Case_Sensitivity",
    );

    let mut no_pipe = legacy_json(vec![], vec![], None);
    remove(&mut no_pipe, "/children/Pipe");
    err(tree_from(no_pipe), "Pipe");
}

#[test]
fn legacy_packing_is_not_supported() {
    let r = pack(
        &plain(),
        &PackOptions {
            target_schema: Some("legacy".into()),
        },
    );
    expect_err(r, ErrorCode::LossyDowngrade, "");
}
