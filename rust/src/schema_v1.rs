use crate::fields::{Blank, DefaultValue as D, FieldDtype as T, FieldSpec};

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
const MACHINE_PROFILES: &[&str] = &["", "Aconity", "Eos"];
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
const COMBINATORS: &[&str] = &["", "And", "Or"];
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
        .blank(Blank::Default("Anonymous")),
    FieldSpec::req("BFS_Max_Depth", T::U32),
    FieldSpec::req("Keep_Alive_Count", T::U32),
    FieldSpec::req("Lifetime_Count", T::U32),
    FieldSpec::req("Machine_Profile", T::Utf8)
        .one_of(MACHINE_PROFILES)
        .ignore_case().blank(Blank::Value),
    FieldSpec::req("Publish_Interval", T::U32),
    FieldSpec::req("Queue_Policy", T::Utf8)
        .one_of(QUEUE_POLICIES)
        .normalized(canonical_queue_policy)
        .blank(Blank::Default("DiscardOldest")),
    FieldSpec::req("Queue_Size_Data_Change", T::U32),
    FieldSpec::req("Queue_Size_Events", T::U32),
    FieldSpec::req("Reconnect_Interval", T::U32),
    FieldSpec::req("Root_Node", T::Utf8),
    FieldSpec::req("Sampling_Interval", T::I32),
    FieldSpec::req("Security_Mode", T::Utf8)
        .one_of(SECURITY_MODES)
        .blank(Blank::Default("SignAndEncrypt")),
    FieldSpec::req("Security_Policy", T::Utf8)
        .one_of(SECURITY_POLICIES)
        .normalized(canonical_security_policy).blank(Blank::Value),
    FieldSpec::req("Server_URL", T::Utf8),
    FieldSpec::req("Session_Timeout", T::U32),
    FieldSpec::req("Sync_Loop_Interval_Initial", T::U32),
    FieldSpec::req("Sync_Loop_Interval_Settled", T::U32),
];

pub(crate) static PIPE_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Buffer_Size", T::U32),
    FieldSpec::req("Configure_Client", T::U32).as_flag(),
    FieldSpec::req("Inbound_Rate_Limit", T::I32),
    FieldSpec::req("Max_Inbound_Message_Size", T::U32),
    FieldSpec::req("Min_Integrity_Level", T::Utf8)
        .one_of(MIN_INTEGRITY_LEVELS)
        .ignore_case()
        .normalized(canonical_integrity_level)
        .blank(Blank::Default("ProtectedProcess")),
    FieldSpec::req("Pipe_Enabled", T::U32).as_flag(),
    FieldSpec::req("Pipe_Name", T::Utf8).prefixed(PIPE_NAME_PREFIX),
    FieldSpec::req("User_Access_Level", T::Utf8)
        .one_of(USER_ACCESS_LEVELS)
        .blank(Blank::Default("CurrentUserOnly")),
];

pub(crate) static TRIGGER_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Triggers_Enabled", T::U32).as_flag(),
    FieldSpec::req("Trigger_Stop_Ceiling_Layers", T::U32),
];

pub(crate) static RULE_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Action", T::Utf8).one_of(RULE_ACTIONS),
    FieldSpec::req("Priority", T::U32),
    FieldSpec::req("Rule_Enabled", T::U32).as_flag(),
    FieldSpec::req("Rule_Id", T::U32).non_zero(),
    FieldSpec::req("Rule_Label", T::Utf8),
    FieldSpec::opt("Combinator", T::Utf8, D::Str("")).one_of(COMBINATORS).blank(Blank::Value),
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
    FieldSpec::req("Signal", T::Utf8),
    FieldSpec::req("Value", T::Utf8),
    FieldSpec::opt("Event", T::Utf8, D::Str("")),
    FieldSpec::opt("Subsystem", T::Utf8, D::Str("")),
    FieldSpec::opt("Component", T::Utf8, D::Str("")),
    FieldSpec::opt("Case_Sensitivity", T::Utf8, D::Str("Exact"))
        .one_of(CASE_SENSITIVITIES)
        .blank(Blank::ModeUnused),
    FieldSpec::opt("Comparison_Operator", T::Utf8, D::Str("Equal"))
        .one_of(COMPARISON_OPERATORS)
        .blank(Blank::ModeUnused),
    FieldSpec::opt("Epsilon", T::F64, D::Float(0.0)),
];
