mod common;
use mcf_ext_opcua::ClientConfigMessage;
use std::fs;

#[test]
fn client_config_message_matches_the_shared_fixtures() {
    for case in ["minimal", "rich", "lenient_read"] {
        let model = common::fixture_model(&format!("v1_2/{case}"));
        let expected: ClientConfigMessage = serde_json::from_str(
            &fs::read_to_string(
                common::fixtures().join(format!("messages/client_config/{case}.json")),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(model.client.to_message(), expected, "{case}");
    }
}
