//! Pack/unpack codec for the OPC UA extension (`TM_OPCUA`) of a machine configuration file.
//!
//! Pure functions, no I/O. The extension tree types are declared structurally in [`contract`];
//! machine-config core is never a dependency.
//!
//! - [`unpack`] reads any `1.x` tree or the legacy layout (no schema version attribute) into an
//!   [`OpcuaConfig`].
//! - [`pack`] always writes the current schema ([`CURRENT_SCHEMA_VERSION`]).

mod adapters;
pub mod contract;
mod defaults;
mod errors;
mod fields;
mod model;
mod number;
mod schema_v1;

pub use contract::{
    Attr, Dataset, Dtype, ExtensionTree, Scalar, Value, EXTENSION_NAME, REQUIRES_CONTRACT,
};
pub use defaults::{
    suggested_defaults, SuggestedClientDefaults, SuggestedDefaults, SuggestedPipeDefaults,
};
pub use errors::{ErrorCode, ExtensionError, Result};
pub use model::*;
pub use schema_v1::CURRENT_SCHEMA_VERSION;

use adapters::legacy::unpack_legacy;
use adapters::v1::{pack_v1, unpack_v1};
use errors::validation;
use number::js_number_to_string;
use schema_v1::SCHEMA_VERSION_ATTR;

#[derive(Debug, Clone, Default)]
pub struct PackOptions {
    /// Defaults to [`CURRENT_SCHEMA_VERSION`].
    pub target_schema: Option<String>,
}

/// Schema versions `pack` can write. The legacy layout is read-only and not a version.
pub fn supported_schema_versions() -> Vec<&'static str> {
    vec![CURRENT_SCHEMA_VERSION]
}

fn scalar_text(s: &Scalar) -> String {
    let float = |v: f64| {
        if v.is_nan() {
            "NaN".to_string()
        } else if v.is_infinite() {
            if v > 0.0 { "Infinity" } else { "-Infinity" }.to_string()
        } else {
            js_number_to_string(v)
        }
    };
    match s {
        Scalar::Utf8(v) => v.clone(),
        Scalar::I8(v) => v.to_string(),
        Scalar::U8(v) => v.to_string(),
        Scalar::I16(v) => v.to_string(),
        Scalar::U16(v) => v.to_string(),
        Scalar::I32(v) => v.to_string(),
        Scalar::U32(v) => v.to_string(),
        Scalar::I64(v) => v.to_string(),
        Scalar::U64(v) => v.to_string(),
        Scalar::F32(v) => float(f64::from(*v)),
        Scalar::F64(v) => float(*v),
    }
}

/// The root version attribute as stored; `"legacy"` when it is absent.
pub fn peek_schema_version(tree: &ExtensionTree) -> String {
    match tree.attrs.get(SCHEMA_VERSION_ATTR) {
        None => "legacy".to_string(),
        Some(a) => match &a.value {
            Value::Scalar(s) => scalar_text(s),
            Value::Array(items) => items.iter().map(scalar_text).collect::<Vec<_>>().join(","),
        },
    }
}

/// Parses `<major>.<minor>`; the inner `None` means the major does not fit in a `u64`.
fn major_of(version: &str, path: &str) -> Result<Option<u64>> {
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    match version.split_once('.') {
        Some((major, minor)) if digits(major) && digits(minor) => Ok(major.parse().ok()),
        _ => Err(validation(
            path,
            format!("invalid schema version \"{version}\"; expected \"<major>.<minor>\""),
        )),
    }
}

fn minor_of(version: &str) -> u32 {
    version.split_once('.').and_then(|(_, m)| m.parse().ok()).unwrap_or(0)
}

/// Unpacks a `1.x` tree (unknown attributes go to `extra`) or the legacy layout.
pub fn unpack(tree: &ExtensionTree) -> Result<OpcuaConfig> {
    let Some(attr) = tree.attrs.get(SCHEMA_VERSION_ATTR) else {
        return unpack_legacy(tree);
    };
    let path = format!("@{SCHEMA_VERSION_ATTR}");
    let (Dtype::Utf8, Value::Scalar(Scalar::Utf8(version))) = (attr.dtype, &attr.value) else {
        return Err(validation(path, "expected a utf8 scalar"));
    };
    if major_of(version, &path)? != Some(1) {
        return Err(ExtensionError::new(
            ErrorCode::UnsupportedSchemaVersion,
            path,
            format!("unsupported schema version \"{version}\""),
        ));
    }
    unpack_v1(tree, minor_of(version))
}

/// Packs to `opts.target_schema` (default: current). Packing the legacy layout is not
/// implemented and returns `LossyDowngrade`.
pub fn pack(config: &OpcuaConfig, opts: &PackOptions) -> Result<ExtensionTree> {
    let target = opts
        .target_schema
        .as_deref()
        .unwrap_or(CURRENT_SCHEMA_VERSION);
    if target == "legacy" {
        return Err(ExtensionError::new(
            ErrorCode::LossyDowngrade,
            "",
            "packing to the legacy layout is not implemented",
        ));
    }
    if target != CURRENT_SCHEMA_VERSION {
        return Err(ExtensionError::new(
            ErrorCode::UnsupportedSchemaVersion,
            "",
            format!("cannot pack schema version \"{target}\"; supported: {CURRENT_SCHEMA_VERSION}"),
        ));
    }
    pack_v1(config)
}
