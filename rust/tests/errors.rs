mod common;

use common::*;
use mcf_ext_opcua::{
    pack, unpack, Attr, Dtype, ErrorCode, Evaluation, OpcuaConfig, PackOptions, RootNode,
    SamplingInterval, Scalar, Value,
};
use serde_json::{json, Value as Json};

use ErrorCode::{UnsupportedSchemaVersion, Validation};

fn unpack_mutated(
    edit: impl FnOnce(&mut Json),
) -> Result<OpcuaConfig, mcf_ext_opcua::ExtensionError> {
    let mut v = rich_json();
    edit(&mut v);
    unpack(&tree_from(v))
}

fn check_unpack(code: ErrorCode, path: &str, edit: impl FnOnce(&mut Json)) {
    expect_err(unpack_mutated(edit), code, path);
}

const CLIENT: &str = "/children/Client/attrs";
const PIPE: &str = "/children/Pipe/attrs";
const START: &str = "/children/Trigger/children/Job Start";
const STOP: &str = "/children/Trigger/children/Job Stop";

fn utf8(s: &str) -> Json {
    json!({"dtype": "utf8", "value": s})
}

fn u32v(n: u32) -> Json {
    json!({"dtype": "u32", "value": n})
}

#[test]
fn rich_fixture_is_valid() {
    assert!(unpack(&rich_tree()).is_ok());
}

#[test]
fn missing_strict_attributes() {
    check_unpack(Validation, "Client/@Server_URL", |v| {
        remove(v, &format!("{CLIENT}/Server_URL"))
    });
    check_unpack(Validation, "Pipe/@Pipe_Name", |v| {
        remove(v, &format!("{PIPE}/Pipe_Name"))
    });
    check_unpack(Validation, "Trigger/Job Stop/@Action", |v| {
        remove(v, &format!("{STOP}/attrs/Action"))
    });
    check_unpack(Validation, "Trigger/Job Start/Laser On/@Value", |v| {
        remove(v, &format!("{START}/children/Laser On/attrs/Value"))
    });
}

#[test]
fn missing_groups() {
    check_unpack(Validation, "Client", |v| remove(v, "/children/Client"));
    check_unpack(Validation, "Trigger", |v| remove(v, "/children/Trigger"));
}

#[test]
fn bad_enum_values() {
    check_unpack(Validation, "Client/@Queue_Policy", |v| {
        set(v, &format!("{CLIENT}/Queue_Policy"), utf8("Kerberos"))
    });
    check_unpack(
        Validation,
        "Trigger/Job Start/Laser On/@Case_Sensitivity",
        |v| {
            set(
                v,
                &format!("{START}/children/Laser On/attrs/Case_Sensitivity"),
                utf8("Fuzzy"),
            )
        },
    );
    // User_Access_Level is case-sensitive.
    check_unpack(Validation, "Pipe/@User_Access_Level", |v| {
        set(v, &format!("{PIPE}/User_Access_Level"), utf8("Everyone"))
    });
    check_unpack(Validation, "Client/@Queue_Policy", |v| {
        set(v, &format!("{CLIENT}/Queue_Policy"), utf8("Lifo"))
    });
}

#[test]
fn flag_value_2() {
    check_unpack(Validation, "Pipe/@Pipe_Enabled", |v| {
        set(v, &format!("{PIPE}/Pipe_Enabled"), u32v(2))
    });
    check_unpack(
        Validation,
        "Trigger/Job Start/Laser On/@Condition_Enabled",
        |v| {
            set(
                v,
                &format!("{START}/children/Laser On/attrs/Condition_Enabled"),
                u32v(2),
            )
        },
    );
}

#[test]
fn wrong_dtype() {
    check_unpack(Validation, "Client/@Publish_Interval", |v| {
        set(
            v,
            &format!("{CLIENT}/Publish_Interval"),
            json!({"dtype": "utf8", "value": "5"}),
        )
    });
}

#[test]
fn id_zero_and_duplicates() {
    check_unpack(Validation, "Trigger/Job Start/@Rule_Id", |v| {
        set(v, &format!("{START}/attrs/Rule_Id"), u32v(0))
    });
    check_unpack(
        Validation,
        "Trigger/Job Start/Laser On/@Condition_Id",
        |v| {
            set(
                v,
                &format!("{START}/children/Laser On/attrs/Condition_Id"),
                u32v(0),
            )
        },
    );
    check_unpack(Validation, "Trigger/Job Stop/@Rule_Id", |v| {
        set(v, &format!("{STOP}/attrs/Rule_Id"), u32v(1))
    });
    check_unpack(
        Validation,
        "Trigger/Job Start/Oxygen High/@Condition_Id",
        |v| {
            set(
                v,
                &format!("{START}/children/Oxygen High/attrs/Condition_Id"),
                u32v(1),
            )
        },
    );
}

