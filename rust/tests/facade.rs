mod common;

use common::tree;
use common::*;
use mcf_ext_opcua::{
    pack, peek_schema_version, suggested_defaults, supported_schema_versions, unpack, ClientConfig,
    ErrorCode, MachineProfile, OpcuaConfig, PackOptions, PipeConfig, SamplingInterval,
    TriggerConfig, CURRENT_SCHEMA_VERSION, EXTENSION_NAME, REQUIRES_CONTRACT,
};
use serde_json::json;

fn with_version(version: &str) -> mcf_ext_opcua::ExtensionTree {
    let mut v = rich_json();
    set(&mut v, "/attrs/Opcua_Schema_Version/value", json!(version));
    tree_from(v)
}

#[test]
fn exports_constants() {
    assert_eq!(EXTENSION_NAME, "TM_OPCUA");
    assert_eq!(REQUIRES_CONTRACT, "^0.1");
    assert_eq!(CURRENT_SCHEMA_VERSION, "1.2");
    assert_eq!(supported_schema_versions(), ["1.2"]);
}

#[test]
fn peek_schema_version_reports_the_stored_value() {
    assert_eq!(peek_schema_version(&rich_tree()), "1.2");
    assert_eq!(peek_schema_version(&with_version("3.1")), "3.1");
    let mut v = rich_json();
    remove(&mut v, "/attrs/Opcua_Schema_Version");
    assert_eq!(peek_schema_version(&tree_from(v)), "legacy");
}

#[test]
fn any_1x_minor_is_read_by_the_v1_adapter() {
    for v in ["1.0", "1.2", "1.9", "01.2"] {
        assert!(unpack(&with_version(v)).is_ok(), "{v}");
    }
}

#[test]
fn pack_target_schema() {
    let opts = |t: &str| PackOptions {
        target_schema: Some(t.into()),
    };
    assert!(pack(&rich_model(), &opts("1.2")).is_ok());
    assert!(pack(&rich_model(), &PackOptions::default()).is_ok());
    expect_err(
        pack(&rich_model(), &opts("legacy")),
        ErrorCode::LossyDowngrade,
        "",
    );
    expect_err(
        pack(&rich_model(), &opts("2.0")),
        ErrorCode::UnsupportedSchemaVersion,
        "",
    );
}

#[test]
fn suggested_defaults_differ_for_aconity_only_where_documented() {
    let base = suggested_defaults(None);
    let aconity = suggested_defaults(Some(MachineProfile::Aconity));
    assert_eq!(
        base.client.sampling_interval,
        SamplingInterval::Ms { ms: 250 }
    );
    assert_eq!(
        aconity.client.sampling_interval,
        SamplingInterval::Ms { ms: 50 }
    );
    assert_eq!(base.client.queue_size_data_change, 10);
    assert_eq!(aconity.client.queue_size_data_change, 50);
    assert_eq!(suggested_defaults(Some(MachineProfile::Eos)), base);
}

#[test]
fn suggested_defaults_produce_a_packable_config() {
    let d = suggested_defaults(None);
    let c = &d.client;
    let p = &d.pipe;
    let config = OpcuaConfig {
        client: ClientConfig {
            server_url: "opc.tcp://h:1".into(),
            auth_mode: c.auth_mode,
            machine_profile: None,
            security_mode: c.security_mode,
            security_policy: c.security_policy,
            root_node: c.root_node.clone(),
            sampling_interval: c.sampling_interval.clone(),
            publish_interval: c.publish_interval,
            bfs_max_depth: c.bfs_max_depth,
            keep_alive_count: c.keep_alive_count,
            lifetime_count: c.lifetime_count,
            queue_policy: c.queue_policy,
            queue_size_data_change: c.queue_size_data_change,
            queue_size_events: c.queue_size_events,
            reconnect_interval: c.reconnect_interval,
            session_timeout: c.session_timeout,
            sync_loop_interval_initial: c.sync_loop_interval_initial,
            sync_loop_interval_settled: c.sync_loop_interval_settled,
            extra: Default::default(),
        },
        pipe: PipeConfig {
            enabled: p.enabled,
            configure_client: p.configure_client,
            buffer_size: p.buffer_size,
            inbound_rate_limit_hz: p.inbound_rate_limit_hz,
            max_inbound_message_size: p.max_inbound_message_size,
            min_integrity_level: p.min_integrity_level,
            pipe_name: p.pipe_name.clone(),
            user_access_level: p.user_access_level,
            extra: Default::default(),
        },
        triggers: TriggerConfig {
            enabled: false,
            stop_ceiling_layers: None,
            rules: vec![],
            extra: Default::default(),
        },
        extra: Default::default(),
        warnings: None,
    };
    assert_eq!(without_warnings(unpack_ok(&pack_ok(&config))), config);
}

#[test]
fn missing_and_blank_fields_read_as_the_suggested_defaults() {
    let s = suggested_defaults(None);
    let mut t = rich_tree();
    for name in [
        "BFS_Max_Depth",
        "Keep_Alive_Count",
        "Lifetime_Count",
        "Publish_Interval",
        "Queue_Policy",
        "Queue_Size_Data_Change",
        "Queue_Size_Events",
        "Reconnect_Interval",
        "Sampling_Interval",
        "Session_Timeout",
        "Sync_Loop_Interval_Initial",
        "Sync_Loop_Interval_Settled",
    ] {
        tree::remove(&mut t, &["Client"], name);
    }
    for name in [
        "Buffer_Size",
        "Inbound_Rate_Limit",
        "Max_Inbound_Message_Size",
    ] {
        tree::remove(&mut t, &["Pipe"], name);
    }
    tree::set(&mut t, &["Client"], "Security_Mode", tree::utf8(""));
    tree::set(&mut t, &["Pipe"], "User_Access_Level", tree::utf8(""));
    tree::set(&mut t, &["Pipe"], "Min_Integrity_Level", tree::utf8(""));

    let c = unpack_ok(&t);
    let (cl, sc) = (&c.client, &s.client);
    assert_eq!(
        (
            cl.bfs_max_depth,
            cl.keep_alive_count,
            cl.lifetime_count,
            cl.publish_interval
        ),
        (
            sc.bfs_max_depth,
            sc.keep_alive_count,
            sc.lifetime_count,
            sc.publish_interval
        )
    );
    assert_eq!(
        (
            cl.queue_policy,
            cl.queue_size_data_change,
            cl.queue_size_events
        ),
        (
            sc.queue_policy,
            sc.queue_size_data_change,
            sc.queue_size_events
        )
    );
    assert_eq!(
        (
            cl.reconnect_interval,
            cl.session_timeout,
            cl.sync_loop_interval_initial,
            cl.sync_loop_interval_settled
        ),
        (
            sc.reconnect_interval,
            sc.session_timeout,
            sc.sync_loop_interval_initial,
            sc.sync_loop_interval_settled
        )
    );
    assert_eq!(cl.sampling_interval, sc.sampling_interval);
    assert_eq!(cl.security_mode, sc.security_mode);
    let (p, sp) = (&c.pipe, &s.pipe);
    assert_eq!(
        (
            p.buffer_size,
            p.inbound_rate_limit_hz,
            p.max_inbound_message_size
        ),
        (
            sp.buffer_size,
            sp.inbound_rate_limit_hz,
            sp.max_inbound_message_size
        )
    );
    assert_eq!(
        (p.user_access_level, p.min_integrity_level),
        (sp.user_access_level, sp.min_integrity_level)
    );
}
