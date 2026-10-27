use crate::defaults::base;
use crate::fields::{Blank, DefaultValue as D, FieldDtype as T, FieldSpec};
use crate::model::{CaseSensitivity, ComparisonOperator};

pub(crate) const SCHEMA_VERSION_ATTR: &str = "Opcua_Schema_Version";
pub const CURRENT_SCHEMA_VERSION: &str = "1.2";
/// Minor of `CURRENT_SCHEMA_VERSION`; fields carry the minor that introduced them.
pub(crate) const CURRENT_MINOR: u32 = 2;

const AUTH_MODES: &[&str] = &["Anonymous", "UsernamePassword", "Certificate"];
const SECURITY_MODES: &[&str] = &["None", "Sign", "SignAndEncrypt"];
// NOTE: A blank Security_Policy is Auto (SDK auto-select): declared with Blank::Value on the field.
pub(crate) const SECURITY_POLICIES: &[&str] = &[
    "None",
    "Basic256Sha256",
    "Aes128_Sha256_RsaOaep",
    "Aes256_Sha256_RsaPss",
    "ECC_nistP256",
    "ECC_nistP384",
    "ECC_brainpoolP256r1",
    "ECC_brainpoolP384r1",
];
const QUEUE_POLICIES: &[&str] = &["DiscardOldest", "KeepOldest"];
const MACHINE_PROFILES: &[&str] = &["Aconity", "Eos"];
const MIN_INTEGRITY_LEVELS: &[&str] = &[
    "Untrusted",
    "Low",
    "Medium",
    "High",
    "System",
    "ProtectedProcess",
];
const USER_ACCESS_LEVELS: &[&str] = &[
    "CurrentUserOnly",
    "CurrentUserAndAdmins",
    "AnyLocalUser",
    "Unrestricted",
];
const RULE_ACTIONS: &[&str] = &["CaptureStart", "CaptureStop"];
const COMBINATORS: &[&str] = &["And", "Or"];
const EVALUATION_MODES: &[&str] = &["String", "Numeric"];
const CASE_SENSITIVITIES: &[&str] = &[
    "Exact",
    "Contains",
    "ExactCaseSensitive",
    "ContainsCaseSensitive",
];
const COMPARISON_OPERATORS: &[&str] = &[
    "Equal",
    "NotEqual",
    "GreaterThan",
    "LessThan",
    "GreaterThanOrEqual",
    "LessThanOrEqual",
];

pub(crate) const PIPE_NAME_PREFIX: &str = "\\\\.\\pipe\\";

const fn int(n: u32) -> D {
    D::Int(n as i64)
}

/// client-proj `IdentityResolver.Resolve`: case-insensitive, "username"/"password" alias.
pub(crate) fn canonical_auth_mode(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().as_str() {
        "anonymous" => Some("Anonymous"),
        "usernamepassword" | "username" | "password" => Some("UsernamePassword"),
        "certificate" => Some("Certificate"),
        _ => None,
    }
}

/// client-proj `SecurityPolicyNames.Canonical`: URI or short name, '_' and case ignored,
/// optional "ECC" prefix, and NIST curves never carry "r1".
pub(crate) fn canonical_security_policy(s: &str) -> Option<&'static str> {
    let short = s.rsplit('#').next().unwrap_or(s).trim();
    if short.is_empty() {
        return None;
    }
    let key = |v: &str| {
        let mut k = v.replace('_', "").to_ascii_lowercase();
        if let Some(rest) = k.strip_prefix("ecc") {
            k = rest.to_string();
        }
        if k.starts_with("nistp") && k.ends_with("r1") {
            k.truncate(k.len() - 2);
        }
        k
    };
    SECURITY_POLICIES
        .iter()
        .copied()
        .find(|p| key(p) == key(short))
}

/// client-proj treats anything but "KeepOldest" as discard; "DropOldest" is the legacy name.
pub(crate) fn canonical_queue_policy(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().as_str() {
        "discardoldest" | "dropoldest" => Some("DiscardOldest"),
        "keepoldest" => Some("KeepOldest"),
        _ => None,
    }
}

/// winnt.h SECURITY_MANDATORY_* RIDs as written by older tools ("0x2000") -> level names.
pub(crate) fn canonical_integrity_level(s: &str) -> Option<&'static str> {
    match s.trim().to_ascii_lowercase().as_str() {
        "0x0" | "0x0000" => Some("Untrusted"),
        "0x1000" => Some("Low"),
        "0x2000" => Some("Medium"),
        "0x3000" => Some("High"),
        "0x4000" => Some("System"),
        "0x5000" => Some("ProtectedProcess"),
        _ => None,
    }
}

pub(crate) static VERSION_FIELDS: &[FieldSpec] = &[FieldSpec::req(SCHEMA_VERSION_ATTR, T::Utf8)];

