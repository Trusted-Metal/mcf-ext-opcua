use crate::fields::{DefaultValue as D, FieldDtype as T, FieldSpec};

pub(crate) const SCHEMA_VERSION_ATTR: &str = "Opcua_Schema_Version";
pub const CURRENT_SCHEMA_VERSION: &str = "1.2";

const AUTH_MODES: &[&str] = &["Anonymous", "UsernamePassword", "Certificate"];
const SECURITY_MODES: &[&str] = &["None", "Sign", "SignAndEncrypt"];
// "" = let the SDK auto-select (model: Auto).
const SECURITY_POLICIES: &[&str] = &[
    "",
    "Basic256Sha256",
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
    "GreaterThan",
    "LessThan",
    "GreaterThanOrEqual",
    "LessThanOrEqual",
];

pub(crate) const PIPE_NAME_PREFIX: &str = "\\\\.\\pipe\\";

pub(crate) static VERSION_FIELDS: &[FieldSpec] = &[FieldSpec::req(SCHEMA_VERSION_ATTR, T::Utf8)];

pub(crate) static CLIENT_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Auth_Mode", T::Utf8).one_of(AUTH_MODES),
    FieldSpec::req("BFS_Max_Depth", T::U32),
    FieldSpec::req("Keep_Alive_Count", T::U32),
    FieldSpec::req("Lifetime_Count", T::U32),
    FieldSpec::req("Machine_Profile", T::Utf8)
        .one_of(MACHINE_PROFILES)
        .ignore_case(),
    FieldSpec::req("Publish_Interval", T::U32),
    FieldSpec::req("Queue_Policy", T::Utf8).one_of(QUEUE_POLICIES),
    FieldSpec::req("Queue_Size_Data_Change", T::U32),
    FieldSpec::req("Queue_Size_Events", T::U32),
    FieldSpec::req("Reconnect_Interval", T::U32),
    FieldSpec::req("Root_Node", T::Utf8),
    FieldSpec::req("Sampling_Interval", T::I32),
    FieldSpec::req("Security_Mode", T::Utf8).one_of(SECURITY_MODES),
    FieldSpec::req("Security_Policy", T::Utf8).one_of(SECURITY_POLICIES),
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
        .ignore_case(),
    FieldSpec::req("Pipe_Enabled", T::U32).as_flag(),
    FieldSpec::req("Pipe_Name", T::Utf8).prefixed(PIPE_NAME_PREFIX),
    FieldSpec::req("User_Access_Level", T::Utf8).one_of(USER_ACCESS_LEVELS),
];

pub(crate) static TRIGGER_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Triggers_Enabled", T::U32).as_flag(),
    FieldSpec::req("Trigger_Stop_Ceiling_Layers", T::U32),
];

pub(crate) static RULE_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Rule_Id", T::U32).non_zero(),
    FieldSpec::req("Rule_Label", T::Utf8),
    FieldSpec::req("Action", T::Utf8).one_of(RULE_ACTIONS),
    FieldSpec::req("Rule_Enabled", T::U32).as_flag(),
    FieldSpec::opt("Combinator", T::Utf8, D::Str("")).one_of(COMBINATORS),
    FieldSpec::opt("Cooldown_Period", T::U32, D::Int(0)),
    FieldSpec::opt("Description", T::Utf8, D::Str("")),
    FieldSpec::opt("Max_Fires_Per_Job", T::U32, D::Int(0)),
    FieldSpec::opt("Priority", T::U32, D::Int(0)),
];

pub(crate) static CONDITION_FIELDS: &[FieldSpec] = &[
    FieldSpec::req("Condition_Id", T::U32).non_zero(),
    FieldSpec::req("Condition_Label", T::Utf8),
    FieldSpec::req("Condition_Enabled", T::U32).as_flag(),
    FieldSpec::req("Signal", T::Utf8),
    FieldSpec::req("Value", T::Utf8),
    FieldSpec::opt("Event", T::Utf8, D::Str("")),
    FieldSpec::opt("Subsystem", T::Utf8, D::Str("")),
    FieldSpec::opt("Component", T::Utf8, D::Str("")),
    FieldSpec::opt("Evaluation_Mode", T::Utf8, D::Str("String")).one_of(EVALUATION_MODES),
    FieldSpec::opt("Case_Sensitivity", T::Utf8, D::Str("Exact")).one_of(CASE_SENSITIVITIES),
    FieldSpec::opt("Comparison_Operator", T::Utf8, D::Str("Equal")).one_of(COMPARISON_OPERATORS),
    FieldSpec::opt("Epsilon", T::F64, D::Float(0.0)),
];
