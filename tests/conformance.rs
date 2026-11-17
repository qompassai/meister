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
//!
//! Since EFLO-03 the server serves one surface profile per launch
//! (`MEISTER_PROFILE`, default `core`). The pre-profile tests run
//! on `full`, the conformance profile; the profile tests at the end
//! launch every declared profile and pin its served set.

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
    rpc_profile(Some("full"), cwd, store, requests)
}

/// One session under `profile` (`None` leaves `MEISTER_PROFILE`
/// unset: the default surface).
fn rpc_profile(
    profile: Option<&str>,
    cwd: &std::path::Path,
    store: &std::path::Path,
    requests: &[&str],
) -> Vec<serde_json::Value> {
    let bin = env!("CARGO_BIN_EXE_meister");
    let mut command = Command::new(bin);
    command
        .current_dir(cwd)
        .env("MEISTER_PROJECTS_ROOT", store)
        .env_remove("MEISTER_PROFILE")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    if let Some(profile) = profile {
        command.env("MEISTER_PROFILE", profile);
    }
    let mut child = command.spawn().unwrap();
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
    // The cast record carries its own envelope record_id:
    // STORE-03 holds record_ids unique per project, so it may not
    // reuse the project record envelope id.
    let mut add_args = serde_json::json!({"envelope": envelope("sfw", "qompass"),
                       "project_id": "wire-game", "character_id": "warden",
                       "identity": wire_identity(&source)});
    add_args["envelope"]["record_id"] = serde_json::json!("rec-wire-cast-1");
    let add = call(2, "meister_cast_add", add_args);
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
    // The cast record carries its own envelope record_id:
    // STORE-03 holds record_ids unique per project, so it may not
    // reuse the project record envelope id.
    let mut add_args = serde_json::json!({"envelope": envelope("sfw", "qompass"),
                       "project_id": "wire-game", "character_id": "warden",
                       "identity": wire_identity(&source)});
    add_args["envelope"]["record_id"] = serde_json::json!("rec-wire-cast-1");
    let add = call(2, "meister_cast_add", add_args);
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
fn sys_a_kinds_work_over_the_wire() {
    let store = fresh_store();
    let source = fixture_file("sysa");
    let _ = wire_create(&store, &source);
    let define = |id: i64, system_id: &str, kind: &str, spec: serde_json::Value| {
        let mut args = serde_json::json!({"envelope": envelope("sfw", "qompass"),
            "project_id": "wire-game", "system_id": system_id,
            "kind": kind, "spec": spec});
        args["envelope"]["record_id"] = serde_json::json!(format!("rec-wire-{system_id}"));
        call(id, "meister_system_define", args)
    };
    // Flags first: guards name the declared-flag namespace.
    let flags = define(
        2,
        "core-flags",
        "flags",
        serde_json::json!({"flags": [
            {"id": "met-warden", "type": "bool", "initial": false},
            {"id": "depth", "type": "int", "initial": 1, "min": 0, "max": 12}
        ]}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &flags]);
    let body = result_json(&resps[1]);
    assert_eq!(body["status"], "candidate", "flags body: {body}");
    assert!(store.join("wire-game/systems/core-flags.json").is_file());
    // A state machine whose guard references the declared flag.
    let machine = define(
        3,
        "chamber",
        "state_machine",
        serde_json::json!({"states": ["sealed", "open", "cleared"],
        "initial": "sealed", "terminal": ["cleared"],
        "transitions": [
            {"from": "sealed", "to": "open", "guard": "met-warden"},
            {"from": "open", "to": "cleared"}
        ]}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &machine]);
    assert_eq!(result_json(&resps[1])["kind"], "state_machine");
    // A float economy amount is economy_float over the wire.
    let economy = define(
        4,
        "market",
        "economy",
        serde_json::json!({"currencies": [{"id": "gold", "min": 0, "max": 99}],
            "sources": [{"currency": "gold", "amount": 2.5}]}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &economy]);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "economy_float", "body: {body}");
    assert!(!store.join("wire-game/systems/market.json").exists());
    // A progression cycle is graph_cycle over the wire.
    let dag = define(
        5,
        "descent",
        "progression_dag",
        serde_json::json!({"nodes": [
            {"id": "a", "requires": ["b"]},
            {"id": "b", "requires": ["a"]}
        ]}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &dag]);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "graph_cycle", "body: {body}");
    let _ = std::fs::remove_dir_all(&store);
}

