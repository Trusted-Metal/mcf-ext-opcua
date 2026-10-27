use std::collections::{BTreeMap, HashMap};

use crate::contract::{Attr, Dtype, ExtensionTree, Scalar, Value};
use crate::errors::{join_path, validation, Result};
use crate::model::{ExtraAttrs, Warning, WarningKind};

// ---- attribute-name suggestions ("did you mean") ----
// Names are compared after removing case and separators (name_key). A suggestion is offered only
// when exactly one known field is closest and within the edit budget for that field's length.
/// (longest normalised name length, edits allowed): names up to 8 chars allow 1 edit, up to 16
/// allow 2; anything longer allows `SUGGEST_MAX_EDITS`.
const SUGGEST_EDIT_BUDGET: &[(usize, usize)] = &[(8, 1), (16, 2)];
const SUGGEST_MAX_EDITS: usize = 3;
/// Cost of one insertion, deletion, substitution or adjacent transposition.
const EDIT_COST: usize = 1;

/// A validated on-disk attribute value; integers of any width are carried as `i64`.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Raw {
    Str(String),
    Int(i64),
    Float(f64),
}

impl From<&str> for Raw {
    fn from(v: &str) -> Self {
        Raw::Str(v.to_string())
    }
}
impl From<String> for Raw {
    fn from(v: String) -> Self {
        Raw::Str(v)
    }
}
impl From<u32> for Raw {
    fn from(v: u32) -> Self {
        Raw::Int(i64::from(v))
    }
}
impl From<i32> for Raw {
    fn from(v: i32) -> Self {
        Raw::Int(i64::from(v))
    }
}
impl From<f64> for Raw {
    fn from(v: f64) -> Self {
        Raw::Float(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldDtype {
    Utf8,
    U32,
    I32,
    F64,
}

impl FieldDtype {
    fn dtype(self) -> Dtype {
        match self {
            FieldDtype::Utf8 => Dtype::Utf8,
            FieldDtype::U32 => Dtype::U32,
            FieldDtype::I32 => Dtype::I32,
            FieldDtype::F64 => Dtype::F64,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DefaultValue {
    Str(&'static str),
    Int(i64),
    Float(f64),
}

impl DefaultValue {
    fn raw(self) -> Raw {
        match self {
            DefaultValue::Str(s) => Raw::Str(s.to_string()),
            DefaultValue::Int(n) => Raw::Int(n),
            DefaultValue::Float(f) => Raw::Float(f),
        }
    }

    /// How the value reads in a warning.
    pub fn describe(self) -> String {
        match self {
            DefaultValue::Str(s) => format!("{s:?}"),
            DefaultValue::Int(n) => n.to_string(),
            DefaultValue::Float(x) => x.to_string(),
        }
    }
}

/// What a missing attribute means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    /// Missing is an error (structure, identity, security/connection, matching semantics).
    Required,
    /// Missing reads as the field's default, with a warning.
    Warned,
}

/// What a blank utf8 value ("") means for an enum field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Blank {
    /// Blank is an error such as for required fields `[Actions/Id's]`.
    Invalid,
    /// Blank is a value in its own right; the caller decides (mode-dependent condition fields).
    Value,
    /// Blank reads as this canonical value, with a warning (strictest where security-relevant).
    Default(&'static str),
    /// Blank is the normal form when the condition's mode ignores the field.
    ModeUnused,
}

/// One on-disk attribute of a group.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FieldSpec {
    pub name: &'static str,
    pub dtype: FieldDtype,
    pub default: Option<DefaultValue>,
    pub enum_values: Option<&'static [&'static str]>,
    /// u32 that must be 0 or 1.
    pub flag: bool,
    pub nonzero: bool,
    pub starts_with: Option<&'static str>,
    pub presence: Presence,
    /// Schema minor that introduced the field; a file declaring an older minor never needs it.
    pub since_minor: u32,
    pub blank: Blank,
    /// Maps alternative spellings consumers accept to the canonical one, before the enum check.
    pub normalize: Option<fn(&str) -> Option<&'static str>>,
}

impl FieldSpec {
    /// Missing is an error.
    pub const fn req(name: &'static str, dtype: FieldDtype) -> Self {
        Self {
            name,
            dtype,
            default: None,
            enum_values: None,
            flag: false,
            nonzero: false,
            starts_with: None,
            presence: Presence::Required,
            since_minor: 2,
            blank: Blank::Invalid,
            normalize: None,
        }
    }

    pub const fn opt(name: &'static str, dtype: FieldDtype, default: DefaultValue) -> Self {
        Self {
            default: Some(default),
            presence: Presence::Warned,
            ..Self::req(name, dtype)
        }
    }

    pub const fn blank(self, blank: Blank) -> Self {
        Self { blank, ..self }
    }

    pub const fn normalized(self, f: fn(&str) -> Option<&'static str>) -> Self {
        Self {
            normalize: Some(f),
            ..self
        }
    }

    pub const fn one_of(self, values: &'static [&'static str]) -> Self {
        Self {
            enum_values: Some(values),
            ..self
        }
    }

    pub const fn as_flag(self) -> Self {
        Self { flag: true, ..self }
    }

    pub const fn non_zero(self) -> Self {
        Self {
            nonzero: true,
            ..self
        }
    }

    pub const fn prefixed(self, prefix: &'static str) -> Self {
        Self {
            starts_with: Some(prefix),
            ..self
        }
    }
}

pub(crate) fn spec(list: &'static [FieldSpec], name: &str) -> &'static FieldSpec {
    list.iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("internal: no field spec {name:?}"))
}

/// Validates `v` against `f` and returns its canonical form.
pub(crate) fn check_value(f: &FieldSpec, v: Raw, path: &str) -> Result<Raw> {
    let here = join_path(path, &format!("@{}", f.name));
    match (f.dtype, v) {
        (FieldDtype::Utf8, Raw::Str(s)) => {
            if s.contains('\0') {
                return Err(validation(here, "must not contain NUL"));
            }
            let mut out = s;
            if out.is_empty() {
                match f.blank {
                    Blank::Value | Blank::ModeUnused => return Ok(Raw::Str(out)),
                    Blank::Default(d) => out = d.to_string(),
                    Blank::Invalid => {}
                }
            }
            if let Some(canonical) = f.normalize.and_then(|n| n(&out)) {
                out = canonical.to_string();
            }
            if let Some(list) = f.enum_values {
                let found = list.iter().find(|e| e.eq_ignore_ascii_case(&out));
                match found {
                    Some(e) => out = (*e).to_string(),
                    None => {
                        let expected: Vec<String> = list.iter().map(|e| format!("{e:?}")).collect();
                        return Err(validation(
                            here,
                            format!(
                                "invalid value {out:?}; expected one of {}",
                                expected.join(", ")
                            ),
                        ));
                    }
                }
            }
            if let Some(prefix) = f.starts_with {
                if !(out.starts_with(prefix) && out.len() > prefix.len()) {
                    return Err(validation(
                        here,
                        format!("must start with {prefix} followed by a name"),
                    ));
                }
            }
            Ok(Raw::Str(out))
        }
        (FieldDtype::F64, Raw::Float(x)) => {
            if !x.is_finite() {
                return Err(validation(here, "must be finite"));
            }
            Ok(Raw::Float(x))
        }
        (dt @ (FieldDtype::U32 | FieldDtype::I32), Raw::Int(n)) => {
            let (min, max) = if dt == FieldDtype::U32 {
                (0, i64::from(u32::MAX))
            } else {
                (i64::from(i32::MIN), i64::from(i32::MAX))
            };
            if n < min || n > max {
                return Err(validation(
                    here,
                    format!("{n} is out of range for {}", dt.dtype().name()),
                ));
            }
            if f.flag && n != 0 && n != 1 {
                return Err(validation(here, format!("flag must be 0 or 1, got {n}")));
            }
            if f.nonzero && n == 0 {
                return Err(validation(here, "must not be 0"));
            }
            Ok(Raw::Int(n))
        }
        (dt, _) => Err(validation(
            here,
            format!("expected a {} value", dt.dtype().name()),
        )),
    }
}

fn raw_to_attr(f: &FieldSpec, raw: Raw) -> Attr {
    let scalar = match (f.dtype, raw) {
        (FieldDtype::Utf8, Raw::Str(s)) => Scalar::Utf8(s),
        (FieldDtype::U32, Raw::Int(n)) => Scalar::U32(n as u32),
        (FieldDtype::I32, Raw::Int(n)) => Scalar::I32(n as i32),
        (FieldDtype::F64, Raw::Float(x)) => Scalar::F64(x),
        _ => unreachable!("check_value guarantees the value matches the field dtype"),
    };
    Attr {
        dtype: f.dtype.dtype(),
        value: Value::Scalar(scalar),
    }
}

fn attr_to_raw(f: &FieldSpec, attr: &Attr, path: &str) -> Result<Raw> {
    let here = join_path(path, &format!("@{}", f.name));
    if attr.dtype != f.dtype.dtype() {
        return Err(validation(
            here,
            format!(
                "expected dtype {}, got {}",
                f.dtype.dtype().name(),
                attr.dtype.name()
            ),
        ));
    }
    let Value::Scalar(scalar) = &attr.value else {
        return Err(validation(here, "expected a scalar, got an array"));
    };
    let raw = match scalar {
        Scalar::Utf8(s) => Raw::Str(s.clone()),
        Scalar::U32(n) => Raw::Int(i64::from(*n)),
        Scalar::I32(n) => Raw::Int(i64::from(*n)),
        Scalar::F64(x) => Raw::Float(*x),
        _ => return Err(validation(here, "unsupported attribute type")),
    };
    check_value(f, raw, path)
}

fn check_scalar(dtype: Dtype, scalar: &Scalar, path: &str) -> Result<()> {
    if scalar.dtype() != dtype {
        return Err(validation(
            path,
            format!(
                "{} value does not match dtype {}",
                scalar.dtype().name(),
                dtype.name()
            ),
        ));
    }
    if matches!(scalar, Scalar::Utf8(s) if s.contains('\0')) {
        return Err(validation(path, "utf8 value must not contain NUL"));
    }
    Ok(())
}

/// Validates an attribute the codec carries opaquely (name, dtype and value shape).
pub(crate) fn check_extra_attr(name: &str, attr: &Attr, path: &str) -> Result<()> {
    let here = join_path(path, &format!("@{name}"));
    if name.is_empty() || name.contains('/') || name.contains('\0') {
        return Err(validation(here, "invalid attribute name"));
    }
    match &attr.value {
        Value::Scalar(s) => check_scalar(attr.dtype, s, &here),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .try_for_each(|(i, s)| check_scalar(attr.dtype, s, &format!("{here}[{i}]"))),
    }
}

pub(crate) fn assert_no_datasets(tree: &ExtensionTree, path: &str) -> Result<()> {
    match tree.datasets.keys().next() {
        Some(name) => Err(validation(join_path(path, name), "unexpected dataset")),
        None => Ok(()),
    }
}

pub(crate) fn assert_leaf(tree: &ExtensionTree, path: &str) -> Result<()> {
    assert_no_datasets(tree, path)?;
    match tree.children.keys().next() {
        Some(name) => Err(validation(join_path(path, name), "unexpected child group")),
        None => Ok(()),
    }
}

pub(crate) fn require_child<'a>(tree: &'a ExtensionTree, name: &str) -> Result<&'a ExtensionTree> {
    tree.children
        .get(name)
        .ok_or_else(|| validation(name, "missing group"))
}

pub(crate) struct Values(HashMap<&'static str, Raw>);

impl Values {
    pub fn str(&self, name: &str) -> &str {
        match self.0.get(name) {
            Some(Raw::Str(s)) => s,
            _ => panic!("internal: {name} is not a string field"),
        }
    }

    pub fn int(&self, name: &str) -> i64 {
        match self.0.get(name) {
            Some(Raw::Int(n)) => *n,
            _ => panic!("internal: {name} is not an integer field"),
        }
    }

    pub fn u32(&self, name: &str) -> u32 {
        self.int(name) as u32
    }

    pub fn f64(&self, name: &str) -> f64 {
        match self.0.get(name) {
            Some(Raw::Float(x)) => *x,
            _ => panic!("internal: {name} is not a float field"),
        }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.int(name) == 1
    }
}

/// Read-side state shared by every group of one unpack.
pub(crate) struct ReadCtx<'a> {
    /// Minor of the file's declared schema version; fields introduced later are never required.
    pub file_minor: u32,
    /// Minor this reader implements; unknown names in a newer-minor file are expected, not typos.
    pub reader_minor: u32,
    pub warnings: &'a mut Vec<Warning>,
}

impl ReadCtx<'_> {
    pub fn warn(&mut self, kind: WarningKind, at: &str, message: impl std::fmt::Display) {
        self.warnings.push(Warning {
            kind,
            path: at.to_string(),
            message: message.to_string(),
        });
    }
}

