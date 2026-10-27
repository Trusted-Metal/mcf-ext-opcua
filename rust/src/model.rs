//! Domain model. Deliberately diverges from the on-disk spec (see DESIGN.md section 4): flags are
//! `bool`, sentinels are `Option`/variants, enums use canonical spelling.
//!
//! The serde form is camelCase and is the same as `fixtures/**/model.json`.

use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, Deserializer};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::contract::Attr;

macro_rules! string_enum {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $text:literal),+ $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $text),+
                }
            }

            pub fn parse(s: &str) -> Option<Self> {
                match s {
                    $($text => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.serialize_str(self.as_str())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                Self::parse(&s).ok_or_else(|| {
                    de::Error::custom(format!("invalid {}: {s:?}", stringify!($name)))
                })
            }
        }
    };
}

/// What kind of note a reader produced. Consumers filter and count by kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarningKind {
    /// A missing attribute or a blank value was read as its default.
    Defaulted,
    /// A value or attribute name was read in a different spelling than the canonical one.
    Normalized,
    /// An attribute this reader does not know was kept but not used.
    UnknownName,
    /// The legacy layout was converted in a way worth knowing (skipped sides, changed meanings).
    Conversion,
    /// Allowed, but weaker than the preferred security or behaviour.
    Advisory,
    /// Legal, but very likely a mistake.
    Suspicious,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub kind: WarningKind,
    /// Slash-joined location; attributes are prefixed with `@`.
    pub path: String,
    pub message: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

string_enum!(AuthMode {
    Anonymous => "Anonymous",
    UsernamePassword => "UsernamePassword",
    Certificate => "Certificate",
});

string_enum!(MachineProfile {
    Aconity => "Aconity",
    Eos => "Eos",
});

string_enum!(SecurityMode {
    None => "None",
    Sign => "Sign",
    SignAndEncrypt => "SignAndEncrypt",
});

string_enum!(
    /// `Auto` lets the SDK choose; it is stored on disk as an empty string. `None` is the OPC UA
    /// no-security policy (paired with `SecurityMode::None`), a supported option, not a default.
    SecurityPolicy {
        Auto => "Auto",
        None => "None",
        Basic256Sha256 => "Basic256Sha256",
        Aes128Sha256RsaOaep => "Aes128_Sha256_RsaOaep",
        Aes256Sha256RsaPss => "Aes256_Sha256_RsaPss",
        EccNistP256 => "ECC_nistP256",
        EccNistP384 => "ECC_nistP384",
        EccBrainpoolP256r1 => "ECC_brainpoolP256r1",
        EccBrainpoolP384r1 => "ECC_brainpoolP384r1",
    }
);

string_enum!(QueuePolicy {
    DiscardOldest => "DiscardOldest",
    KeepOldest => "KeepOldest",
});

string_enum!(MinIntegrityLevel {
    Untrusted => "Untrusted",
    Low => "Low",
    Medium => "Medium",
    High => "High",
    System => "System",
    ProtectedProcess => "ProtectedProcess",
});

string_enum!(UserAccessLevel {
    CurrentUserOnly => "CurrentUserOnly",
    CurrentUserAndAdmins => "CurrentUserAndAdmins",
    AnyLocalUser => "AnyLocalUser",
    Unrestricted => "Unrestricted",
});

string_enum!(RuleAction {
    CaptureStart => "CaptureStart",
    CaptureStop => "CaptureStop",
});

string_enum!(Combinator {
    And => "And",
    Or => "Or",
});

string_enum!(CaseSensitivity {
    Exact => "Exact",
    Contains => "Contains",
    ExactCaseSensitive => "ExactCaseSensitive",
    ContainsCaseSensitive => "ContainsCaseSensitive",
});

string_enum!(ComparisonOperator {
    Equal => "Equal",
    NotEqual => "NotEqual",
    GreaterThan => "GreaterThan",
    LessThan => "LessThan",
    GreaterThanOrEqual => "GreaterThanOrEqual",
    LessThanOrEqual => "LessThanOrEqual",
});

