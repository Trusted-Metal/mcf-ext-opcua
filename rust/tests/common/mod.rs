#![allow(dead_code)]

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};

use mcf_ext_opcua::{ErrorCode, ExtensionError, ExtensionTree, OpcuaConfig, PackOptions};
use serde_json::Value as Json;

pub fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../fixtures")
}

fn read(rel: &str, file: &str) -> Json {
    let text = fs::read_to_string(fixtures().join(rel).join(file)).unwrap();
    serde_json::from_str(&text).unwrap()
}

pub fn tree_json(rel: &str) -> Json {
    read(rel, "tree.json")
}

pub fn tree_from(v: Json) -> ExtensionTree {
    serde_json::from_value(v).unwrap_or_else(|e| panic!("invalid tree json: {e}"))
}

pub fn fixture_tree(rel: &str) -> ExtensionTree {
    tree_from(tree_json(rel))
}

pub fn fixture_model(rel: &str) -> OpcuaConfig {
    serde_json::from_value(read(rel, "model.json"))
        .unwrap_or_else(|e| panic!("{rel}/model.json: {e}"))
}

pub fn rich_json() -> Json {
    tree_json("v1_2/rich")
}

pub fn rich_tree() -> ExtensionTree {
    fixture_tree("v1_2/rich")
}

pub fn rich_model() -> OpcuaConfig {
    fixture_model("v1_2/rich")
}

pub fn pack_ok(config: &OpcuaConfig) -> ExtensionTree {
    mcf_ext_opcua::pack(config, &PackOptions::default()).unwrap_or_else(|e| panic!("{e}"))
}

pub fn unpack_ok(tree: &ExtensionTree) -> OpcuaConfig {
    mcf_ext_opcua::unpack(tree).unwrap_or_else(|e| panic!("{e}"))
}

pub fn expect_err<T: Debug>(r: Result<T, ExtensionError>, code: ErrorCode, path: &str) {
    let e = r.expect_err("expected an error");
    assert_eq!((e.code, e.path.as_str()), (code, path), "{e}");
}

/// Escapes a name for use as a JSON pointer segment.
pub fn esc(name: &str) -> String {
    name.replace('~', "~0").replace('/', "~1")
}

fn unescape(segment: &str) -> String {
    segment.replace("~1", "/").replace("~0", "~")
}

/// Inserts or replaces the value at `pointer`; the parent object must exist.
pub fn set(v: &mut Json, pointer: &str, new: Json) {
    let (parent, last) = pointer.rsplit_once('/').unwrap();
    v.pointer_mut(parent)
        .and_then(Json::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {parent}"))
        .insert(unescape(last), new);
}

pub fn remove(v: &mut Json, pointer: &str) {
    let (parent, last) = pointer.rsplit_once('/').unwrap();
    v.pointer_mut(parent)
        .and_then(Json::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {parent}"))
        .remove(&unescape(last))
        .unwrap_or_else(|| panic!("nothing to remove at {pointer}"));
}

pub fn rename_child(v: &mut Json, parent_pointer: &str, from: &str, to: &str) {
    let children = v
        .pointer_mut(parent_pointer)
        .and_then(Json::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {parent_pointer}"));
    let moved = children.remove(from).unwrap();
    children.insert(to.to_string(), moved);
}
