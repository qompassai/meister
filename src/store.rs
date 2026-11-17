//! The Rung 1 record store: project scaffolding and cast/canon
//! records as real files on disk.
//!
//! Layout under the store root:
//!
//! ```text
//! <root>/<project_id>/project.json
//! <root>/<project_id>/cast/<character_id>.json
//! ```
//!
//! The root is `MEISTER_PROJECTS_ROOT` when set, else
//! `$XDG_DATA_HOME/meister/projects`, else
//! `$HOME/.local/share/meister/projects`. Records are JSON,
//! written atomically (temp file + rename) only after every
//! validation for the call has passed: a refused call writes
//! nothing. All work is bounded by the constants below; a record
//! or source file over its bound is refused or reported
//! unverified, never absorbed.
//!
//! Mutations are serialized twice over (STORE-01): by an
//! in-process `Mutex`, and across processes by an advisory
//! `flock` on `<root>/.meister.lock` — several Meister processes
//! (one per surface profile) can share one store. The wait for
//! both locks together is bounded by `LOCK_WAIT`; a call that
//! cannot take them in time returns `store_busy` and writes
//! nothing. The lock file is store infrastructure, not a record:
//! zero bytes, never read, never removed. Reads take no lock:
//! every record lands by atomic rename, so a reader sees the old
//! record or the new one, never a torn one.
//!
//! Canon governance mirrors the estate's character-art discipline
//! (Light Show's canon practice, encoded): one identity record
//! per character with its governing references named inside the
//! record; a `two_model` character carries exactly one canonical
//! base form and one canonical adult form, and every further
//! design is quarantined or superseded with a reason and a ruling
//! reference; media enters through a keeper chain whose entries
//! are born `candidate` and are `admitted` only with a recorded
//! verdict. The canon census (see `canon_census`) recomputes
//! every claim from the files themselves: each check names its
//! source artifact and the SHA-256 it must carry, and any claim
//! whose source is missing or mismatched is unverified — the
//! census fails closed (D-713).
//!
//! Every census also records the project's `state_hash` — one
//! SHA-256 over every record in the project (see `state_hash`) —
//! so a census names the exact state it ran against (STORE-02).
//! A census whose recorded hash no longer matches the project is
//! stale: it describes a state that is gone.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::hash::sha256_hex;

/// Environment variable naming the store root.
pub const ENV_ROOT: &str = "MEISTER_PROJECTS_ROOT";

const MAX_ID_CHARS: usize = 64;
const MAX_NAME_CHARS: usize = 120;
const MAX_TEXT_CHARS: usize = 500;
const MAX_PATH_CHARS: usize = 1024;
const MAX_PROJECTS_SCANNED: usize = 10_000;
const MAX_CAST_PER_PROJECT: usize = 256;
const MAX_CANON_FACTS: usize = 128;
const MAX_FORMS: usize = 16;
const MAX_REFERENCES: usize = 32;
const MAX_CHAIN_ENTRIES: usize = 64;
const MAX_PALETTE_ENTRIES: usize = 32;
const MAX_SECONDARY_PACKS: usize = 12;
const MAX_RECORD_BYTES: u64 = 1_048_576;

/// Directories under a project directory that hold records, one
/// JSON file per record, each bounded by `MAX_CAST_PER_PROJECT`.
/// `project.json` beside them is the one record outside a
/// directory. A rung that adds a record kind adds its directory
/// here, and the state hash covers it from that moment.
const RECORD_DIRS: &[&str] = &["cast"];
const MAX_SOURCE_BYTES: u64 = 33_554_432;

/// Name of the cross-process lock file under the store root. The
/// leading dot keeps it out of the id space: no project can carry
/// this name (see `valid_id`).
const LOCK_FILE: &str = ".meister.lock";

/// Bound on the whole wait for the store lock — the in-process
/// mutex and the cross-process `flock` together. Past it the call
/// returns `store_busy`.
const LOCK_WAIT: Duration = Duration::from_secs(10);

/// Interval between lock attempts while waiting.
const LOCK_POLL: Duration = Duration::from_millis(10);

/// Serializes store mutations in-process: dispatch may run calls
/// concurrently, and a create/lock pair must never interleave.
static STORE_LOCK: Mutex<()> = Mutex::new(());

/// The store lock, held for one mutation: the in-process mutex
/// and the `flock` on the lock file. Dropping it releases both —
/// the `flock` first (closing the file releases it), then the
/// mutex, the reverse of the order they were taken in.
struct StoreGuard {
    _file: File,
    _process: MutexGuard<'static, ()>,
}

/// A typed store failure. `census` carries the full census body
/// when the failure is an unverified census, so the caller sees
/// exactly which claims failed instead of a bare refusal.
#[derive(Debug)]
pub struct StoreError {
    /// Stable typed code (`invalid_input`, `not_found`,
    /// `conflict`, `invalid_record`, `identity_lock_conflict`,
    /// `canon_unverified`, `census_stale`, `store_busy`, `io`).
    pub code: &'static str,
    /// Human-readable reason, safe to return to the caller.
    pub message: String,
    /// The census body, for `canon_unverified` and `census_stale`
    /// failures only.
    pub census: Option<Value>,
}

impl StoreError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        StoreError {
            code,
            message: message.into(),
            census: None,
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_input", message)
    }

    fn invalid_record(message: impl Into<String>) -> Self {
        Self::new("invalid_record", message)
    }

    fn io(message: impl Into<String>) -> Self {
        Self::new("io", message)
    }
}

type StoreResult<T> = Result<T, StoreError>;

/// A mutation that has passed every validation: the record to
/// write (none when the call is an idempotent repeat) and the
/// body to answer with.
struct Planned {
    write: Option<(PathBuf, Value)>,
    body: Value,
}

/// The store: one root directory holding every project.
pub struct Store {
    root: PathBuf,
    /// Bound on the store-lock wait; `LOCK_WAIT` outside tests.
    lock_wait: Duration,
}

impl Store {
    /// A store at an explicit root (tests, gate drivers).
    pub fn new(root: PathBuf) -> Self {
        Store {
            root,
            lock_wait: LOCK_WAIT,
        }
    }

    /// Run one mutation. The plan — every validation for the call,
    /// read-only — runs twice: once before the lock, so a refused
    /// call returns having touched nothing, not even the lock
    /// file; and once under the lock, so the write commits against
    /// the state it was validated on. Only then is anything
    /// written.
    fn mutate(&self, plan: impl Fn() -> StoreResult<Planned>) -> StoreResult<Value> {
        plan()?;
        let _guard = lock_store(&self.root, self.lock_wait)?;
        let planned = plan()?;
        if let Some((file, record)) = &planned.write {
            write_json_atomic(file, record)?;
        }
        Ok(planned.body)
    }

    /// The serving store: root from the environment contract in
    /// the module docs.
    pub fn from_env() -> Self {
        if let Ok(root) = std::env::var(ENV_ROOT)
            && !root.trim().is_empty()
        {
            return Store::new(PathBuf::from(root));
        }
        if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
            && !xdg.trim().is_empty()
        {
            return Store::new(PathBuf::from(xdg).join("meister").join("projects"));
        }
        if let Ok(home) = std::env::var("HOME")
            && !home.trim().is_empty()
        {
            return Store::new(
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("meister")
                    .join("projects"),
            );
        }
        Store::new(PathBuf::from("meister-projects"))
    }

    fn project_dir(&self, project_id: &str) -> PathBuf {
        self.root.join(project_id)
    }

    fn project_file(&self, project_id: &str) -> PathBuf {
        self.project_dir(project_id).join("project.json")
    }

    fn cast_file(&self, project_id: &str, character_id: &str) -> PathBuf {
        self.project_dir(project_id)
            .join("cast")
            .join(format!("{character_id}.json"))
    }

    // ---------------------------------------------------------------
    // Group 1 — project scaffolding
    // ---------------------------------------------------------------

