//! Checks across fields and against sensible ranges, run on the unpacked model by both readers
//! (and `coherence` by `pack`). Only combinations that can never work are errors; values that are
//! allowed but weaker than preferred are Advisory, and legal-but-unlikely values are Suspicious.

use crate::errors::{validation, Result};
use crate::model::{
    Evaluation, MinIntegrityLevel, OpcuaConfig, RuleAction, SecurityMode, SecurityPolicy,
    UserAccessLevel, Warning, WarningKind,
};

/// Windows limits a named pipe's full name (including `\\.\pipe\`) to 256 characters.
const WIN_PIPE_NAME_MAX: usize = 256;
/// Servers revise LifetimeCount up to at least this multiple of the keep-alive count.
const MIN_LIFETIME_PER_KEEPALIVE: u32 = 3;
/// Endpoint URL schemes OPC UA defines (compared case-insensitively).
const OPC_UA_URL_SCHEMES: &[&str] = &["opc.tcp://", "opc.https://", "opc.wss://", "https://"];

const AT_SECURITY_MODE: &str = "Client/@Security_Mode";
const AT_SECURITY_POLICY: &str = "Client/@Security_Policy";

fn client_at(name: &str) -> String {
    format!("Client/@{name}")
}
fn pipe_at(name: &str) -> String {
    format!("Pipe/@{name}")
}

/// OPC UA pairs security mode None with policy None (blank/Auto lets the SDK choose either way).
pub(crate) fn coherence(c: &OpcuaConfig) -> Result<()> {
    use SecurityMode as M;
    use SecurityPolicy as P;
    match (c.client.security_mode, c.client.security_policy) {
        (M::None | M::Sign | M::SignAndEncrypt, P::Auto) => Ok(()),
        (M::None, P::None) => Ok(()),
        (M::None, policy) => Err(validation(
            AT_SECURITY_POLICY,
            format!(
                "{} needs Security_Mode Sign or SignAndEncrypt; Security_Mode None pairs with Security_Policy None or blank",
                policy.as_str()
            ),
        )),
        (M::Sign | M::SignAndEncrypt, P::None) => Err(validation(
            AT_SECURITY_POLICY,
            "None cannot be used with Security_Mode Sign or SignAndEncrypt",
        )),
        (M::Sign | M::SignAndEncrypt, _) => Ok(()),
    }
}

pub(crate) fn advise(c: &OpcuaConfig, out: &mut Vec<Warning>) {
    let mut note = |kind, path: String, message: String| out.push(Warning { kind, path, message });
    let (cl, p, t) = (&c.client, &c.pipe, &c.triggers);

    // ---- advisory: allowed, weaker than the preferred posture ----
    if cl.security_mode == SecurityMode::None {
        note(WarningKind::Advisory, client_at("Security_Mode"), "None: OPC UA messages are not signed or encrypted".into());
    }
    if matches!(p.user_access_level, UserAccessLevel::AnyLocalUser | UserAccessLevel::Unrestricted) {
        note(WarningKind::Advisory, pipe_at("User_Access_Level"), format!("{}: the pipe accepts other local users", p.user_access_level.as_str()));
    }
    if matches!(p.min_integrity_level, MinIntegrityLevel::Untrusted | MinIntegrityLevel::Low) {
        note(WarningKind::Advisory, pipe_at("Min_Integrity_Level"), format!("{}: low-integrity processes may connect", p.min_integrity_level.as_str()));
    }
    if p.max_inbound_message_size == 0 {
        note(WarningKind::Advisory, pipe_at("Max_Inbound_Message_Size"), "0: no message size limit".into());
    }

    // ---- suspicious: legal, very likely a mistake ----
    for (name, v) in [
        ("Reconnect_Interval", cl.reconnect_interval),
        ("Sync_Loop_Interval_Initial", cl.sync_loop_interval_initial),
        ("Sync_Loop_Interval_Settled", cl.sync_loop_interval_settled),
        ("Session_Timeout", cl.session_timeout),
        ("Keep_Alive_Count", cl.keep_alive_count),
        ("Lifetime_Count", cl.lifetime_count),
        ("BFS_Max_Depth", cl.bfs_max_depth),
    ] {
        if v == 0 {
            note(WarningKind::Suspicious, format!("Client/@{name}"), "0 is almost certainly unintended".into());
        }
    }
    if p.buffer_size == 0 {
        note(WarningKind::Suspicious, pipe_at("Buffer_Size"), "0: the pipe has no buffer".into());
    }
    if cl.lifetime_count < cl.keep_alive_count.saturating_mul(3) {
        note(
            WarningKind::Suspicious,
            client_at("Lifetime_Count"),
            format!("{} is below {MIN_LIFETIME_PER_KEEPALIVE} x Keep_Alive_Count ({}); servers revise it upward", cl.lifetime_count, cl.keep_alive_count),
        );
    }
    let url = cl.server_url.to_ascii_lowercase();
    if !OPC_UA_URL_SCHEMES.iter().any(|s| url.starts_with(s)) {
        note(WarningKind::Suspicious, client_at("Server_URL"), format!("{:?} is not an OPC UA endpoint URL", cl.server_url));
    }
    if p.pipe_name.len() > WIN_PIPE_NAME_MAX {
        note(WarningKind::Suspicious, pipe_at("Pipe_Name"), format!("longer than Windows' {WIN_PIPE_NAME_MAX}-character limit"));
    }

    // ---- triggers ----
    for r in &t.rules {
        let rule_path = format!("Trigger/{}", r.label);
        if r.enabled && !r.conditions.iter().any(|c| c.enabled) {
            note(WarningKind::Suspicious, rule_path.clone(), "enabled, but every condition is disabled: it can never fire".into());
        }
        for c in r.conditions.iter().filter(|c| c.enabled) {
            let cond_path = format!("{rule_path}/{}", c.label);
            if c.signal.trim().is_empty() {
                note(WarningKind::Suspicious, format!("{cond_path}/@Signal"), "empty: consumers skip this condition".into());
            }
            if let Evaluation::String { value, .. } = &c.evaluation {
                if value.is_empty() {
                    note(WarningKind::Suspicious, format!("{cond_path}/@Value"), "empty: only an empty signal value matches".into());
                }
            }
        }
    }
    let enabled_start = t.rules.iter().any(|r| r.enabled && r.action == RuleAction::CaptureStart);
    let enabled_stop = t.rules.iter().any(|r| r.enabled && r.action == RuleAction::CaptureStop);
    if t.enabled && enabled_start && !enabled_stop && t.stop_ceiling_layers.is_none() {
        note(
            WarningKind::Advisory,
            "Trigger".into(),
            "rules can start a capture but none can stop it, and Trigger_Stop_Ceiling_Layers is unset".into(),
        );
    }
}
