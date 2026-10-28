//! Read rules: writers emit every field; readers accept blanks, reject *missing* only for
//! structural, security/connection and mode-used fields, accept case/separator variants of names
//! and values, and report every default and normalisation with its kind. Built on the `rich`
//! fixture; checks across fields (Advisory / Suspicious) are covered in checks.rs.

mod common;

use common::tree::{f64_attr, group, has, i32_attr, of_kind, remove, rename, set, u32_attr, utf8};
use common::{expect_err, pack_ok, rich_tree, unpack_ok};
use mcf_ext_opcua::{
    unpack, AuthMode, CaseSensitivity, ComparisonOperator, ErrorCode, Evaluation, ExtensionTree,
    MinIntegrityLevel, QueuePolicy, RootNode, SamplingInterval, SecurityMode, SecurityPolicy,
    UserAccessLevel, Warning, WarningKind,
};

const LASER_ON: [&str; 3] = ["Trigger", "Job Start", "Laser On"]; // String condition
const OXYGEN: [&str; 3] = ["Trigger", "Job Start", "Oxygen High"]; // Numeric condition
const JOB_START: [&str; 2] = ["Trigger", "Job Start"];

/// `rich` carries six extra attributes on purpose (Root_Extra, Client_Extra_I64, ...) to test
/// that unknown attributes survive; each one warns as UnknownName.
const RICH_EXTRAS: usize = 6;

/// Read-time notes (Defaulted, Normalized, Conversion, and UnknownName other than the fixture's
/// deliberate extras), rendered "path: message". Advisory / Suspicious belong to checks.rs.
fn reads(ws: &Option<Vec<Warning>>) -> Vec<String> {
    ws.iter()
        .flatten()
        .filter(|w| !matches!(w.kind, WarningKind::Advisory | WarningKind::Suspicious))
        .filter(|w| !(w.kind == WarningKind::UnknownName && w.path.contains("_Extra")))
        .map(Warning::to_string)
        .collect()
}

fn read_notes(tree: &ExtensionTree) -> Vec<String> {
    reads(&unpack_ok(tree).warnings)
}

#[test]
fn canonical_fixture_reads_cleanly_apart_from_its_deliberate_extras() {
    let c = unpack_ok(&rich_tree());
    let unknown = of_kind(&c.warnings, WarningKind::UnknownName);
    assert_eq!(unknown.len(), RICH_EXTRAS, "{unknown:#?}");
    assert!(
        unknown
            .iter()
            .all(|w| w.contains("unknown attribute, kept but not used")),
        "{unknown:#?}"
    );
    assert_eq!(reads(&c.warnings), Vec::<String>::new());
}

// ---- missing ----

#[test]
fn missing_security_field_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &["Client"], "Security_Mode");
    expect_err(unpack(&t), ErrorCode::Validation, "Client/@Security_Mode");
}

#[test]
fn missing_evaluation_mode_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &LASER_ON, "Evaluation_Mode");
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/Laser On/@Evaluation_Mode",
    );
}

#[test]
fn missing_mode_used_field_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &LASER_ON, "Case_Sensitivity");
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/Laser On/@Case_Sensitivity",
    );

    let mut t = rich_tree();
    remove(&mut t, &OXYGEN, "Comparison_Operator");
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/Oxygen High/@Comparison_Operator",
    );
}

#[test]
fn missing_mode_unused_and_cosmetic_fields_default_with_a_warning() {
    let mut t = rich_tree();
    remove(&mut t, &LASER_ON, "Comparison_Operator");
    remove(&mut t, &LASER_ON, "Epsilon");
    remove(&mut t, &JOB_START, "Cooldown_Period");
    remove(&mut t, &JOB_START, "Description");
    let c = unpack_ok(&t);
    let defaulted = of_kind(&c.warnings, WarningKind::Defaulted);
    assert_eq!(defaulted.len(), 4, "{defaulted:#?}");
    assert!(has(
        &defaulted,
        "Trigger/Job Start/@Cooldown_Period: missing, read as 0"
    ));
    assert!(has(
        &defaulted,
        "Laser On/@Comparison_Operator: missing, read as \"Equal\""
    ));
}

#[test]
fn missing_combinator_with_several_conditions_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &JOB_START, "Combinator");
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/@Combinator",
    );
}

