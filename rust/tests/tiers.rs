//! Tiered read rules: writers emit every field; readers accept blanks, reject *missing* only for
//! structural, security/connection and mode-used fields, and warn for everything they default or
//! normalise. Built on the `rich` fixture, whose only warnings are for the six extra attributes
//! it carries on purpose (to test that unknown attributes survive); `warnings` filters those out.

mod common;

use common::{expect_err, pack_ok, rich_tree, unpack_ok};
use mcf_ext_opcua::{
    unpack, Attr, AuthMode, CaseSensitivity, ComparisonOperator, Dtype, ErrorCode, Evaluation,
    ExtensionTree, MinIntegrityLevel, QueuePolicy, RootNode, SamplingInterval, Scalar,
    SecurityMode, SecurityPolicy, UserAccessLevel, Value,
};

const LASER_ON: [&str; 3] = ["Trigger", "Job Start", "Laser On"]; // String condition
const OXYGEN: [&str; 3] = ["Trigger", "Job Start", "Oxygen High"]; // Numeric condition

fn utf8(s: &str) -> Attr {
    Attr { dtype: Dtype::Utf8, value: Value::Scalar(Scalar::Utf8(s.to_string())) }
}

fn f64_attr(x: f64) -> Attr {
    Attr { dtype: Dtype::F64, value: Value::Scalar(Scalar::F64(x)) }
}

fn group<'a>(tree: &'a mut ExtensionTree, path: &[&str]) -> &'a mut ExtensionTree {
    path.iter().fold(tree, |t, name| t.children.get_mut(*name).unwrap())
}

fn set(tree: &mut ExtensionTree, path: &[&str], name: &str, attr: Attr) {
    group(tree, path).attrs.insert(name.to_string(), attr);
}

fn remove(tree: &mut ExtensionTree, path: &[&str], name: &str) {
    assert!(group(tree, path).attrs.remove(name).is_some(), "{name} not in fixture");
}

/// `rich`'s deliberate extras (Root_Extra, Client_Extra_I64, ...) always warn as unknown.
const RICH_EXTRAS: usize = 6;

/// Warnings other than the fixture's deliberate unknown attributes.
fn own(ws: Option<Vec<String>>) -> Vec<String> {
    ws.unwrap_or_default()
        .into_iter()
        .filter(|w| !w.contains("_Extra"))
        .collect()
}

fn warnings(tree: &ExtensionTree) -> Vec<String> {
    own(unpack_ok(tree).warnings)
}

fn has(ws: &[String], needle: &str) -> bool {
    ws.iter().any(|w| w.contains(needle))
}

#[test]
fn canonical_fixture_warns_only_for_its_deliberate_extras() {
    let ws = unpack_ok(&rich_tree()).warnings.unwrap_or_default();
    assert_eq!(ws.len(), RICH_EXTRAS, "{ws:#?}");
    assert!(ws.iter().all(|w| w.contains("unknown attribute, kept but not used")), "{ws:#?}");
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
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/Laser On/@Evaluation_Mode");
}

#[test]
fn missing_mode_used_field_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &LASER_ON, "Case_Sensitivity");
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/Laser On/@Case_Sensitivity");

    let mut t = rich_tree();
    remove(&mut t, &OXYGEN, "Comparison_Operator");
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/Oxygen High/@Comparison_Operator");
}

#[test]
fn missing_mode_unused_and_cosmetic_fields_default_with_a_warning() {
    let mut t = rich_tree();
    remove(&mut t, &LASER_ON, "Comparison_Operator");
    remove(&mut t, &LASER_ON, "Epsilon");
    remove(&mut t, &["Trigger", "Job Start"], "Cooldown_Period");
    remove(&mut t, &["Trigger", "Job Start"], "Description");
    let ws = warnings(&t);
    assert_eq!(ws.len(), 4, "{ws:#?}");
    assert!(has(&ws, "Trigger/Job Start/@Cooldown_Period: missing, read as 0"), "{ws:#?}");
    assert!(has(&ws, "Laser On/@Comparison_Operator: missing, read as \"Equal\""), "{ws:#?}");
}

#[test]
fn missing_combinator_with_several_conditions_is_an_error() {
    let mut t = rich_tree();
    remove(&mut t, &["Trigger", "Job Start"], "Combinator");
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/@Combinator");
}

// ---- blank ----

#[test]
fn blank_mode_unused_field_is_silent() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Comparison_Operator", utf8(""));
    set(&mut t, &OXYGEN, "Case_Sensitivity", utf8(""));
    assert_eq!(warnings(&t), Vec::<String>::new());
}

