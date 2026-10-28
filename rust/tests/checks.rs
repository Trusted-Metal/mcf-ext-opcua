//! Checks across fields and against sensible ranges (src/checks.rs): contradictory security
//! pairings are errors on read and on pack; weaker-than-preferred settings are Advisory; legal but
//! unlikely values are Suspicious. `rich` itself produces exactly one Advisory (AnyLocalUser).

mod common;

use common::tree::{has, of_kind, set, u32_attr, utf8};
use common::{expect_err, rich_model, rich_tree, unpack_ok};
use mcf_ext_opcua::{
    pack, unpack, ErrorCode, PackOptions, SecurityMode, SecurityPolicy, WarningKind,
};

const LASER_ON: [&str; 3] = ["Trigger", "Job Start", "Laser On"];

fn advisories(t: &mcf_ext_opcua::ExtensionTree) -> Vec<String> {
    of_kind(&unpack_ok(t).warnings, WarningKind::Advisory)
}

fn suspicious(t: &mcf_ext_opcua::ExtensionTree) -> Vec<String> {
    of_kind(&unpack_ok(t).warnings, WarningKind::Suspicious)
}

#[test]
fn rich_fixture_has_one_advisory_and_nothing_suspicious() {
    let t = rich_tree();
    let adv = advisories(&t);
    assert_eq!(adv.len(), 1, "{adv:#?}");
    assert!(has(&adv, "Pipe/@User_Access_Level: AnyLocalUser"));
    assert_eq!(suspicious(&t), Vec::<String>::new());
}

// ---- coherence (errors) ----

#[test]
fn mode_none_with_a_real_policy_is_an_error() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Security_Mode", utf8("None"));
    set(
        &mut t,
        &["Client"],
        "Security_Policy",
        utf8("Basic256Sha256"),
    );
    expect_err(unpack(&t), ErrorCode::Validation, "Client/@Security_Policy");
}

#[test]
fn policy_none_with_a_signing_mode_is_an_error() {
    for mode in ["Sign", "SignAndEncrypt"] {
        let mut t = rich_tree();
        set(&mut t, &["Client"], "Security_Mode", utf8(mode));
        set(&mut t, &["Client"], "Security_Policy", utf8("None"));
        expect_err(unpack(&t), ErrorCode::Validation, "Client/@Security_Policy");
    }
}

#[test]
fn blank_policy_is_coherent_with_every_mode() {
    for mode in ["None", "Sign", "SignAndEncrypt"] {
        let mut t = rich_tree();
        set(&mut t, &["Client"], "Security_Mode", utf8(mode));
        set(&mut t, &["Client"], "Security_Policy", utf8(""));
        assert_eq!(
            unpack_ok(&t).client.security_policy,
            SecurityPolicy::Auto,
            "{mode}"
        );
    }
}

#[test]
fn no_security_is_coherent_but_advisory() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Security_Mode", utf8("None"));
    set(&mut t, &["Client"], "Security_Policy", utf8("None"));
    let c = unpack_ok(&t);
    assert_eq!(c.client.security_mode, SecurityMode::None);
    assert!(has(
        &of_kind(&c.warnings, WarningKind::Advisory),
        "Client/@Security_Mode: None"
    ));
}

#[test]
fn a_password_without_message_security_is_advisory() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Security_Mode", utf8("None"));
    set(&mut t, &["Client"], "Security_Policy", utf8("None"));
    assert!(has(
        &advisories(&t),
        "Client/@Auth_Mode: UsernamePassword over Security_Mode None"
    ));

    set(&mut t, &["Client"], "Auth_Mode", utf8("Anonymous"));
    assert!(!has(&advisories(&t), "Client/@Auth_Mode"));
}

#[test]
fn pack_refuses_an_incoherent_model() {
    let mut c = rich_model();
    c.client.security_mode = SecurityMode::None;
    c.client.security_policy = SecurityPolicy::EccNistP256;
    let e = pack(&c, &PackOptions::default()).unwrap_err();
    assert_eq!(
        (e.code, e.path.as_str()),
        (ErrorCode::Validation, "Client/@Security_Policy")
    );
}

// ---- advisory ----

