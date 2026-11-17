//! STORE-01: the cross-process store lock, driven through real
//! server processes over stdio. Half validation, half adversarial.
//!
//! Several Meister processes — one per surface profile — can share
//! one store root. Every mutation holds an advisory `flock` on
//! `<root>/.meister.lock`; a call that cannot take it within the
//! bound answers `store_busy` and writes nothing.
//!
//! The store roots here live under cargo's target tmpdir, not the
//! system temp directory: the lock has to hold on the filesystem
//! the build and the real store sit on, and the system temp
//! directory is usually a different one (tmpfs).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The store's lock file, relative to the store root.
const LOCK_FILE: &str = ".meister.lock";

/// The server's lock-wait bound (`LOCK_WAIT` in `src/store.rs`).
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// Creates per process in the interleaving tests (the plan's 50).
const CREATES_PER_PROCESS: usize = 50;

fn fresh_store(tag: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("store-lock-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn request(id: usize, method: &str, mut params: Value) -> String {
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": {"name": "meister-test", "version": "0"},
        "io.modelcontextprotocol/clientCapabilities": {}});
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string()
}

fn discover() -> String {
    request(0, "server/discover", json!({}))
}

fn create(id: usize, project_id: &str) -> String {
    let envelope = json!({"record_id": format!("rec-{project_id}"), "origin_lane": "qompass",
                          "content_label": "sfw", "evidence": "store lock test"});
    request(
        id,
        "tools/call",
        json!({"name": "meister_project_create",
               "arguments": {"envelope": envelope, "project_id": project_id,
                             "name": project_id, "engine": "bevy",
                             "genre_pack": "puzzle"}}),
    )
}

/// One launched server and how many responses it owes.
struct Session {
    child: Child,
    owed: usize,
}

/// Launch one server on `store` with its requests already written
/// to stdin. The process is running when this returns, and its
/// stdin stays open: the server ends its session at end of input,
/// and a call still waiting on the store lock at that moment
/// would never be answered.
fn launch(store: &Path, requests: &[String]) -> Session {
    let mut child = Command::new(env!("CARGO_BIN_EXE_meister"))
        .current_dir(env!("CARGO_TARGET_TMPDIR"))
        .env("MEISTER_PROJECTS_ROOT", store)
        .env("MEISTER_PROFILE", "full")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let stdin = child.stdin.as_mut().unwrap();
    for req in requests {
        writeln!(stdin, "{req}").unwrap();
    }
    Session {
        child,
        owed: requests.len(),
    }
}

/// Read every response a server owes, then end its input and wait
/// for it. Returns each tool call's body, by request id (index 0,
/// the discover, is dropped).
fn bodies(session: Session) -> Vec<Value> {
    let Session { mut child, owed } = session;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut resps: Vec<Value> = Vec::with_capacity(owed);
    while resps.len() < owed {
        let line = lines
            .next()
            .expect("the server closed its output with responses owed")
            .unwrap();
        if !line.trim().is_empty() {
            resps.push(serde_json::from_str(&line).unwrap());
        }
    }
    drop(child.stdin.take());
    let status = child.wait().unwrap();
    assert!(status.success(), "server exit: {status:?}");
    resps.sort_by_key(|r| r["id"].as_u64().unwrap_or(u64::MAX));
    resps
        .iter()
        .skip(1)
        .map(|r| {
            let text = r["result"]["content"][0]["text"].as_str().unwrap_or("");
            serde_json::from_str(text).unwrap()
        })
        .collect()
}

fn session(ids: &[String]) -> Vec<String> {
    let mut requests = vec![discover()];
    for (n, id) in ids.iter().enumerate() {
        requests.push(create(n + 1, id));
    }
    requests
}

fn ids(prefix: &str) -> Vec<String> {
    (0..CREATES_PER_PROCESS)
        .map(|n| format!("{prefix}-{n:02}"))
        .collect()
}

/// Every path under `root` with every file's bytes, sorted. Equal
/// snapshots before and after mean nothing was written.
fn tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
    fn walk(dir: &Path, out: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read {
            let path = entry.unwrap().path();
            if path.is_dir() {
                out.push((path.clone(), None));
                walk(&path, out);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                out.push((path, Some(bytes)));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out);
    out.sort();
    out
}

/// Assert the store holds exactly `expected` projects, every
/// record parsing and naming its own directory, with no temp file
/// left behind — no torn record, no lost write.
fn assert_store_whole(store: &Path, expected: &[String]) {
    let mut found = Vec::new();
    for (path, bytes) in tree(store) {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(!name.ends_with(".tmp"), "temp file left behind: {path:?}");
        let Some(bytes) = bytes else { continue };
        if name == LOCK_FILE {
            assert!(bytes.is_empty(), "the lock file carries bytes");
            continue;
        }
        assert_eq!(name, "project.json", "unexpected file {path:?}");
        let record: Value =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("torn record {path:?}: {e}"));
        let dir = path.parent().unwrap().file_name().unwrap();
        assert_eq!(record["project_id"].as_str().unwrap(), dir);
        assert_eq!(record["record_type"], "meister_project");
        found.push(record["project_id"].as_str().unwrap().to_string());
    }
    found.sort();
    let mut expected = expected.to_vec();
    expected.sort();
    assert_eq!(found, expected, "lost or stray writes");
}

