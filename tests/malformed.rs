//! SYS-10 — the malformed-input corpus.
//!
//! Fixtures live in `tests/fixtures/malformed/*.json`, each
//! carrying one tool call whose arguments are malformed in one
//! named way, the typed refusal code it must earn, and its
//! provenance. The classes are curated from failure shapes
//! observed in real local-model output (stringified and fenced
//! JSON, type drift, hallucinated references) plus
//! synthetic-classic structural defects; the provenance field
//! says which, per fixture.
//!
//! The runner asserts three things for the whole corpus:
//! every fixture is refused with exactly its expected code
//! (never a panic, a hang, or a protocol error), the server is
//! alive afterwards, and the throwaway store is byte-identical
//! to its post-setup snapshot — a refused call wrote nothing.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

const PROTOCOL_VERSION: &str = "2026-07-28";

struct Server {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Server {
    fn spawn(root: &Path) -> Server {
        let bin = env!("CARGO_BIN_EXE_meister");
        let mut child = Command::new(bin)
            .env("MEISTER_PROJECTS_ROOT", root)
            .env("MEISTER_PROFILE", "full")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn meister");
        let stdin = child.stdin.take().expect("child stdin");
        let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
        Server {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn send(&mut self, msg: &Value) {
        let mut line = serde_json::to_string(msg).expect("serialize");
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).expect("write");
        self.stdin.flush().expect("flush");
    }

    fn recv(&mut self) -> Value {
        let mut line = String::new();
        let read = self.stdout.read_line(&mut line).expect("read line");
        assert!(read > 0, "server closed stdout (panic or exit?)");
        serde_json::from_str(&line).expect("server emitted non-JSON")
    }

    /// One request/response round trip.
    fn rpc(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({
            "jsonrpc": "2.0", "id": id, "method": method, "params": params
        }));
        let resp = self.recv();
        assert_eq!(resp["id"], json!(id), "response id mismatch");
        resp
    }

    fn meta() -> Value {
        json!({
            "io.modelcontextprotocol/protocolVersion": PROTOCOL_VERSION,
            "io.modelcontextprotocol/clientInfo": {"name": "malformed", "version": "0"},
            "io.modelcontextprotocol/clientCapabilities": {}
        })
    }

    fn discover(&mut self) {
        let resp = self.rpc("server/discover", json!({"_meta": Self::meta()}));
        assert!(resp.get("error").is_none(), "discover failed: {resp}");
    }

    fn call(&mut self, name: &str, arguments: Value) -> Value {
        let resp = self.rpc(
            "tools/call",
            json!({"name": name, "arguments": arguments, "_meta": Self::meta()}),
        );
        assert!(
            resp.get("error").is_none(),
            "tools/call for {name} was a protocol error: {resp}"
        );
        resp["result"].clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fresh_store(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("meister-malformed-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("create store dir");
    dir
}

fn envelope(record_id: &str) -> Value {
    json!({
        "record_id": record_id,
        "origin_lane": "qompass",
        "content_label": "sfw",
        "evidence": "SYS-10 malformed corpus"
    })
}

fn body_json(result: &Value) -> Value {
    let text = result["content"][0]["text"]
        .as_str()
        .expect("tool result carries text content");
    serde_json::from_str(text).expect("tool body is JSON")
}

/// A stable digest of every file under the store, keyed by
/// relative path — the byte-identity witness. The witness
/// only compares two snapshots within one run, so a simple
/// deterministic FNV-1a pair plus the length suffices.
fn store_snapshot(root: &Path) -> BTreeMap<String, String> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, String>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("read dir")
            .map(|e| e.expect("entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                let bytes = std::fs::read(&path).expect("read file");
                let rel = path
                    .strip_prefix(base)
                    .expect("under base")
                    .to_string_lossy()
                    .into_owned();
                out.insert(rel, digest_hex(&bytes));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

fn digest_hex(bytes: &[u8]) -> String {
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4;
    for (i, b) in bytes.iter().enumerate() {
        h1 ^= u64::from(*b) + (i as u64 % 251);
        h1 = h1.wrapping_mul(0x0000_0100_0000_01b3);
        h2 ^= u64::from(*b).rotate_left(17) + (i as u64 % 241);
        h2 = h2.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h1:016x}{h2:016x}:{}", bytes.len())
}

#[test]
fn malformed_corpus_refuses_every_fixture_with_its_code() {
    let fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/malformed");
    let mut fixture_files: Vec<PathBuf> = std::fs::read_dir(&fixtures_dir)
        .expect("fixtures dir")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    fixture_files.sort();
    assert!(
        fixture_files.len() >= 48,
        "SYS-10 needs at least 48 fixtures, found {}",
        fixture_files.len()
    );

    let root = fresh_store("corpus");
    let mut server = Server::spawn(&root);
    server.discover();

    // Setup: a project and the flags record several fixtures
    // legitimately reference (so their refusal is the targeted
    // defect, not an undeclared-flag refusal).
    let created = server.call(
        "meister_project_create",
        json!({
            "envelope": envelope("rec-mal-setup"),
            "name": "Wire Game",
            "engine": "bevy",
            "genre_pack": "puzzle",
            "canon": []
        }),
    );
    assert_eq!(created["isError"], json!(false), "setup create: {created}");
    let flags = server.call(
        "meister_system_define",
        json!({
            "envelope": envelope("rec-mal-setup-flags"),
            "project_id": "wire-game",
            "system_id": "core-flags",
            "kind": "flags",
            "spec": {"flags": [
                {"id": "met-warden", "type": "bool", "initial": false},
                {"id": "depth", "type": "int", "initial": 0, "min": 0, "max": 99}
            ]}
        }),
    );
    assert_eq!(flags["isError"], json!(false), "setup flags: {flags}");

    let before = store_snapshot(&root);

    let mut failures: Vec<String> = Vec::new();
    for path in &fixture_files {
        let raw = std::fs::read_to_string(path).expect("read fixture");
        let fixture: Value = serde_json::from_str(&raw).expect("fixture is JSON");
        let fid = fixture["id"].as_str().expect("fixture id");
        let tool = fixture["tool"].as_str().expect("fixture tool");
        let expected = fixture["expected_code"].as_str().expect("expected code");
        let result = server.call(tool, fixture["arguments"].clone());
        if result["isError"] != json!(true) {
            failures.push(format!("{fid}: expected refusal {expected}, call succeeded"));
            continue;
        }
        let body = body_json(&result);
        let code = body["error"]["code"].as_str().unwrap_or("<none>");
        if code != expected {
            failures.push(format!(
                "{fid}: expected {expected}, got {code} ({})",
                body["error"]["message"].as_str().unwrap_or("")
            ));
        }
    }
    assert!(failures.is_empty(), "corpus failures:\n{}", failures.join("\n"));

    // The server is alive and the store is untouched.
    let alive = server.call(
        "meister_lane_check",
        json!({"envelope": envelope("rec-mal-alive")}),
    );
    assert_eq!(alive["isError"], json!(false), "server not alive: {alive}");
    let after = store_snapshot(&root);
    assert_eq!(before, after, "a refused call changed the store");

    let _ = std::fs::remove_dir_all(&root);
}
