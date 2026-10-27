use crate::model::{
    AuthMode, MachineProfile, MinIntegrityLevel, QueuePolicy, RootNode, SamplingInterval,
    SecurityMode, SecurityPolicy, UserAccessLevel,
};

/// Values shared by the reader (what a missing or blank field reads as) and `suggested_defaults`.
/// Change them here only; `schema_v1.rs` and the parity tests read the same constants.
pub(crate) mod base {
    use crate::model::{AuthMode, MinIntegrityLevel, QueuePolicy, SecurityMode, UserAccessLevel};

    // ---- Client ----
    pub const BFS_MAX_DEPTH: u32 = 16;
    pub const KEEP_ALIVE_COUNT: u32 = 240;
    pub const LIFETIME_COUNT: u32 = 2400;
    pub const PUBLISH_INTERVAL_MS: u32 = 250;
    pub const QUEUE_POLICY: QueuePolicy = QueuePolicy::DiscardOldest;
    pub const QUEUE_SIZE_DATA_CHANGE: u32 = 10;
    pub const QUEUE_SIZE_EVENTS: u32 = 10;
    pub const RECONNECT_INTERVAL_MS: u32 = 1000;
    pub const SAMPLING_INTERVAL_MS: u32 = 250;
    pub const SESSION_TIMEOUT_MS: u32 = 60_000;
    pub const SYNC_LOOP_INTERVAL_INITIAL_MS: u32 = 1000;
    pub const SYNC_LOOP_INTERVAL_SETTLED_MS: u32 = 30_000;

    // ---- Pipe ----
    pub const BUFFER_SIZE: u32 = 65_536;
    pub const INBOUND_RATE_LIMIT_HZ: u32 = 100;
    pub const MAX_INBOUND_MESSAGE_SIZE: u32 = 4096;
    pub const PIPE_NAME: &str = r"\\.\pipe\opc_ua_client_pipe";

    // ---- Strictest security posture (blank reads and suggestions) ----
    pub const SECURITY_MODE: SecurityMode = SecurityMode::SignAndEncrypt;
    pub const USER_ACCESS_LEVEL: UserAccessLevel = UserAccessLevel::CurrentUserOnly;
    pub const MIN_INTEGRITY_LEVEL: MinIntegrityLevel = MinIntegrityLevel::ProtectedProcess;
    pub const AUTH_MODE: AuthMode = AuthMode::Certificate;

    // ---- Profile-specific suggestions (never read defaults) ----
    pub const ACONITY_SAMPLING_INTERVAL_MS: u32 = 50;
    pub const ACONITY_QUEUE_SIZE_DATA_CHANGE: u32 = 50;
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestedClientDefaults {
    pub auth_mode: AuthMode,
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestedPipeDefaults {
    pub enabled: bool,
    pub configure_client: bool,
    pub buffer_size: u32,
    pub inbound_rate_limit_hz: Option<u32>,
    pub max_inbound_message_size: u32,
    pub min_integrity_level: MinIntegrityLevel,
    pub pipe_name: String,
    pub user_access_level: UserAccessLevel,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SuggestedDefaults {
    pub client: SuggestedClientDefaults,
    pub pipe: SuggestedPipeDefaults,
}

/// Documented defaults for building a new config (e.g. in a UI). `pack` never applies them.
/// Security follows the preferred posture (SignAndEncrypt, policy chosen by the SDK) and identity
/// remains at Certificate in-keeping with highest security options
pub fn suggested_defaults(profile: Option<MachineProfile>) -> SuggestedDefaults {
    use base::*;
    let aconity = profile == Some(MachineProfile::Aconity);
    SuggestedDefaults {
        client: SuggestedClientDefaults {
            auth_mode: AUTH_MODE,
            security_mode: SECURITY_MODE,
            security_policy: SecurityPolicy::Auto,
            root_node: RootNode::AutoDiscover,
            sampling_interval: SamplingInterval::Ms {
                ms: if aconity {
                    ACONITY_SAMPLING_INTERVAL_MS
                } else {
                    SAMPLING_INTERVAL_MS
                },
            },
            publish_interval: PUBLISH_INTERVAL_MS,
            bfs_max_depth: BFS_MAX_DEPTH,
            keep_alive_count: KEEP_ALIVE_COUNT,
            lifetime_count: LIFETIME_COUNT,
            queue_policy: QUEUE_POLICY,
            queue_size_data_change: if aconity {
                ACONITY_QUEUE_SIZE_DATA_CHANGE
            } else {
                QUEUE_SIZE_DATA_CHANGE
            },
            queue_size_events: QUEUE_SIZE_EVENTS,
            reconnect_interval: RECONNECT_INTERVAL_MS,
            session_timeout: SESSION_TIMEOUT_MS,
            sync_loop_interval_initial: SYNC_LOOP_INTERVAL_INITIAL_MS,
            sync_loop_interval_settled: SYNC_LOOP_INTERVAL_SETTLED_MS,
        },
        pipe: SuggestedPipeDefaults {
            enabled: false,
            configure_client: false,
            buffer_size: BUFFER_SIZE,
            inbound_rate_limit_hz: Some(INBOUND_RATE_LIMIT_HZ),
            max_inbound_message_size: MAX_INBOUND_MESSAGE_SIZE,
            min_integrity_level: MIN_INTEGRITY_LEVEL,
            pipe_name: PIPE_NAME.to_string(),
            user_access_level: USER_ACCESS_LEVEL,
        },
    }
}
