use std::fs;
use std::path::{Path, PathBuf};

use mcf_ext_opcua::{ExtensionTree, Scalar, Value};
use serde_json::Value as Json;

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

fn tree_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            tree_files(&path, out);
        } else if path.file_name().is_some_and(|n| n == "tree.json") {
            out.push(path);
        }
    }
}

fn load(rel: &str) -> ExtensionTree {
    let text = fs::read_to_string(fixtures().join(rel).join("tree.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// `0` and `0.0` are the same value; the fixtures are written by JS, which has no distinction.
fn json_eq(a: &Json, b: &Json) -> bool {
    match (a, b) {
        (Json::Number(x), Json::Number(y)) => x.as_f64() == y.as_f64(),
        (Json::Array(x), Json::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(p, q)| json_eq(p, q))
        }
        (Json::Object(x), Json::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| json_eq(v, w)))
        }
        _ => a == b,
    }
}

#[test]
fn every_fixture_tree_round_trips() {
    let mut files = Vec::new();
    tree_files(&fixtures(), &mut files);
    assert!(
        files.len() >= 4,
        "expected the shared fixtures, found {files:?}"
    );

    for path in files {
        let original: Json = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        let tree: ExtensionTree = serde_json::from_value(original.clone())
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let again = serde_json::to_value(&tree).unwrap();
        assert!(
            json_eq(&original, &again),
            "{} did not round-trip",
            path.display()
        );
    }
}

#[test]
fn rich_fixture_values_have_the_right_types() {
    let tree = load("v1_2/rich");
    let client = &tree.children["Client"].attrs;
    assert_eq!(
        client["Client_Extra_I64"].value,
        Value::Scalar(Scalar::I64(-42))
    );
    assert_eq!(
        tree.children["Pipe"].attrs["Pipe_Extra_Array"].value,
        Value::Array(vec![Scalar::I32(1), Scalar::I32(2), Scalar::I32(3)])
    );
    assert_eq!(
        tree.children["Trigger"].attrs["Trigger_Extra"].value,
        Value::Scalar(Scalar::F64(1.5))
    );
}