#[test]
fn blank_mode_used_field_reads_as_default_with_a_warning() {
    let mut t = rich_tree();
    set(&mut t, &LASER_ON, "Case_Sensitivity", utf8(""));
    let c = unpack_ok(&t);
    let cond = c.triggers.rules.iter().flat_map(|r| &r.conditions).find(|c| c.label == "Laser On").unwrap();
    assert!(matches!(cond.evaluation, Evaluation::String { case_sensitivity: CaseSensitivity::Exact, .. }));
    assert!(has(&own(c.warnings), "Laser On/@Case_Sensitivity: blank, read as \"Exact\""));
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
    assert_eq!(c.pipe.min_integrity_level, MinIntegrityLevel::ProtectedProcess);
    assert_eq!(own(c.warnings).len(), 3);
}

#[test]
fn blank_action_stays_an_error() {
    let mut t = rich_tree();
    set(&mut t, &["Trigger", "Job Start"], "Action", utf8(""));
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/@Action");
}

#[test]
fn blank_combinator_with_several_conditions_warns() {
    let mut t = rich_tree();
    set(&mut t, &["Trigger", "Job Start"], "Combinator", utf8(""));
    assert!(has(&warnings(&t), "Trigger/Job Start/@Combinator: blank with 2 conditions"));
}

// ---- spellings and full range ----

#[test]
fn consumer_spellings_normalise_with_a_warning_and_casing_stays_silent() {
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
    let ws = own(c.warnings);
    assert_eq!(ws.len(), 4, "casing (EOS -> Eos) is silent: {ws:#?}");
    assert!(has(&ws, "Client/@Security_Policy: \"NistP384\" read as \"ECC_nistP384\""));
}

#[test]
fn user_access_level_casing_stays_strict() {
    let mut t = rich_tree();
    set(&mut t, &["Pipe"], "User_Access_Level", utf8("currentuseronly"));
    expect_err(unpack(&t), ErrorCode::Validation, "Pipe/@User_Access_Level");
}

#[test]
fn full_range_values_round_trip() {
    for policy in ["None", "Aes128_Sha256_RsaOaep", "Aes256_Sha256_RsaPss"] {
        let mut t = rich_tree();
        set(&mut t, &["Client"], "Security_Policy", utf8(policy));
        set(&mut t, &OXYGEN, "Comparison_Operator", utf8("NotEqual"));
        let c = unpack_ok(&t);
        assert_eq!(c.client.security_policy.as_str(), policy);
        assert_eq!(unpack_ok(&pack_ok(&c)).client.security_policy, c.client.security_policy);
        let cond = c.triggers.rules.iter().flat_map(|r| &r.conditions).find(|c| c.label == "Oxygen High").unwrap();
        assert!(matches!(cond.evaluation, Evaluation::Numeric { operator: ComparisonOperator::NotEqual, .. }));
    }
}

#[test]
fn negative_epsilon_is_an_error() {
    let mut t = rich_tree();
    set(&mut t, &OXYGEN, "Epsilon", f64_attr(-0.5));
    expect_err(unpack(&t), ErrorCode::Validation, "Trigger/Job Start/Oxygen High/@Epsilon");
}

// ---- unknown names and versions ----

#[test]
fn near_miss_attribute_name_suggests_the_field() {
    let mut t = rich_tree();
    set(&mut t, &["Trigger", "Job Start"], "Cooldown_Periood", utf8("5"));
    assert!(has(&warnings(&t), "@Cooldown_Periood: unknown attribute (did you mean Cooldown_Period?)"));
}

#[test]
fn unknown_attribute_in_a_newer_minor_file_is_silent() {
    let mut t = rich_tree();
    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.7"));
    set(&mut t, &["Client"], "Brand_New_Setting", utf8("x"));
    // A newer-minor file warns about nothing unknown, including the fixture's extras.
    assert_eq!(unpack_ok(&t).warnings, None);

    t.attrs.insert("Opcua_Schema_Version".into(), utf8("1.2"));
    assert!(has(&warnings(&t), "Client/@Brand_New_Setting: unknown attribute, kept but not used"));
}

// ---- on-disk encodings for consumers ----

#[test]
fn to_spec_matches_the_on_disk_encoding() {
    assert_eq!(RootNode::AutoDiscover.to_spec(), "");
    assert_eq!(RootNode::NodeId { value: "ns=2;s=X".into() }.to_spec(), "ns=2;s=X");
    assert_eq!(SamplingInterval::ServerNegotiated.to_spec(), -1);
    assert_eq!(SamplingInterval::Fastest.to_spec(), 0);
    assert_eq!(SamplingInterval::Ms { ms: 250 }.to_spec(), 250);
    assert_eq!(SecurityPolicy::Auto.to_spec(), "");
    assert_eq!(SecurityPolicy::None.to_spec(), "None");
}