    /// `meister_project_create`: scaffold a project record set.
    /// Validates everything before writing anything; a duplicate
    /// project id is a conflict, never an overwrite.
    pub fn create_project(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_create_project(args))
    }

    fn plan_create_project(&self, args: &Value) -> StoreResult<Planned> {
        let name = req_text(args, "name", MAX_NAME_CHARS)?;
        let engine = req_text(args, "engine", MAX_TEXT_CHARS)?;
        if !ENGINES.contains(&engine.as_str()) {
            return Err(StoreError::invalid(format!("unknown engine {engine}")));
        }
        let genre_pack = req_text(args, "genre_pack", MAX_TEXT_CHARS)?;
        if !GENRE_PACKS.contains(&genre_pack.as_str()) {
            return Err(StoreError::invalid(format!(
                "unknown genre_pack {genre_pack}"
            )));
        }
        let project_id = match args.get("project_id") {
            Some(id) => valid_id(id.as_str().unwrap_or(""))?,
            None => slugify(&name)?,
        };
        let secondary = secondary_packs(args, &genre_pack)?;
        let canon = canon_facts(args.get("canon"))?;
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let file = self.project_file(&project_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("project {project_id} already exists"),
            ));
        }
        let record = json!({
            "record_type": "meister_project",
            "schema_version": 1,
            "project_id": project_id,
            "name": name,
            "engine": engine,
            "genre_pack": genre_pack,
            "secondary_genre_packs": secondary,
            "status": "scaffolded",
            "envelope": envelope,
            "canon": canon,
            "created_by_tool": "meister_project_create",
            "server_version": env!("CARGO_PKG_VERSION"),
        });
        let body = json!({
            "project_id": project_id,
            "engine": engine,
            "genre_pack": genre_pack,
            "secondary_genre_packs": record["secondary_genre_packs"],
            "canon_fact_count": record["canon"].as_array().map_or(0, Vec::len),
            "status": "scaffolded",
            "project_path": self.project_dir(&project_id).display().to_string(),
        });
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// `meister_project_inspect`: one project's record, with the
    /// cast and canon counts computed from the files on disk.
    pub fn inspect_project(&self, args: &Value) -> StoreResult<Value> {
        let project_id = arg_id(args, "project_id")?;
        let record = self.read_project(&project_id)?;
        let cast = self.read_cast_records(&project_id)?;
        Ok(json!({
            "project_id": project_id,
            "name": record["name"],
            "engine": record["engine"],
            "genre_pack": record["genre_pack"],
            "secondary_genre_packs": record["secondary_genre_packs"],
            "origin_lane": record["envelope"]["origin_lane"],
            "content_label": record["envelope"]["content_label"],
            "status": record["status"],
            "cast_count": cast.len(),
            "canon_fact_count": record["canon"].as_array().map_or(0, Vec::len),
            "project_path": self.project_dir(&project_id).display().to_string(),
        }))
    }

    /// `meister_project_list`: bounded, cursor-paginated, sorted
    /// by project id so pages are stable.
    pub fn list_projects(&self, args: &Value) -> StoreResult<Value> {
        let limit = match args.get("limit") {
            None | Some(Value::Null) => 25usize,
            Some(v) => {
                let n = v
                    .as_u64()
                    .ok_or_else(|| StoreError::invalid("limit must be an integer"))?;
                usize::try_from(n)
                    .ok()
                    .filter(|n| (1..=100).contains(n))
                    .ok_or_else(|| StoreError::invalid("limit must be within 1..=100"))?
            }
        };
        let cursor = match args.get("cursor") {
            None | Some(Value::Null) => None,
            Some(v) => Some(valid_id(v.as_str().unwrap_or(""))?),
        };
        let mut ids = self.project_ids()?;
        if let Some(cursor) = &cursor {
            ids.retain(|id| id > cursor);
        }
        let page: Vec<String> = ids.iter().take(limit).cloned().collect();
        let next_cursor = if ids.len() > limit {
            page.last().cloned()
        } else {
            None
        };
        let mut projects = Vec::with_capacity(page.len());
        for id in &page {
            let record = self.read_project(id)?;
            projects.push(json!({
                "project_id": id,
                "name": record["name"],
                "engine": record["engine"],
                "genre_pack": record["genre_pack"],
                "status": record["status"],
            }));
        }
        Ok(json!({
            "projects": projects,
            "count": projects.len(),
            "next_cursor": next_cursor,
        }))
    }

    // ---------------------------------------------------------------
    // Group 2 — cast and canon
    // ---------------------------------------------------------------

    /// `meister_cast_add`: add one identity record, born
    /// `candidate`. The identity is validated against the canon
    /// governance rules before anything is written.
    pub fn add_cast(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_add_cast(args))
    }

    fn plan_add_cast(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let character_id = arg_id(args, "character_id")?;
        self.read_project(&project_id)?;
        let identity = args
            .get("identity")
            .filter(|v| v.is_object())
            .ok_or_else(|| StoreError::invalid_record("identity must be an object"))?;
        validate_identity(identity)?;
        let file = self.cast_file(&project_id, &character_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("character {character_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let record = json!({
            "record_type": "meister_cast_member",
            "schema_version": 1,
            "project_id": project_id,
            "character_id": character_id,
            "status": "candidate",
            "envelope": envelope,
            "identity": identity,
            "identity_lock": Value::Null,
        });
        let body = json!({
            "project_id": project_id,
            "character_id": character_id,
            "status": "candidate",
            "governing_reference_count":
                identity["governing_references"].as_array().map_or(0, Vec::len),
            "record_path": file.display().to_string(),
        });
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// `meister_cast_lock`: identity-lock one character (D-1410).
    /// The lock must name the canonical form and its governing
    /// reference, and carry silhouette, palette, face and body
    /// proportions, and thumbnail recognisability — each with a
    /// source artifact and its SHA-256. A locked character can be
    /// re-locked only with the identical lock; a different lock is
    /// a conflict, never a silent overwrite.
    pub fn lock_cast(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_lock_cast(args))
    }

    fn plan_lock_cast(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let character_id = arg_id(args, "character_id")?;
        self.read_project(&project_id)?;
        let file = self.cast_file(&project_id, &character_id);
        let mut record = read_json_file(&file)?.ok_or_else(|| {
            StoreError::new(
                "not_found",
                format!("character {character_id} not found in {project_id}"),
            )
        })?;
        let lock = args
            .get("lock")
            .filter(|v| v.is_object())
            .ok_or_else(|| StoreError::invalid_record("lock must be an object"))?;
        validate_lock(lock, &record["identity"])?;
        let lock_hash = sha256_hex(canonical_json(lock).as_bytes());
        if let Some(existing) = record.get("identity_lock").filter(|v| !v.is_null()) {
            if existing["lock_hash"].as_str() == Some(lock_hash.as_str()) {
                return Ok(Planned {
                    write: None,
                    body: lock_body(&project_id, &character_id, &file, &lock_hash),
                });
            }
            return Err(StoreError::new(
                "identity_lock_conflict",
                format!(
                    "character {character_id} is already locked under a different lock; \
                     a changed lock needs a new canon ruling, not an overwrite"
                ),
            ));
        }
        let mut stored_lock = lock.clone();
        stored_lock["lock_hash"] = json!(lock_hash);
        stored_lock["locked_by_tool"] = json!("meister_cast_lock");
        record["identity_lock"] = stored_lock;
        record["status"] = json!("identity_locked");
        let body = lock_body(&project_id, &character_id, &file, &lock_hash);
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// `meister_canon_check`: the canon census, computed from the
    /// files. Succeeds only when it is current and carries zero
    /// unverified claims. Otherwise the full census rides the
    /// refusal: `census_stale` when the census does not describe
    /// the project's present state, else `canon_unverified`.
    ///
    /// `prior_state_hash` (optional) is the `state_hash` recorded
    /// by a census the caller already holds. If the project has
    /// moved on from it, that census is stale. Staleness outranks
    /// unverified claims in the code returned: the caller's frame
    /// of reference is wrong before any single claim is. The body
    /// carries both facts either way.
    pub fn canon_census(&self, args: &Value) -> StoreResult<Value> {
        let project_id = arg_id(args, "project_id")?;
        let prior = match args.get("prior_state_hash") {
            None | Some(Value::Null) => None,
            Some(v) => Some(
                v.as_str()
                    .filter(|raw| is_hex64(raw))
                    .ok_or_else(|| {
                        StoreError::invalid("prior_state_hash must be 64 lowercase hex characters")
                    })?
                    .to_string(),
            ),
        };
        let census = Census::run(self, &project_id, prior.as_deref())?;
        if census.stale {
            Err(StoreError {
                code: "census_stale",
                message: format!(
                    "canon census for {project_id} is stale: {}",
                    census.stale_reason
                ),
                census: Some(census.body),
            })
        } else if census.unverified == 0 {
            Ok(census.body)
        } else {
            Err(StoreError {
                code: "canon_unverified",
                message: format!(
                    "canon census for {project_id}: {} of {} checks unverified",
                    census.unverified, census.checked
                ),
                census: Some(census.body),
            })
        }
    }

    /// The project's state hash: SHA-256 over the canonical JSON
    /// of every record in the project, in sorted path order.
    ///
    /// Concretely: one JSON object keyed by each record's path
    /// relative to the project directory (`project.json`,
    /// `cast/<id>.json`, ...), serialized canonically — keys
    /// sorted, compact — and hashed. Sorted keys are the sorted
    /// path order, and keying by path makes a moved record a
    /// changed state. The hash is over record content, not file
    /// bytes: key order and whitespace inside a file do not move
    /// it, any changed value does. The lock file and temp files
    /// are not records and are not covered.
    ///
    /// Fails closed: a record that cannot be read, parsed, or
    /// bounded is an error, never skipped.
    pub fn state_hash(&self, project_id: &str) -> StoreResult<String> {
        let records = self.read_records(project_id)?;
        Ok(sha256_hex(
            canonical_json(&Value::Object(records.into_iter().collect())).as_bytes(),
        ))
    }

    // ---------------------------------------------------------------
    // Shared reads
    // ---------------------------------------------------------------

    fn read_project(&self, project_id: &str) -> StoreResult<Value> {
        read_json_file(&self.project_file(project_id))?
            .ok_or_else(|| StoreError::new("not_found", format!("project {project_id} not found")))
    }

    fn project_ids(&self) -> StoreResult<Vec<String>> {
        let mut ids = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ids),
            Err(e) => return Err(StoreError::io(format!("cannot list store root: {e}"))),
        };
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(format!("store entry: {e}")))?;
            if ids.len() >= MAX_PROJECTS_SCANNED {
                return Err(StoreError::io("store exceeds the project scan bound"));
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().join("project.json").is_file() && valid_id(&name).is_ok() {
                ids.push(name);
            }
        }
        ids.sort();
        Ok(ids)
    }

    /// Every record in a project, keyed by its path relative to
    /// the project directory: `project.json`, then each `*.json`
    /// under the `RECORD_DIRS`.
    fn read_records(&self, project_id: &str) -> StoreResult<BTreeMap<String, Value>> {
        let mut records = BTreeMap::new();
        records.insert("project.json".to_string(), self.read_project(project_id)?);
        for dir_name in RECORD_DIRS {
            let dir = self.project_dir(project_id).join(dir_name);
            let entries = match fs::read_dir(&dir) {
                Ok(entries) => entries,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(StoreError::io(format!("cannot list {dir_name}: {e}"))),
            };
            let mut seen = 0usize;
            for entry in entries {
                let entry = entry.map_err(|e| StoreError::io(format!("{dir_name} entry: {e}")))?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("json") {
                    continue;
                }
                seen += 1;
                if seen > MAX_CAST_PER_PROJECT {
                    return Err(StoreError::io(format!(
                        "{dir_name} exceeds the per-project record bound"
                    )));
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(record) = read_json_file(&path)? {
                    records.insert(format!("{dir_name}/{name}"), record);
                }
            }
        }
        Ok(records)
    }

    fn read_cast_records(&self, project_id: &str) -> StoreResult<Vec<Value>> {
        let dir = self.project_dir(project_id).join("cast");
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::io(format!("cannot list cast: {e}"))),
        };
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(format!("cast entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if records.len() >= MAX_CAST_PER_PROJECT {
                return Err(StoreError::io("cast exceeds the per-project bound"));
            }
            if let Some(record) = read_json_file(&path)? {
                records.push(record);
            }
        }
        records.sort_by(|a, b| a["character_id"].as_str().cmp(&b["character_id"].as_str()));
        Ok(records)
    }
}

/// Engines a project may be born with (README §"Tool groups").
const ENGINES: &[&str] = &["bevy", "godot", "renpy", "unity", "unreal"];

/// The thirteen genre category slugs (README §"Genre categories").
const GENRE_PACKS: &[&str] = &[
    "action",
    "adventure",
    "educational",
    "fighting",
    "narrative",
    "platformer",
    "puzzle",
    "rhythm",
    "roguelike",
    "role_playing",
    "shooter",
    "simulation",
    "strategy",
];

fn store_busy(wait: Duration) -> StoreError {
    StoreError::new(
        "store_busy",
        format!(
            "the store lock was not free within {} ms; nothing was written",
            wait.as_millis()
        ),
    )
}

/// Take the store lock for one mutation: the in-process mutex,
/// then an exclusive advisory `flock` on `<root>/.meister.lock`.
/// Both are polled against one deadline, so the whole wait is
/// bounded by `wait`; past it the answer is `store_busy`. Creates
/// the root and the (empty) lock file if absent — the caller has
/// already validated its mutation, so a write is about to follow.
fn lock_store(root: &Path, wait: Duration) -> StoreResult<StoreGuard> {
    let deadline = Instant::now() + wait;
    let process = loop {
        match STORE_LOCK.try_lock() {
            Ok(guard) => break guard,
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(StoreError::io("store lock poisoned"));
            }
            Err(std::sync::TryLockError::WouldBlock) => {}
        }
        if Instant::now() >= deadline {
            return Err(store_busy(wait));
        }
        std::thread::sleep(LOCK_POLL);
    };
    fs::create_dir_all(root)
        .map_err(|e| StoreError::io(format!("cannot create {}: {e}", root.display())))?;
    let path = root.join(LOCK_FILE);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| StoreError::io(format!("cannot open {}: {e}", path.display())))?;
    loop {
        match file.try_lock() {
            Ok(()) => break,
            Err(fs::TryLockError::WouldBlock) => {}
            Err(fs::TryLockError::Error(e)) => {
                return Err(StoreError::io(format!(
                    "cannot lock {}: {e}",
                    path.display()
                )));
            }
        }
        if Instant::now() >= deadline {
            return Err(store_busy(wait));
        }
        std::thread::sleep(LOCK_POLL);
    }
    Ok(StoreGuard {
        _file: file,
        _process: process,
    })
}