/// Attributes the codec does not know about, kept with their dtype and written back unchanged.
pub type ExtraAttrs = BTreeMap<String, Attr>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum RootNode {
    AutoDiscover,
    NodeId { value: String },
    BrowseName { value: String },
}

impl RootNode {
    /// On-disk form: "" for AutoDiscover, otherwise the NodeId or BrowseName text.
    pub fn to_spec(&self) -> &str {
        match self {
            RootNode::AutoDiscover => "",
            RootNode::NodeId { value } | RootNode::BrowseName { value } => value,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum SamplingInterval {
    ServerNegotiated,
    Fastest,
    Ms { ms: u32 },
}

impl SamplingInterval {
    /// On-disk form: -1, 0 or ms. `pack` rejects ms above i32::MAX, so the clamp only applies to
    /// models that were never packed.
    pub fn to_spec(&self) -> i32 {
        match self {
            SamplingInterval::ServerNegotiated => -1,
            SamplingInterval::Fastest => 0,
            SamplingInterval::Ms { ms } => i32::try_from(*ms).unwrap_or(i32::MAX),
        }
    }
}

impl SecurityPolicy {
    /// On-disk form: "" for Auto (SDK auto-select), never the literal "Auto".
    pub fn to_spec(self) -> &'static str {
        match self {
            SecurityPolicy::Auto => "",
            other => other.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientConfig {
    pub server_url: String,
    pub auth_mode: AuthMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_profile: Option<MachineProfile>,
    pub security_mode: SecurityMode,
    pub security_policy: SecurityPolicy,
    pub root_node: RootNode,
    pub sampling_interval: SamplingInterval,
    pub publish_interval: u32,
    pub bfs_max_depth: u32,
    pub keep_alive_count: u32,
    pub lifetime_count: u32,
    pub queue_policy: QueuePolicy,
    pub queue_size_data_change: u32,
    pub queue_size_events: u32,
    pub reconnect_interval: u32,
    pub session_timeout: u32,
    pub sync_loop_interval_initial: u32,
    pub sync_loop_interval_settled: u32,
    #[serde(default)]
    pub extra: ExtraAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipeConfig {
    pub enabled: bool,
    pub configure_client: bool,
    pub buffer_size: u32,
    /// Absent when the on-disk value is 0 or -1 (no cap).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inbound_rate_limit_hz: Option<u32>,
    pub max_inbound_message_size: u32,
    pub min_integrity_level: MinIntegrityLevel,
    pub pipe_name: String,
    pub user_access_level: UserAccessLevel,
    #[serde(default)]
    pub extra: ExtraAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all_fields = "camelCase")]
pub enum Evaluation {
    String {
        value: String,
        case_sensitivity: CaseSensitivity,
    },
    Numeric {
        threshold: f64,
        operator: ComparisonOperator,
        epsilon: f64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Condition {
    /// >= 1, unique within the rule.
    pub id: u32,
    pub label: String,
    pub enabled: bool,
    pub event: String,
    pub subsystem: String,
    pub component: String,
    pub signal: String,
    pub evaluation: Evaluation,
    #[serde(default)]
    pub extra: ExtraAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    /// >= 1, unique within the config.
    pub id: u32,
    pub label: String,
    pub description: String,
    pub action: RuleAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub combinator: Option<Combinator>,
    pub enabled: bool,
    pub priority: u32,
    pub cooldown_ms: u32,
    /// Absent when the on-disk value is 0 (unlimited).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_fires_per_job: Option<u32>,
    pub conditions: Vec<Condition>,
    #[serde(default)]
    pub extra: ExtraAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TriggerConfig {
    pub enabled: bool,
    /// Absent when the on-disk value is 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_ceiling_layers: Option<u32>,
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub extra: ExtraAttrs,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpcuaConfig {
    pub client: ClientConfig,
    pub pipe: PipeConfig,
    pub triggers: TriggerConfig,
    /// Unknown root-group attributes (the schema version attribute is not included).
    #[serde(default)]
    pub extra: ExtraAttrs,
    /// Only set when unpacked from the legacy layout: trigger sides that could not become rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<Warning>>,
}
