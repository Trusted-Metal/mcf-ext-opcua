mod common;

use std::fs;

use common::*;
use mcf_ext_opcua::{peek_schema_version, unpack, Scalar, Value};

fn v1_2_cases() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(fixtures().join("v1_2"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

// Trees written in canonical form: pack(model) must reproduce them exactly.
const CANONICAL: &[&str] = &["minimal", "rich"];

#[test]
fn has_at_least_three_cases() {
    assert!(v1_2_cases().len() >= 3);
}

#[test]
fn unpack_tree_equals_model() {
    for name in v1_2_cases() {
        let rel = format!("v1_2/{name}");
        assert_eq!(
            unpack_ok(&fixture_tree(&rel)),
            fixture_model(&rel),
            "{name}"
        );
    }
}

#[test]
fn pack_model_re_unpacks_to_model() {
    for name in v1_2_cases() {
        let rel = format!("v1_2/{name}");
        let packed = pack_ok(&fixture_model(&rel));
        assert_eq!(peek_schema_version(&packed), "1.2", "{name}");
        assert_eq!(unpack_ok(&packed), fixture_model(&rel), "{name}");
    }
}

#[test]
fn pack_model_equals_canonical_tree() {
    for name in CANONICAL {
        let rel = format!("v1_2/{name}");
        assert_eq!(pack_ok(&fixture_model(&rel)), fixture_tree(&rel), "{name}");
    }
}

#[test]
fn lenient_read_normalises_casing_sentinels_ordering_and_unknown_minor() {
    let tree = fixture_tree("v1_2/lenient_read");
    assert_eq!(peek_schema_version(&tree), "1.7");
    assert!(unpack(&tree).is_ok());

    let packed = pack_ok(&fixture_model("v1_2/lenient_read"));
    let text = |g: &str, n: &str| match &packed.children[g].attrs[n].value {
        Value::Scalar(Scalar::Utf8(s)) => s.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(text("Client", "Machine_Profile"), "Aconity");
    assert_eq!(text("Pipe", "Min_Integrity_Level"), "High");
    assert_eq!(
        packed.children["Pipe"].attrs["Inbound_Rate_Limit"].value,
        Value::Scalar(Scalar::I32(0))
    );
    assert_eq!(
        packed.attrs["Opcua_Schema_Version"].value,
        Value::Scalar(Scalar::Utf8("1.2".to_string()))
    );
}