fn lock_body(project_id: &str, character_id: &str, file: &Path, lock_hash: &str) -> Value {
    json!({
        "project_id": project_id,
        "character_id": character_id,
        "locked": true,
        "lock_hash": lock_hash,
        "record_path": file.display().to_string(),
    })
}

/// An id: `[a-z0-9]` then up to 63 of `[a-z0-9_-]`. Ids name
/// files, so anything else — dots, slashes, uppercase — is
/// refused before it can become a path.
fn valid_id(raw: &str) -> StoreResult<String> {
    let mut chars = raw.chars();
    let ok = raw.len() <= MAX_ID_CHARS
        && matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    if ok {
        Ok(raw.to_string())
    } else {
        Err(StoreError::invalid(format!(
            "id {raw:?} is not a valid record id (lowercase slug, max {MAX_ID_CHARS} chars)"
        )))
    }
}

fn arg_id(args: &Value, field: &str) -> StoreResult<String> {
    let raw = args
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::invalid(format!("{field} must be a string")))?;
    valid_id(raw)
}

fn slugify(name: &str) -> StoreResult<String> {
    let mut slug = String::new();
    let mut last_dash = true; // suppresses a leading dash
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            slug.push('-');
            last_dash = true;
        }
        if slug.len() > MAX_ID_CHARS {
            break;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    valid_id(&slug).map_err(|_| {
        StoreError::invalid(format!(
            "name {name:?} does not reduce to a valid project id; pass project_id explicitly"
        ))
    })
}

fn req_text(args: &Value, field: &str, max_chars: usize) -> StoreResult<String> {
    let text = args
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= max_chars)
        .ok_or_else(|| {
            StoreError::invalid(format!(
                "{field} must be a non-empty string of at most {max_chars} chars"
            ))
        })?;
    Ok(text.to_string())
}

fn opt_text(obj: &Value, field: &str, max_chars: usize) -> StoreResult<String> {
    obj.get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= max_chars)
        .map(str::to_string)
        .ok_or_else(|| {
            StoreError::invalid_record(format!(
                "{field} must be a non-empty string of at most {max_chars} chars"
            ))
        })
}

fn req_arr<'a>(obj: &'a Value, field: &str, max: usize) -> StoreResult<&'a Vec<Value>> {
    let arr = obj
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| StoreError::invalid_record(format!("{field} must be an array")))?;
    if arr.len() > max {
        return Err(StoreError::invalid_record(format!(
            "{field} exceeds its bound of {max} entries"
        )));
    }
    Ok(arr)
}

fn is_hex64(raw: &str) -> bool {
    raw.len() == 64
        && raw
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

fn req_sha256(obj: &Value, field: &str) -> StoreResult<String> {
    let raw = obj
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| StoreError::invalid_record(format!("{field} must be a string")))?;
    if is_hex64(raw) {
        Ok(raw.to_string())
    } else {
        Err(StoreError::invalid_record(format!(
            "{field} must be 64 lowercase hex characters"
        )))
    }
}

