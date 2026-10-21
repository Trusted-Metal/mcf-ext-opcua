//! Structural copy of core's extension contract (types only; core is never a dependency).
//!
//! The serde form is the contract's tree JSON, the same as `fixtures/**/tree.json`:
//! `i64`/`u64` are decimal strings and non-finite floats are `"NaN"`, `"Infinity"`, `"-Infinity"`.

use std::collections::BTreeMap;

use serde::de::{self, Deserializer};
use serde::ser::{SerializeSeq, SerializeStruct, Serializer};
use serde::{Deserialize, Serialize};

pub const EXTENSION_NAME: &str = "TM_OPCUA";
pub const REQUIRES_CONTRACT: &str = "^0.1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Dtype {
    Utf8,
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl Dtype {
    pub fn name(self) -> &'static str {
        match self {
            Dtype::Utf8 => "utf8",
            Dtype::I8 => "i8",
            Dtype::U8 => "u8",
            Dtype::I16 => "i16",
            Dtype::U16 => "u16",
            Dtype::I32 => "i32",
            Dtype::U32 => "u32",
            Dtype::I64 => "i64",
            Dtype::U64 => "u64",
            Dtype::F32 => "f32",
            Dtype::F64 => "f64",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Scalar {
    Utf8(String),
    I8(i8),
    U8(u8),
    I16(i16),
    U16(u16),
    I32(i32),
    U32(u32),
    I64(i64),
    U64(u64),
    F32(f32),
    F64(f64),
}

impl Scalar {
    pub fn dtype(&self) -> Dtype {
        match self {
            Scalar::Utf8(_) => Dtype::Utf8,
            Scalar::I8(_) => Dtype::I8,
            Scalar::U8(_) => Dtype::U8,
            Scalar::I16(_) => Dtype::I16,
            Scalar::U16(_) => Dtype::U16,
            Scalar::I32(_) => Dtype::I32,
            Scalar::U32(_) => Dtype::U32,
            Scalar::I64(_) => Dtype::I64,
            Scalar::U64(_) => Dtype::U64,
            Scalar::F32(_) => Dtype::F32,
            Scalar::F64(_) => Dtype::F64,
        }
    }
}

fn nonfinite(v: f64) -> &'static str {
    if v.is_nan() {
        "NaN"
    } else if v > 0.0 {
        "Infinity"
    } else {
        "-Infinity"
    }
}

impl Serialize for Scalar {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Scalar::Utf8(v) => s.serialize_str(v),
            Scalar::I8(v) => s.serialize_i8(*v),
            Scalar::U8(v) => s.serialize_u8(*v),
            Scalar::I16(v) => s.serialize_i16(*v),
            Scalar::U16(v) => s.serialize_u16(*v),
            Scalar::I32(v) => s.serialize_i32(*v),
            Scalar::U32(v) => s.serialize_u32(*v),
            Scalar::I64(v) => s.serialize_str(&v.to_string()),
            Scalar::U64(v) => s.serialize_str(&v.to_string()),
            Scalar::F32(v) if v.is_finite() => s.serialize_f32(*v),
            Scalar::F32(v) => s.serialize_str(nonfinite(f64::from(*v))),
            Scalar::F64(v) if v.is_finite() => s.serialize_f64(*v),
            Scalar::F64(v) => s.serialize_str(nonfinite(*v)),
        }
    }
}

/// A scalar or a 1-D array.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Scalar(Scalar),
    Array(Vec<Scalar>),
}

impl Serialize for Value {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Value::Scalar(v) => v.serialize(s),
            Value::Array(items) => {
                let mut seq = s.serialize_seq(Some(items.len()))?;
                for item in items {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
        }
    }
}

/// `dtype` must match the variant of every scalar in `value`; deserializing guarantees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Attr {
    pub dtype: Dtype,
    pub value: Value,
}

