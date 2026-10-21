mod common;

use std::collections::BTreeMap;

use common::*;
use mcf_ext_opcua::{
    Attr, AuthMode, CaseSensitivity, ClientConfig, Combinator, ComparisonOperator, Condition,
    Dtype, Evaluation, ExtensionTree, MachineProfile, MinIntegrityLevel, OpcuaConfig, PipeConfig,
    QueuePolicy, RootNode, Rule, RuleAction, SamplingInterval, Scalar, SecurityMode,
    SecurityPolicy, TriggerConfig, UserAccessLevel, Value,
};

fn attr(dtype: Dtype, scalar: Scalar) -> Attr {
    Attr {
        dtype,
        value: Value::Scalar(scalar),
    }
}

fn extra(entries: Vec<(&str, Attr)>) -> BTreeMap<String, Attr> {
    entries
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect()
}

fn rich_config() -> OpcuaConfig {
    OpcuaConfig {
        client: ClientConfig {
            server_url: "opc.tcp://example:4840/srv".into(),
            auth_mode: AuthMode::Certificate,
            machine_profile: Some(MachineProfile::Aconity),
            security_mode: SecurityMode::Sign,
            security_policy: SecurityPolicy::EccBrainpoolP256r1,
            root_node: RootNode::BrowseName {
                value: "Fleet".into(),
            },
            sampling_interval: SamplingInterval::Ms { ms: 50 },
            publish_interval: 100,
            bfs_max_depth: 4,
            keep_alive_count: 10,
            lifetime_count: 30,
            queue_policy: QueuePolicy::KeepOldest,
            queue_size_data_change: 50,
            queue_size_events: 20,
            reconnect_interval: 2000,
            session_timeout: 45000,
            sync_loop_interval_initial: 100,
            sync_loop_interval_settled: 1000,
            extra: extra(vec![
                ("Client_Utf8", attr(Dtype::Utf8, Scalar::Utf8("x".into()))),
                ("Client_U64", attr(Dtype::U64, Scalar::U64(1 << 63))),
            ]),
        },
        pipe: PipeConfig {
            enabled: true,
            configure_client: false,
            buffer_size: 1024,
            inbound_rate_limit_hz: Some(60),
            max_inbound_message_size: 2048,
            min_integrity_level: MinIntegrityLevel::System,
            pipe_name: "\\\\.\\pipe\\rt".into(),
            user_access_level: UserAccessLevel::CurrentUserAndAdmins,
            extra: extra(vec![
                ("Pipe_F32", attr(Dtype::F32, Scalar::F32(0.5))),
                (
                    "Pipe_Arr",
                    Attr {
                        dtype: Dtype::Utf8,
                        value: Value::Array(vec![
                            Scalar::Utf8("a".into()),
                            Scalar::Utf8("b".into()),
                        ]),
                    },
                ),
            ]),
        },
        triggers: TriggerConfig {
            enabled: true,
            stop_ceiling_layers: Some(2),
            extra: extra(vec![("Trig_I8", attr(Dtype::I8, Scalar::I8(-3)))]),
            rules: vec![
                Rule {
                    id: 1,
                    label: "Start A".into(),
                    description: "first".into(),
                    action: RuleAction::CaptureStart,
                    combinator: Some(Combinator::Or),
                    enabled: true,
                    priority: 5,
                    cooldown_ms: 250,
                    max_fires_per_job: Some(2),
                    extra: extra(vec![("Rule_Extra", attr(Dtype::I32, Scalar::I32(-1)))]),
                    conditions: vec![
                        Condition {
                            id: 1,
                            label: "cond one".into(),
                            enabled: true,
                            event: "E".into(),
                            subsystem: "S".into(),
                            component: "C".into(),
                            signal: "sig".into(),
                            evaluation: Evaluation::String {
                                value: "go".into(),
                                case_sensitivity: CaseSensitivity::Contains,
                            },
                            extra: extra(vec![(
                                "Cond_Extra",
                                attr(Dtype::U16, Scalar::U16(65535)),
                            )]),
                        },
                        Condition {
                            id: 4,
                            label: "cond numeric".into(),
                            enabled: false,
                            event: String::new(),
                            subsystem: String::new(),
                            component: String::new(),
                            signal: "temp".into(),
                            evaluation: Evaluation::Numeric {
                                threshold: -0.125,
                                operator: ComparisonOperator::LessThanOrEqual,
                                epsilon: 1e-9,
                            },
                            extra: BTreeMap::new(),
                        },
                    ],
                },
                Rule {
                    id: 9,
                    label: "Stop A".into(),
                    description: String::new(),
                    action: RuleAction::CaptureStop,
                    combinator: None,
                    enabled: false,
                    priority: 0,
                    cooldown_ms: 0,
                    max_fires_per_job: None,
                    conditions: vec![],
                    extra: BTreeMap::new(),
                },
            ],
        },
        extra: extra(vec![("Root_Extra", attr(Dtype::I64, Scalar::I64(-9)))]),
        warnings: None,
    }
}