/// Deterministic JSON text for hashing: serde_json's map is
/// key-sorted, so the same record always hashes identically.
fn canonical_json(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn secondary_packs(args: &Value, primary: &str) -> StoreResult<Vec<Value>> {
    let Some(raw) = args.get("secondary_genre_packs") else {
        return Ok(Vec::new());
    };
    let arr = raw
        .as_array()
        .ok_or_else(|| StoreError::invalid("secondary_genre_packs must be an array"))?;
    if arr.len() > MAX_SECONDARY_PACKS {
        return Err(StoreError::invalid(
            "secondary_genre_packs exceeds its bound",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut out = Vec::with_capacity(arr.len());
    for pack in arr {
        let pack = pack
            .as_str()
            .ok_or_else(|| StoreError::invalid("secondary genre pack must be a string"))?;
        if !GENRE_PACKS.contains(&pack) {
            return Err(StoreError::invalid(format!(
                "unknown secondary genre_pack {pack}"
            )));
        }
        if pack == primary || !seen.insert(pack.to_string()) {
            return Err(StoreError::invalid(format!(
                "secondary genre_pack {pack} duplicates the primary or itself"
            )));
        }
        out.push(json!(pack));
    }
    Ok(out)
}

/// Validate the canon facts carried at project birth. Structure
/// only: whether a fact's source artifact actually carries the
/// recorded hash is the census's job, not birth's.
fn canon_facts(raw: Option<&Value>) -> StoreResult<Vec<Value>> {
    let Some(raw) = raw else {
        return Ok(Vec::new());
    };
    let arr = raw
        .as_array()
        .ok_or_else(|| StoreError::invalid("canon must be an array of fact objects"))?;
    if arr.len() > MAX_CANON_FACTS {
        return Err(StoreError::invalid("canon exceeds its fact bound"));
    }
    let mut seen = BTreeSet::new();
    for fact in arr {
        if !fact.is_object() {
            return Err(StoreError::invalid_record("canon fact must be an object"));
        }
        let id = valid_id(fact.get("id").and_then(Value::as_str).unwrap_or_default())
            .map_err(|_| StoreError::invalid_record("canon fact id is not a valid id"))?;
        if !seen.insert(id) {
            return Err(StoreError::invalid_record("duplicate canon fact id"));
        }
        opt_text(fact, "statement", MAX_TEXT_CHARS)
            .map_err(|_| StoreError::invalid_record("canon fact statement is required"))?;
        if fact.get("value").is_none() {
            return Err(StoreError::invalid_record(
                "canon fact value is required (null is a value; absence is not)",
            ));
        }
        opt_text(fact, "source_artifact", MAX_PATH_CHARS)
            .map_err(|_| StoreError::invalid_record("canon fact source_artifact is required"))?;
        req_sha256(fact, "source_sha256")
            .map_err(|_| StoreError::invalid_record("canon fact source_sha256 must be hex64"))?;
    }
    Ok(arr.clone())
}

// ---------------------------------------------------------------
// Identity governance (the Light Show canon model, encoded)
// ---------------------------------------------------------------

/// One named reference object (`{id, path, sha256, source, ...}`).
fn validate_reference(reference: &Value, kind_field: bool) -> StoreResult<String> {
    if !reference.is_object() {
        return Err(StoreError::invalid_record("reference must be an object"));
    }
    let id = valid_id(
        reference
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default(),
    )
    .map_err(|_| StoreError::invalid_record("reference id is not a valid id"))?;
    opt_text(reference, "path", MAX_PATH_CHARS)?;
    req_sha256(reference, "sha256")?;
    opt_text(reference, "source", MAX_TEXT_CHARS)?;
    if kind_field {
        opt_text(reference, "kind", MAX_TEXT_CHARS)?;
    }
    Ok(id)
}

/// Validate one character identity record against the governance
/// rules. Pure structure and consistency — file/hash truth is
/// the census's layer.
fn validate_identity(identity: &Value) -> StoreResult<()> {
    opt_text(identity, "display_name", MAX_NAME_CHARS)?;
    opt_text(identity, "role", MAX_TEXT_CHARS)?;
    let scheme = opt_text(identity, "model_scheme", MAX_TEXT_CHARS)?;
    if scheme != "two_model" && scheme != "single_model" {
        return Err(StoreError::invalid_record(
            "model_scheme must be two_model or single_model",
        ));
    }
    let refs = req_arr(identity, "governing_references", MAX_REFERENCES)?;
    if refs.is_empty() {
        return Err(StoreError::invalid_record(
            "governing_references must name at least one governing reference",
        ));
    }
    let mut ref_ids = BTreeSet::new();
    let mut canonical_refs = BTreeSet::new();
    for reference in refs {
        let id = validate_reference(reference, true)?;
        if !ref_ids.insert(id.clone()) {
            return Err(StoreError::invalid_record(format!(
                "duplicate governing reference {id}"
            )));
        }
        match reference.get("status").and_then(Value::as_str) {
            Some("canonical") => {
                canonical_refs.insert(id);
            }
            Some("supporting") => {}
            _ => {
                return Err(StoreError::invalid_record(
                    "governing reference status must be canonical or supporting",
                ));
            }
        }
    }
    for field in ["appearance_references", "rig_references"] {
        let arr = req_arr(identity, field, MAX_REFERENCES)?;
        let mut seen = BTreeSet::new();
        for reference in arr {
            let id = validate_reference(reference, false)?;
            if !seen.insert(id.clone()) {
                return Err(StoreError::invalid_record(format!(
                    "duplicate {field} entry {id}"
                )));
            }
        }
    }
    validate_forms(identity, &scheme, &ref_ids, &canonical_refs)?;
    validate_media_chain(identity)?;
    validate_quarantine(identity)?;
    Ok(())
}

fn validate_forms(
    identity: &Value,
    scheme: &str,
    ref_ids: &BTreeSet<String>,
    canonical_refs: &BTreeSet<String>,
) -> StoreResult<()> {
    let forms = req_arr(identity, "forms", MAX_FORMS)?;
    if forms.is_empty() {
        return Err(StoreError::invalid_record(
            "forms must carry at least one form",
        ));
    }
    let mut canonical_kinds: Vec<String> = Vec::new();
    for form in forms {
        if !form.is_object() {
            return Err(StoreError::invalid_record("form must be an object"));
        }
        valid_id(
            form.get("form_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .map_err(|_| StoreError::invalid_record("form_id is not a valid id"))?;
        valid_id(
            form.get("design_id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .map_err(|_| StoreError::invalid_record("design_id is not a valid id"))?;
        let kind = opt_text(form, "kind", MAX_TEXT_CHARS)?;
        let governing = opt_text(form, "governing_reference_id", MAX_ID_CHARS)?;
        if !ref_ids.contains(&governing) {
            return Err(StoreError::invalid_record(format!(
                "form governing_reference_id {governing} is not a governing reference"
            )));
        }
        match form.get("canon_status").and_then(Value::as_str) {
            Some("canonical") => {
                if !canonical_refs.contains(&governing) {
                    return Err(StoreError::invalid_record(format!(
                        "canonical form is governed by non-canonical reference {governing}"
                    )));
                }
                if canonical_kinds.contains(&kind) {
                    return Err(StoreError::invalid_record(format!(
                        "two canonical forms of kind {kind}: one canonical model per kind"
                    )));
                }
                canonical_kinds.push(kind);
            }
            Some("quarantined") | Some("superseded") => {
                let quarantine = form
                    .get("quarantine")
                    .filter(|q| q.is_object())
                    .ok_or_else(|| {
                        StoreError::invalid_record(
                            "a non-canonical form must carry its quarantine record \
                             (reason + ruling_ref)",
                        )
                    })?;
                opt_text(quarantine, "reason", MAX_TEXT_CHARS)?;
                opt_text(quarantine, "ruling_ref", MAX_TEXT_CHARS)?;
            }
            _ => {
                return Err(StoreError::invalid_record(
                    "form canon_status must be canonical, quarantined, or superseded",
                ));
            }
        }
    }
    match scheme {
        "two_model" => {
            let mut kinds = canonical_kinds.clone();
            kinds.sort();
            if kinds != ["adult", "base"] {
                return Err(StoreError::invalid_record(
                    "two_model governance: exactly one canonical base form and one \
                     canonical adult form; every further design is quarantined",
                ));
            }
        }
        _ => {
            if canonical_kinds.len() != 1 {
                return Err(StoreError::invalid_record(
                    "single_model governance: exactly one canonical form",
                ));
            }
        }
    }
    Ok(())
}

fn validate_media_chain(identity: &Value) -> StoreResult<()> {
    let chain = req_arr(identity, "media_chain", MAX_CHAIN_ENTRIES)?;
    for entry in chain {
        if !entry.is_object() {
            return Err(StoreError::invalid_record(
                "media chain entry must be an object",
            ));
        }
        opt_text(entry, "path", MAX_PATH_CHARS)?;
        opt_text(entry, "snapshot_id", MAX_TEXT_CHARS)?;
        req_sha256(entry, "sha256")?;
        match entry.get("status").and_then(Value::as_str) {
            Some("candidate") => {}
            Some("admitted") => {
                let verdict = opt_text(entry, "verdict", MAX_TEXT_CHARS)?;
                if verdict.eq_ignore_ascii_case("pending") {
                    return Err(StoreError::invalid_record(
                        "an admitted keeper needs a recorded verdict, not pending",
                    ));
                }
            }
            Some("quarantined") => {
                opt_text(entry, "verdict", MAX_TEXT_CHARS)?;
            }
            _ => {
                return Err(StoreError::invalid_record(
                    "media chain status must be candidate, admitted, or quarantined",
                ));
            }
        }
    }
    Ok(())
}

fn validate_quarantine(identity: &Value) -> StoreResult<()> {
    let arr = req_arr(identity, "quarantined_variants", MAX_REFERENCES)?;
    for variant in arr {
        if !variant.is_object() {
            return Err(StoreError::invalid_record(
                "quarantined variant must be an object",
            ));
        }
        valid_id(
            variant
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default(),
        )
        .map_err(|_| StoreError::invalid_record("quarantined variant id is invalid"))?;
        opt_text(variant, "path", MAX_PATH_CHARS)?;
        req_sha256(variant, "sha256")?;
        opt_text(variant, "reason", MAX_TEXT_CHARS)?;
        opt_text(variant, "ruling_ref", MAX_TEXT_CHARS)?;
    }
    Ok(())
}

// ---------------------------------------------------------------
// Identity locks (D-1410)
// ---------------------------------------------------------------

/// A sourced lock component: `{description, source_artifact,
/// source_sha256}`.
fn validate_sourced(obj: &Value, field: &str) -> StoreResult<()> {
    let component = obj
        .get(field)
        .filter(|v| v.is_object())
        .ok_or_else(|| StoreError::invalid_record(format!("lock {field} must be an object")))?;
    opt_text(component, "description", MAX_TEXT_CHARS)
        .map_err(|_| StoreError::invalid_record(format!("lock {field}.description is required")))?;
    opt_text(component, "source_artifact", MAX_PATH_CHARS).map_err(|_| {
        StoreError::invalid_record(format!("lock {field}.source_artifact is required"))
    })?;
    req_sha256(component, "source_sha256")
        .map_err(|_| StoreError::invalid_record(format!("lock {field}.source_sha256 invalid")))?;
    Ok(())
}

/// Validate a lock against the identity it locks. The lock is a
/// hard gate: every D-1410 dimension must be present and sourced,
/// and it must name the identity's canonical form and that
/// form's governing reference.
fn validate_lock(lock: &Value, identity: &Value) -> StoreResult<()> {
    let form_id = opt_text(lock, "form_id", MAX_ID_CHARS)?;
    let governing = opt_text(lock, "governing_reference_id", MAX_ID_CHARS)?;
    let forms = identity["forms"].as_array().cloned().unwrap_or_default();
    let form = forms
        .iter()
        .find(|f| f["form_id"].as_str() == Some(form_id.as_str()))
        .ok_or_else(|| StoreError::invalid_record(format!("lock names unknown form {form_id}")))?;
    if form["canon_status"].as_str() != Some("canonical") {
        return Err(StoreError::invalid_record(format!(
            "lock names form {form_id}, which is not the canonical form"
        )));
    }
    if form["governing_reference_id"].as_str() != Some(governing.as_str()) {
        return Err(StoreError::invalid_record(format!(
            "lock governing_reference_id {governing} does not govern form {form_id}"
        )));
    }
    validate_sourced(lock, "silhouette")?;
    validate_sourced(lock, "face_proportions")?;
    validate_sourced(lock, "body_proportions")?;
    let palette = req_arr(lock, "palette", MAX_PALETTE_ENTRIES)?;
    if palette.is_empty() {
        return Err(StoreError::invalid_record(
            "lock palette must carry at least one colour",
        ));
    }
    for colour in palette {
        opt_text(colour, "name", MAX_TEXT_CHARS)?;
        let hex = opt_text(colour, "hex", 7)?;
        let bytes = hex.as_bytes();
        if bytes.len() != 7 || bytes[0] != b'#' || !bytes[1..].iter().all(|b| b.is_ascii_hexdigit())
        {
            return Err(StoreError::invalid_record(format!(
                "palette hex {hex:?} is not #RRGGBB"
            )));
        }
        opt_text(colour, "source_artifact", MAX_PATH_CHARS)?;
        req_sha256(colour, "source_sha256")?;
    }
    let thumb = lock
        .get("thumbnail_recognizability")
        .filter(|v| v.is_object())
        .ok_or_else(|| {
            StoreError::invalid_record("lock thumbnail_recognizability must be an object")
        })?;
    for dim in ["width_px", "height_px"] {
        let v = thumb.get(dim).and_then(Value::as_u64).ok_or_else(|| {
            StoreError::invalid_record(format!("thumbnail {dim} must be an integer"))
        })?;
        if !(8..=512).contains(&v) {
            return Err(StoreError::invalid_record(format!(
                "thumbnail {dim} must be within 8..=512"
            )));
        }
    }
    if thumb.get("recognisable").and_then(Value::as_bool) != Some(true) {
        return Err(StoreError::invalid_record(
            "thumbnail_recognizability.recognisable must be true to lock: \
             a thumbnail that does not read as the character cannot be locked",
        ));
    }
    opt_text(thumb, "evidence", MAX_TEXT_CHARS)?;
    opt_text(thumb, "source_artifact", MAX_PATH_CHARS)?;
    req_sha256(thumb, "source_sha256")?;
    opt_text(lock, "ruling", MAX_TEXT_CHARS)?;
    Ok(())
}

// ---------------------------------------------------------------
// The canon census
// ---------------------------------------------------------------

struct Census {
    body: Value,
    checked: usize,
    unverified: usize,
    /// The census does not describe the project's present state.
    stale: bool,
    /// Why, when `stale`; empty otherwise.
    stale_reason: String,
}

/// Decide staleness from the three hashes a census run knows: the
/// state it started on, the state it ended on, and (optionally)
/// the state a census the caller holds was run against. Returns
/// the reason when stale.
///
/// Two ways to be stale: the project changed while the census was
/// running, so its claims straddle two states; or the caller's
/// earlier census was run against a state that is no longer the
/// project's.
fn staleness(before: &str, after: &str, prior: Option<&str>) -> Option<String> {
    if before != after {
        return Some(format!(
            "the project changed while the census ran (state {before} became {after})"
        ));
    }
    match prior {
        Some(prior) if prior != before => Some(format!(
            "the census recorded against state {prior} no longer describes the project \
             (state is now {before})"
        )),
        _ => None,
    }
}

struct CensusBuilder {
    checks: Vec<Value>,
    failures: Vec<String>,
}

impl CensusBuilder {
    fn new() -> Self {
        CensusBuilder {
            checks: Vec::new(),
            failures: Vec::new(),
        }
    }

    /// Record one claim. `verified` is the claim's truth as
    /// recomputed from its source; a false claim is also a named
    /// failure.
    fn check(&mut self, claim: Claim) {
        if !claim.verified {
            self.failures
                .push(format!("{}: {}", claim.id, claim.detail));
        }
        self.checks.push(json!({
            "id": claim.id,
            "subject": claim.subject,
            "claim": claim.claim,
            "value": claim.value,
            "source_artifact": claim.source_artifact,
            "source_sha256": claim.source_sha256,
            "status": if claim.verified { "verified" } else { "unverified" },
            "detail": claim.detail,
        }));
    }
}

/// One census claim: what was asserted, the value it carried,
/// the source artifact its truth recomputes from (with the
/// SHA-256 that source must carry), and the recomputed verdict.
struct Claim {
    id: String,
    subject: String,
    claim: String,
    value: Value,
    source_artifact: String,
    source_sha256: Option<String>,
    verified: bool,
    detail: String,
}

/// Verify one source artifact: resolve (relative paths resolve
/// against the project directory), bound, hash, compare.
fn verify_source(
    project_dir: &Path,
    source_artifact: &str,
    expected_sha256: &str,
) -> (bool, String) {
    let path = if Path::new(source_artifact).is_absolute() {
        PathBuf::from(source_artifact)
    } else {
        project_dir.join(source_artifact)
    };
    let metadata = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) => return (false, format!("source unreadable: {e}")),
    };
    if metadata.len() > MAX_SOURCE_BYTES {
        return (false, "source exceeds the census byte bound".to_string());
    }
    match fs::read(&path) {
        Ok(bytes) => {
            let actual = sha256_hex(&bytes);
            if actual == expected_sha256 {
                (true, "source hash matches".to_string())
            } else {
                (false, format!("source hash mismatch: actual {actual}"))
            }
        }
        Err(e) => (false, format!("source unreadable: {e}")),
    }
}

/// Hash a record file the census itself read, for use as the
/// source hash of structural checks over that record.
fn file_sha256(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if metadata.len() > MAX_RECORD_BYTES {
        return None;
    }
    fs::read(path).ok().map(|bytes| sha256_hex(&bytes))
}

impl Census {
    fn run(store: &Store, project_id: &str, prior: Option<&str>) -> StoreResult<Census> {
        // The state is hashed before the first claim and again
        // after the last: a census is only as good as the state it
        // names, so a state that moved underneath it is reported,
        // not papered over.
        let state_hash = store.state_hash(project_id)?;
        let project = store.read_project(project_id)?;
        let project_dir = store.project_dir(project_id);
        let project_file = store.project_file(project_id);
        let project_sha = file_sha256(&project_file);
        let mut b = CensusBuilder::new();
        census_project(&mut b, &project, &project_dir, &project_sha);
        let cast = store.read_cast_records(project_id)?;
        let mut locks = 0usize;
        let mut governing_refs = 0usize;
        let mut chain_entries = 0usize;
        for record in &cast {
            let (l, g, m) = census_cast(&mut b, store, project_id, &project_dir, record);
            locks += l;
            governing_refs += g;
            chain_entries += m;
        }
        let (references, duplicate_ids) =
            census_references(&mut b, store, project_id, &project, &cast);
        let stale_reason = staleness(&state_hash, &store.state_hash(project_id)?, prior);
        let stale = stale_reason.is_some();
        let stale_reason = stale_reason.unwrap_or_default();
        if stale {
            b.failures.push(format!("census_stale: {stale_reason}"));
        }
        let checked = b.checks.len();
        let unverified = b
            .checks
            .iter()
            .filter(|c| c["status"] == "unverified")
            .count();
        let body = json!({
            "project_id": project_id,
            "state_hash": state_hash,
            "stale": stale,
            "checked": checked,
            "unverified": unverified,
            "verdict": if unverified == 0 && !stale { "pass" } else { "review" },
            "counts": {
                "cast_records": cast.len(),
                "identity_locks": locks,
                "canon_facts": project["canon"].as_array().map_or(0, Vec::len),
                "governing_references": governing_refs,
                "media_chain_entries": chain_entries,
                "references_checked": references,
                "duplicate_record_ids": duplicate_ids,
            },
            "checks": b.checks,
            "failures": b.failures,
        });
        Ok(Census {
            body,
            checked,
            unverified,
            stale,
            stale_reason,
        })
    }
}

/// One reference-resolution claim (STORE-03, `canon_orphan`).
#[allow(clippy::too_many_arguments)]
fn ref_claim(
    b: &mut CensusBuilder,
    character_id: &str,
    kind: &str,
    target: &str,
    rel: &str,
    sha: &Option<String>,
    resolved: bool,
    project_id: &str,
) {
    b.check(Claim {
        id: format!("cast:{character_id}:ref:{kind}"),
        subject: character_id.to_string(),
        claim: "canon_orphan: every id a record references resolves".to_string(),
        value: json!(target),
        source_artifact: rel.to_string(),
        source_sha256: sha.clone(),
        verified: resolved,
        detail: if resolved {
            format!("reference to {kind} {target} resolves")
        } else {
            format!(
                "{rel} references {kind} {target}, which does not resolve in project {project_id}"
            )
        },
    });
}

/// STORE-03: the reference graph and record-id uniqueness, as
/// census claims over the records the census already read.
///
/// `canon_orphan` — every id a record references resolves: a cast
/// record's project, each form's governing reference, and the
/// identity lock's form and governing reference (the lock's
/// governing reference must also be the locked form's own, the
/// D-1410 contract recomputed as a reference claim).
///
/// `record_id_duplicate` — `envelope.record_id` is unique per
/// project (audit item A13): one claim per record; every record
/// carrying a shared id is unverified, naming all its holders.
///
/// Returns (references checked, distinct duplicated record ids).
fn census_references(
    b: &mut CensusBuilder,
    store: &Store,
    project_id: &str,
    project: &Value,
    cast: &[Value],
) -> (usize, usize) {
    let project_dir = store.project_dir(project_id);
    // Record-id uniqueness across the project's records.
    let mut holders: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut records: Vec<(String, Option<String>)> = Vec::new();
    let project_id_value = project["envelope"]["record_id"]
        .as_str()
        .map(str::to_string);
    if let Some(id) = &project_id_value {
        holders
            .entry(id.clone())
            .or_default()
            .push("project.json".to_string());
    }
    records.push(("project.json".to_string(), project_id_value));
    for record in cast {
        let character_id = record["character_id"].as_str().unwrap_or("?");
        let rel = format!("cast/{character_id}.json");
        let id = record["envelope"]["record_id"].as_str().map(str::to_string);
        if let Some(id) = &id {
            holders.entry(id.clone()).or_default().push(rel.clone());
        }
        records.push((rel, id));
    }
    let duplicate_ids = holders.values().filter(|v| v.len() > 1).count();
    for (rel, id) in &records {
        let (verified, detail) = match id {
            None => (false, "record carries no envelope.record_id".to_string()),
            Some(id) => {
                let held_by = &holders[id];
                if held_by.len() > 1 {
                    (
                        false,
                        format!("record_id {id} is shared by {}", held_by.join(", ")),
                    )
                } else {
                    (true, format!("record_id {id} is unique in the project"))
                }
            }
        };
        b.check(Claim {
            id: format!("record:{rel}:record_id"),
            subject: project_id.to_string(),
            claim: "record_id_duplicate: envelope.record_id is unique per project".to_string(),
            value: json!(id),
            source_artifact: rel.clone(),
            source_sha256: file_sha256(&project_dir.join(rel)),
            verified,
            detail,
        });
    }
    // Reference resolution, per cast record.
    let mut references = 0usize;
    for record in cast {
        let character_id = record["character_id"].as_str().unwrap_or("?");
        let rel = format!("cast/{character_id}.json");
        let sha = file_sha256(&project_dir.join(&rel));
        let identity = &record["identity"];
        let governing_refs = identity["governing_references"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let governing: BTreeSet<&str> = governing_refs
            .iter()
            .filter_map(|r| r["id"].as_str())
            .collect();
        let forms = identity["forms"].as_array().cloned().unwrap_or_default();
        references += 1;
        let target = record["project_id"].as_str().unwrap_or("");
        ref_claim(
            b,
            character_id,
            "project",
            target,
            &rel,
            &sha,
            target == project_id,
            project_id,
        );
        for form in &forms {
            if let Some(g) = form["governing_reference_id"].as_str() {
                references += 1;
                ref_claim(
                    b,
                    character_id,
                    "form_governing",
                    g,
                    &rel,
                    &sha,
                    governing.contains(g),
                    project_id,
                );
            }
        }
        if let Some(lock) = record.get("identity_lock").filter(|v| !v.is_null()) {
            let locked_form = lock["form_id"]
                .as_str()
                .and_then(|fid| forms.iter().find(|f| f["form_id"].as_str() == Some(fid)));
            if let Some(fid) = lock["form_id"].as_str() {
                references += 1;
                ref_claim(
                    b,
                    character_id,
                    "lock_form",
                    fid,
                    &rel,
                    &sha,
                    locked_form.is_some(),
                    project_id,
                );
            }
            if let Some(g) = lock["governing_reference_id"].as_str() {
                references += 1;
                let resolved = governing.contains(g)
                    && locked_form.is_some_and(|f| f["governing_reference_id"].as_str() == Some(g));
                ref_claim(
                    b,
                    character_id,
                    "lock_governing",
                    g,
                    &rel,
                    &sha,
                    resolved,
                    project_id,
                );
            }
        }
    }
    (references, duplicate_ids)
}

fn census_project(
    b: &mut CensusBuilder,
    project: &Value,
    project_dir: &Path,
    project_sha: &Option<String>,
) {
    let envelope = &project["envelope"];
    let lane_ok = envelope["origin_lane"].as_str() == Some("qompass")
        && envelope["content_label"].as_str() == Some("sfw")
        && envelope["evidence"]
            .as_str()
            .is_some_and(|e| !e.trim().is_empty());
    b.check(Claim {
        id: "project:lane_stamp".to_string(),
        subject: "project".to_string(),
        claim: "project record is born qompass/sfw with evidence".to_string(),
        value: json!(lane_ok),
        source_artifact: "project.json".to_string(),
        source_sha256: project_sha.clone(),
        verified: lane_ok && project_sha.is_some(),
        detail: if lane_ok {
            "lane stamp present".to_string()
        } else {
            "lane stamp missing or wrong".to_string()
        },
    });
    let genre = project["genre_pack"].as_str().unwrap_or("");
    b.check(Claim {
        id: "project:genre_pack".to_string(),
        subject: "project".to_string(),
        claim: "primary genre pack is a known category".to_string(),
        value: json!(genre),
        source_artifact: "project.json".to_string(),
        source_sha256: project_sha.clone(),
        verified: GENRE_PACKS.contains(&genre),
        detail: "genre pack checked against the thirteen categories".to_string(),
    });
    let facts = project["canon"].as_array().cloned().unwrap_or_default();
    for fact in &facts {
        let id = fact["id"].as_str().unwrap_or("?");
        let source = fact["source_artifact"].as_str().unwrap_or("");
        let expected = fact["source_sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!("canon_fact:{id}"),
            subject: "canon_fact".to_string(),
            claim: fact["statement"].as_str().unwrap_or("").to_string(),
            value: fact["value"].clone(),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
}

fn census_cast(
    b: &mut CensusBuilder,
    store: &Store,
    project_id: &str,
    project_dir: &Path,
    record: &Value,
) -> (usize, usize, usize) {
    let character_id = record["character_id"].as_str().unwrap_or("?");
    let file = store.cast_file(project_id, character_id);
    let rel = format!("cast/{character_id}.json");
    let file_sha = file_sha256(&file);
    let identity = &record["identity"];
    let governance = validate_identity(identity);
    b.check(Claim {
        id: format!("cast:{character_id}:governance"),
        subject: character_id.to_string(),
        claim: "identity record satisfies the canon governance rules".to_string(),
        value: json!(governance.is_ok()),
        source_artifact: rel.clone(),
        source_sha256: file_sha.clone(),
        verified: governance.is_ok(),
        detail: governance
            .as_ref()
            .err()
            .map_or("governance rules hold".to_string(), |e| e.message.clone()),
    });
    let mut governing_refs = 0usize;
    for field in [
        "governing_references",
        "appearance_references",
        "rig_references",
    ] {
        for reference in identity[field].as_array().cloned().unwrap_or_default() {
            if field == "governing_references" {
                governing_refs += 1;
            }
            let source = reference["path"].as_str().unwrap_or("");
            let expected = reference["sha256"].as_str().unwrap_or("");
            let (ok, detail) = verify_source(project_dir, source, expected);
            b.check(Claim {
                id: format!(
                    "cast:{character_id}:{field}:{}",
                    reference["id"].as_str().unwrap_or("?")
                ),
                subject: character_id.to_string(),
                claim: format!("{field} entry carries its recorded hash"),
                value: json!(expected),
                source_artifact: source.to_string(),
                source_sha256: Some(expected.to_string()),
                verified: ok,
                detail,
            });
        }
    }
    let mut chain_entries = 0usize;
    for entry in identity["media_chain"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        chain_entries += 1;
        let source = entry["path"].as_str().unwrap_or("");
        let expected = entry["sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!(
                "cast:{character_id}:media_chain:{}",
                entry["snapshot_id"].as_str().unwrap_or("?")
            ),
            subject: character_id.to_string(),
            claim: "keeper-chain entry carries its recorded hash".to_string(),
            value: json!(entry["status"]),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
    for variant in identity["quarantined_variants"]
        .as_array()
        .cloned()
        .unwrap_or_default()
    {
        let source = variant["path"].as_str().unwrap_or("");
        let expected = variant["sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!(
                "cast:{character_id}:quarantined:{}",
                variant["id"].as_str().unwrap_or("?")
            ),
            subject: character_id.to_string(),
            claim: "quarantined variant is manifested with its hash and ruling".to_string(),
            value: json!(variant["ruling_ref"]),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
    let locks = census_lock(b, project_dir, character_id, record);
    (locks, governing_refs, chain_entries)
}

fn census_lock(
    b: &mut CensusBuilder,
    project_dir: &Path,
    character_id: &str,
    record: &Value,
) -> usize {
    let Some(lock) = record.get("identity_lock").filter(|v| !v.is_null()) else {
        b.check(Claim {
            id: format!("cast:{character_id}:identity_lock"),
            subject: character_id.to_string(),
            claim: "character is identity-locked before detail work (D-1410)".to_string(),
            value: json!(false),
            source_artifact: format!("cast/{character_id}.json"),
            source_sha256: None,
            verified: false,
            detail: "no identity lock on record".to_string(),
        });
        return 0;
    };
    let mut unsigned = lock.clone();
    if let Some(obj) = unsigned.as_object_mut() {
        obj.remove("lock_hash");
        obj.remove("locked_by_tool");
    }
    let recomputed = sha256_hex(canonical_json(&unsigned).as_bytes());
    let stored = lock["lock_hash"].as_str().unwrap_or("");
    b.check(Claim {
        id: format!("cast:{character_id}:lock_hash"),
        subject: character_id.to_string(),
        claim: "stored lock hash recomputes from the lock record".to_string(),
        value: json!(stored),
        source_artifact: format!("cast/{character_id}.json"),
        source_sha256: None,
        verified: recomputed == stored,
        detail: "lock hash recomputed by the census".to_string(),
    });
    let lock_valid = validate_lock(lock, &record["identity"]);
    b.check(Claim {
        id: format!("cast:{character_id}:lock_shape"),
        subject: character_id.to_string(),
        claim: "lock still satisfies the D-1410 lock contract".to_string(),
        value: json!(lock_valid.is_ok()),
        source_artifact: format!("cast/{character_id}.json"),
        source_sha256: None,
        verified: lock_valid.is_ok(),
        detail: lock_valid
            .as_ref()
            .err()
            .map_or("lock contract holds".to_string(), |e| e.message.clone()),
    });
    for field in ["silhouette", "face_proportions", "body_proportions"] {
        let source = lock[field]["source_artifact"].as_str().unwrap_or("");
        let expected = lock[field]["source_sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!("cast:{character_id}:lock:{field}"),
            subject: character_id.to_string(),
            claim: format!("lock {field} source carries its recorded hash"),
            value: json!(expected),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
    for colour in lock["palette"].as_array().cloned().unwrap_or_default() {
        let source = colour["source_artifact"].as_str().unwrap_or("");
        let expected = colour["source_sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!(
                "cast:{character_id}:lock:palette:{}",
                colour["name"].as_str().unwrap_or("?")
            ),
            subject: character_id.to_string(),
            claim: "lock palette colour source carries its recorded hash".to_string(),
            value: json!(colour["hex"]),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
    let thumb = &lock["thumbnail_recognizability"];
    let source = thumb["source_artifact"].as_str().unwrap_or("");
    let expected = thumb["source_sha256"].as_str().unwrap_or("");
    let (ok, detail) = verify_source(project_dir, source, expected);
    b.check(Claim {
        id: format!("cast:{character_id}:lock:thumbnail"),
        subject: character_id.to_string(),
        claim: "thumbnail recognisability evidence carries its recorded hash".to_string(),
        value: json!({
            "width_px": thumb["width_px"],
            "height_px": thumb["height_px"],
            "recognisable": thumb["recognisable"],
        }),
        source_artifact: source.to_string(),
        source_sha256: Some(expected.to_string()),
        verified: ok,
        detail,
    });
    1
}

// ---------------------------------------------------------------
// File primitives
// ---------------------------------------------------------------

fn read_json_file(path: &Path) -> StoreResult<Option<Value>> {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(StoreError::io(format!(
                "cannot stat {}: {e}",
                path.display()
            )));
        }
    };
    if metadata.len() > MAX_RECORD_BYTES {
        return Err(StoreError::invalid_record(format!(
            "record {} exceeds the byte bound",
            path.display()
        )));
    }
    let text = fs::read_to_string(path)
        .map_err(|e| StoreError::io(format!("cannot read {}: {e}", path.display())))?;
    let value = serde_json::from_str(&text).map_err(|e| {
        StoreError::invalid_record(format!("record {} is not valid JSON: {e}", path.display()))
    })?;
    Ok(Some(value))
}

/// Write one record atomically: temp file in the same directory,
/// then rename. A crash leaves the old record or nothing, never
/// a torn record.
fn write_json_atomic(path: &Path, value: &Value) -> StoreResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| StoreError::io("record path has no parent"))?;
    fs::create_dir_all(parent)
        .map_err(|e| StoreError::io(format!("cannot create {}: {e}", parent.display())))?;
    let text = serde_json::to_string_pretty(value)
        .map_err(|e| StoreError::io(format!("cannot serialize record: {e}")))?;
    if text.len() as u64 > MAX_RECORD_BYTES {
        return Err(StoreError::invalid_record(
            "record exceeds the byte bound; refused before writing",
        ));
    }
    let temp = parent.join(format!(
        ".{}-{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("record"),
        std::process::id()
    ));
    fs::write(&temp, format!("{text}\n"))
        .map_err(|e| StoreError::io(format!("cannot write {}: {e}", temp.display())))?;
    fs::rename(&temp, path).map_err(|e| {
        let _ = fs::remove_file(&temp);
        StoreError::io(format!("cannot commit {}: {e}", path.display()))
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ABC_SHA: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn temp_store(name: &str) -> (Store, PathBuf) {
        let root =
            std::env::temp_dir().join(format!("meister-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        (Store::new(root.clone()), root)
    }

    fn envelope() -> Value {
        json!({"record_id": "rec-1", "origin_lane": "qompass",
               "content_label": "sfw", "evidence": "store test"})
    }

    fn source_file(root: &Path) -> PathBuf {
        let dir = root.join("fixtures");
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("source.txt");
        fs::write(&file, b"abc").unwrap();
        file
    }

    fn create_args() -> Value {
        json!({"envelope": envelope(), "name": "First Light", "engine": "bevy",
               "genre_pack": "puzzle"})
    }

    fn identity(source: &Path) -> Value {
        let p = source.display().to_string();
        json!({
            "display_name": "Warden",
            "role": "keeper of the first floor",
            "model_scheme": "single_model",
            "governing_references": [
                {"id": "sheet", "kind": "sprite_sheet", "path": p, "sha256": ABC_SHA,
                 "status": "canonical", "source": "store test fixture"}
            ],
            "appearance_references": [
                {"id": "sheet-view", "path": p, "sha256": ABC_SHA, "source": "fixture"}
            ],
            "rig_references": [
                {"id": "rig", "path": p, "sha256": ABC_SHA, "source": "fixture"}
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

    fn lock(source: &Path) -> Value {
        let p = source.display().to_string();
        let sourced = |description: &str| json!({"description": description, "source_artifact": p, "source_sha256": ABC_SHA});
        json!({
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
                "evidence": "store test fixture", "source_artifact": p,
                "source_sha256": ABC_SHA
            },
            "ruling": "D-1410 store test"
        })
    }

    fn scaffolded(name: &str) -> (Store, PathBuf, PathBuf) {
        let (store, root) = temp_store(name);
        let source = source_file(&root);
        store.create_project(&create_args()).unwrap();
        (store, root, source)
    }

    /// Every path under `root` with every file's bytes, sorted.
    /// Equal snapshots before and after a call mean the call
    /// wrote nothing.
    fn tree(root: &Path) -> Vec<(PathBuf, Option<Vec<u8>>)> {
        fn walk(dir: &Path, out: &mut Vec<(PathBuf, Option<Vec<u8>>)>) {
            let Ok(read) = fs::read_dir(dir) else {
                return;
            };
            for entry in read {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    out.push((path.clone(), None));
                    walk(&path, out);
                } else {
                    let bytes = fs::read(&path).unwrap();
                    out.push((path, Some(bytes)));
                }
            }
        }
        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    /// Hold the store's `flock` from a second open file
    /// description, as another process would.
    fn hold_lock(root: &Path) -> File {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(LOCK_FILE))
            .unwrap();
        file.try_lock().expect("the test takes the free lock");
        file
    }

    fn named_create(name: &str) -> Value {
        let mut args = create_args();
        args["name"] = json!(name);
        args
    }

    fn cast_args(source: &Path) -> Value {
        // The cast record carries its own envelope record_id:
        // STORE-03 holds record_ids unique per project, so a
        // fixture may not reuse the project's "rec-1".
        let mut args = json!({"envelope": envelope(), "project_id": "first-light",
               "character_id": "warden", "identity": identity(source)});
        args["envelope"]["record_id"] = json!("rec-cast-warden");
        args
    }

    fn lock_args(source: &Path) -> Value {
        json!({"envelope": envelope(), "project_id": "first-light",
               "character_id": "warden", "lock": lock(source)})
    }

    // ---------- validation ----------

    #[test]
    fn create_inspect_list_roundtrip() {
        let (store, root) = temp_store("roundtrip");
        let body = store.create_project(&create_args()).unwrap();
        assert_eq!(body["project_id"], "first-light");
        assert!(root.join("first-light/project.json").is_file());
        let inspected = store
            .inspect_project(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(inspected["genre_pack"], "puzzle");
        assert_eq!(inspected["cast_count"], 0);
        let listed = store.list_projects(&json!({})).unwrap();
        assert_eq!(listed["count"], 1);
        assert_eq!(listed["next_cursor"], Value::Null);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn list_paginates_by_cursor() {
        let (store, root) = temp_store("paging");
        for name in ["Alpha", "Beta", "Gamma"] {
            let mut args = create_args();
            args["name"] = json!(name);
            store.create_project(&args).unwrap();
        }
        let page1 = store.list_projects(&json!({"limit": 2})).unwrap();
        assert_eq!(page1["count"], 2);
        let cursor = page1["next_cursor"].as_str().unwrap().to_string();
        let page2 = store
            .list_projects(&json!({"limit": 2, "cursor": cursor}))
            .unwrap();
        assert_eq!(page2["count"], 1);
        assert_eq!(page2["projects"][0]["project_id"], "gamma");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn cast_add_lock_and_census_pass() {
        let (store, root, source) = scaffolded("census-pass");
        let added = store.add_cast(&cast_args(&source)).unwrap();
        assert_eq!(added["status"], "candidate");
        let locked = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": lock(&source)}))
            .unwrap();
        assert_eq!(locked["locked"], true);
        assert_eq!(locked["lock_hash"].as_str().unwrap().len(), 64);
        let census = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(census["unverified"], 0);
        assert_eq!(census["verdict"], "pass");
        assert_eq!(census["counts"]["cast_records"], 1);
        assert_eq!(census["counts"]["identity_locks"], 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn canon_fact_with_real_source_passes() {
        let (store, root) = temp_store("fact-pass");
        let source = source_file(&root);
        let mut args = create_args();
        args["canon"] = json!([{
            "id": "floors", "statement": "the descent has eighteen floors",
            "value": 18, "source_artifact": source.display().to_string(),
            "source_sha256": ABC_SHA
        }]);
        store.create_project(&args).unwrap();
        let census = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(census["counts"]["canon_facts"], 1);
        assert_eq!(census["unverified"], 0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn first_mutation_creates_an_empty_lock_file_that_is_not_a_project() {
        let (store, root) = temp_store("lock-file");
        store.create_project(&create_args()).unwrap();
        let lock_file = root.join(LOCK_FILE);
        assert!(lock_file.is_file());
        assert_eq!(fs::metadata(&lock_file).unwrap().len(), 0);
        let listed = store.list_projects(&json!({})).unwrap();
        assert_eq!(listed["count"], 1);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn lock_is_released_after_a_write_and_after_a_refusal() {
        let (mut store, root, source) = scaffolded("lock-release");
        // Success-path calls only: a production-scale bound, so a
        // parallel test's deliberate hold of the process-global
        // STORE_LOCK cannot starve them (see the held-lock test).
        store.lock_wait = Duration::from_secs(5);
        // After a committed write the next mutation takes the lock.
        store.add_cast(&cast_args(&source)).unwrap();
        // After a refusal under the lock, too.
        let dup = store.add_cast(&cast_args(&source)).unwrap_err();
        assert_eq!(dup.code, "conflict");
        store.lock_cast(&lock_args(&source)).unwrap();
        // And the lock is free for another holder afterwards.
        drop(hold_lock(&root));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn concurrent_threads_lose_no_writes() {
        let (store, root) = temp_store("lock-threads");
        std::thread::scope(|scope| {
            for t in 0..4 {
                let store = &store;
                scope.spawn(move || {
                    for i in 0..10 {
                        store
                            .create_project(&named_create(&format!("T{t} P{i}")))
                            .unwrap();
                    }
                });
            }
        });
        let listed = store.list_projects(&json!({"limit": 100})).unwrap();
        assert_eq!(listed["count"], 40);
        let leftovers = tree(&root)
            .iter()
            .filter(|(path, _)| path.extension().is_some_and(|e| e == "tmp"))
            .count();
        assert_eq!(leftovers, 0, "a temp file outlived its write");
        let _ = fs::remove_dir_all(&root);
    }

    /// A project with one locked character: a passing census.
    fn locked_project(name: &str) -> (Store, PathBuf, PathBuf) {
        let (store, root, source) = scaffolded(name);
        store.add_cast(&cast_args(&source)).unwrap();
        store.lock_cast(&lock_args(&source)).unwrap();
        (store, root, source)
    }

    fn census_args(prior: &str) -> Value {
        json!({"project_id": "first-light", "prior_state_hash": prior})
    }

    #[test]
    fn same_state_gives_the_same_hash_and_byte_identical_census_bodies() {
        let (store, root, _source) = locked_project("state-stable");
        let first = store.state_hash("first-light").unwrap();
        assert!(is_hex64(&first));
        assert_eq!(store.state_hash("first-light").unwrap(), first);
        let one = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        let two = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(one.to_string(), two.to_string());
        // The census names the state it ran against.
        assert_eq!(one["state_hash"], first.as_str());
        assert_eq!(one["stale"], false);
        assert_eq!(one["verdict"], "pass");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn census_against_the_current_state_hash_is_current() {
        let (store, root, _source) = locked_project("state-current");
        let hash = store.state_hash("first-light").unwrap();
        let census = store.canon_census(&census_args(&hash)).unwrap();
        assert_eq!(census["stale"], false);
        assert_eq!(census["state_hash"], hash.as_str());
        // An absent or null prior asks nothing about staleness.
        let census = store
            .canon_census(&json!({"project_id": "first-light", "prior_state_hash": null}))
            .unwrap();
        assert_eq!(census["stale"], false);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn state_hash_covers_every_record_and_nothing_else() {
        let (store, root, source) = scaffolded("state-cover");
        let bare = store.state_hash("first-light").unwrap();
        // Not records: the lock file, a temp file, a stray source.
        fs::write(root.join("first-light/notes.txt"), b"not a record").unwrap();
        fs::create_dir_all(root.join("first-light/cast")).unwrap();
        fs::write(root.join("first-light/cast/.warden.json-1.tmp"), b"{").unwrap();
        assert_eq!(store.state_hash("first-light").unwrap(), bare);
        fs::remove_file(root.join("first-light/cast/.warden.json-1.tmp")).unwrap();
        // A record: the hash moves.
        store.add_cast(&cast_args(&source)).unwrap();
        assert_ne!(store.state_hash("first-light").unwrap(), bare);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn census_is_a_read_it_writes_nothing() {
        let (store, root, _source) = locked_project("state-readonly");
        let before = tree(&root);
        store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(tree(&root), before);
        let _ = fs::remove_dir_all(&root);
    }

    // ---------- STORE-03: references and record ids ----------

    /// Read a stored record, mutate it, write it back — the
    /// behind-the-server's-back tamper the census exists to catch.
    fn tamper_cast(root: &Path, mutate: impl FnOnce(&mut Value)) {
        let file = root.join("first-light/cast/warden.json");
        let mut record: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        mutate(&mut record);
        fs::write(&file, record.to_string()).unwrap();
    }

    fn check_named<'v>(body: &'v Value, needle: &str) -> &'v Value {
        body["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"].as_str().unwrap().contains(needle))
            .unwrap_or_else(|| panic!("no check naming {needle}: {body}"))
    }

    #[test]
    fn census_reference_graph_resolves_on_a_locked_project() {
        let (store, root, _source) = locked_project("refs-pass");
        let census = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(census["verdict"], "pass", "census: {census}");
        // project + form governing + lock form + lock governing.
        assert_eq!(census["counts"]["references_checked"], 4);
        assert_eq!(census["counts"]["duplicate_record_ids"], 0);
        for needle in [
            "ref:project",
            "ref:form_governing",
            "ref:lock_form",
            "ref:lock_governing",
            ":record_id",
        ] {
            assert_eq!(
                check_named(&census, needle)["status"],
                "verified",
                "check {needle}: {census}"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_census_names_a_dangling_cast_project_reference() {
        let (store, root, _source) = locked_project("refs-dangling-project");
        tamper_cast(&root, |r| r["project_id"] = json!("elsewhere"));
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "canon_unverified");
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(check_named(&body, "ref:project")["status"], "unverified");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_census_names_a_dangling_form_governing_reference() {
        let (store, root, _source) = locked_project("refs-dangling-form");
        tamper_cast(&root, |r| {
            r["identity"]["forms"][0]["governing_reference_id"] = json!("missing")
        });
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(
            check_named(&body, "ref:form_governing")["status"],
            "unverified"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_census_names_a_dangling_lock_form() {
        let (store, root, _source) = locked_project("refs-dangling-lock");
        tamper_cast(&root, |r| r["identity_lock"]["form_id"] = json!("nope"));
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(check_named(&body, "ref:lock_form")["status"], "unverified");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_census_names_a_duplicate_record_id() {
        let (store, root, _source) = locked_project("refs-duplicate");
        tamper_cast(&root, |r| r["envelope"]["record_id"] = json!("rec-1"));
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(body["counts"]["duplicate_record_ids"], 1);
        // Both holders of the shared id are unverified.
        let dupes = body["checks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| {
                c["id"].as_str().unwrap().contains(":record_id") && c["status"] == "unverified"
            })
            .count();
        assert_eq!(dupes, 2, "census: {body}");
        assert!(
            body["failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f.as_str().unwrap().contains("record_id rec-1 is shared by")),
            "failures: {body}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_census_reports_orphan_and_duplicate_together() {
        let (store, root, _source) = locked_project("refs-both");
        tamper_cast(&root, |r| {
            r["project_id"] = json!("elsewhere");
            r["envelope"]["record_id"] = json!("rec-1");
        });
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(check_named(&body, "ref:project")["status"], "unverified");
        assert_eq!(body["counts"]["duplicate_record_ids"], 1);
        let _ = fs::remove_dir_all(&root);
    }

    // ---------- adversarial ----------

    #[test]
    fn adv_any_write_after_a_census_makes_it_stale() {
        let (store, root, source) = scaffolded("stale-writes");
        let census = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        let mut held = census["state_hash"].as_str().unwrap().to_string();
        // Each kind of write, in turn, strands the census held.
        let mut herald = cast_args(&source);
        herald["character_id"] = json!("herald");
        let mut herald_lock = lock_args(&source);
        herald_lock["character_id"] = json!("herald");
        let writes: [&dyn Fn(); 3] = [
            &|| {
                store.add_cast(&herald).unwrap();
            },
            &|| {
                store.lock_cast(&herald_lock).unwrap();
            },
            // A write behind the server's back counts too.
            &|| {
                let file = root.join("first-light/project.json");
                let mut record: Value =
                    serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
                record["name"] = json!("Second Light");
                fs::write(&file, record.to_string()).unwrap();
            },
        ];
        for write in writes {
            write();
            let before = tree(&root);
            let err = store.canon_census(&census_args(&held)).unwrap_err();
            assert_eq!(err.code, "census_stale");
            let body = err.census.expect("the census rides the refusal");
            assert_eq!(body["stale"], true);
            assert_eq!(body["verdict"], "review");
            assert_ne!(body["state_hash"], held.as_str());
            assert!(
                body["failures"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|f| f.as_str().unwrap().starts_with("census_stale: ")),
                "no named census_stale failure: {body}"
            );
            // The refusal is a read: the store is untouched.
            assert_eq!(tree(&root), before);
            held = body["state_hash"].as_str().unwrap().to_string();
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_reordered_records_hash_the_same_and_a_changed_value_does_not() {
        // Two stores, the same records written in opposite order.
        let (first, root_a, source) = scaffolded("reorder-a");
        let (second, root_b) = temp_store("reorder-b");
        second.create_project(&create_args()).unwrap();
        let mut herald = cast_args(&source);
        herald["character_id"] = json!("herald");
        first.add_cast(&cast_args(&source)).unwrap();
        first.add_cast(&herald).unwrap();
        second.add_cast(&herald).unwrap();
        second.add_cast(&cast_args(&source)).unwrap();
        let hash = first.state_hash("first-light").unwrap();
        assert_eq!(second.state_hash("first-light").unwrap(), hash);
        // The keys inside a record file, reversed and re-spaced.
        let file = root_a.join("first-light/cast/warden.json");
        let record: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        let reversed: Vec<String> = record
            .as_object()
            .unwrap()
            .iter()
            .rev()
            .map(|(key, value)| format!("  {}:\n {value}", json!(key)))
            .collect();
        fs::write(&file, format!("{{\n{}\n}}", reversed.join(",\n"))).unwrap();
        assert_eq!(first.state_hash("first-light").unwrap(), hash);
        // One changed value is a different state.
        let mut changed = record.clone();
        changed["identity"]["role"] = json!("keeper of the second floor");
        fs::write(&file, changed.to_string()).unwrap();
        assert_ne!(first.state_hash("first-light").unwrap(), hash);
        // So is the same record under another path.
        fs::write(&file, record.to_string()).unwrap();
        assert_eq!(first.state_hash("first-light").unwrap(), hash);
        fs::rename(&file, root_a.join("first-light/cast/moved.json")).unwrap();
        assert_ne!(first.state_hash("first-light").unwrap(), hash);
        let _ = fs::remove_dir_all(&root_a);
        let _ = fs::remove_dir_all(&root_b);
    }

    #[test]
    fn adv_stale_outranks_unverified_and_the_body_shows_both() {
        let (store, root, source) = scaffolded("stale-and-unverified");
        let held = store.state_hash("first-light").unwrap();
        // An unlocked character: an unverified claim, and a write.
        store.add_cast(&cast_args(&source)).unwrap();
        let err = store.canon_census(&census_args(&held)).unwrap_err();
        assert_eq!(err.code, "census_stale");
        let body = err.census.unwrap();
        assert_eq!(body["stale"], true);
        assert!(body["unverified"].as_u64().unwrap() > 0);
        // Asked without a prior, the same state is unverified, and
        // says it is current.
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "canon_unverified");
        assert_eq!(err.census.unwrap()["stale"], false);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_malformed_prior_state_hash_is_refused_not_ignored() {
        let (store, root, _source) = locked_project("stale-bad-prior");
        let upper = "A".repeat(64);
        for bad in [json!("abc"), json!(upper), json!(7), json!(["x"])] {
            let err = store
                .canon_census(&json!({"project_id": "first-light", "prior_state_hash": bad}))
                .unwrap_err();
            assert_eq!(err.code, "invalid_input", "{bad}");
            assert!(err.census.is_none());
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_record_that_cannot_be_read_fails_the_hash_and_the_census_closed() {
        let (store, root, _source) = locked_project("state-torn");
        fs::write(root.join("first-light/cast/herald.json"), b"{ torn").unwrap();
        let err = store.state_hash("first-light").unwrap_err();
        assert_eq!(err.code, "invalid_record");
        // No hash, no census: never a pass over a state it could
        // not read.
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_state_that_moves_mid_census_is_stale() {
        let (a, b) = ("a".repeat(64), "b".repeat(64));
        // Unmoved, with no prior or the matching prior: current.
        assert!(staleness(&a, &a, None).is_none());
        assert!(staleness(&a, &a, Some(&a)).is_none());
        // Moved while running: stale even when the prior matched
        // the state the census started on.
        assert!(staleness(&a, &b, None).is_some());
        assert!(staleness(&a, &b, Some(&a)).is_some());
        // Unmoved, but not the state the caller's census saw.
        assert!(staleness(&a, &a, Some(&b)).is_some());
    }

    #[test]
    fn adv_held_lock_refuses_every_mutation_store_busy_and_writes_nothing() {
        let (mut store, root, source) = scaffolded("lock-busy");
        store.add_cast(&cast_args(&source)).unwrap();
        store.lock_wait = Duration::from_millis(150);
        let holder = hold_lock(&root);
        let before = tree(&root);
        let started = Instant::now();
        let create = store.create_project(&named_create("Second")).unwrap_err();
        let waited = started.elapsed();
        assert_eq!(create.code, "store_busy");
        // The wait is the bound: not shorter, and not unbounded.
        assert!(waited >= Duration::from_millis(150), "waited {waited:?}");
        assert!(waited < Duration::from_secs(5), "waited {waited:?}");
        let mut other_cast = cast_args(&source);
        other_cast["character_id"] = json!("herald");
        assert_eq!(store.add_cast(&other_cast).unwrap_err().code, "store_busy");
        assert_eq!(
            store.lock_cast(&lock_args(&source)).unwrap_err().code,
            "store_busy"
        );
        assert_eq!(tree(&root), before, "a store_busy call wrote something");
        // Released, the same calls land. The success phase runs
        // with a production-scale bound: STORE_LOCK is process-
        // global, and a parallel test may be inside its own
        // deliberate hold (the mutex-bound test holds it for its
        // full 300 ms), which must not starve these calls.
        drop(holder);
        store.lock_wait = Duration::from_secs(5);
        store.create_project(&named_create("Second")).unwrap();
        store.lock_cast(&lock_args(&source)).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_refusal_on_a_fresh_root_creates_nothing_not_even_the_lock_file() {
        let (store, root) = temp_store("lock-fresh-refusal");
        let mut bad = create_args();
        bad["engine"] = json!("frostbite");
        assert_eq!(
            store.create_project(&bad).unwrap_err().code,
            "invalid_input"
        );
        let missing = store
            .add_cast(&json!({"envelope": envelope(), "project_id": "nope",
                              "character_id": "warden", "identity": json!({})}))
            .unwrap_err();
        assert_eq!(missing.code, "not_found");
        assert!(!root.exists(), "a refused call created the store root");
    }

    #[test]
    fn adv_refusals_on_a_store_without_a_lock_file_leave_it_byte_identical() {
        // A Rung 1 store predates the lock file. Every refusal —
        // input, record, and state conflicts alike — must leave it
        // exactly as found: no lock file appears.
        let (store, root, source) = scaffolded("lock-legacy");
        store.add_cast(&cast_args(&source)).unwrap();
        store.lock_cast(&lock_args(&source)).unwrap();
        fs::remove_file(root.join(LOCK_FILE)).unwrap();
        let before = tree(&root);
        assert_eq!(
            store.create_project(&create_args()).unwrap_err().code,
            "conflict"
        );
        assert_eq!(
            store.add_cast(&cast_args(&source)).unwrap_err().code,
            "conflict"
        );
        let mut changed = lock_args(&source);
        changed["lock"]["ruling"] = json!("a different ruling");
        assert_eq!(
            store.lock_cast(&changed).unwrap_err().code,
            "identity_lock_conflict"
        );
        let mut broken = cast_args(&source);
        broken["character_id"] = json!("herald");
        broken["identity"]["forms"] = json!([]);
        assert_eq!(store.add_cast(&broken).unwrap_err().code, "invalid_record");
        assert_eq!(tree(&root), before, "a refusal touched the store");
        assert!(!root.join(LOCK_FILE).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_lock_wait_covers_the_in_process_mutex_too() {
        // A thread parked on the flock holds the in-process mutex;
        // a second thread must give up at the same bound, not wait
        // behind it without limit.
        let (mut store, root, _source) = scaffolded("lock-mutex-bound");
        store.lock_wait = Duration::from_millis(300);
        let holder = hold_lock(&root);
        let started = Instant::now();
        std::thread::scope(|scope| {
            for name in ["Left", "Right"] {
                let store = &store;
                scope.spawn(move || {
                    let err = store.create_project(&named_create(name)).unwrap_err();
                    assert_eq!(err.code, "store_busy");
                });
            }
        });
        let waited = started.elapsed();
        assert!(waited >= Duration::from_millis(300), "waited {waited:?}");
        assert!(waited < Duration::from_secs(5), "waited {waited:?}");
        drop(holder);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn duplicate_project_and_traversal_ids_are_refused() {
        let (store, root) = temp_store("conflict");
        store.create_project(&create_args()).unwrap();
        let dup = store.create_project(&create_args()).unwrap_err();
        assert_eq!(dup.code, "conflict");
        let mut args = create_args();
        args["project_id"] = json!("../escape");
        let bad = store.create_project(&args).unwrap_err();
        assert_eq!(bad.code, "invalid_input");
        assert!(!root.join("escape").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn two_model_with_two_canonical_adults_is_refused() {
        let (store, root, source) = scaffolded("two-adults");
        let mut id = identity(&source);
        id["model_scheme"] = json!("two_model");
        id["forms"] = json!([
            {"form_id": "base", "kind": "base", "design_id": "d-base",
             "canon_status": "canonical", "governing_reference_id": "sheet"},
            {"form_id": "adult-a", "kind": "adult", "design_id": "d-a",
             "canon_status": "canonical", "governing_reference_id": "sheet"},
            {"form_id": "adult-b", "kind": "adult", "design_id": "d-b",
             "canon_status": "canonical", "governing_reference_id": "sheet"}
        ]);
        let err = store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": id}))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        assert!(
            !root.join("first-light/cast/warden.json").exists(),
            "a refused cast_add wrote a record"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn quarantined_form_without_ruling_is_refused() {
        let (store, root, source) = scaffolded("quarantine");
        let mut id = identity(&source);
        id["forms"] = json!([
            {"form_id": "sheet", "kind": "stage_sheet", "design_id": "warden-ns1",
             "canon_status": "canonical", "governing_reference_id": "sheet"},
            {"form_id": "alt", "kind": "stage_sheet", "design_id": "warden-alt",
             "canon_status": "quarantined", "governing_reference_id": "sheet"}
        ]);
        // single_model with a second canonical-kind form is fine as
        // long as it is quarantined — but it carries no quarantine
        // record, so the refusal must land.
        let err = store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": id}))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn incomplete_or_mismatched_lock_is_refused() {
        let (store, root, source) = scaffolded("lock-refusals");
        store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": identity(&source)}))
            .unwrap();
        let mut no_thumb = lock(&source);
        no_thumb
            .as_object_mut()
            .unwrap()
            .remove("thumbnail_recognizability");
        let err = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": no_thumb}))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let mut wrong_ref = lock(&source);
        wrong_ref["governing_reference_id"] = json!("other");
        let err = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": wrong_ref}))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn conflicting_relock_is_refused_identical_relock_stands() {
        let (store, root, source) = scaffolded("relock");
        store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": identity(&source)}))
            .unwrap();
        let first = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": lock(&source)}))
            .unwrap();
        let again = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": lock(&source)}))
            .unwrap();
        assert_eq!(first["lock_hash"], again["lock_hash"]);
        let mut changed = lock(&source);
        changed["ruling"] = json!("a different ruling");
        let err = store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": changed}))
            .unwrap_err();
        assert_eq!(err.code, "identity_lock_conflict");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn tampered_source_fails_the_census_closed() {
        let (store, root, source) = scaffolded("tamper");
        store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": identity(&source)}))
            .unwrap();
        store
            .lock_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                               "character_id": "warden", "lock": lock(&source)}))
            .unwrap();
        fs::write(&source, b"tampered").unwrap();
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "canon_unverified");
        let census = err.census.expect("census body rides the refusal");
        assert!(census["unverified"].as_u64().unwrap() > 0);
        assert_eq!(census["verdict"], "review");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unlocked_cast_fails_the_census() {
        let (store, root, source) = scaffolded("unlocked");
        store
            .add_cast(&json!({"envelope": envelope(), "project_id": "first-light",
                              "character_id": "warden", "identity": identity(&source)}))
            .unwrap();
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "canon_unverified");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn missing_project_is_not_found_not_a_crash() {
        let (store, root) = temp_store("missing");
        let err = store
            .inspect_project(&json!({"project_id": "nope"}))
            .unwrap_err();
        assert_eq!(err.code, "not_found");
        let err = store
            .add_cast(&json!({"envelope": envelope(), "project_id": "nope",
                              "character_id": "warden", "identity": json!({})}))
            .unwrap_err();
        assert_eq!(err.code, "not_found");
        let _ = fs::remove_dir_all(&root);
    }
}