/// Build the locked wire project and return its passing census.
fn wire_locked_project(store: &std::path::Path, source: &std::path::Path) -> serde_json::Value {
    let _ = wire_create(store, source);
    // The cast record carries its own envelope record_id
    // (STORE-03: record_ids are unique per project).
    let mut add_args = serde_json::json!({"envelope": envelope("sfw", "qompass"),
                       "project_id": "wire-game", "character_id": "warden",
                       "identity": wire_identity(source)});
    add_args["envelope"]["record_id"] = serde_json::json!("rec-wire-cast-1");
    let add = call(2, "meister_cast_add", add_args);
    let _ = rpc_store(&std::env::temp_dir(), store, &[&discover(), &add]);
    let lock = call(
        3,
        "meister_cast_lock",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "lock": wire_lock(source)}),
    );
    let _ = rpc_store(&std::env::temp_dir(), store, &[&discover(), &lock]);
    let check = call(
        4,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game"}),
    );
    let resps = rpc_store(&std::env::temp_dir(), store, &[&discover(), &check]);
    assert_eq!(resps[1]["result"]["isError"], false);
    result_json(&resps[1])
}

#[test]
fn store02_census_names_its_state_and_repeats_byte_identically() {
    let store = fresh_store();
    let source = fixture_file("state");
    let census = wire_locked_project(&store, &source);
    let state_hash = census["state_hash"].as_str().unwrap();
    assert_eq!(state_hash.len(), 64);
    assert_eq!(census["stale"], false);
    // Two more runs in two more processes: the same bytes, and a
    // census pinned to the current state is current.
    let plain = call(
        5,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game"}),
    );
    let pinned = call(
        6,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game", "prior_state_hash": state_hash}),
    );
    let one = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &plain]);
    let two = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &pinned]);
    assert_eq!(result_text(&one[1]), census.to_string());
    assert_eq!(result_text(&two[1]), census.to_string());
    // The served schema carries the new fields: the contract, not
    // just the behaviour.
    let list = request(7, "tools/list", serde_json::json!({}));
    let resps = rpc(&[&discover(), &list]);
    let tool = resps[1]["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["name"] == "meister_canon_check")
        .unwrap()
        .clone();
    let required = tool["outputSchema"]["required"].as_array().unwrap();
    for field in ["state_hash", "stale"] {
        assert!(required.iter().any(|r| r == field), "{field} not required");
        assert!(census.get(field).is_some(), "{field} missing from the body");
    }
    assert_eq!(
        tool["outputSchema"]["properties"]["stale"]["type"],
        "boolean"
    );
    assert_eq!(
        tool["inputSchema"]["properties"]["prior_state_hash"]["pattern"],
        "^[0-9a-f]{64}$"
    );
    assert_eq!(
        tool["inputSchema"]["required"],
        serde_json::json!(["project_id"])
    );
    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn adv_store02_a_write_after_the_census_answers_census_stale_and_writes_nothing() {
    let store = fresh_store();
    let source = fixture_file("stale");
    let census = wire_locked_project(&store, &source);
    let held = census["state_hash"].as_str().unwrap();
    // A write after the census: a second character.
    let add = call(
        5,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "wire-game", "character_id": "herald",
                           "identity": wire_identity(&source)}),
    );
    let resps = rpc_store(&std::env::temp_dir(), &store, &[&discover(), &add]);
    assert_eq!(result_json(&resps[1])["status"], "candidate");
    let before = tree_digest(&store);
    let stale = call(
        6,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game", "prior_state_hash": held}),
    );
    let malformed = call(
        7,
        "meister_canon_check",
        serde_json::json!({"project_id": "wire-game", "prior_state_hash": "not-a-hash"}),
    );
    let resps = rpc_store(
        &std::env::temp_dir(),
        &store,
        &[&discover(), &stale, &malformed],
    );
    assert_eq!(resps[1]["result"]["isError"], true);
    let body = result_json(&resps[1]);
    assert_eq!(body["error"]["code"], "census_stale");
    assert_eq!(body["error"]["census"]["stale"], true);
    assert_eq!(body["error"]["census"]["verdict"], "review");
    assert_ne!(body["error"]["census"]["state_hash"], held);
    // A prior that is not a hash is refused, never read as "no
    // prior".
    assert_eq!(result_json(&resps[2])["error"]["code"], "invalid_input");
    assert_eq!(tree_digest(&store), before, "a refused census wrote");
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

