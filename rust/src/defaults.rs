use crate::model::{
    AuthMode, MachineProfile, MinIntegrityLevel, QueuePolicy, RootNode, SamplingInterval,
    SecurityMode, SecurityPolicy, UserAccessLevel,
};

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
/// Aconity differs from the base values in `sampling_interval` and `queue_size_data_change`.
/// `auth_mode`, `security_mode`, `security_policy` and `root_node` are the neutral choices
/// (Anonymous, None, Auto, AutoDiscover).
pub fn suggested_defaults(profile: Option<MachineProfile>) -> SuggestedDefaults {
    let aconity = profile == Some(MachineProfile::Aconity);
    SuggestedDefaults {
        client: SuggestedClientDefaults {
            auth_mode: AuthMode::Anonymous,
            security_mode: SecurityMode::None,
            security_policy: SecurityPolicy::Auto,
            root_node: RootNode::AutoDiscover,
            sampling_interval: SamplingInterval::Ms {
                ms: if aconity { 50 } else { 250 },
            },
            publish_interval: 250,
            bfs_max_depth: 16,
            keep_alive_count: 240,
            lifetime_count: 2400,
            queue_policy: QueuePolicy::DiscardOldest,
            queue_size_data_change: if aconity { 50 } else { 10 },
            queue_size_events: 10,
            reconnect_interval: 1000,
            session_timeout: 60000,
            sync_loop_interval_initial: 1000,
            sync_loop_interval_settled: 30000,
        },
        pipe: SuggestedPipeDefaults {
            enabled: false,
            configure_client: false,
            buffer_size: 65536,
            inbound_rate_limit_hz: Some(100),
            max_inbound_message_size: 4096,
            min_integrity_level: MinIntegrityLevel::ProtectedProcess,
            pipe_name: "\\\\.\\pipe\\opc_ua_client_pipe".to_string(),
            user_access_level: UserAccessLevel::CurrentUserOnly,
        },
    }
}
