//! Conformance tests: drive the real binary over stdio.
//! Half validation, half adversarial (the estate test split).
//!
//! The wire is the modern era only: sessions open with
//! `server/discover`, every request carries `_meta` naming
//! protocol 2026-07-28, and legacy `initialize` contact is refused
//! with the typed `-32022`. The lane gate is exercised end to end:
//! an sfw/qompass envelope reaches the handlers; nsfw and
//! unclassified envelopes are refused before them, and a refused
//! run writes nothing anywhere. Since Rung 1 the Group 1–2 tools
//! are real: every session runs against a throwaway store root
//! (`MEISTER_PROJECTS_ROOT`), and the full
//! create → cast → lock → census flow is driven over the wire.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

const META: &str = r#""_meta": {"io.modelcontextprotocol/protocolVersion": "2026-07-28", "io.modelcontextprotocol/clientInfo": {"name": "meister-test", "version": "0"}, "io.modelcontextprotocol/clientCapabilities": {}}"#;

/// SHA-256 of the fixture bytes `abc` (FIPS 180-4 example vector).
const ABC_SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

static STORE_SEQ: AtomicU64 = AtomicU64::new(0);

fn fresh_store() -> PathBuf {
    let seq = STORE_SEQ.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("meister-test-store-{}-{seq}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn rpc_in(cwd: &std::path::Path, requests: &[&str]) -> Vec<serde_json::Value> {
    rpc_store(cwd, &fresh_store(), requests)
}

fn rpc_store(
    cwd: &std::path::Path,
    store: &std::path::Path,
    requests: &[&str],
) -> Vec<serde_json::Value> {
    let bin = env!("CARGO_BIN_EXE_meister");
    let mut child = Command::new(bin)
        .current_dir(cwd)
        .env("MEISTER_PROJECTS_ROOT", store)
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
    // A Group 3 seam (Rung 2): the allowed envelope still falls
    // through the gate to the seam error, unchanged by Rung 1.
    let req = call(
        1,
        "meister_scene_define",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "p1", "scene_id": "s1",
                           "graph": {}}),
    );
    let resps = rpc(&[&discover(), &req]);
    assert_eq!(resps[1]["result"]["isError"], true);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "not_implemented");
    assert_eq!(body["error"]["rung"], 2);
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
    let store = fresh_store();
    let bad = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("nsfw", "ntranch"),
                           "name": "x", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    let good = call(
        2,
        "meister_scene_define",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "p1", "scene_id": "s1",
                           "graph": {}}),
    );
    let _ = rpc_store(&dir, &store, &[&discover(), &bad, &good]);
    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert!(entries.is_empty(), "server wrote into its cwd: {entries:?}");
    assert!(
        !store.exists(),
        "a refused call and a seam call created the store root"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

// ---------- Rung 1: the live Group 1–2 flow, over the wire ----------

/// Write the fixture source file (bytes `abc`) and return its path.
fn fixture_file(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("meister-wire-fixture-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("source.txt");
    std::fs::write(&file, b"abc").unwrap();
    file
}

fn wire_identity(source: &std::path::Path) -> serde_json::Value {
    let p = source.display().to_string();
    serde_json::json!({
        "display_name": "Warden",
        "role": "keeper of the first floor",
        "model_scheme": "single_model",
        "governing_references": [
            {"id": "sheet", "kind": "sprite_sheet", "path": p, "sha256": ABC_SHA,
             "status": "canonical", "source": "wire fixture"}
        ],
        "appearance_references": [
            {"id": "sheet-view", "path": p, "sha256": ABC_SHA, "source": "wire fixture"}
        ],
        "rig_references": [
            {"id": "rig", "path": p, "sha256": ABC_SHA, "source": "wire fixture"}
        ],
        "forms": [
            {"form_id": "sheet", "kind": "stage_sheet", "design_id": "warden-ns1",
             "canon_status": "canonical", "governing_reference_id": "sheet"}
        ],
        "media_chain": [
            {"path": p, "snapshot_id": "snap-1", "sha256": ABC_SHA,
             "status": "candidate", "verdict": "pending"}
        ],
        "quarantined_variants": []
    })
}

fn wire_lock(source: &std::path::Path) -> serde_json::Value {
    let p = source.display().to_string();
    let sourced = |description: &str| {
        serde_json::json!({"description": description, "source_artifact": p,
                           "source_sha256": ABC_SHA})
    };
    serde_json::json!({
        "form_id": "sheet",
        "governing_reference_id": "sheet",
        "silhouette": sourced("hooded triangle"),
        "palette": [
            {"name": "accent", "hex": "#B82921", "source_artifact": p,
             "source_sha256": ABC_SHA}
        ],
        "face_proportions": sourced("head 30x36"),
        "body_proportions": sourced("height 234 units"),
        "thumbnail_recognizability": {
            "width_px": 48, "height_px": 64, "recognisable": true,
            "evidence": "wire fixture", "source_artifact": p,
            "source_sha256": ABC_SHA
        },
        "ruling": "D-1410 wire test"
    })
}

fn wire_create(store: &std::path::Path, source: &std::path::Path) -> Vec<serde_json::Value> {
    let create = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
        "name": "Wire Game", "engine": "bevy",
        "genre_pack": "role_playing",
        "canon": [{
            "id": "floors",
            "statement": "the descent has eighteen floors",
            "value": 18,
            "source_artifact": source.display().to_string(),
            "source_sha256": ABC_SHA,
        }]}),
    );
    rpc_store(&std::env::temp_dir(), store, &[&discover(), &create])
}