fn at<'a>(tree: &'a ExtensionTree, path: &[&str], name: &str) -> &'a Attr {
    let mut g = tree;
    for p in path {
        g = &g.children[*p];
    }
    &g.attrs[name]
}

#[test]
fn pack_then_unpack_equals_the_original() {
    let config = rich_config();
    assert_eq!(unpack_ok(&pack_ok(&config)), config);
}

#[test]
fn pack_is_deterministic() {
    assert_eq!(pack_ok(&rich_config()), pack_ok(&rich_config()));
}

#[test]
fn writes_the_documented_dtypes() {
    let t = pack_ok(&rich_config());
    let utf8 = |s: &str| attr(Dtype::Utf8, Scalar::Utf8(s.into()));
    let u32v = |n| attr(Dtype::U32, Scalar::U32(n));
    assert_eq!(t.attrs["Opcua_Schema_Version"], utf8("1.2"));
    assert_eq!(at(&t, &["Client"], "Server_URL").dtype, Dtype::Utf8);
    assert_eq!(
        at(&t, &["Client"], "Sampling_Interval"),
        &attr(Dtype::I32, Scalar::I32(50))
    );
    assert_eq!(at(&t, &["Client"], "Publish_Interval"), &u32v(100));
    assert_eq!(
        at(&t, &["Client"], "Security_Policy"),
        &utf8("ECC_brainpoolP256r1")
    );
    assert_eq!(at(&t, &["Pipe"], "Pipe_Enabled"), &u32v(1));
    assert_eq!(at(&t, &["Pipe"], "Configure_Client"), &u32v(0));
    assert_eq!(
        at(&t, &["Pipe"], "Inbound_Rate_Limit"),
        &attr(Dtype::I32, Scalar::I32(60))
    );
    assert_eq!(at(&t, &["Trigger"], "Triggers_Enabled"), &u32v(1));
    assert_eq!(
        at(&t, &["Trigger"], "Trigger_Stop_Ceiling_Layers"),
        &u32v(2)
    );
    assert_eq!(at(&t, &["Trigger", "Start A"], "Rule_Id"), &u32v(1));
    assert_eq!(at(&t, &["Trigger", "Start A"], "Combinator"), &utf8("Or"));
    assert_eq!(
        at(&t, &["Trigger", "Start A"], "Cooldown_Period"),
        &u32v(250)
    );
    let numeric = ["Trigger", "Start A", "cond numeric"];
    assert_eq!(
        at(&t, &numeric, "Epsilon"),
        &attr(Dtype::F64, Scalar::F64(1e-9))
    );
    assert_eq!(at(&t, &numeric, "Value"), &utf8("-0.125"));
    assert_eq!(at(&t, &numeric, "Evaluation_Mode"), &utf8("Numeric"));
    let string = ["Trigger", "Start A", "cond one"];
    assert_eq!(
        at(&t, &string, "Epsilon"),
        &attr(Dtype::F64, Scalar::F64(0.0))
    );
    assert_eq!(at(&t, &string, "Comparison_Operator"), &utf8("Equal"));
}