// ---------- EFLO-03: surface profiles, over the wire ----------

const CORE: [&str; 4] = [
    "meister_canon_check",
    "meister_lane_check",
    "meister_project_inspect",
    "meister_project_list",
];
const CANON_OWN: [&str; 3] = [
    "meister_cast_add",
    "meister_cast_lock",
    "meister_project_create",
];
const SYSTEMS_OWN: [&str; 4] = [
    "meister_level_compose",
    "meister_scene_define",
    "meister_scene_validate",
    "meister_system_define",
];
const NARRATIVE_OWN: [&str; 5] = [
    "meister_dialogue_author",
    "meister_dialogue_get",
    "meister_playtest_generate",
    "meister_reaction_set",
    "meister_story_define",
];
const ASSETS_OWN: [&str; 3] = [
    "meister_asset_chain_get",
    "meister_asset_intake",
    "meister_asset_route",
];
const QA_OWN: [&str; 6] = [
    "meister_accessibility_check",
    "meister_content_summary",
    "meister_defect_record",
    "meister_playtest_define",
    "meister_rating_record",
    "meister_replay_check",
];
const RELEASE_OWN: [&str; 6] = [
    "meister_account_inspect",
    "meister_account_register",
    "meister_account_verify",
    "meister_channel_list",
    "meister_preflight_run",
    "meister_release_prepare",
];