impl Serialize for Attr {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("Attr", 2)?;
        st.serialize_field("dtype", &self.dtype)?;
        st.serialize_field("value", &self.value)?;
        st.end()
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawScalar {
    Str(String),
    Int(i64),
    UInt(u64),
    Float(f64),
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RawValue {
    Array(Vec<RawScalar>),
    Scalar(RawScalar),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttr {
    dtype: Dtype,
    value: RawValue,
}

fn to_int<T: TryFrom<i128>>(raw: &RawScalar, dtype: Dtype, allow_str: bool) -> Result<T, String> {
    let n: i128 = match raw {
        RawScalar::Int(v) => i128::from(*v),
        RawScalar::UInt(v) => i128::from(*v),
        RawScalar::Str(s) if allow_str => s
            .parse()
            .map_err(|_| format!("invalid {dtype:?} value {s:?}"))?,
        _ => return Err(format!("expected an integer for {dtype:?}")),
    };
    T::try_from(n).map_err(|_| format!("{n} is out of range for {dtype:?}"))
}

fn to_f64(raw: &RawScalar, dtype: Dtype) -> Result<f64, String> {
    match raw {
        RawScalar::Int(v) => Ok(*v as f64),
        RawScalar::UInt(v) => Ok(*v as f64),
        RawScalar::Float(v) => Ok(*v),
        RawScalar::Str(s) => match s.as_str() {
            "NaN" => Ok(f64::NAN),
            "Infinity" => Ok(f64::INFINITY),
            "-Infinity" => Ok(f64::NEG_INFINITY),
            _ => Err(format!("invalid {dtype:?} value {s:?}")),
        },
    }
}

fn to_scalar(raw: &RawScalar, dtype: Dtype) -> Result<Scalar, String> {
    Ok(match dtype {
        Dtype::Utf8 => match raw {
            RawScalar::Str(s) => Scalar::Utf8(s.clone()),
            _ => return Err("expected a string for Utf8".to_string()),
        },
        Dtype::I8 => Scalar::I8(to_int(raw, dtype, false)?),
        Dtype::U8 => Scalar::U8(to_int(raw, dtype, false)?),
        Dtype::I16 => Scalar::I16(to_int(raw, dtype, false)?),
        Dtype::U16 => Scalar::U16(to_int(raw, dtype, false)?),
        Dtype::I32 => Scalar::I32(to_int(raw, dtype, false)?),
        Dtype::U32 => Scalar::U32(to_int(raw, dtype, false)?),
        Dtype::I64 => Scalar::I64(to_int(raw, dtype, true)?),
        Dtype::U64 => Scalar::U64(to_int(raw, dtype, true)?),
        Dtype::F32 => Scalar::F32(to_f64(raw, dtype)? as f32),
        Dtype::F64 => Scalar::F64(to_f64(raw, dtype)?),
    })
}

impl<'de> Deserialize<'de> for Attr {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = RawAttr::deserialize(d)?;
        let value = match &raw.value {
            RawValue::Scalar(s) => to_scalar(s, raw.dtype).map(Value::Scalar),
            RawValue::Array(items) => items
                .iter()
                .map(|s| to_scalar(s, raw.dtype))
                .collect::<Result<_, _>>()
                .map(Value::Array),
        }
        .map_err(de::Error::custom)?;
        Ok(Attr {
            dtype: raw.dtype,
            value,
        })
    }
}

/// Array data in row-major order. Unused by OPC UA; kept so the tree type matches core's.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Dataset {
    pub dtype: Dtype,
    pub shape: Vec<usize>,
    pub data: Vec<Scalar>,
    pub attrs: BTreeMap<String, Attr>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDataset {
    dtype: Dtype,
    shape: Vec<usize>,
    data: Vec<RawScalar>,
    #[serde(default)]
    attrs: BTreeMap<String, Attr>,
}

impl<'de> Deserialize<'de> for Dataset {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = RawDataset::deserialize(d)?;
        if raw.dtype == Dtype::Utf8 {
            return Err(de::Error::custom("datasets cannot be Utf8"));
        }
        let expected: usize = raw.shape.iter().product();
        if raw.data.len() != expected {
            return Err(de::Error::custom(format!(
                "shape {:?} needs {expected} elements, got {}",
                raw.shape,
                raw.data.len()
            )));
        }
        let data = raw
            .data
            .iter()
            .map(|s| to_scalar(s, raw.dtype))
            .collect::<Result<_, _>>()
            .map_err(de::Error::custom)?;
        Ok(Dataset {
            dtype: raw.dtype,
            shape: raw.shape,
            data,
            attrs: raw.attrs,
        })
    }
}

/// Names are sorted (`BTreeMap`), which is the contract's canonical order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtensionTree {
    #[serde(default)]
    pub attrs: BTreeMap<String, Attr>,
    #[serde(default)]
    pub children: BTreeMap<String, ExtensionTree>,
    #[serde(default)]
    pub datasets: BTreeMap<String, Dataset>,
}