#[test]
fn combinator_under_another_spelling_counts_as_present() {
    let mut t = rich_tree();
    rename(&mut t, &JOB_START, "Combinator", "combinator");
    let c = unpack_ok(&t);
    assert!(c.triggers.rules[0].combinator.is_some());
    assert!(has(
        &reads(&c.warnings),
        "@combinator: attribute name read as Combinator"
    ));
}

// ---- blank ----

#[test]
fn blank_mode_unused_field_is_silent() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Comparison_Operator", utf8(""));
    set(&mut t, &OXYGEN, "Case_Sensitivity", utf8(""));
    assert_eq!(read_notes(&t), Vec::<String>::new());
}

#[test]
fn blank_mode_used_field_reads_as_default_with_a_warning() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Case_Sensitivity", utf8(""));
    let c = unpack_ok(&t);
    let cond = c
        .triggers
        .rules
        .iter()
        .flat_map(|r| &r.conditions)
        .find(|c| c.label == "Laser On")
        .unwrap();
    assert!(matches!(
        cond.evaluation,
        Evaluation::String {
            case_sensitivity: CaseSensitivity::Exact,
            ..
        }
    ));
    assert!(has(
        &of_kind(&c.warnings, WarningKind::Defaulted),
        "Laser On/@Case_Sensitivity: blank, read as \"Exact\""
    ));
}

#[test]
fn blank_security_fields_read_as_strictest() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Security_Mode", utf8(""));
    set(&mut t, &["Pipe"], "User_Access_Level", utf8(""));
    set(&mut t, &["Pipe"], "Min_Integrity_Level", utf8(""));
    let c = unpack_ok(&t);
    assert_eq!(c.client.security_mode, SecurityMode::SignAndEncrypt);
    assert_eq!(c.pipe.user_access_level, UserAccessLevel::CurrentUserOnly);
    assert_eq!(
        c.pipe.min_integrity_level,
        MinIntegrityLevel::ProtectedProcess
    );
    assert_eq!(of_kind(&c.warnings, WarningKind::Defaulted).len(), 3);
}

#[test]
fn blank_values_that_are_values_are_silent() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Security_Policy", utf8(""));
    set(&mut t, &["Client"], "Machine_Profile", utf8(""));
    set(&mut t, &["Client"], "Root_Node", utf8(""));
    let c = unpack_ok(&t);
    assert_eq!(c.client.security_policy, SecurityPolicy::Auto);
    assert_eq!(c.client.machine_profile, None);
    assert_eq!(c.client.root_node, RootNode::AutoDiscover);
    assert_eq!(reads(&c.warnings), Vec::<String>::new());
}

#[test]
fn blank_action_stays_an_error() {
    let mut t = rich_tree();
    set(&mut t, &JOB_START, "Action", utf8(""));
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/@Action",
    );
}

#[test]
fn blank_combinator_with_several_conditions_is_an_error() {
    let mut t = rich_tree();
    set(&mut t, &JOB_START, "Combinator", utf8(""));
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/@Combinator",
    );
}

// ---- spellings, casing, dtypes ----

#[test]
fn consumer_spellings_normalize_with_a_warning() {
    let mut t = rich_tree();
    set(&mut t, &["Client"], "Auth_Mode", utf8("username"));
    set(&mut t, &["Client"], "Security_Policy", utf8("NistP384"));
    set(&mut t, &["Client"], "Queue_Policy", utf8("DropOldest"));
    set(&mut t, &["Pipe"], "Min_Integrity_Level", utf8("0x2000"));
    set(&mut t, &["Client"], "Machine_Profile", utf8("EOS"));
    let c = unpack_ok(&t);
    assert_eq!(c.client.auth_mode, AuthMode::UsernamePassword);
    assert_eq!(c.client.security_policy, SecurityPolicy::EccNistP384);
    assert_eq!(c.client.queue_policy, QueuePolicy::DiscardOldest);
    assert_eq!(c.pipe.min_integrity_level, MinIntegrityLevel::Medium);
    let normalized = of_kind(&c.warnings, WarningKind::Normalized);
    assert_eq!(
        normalized.len(),
        5,
        "casing (EOS -> Eos) is reported too: {normalized:#?}"
    );
    assert!(has(
        &normalized,
        "Client/@Security_Policy: \"NistP384\" read as \"ECC_nistP384\""
    ));
    assert!(has(
        &normalized,
        "Client/@Machine_Profile: \"EOS\" read as \"Eos\" (casing)"
    ));
}