#[test]
fn label_rules() {
    check_unpack(Validation, "Trigger/a/b", |v| {
        rename_child(v, "/children/Trigger/children", "Job Stop", "a/b");
        set(
            v,
            &format!("/children/Trigger/children/{}/attrs/Rule_Label", esc("a/b")),
            utf8("a/b"),
        );
    });
    check_unpack(Validation, "Trigger/x\0y", |v| {
        rename_child(v, "/children/Trigger/children", "Job Stop", "x\0y")
    });
    check_unpack(Validation, "Trigger/Job Stop/@Rule_Label", |v| {
        set(v, &format!("{STOP}/attrs/Rule_Label"), utf8("Other"))
    });
    check_unpack(Validation, "Trigger/Job Start/bad/label", |v| {
        rename_child(v, &format!("{START}/children"), "Laser On", "bad/label")
    });
}

#[test]
fn bad_pipe_name() {
    for name in ["opc_ua_client_pipe", "\\\\.\\pipe\\"] {
        check_unpack(Validation, "Pipe/@Pipe_Name", |v| {
            set(v, &format!("{PIPE}/Pipe_Name"), utf8(name))
        });
    }
}

#[test]
fn bad_sampling_interval() {
    check_unpack(Validation, "Client/@Sampling_Interval", |v| {
        set(
            v,
            &format!("{CLIENT}/Sampling_Interval"),
            json!({"dtype": "i32", "value": -2}),
        )
    });
}

#[test]
fn numeric_condition_with_non_numeric_value() {
    for bad in [
        "abc", "", "0x10", "NaN", "Infinity", "1e999", " 5", "5e", ".",
    ] {
        check_unpack(Validation, "Trigger/Job Start/Oxygen High/@Value", |v| {
            set(
                v,
                &format!("{START}/children/Oxygen High/attrs/Value"),
                utf8(bad),
            )
        });
    }
}

#[test]
fn numeric_condition_accepts_literal_forms() {
    for (text, expected) in [
        ("5", 5.0),
        ("+5.", 5.0),
        (".5", 0.5),
        ("-1E3", -1000.0),
        ("1.5e-2", 0.015),
    ] {
        let c = unpack_mutated(|v| {
            set(
                v,
                &format!("{START}/children/Oxygen High/attrs/Value"),
                utf8(text),
            )
        })
        .unwrap();
        let rule = c
            .triggers
            .rules
            .iter()
            .find(|r| r.label == "Job Start")
            .unwrap();
        let cond = rule
            .conditions
            .iter()
            .find(|c| c.label == "Oxygen High")
            .unwrap();
        match &cond.evaluation {
            Evaluation::Numeric { threshold, .. } => assert_eq!(*threshold, expected, "{text}"),
            other => panic!("{other:?}"),
        }
    }
}

#[test]
fn schema_version_errors() {
    check_unpack(UnsupportedSchemaVersion, "@Opcua_Schema_Version", |v| {
        set(v, "/attrs/Opcua_Schema_Version", utf8("2.0"))
    });
    check_unpack(UnsupportedSchemaVersion, "@Opcua_Schema_Version", |v| {
        set(
            v,
            "/attrs/Opcua_Schema_Version",
            utf8("99999999999999999999.0"),
        )
    });
    for bad in ["one", "1", "1.", ".2", "1.2.3", "-1.0", "1.x"] {
        check_unpack(Validation, "@Opcua_Schema_Version", |v| {
            set(v, "/attrs/Opcua_Schema_Version", utf8(bad))
        });
    }
    check_unpack(Validation, "@Opcua_Schema_Version", |v| {
        set(v, "/attrs/Opcua_Schema_Version", u32v(1))
    });
}

#[test]
fn unexpected_groups_and_datasets_are_rejected_not_dropped() {
    check_unpack(Validation, "Surprise", |v| {
        set(
            v,
            "/children/Surprise",
            json!({"attrs": {}, "children": {}, "datasets": {}}),
        )
    });
    check_unpack(Validation, "Client/Nested", |v| {
        set(
            v,
            "/children/Client/children/Nested",
            json!({"attrs": {}, "children": {}, "datasets": {}}),
        )
    });
    check_unpack(Validation, "Pipe/Raw", |v| {
        set(
            v,
            "/children/Pipe/datasets/Raw",
            json!({"dtype": "f64", "shape": [1], "data": [1.0]}),
        )
    });
}

// ---- pack ----

fn pack_mutated(
    edit: impl FnOnce(&mut OpcuaConfig),
) -> Result<mcf_ext_opcua::ExtensionTree, mcf_ext_opcua::ExtensionError> {
    let mut c = rich_model();
    edit(&mut c);
    pack(&c, &PackOptions::default())
}

fn check_pack(path: &str, edit: impl FnOnce(&mut OpcuaConfig)) {
    expect_err(pack_mutated(edit), Validation, path);
}

#[test]
fn valid_rich_model_packs() {
    assert!(pack_mutated(|_| {}).is_ok());
}