#[test]
fn writes_every_defaulted_attribute_and_the_sentinel_forms() {
    let t = pack_ok(&rich_config());
    let stop = &t.children["Trigger"].children["Stop A"];
    assert_eq!(
        stop.attrs["Combinator"],
        attr(Dtype::Utf8, Scalar::Utf8(String::new()))
    );
    assert_eq!(
        stop.attrs["Max_Fires_Per_Job"],
        attr(Dtype::U32, Scalar::U32(0))
    );
    assert_eq!(
        stop.attrs["Description"],
        attr(Dtype::Utf8, Scalar::Utf8(String::new()))
    );
    assert_eq!(stop.attrs.len(), 9);

    let mut c = rich_config();
    c.client.machine_profile = None;
    c.pipe.inbound_rate_limit_hz = None;
    c.triggers.stop_ceiling_layers = None;
    let t2 = pack_ok(&c);
    assert_eq!(
        at(&t2, &["Client"], "Machine_Profile"),
        &attr(Dtype::Utf8, Scalar::Utf8(String::new()))
    );
    assert_eq!(
        at(&t2, &["Pipe"], "Inbound_Rate_Limit"),
        &attr(Dtype::I32, Scalar::I32(0))
    );
    assert_eq!(
        at(&t2, &["Trigger"], "Trigger_Stop_Ceiling_Layers"),
        &attr(Dtype::U32, Scalar::U32(0))
    );
    assert_eq!(unpack_ok(&t2), c);
}

#[test]
fn security_policy_auto_is_written_as_empty() {
    let mut c = rich_config();
    c.client.security_policy = SecurityPolicy::Auto;
    let t = pack_ok(&c);
    assert_eq!(
        at(&t, &["Client"], "Security_Policy"),
        &attr(Dtype::Utf8, Scalar::Utf8(String::new()))
    );
    assert_eq!(unpack_ok(&t), c);
}

#[test]
fn keeps_the_dtype_of_unknown_attributes() {
    let t = pack_ok(&rich_config());
    assert_eq!(t.attrs["Root_Extra"], attr(Dtype::I64, Scalar::I64(-9)));
    assert_eq!(
        at(&t, &["Client"], "Client_U64"),
        &attr(Dtype::U64, Scalar::U64(1 << 63))
    );
    assert_eq!(
        at(&t, &["Pipe"], "Pipe_F32"),
        &attr(Dtype::F32, Scalar::F32(0.5))
    );
    assert_eq!(
        at(&t, &["Trigger"], "Trig_I8"),
        &attr(Dtype::I8, Scalar::I8(-3))
    );
    assert_eq!(
        at(&t, &["Trigger", "Start A", "cond one"], "Cond_Extra"),
        &attr(Dtype::U16, Scalar::U16(65535))
    );
}

#[test]
fn writes_rules_and_conditions_sorted_by_id() {
    let mut c = rich_config();
    c.triggers.rules.reverse();
    c.triggers.rules[1].conditions.reverse();
    let t = pack_ok(&c);
    // The tree is name-ordered, so check ids instead: unpack must return id order.
    let back = unpack_ok(&t);
    assert_eq!(
        back.triggers.rules.iter().map(|r| r.id).collect::<Vec<_>>(),
        [1, 9]
    );
    assert_eq!(
        back.triggers.rules[0]
            .conditions
            .iter()
            .map(|c| c.id)
            .collect::<Vec<_>>(),
        [1, 4]
    );
}

#[test]
fn unpack_then_pack_reproduces_a_packed_tree() {
    let first = pack_ok(&rich_config());
    let model = unpack_ok(&first);
    assert_eq!(pack_ok(&model), first);
}

#[test]
fn numeric_thresholds_use_the_shortest_round_trip_decimal() {
    let cases: [(f64, &str); 9] = [
        (700.0, "700"),
        (700.5, "700.5"),
        (1e21, "1e+21"),
        (1e-7, "1e-7"),
        (1e-6, "0.000001"),
        (-0.1 - 0.2, "-0.30000000000000004"),
        (123_456_789.123_456_79, "123456789.12345679"),
        (1.5e300, "1.5e+300"),
        (-0.0, "0"),
    ];
    for (threshold, text) in cases {
        let mut c = rich_config();
        c.triggers.rules[0].conditions[1].evaluation = Evaluation::Numeric {
            threshold,
            operator: ComparisonOperator::LessThanOrEqual,
            epsilon: 0.0,
        };
        let t = pack_ok(&c);
        assert_eq!(
            at(&t, &["Trigger", "Start A", "cond numeric"], "Value"),
            &attr(Dtype::Utf8, Scalar::Utf8(text.into())),
            "{threshold}"
        );
        assert_eq!(
            unpack_ok(&t).triggers.rules[0].conditions[1].evaluation,
            c.triggers.rules[0].conditions[1].evaluation,
            "{threshold}"
        );
    }
}