#[test]
fn any_casing_reads_with_a_casing_warning() {
    let mut t = rich_tree();
    set(
        &mut t,
        &["Pipe"],
        "User_Access_Level",
        utf8("currentuseronly"),
    );
    let c = unpack_ok(&t);
    assert_eq!(c.pipe.user_access_level, UserAccessLevel::CurrentUserOnly);
    assert!(has(
        &of_kind(&c.warnings, WarningKind::Normalized),
        "Pipe/@User_Access_Level: \"currentuseronly\" read as \"CurrentUserOnly\" (casing)"
    ));
}

#[test]
fn integral_dtypes_widen_with_a_warning_and_keep_range_checks() {
    let mut t = rich_tree();
    set(&mut t, &["Pipe"], "Pipe_Enabled", i32_attr(1));
    let c = unpack_ok(&t);
    assert!(c.pipe.enabled);
    assert!(has(
        &of_kind(&c.warnings, WarningKind::Normalized),
        "Pipe/@Pipe_Enabled: dtype i32 read as u32"
    ));

    let mut t = rich_tree();
    set(&mut t, &["Pipe"], "Buffer_Size", i32_attr(-1));
    expect_err(unpack(&t), ErrorCode::Validation, "Pipe/@Buffer_Size");
}

// ---- full range ----

#[test]
fn full_range_values_round_trip() {
    // Security_Policy None needs Security_Mode None (coherence); the Aes policies need a signing mode.
    for (mode, policy) in [
        ("None", "None"),
        ("SignAndEncrypt", "Aes128_Sha256_RsaOaep"),
        ("Sign", "Aes256_Sha256_RsaPss"),
    ] {
        let mut t = rich_tree();
        set(&mut t, &["Client"], "Security_Mode", utf8(mode));
        set(&mut t, &["Client"], "Security_Policy", utf8(policy));
        set(&mut t, &OXYGEN, "Comparison_Operator", utf8("NotEqual"));
        let c = unpack_ok(&t);
        assert_eq!(c.client.security_policy.as_str(), policy);
        assert_eq!(
            unpack_ok(&pack_ok(&c)).client.security_policy,
            c.client.security_policy
        );
        let cond = c
            .triggers
            .rules
            .iter()
            .flat_map(|r| &r.conditions)
            .find(|c| c.label == "Oxygen High")
            .unwrap();
        assert!(matches!(
            cond.evaluation,
            Evaluation::Numeric {
                operator: ComparisonOperator::NotEqual,
                ..
            }
        ));
    }
}

#[test]
fn negative_epsilon_is_an_error() {
    let mut t = rich_tree();
    set(&mut t, &OXYGEN, "Epsilon", f64_attr(-0.5));
    expect_err(
        unpack(&t),
        ErrorCode::Validation,
        "Trigger/Job Start/Oxygen High/@Epsilon",
    );
}

// ---- attribute names ----

#[test]
fn case_and_separator_variants_of_a_name_are_accepted_with_a_warning() {
    let mut t = rich_tree();
    rename(&mut t, &JOB_START, "Cooldown_Period", "cooldownPeriod");
    let c = unpack_ok(&t);
    assert_eq!(c.triggers.rules[0].cooldown_ms, 500);
    assert!(has(
        &reads(&c.warnings),
        "@cooldownPeriod: attribute name read as Cooldown_Period"
    ));
}

#[test]
fn a_duplicate_spelling_is_reported_and_the_canonical_name_wins() {
    let mut t = rich_tree();
    set(&mut t, &JOB_START, "cooldown_period", u32_attr(9));
    let c = unpack_ok(&t);
    assert_eq!(c.triggers.rules[0].cooldown_ms, 500);
    assert!(has(
        &reads(&c.warnings),
        "@cooldown_period: duplicate spelling of Cooldown_Period"
    ));
}