#[test]
fn weak_pipe_security_is_advisory() {
    let mut t = rich_tree();
    set(&mut t, &["Pipe"], "Min_Integrity_Level", utf8("Low"));
    let adv = advisories(&t);
    assert!(
        has(&adv, "Pipe/@User_Access_Level: AnyLocalUser"),
        "{adv:#?}"
    );
    assert!(has(&adv, "Pipe/@Min_Integrity_Level: Low"), "{adv:#?}");
}

#[test]
fn unlimited_message_size_is_advisory() {
    let mut t = rich_tree();
    set(&mut t, &["Pipe"], "Max_Inbound_Message_Size", u32_attr(0));
    assert!(has(
        &advisories(&t),
        "Pipe/@Max_Inbound_Message_Size: 0: no message size limit"
    ));
}

#[test]
fn start_without_any_stop_or_ceiling_is_advisory() {
    // rich's only CaptureStop rule is disabled; clearing the ceiling leaves nothing to stop a capture.
    let mut t = rich_tree();
    set(
        &mut t,
        &["Trigger"],
        "Trigger_Stop_Ceiling_Layers",
        u32_attr(0),
    );
    assert!(has(
        &advisories(&t),
        "Trigger: rules can start a capture but none can stop it"
    ));
}

// ---- suspicious ----

#[test]
fn zero_intervals_and_a_non_opc_url_are_suspicious() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Reconnect_Interval", u32_attr(0));
    set(
        &mut t,
        &["Client"],
        "Server_URL",
        utf8("http://10.0.0.5:62541"),
    );
    let s = suspicious(&t);
    assert!(
        has(
            &s,
            "Client/@Reconnect_Interval: 0 is almost certainly unintended"
        ),
        "{s:#?}"
    );
    assert!(
        has(
            &s,
            "Client/@Server_URL: \"http://10.0.0.5:62541\" is not an OPC UA endpoint URL"
        ),
        "{s:#?}"
    );
}

#[test]
fn a_short_lifetime_is_suspicious() {
    // rich: Keep_Alive_Count 100, so anything below 300 is revised by the server.
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Lifetime_Count", u32_attr(100));
    assert!(has(
        &suspicious(&t),
        "Client/@Lifetime_Count: 100 is below 3 x Keep_Alive_Count (100)"
    ));
}

#[test]
fn an_enabled_rule_with_every_condition_disabled_is_suspicious() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Condition_Enabled", u32_attr(0)); // Oxygen High is already disabled
    assert!(has(
        &suspicious(&t),
        "Trigger/Job Start: enabled, but every condition is disabled"
    ));
}

#[test]
fn empty_signal_on_an_enabled_condition_is_an_error() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Signal", utf8(""));
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/Laser On/@Signal",
    );
}

#[test]
fn empty_signal_on_a_disabled_condition_is_allowed() {
    // Oxygen High is disabled in rich: a placeholder without a signal is fine.
    let mut t = rich_tree();
    set(
        &mut t,
        &["Trigger", "Job Start", "Oxygen High"],
        "Signal",
        utf8(""),
    );
    unpack_ok(&t);
}

#[test]
fn empty_string_value_on_an_enabled_condition_is_suspicious() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Value", utf8(""));
    assert!(has(
        &suspicious(&t),
        "Trigger/Job Start/Laser On/@Value: empty"
    ));
}

#[test]
fn pack_refuses_a_multi_condition_rule_without_a_combinator() {
    let mut c = rich_model();
    c.triggers.rules[0].combinator = None; // Job Start has two conditions
    let e = pack(&c, &PackOptions::default()).unwrap_err();
    assert_eq!(
        (e.code, e.path.as_str()),
        (ErrorCode::Validation, "Trigger/Job Start/@Combinator")
    );
}

#[test]
fn checks_never_reject_a_coherent_config() {
    // Every Advisory / Suspicious case at once still loads.
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Reconnect_Interval", u32_attr(0));
    set(&mut t, &["Client"], "Lifetime_Count", u32_attr(1));
    set(&mut t, &["Pipe"], "Min_Integrity_Level", utf8("Untrusted"));
    set(&mut t, &["Pipe"], "Max_Inbound_Message_Size", u32_attr(0));
    set(
        &mut t,
        &["Trigger"],
        "Trigger_Stop_Ceiling_Layers",
        u32_attr(0),
    );
    let c = unpack_ok(&t);
    assert!(!of_kind(&c.warnings, WarningKind::Advisory).is_empty());
    assert!(!of_kind(&c.warnings, WarningKind::Suspicious).is_empty());
}