pub(crate) static CLIENT_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Auth_Mode", T::Utf8)
        .one_of(AUTH_MODES)
        .normalized(canonical_auth_mode)
        .blank(Blank::Invalid),
    FieldSpec::opt("BFS_Max_Depth", T::U32, int(base::BFS_MAX_DEPTH)),
    FieldSpec::opt("Keep_Alive_Count", T::U32, int(base::KEEP_ALIVE_COUNT)),
    FieldSpec::opt("Lifetime_Count", T::U32, int(base::LIFETIME_COUNT)),
    FieldSpec::opt("Machine_Profile", T::Utf8, D::Str(""))
        .one_of(MACHINE_PROFILES)
        .blank(Blank::Value),
    FieldSpec::opt("Publish_Interval", T::U32, int(base::PUBLISH_INTERVAL_MS)),
    FieldSpec::opt("Queue_Policy", T::Utf8, D::Str(base::QUEUE_POLICY.as_str()))
        .one_of(QUEUE_POLICIES)
        .normalized(canonical_queue_policy)
        .blank(Blank::Default(base::QUEUE_POLICY.as_str())),
    FieldSpec::opt(
        "Queue_Size_Data_Change",
        T::U32,
        int(base::QUEUE_SIZE_DATA_CHANGE),
    ),
    FieldSpec::opt("Queue_Size_Events", T::U32, int(base::QUEUE_SIZE_EVENTS)),
    FieldSpec::opt(
        "Reconnect_Interval",
        T::U32,
        int(base::RECONNECT_INTERVAL_MS),
    ),
    FieldSpec::opt("Root_Node", T::Utf8, D::Str("")),
    FieldSpec::opt("Sampling_Interval", T::I32, int(base::SAMPLING_INTERVAL_MS)),
    FieldSpec::req("Security_Mode", T::Utf8)
        .one_of(SECURITY_MODES)
        .blank(Blank::Default(base::SECURITY_MODE.as_str())),
    FieldSpec::req("Security_Policy", T::Utf8)
        .one_of(SECURITY_POLICIES)
        .normalized(canonical_security_policy)
        .blank(Blank::Value),
    FieldSpec::req("Server_URL", T::Utf8),
    FieldSpec::opt("Session_Timeout", T::U32, int(base::SESSION_TIMEOUT_MS)),
    FieldSpec::opt(
        "Sync_Loop_Interval_Initial",
        T::U32,
        int(base::SYNC_LOOP_INTERVAL_INITIAL_MS),
    ),
    FieldSpec::opt(
        "Sync_Loop_Interval_Settled",
        T::U32,
        int(base::SYNC_LOOP_INTERVAL_SETTLED_MS),
    ),
];

pub(crate) static PIPE_FIELDS: &[FieldSpec] = &[
    FieldSpec::opt("Buffer_Size", T::U32, int(base::BUFFER_SIZE)),
    FieldSpec::opt("Configure_Client", T::U32, D::Int(0)).as_flag(),
    FieldSpec::opt("Inbound_Rate_Limit", T::I32, int(base::INBOUND_RATE_LIMIT_HZ)),
    FieldSpec::opt(
        "Max_Inbound_Message_Size",
        T::U32,
        int(base::MAX_INBOUND_MESSAGE_SIZE),
    ),
    FieldSpec::req("Min_Integrity_Level", T::Utf8)
        .one_of(MIN_INTEGRITY_LEVELS)
        .normalized(canonical_integrity_level)
        .blank(Blank::Default(base::MIN_INTEGRITY_LEVEL.as_str())),
    FieldSpec::req("Pipe_Enabled", T::U32).as_flag(),
    FieldSpec::req("Pipe_Name", T::Utf8).prefixed(PIPE_NAME_PREFIX),
    FieldSpec::req("User_Access_Level", T::Utf8)
        .one_of(USER_ACCESS_LEVELS)
        .blank(Blank::Default(base::USER_ACCESS_LEVEL.as_str())),
];

pub(crate) static TRIGGER_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Triggers_Enabled", T::U32).as_flag(),
    FieldSpec::opt("Trigger_Stop_Ceiling_Layers", T::U32, D::Int(0)),
];

pub(crate) static RULE_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Action", T::Utf8).one_of(RULE_ACTIONS),
    FieldSpec::req("Priority", T::U32),
    FieldSpec::req("Rule_Enabled", T::U32).as_flag(),
    FieldSpec::req("Rule_Id", T::U32).non_zero(),
    FieldSpec::req("Rule_Label", T::Utf8),
    FieldSpec::opt("Combinator", T::Utf8, D::Str(""))
        .one_of(COMBINATORS)
        .blank(Blank::Value),
    FieldSpec::opt("Cooldown_Period", T::U32, D::Int(0)),
    FieldSpec::opt("Description", T::Utf8, D::Str("")),
    FieldSpec::opt("Max_Fires_Per_Job", T::U32, D::Int(0)),
];

pub(crate) static CONDITION_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Condition_Id", T::U32).non_zero(),
    FieldSpec::req("Condition_Label", T::Utf8),
    FieldSpec::req("Condition_Enabled", T::U32).as_flag(),
    FieldSpec::req("Evaluation_Mode", T::Utf8)
        .one_of(EVALUATION_MODES)
        .blank(Blank::Default("String")),
    FieldSpec::req("Signal", T::Utf8).blank(Blank::Value),
    FieldSpec::req("Value", T::Utf8).blank(Blank::Value),
    FieldSpec::opt("Event", T::Utf8, D::Str("")),
    FieldSpec::opt("Subsystem", T::Utf8, D::Str("")),
    FieldSpec::opt("Component", T::Utf8, D::Str("")),
    FieldSpec::opt(
        "Case_Sensitivity",
        T::Utf8,
        D::Str(CaseSensitivity::Exact.as_str()),
    )
    .one_of(CASE_SENSITIVITIES)
    .blank(Blank::ModeUnused),
    FieldSpec::opt(
        "Comparison_Operator",
        T::Utf8,
        D::Str(ComparisonOperator::Equal.as_str()),
    )
    .one_of(COMPARISON_OPERATORS)
    .blank(Blank::ModeUnused),
    FieldSpec::opt("Epsilon", T::F64, D::Float(0.0)),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_names_are_unique_in_each_group() {
        for list in [
            VERSION_FIELDS,
            CLIENT_FIELDS,
            PIPE_FIELDS,
            TRIGGER_FIELDS,
            RULE_FIELDS,
            CONDITION_FIELDS,
        ] {
            let mut seen = std::collections::HashSet::new();
            for f in list {
                assert!(seen.insert(f.name), "duplicate field spec {}", f.name);
            }
        }
    }
}
