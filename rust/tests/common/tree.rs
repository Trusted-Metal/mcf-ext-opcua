//! Typed `ExtensionTree` editing (the counterpart of mod.rs's JSON helpers) and warning filters,
//! shared by tiers.rs and checks.rs.
#![allow(dead_code)]

use mcf_ext_opcua::{Attr, Dtype, ExtensionTree, Scalar, Value, Warning, WarningKind};

pub fn utf8(s: &str) -> Attr {
    Attr {
        dtype: Dtype::Utf8,
        value: Value::Scalar(Scalar::Utf8(s.to_string())),
    }
}

pub fn u32_attr(n: u32) -> Attr {
    Attr {
        dtype: Dtype::U32,
        value: Value::Scalar(Scalar::U32(n)),
    }
}

pub fn i32_attr(n: i32) -> Attr {
    Attr {
        dtype: Dtype::I32,
        value: Value::Scalar(Scalar::I32(n)),
    }
}

pub fn f64_attr(x: f64) -> Attr {
    Attr {
        dtype: Dtype::F64,
        value: Value::Scalar(Scalar::F64(x)),
    }
}

pub fn group<'a>(tree: &'a mut ExtensionTree, path: &[&str]) -> &'a mut ExtensionTree {
    path.iter()
        .fold(tree, |t, name| t.children.get_mut(*name).unwrap())
}

pub fn set(tree: &mut ExtensionTree, path: &[&str], name: &str, attr: Attr) {
    group(tree, path).attrs.insert(name.to_string(), attr);
}

/// Removes an attribute and returns it (so a test can re-insert it under another name).
pub fn remove(tree: &mut ExtensionTree, path: &[&str], name: &str) -> Attr {
    group(tree, path)
        .attrs
        .remove(name)
        .unwrap_or_else(|| panic!("{name} not in fixture"))
}

pub fn rename(tree: &mut ExtensionTree, path: &[&str], from: &str, to: &str) {
    let attr = remove(tree, path, from);
    set(tree, path, to, attr);
}

/// Warnings of one kind, rendered as "path: message".
pub fn of_kind(ws: &Option<Vec<Warning>>, kind: WarningKind) -> Vec<String> {
    ws.iter()
        .flatten()
        .filter(|w| w.kind == kind)
        .map(Warning::to_string)
        .collect()
}

pub fn has(ws: &[String], needle: &str) -> bool {
    ws.iter().any(|w| w.contains(needle))
}