/// The declared profiles and their served sets, written out here
/// independently of the catalog (COMPLIANCE.md §3): each entry is
/// the profile name and the tool groups it serves on top of core.
fn declared_profiles() -> Vec<(&'static str, Vec<&'static str>)> {
    let with_core = |own: &[&[&'static str]]| {
        let mut set: Vec<&'static str> = CORE.to_vec();
        for part in own {
            set.extend_from_slice(part);
        }
        set.sort_unstable();
        set
    };
    vec![
        ("core", with_core(&[])),
        ("canon", with_core(&[&CANON_OWN])),
        ("systems", with_core(&[&SYSTEMS_OWN])),
        ("narrative", with_core(&[&NARRATIVE_OWN])),
        ("assets", with_core(&[&ASSETS_OWN])),
        ("qa", with_core(&[&QA_OWN])),
        ("release", with_core(&[&RELEASE_OWN])),
        ("niflheim-floor", with_core(&[&SYSTEMS_OWN, &NARRATIVE_OWN])),
        (
            "full",
            with_core(&[
                &CANON_OWN,
                &SYSTEMS_OWN,
                &NARRATIVE_OWN,
                &ASSETS_OWN,
                &QA_OWN,
                &RELEASE_OWN,
            ]),
        ),
    ]
}

/// The tool names one launch serves, sorted.
fn served_names(profile: Option<&str>) -> Vec<String> {
    let list = request(2, "tools/list", serde_json::json!({}));
    let resps = rpc_profile(
        profile,
        &std::env::temp_dir(),
        &fresh_store(),
        &[&discover(), &list],
    );
    let mut names: Vec<String> = resps[1]["result"]["tools"]
        .as_array()
        .expect("a tools array")
        .iter()
        .map(|tool| tool["name"].as_str().unwrap().to_string())
        .collect();
    names.sort_unstable();
    names
}

/// A digest of every path and every file's bytes under `root`
/// (a missing root digests as empty). Equal digests before and
/// after a call mean the call wrote nothing.
fn tree_digest(root: &std::path::Path) -> u64 {
    use std::hash::{Hash, Hasher};
    fn walk(dir: &std::path::Path, entries: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read {
            let path = entry.unwrap().path();
            if path.is_dir() {
                entries.push((path.clone(), None));
                walk(&path, entries);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                entries.push((path, Some(bytes)));
            }
        }
    }
    let mut entries = Vec::new();
    walk(root, &mut entries);
    entries.sort();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entries.hash(&mut hasher);
    hasher.finish()
}

/// Launch under a refused `MEISTER_PROFILE` value and return
/// (exit code, stdout, stderr). A discover and a tools/list are
/// offered on stdin: a server that started anyway would answer.
fn launch_refused(value: &str) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_meister"))
        .current_dir(std::env::temp_dir())
        .env("MEISTER_PROJECTS_ROOT", fresh_store())
        .env("MEISTER_PROFILE", value)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        // The server may already have exited: a broken pipe here
        // is the expected outcome, not a test failure.
        let stdin = child.stdin.as_mut().unwrap();
        let list = request(2, "tools/list", serde_json::json!({}));
        let _ = writeln!(stdin, "{}", discover());
        let _ = writeln!(stdin, "{list}");
    }
    let out = child.wait_with_output().unwrap();
    (
        out.status.code(),
        String::from_utf8(out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

// ---------- validation ----------

#[test]
fn every_declared_profile_serves_its_declared_set() {
    for (profile, expected) in declared_profiles() {
        assert_eq!(served_names(Some(profile)), expected, "profile {profile}");
    }
}

#[test]
fn the_default_profile_is_core() {
    let unset = served_names(None);
    assert_eq!(unset, CORE);
    assert_eq!(unset, served_names(Some("core")));
}

#[test]
fn in_profile_calls_run_under_their_profile() {
    // canon carries Group 1–2 writes: a create is real.
    let store = fresh_store();
    let create = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "name": "Profile Game", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    let resps = rpc_profile(
        Some("canon"),
        &std::env::temp_dir(),
        &store,
        &[&discover(), &create],
    );
    assert_eq!(resps[1]["result"]["isError"], false);
    assert!(store.join("profile-game/project.json").is_file());
    // core reads what canon wrote.
    let inspect = call(
        1,
        "meister_project_inspect",
        serde_json::json!({"project_id": "profile-game"}),
    );
    let resps = rpc_profile(
        None,
        &std::env::temp_dir(),
        &store,
        &[&discover(), &inspect],
    );
    assert_eq!(result_json(&resps[1])["project_id"], "profile-game");
    // The bundle carries Group 3: the call reaches its seam.
    let scene = call(
        1,
        "meister_scene_define",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "profile-game", "scene_id": "s1",
                           "graph": {}}),
    );
    let resps = rpc_profile(
        Some("niflheim-floor"),
        &std::env::temp_dir(),
        &store,
        &[&discover(), &scene],
    );
    assert_eq!(result_json(&resps[1])["error"]["code"], "not_implemented");
    let _ = std::fs::remove_dir_all(&store);
}

// ---------- adversarial ----------

#[test]
fn adv_unknown_profile_exits_nonzero_before_serving_anything() {
    for value in ["nope", "", "Core", "core,canon", " core", "full "] {
        let (code, stdout, stderr) = launch_refused(value);
        assert_eq!(code, Some(2), "{value:?} must refuse to start");
        assert!(stdout.is_empty(), "{value:?} served: {stdout}");
        assert!(
            stderr.contains("refusing to start: unknown_profile"),
            "{value:?} stderr: {stderr}"
        );
        // The typed message names the declared profiles.
        assert!(stderr.contains("niflheim-floor"), "stderr: {stderr}");
    }
}

#[test]
fn adv_out_of_profile_call_is_group_not_loaded_never_unknown_tool() {
    let read = call(
        1,
        "meister_dialogue_get",
        serde_json::json!({"project_id": "p1", "dialogue_id": "d1"}),
    );
    let write = call(
        2,
        "meister_cast_add",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "project_id": "p1", "character_id": "c1",
                           "identity": {}}),
    );
    let nope = call(3, "meister_nope", serde_json::json!({}));
    let resps = rpc_profile(
        None,
        &std::env::temp_dir(),
        &fresh_store(),
        &[&discover(), &read, &write, &nope],
    );
    for (resp, tool, carriers) in [
        (
            &resps[1],
            "meister_dialogue_get",
            serde_json::json!(["narrative", "niflheim-floor"]),
        ),
        (&resps[2], "meister_cast_add", serde_json::json!(["canon"])),
    ] {
        assert_eq!(resp["result"]["isError"], true);
        let body = result_json(resp);
        assert_eq!(body["error"]["code"], "group_not_loaded", "{tool}");
        assert_eq!(body["error"]["tool"], tool);
        assert_eq!(body["error"]["active_profile"], "core");
        assert_eq!(body["error"]["profiles"], carriers);
        assert_ne!(body["error"], "unknown tool");
    }
    // A name the catalog does not hold is still unknown.
    assert_eq!(result_json(&resps[3])["error"], "unknown tool");
}

