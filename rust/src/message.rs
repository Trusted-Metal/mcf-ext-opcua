//! Messages derived from the model, for consumers that receive config over a transport instead of
//! reading the file. Keys and values use the on-disk (spec) encoding: the versioned contract.
//! `schema` names the schema version that encoding follows, so receivers can check compatibility.

use serde::{Deserialize, Serialize};

use crate::model::{ClientConfig, MachineProfile};
use crate::schema_v1::CURRENT_SCHEMA_VERSION;

pub const CLIENT_CONFIG_MESSAGE_TYPE: &str = "client_config";

/// The Client group as sent from capture to client-proj. Keys are the attribute names, lower-cased.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientConfigMessage {
    #[serde(rename = "type")]
    pub kind: String,
    pub schema: String,
    pub auth_mode: String,
    pub bfs_max_depth: u32,
    pub keep_alive_count: u32,
    pub lifetime_count: u32,
    /// "" when unset (not null: receivers treat null as "keep the previous value").
    pub machine_profile: String,
    pub publish_interval: u32,
    pub queue_policy: String,
    pub queue_size_data_change: u32,
    pub queue_size_events: u32,
    pub reconnect_interval: u32,
    pub root_node: String,
    pub sampling_interval: i32,
    pub security_mode: String,
    /// "" for Auto (SDK auto-select), never "Auto".
    pub security_policy: String,
    pub server_url: String,
    pub session_timeout: u32,
    pub sync_loop_interval_initial: u32,
    pub sync_loop_interval_settled: u32,
}

impl ClientConfig {
    /// This group in its on-disk encoding, as a `client_config` message.
    pub fn to_message(&self) -> ClientConfigMessage {
        // Exhaustive destructure: a field added to the model fails to compile here until it is
        // mapped (or explicitly ignored), so the message cannot silently fall behind the model.
        let ClientConfig {
            server_url,
            auth_mode,
            machine_profile,
            security_mode,
            security_policy,
            root_node,
            sampling_interval,
            publish_interval,
            bfs_max_depth,
            keep_alive_count,
            lifetime_count,
            queue_policy,
            queue_size_data_change,
            queue_size_events,
            reconnect_interval,
            session_timeout,
            sync_loop_interval_initial,
            sync_loop_interval_settled,
            extra: _,
        } = self;
        ClientConfigMessage {
            kind: CLIENT_CONFIG_MESSAGE_TYPE.to_string(),
            schema: CURRENT_SCHEMA_VERSION.to_string(),
            auth_mode: auth_mode.as_str().to_string(),
            bfs_max_depth: *bfs_max_depth,
            keep_alive_count: *keep_alive_count,
            lifetime_count: *lifetime_count,
            machine_profile: machine_profile
                .map_or("", MachineProfile::as_str)
                .to_string(),
            publish_interval: *publish_interval,
            queue_policy: queue_policy.as_str().to_string(),
            queue_size_data_change: *queue_size_data_change,
            queue_size_events: *queue_size_events,
            reconnect_interval: *reconnect_interval,
            root_node: root_node.to_spec().to_string(),
            sampling_interval: sampling_interval.to_spec(),
            security_mode: security_mode.as_str().to_string(),
            security_policy: security_policy.to_spec().to_string(),
            server_url: server_url.clone(),
            session_timeout: *session_timeout,
            sync_loop_interval_initial: *sync_loop_interval_initial,
            sync_loop_interval_settled: *sync_loop_interval_settled,
        }
    }
}