/// Note for a read value that differs from what was stored. Casing is reported too: the value is
/// unambiguous, but a non-canonical spelling means the file was not written by a validating tool.
pub(crate) fn change_note(before: &str, after: &str) -> Option<(WarningKind, String)> {
    if before == after || before.eq_ignore_ascii_case(after) {
        None
    } else if before.is_empty() {
        Some((WarningKind::Defaulted, format!("blank, read as {after:?}")))
    } else if before.eq_ignore_ascii_case(after) {
        Some((
            WarningKind::Normalized,
            format!("{before:?} read as {after:?} (casing)"),
        ))
    } else {
        Some((
            WarningKind::Normalized,
            format!("{before:?} read as {after:?}"),
        ))
    }
}

/// Name compared without case or separators: "Cooldown_Period", "cooldown-period" and
/// "CooldownPeriod" are the same name.
pub(crate) fn name_key(s: &str) -> String {
    s.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Optimal string alignment distance: Levenshtein plus adjacent transpositions ("Peroid").
fn osa_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut d = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=b.len() {
        d[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { EDIT_COST };
            d[i][j] = (d[i - 1][j] + EDIT_COST)
                .min(d[i][j - 1] + EDIT_COST)
                .min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + EDIT_COST);
            }
        }
    }
    d[a.len()][b.len()]
}