#[test]
fn adv_lane_gate_refuses_before_the_profile_check() {
    // Out of profile AND off-lane, under every profile that does
    // not carry the tool: the lane code wins and the answer never
    // says where the tool lives.
    for profile in ["core", "systems", "release"] {
        let nsfw = call(
            1,
            "meister_cast_add",
            serde_json::json!({"envelope": envelope("nsfw", "qompass"),
                               "project_id": "p1", "character_id": "c1",
                               "identity": {}}),
        );
        let unlabelled = call(
            2,
            "meister_cast_add",
            serde_json::json!({"envelope": {"record_id": "rec-wire-2",
                                            "origin_lane": "qompass",
                                            "evidence": "no label"},
                               "project_id": "p1", "character_id": "c1",
                               "identity": {}}),
        );
        let resps = rpc_profile(
            Some(profile),
            &std::env::temp_dir(),
            &fresh_store(),
            &[&discover(), &nsfw, &unlabelled],
        );
        let body = result_json(&resps[1]);
        assert_eq!(body["error"]["code"], "lane_mismatch", "profile {profile}");
        assert!(body["error"].get("profiles").is_none());
        assert!(!result_text(&resps[1]).contains("canon"));
        assert_eq!(
            result_json(&resps[2])["error"]["code"],
            "lane_unclassified",
            "profile {profile}"
        );
    }
}

#[test]
fn adv_out_of_profile_refusals_are_pre_mutation() {
    // A populated store, then out-of-profile writes against it
    // under core: every one is refused and the tree is unchanged.
    let store = fresh_store();
    let source = fixture_file("profile-premutation");
    let _ = wire_create(&store, &source);
    let before = tree_digest(&store);
    assert_ne!(
        before,
        tree_digest(&fresh_store()),
        "the store is populated"
    );
    let create = call(
        1,
        "meister_project_create",
        serde_json::json!({"envelope": envelope("sfw", "qompass"),
                           "name": "Second Game", "engine": "bevy",
                           "genre_pack": "puzzle"}),
    );
    // The cast record carries its own envelope record_id:
    // STORE-03 holds record_ids unique per project, so it may not
    // reuse the project record envelope id.
    let mut add_args = serde_json::json!({"envelope": envelope("sfw", "qompass"),
                       "project_id": "wire-game", "character_id": "warden",
                       "identity": wire_identity(&source)});
    add_args["envelope"]["record_id"] = serde_json::json!("rec-wire-cast-1");
    let add = call(2, "meister_cast_add", add_args);
    let nsfw = call(
        3,
        "meister_cast_lock",
        serde_json::json!({"envelope": envelope("nsfw", "qompass"),
                           "project_id": "wire-game", "character_id": "warden",
                           "lock": wire_lock(&source)}),
    );
    let resps = rpc_profile(
        Some("core"),
        &std::env::temp_dir(),
        &store,
        &[&discover(), &create, &add, &nsfw],
    );
    assert_eq!(result_json(&resps[1])["error"]["code"], "group_not_loaded");
    assert_eq!(result_json(&resps[2])["error"]["code"], "group_not_loaded");
    assert_eq!(result_json(&resps[3])["error"]["code"], "lane_mismatch");
    assert_eq!(
        tree_digest(&store),
        before,
        "a refused call wrote to the store"
    );
    assert!(!store.join("second-game").exists());
    // Against an absent store the refusal does not create the root.
    let empty = fresh_store();
    let _ = rpc_profile(
        Some("core"),
        &std::env::temp_dir(),
        &empty,
        &[&discover(), &create],
    );
    assert!(!empty.exists(), "a refused call created the store root");
    let _ = std::fs::remove_dir_all(&store);
}