#[test]
fn duplicate_rule_label_rule_id_condition_label_and_id() {
    check_pack("Trigger/Job Start", |c| {
        c.triggers.rules[1].label = "Job Start".into()
    });
    check_pack("Trigger/Job Stop/@Rule_Id", |c| c.triggers.rules[1].id = 1);
    check_pack("Trigger/Job Start/Laser On", |c| {
        c.triggers.rules[0].conditions[1].label = "Laser On".into()
    });
    check_pack("Trigger/Job Start/Oxygen High/@Condition_Id", |c| {
        c.triggers.rules[0].conditions[1].id = 1
    });
}

#[test]
fn slash_nul_and_empty_labels() {
    check_pack("Trigger/a/b", |c| c.triggers.rules[0].label = "a/b".into());
    check_pack("Trigger/a\0b", |c| {
        c.triggers.rules[0].label = "a\0b".into()
    });
    check_pack("Trigger/", |c| c.triggers.rules[0].label = String::new());
    check_pack("Trigger/Job Start/x/y", |c| {
        c.triggers.rules[0].conditions[0].label = "x/y".into()
    });
}

#[test]
fn ids_must_be_at_least_one() {
    check_pack("Trigger/Job Start/@Rule_Id", |c| c.triggers.rules[0].id = 0);
    check_pack("Trigger/Job Start/Laser On/@Condition_Id", |c| {
        c.triggers.rules[0].conditions[0].id = 0
    });
}

#[test]
fn bad_pipe_name_on_pack() {
    check_pack("Pipe/@Pipe_Name", |c| c.pipe.pipe_name = "nope".into());
}

#[test]
fn bad_sampling_interval_on_pack() {
    for ms in [0, i32::MAX as u32 + 1, u32::MAX] {
        check_pack("Client/@Sampling_Interval", |c| {
            c.client.sampling_interval = SamplingInterval::Ms { ms }
        });
    }
    assert!(
        pack_mutated(|c| c.client.sampling_interval = SamplingInterval::Ms {
            ms: i32::MAX as u32
        })
        .is_ok()
    );
}

#[test]
fn numeric_condition_needs_a_finite_threshold() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        check_pack("Trigger/Job Start/Oxygen High/@Value", |c| {
            if let Evaluation::Numeric { threshold, .. } =
                &mut c.triggers.rules[0].conditions[1].evaluation
            {
                *threshold = bad;
            }
        });
    }
    check_pack("Trigger/Job Start/Oxygen High/@Epsilon", |c| {
        if let Evaluation::Numeric { epsilon, .. } =
            &mut c.triggers.rules[0].conditions[1].evaluation
        {
            *epsilon = f64::NAN;
        }
    });
}

#[test]
fn counts_beyond_the_disk_range_are_rejected() {
    // Integer overflow cannot happen for u32 fields; the i32-backed ones can.
    check_pack("Pipe/@Inbound_Rate_Limit", |c| {
        c.pipe.inbound_rate_limit_hz = Some(i32::MAX as u32 + 1)
    });
}

#[test]
fn optional_numbers_must_be_positive_when_present() {
    check_pack("Pipe/@Inbound_Rate_Limit", |c| {
        c.pipe.inbound_rate_limit_hz = Some(0)
    });
    check_pack("Trigger/@Trigger_Stop_Ceiling_Layers", |c| {
        c.triggers.stop_ceiling_layers = Some(0)
    });
    check_pack("Trigger/Job Start/@Max_Fires_Per_Job", |c| {
        c.triggers.rules[0].max_fires_per_job = Some(0)
    });
}

#[test]
fn root_node_kinds_must_round_trip() {
    check_pack("Client/@Root_Node", |c| {
        c.client.root_node = RootNode::NodeId {
            value: "no-equals".into(),
        }
    });
    check_pack("Client/@Root_Node", |c| {
        c.client.root_node = RootNode::BrowseName {
            value: "a=b".into(),
        }
    });
    check_pack("Client/@Root_Node", |c| {
        c.client.root_node = RootNode::BrowseName {
            value: String::new(),
        }
    });
}

#[test]
fn extra_attributes_may_not_shadow_known_attributes_or_be_malformed() {
    let utf8_attr = |s: &str| Attr {
        dtype: Dtype::Utf8,
        value: Value::Scalar(Scalar::Utf8(s.into())),
    };
    check_pack("Client/@Server_URL", |c| {
        c.client.extra.insert("Server_URL".into(), utf8_attr("x"));
    });
    check_pack("@Opcua_Schema_Version", |c| {
        c.extra
            .insert("Opcua_Schema_Version".into(), utf8_attr("9.9"));
    });
    check_pack("Pipe/@Bad", |c| {
        c.pipe.extra.insert(
            "Bad".into(),
            Attr {
                dtype: Dtype::U32,
                value: Value::Scalar(Scalar::U64(1)),
            },
        );
    });
    check_pack("Pipe/@a/b", |c| {
        c.pipe.extra.insert(
            "a/b".into(),
            Attr {
                dtype: Dtype::U32,
                value: Value::Scalar(Scalar::U32(1)),
            },
        );
    });
    check_pack("Pipe/@Nul", |c| {
        c.pipe.extra.insert("Nul".into(), utf8_attr("a\0b"));
    });
}
