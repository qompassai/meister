//! Manifest agreement: the agent card and server.json are a
//! provisioning pair (D-1605). Client configs for the estate's
//! agent runtimes are to be generated from card + server.json
//! pairs, so the join keys must agree exactly, from birth:
//!
//! - card `name` == server.json `title`
//! - card `version` == server.json `version`
//! - the card's MCP-interface extension `registryName` ==
//!   server.json `name`
//!
//! The card must also stay honest: it declares no A2A interface
//! (Meister is an MCP server over stdio) and carries the
//! invocation facts in its extension params.

use std::path::PathBuf;

fn read_json(rel: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("parse {}: {e}", path.display()))
}

#[test]
fn card_and_server_json_agree_on_the_join_keys() {
    let card = read_json(".well-known/agent-card.json");
    let server = read_json("server.json");
    assert_eq!(card["name"], server["title"], "card name / server title");
    assert_eq!(
        card["version"], server["version"],
        "card version / server version"
    );
    let extension = &card["capabilities"]["extensions"][0];
    assert_eq!(
        extension["params"]["registryName"], server["name"],
        "extension registryName / server name"
    );
}

#[test]
fn card_declares_no_a2a_endpoint_and_carries_invocation() {
    let card = read_json(".well-known/agent-card.json");
    // No supported interface is asserted: Meister has no A2A
    // endpoint, and the card must not imply one.
    assert_eq!(
        card["supportedInterfaces"],
        serde_json::json!([]),
        "supportedInterfaces must stay empty until an A2A endpoint exists"
    );
    let invocation = &card["capabilities"]["extensions"][0]["params"]["invocation"];
    assert_eq!(invocation["transport"], "stdio");
    assert_eq!(invocation["command"], "~/.local/bin/meister");
    assert_eq!(invocation["args"], serde_json::json!([]));
    assert_eq!(invocation["protocolEra"], "2026-07-28");
    // The ten tool groups are the card's skills.
    assert_eq!(card["skills"].as_array().map(Vec::len), Some(10));
}