// ---------- validation ----------

#[test]
fn two_processes_fifty_interleaved_creates_each_lose_nothing() {
    let store = fresh_store("interleave");
    let (left, right) = (ids("left"), ids("right"));
    let started = Instant::now();
    let a = launch(&store, &session(&left));
    let b = launch(&store, &session(&right));
    let (a, b) = (bodies(a), bodies(b));
    let elapsed = started.elapsed();
    for (body, id) in a.iter().zip(&left).chain(b.iter().zip(&right)) {
        assert_eq!(body["project_id"], id.as_str(), "create refused: {body}");
        assert_eq!(body["status"], "scaffolded");
    }
    let mut all = left;
    all.extend(right);
    assert_eq!(all.len(), 2 * CREATES_PER_PROCESS);
    assert_store_whole(&store, &all);
    // Evidence for the lock-wait bound: the whole contended run
    // sits far inside one bound.
    eprintln!(
        "store_lock: 2 x {CREATES_PER_PROCESS} interleaved creates on {} took {elapsed:?}",
        store.display()
    );
    assert!(elapsed < LOCK_WAIT, "contended run took {elapsed:?}");
    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn a_finished_process_leaves_the_lock_free() {
    let store = fresh_store("sequential");
    let first = bodies(launch(&store, &session(&["one".to_string()])));
    assert_eq!(first[0]["status"], "scaffolded");
    assert!(store.join(LOCK_FILE).is_file());
    // The first process has exited: its flock went with it, and
    // the lock file it left behind blocks nobody.
    let started = Instant::now();
    let second = bodies(launch(&store, &session(&["two".to_string()])));
    assert_eq!(second[0]["status"], "scaffolded");
    assert!(started.elapsed() < LOCK_WAIT);
    assert_store_whole(&store, &["one".to_string(), "two".to_string()]);
    let _ = std::fs::remove_dir_all(&store);
}

// ---------- adversarial ----------

#[test]
fn adv_two_processes_racing_the_same_ids_each_id_is_won_exactly_once() {
    // Both processes create the same fifty projects. Unlocked, both
    // could pass the existence check and both report success — one
    // write silently lost. Locked, each id has one winner and one
    // typed conflict.
    let store = fresh_store("race");
    let shared = ids("shared");
    let a = launch(&store, &session(&shared));
    let b = launch(&store, &session(&shared));
    let (a, b) = (bodies(a), bodies(b));
    for ((left, right), id) in a.iter().zip(&b).zip(&shared) {
        let won = [left, right]
            .iter()
            .filter(|body| body["status"] == "scaffolded")
            .count();
        let conflicted = [left, right]
            .iter()
            .filter(|body| body["error"]["code"] == "conflict")
            .count();
        assert_eq!((won, conflicted), (1, 1), "{id}: {left} / {right}");
    }
    assert_store_whole(&store, &shared);
    let _ = std::fs::remove_dir_all(&store);
}

#[test]
fn adv_a_third_process_holding_the_lock_past_the_bound_makes_writes_store_busy() {
    let store = fresh_store("busy");
    let seeded = bodies(launch(&store, &session(&["seed".to_string()])));
    assert_eq!(seeded[0]["status"], "scaffolded");
    // This test process is the third process: it takes the flock
    // and keeps it past the servers' bound.
    let holder = std::fs::File::open(store.join(LOCK_FILE)).unwrap();
    holder.try_lock().expect("the lock is free after the seed");
    let before = tree(&store);
    let started = Instant::now();
    let a = launch(&store, &session(&["left".to_string()]));
    let b = launch(&store, &session(&["right".to_string()]));
    let (a, b) = (bodies(a), bodies(b));
    let waited = started.elapsed();
    for body in [&a[0], &b[0]] {
        assert_eq!(body["error"]["code"], "store_busy", "{body}");
    }
    assert!(waited >= LOCK_WAIT, "refused early, after {waited:?}");
    assert!(waited < LOCK_WAIT * 2, "refused late, after {waited:?}");
    assert_eq!(tree(&store), before, "a store_busy call wrote something");
    // Released, the same write lands.
    holder.unlock().unwrap();
    let after = bodies(launch(&store, &session(&["left".to_string()])));
    assert_eq!(after[0]["status"], "scaffolded");
    assert_store_whole(&store, &["seed".to_string(), "left".to_string()]);
    let _ = std::fs::remove_dir_all(&store);
}
