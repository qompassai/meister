//! Conformance tests: drive the real binary over stdio.
//! Half validation, half adversarial (the estate test split).
//!
//! The wire is the modern era only: sessions open with
//! `server/discover`, every request carries `_meta` naming
//! protocol 2026-07-28, and legacy `initialize` contact is refused
//! with the typed `-32022`. The lane gate is exercised end to end:
//! an sfw/qompass envelope reaches the seam; nsfw and unclassified
//! envelopes are refused before it, and a refused run writes
//! nothing anywhere.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const META: &str = r#""_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientInfo": {"name": "meister-test", "version": "0"}, "io.modelcontextprotocol/clientCapabilities": {}}"#;

fn rpc_in(cwd: &std::path::Path, requests: &[&str]) -> Vec<serde_json::Value> {
    let bin = env!("CARGO_BIN_EXE_meister");
    let mut child = Command::new(bin)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    {
        let stdin = child.stdin.as_mut().unwrap();
        for req in requests {
            writeln!(stdin, "{req}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "server exit: {:?}", out.status);
    // rmcp dispatches pipelined requests concurrently; responses
    // may arrive out of request order. Correlate by id.
    let mut resps: Vec<serde_json::Value> = String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    resps.sort_by_key(|r| r["id"].as_i64().unwrap_or(i64::MAX));
    resps
}

fn rpc(requests: &[&str]) -> Vec<serde_json::Value> {
    rpc_in(&std::env::temp_dir(), requests)
}

fn discover() -> String {
    format!(r#"{{"jsonrpc":"2.0","id":0,"method":"server/discover","params":{{{META}}}}}"#)
}

fn request(id: i64, method: &str, params: serde_json::Value) -> String {
    let mut params = params;
    params["_meta"] = serde_json::json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name": "meister-test", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {}});
    serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn call(id: i64, tool: &str, args: serde_json::Value) -> String {
    request(
        id,
        "tools/call",
        serde_json::json!({"name": tool, "arguments": args}),
    )
}

fn result_text(resp: &serde_json::Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

fn result_json(resp: &serde_json::Value) -> serde_json::Value {
    serde_json::from_str(&result_text(resp)).unwrap()
}

fn envelope(label: &str, origin: &str) -> serde_json::Value {
    serde_json::json!({"record_id": "rec-wire-1", "origin_lane": origin,
                       "content_label": label, "evidence": "conformance test"})
}

// ---------- validation ----------

#[test]
fn discover_and_list_serve_the_full_annotated_surface() {
    let list = request(2, "tools/list", serde_json::json!({}));
    let resps = rpc(&[&discover(), &list]);
    assert_eq!(
        resps[0]["result"]["supportedVersions"],
        serde_json::json!(["2026-07-28"])
    );
    assert_eq!(
        resps[0]["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
        "meister"
    );
    let tools = resps[1]["result"]["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 31, "the full Rung 0 surface");
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        assert_eq!(tool["inputSchema"]["type"], "object", "{name} input type");
        assert!(
            tool["inputSchema"]["properties"].is_object(),
            "{name} input schema lacks explicit properties"
        );
        assert!(
            tool["outputSchema"]["properties"].is_object(),
            "{name} output schema lacks explicit properties"
        );
        let annotations = &tool["annotations"];
        assert!(
            annotations["readOnlyHint"].is_boolean(),
            "{name} lacks readOnlyHint"
        );
        assert!(
            annotations["destructiveHint"].is_boolean(),
            "{name} lacks destructiveHint"
        );
        assert!(
            annotations["idempotentHint"].is_boolean(),
            "{name} lacks idempotentHint"
        );
        assert!(
            annotations["openWorldHint"].is_boolean(),
            "{name} lacks openWorldHint"
        );
    }
}

#[test]
fn lane_check_allows_sfw_qompass_over_the_wire() {
    let req = call(
        1,
        "meister_lane_check",
        serde_json::json!({"record_id": "r1", "origin_lane": "qompass",
                           "content_label": "sfw", "evidence": "wire test"}),
    );
    let resps = rpc(&[&discover(), &req]);
    let body = result_json(&resps[1]);
    assert_eq!(body["verdict"], "allow");
    assert_eq!(body["refusal_code"], serde_json::Value::Null);
}

#[test]
fn sfw_envelope_reaches_the_seam() {
    let req = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "name": "First Light", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    let resps = rpc(&[&discover(), &req]);
    assert_eq!(resps[1]["result"]["isError"], true);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "not_implemented");
    assert_eq!(body["error"]["rung"], 1);
}

// ---------- adversarial ----------

#[test]
fn legacy_initialize_is_refused_and_the_server_survives() {
    let init = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#;
    let check = call(
        3,
        "meister_lane_check",
        serde_json::json!({"origin_lane": "qompass", "content_label": "sfw",
                           "evidence": "after refusal"}),
    );
    let resps = rpc(&[init, &discover(), &check]);
    // Sorted by id: [discover(0), init(1), check(3)].
    assert_eq!(resps[1]["error"]["code"], -32022);
    assert_eq!(resps[1]["error"]["data"]["requested"], "2024-11-05");
    assert_eq!(
        resps[1]["error"]["data"]["supported"],
        serde_json::json!(["2026-07-28"])
    );
    assert_eq!(result_json(&resps[2])["verdict"], "allow");
}

#[test]
fn tools_list_without_modern_meta_is_an_error() {
    let no_meta = r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#;
    let resps = rpc(&[&discover(), no_meta]);
    assert!(
        resps[1]["error"].is_object(),
        "no-meta tools/list must error, got: {resps:?}"
    );
}

#[test]
fn nsfw_envelope_is_refused_lane_mismatch() {
    let req = call(
        1,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("nsfw", "qompass"),
                           "project_id": "p1", "character_id": "c1",
                           "identity": {}}),
    );
    let resps = rpc(&[&discover(), &req]);
    assert_eq!(resps[1]["result"]["isError"], true);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "lane_mismatch");
    assert_eq!(body["error"]["ids"], serde_json::json!(["rec-wire-1"]));
}

#[test]
fn missing_label_is_refused_lane_unclassified() {
    let bare = serde_json::json!({"record_id": "rec-wire-2",
                                  "origin_lane": "qompass",
                                  "evidence": "no label at all"});
    let req = call(
        1,
        "meister_cast_add",
        serde_json::json!({"envelope": bare, "project_id": "p1",
                           "character_id": "c1", "identity": {}}),
    );
    let resps = rpc(&[&discover(), &req]);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "lane_unclassified");
}

#[test]
fn other_lane_origin_is_refused_even_labelled_sfw() {
    let req = call(
        1,
        "meister_scene_define",
        serde_json::json!({"envelope": envelope("sfw", "ntranch"),
                           "project_id": "p1", "scene_id": "s1",
                           "graph": {}}),
    );
    let resps = rpc(&[&discover(), &req]);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "lane_mismatch");
}

#[test]
fn refused_and_seam_calls_write_nothing() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("meister-rung0-cwd-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let bad = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("nsfw", "ntranch"),
                           "name": "x", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    let good = call(
        2,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "name": "x", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    let _ = rpc_in(&dir, &[&discover(), &bad, &good]);
    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert!(entries.is_empty(), "server wrote into its cwd: {entries:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_json_is_a_parse_error_and_the_server_survives() {
    let garbage = "this is not json";
    let check = call(
        5,
        "meister_lane_check",
        serde_json::json!({"origin_lane": "qompass", "content_label": "sfw",
                           "evidence": "after garbage"}),
    );
    let resps = rpc(&[garbage, &discover(), &check]);
    // The parse error carries id null and sorts last in rpc().
    let parse = resps
        .iter()
        .find(|r| r["error"]["code"] == -32700)
        .expect("a -32700 parse error frame");
    assert_eq!(parse["id"], serde_json::Value::Null);
    let check_resp = resps
        .iter()
        .find(|r| r["id"] == 5)
        .expect("the post-garbage call answered");
    assert_eq!(result_json(check_resp)["verdict"], "allow");
}