#[test]
fn rung1_full_flow_over_the_wire() {
    let store = fresh_store();
    let source = fixture_file("flow");
    let resps = wire_create(&store, &source);
    assert_eq!(resps[1]["result"]["isError"], false);
    let created = result_json(&resps[1]);
    assert_eq!(created["project_id"], "wire-game");
    assert_eq!(created["canon_fact_count"], 1);
    assert!(store.join("wire-game/project.json").is_file());

    // Dependent calls go in separate sessions: rmcp dispatches
    // pipelined requests concurrently, so a lock must not share
    // a pipeline with the add it depends on.
    let add = call(
        2,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "identity": wire_identity(&source)}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &add]);
    let added = result_json(&resps[1]);
    assert_eq!(added["status"], "candidate", "cast_add body: {added}");
    let lock = call(
        3,
        "meister_cast_lock",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "lock": wire_lock(&source)}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &lock]);
    let locked = result_json(&resps[1]);
    assert_eq!(locked["locked"], true, "cast_lock body: {locked}");
    assert_eq!(locked["lock_hash"].as_str().unwrap().len(), 64);
    let check = call(
        4,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game"}),
    );
    let inspect = call(
        5,
        "meister_project_inspect",
        serde_json::json!({"project_id": "wire-game"}),
    );
    let resps = rpc_store(
        &std::env::temp_dir(),
        &store,
        &[&discover(), &check, &inspect],
    );
    // Sorted by id: [discover(0), check(4), inspect(5)].
    let census = result_json(&resps[1]);
    assert_eq!(census["unverified"], 0, "census body: {census}");
    assert_eq!(census["verdict"], "pass");
    assert_eq!(census["counts"]["cast_records"], 1);
    assert_eq!(census["counts"]["canon_facts"], 1);
    assert_eq!(result_json(&resps[2])["cast_count"], 1);
    let record: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(store.join("wire-game/cast/warden.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(record["status"], "identity_locked");
    assert_eq!(record["identity_lock"]["lock_hash"], locked["lock_hash"]);
    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn rung1_tampered_source_fails_the_census_over_the_wire() {
    let store = fresh_store();
    let source = fixture_file("tamper");
    let _ = wire_create(&store, &source);
    let add = call(
        2,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "identity": wire_identity(&source)}),
    );
    let _ = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &add]);
    let lock = call(
        3,
        "meister_cast_lock",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "lock": wire_lock(&source)}),
    );
    let _ = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &lock]);
    std::fs::write(&source, b"tampered").unwrap();
    let check = call(
        4,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game"}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &check]);
    assert_eq!(resps[1]["result"]["isError"], true);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "canon_unverified");
    assert!(body["error"]["census"]["unverified"].as_u64().unwrap() > 0);
    assert_eq!(body["error"]["census"]["verdict"], "review");
    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn rung1_refusals_over_the_wire() {
    let store = fresh_store();
    let source = fixture_file("refusals");
    let _ = wire_create(&store, &source);
    // Duplicate create: conflict, not overwrite.
    let dup = call(
        2,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "name": "Wire Game", "engine": "bevy",
                           "genre_pack": "role_playing"}),
    );
    // Traversal id: refused before it can become a path.
    let traversal = call(
        3,
        "meister_project_inspect",
        serde_json::json!({"project_id": "../escape"}),
    );
    // Cast add to a project that does not exist.
    let ghost = call(
        4,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "ghost", "character_id": "warden",
                           "identity": wire_identity(&source)}),
    );
    let resps = rpc_store(
        &std::env::temp_dir(),
        &store,
        &[&discover(), &dup, &traversal, &ghost],
    );
    assert_eq!(result_json(&resps[1])["error"]["code"], "conflict");
    assert_eq!(result_json(&resps[2])["error"]["code"], "invalid_input");
    assert_eq!(result_json(&resps[3])["error"]["code"], "not_found");
    assert!(!store.join("escape").exists());
    let _ = std::fs::remove_dir_all(&store);
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
