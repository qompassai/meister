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
//! The card must also stay honest AND spec-valid (D-1607): it
//! declares exactly one interface, bound with the qompass MCP
//! custom binding (a URI protocolBinding, per the A2A v1.0
//! custom-transport rule) whose url carries the stdio command
//! under that binding's own semantics; no core A2A binding is
//! claimed, and the invocation facts ride in the extension
//! params.

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
    // Spec-valid honesty (D-1607): supportedInterfaces is a
    // required, non-empty field in A2A v1.0, so the card
    // declares exactly one interface under the qompass MCP
    // custom binding (URI protocolBinding); its url carries
    // the stdio command under that binding's semantics. No
    // core A2A binding (JSONRPC / GRPC / HTTP+JSON) is claimed.
    let interfaces = card["supportedInterfaces"].as_array().expect("interfaces");
    assert_eq!(interfaces.len(), 1, "exactly one declared interface");
    assert_eq!(
        interfaces[0]["protocolBinding"],
        "https://qompass.ai/extensions/mcp-interface/v1"
    );
    assert_eq!(interfaces[0]["url"], "~/.local/bin/meister");
    assert_eq!(interfaces[0]["protocolVersion"], "1.0");
    // A field removed in A2A v1.0 must never reappear.
    assert!(
        card["capabilities"].get("stateTransitionHistory").is_none(),
        "stateTransitionHistory was removed in A2A v1.0"
    );
    let invocation = &card["capabilities"]["extensions"][0]["params"]["invocation"];
    assert_eq!(invocation["transport"], "stdio");
    assert_eq!(invocation["command"], "~/.local/bin/meister");
    assert_eq!(invocation["args"], serde_json::json!([]));
    assert_eq!(invocation["protocolEra"], "2026-07-28");
    // The ten tool groups are the card's skills.
    assert_eq!(card["skills"].as_array().map(Vec::len), Some(10));
}