/// Edits allowed for a suggestion, scaled by the (normalised) name length.
fn max_edits(len: usize) -> usize {
    SUGGEST_EDIT_BUDGET
        .iter()
        .find(|(up_to, _)| len <= *up_to)
        .map_or(SUGGEST_MAX_EDITS, |(_, edits)| *edits)
}

/// The single closest candidate within the edit budget; None when nothing is close or two
/// candidates tie (a suggestion must point at exactly one field).
fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let key = name_key(name);
    let mut best: Option<(usize, &'a str)> = None;
    let mut tied = false;
    for c in candidates {
        let ck = name_key(c);
        let d = osa_distance(&key, &ck);
        if d == 0 || d > max_edits(ck.len()) {
            continue;
        }
        match best {
            Some((bd, _)) if d > bd => {}
            Some((bd, _)) if d == bd => tied = true,
            _ => {
                best = Some((d, c));
                tied = false;
            }
        }
    }
    if tied {
        None
    } else {
        best.map(|(_, c)| c)
    }
}

fn widened(f: &FieldSpec, a: &Attr) -> Option<Raw> {
    let Value::Scalar(s) = &a.value else {
        return None;
    };
    match (f.dtype, s) {
        (FieldDtype::U32 | FieldDtype::I32, Scalar::I8(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::U8(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::I16(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::U16(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::I32(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::U32(v)) => Some(Raw::Int(i64::from(*v))),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::I64(v)) => Some(Raw::Int(*v)),
        (FieldDtype::U32 | FieldDtype::I32, Scalar::U64(v)) => i64::try_from(*v).ok().map(Raw::Int),
        (FieldDtype::F64, Scalar::F32(v)) => Some(Raw::Float(f64::from(*v))),
        _ => None,
    }
}

/// Reads every spec'd attribute (exact dtype, validated) and collects the rest into `extra`.
/// Missing: `Required` is an error, `Warned` reads as the default with a warning, and a field
/// newer than the file's declared minor reads as its default silently. Unknown names are kept
/// in `extra` and warned (with a suggestion when one is close).
pub(crate) fn read_fields(
    attrs: &BTreeMap<String, Attr>,
    specs: &[FieldSpec],
    path: &str,
    ignore: &[&str],
    ctx: &mut ReadCtx<'_>,
) -> Result<(Values, ExtraAttrs)> {
    let mut values = HashMap::new();
    let mut consumed: Vec<&str> = Vec::new();
    for f in specs {
        let at = join_path(path, &format!("@{}", f.name));
        let found = attrs.get_key_value(f.name).or_else(|| {
            let key = name_key(f.name);
            let mut same = attrs
                .iter()
                .filter(|(n, _)| !specs.iter().any(|s| s.name == n.as_str()) && name_key(n) == key);
            match (same.next(), same.next()) {
                (Some(hit), None) => Some(hit),
                _ => None, // none, or ambiguous: treat as missing
            }
        });
        let raw = match found {
            Some((stored_name, a)) => {
                if stored_name != f.name {
                    ctx.warn(
                        WarningKind::Normalized,
                        &join_path(path, &format!("@{stored_name}")),
                        format!("attribute name read as {}", f.name),
                    );
                    consumed.push(stored_name.as_str());
                }
                let raw = if a.dtype != f.dtype.dtype() {
                    match widened(f, a) {
                        Some(r) => {
                            ctx.warn(
                                WarningKind::Normalized,
                                &at,
                                format!(
                                    "dtype {} read as {}",
                                    a.dtype.name(),
                                    f.dtype.dtype().name()
                                ),
                            );
                            check_value(f, r, path)? // range, flag and nonzero checks still apply
                        }
                        None => attr_to_raw(f, a, path)?, // unchanged error for strings, arrays, etc.
                    }
                } else {
                    attr_to_raw(f, a, path)?
                };
                if let (Value::Scalar(Scalar::Utf8(before)), Raw::Str(after)) = (&a.value, &raw) {
                    if let Some((kind, note)) = change_note(before, after) {
                        ctx.warn(kind, &at, note);
                    }
                }
                raw
            }
            None => match (f.presence, f.default) {
                (_, Some(d)) if f.since_minor > ctx.file_minor => d.raw(),
                (Presence::Warned, Some(d)) => {
                    ctx.warn(
                        WarningKind::Defaulted,
                        &at,
                        format!("missing, read as {}", d.describe()),
                    );
                    d.raw()
                }
                _ => {
                    let unknown = attrs
                        .keys()
                        .map(String::as_str)
                        .filter(|n| !specs.iter().any(|s| s.name == *n));
                    let hint =
                        suggest(f.name, unknown).map_or(String::new(), |n| format!(" (found {n})"));
                    return Err(validation(at, format!("missing required attribute{hint}")));
                }
            },
        };
        values.insert(f.name, raw);
    }
    let extra: ExtraAttrs = attrs
        .iter()
        .filter(|(name, _)| {
            !specs.iter().any(|f| f.name == name.as_str()) && !ignore.contains(&name.as_str())
        })
        .map(|(name, attr)| (name.clone(), attr.clone()))
        .collect();
    for name in extra.keys() {
        let at = join_path(path, &format!("@{name}"));
        let known = specs.iter().map(|f| f.name).chain(ignore.iter().copied());
        if let Some(s) = specs
            .iter()
            .map(|f| f.name)
            .find(|s| name_key(s) == name_key(name))
        {
            ctx.warn(
                WarningKind::UnknownName,
                &at,
                format!("duplicate spelling of {s} (the canonical attribute was used), kept but not used"),
            );
            continue;
        }
        match suggest(name, known) {
            Some(s) => ctx.warn(
                WarningKind::UnknownName,
                &at,
                format!("unknown attribute (did you mean {s}?), kept but not used"),
            ),
            None if ctx.file_minor <= ctx.reader_minor => ctx.warn(
                WarningKind::UnknownName,
                &at,
                "unknown attribute, kept but not used",
            ),
            None => {}
        }
    }
    Ok((Values(values), extra))
}

/// Writes every spec'd attribute with its exact dtype, plus `extra`; output is sorted by name.
pub(crate) fn write_fields(
    mut values: HashMap<&'static str, Raw>,
    specs: &[FieldSpec],
    extra: &ExtraAttrs,
    path: &str,
) -> Result<BTreeMap<String, Attr>> {
    let mut out = BTreeMap::new();
    for f in specs {
        let raw = values
            .remove(f.name)
            .unwrap_or_else(|| panic!("internal: no value for {}", f.name));
        out.insert(
            f.name.to_string(),
            raw_to_attr(f, check_value(f, raw, path)?),
        );
    }
    for (name, attr) in extra {
        if out.contains_key(name) {
            return Err(validation(
                join_path(path, &format!("@{name}")),
                "extra attribute collides with a known attribute",
            ));
        }
        check_extra_attr(name, attr, path)?;
        out.insert(name.clone(), attr.clone());
    }
    Ok(out)
}

pub(crate) fn is_label(label: &str) -> bool {
    !label.is_empty() && !label.contains('/') && !label.contains('\0')
}