#[test]
fn a_transposition_is_suggested_not_accepted() {
    let mut t = rich_tree();
    rename(&mut t, &JOB_START, "Cooldown_Period", "Cooldown_Peroid");
    let c = unpack_ok(&t);
    assert_eq!(
        c.triggers.rules[0].cooldown_ms, 0,
        "a near miss is never applied"
    );
    assert!(has(
        &of_kind(&c.warnings, WarningKind::UnknownName),
        "@Cooldown_Peroid: unknown attribute (did you mean Cooldown_Period?)"
    ));
    assert!(has(
        &of_kind(&c.warnings, WarningKind::Defaulted),
        "@Cooldown_Period: missing, read as 0"
    ));
}

#[test]
fn missing_required_field_names_its_near_miss() {
    let mut t = rich_tree();
    rename(&mut t, &["Client"], "Security_Mode", "Securty_Mode");
    let e = unpack(&t).unwrap_err();
    assert_eq!(e.path, "Client/@Security_Mode");
    assert!(e.message.contains("(found Securty_Mode)"), "{e}");
}

#[test]
fn ambiguous_suggestions_are_not_made() {
    // Three edits from both Sync_Loop_Interval_Initial and Sync_Loop_Interval_Settled: a tie,
    // so no suggestion points at either.
    let mut t = rich_tree();
    set(
        &mut t,
        &["Client"],
        "Sync_Loop_Interval_Settial",
        u32_attr(1),
    );
    let unknown = of_kind(&unpack_ok(&t).warnings, WarningKind::UnknownName);
    assert!(
        has(
            &unknown,
            "Client/@Sync_Loop_Interval_Settial: unknown attribute, kept but not used"
        ),
        "{unknown:#?}"
    );
    assert!(
        !has(&unknown, "Settial: unknown attribute (did you mean"),
        "{unknown:#?}"
    );
}

#[test]
fn near_miss_attribute_name_suggests_the_field() {
    let mut t = rich_tree();
    set(&mut t, &JOB_START, "Cooldown_Periood", utf8("5"));
    assert!(has(
        &of_kind(&unpack_ok(&t).warnings, WarningKind::UnknownName),
        "@Cooldown_Periood: unknown attribute (did you mean Cooldown_Period?)"
    ));
}

#[test]
fn unknown_attribute_in_a_newer_minor_file_is_silent() {
    let mut t = rich_tree();
    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.7"));
    set(&mut t, &["Client"], "Brand_New_Setting", utf8("x"));
    // A newer-minor file reports nothing unknown, including the fixture's extras.
    assert_eq!(
        of_kind(&unpack_ok(&t).warnings, WarningKind::UnknownName),
        Vec::<String>::new()
    );

    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.2"));
    assert!(has(
        &of_kind(&unpack_ok(&t).warnings, WarningKind::UnknownName),
        "Client/@Brand_New_Setting: unknown attribute, kept but not used"
    ));
}

// ---- on-disk encodings for consumers ----

#[test]
fn to_spec_matches_the_on_disk_encoding() {
    assert_eq!(RootNode::AutoDiscover.to_spec(), "");
    assert_eq!(
        RootNode::NodeId {
            value: "ns=2;s=X".into()
        }
        .to_spec(),
        "ns=2;s=X"
    );
    assert_eq!(SamplingInterval::ServerNegotiated.to_spec(), -1);
    assert_eq!(SamplingInterval::Fastest.to_spec(), 0);
    assert_eq!(SamplingInterval::Ms { ms: 250 }.to_spec(), 250);
    assert_eq!(SecurityPolicy::Auto.to_spec(), "");
    assert_eq!(SecurityPolicy::None.to_spec(), "None");
}

// `group` is re-exported for tests that edit a subtree directly.
#[allow(unused_imports)]
use group as _;

#[test]
fn an_unknown_value_in_a_newer_minor_file_names_the_version_it_needs() {
    let mut t = rich_tree();
    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.7"));
    set(&mut t, &JOB_START, "Action", utf8("AnnotateEvent"));
    let e = unpack(&t).unwrap_err();
    assert_eq!(e.path, "Trigger/Job Start/@Action");
    assert!(
        e.message
            .contains("the file declares schema 1.7, newer than this reader (1.2)"),
        "{e}"
    );

    // Same value in a current-minor file is a plain invalid value (most likely a typo).
    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.2"));
    let e = unpack(&t).unwrap_err();
    assert!(!e.message.contains("newer than this reader"), "{e}");
}
