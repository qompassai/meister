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

use crate::graph::{self, GraphError};

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
const RECORD_DIRS: &[&str] = &["cast", "systems", "scenes", "levels", "narrative"];
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

/// Validate a record-level `unlock` list (FRZ-03): every entry
/// names a declared flag. Undeclared -> `unlock_undeclared`.
fn validate_unlock(args: &Value, flags: &BTreeSet<String>) -> StoreResult<Vec<String>> {
    let mut out = Vec::new();
    if let Some(list) = args.get("unlock") {
        let arr = list
            .as_array()
            .ok_or_else(|| StoreError::invalid_record("unlock must be an array of flag ids"))?;
        for entry in arr {
            let flag = entry.as_str().ok_or_else(|| {
                StoreError::invalid_record("unlock entries must be flag id strings")
            })?;
            if !flags.contains(flag) {
                return Err(StoreError::new(
                    "unlock_undeclared",
                    format!("unlock condition names undeclared flag {flag}"),
                ));
            }
            out.push(flag.to_string());
        }
    }
    Ok(out)
}

/// Validate dialogue lines (NAR-01): structured lines whose
/// speakers resolve to cast, whose conditions and continuity
/// effects name declared flags, and whose choices and
/// prerequisites resolve inside the record.
fn validate_dialogue_lines(
    store: &Store,
    project_id: &str,
    lines: &[Value],
    flags: &BTreeSet<String>,
) -> StoreResult<()> {
    if lines.is_empty() {
        return Err(StoreError::invalid_record(
            "a dialogue record needs at least one line",
        ));
    }
    if lines.len() > 2048 {
        return Err(StoreError::new(
            "bound_exceeded",
            format!("dialogue has {} lines; the bound is 2048", lines.len()),
        ));
    }
    let mut ids: Vec<String> = Vec::new();
    for line in lines {
        if !line.is_object() {
            return Err(StoreError::invalid_record(
                "dialogue lines must be objects (structured data, never loose prose)",
            ));
        }
        let id = req_text(line, "id", MAX_ID_CHARS)?;
        if ids.contains(&id) {
            return Err(StoreError::invalid_record(format!(
                "duplicate dialogue line id {id}"
            )));
        }
        ids.push(id);
    }
    for line in lines {
        let speaker = req_text(line, "speaker", MAX_ID_CHARS)?;
        if !store.cast_file(project_id, &speaker).exists() {
            return Err(StoreError::new(
                "speaker_unknown",
                format!("speaker {speaker} is not a cast character of this project"),
            ));
        }
        let text = line
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::invalid_record("dialogue line text must be a string"))?;
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(StoreError::new(
                "bound_exceeded",
                "dialogue line text exceeds 500 chars",
            ));
        }
        if text.trim().is_empty() {
            return Err(StoreError::invalid_record(
                "dialogue line text must not be empty",
            ));
        }
        if let Some(choices) = line.get("choices") {
            let choices = choices
                .as_array()
                .ok_or_else(|| StoreError::invalid_record("line choices must be an array"))?;
            if choices.len() > 16 {
                return Err(StoreError::new(
                    "bound_exceeded",
                    "a dialogue line has more than 16 choices",
                ));
            }
            for choice in choices {
                req_text(choice, "label", MAX_NAME_CHARS)?;
                let target = req_text(choice, "target", MAX_ID_CHARS)?;
                if !ids.contains(&target) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("choice target {target} is not a line of this dialogue"),
                    ));
                }
            }
        }
        for key in ["conditions", "effects"] {
            if let Some(list) = line.get(key) {
                let list = list.as_array().ok_or_else(|| {
                    StoreError::invalid_record(format!("line {key} must be an array"))
                })?;
                for entry in list {
                    let flag = req_text(entry, "flag", MAX_ID_CHARS)?;
                    if !flags.contains(&flag) {
                        return Err(StoreError::new(
                            "flag_undeclared",
                            format!("flag {flag} is not declared"),
                        ));
                    }
                }
            }
        }
        if let Some(prereqs) = line.get("prerequisites") {
            let prereqs = prereqs
                .as_array()
                .ok_or_else(|| StoreError::invalid_record("line prerequisites must be an array"))?;
            for prereq in prereqs {
                let target = prereq.as_str().ok_or_else(|| {
                    StoreError::invalid_record("prerequisites must be line id strings")
                })?;
                if !ids.iter().any(|i| i == target) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("prerequisite {target} is not a line of this dialogue"),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Validate story beats (NAR-03): references resolve to the
/// project's scenes, dialogues, and progression nodes; quest
/// objectives name declared flags. An ending no beat leads to
/// is NOT refused here — playtest reports it (NAR-06).
fn validate_story_beats(
    store: &Store,
    project_id: &str,
    beats: &[Value],
    flags: &BTreeSet<String>,
) -> StoreResult<()> {
    if beats.is_empty() {
        return Err(StoreError::invalid_record(
            "a story arc needs at least one beat",
        ));
    }
    if beats.len() > crate::graph::MAX_NODES {
        return Err(StoreError::new(
            "bound_exceeded",
            format!("story has {} beats; the bound is 512", beats.len()),
        ));
    }
    let mut ids: Vec<String> = Vec::new();
    for beat in beats {
        if !beat.is_object() {
            return Err(StoreError::invalid_record("story beats must be objects"));
        }
        let id = req_text(beat, "id", MAX_ID_CHARS)?;
        if ids.contains(&id) {
            return Err(StoreError::invalid_record(format!(
                "duplicate story beat id {id}"
            )));
        }
        ids.push(id);
    }
    let scene_ids: Vec<String> = store
        .read_family_records(project_id, "scenes")?
        .into_iter()
        .filter_map(|(_, r)| {
            r.get("scene_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let dialogue_ids: Vec<String> = store
        .read_family_records(project_id, "narrative")?
        .into_iter()
        .filter(|(_, r)| r.get("record_type").and_then(Value::as_str) == Some("meister_dialogue"))
        .filter_map(|(_, r)| {
            r.get("dialogue_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let mut progression_nodes: Vec<String> = Vec::new();
    for (_, record) in store.read_family_records(project_id, "systems")? {
        if record.get("kind").and_then(Value::as_str) == Some("progression_dag")
            && let Some(nodes) = record
                .get("spec")
                .and_then(|s| s.get("nodes"))
                .and_then(Value::as_array)
        {
            for node in nodes {
                if let Some(id) = node.get("id").and_then(Value::as_str) {
                    progression_nodes.push(id.to_string());
                }
            }
        }
    }
    for beat in beats {
        if let Some(choices) = beat.get("choices") {
            let choices = choices
                .as_array()
                .ok_or_else(|| StoreError::invalid_record("beat choices must be an array"))?;
            for choice in choices {
                let target = req_text(choice, "target", MAX_ID_CHARS)?;
                if !ids.contains(&target) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("beat choice target {target} is not a beat of this arc"),
                    ));
                }
            }
        }
        if let Some(refs) = beat.get("refs") {
            for (key, known, what) in [
                ("scenes", &scene_ids, "scene"),
                ("dialogues", &dialogue_ids, "dialogue"),
                ("progression", &progression_nodes, "progression node"),
            ] {
                if let Some(list) = refs.get(key) {
                    let list = list.as_array().ok_or_else(|| {
                        StoreError::invalid_record(format!("beat refs {key} must be an array"))
                    })?;
                    for entry in list {
                        let target = entry.as_str().ok_or_else(|| {
                            StoreError::invalid_record("beat refs must be id strings")
                        })?;
                        if !known.iter().any(|k| k == target) {
                            return Err(StoreError::new(
                                "graph_dangling_ref",
                                format!("beat references undeclared {what} {target}"),
                            ));
                        }
                    }
                }
            }
        }
        if let Some(quests) = beat.get("quests") {
            let quests = quests
                .as_array()
                .ok_or_else(|| StoreError::invalid_record("beat quests must be an array"))?;
            for quest in quests {
                req_text(quest, "id", MAX_ID_CHARS)?;
                req_text(quest, "title", MAX_NAME_CHARS)?;
                if let Some(flag) = quest.get("objective_flag") {
                    let flag = flag.as_str().ok_or_else(|| {
                        StoreError::invalid_record("quest objective_flag must be a string")
                    })?;
                    if !flags.contains(flag) {
                        return Err(StoreError::new(
                            "flag_undeclared",
                            format!("flag {flag} is not declared"),
                        ));
                    }
                }
            }
        }
    }
    Ok(())
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

    fn scene_file(&self, project_id: &str, scene_id: &str) -> PathBuf {
        self.project_dir(project_id)
            .join("scenes")
            .join(format!("{scene_id}.json"))
    }

    fn level_file(&self, project_id: &str, level_id: &str) -> PathBuf {
        self.project_dir(project_id)
            .join("levels")
            .join(format!("{level_id}.json"))
    }

    fn narrative_file(&self, project_id: &str, name: &str) -> PathBuf {
        self.project_dir(project_id)
            .join("narrative")
            .join(format!("{name}.json"))
    }

    fn system_file(&self, project_id: &str, system_id: &str) -> PathBuf {
        self.project_dir(project_id)
            .join("systems")
            .join(format!("{system_id}.json"))
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
    /// `meister_system_define`: author one game-systems record
    /// (SYS-02..07). The spec is validated per kind before
    /// anything is written; the record is born `candidate` in
    /// the project's `systems/` family. Redefinition is a
    /// conflict, never an overwrite.
    pub fn define_system(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_define_system(args))
    }

    fn plan_define_system(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let system_id = arg_id(args, "system_id")?;
        self.read_project(&project_id)?;
        let kind = req_text(args, "kind", MAX_ID_CHARS)?;
        if !SYSTEM_KINDS.contains(&kind.as_str()) {
            return Err(StoreError::invalid(format!("unknown system kind {kind}")));
        }
        let spec = args
            .get("spec")
            .filter(|v| v.is_object())
            .ok_or_else(|| StoreError::invalid_record("spec must be an object"))?;
        let flags = self.declared_flags(&project_id)?;
        validate_system_spec(&kind, spec, &flags)?;
        let file = self.system_file(&project_id, &system_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("system {system_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let record = json!({
            "record_type": "meister_system",
            "schema_version": 1,
            "project_id": project_id,
            "system_id": system_id,
            "kind": kind,
            "status": "candidate",
            "envelope": envelope,
            "spec": spec,
        });
        let body = json!({
            "project_id": project_id,
            "system_id": system_id,
            "kind": kind,
            "status": "candidate",
            "record_path": file.display().to_string(),
        });
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// The project's system records as (relative path, record),
    /// sorted by path: the census's view of the `systems/` family.
    fn read_system_records(&self, project_id: &str) -> StoreResult<Vec<(String, Value)>> {
        self.read_family_records(project_id, "systems")
    }

    /// One record family's records as (relative path, record),
    /// sorted by path.
    fn read_family_records(
        &self,
        project_id: &str,
        family: &str,
    ) -> StoreResult<Vec<(String, Value)>> {
        let dir = self.project_dir(project_id).join(family);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(StoreError::io(format!("cannot list {family}: {e}")));
            }
        };
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|e| StoreError::io(format!("{family} entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if records.len() >= MAX_CAST_PER_PROJECT {
                return Err(StoreError::io(
                    "a record family exceeds the per-project bound",
                ));
            }
            if let Some(record) = read_json_file(&path)? {
                let name = entry.file_name().to_string_lossy().into_owned();
                records.push((format!("{family}/{name}"), record));
            }
        }
        records.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(records)
    }

    /// Every flag id declared by the project's `flags` systems:
    /// the namespace state-machine guards and schedule events
    /// must name (SYS-03, SYS-06).
    fn declared_flags(&self, project_id: &str) -> StoreResult<BTreeSet<String>> {
        let mut flags = BTreeSet::new();
        for (_, record) in self.read_system_records(project_id)? {
            if record["kind"].as_str() != Some("flags") {
                continue;
            }
            if let Some(list) = record["spec"]["flags"].as_array() {
                for flag in list {
                    if let Some(id) = flag["id"].as_str() {
                        flags.insert(id.to_string());
                    }
                }
            }
        }
        Ok(flags)
    }

    /// `meister_scene_define` (SYS-08): author one scene graph
    /// record into the project's `scenes/` family, born
    /// `candidate`. Define checks the graph's shape (nodes with
    /// unique valid ids, edge/link endpoints as strings); the
    /// structural verdict is `meister_scene_validate`'s job.
    pub fn define_scene(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_define_scene(args))
    }

    fn plan_define_scene(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let scene_id = arg_id(args, "scene_id")?;
        self.read_project(&project_id)?;
        let graph = args
            .get("graph")
            .filter(|v| v.is_object())
            .ok_or_else(|| StoreError::invalid_record("graph must be an object"))?;
        scene_graph_shape(graph)?;
        let file = self.scene_file(&project_id, &scene_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("scene {scene_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let record = json!({
            "record_type": "meister_scene",
            "schema_version": 1,
            "project_id": project_id,
            "scene_id": scene_id,
            "status": "candidate",
            "envelope": envelope,
            "graph": graph,
        });
        let body = json!({
            "project_id": project_id,
            "scene_id": scene_id,
            "status": "candidate",
            "record_path": file.display().to_string(),
        });
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// `meister_scene_validate` (SYS-08): the structural verdict
    /// over a stored scene — SYS-01 checks plus orphan checks,
    /// returned as `{valid, refusals[]}` with one typed code per
    /// refusal. A read: it writes nothing.
    pub fn validate_scene(&self, args: &Value) -> StoreResult<Value> {
        let project_id = arg_id(args, "project_id")?;
        let scene_id = arg_id(args, "scene_id")?;
        self.read_project(&project_id)?;
        let file = self.scene_file(&project_id, &scene_id);
        let record = read_json_file(&file)?.ok_or_else(|| {
            StoreError::new(
                "not_found",
                format!("scene {scene_id} not found in {project_id}"),
            )
        })?;
        let refusals = scene_refusals(&record["graph"]);
        Ok(json!({
            "scene_id": scene_id,
            "valid": refusals.is_empty(),
            "refusals": refusals,
        }))
    }

    /// `meister_level_compose` (SYS-09), structural half: author
    /// one level record into the project's `levels/` family,
    /// born `candidate`. The layout must satisfy the descent law
    /// (D-1659, transcribed in `validate_descent_layout`);
    /// narrative fields refuse as `not_implemented_until_rung3`
    /// — Rung 3 owns them.
    pub fn compose_level(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_compose_level(args))
    }

    fn plan_compose_level(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let level_id = arg_id(args, "level_id")?;
        self.read_project(&project_id)?;
        let layout = args
            .get("layout")
            .filter(|v| v.is_object())
            .ok_or_else(|| StoreError::invalid_record("layout must be an object"))?;
        let triggers = match args.get("triggers") {
            None => Vec::new(),
            Some(value) => value
                .as_array()
                .cloned()
                .ok_or_else(|| StoreError::invalid_record("triggers must be an array"))?,
        };
        let mut dag_ids: BTreeSet<String> = BTreeSet::new();
        let mut minigame_ids: BTreeSet<String> = BTreeSet::new();
        for (_, record) in self.read_system_records(&project_id)? {
            match record["kind"].as_str() {
                Some("progression_dag") => {
                    if let Some(id) = record["system_id"].as_str() {
                        dag_ids.insert(id.to_string());
                    }
                }
                Some("minigame") => {
                    if let Some(id) = record["system_id"].as_str() {
                        minigame_ids.insert(id.to_string());
                    }
                }
                _ => {}
            }
        }
        let floor_count = validate_descent_layout(layout, &dag_ids, &minigame_ids)?;
        validate_level_narrative(self, &project_id, layout, &triggers)?;
        let file = self.level_file(&project_id, &level_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("level {level_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let level_flags = self.declared_flags(&project_id)?;
        let unlock = validate_unlock(args, &level_flags)?;
        let record = json!({
            "record_type": "meister_level",
            "schema_version": 1,
            "project_id": project_id,
            "level_id": level_id,
            "status": "candidate",
            "unit_state": "ungated",
            "unlock": unlock,
            "envelope": envelope,
            "layout": layout,
            "triggers": triggers,
        });
        let body = json!({
            "project_id": project_id,
            "level_id": level_id,
            "status": "candidate",
            "floor_count": floor_count,
            "record_path": file.display().to_string(),
        });
        Ok(Planned {
            write: Some((file, record)),
            body,
        })
    }

    /// `meister_dialogue_author` (NAR-01).
    pub fn author_dialogue(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_author_dialogue(args))
    }

    fn plan_author_dialogue(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let dialogue_id = arg_id(args, "dialogue_id")?;
        self.read_project(&project_id)?;
        let lines = args
            .get("lines")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                StoreError::invalid_record("lines must be an array of structured line objects")
            })?
            .clone();
        let flags = self.declared_flags(&project_id)?;
        validate_dialogue_lines(self, &project_id, &lines, &flags)?;
        let unlock = validate_unlock(args, &flags)?;
        let file = self.narrative_file(&project_id, &dialogue_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("dialogue {dialogue_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let line_count = lines.len();
        let record = json!({
            "record_type": "meister_dialogue",
            "schema_version": 1,
            "project_id": project_id,
            "dialogue_id": dialogue_id,
            "envelope": envelope,
            "status": "candidate",
            "unit_state": "ungated",
            "unlock": unlock,
            "lines": lines,
        });
        Ok(Planned {
            write: Some((file, record)),
            body: json!({"dialogue_id": dialogue_id, "line_count": line_count}),
        })
    }

    /// `meister_dialogue_get` (NAR-01): read one dialogue back.
    pub fn get_dialogue(&self, args: &Value) -> StoreResult<Value> {
        let project_id = arg_id(args, "project_id")?;
        let dialogue_id = arg_id(args, "dialogue_id")?;
        self.read_project(&project_id)?;
        let file = self.narrative_file(&project_id, &dialogue_id);
        let raw = fs::read_to_string(&file).map_err(|_| {
            StoreError::new(
                "not_found",
                format!("dialogue {dialogue_id} not found in {project_id}"),
            )
        })?;
        let record: Value = serde_json::from_str(&raw)
            .map_err(|e| StoreError::invalid_record(format!("dialogue record: {e}")))?;
        if record.get("record_type").and_then(Value::as_str) != Some("meister_dialogue") {
            return Err(StoreError::new(
                "not_found",
                format!("{dialogue_id} is not a dialogue record"),
            ));
        }
        Ok(json!({
            "dialogue_id": dialogue_id,
            "lines": record["lines"].clone(),
        }))
    }

    /// `meister_reaction_set` (NAR-02): whole-body reaction
    /// detail for one emotion of one character. The character
    /// must be identity-locked first (D-1410).
    pub fn set_reaction(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_set_reaction(args))
    }

    fn plan_set_reaction(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let character_id = arg_id(args, "character_id")?;
        let emotion = arg_id(args, "emotion")?;
        let posture = req_text(args, "posture", MAX_TEXT_CHARS)?;
        let gesture = req_text(args, "gesture", MAX_TEXT_CHARS)?;
        let stance = req_text(args, "stance", MAX_TEXT_CHARS)?;
        self.read_project(&project_id)?;
        let cast_file = self.cast_file(&project_id, &character_id);
        let raw = fs::read_to_string(&cast_file).map_err(|_| {
            StoreError::new(
                "not_found",
                format!("character {character_id} not found in {project_id}"),
            )
        })?;
        let cast: Value = serde_json::from_str(&raw)
            .map_err(|e| StoreError::invalid_record(format!("cast record: {e}")))?;
        if cast.get("identity_lock").is_none_or(Value::is_null) {
            return Err(StoreError::new(
                "identity_unlocked",
                format!(
                    "character {character_id} is not identity-locked;                      reaction detail work waits for the lock (D-1410)"
                ),
            ));
        }
        let name = format!("reaction-{character_id}-{emotion}");
        let file = self.narrative_file(&project_id, &name);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("reaction {emotion} for {character_id} already exists"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let record = json!({
            "record_type": "meister_reaction",
            "schema_version": 1,
            "project_id": project_id,
            "character_id": character_id,
            "emotion": emotion,
            "envelope": envelope,
            "posture": posture,
            "gesture": gesture,
            "stance": stance,
            "conditions": args.get("conditions").cloned().unwrap_or(json!({})),
            "status": "candidate",
        });
        Ok(Planned {
            write: Some((file, record)),
            body: json!({
                "character_id": character_id,
                "emotion": emotion,
                "status": "candidate",
            }),
        })
    }

    /// `meister_story_define` (NAR-03).
    pub fn define_story(&self, args: &Value) -> StoreResult<Value> {
        self.mutate(|| self.plan_define_story(args))
    }

    fn plan_define_story(&self, args: &Value) -> StoreResult<Planned> {
        let project_id = arg_id(args, "project_id")?;
        let arc_id = arg_id(args, "arc_id")?;
        self.read_project(&project_id)?;
        let beats = args
            .get("beats")
            .and_then(Value::as_array)
            .ok_or_else(|| StoreError::invalid_record("beats must be an array"))?
            .clone();
        let flags = self.declared_flags(&project_id)?;
        validate_story_beats(self, &project_id, &beats, &flags)?;
        let unlock = validate_unlock(args, &flags)?;
        let file = self.narrative_file(&project_id, &arc_id);
        if file.exists() {
            return Err(StoreError::new(
                "conflict",
                format!("story arc {arc_id} already exists in {project_id}"),
            ));
        }
        let envelope = args
            .get("envelope")
            .cloned()
            .ok_or_else(|| StoreError::invalid("envelope is required"))?;
        let beat_count = beats.len();
        let record = json!({
            "record_type": "meister_story_arc",
            "schema_version": 1,
            "project_id": project_id,
            "arc_id": arc_id,
            "envelope": envelope,
            "status": "candidate",
            "unit_state": "ungated",
            "unlock": unlock,
            "beats": beats,
        });
        Ok(Planned {
            write: Some((file, record)),
            body: json!({"arc_id": arc_id, "beat_count": beat_count}),
        })
    }

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
        let systems = store.read_system_records(project_id)?;
        let mut census_extra = systems.clone();
        for family in ["scenes", "levels", "narrative"] {
            census_extra.extend(store.read_family_records(project_id, family)?);
        }
        let (references, duplicate_ids) =
            census_references(&mut b, store, project_id, &project, &cast, &census_extra);
        census_systems(&mut b, &project_dir, &systems);
        let (sanitization_hits, differentiation) =
            census_narrative_checks(&mut b, store, project_id, &project_dir, &cast)?;
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
                "system_records": systems.len(),
                "sanitization_hits": sanitization_hits,
            },
            "differentiation": differentiation,
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

/// NAR-05: sanitization and differentiation, as census claims
/// and reports.
///
/// Sanitization: when the project carries a `denylist.json`
/// record (`{terms: [...]}` — the teaching games' law: no
/// studio name, no real project, no real person in shipped
/// text), every shippable text field of the narrative records
/// is scanned: dialogue line text and choice labels, story
/// beat titles and quest titles. A hit is an unverified claim
/// locating the field; the text is never silently edited.
///
/// Differentiation: for each cast pair, a shared display name
/// is a collision outright (an unverified claim); descriptor
/// overlap is reported as a Jaccard number in the body — the
/// machine reports, the Owner decides.
fn census_narrative_checks(
    b: &mut CensusBuilder,
    store: &Store,
    project_id: &str,
    project_dir: &Path,
    cast: &[Value],
) -> StoreResult<(usize, Vec<Value>)> {
    let mut hits = 0usize;
    let denylist_path = project_dir.join("denylist.json");
    if denylist_path.exists() {
        let raw = fs::read_to_string(&denylist_path)
            .map_err(|e| StoreError::io(format!("denylist unreadable: {e}")))?;
        let parsed: Value = serde_json::from_str(&raw).map_err(|e| {
            StoreError::invalid_record(format!("denylist record does not parse: {e}"))
        })?;
        let terms: Vec<String> = parsed
            .get("terms")
            .and_then(Value::as_array)
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(|t| t.to_lowercase()))
                    .collect()
            })
            .unwrap_or_default();
        if !terms.is_empty() {
            for (rel, record) in store.read_family_records(project_id, "narrative")? {
                let mut fields: Vec<(String, String)> = Vec::new();
                if record.get("record_type").and_then(Value::as_str) == Some("meister_dialogue")
                    && let Some(lines) = record.get("lines").and_then(Value::as_array)
                {
                    for (i, line) in lines.iter().enumerate() {
                        if let Some(text) = line.get("text").and_then(Value::as_str) {
                            fields.push((format!("lines[{i}].text"), text.to_string()));
                        }
                        if let Some(choices) = line.get("choices").and_then(Value::as_array) {
                            for (j, choice) in choices.iter().enumerate() {
                                if let Some(label) = choice.get("label").and_then(Value::as_str) {
                                    fields.push((
                                        format!("lines[{i}].choices[{j}].label"),
                                        label.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                if record.get("record_type").and_then(Value::as_str) == Some("meister_story_arc")
                    && let Some(beats) = record.get("beats").and_then(Value::as_array)
                {
                    for (i, beat) in beats.iter().enumerate() {
                        if let Some(title) = beat.get("title").and_then(Value::as_str) {
                            fields.push((format!("beats[{i}].title"), title.to_string()));
                        }
                        if let Some(quests) = beat.get("quests").and_then(Value::as_array) {
                            for (j, quest) in quests.iter().enumerate() {
                                if let Some(title) = quest.get("title").and_then(Value::as_str) {
                                    fields.push((
                                        format!("beats[{i}].quests[{j}].title"),
                                        title.to_string(),
                                    ));
                                }
                            }
                        }
                    }
                }
                for (field, text) in &fields {
                    let lowered = text.to_lowercase();
                    for term in &terms {
                        if lowered.contains(term.as_str()) {
                            hits += 1;
                            b.check(Claim {
                                id: format!("sanitize:{rel}:{field}"),
                                subject: rel.clone(),
                                claim: "sanitization_hit: shipped text carries no denylisted term"
                                    .to_string(),
                                value: json!(term),
                                source_artifact: "denylist.json".to_string(),
                                source_sha256: None,
                                verified: false,
                                detail: format!(
                                    "denylisted term {term:?} found at {field} of {rel}"
                                ),
                            });
                        }
                    }
                }
            }
        }
    }
    // Differentiation over cast pairs.
    let descriptors = |record: &Value| -> Vec<String> {
        let identity = &record["identity"];
        if let Some(list) = identity.get("descriptors").and_then(Value::as_array) {
            return list
                .iter()
                .filter_map(|d| d.as_str().map(|d| d.to_lowercase()))
                .collect();
        }
        let mut tokens: Vec<String> = Vec::new();
        for key in ["display_name", "role"] {
            if let Some(text) = identity.get(key).and_then(Value::as_str) {
                for token in text.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
                    if !token.is_empty() {
                        tokens.push(token.to_string());
                    }
                }
            }
        }
        tokens.sort();
        tokens.dedup();
        tokens
    };
    let mut pairs: Vec<Value> = Vec::new();
    for i in 0..cast.len() {
        for j in (i + 1)..cast.len() {
            let (a, b_rec) = (&cast[i], &cast[j]);
            let a_id = a["character_id"].as_str().unwrap_or_default().to_string();
            let b_id = b_rec["character_id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            let a_name = a["identity"]["display_name"]
                .as_str()
                .unwrap_or(&a_id)
                .to_lowercase();
            let b_name = b_rec["identity"]["display_name"]
                .as_str()
                .unwrap_or(&b_id)
                .to_lowercase();
            let collision = a_name == b_name;
            if collision {
                b.check(Claim {
                    id: format!("differentiation:{a_id}:{b_id}"),
                    subject: a_id.clone(),
                    claim: "differentiation_collision: cast display names are distinct".to_string(),
                    value: json!(a_name),
                    source_artifact: format!("cast/{b_id}.json"),
                    source_sha256: None,
                    verified: false,
                    detail: format!(
                        "cast members {a_id} and {b_id} share the display name {a_name:?}"
                    ),
                });
            }
            let da = descriptors(a);
            let db = descriptors(b_rec);
            let set_a: std::collections::BTreeSet<&String> = da.iter().collect();
            let set_b: std::collections::BTreeSet<&String> = db.iter().collect();
            let shared = set_a.intersection(&set_b).count();
            let union = set_a.union(&set_b).count();
            let overlap = if union == 0 {
                0.0
            } else {
                (shared as f64 / union as f64 * 1000.0).round() / 1000.0
            };
            pairs.push(json!({
                "a": a_id,
                "b": b_id,
                "name_collision": collision,
                "descriptor_overlap": overlap,
            }));
        }
    }
    Ok((hits, pairs))
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
    extra: &[(String, Value)],
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
    for (rel, record) in extra {
        let id = record["envelope"]["record_id"].as_str().map(str::to_string);
        if let Some(id) = &id {
            holders.entry(id.clone()).or_default().push(rel.clone());
        }
        records.push((rel.clone(), id));
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

/// The six system kinds (SYS-02..07), matching the catalog's
/// `meister_system_define` kind enum.
const SYSTEM_KINDS: &[&str] = &[
    "schedule",
    "flags",
    "progression_dag",
    "economy",
    "state_machine",
    "minigame",
];

const MAX_FLAGS: usize = 1_024;
const MAX_STATES: usize = 128;
const MAX_TRANSITIONS: usize = 1_024;
const MAX_EVENTS: usize = 1_024;
const MAX_ECONOMY_ENTRIES: usize = 512;
const MAX_FLOWS: usize = 1_024;
const MAX_MINIGAME_INPUTS: usize = 64;

/// A graph refusal as a store refusal, code intact.
fn graph_guard(result: Result<(), GraphError>) -> StoreResult<()> {
    result.map_err(|e| StoreError::new(e.code(), e.message()))
}

/// An integer field of a spec object. Floats are refused as
/// floats: JSON numbers that are not integers never coerce.
fn spec_int(obj: &Value, field: &str, what: &str) -> StoreResult<i64> {
    match obj.get(field) {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| {
            StoreError::invalid_record(format!("{what} {field} must be an integer, not a float"))
        }),
        Some(_) => Err(StoreError::invalid_record(format!(
            "{what} {field} must be an integer"
        ))),
        None => Err(StoreError::invalid_record(format!(
            "{what} needs an integer {field}"
        ))),
    }
}

/// Validate one system spec against its kind's contract. `flags`
/// is the project's declared-flag namespace (SYS-02 records),
/// which guards and schedule events must name.
fn validate_system_spec(kind: &str, spec: &Value, flags: &BTreeSet<String>) -> StoreResult<()> {
    match kind {
        "flags" => validate_flags_spec(spec),
        "state_machine" => validate_state_machine_spec(spec, flags),
        "progression_dag" => validate_dag_spec(spec),
        "economy" => validate_economy_spec(spec),
        "schedule" => validate_schedule_spec(spec, flags),
        "minigame" => validate_minigame_spec(spec),
        _ => Err(StoreError::invalid(format!("unknown system kind {kind}"))),
    }
}

/// SYS-02 — declared flags: `id`, `type` in {bool, int},
/// `initial`, and integer bounds for `int` flags.
fn validate_flags_spec(spec: &Value) -> StoreResult<()> {
    let list = req_arr(spec, "flags", MAX_FLAGS)?;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for flag in list {
        let id = valid_id(&req_text(flag, "id", MAX_ID_CHARS)?)?;
        if !seen.insert(id.clone()) {
            return Err(StoreError::new(
                "flag_duplicate",
                format!("flag {id} is declared twice"),
            ));
        }
        match req_text(flag, "type", MAX_ID_CHARS)?.as_str() {
            "bool" => {
                if !flag["initial"].is_boolean() {
                    return Err(StoreError::invalid_record(format!(
                        "bool flag {id} needs a boolean initial"
                    )));
                }
            }
            "int" => {
                let what = format!("int flag {id}");
                let min = spec_int(flag, "min", &what)?;
                let max = spec_int(flag, "max", &what)?;
                let initial = spec_int(flag, "initial", &what)?;
                if min > max {
                    return Err(StoreError::invalid_record(format!(
                        "int flag {id} has min {min} above max {max}"
                    )));
                }
                if initial < min || initial > max {
                    return Err(StoreError::invalid_record(format!(
                        "int flag {id} initial {initial} is outside [{min}, {max}]"
                    )));
                }
            }
            other => {
                return Err(StoreError::invalid_record(format!(
                    "flag {id} has unknown type {other}"
                )));
            }
        }
    }
    Ok(())
}

/// SYS-03 — a state machine: declared states, an initial state,
/// terminal states, and transitions whose endpoints are declared
/// and whose guards name declared flags. Every non-terminal
/// state must be reachable from the initial state.
fn validate_state_machine_spec(spec: &Value, flags: &BTreeSet<String>) -> StoreResult<()> {
    let raw_states = req_arr(spec, "states", MAX_STATES)?;
    let mut states: Vec<String> = Vec::new();
    for state in raw_states {
        let raw = state
            .as_str()
            .ok_or_else(|| StoreError::invalid_record("states must be id strings"))?;
        let id = valid_id(raw)?;
        if states.contains(&id) {
            return Err(StoreError::invalid_record(format!(
                "state {id} is declared twice"
            )));
        }
        states.push(id);
    }
    if states.is_empty() {
        return Err(StoreError::invalid_record(
            "a state machine declares at least one state",
        ));
    }
    let initial = valid_id(&req_text(spec, "initial", MAX_ID_CHARS)?)?;
    if !states.contains(&initial) {
        return Err(StoreError::new(
            "state_undeclared",
            format!("initial state {initial} is not a declared state"),
        ));
    }
    let mut terminal: BTreeSet<String> = BTreeSet::new();
    if let Some(list) = spec.get("terminal") {
        let list = list
            .as_array()
            .ok_or_else(|| StoreError::invalid_record("terminal must be an array of state ids"))?;
        for state in list {
            let raw = state
                .as_str()
                .ok_or_else(|| StoreError::invalid_record("terminal must be id strings"))?;
            let id = valid_id(raw)?;
            if !states.contains(&id) {
                return Err(StoreError::new(
                    "state_undeclared",
                    format!("terminal state {id} is not a declared state"),
                ));
            }
            terminal.insert(id);
        }
    }
    let transitions = req_arr(spec, "transitions", MAX_TRANSITIONS)?;
    let mut edges: Vec<(String, String)> = Vec::new();
    for transition in transitions {
        let from = valid_id(&req_text(transition, "from", MAX_ID_CHARS)?)?;
        let to = valid_id(&req_text(transition, "to", MAX_ID_CHARS)?)?;
        for (role, id) in [("from", &from), ("to", &to)] {
            if !states.contains(id) {
                return Err(StoreError::new(
                    "state_undeclared",
                    format!("transition {role} names undeclared state {id}"),
                ));
            }
        }
        if let Some(guard) = transition.get("guard") {
            let guard = guard.as_str().ok_or_else(|| {
                StoreError::invalid_record("a transition guard must be a flag id string")
            })?;
            if !flags.contains(guard) {
                return Err(StoreError::new(
                    "flag_undeclared",
                    format!("transition guard names undeclared flag {guard}"),
                ));
            }
        }
        edges.push((from, to));
    }
    let roots = std::slice::from_ref(&initial);
    graph_guard(graph::validate(&states, &edges, roots, false, false))?;
    let seen = graph::reachable(&states, &edges, roots);
    let lost: Vec<&str> = states
        .iter()
        .filter(|s| !seen.contains(*s) && !terminal.contains(*s))
        .map(String::as_str)
        .collect();
    if !lost.is_empty() {
        return Err(StoreError::new(
            "graph_unreachable",
            format!("states unreachable from {initial}: {}", lost.join(", ")),
        ));
    }
    Ok(())
}

/// SYS-04 — a progression DAG: unlock nodes with prerequisites.
/// Prerequisites must be declared, the graph must be acyclic,
/// and every node must be reachable from a root (declared via
/// `roots`, or the prerequisite-free nodes when undeclared).
fn validate_dag_spec(spec: &Value) -> StoreResult<()> {
    let list = req_arr(spec, "nodes", graph::MAX_NODES)?;
    let mut ids: Vec<String> = Vec::new();
    for node in list {
        let id = valid_id(&req_text(node, "id", MAX_ID_CHARS)?)?;
        if ids.contains(&id) {
            return Err(StoreError::invalid_record(format!(
                "node {id} is declared twice"
            )));
        }
        ids.push(id);
    }
    // Edges point prerequisite -> dependent, the direction in
    // which reachability flows.
    let mut edges: Vec<(String, String)> = Vec::new();
    for node in list {
        let id = req_text(node, "id", MAX_ID_CHARS)?;
        if let Some(requires) = node.get("requires") {
            let requires = requires.as_array().ok_or_else(|| {
                StoreError::invalid_record("requires must be an array of node ids")
            })?;
            for req in requires {
                let req = req.as_str().ok_or_else(|| {
                    StoreError::invalid_record("requires entries must be node id strings")
                })?;
                edges.push((req.to_string(), id.clone()));
            }
        }
    }
    let roots: Vec<String> = match spec.get("roots") {
        Some(list) => {
            let list = list
                .as_array()
                .ok_or_else(|| StoreError::invalid_record("roots must be an array of node ids"))?;
            let mut roots = Vec::new();
            for root in list {
                let raw = root.as_str().ok_or_else(|| {
                    StoreError::invalid_record("roots entries must be node id strings")
                })?;
                roots.push(valid_id(raw)?);
            }
            roots
        }
        None => ids
            .iter()
            .filter(|id| !edges.iter().any(|(_, to)| to == *id))
            .cloned()
            .collect(),
    };
    graph_guard(graph::validate(&ids, &edges, &roots, true, true))
}

/// An economy amount or bound (SYS-05). A float is its own
/// refusal: the combat law is integers and booleans, no floats.
fn economy_int(obj: &Value, field: &str, what: &str) -> StoreResult<i64> {
    match obj.get(field) {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| {
            StoreError::new(
                "economy_float",
                format!("{what} {field} must be an integer; floats are refused"),
            )
        }),
        Some(_) => Err(StoreError::invalid_record(format!(
            "{what} {field} must be an integer"
        ))),
        None => Err(StoreError::invalid_record(format!(
            "{what} needs an integer {field}"
        ))),
    }
}

/// A declared economy bound: absent is `economy_unbounded`.
fn economy_bound(obj: &Value, field: &str, what: &str) -> StoreResult<i64> {
    match obj.get(field) {
        None => Err(StoreError::new(
            "economy_unbounded",
            format!("{what} declares no {field} bound; economy values are bounded integers"),
        )),
        Some(Value::Number(n)) => n.as_i64().ok_or_else(|| {
            StoreError::new(
                "economy_float",
                format!("{what} {field} must be an integer; floats are refused"),
            )
        }),
        Some(_) => Err(StoreError::invalid_record(format!(
            "{what} {field} must be an integer"
        ))),
    }
}

/// SYS-05 — an economy: currencies and resources with declared
/// integer bounds, and sources/sinks flowing declared ids in
/// integer amounts. Integers only.
fn validate_economy_spec(spec: &Value) -> StoreResult<()> {
    let mut declared: BTreeSet<String> = BTreeSet::new();
    for family in ["currencies", "resources"] {
        if let Some(list) = spec.get(family) {
            let list = list
                .as_array()
                .ok_or_else(|| StoreError::invalid_record(format!("{family} must be an array")))?;
            if list.len() > MAX_ECONOMY_ENTRIES {
                return Err(StoreError::new(
                    "bound_exceeded",
                    format!("{family} exceeds the bound of {MAX_ECONOMY_ENTRIES}"),
                ));
            }
            for entry in list {
                let id = valid_id(&req_text(entry, "id", MAX_ID_CHARS)?)?;
                if !declared.insert(id.clone()) {
                    return Err(StoreError::invalid_record(format!(
                        "economy id {id} is declared twice"
                    )));
                }
                let what = format!("economy entry {id}");
                let min = economy_bound(entry, "min", &what)?;
                let max = economy_bound(entry, "max", &what)?;
                if min > max {
                    return Err(StoreError::invalid_record(format!(
                        "economy entry {id} has min {min} above max {max}"
                    )));
                }
            }
        }
    }
    if declared.is_empty() {
        return Err(StoreError::invalid_record(
            "an economy declares at least one currency or resource",
        ));
    }
    for family in ["sources", "sinks"] {
        if let Some(list) = spec.get(family) {
            let list = list
                .as_array()
                .ok_or_else(|| StoreError::invalid_record(format!("{family} must be an array")))?;
            if list.len() > MAX_FLOWS {
                return Err(StoreError::new(
                    "bound_exceeded",
                    format!("{family} exceeds the bound of {MAX_FLOWS}"),
                ));
            }
            for flow in list {
                let target = req_text(flow, "currency", MAX_ID_CHARS)?;
                if !declared.contains(&target) {
                    return Err(StoreError::invalid_record(format!(
                        "{family} entry names undeclared currency {target}"
                    )));
                }
                economy_int(flow, "amount", &format!("{family} entry for {target}"))?;
            }
        }
    }
    Ok(())
}

/// SYS-06 — a schedule: time-ordered events over integer ticks.
/// Each event names a declared flag condition and an effect;
/// exclusive events may not share a tick.
fn validate_schedule_spec(spec: &Value, flags: &BTreeSet<String>) -> StoreResult<()> {
    let events = req_arr(spec, "events", MAX_EVENTS)?;
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut last_tick: Option<i64> = None;
    let mut exclusive: BTreeMap<i64, String> = BTreeMap::new();
    for event in events {
        let id = valid_id(&req_text(event, "id", MAX_ID_CHARS)?)?;
        if !seen.insert(id.clone()) {
            return Err(StoreError::invalid_record(format!(
                "event {id} is declared twice"
            )));
        }
        let tick = spec_int(event, "tick", &format!("event {id}"))?;
        if tick < 0 {
            return Err(StoreError::invalid_record(format!(
                "event {id} has negative tick {tick}"
            )));
        }
        if let Some(last) = last_tick
            && tick < last
        {
            return Err(StoreError::invalid_record(format!(
                "events must be time-ordered: event {id} at tick {tick} follows tick {last}"
            )));
        }
        last_tick = Some(tick);
        let flag = req_text(event, "flag", MAX_ID_CHARS)?;
        if !flags.contains(&flag) {
            return Err(StoreError::new(
                "flag_undeclared",
                format!("event {id} names undeclared flag {flag}"),
            ));
        }
        req_text(event, "effect", MAX_TEXT_CHARS)?;
        if event.get("exclusive").and_then(Value::as_bool) == Some(true)
            && let Some(other) = exclusive.insert(tick, id.clone())
        {
            return Err(StoreError::invalid_record(format!(
                "exclusive events {other} and {id} overlap at tick {tick}"
            )));
        }
    }
    Ok(())
}

/// SYS-07 — a minigame: rules, inputs, and a judge named as an
/// artifact path plus SHA-256. Meister never runs the judge;
/// the census verifies the reference. The path is relative and
/// may not climb out of the project.
fn validate_minigame_spec(spec: &Value) -> StoreResult<()> {
    if spec.get("rules").is_none_or(Value::is_null) {
        return Err(StoreError::invalid_record("a minigame names its rules"));
    }
    if let Some(inputs) = spec.get("inputs") {
        let inputs = inputs
            .as_array()
            .ok_or_else(|| StoreError::invalid_record("minigame inputs must be an array"))?;
        if inputs.len() > MAX_MINIGAME_INPUTS {
            return Err(StoreError::new(
                "bound_exceeded",
                format!("minigame inputs exceed the bound of {MAX_MINIGAME_INPUTS}"),
            ));
        }
    }
    let judge = spec.get("judge").filter(|v| v.is_object()).ok_or_else(|| {
        StoreError::invalid_record("a minigame names its judge as {path, sha256}")
    })?;
    let path = judge
        .get("path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.chars().count() <= MAX_PATH_CHARS)
        .ok_or_else(|| StoreError::invalid_record("judge path must be a non-empty string"))?;
    let path_ref = Path::new(path);
    if path_ref.is_absolute()
        || path_ref
            .components()
            .any(|c| c == std::path::Component::ParentDir)
    {
        return Err(StoreError::invalid_record(format!(
            "judge path {path} must be a relative path inside the project"
        )));
    }
    let _sha = judge
        .get("sha256")
        .and_then(Value::as_str)
        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| StoreError::invalid_record("judge sha256 must be 64 hex characters"))?;
    Ok(())
}

/// SYS-07 — the census covers minigame judge references: the
/// judge artifact must carry the hash the record declares.
fn census_systems(b: &mut CensusBuilder, project_dir: &Path, systems: &[(String, Value)]) {
    for (_, record) in systems {
        if record["kind"].as_str() != Some("minigame") {
            continue;
        }
        let system_id = record["system_id"].as_str().unwrap_or("?");
        let judge = &record["spec"]["judge"];
        let source = judge["path"].as_str().unwrap_or("");
        let expected = judge["sha256"].as_str().unwrap_or("");
        let (ok, detail) = verify_source(project_dir, source, expected);
        b.check(Claim {
            id: format!("system:{system_id}:judge"),
            subject: system_id.to_string(),
            claim: "minigame judge artifact carries its recorded hash".to_string(),
            value: json!(expected),
            source_artifact: source.to_string(),
            source_sha256: Some(expected.to_string()),
            verified: ok,
            detail,
        });
    }
}

/// The (from, to) pairs of one scene-graph edge list (`edges`
/// or `links`); a missing list is empty, a malformed entry is
/// a shape refusal.
fn scene_pairs(graph: &Value, field: &str) -> StoreResult<Vec<(String, String)>> {
    let Some(list) = graph.get(field) else {
        return Ok(Vec::new());
    };
    let list = list.as_array().ok_or_else(|| {
        StoreError::invalid_record(format!("scene graph {field} must be an array"))
    })?;
    let mut pairs = Vec::new();
    for entry in list {
        let from = entry["from"].as_str().ok_or_else(|| {
            StoreError::invalid_record(format!(
                "scene graph {field} entries name string from/to ids"
            ))
        })?;
        let to = entry["to"].as_str().ok_or_else(|| {
            StoreError::invalid_record(format!(
                "scene graph {field} entries name string from/to ids"
            ))
        })?;
        pairs.push((from.to_string(), to.to_string()));
    }
    Ok(pairs)
}

/// SYS-08, define-time shape: nodes carry unique valid ids;
/// edge and link lists are well-formed pairs. Structural truth
/// (dangling refs, cycles) is the validator's verdict, not the
/// define's — a scene may be authored before it is sound.
fn scene_graph_shape(graph: &Value) -> StoreResult<()> {
    let nodes = graph["nodes"]
        .as_array()
        .ok_or_else(|| StoreError::invalid_record("scene graph nodes must be an array"))?;
    if nodes.len() > graph::MAX_NODES {
        return Err(StoreError::new(
            "bound_exceeded",
            format!(
                "scene graph has {} nodes, over the bound of {}",
                nodes.len(),
                graph::MAX_NODES
            ),
        ));
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for node in nodes {
        let raw = node["id"]
            .as_str()
            .ok_or_else(|| StoreError::invalid_record("scene graph nodes carry string ids"))?;
        let id = valid_id(raw)?;
        if !seen.insert(id.clone()) {
            return Err(StoreError::invalid_record(format!(
                "scene node {id} is declared twice"
            )));
        }
    }
    let total = scene_pairs(graph, "edges")?.len() + scene_pairs(graph, "links")?.len();
    if total > graph::MAX_EDGES {
        return Err(StoreError::new(
            "bound_exceeded",
            format!(
                "scene graph has {total} edges and links, over the bound of {}",
                graph::MAX_EDGES
            ),
        ));
    }
    Ok(())
}

/// SYS-08, the validator: SYS-01 checks plus orphan checks over
/// a stored scene graph, as a list of typed refusal codes (one
/// per refusal). Bounds first, then dangling references (edges
/// and dialogue-link placeholders alike), then the cycle check
/// a scene that declares itself acyclic must pass.
fn scene_refusals(graph: &Value) -> Vec<String> {
    let refusal = |code: &str| vec![code.to_string()];
    if !graph.is_object() {
        return refusal("invalid_record");
    }
    let Some(nodes) = graph["nodes"].as_array() else {
        return refusal("invalid_record");
    };
    let mut ids: Vec<String> = Vec::new();
    for node in nodes {
        match node["id"].as_str() {
            Some(id) => ids.push(id.to_string()),
            None => return refusal("invalid_record"),
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for id in &ids {
        if !seen.insert(id.as_str()) {
            return refusal("invalid_record");
        }
    }
    let (Ok(edges), Ok(links)) = (scene_pairs(graph, "edges"), scene_pairs(graph, "links")) else {
        return refusal("invalid_record");
    };
    if ids.len() > graph::MAX_NODES || edges.len() + links.len() > graph::MAX_EDGES {
        return refusal("bound_exceeded");
    }
    let declared: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut refusals: Vec<String> = Vec::new();
    for (from, to) in edges.iter().chain(links.iter()) {
        for end in [from, to] {
            if !declared.contains(end.as_str()) {
                refusals.push("graph_dangling_ref".to_string());
            }
        }
    }
    if graph["acyclic"].as_bool() == Some(true)
        && let Err(e) = graph::validate(&ids, &edges, &[], true, false)
    {
        refusals.push(e.code().to_string());
    }
    refusals
}

const MAX_FLOORS: usize = 64;
const MAX_CHAMBERS_PER_FLOOR: usize = 64;
const MAX_DOORS_PER_FLOOR: usize = 128;
const MAX_ENTRY_POINTS_PER_FLOOR: usize = 128;

/// One descent-law refusal, naming the offending id.
fn descent_violation(message: String) -> StoreError {
    StoreError::new("descent_violation", message)
}

/// SYS-09 — the descent law, transcribed from the Owner's
/// D-1659 ruling text (Rung 1 gate pack,
/// source-notes/descent-d1659.md), not from the plan's
/// paraphrase. The ruling says:
///
/// - "Each floor is a dungeon/cave level with doors/entry
///   points" and "doors and entry points are first-class
///   architecture, not decoration" → every floor declares at
///   least one entry point, and every door resolves on both
///   sides (both chambers it joins are declared chambers of
///   the same floor).
/// - "Cave/dungeon chambers are the floor form" → every floor
///   declares at least one chamber.
/// - "the player descends and ascends between floors" →
///   movement between floors is two-way: each adjacent pair
///   of floors is joined by a stairs edge whose direction is
///   `both`.
/// - "Niflheim is about the descent" as one standing
///   structure of floors in order → the floors form an
///   unbroken chain: stairs edges pair adjacent floors only,
///   and every adjacent pair is paired (no floor is skipped).
///
/// Bounds (floors ≤ 64, chambers ≤ 64 per floor, doors ≤ 128
/// per floor) and the `descent_violation` code are the plan's
/// NAR-04: the level's narrative half. Trigger `dialogue`
/// fields name authored dialogues; `layout.narrative` composes
/// chambers with scene and dialogue records; `layout.encounters`
/// places encounters in declared chambers against declared
/// systems. Every reference resolves or the compose refuses.
fn validate_level_narrative(
    store: &Store,
    project_id: &str,
    layout: &Value,
    triggers: &[Value],
) -> StoreResult<()> {
    let dialogue_ids: Vec<String> = store
        .read_family_records(project_id, "narrative")?
        .into_iter()
        .filter(|(_, r)| r.get("record_type").and_then(Value::as_str) == Some("meister_dialogue"))
        .filter_map(|(_, r)| {
            r.get("dialogue_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let scene_ids: Vec<String> = store
        .read_family_records(project_id, "scenes")?
        .into_iter()
        .filter_map(|(_, r)| {
            r.get("scene_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    let system_ids: Vec<String> = store
        .read_system_records(project_id)?
        .into_iter()
        .filter_map(|(_, r)| {
            r.get("system_id")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .collect();
    // floor_id -> chamber ids, from the layout itself.
    let mut chambers_of: Vec<(String, Vec<String>)> = Vec::new();
    if let Some(floors) = layout.get("floors").and_then(Value::as_array) {
        for floor in floors {
            let floor_id = floor
                .get("floor_id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let chambers = floor
                .get("chambers")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|c| c.get("id").and_then(Value::as_str).map(str::to_string))
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();
            chambers_of.push((floor_id, chambers));
        }
    }
    let chamber_known = |floor_id: &str, chamber: &str| -> bool {
        chambers_of
            .iter()
            .any(|(f, cs)| f == floor_id && cs.iter().any(|c| c == chamber))
    };
    for trigger in triggers {
        if let Some(d) = trigger.get("dialogue").filter(|v| !v.is_null()) {
            let d = d.as_str().ok_or_else(|| {
                StoreError::invalid_record("trigger dialogue must be a dialogue id string")
            })?;
            if !dialogue_ids.iter().any(|i| i == d) {
                return Err(StoreError::new(
                    "graph_dangling_ref",
                    format!("trigger names undeclared dialogue {d}"),
                ));
            }
        }
    }
    if let Some(narrative) = layout.get("narrative").filter(|v| !v.is_null()) {
        let entries = narrative.as_array().ok_or_else(|| {
            StoreError::invalid_record("layout narrative must be an array of chamber compositions")
        })?;
        for entry in entries {
            let floor_id = req_text(entry, "floor_id", MAX_ID_CHARS)?;
            let chamber = req_text(entry, "chamber", MAX_ID_CHARS)?;
            if !chamber_known(&floor_id, &chamber) {
                return Err(StoreError::new(
                    "descent_violation",
                    format!(
                        "narrative composition names chamber {chamber}                          of undeclared floor {floor_id}"
                    ),
                ));
            }
            if let Some(s) = entry.get("scene_id") {
                let s = s.as_str().ok_or_else(|| {
                    StoreError::invalid_record("composition scene_id must be a string")
                })?;
                if !scene_ids.iter().any(|i| i == s) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("composition names undeclared scene {s}"),
                    ));
                }
            }
            if let Some(d) = entry.get("dialogue_id") {
                let d = d.as_str().ok_or_else(|| {
                    StoreError::invalid_record("composition dialogue_id must be a string")
                })?;
                if !dialogue_ids.iter().any(|i| i == d) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("composition names undeclared dialogue {d}"),
                    ));
                }
            }
        }
    }
    if let Some(encounters) = layout.get("encounters").filter(|v| !v.is_null()) {
        let encounters = encounters
            .as_array()
            .ok_or_else(|| StoreError::invalid_record("layout encounters must be an array"))?;
        for encounter in encounters {
            let id = req_text(encounter, "id", MAX_ID_CHARS)?;
            let floor_id = req_text(encounter, "floor_id", MAX_ID_CHARS)?;
            let chamber = req_text(encounter, "chamber", MAX_ID_CHARS)?;
            if !chamber_known(&floor_id, &chamber) {
                return Err(StoreError::new(
                    "descent_violation",
                    format!(
                        "encounter {id} placed in chamber {chamber}                          of undeclared floor {floor_id}"
                    ),
                ));
            }
            if let Some(s) = encounter.get("system_id") {
                let s = s.as_str().ok_or_else(|| {
                    StoreError::invalid_record("encounter system_id must be a string")
                })?;
                if !system_ids.iter().any(|i| i == s) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("encounter {id} names undeclared system {s}"),
                    ));
                }
            }
        }
    }
    Ok(())
}

/// SYS-09 contract. Progression gates must name declared
/// progression_dag systems and minigame placements declared
/// minigame systems (the plan's SYS-09 references).
///
/// Returns the floor count.
fn validate_descent_layout(
    layout: &Value,
    dag_ids: &BTreeSet<String>,
    minigame_ids: &BTreeSet<String>,
) -> StoreResult<usize> {
    let floors = layout["floors"]
        .as_array()
        .ok_or_else(|| StoreError::invalid_record("a level layout declares its floors"))?;
    if floors.len() > MAX_FLOORS {
        return Err(StoreError::new(
            "bound_exceeded",
            format!("{} floors exceeds the bound of {MAX_FLOORS}", floors.len()),
        ));
    }
    if floors.is_empty() {
        return Err(descent_violation(
            "a level declares at least one floor".to_string(),
        ));
    }
    let mut floor_ids: Vec<String> = Vec::new();
    for floor in floors {
        let floor_id = valid_id(&req_text(floor, "floor_id", MAX_ID_CHARS)?)?;
        if floor_ids.contains(&floor_id) {
            return Err(StoreError::invalid_record(format!(
                "floor {floor_id} is declared twice"
            )));
        }
        let chambers = floor["chambers"].as_array().ok_or_else(|| {
            StoreError::invalid_record(format!("floor {floor_id} declares its chambers"))
        })?;
        if chambers.len() > MAX_CHAMBERS_PER_FLOOR {
            return Err(StoreError::new(
                "bound_exceeded",
                format!(
                    "floor {floor_id} has {} chambers, over the bound of {MAX_CHAMBERS_PER_FLOOR}",
                    chambers.len()
                ),
            ));
        }
        if chambers.is_empty() {
            return Err(descent_violation(format!(
                "floor {floor_id} declares no chambers: cave/dungeon chambers are the floor form (D-1659)"
            )));
        }
        let mut chamber_ids: BTreeSet<String> = BTreeSet::new();
        for chamber in chambers {
            let raw = chamber["id"]
                .as_str()
                .ok_or_else(|| StoreError::invalid_record("chambers carry string ids"))?;
            chamber_ids.insert(valid_id(raw)?);
        }
        let entry_points = floor["entry_points"].as_array().ok_or_else(|| {
            StoreError::invalid_record(format!("floor {floor_id} declares its entry points"))
        })?;
        if entry_points.len() > MAX_ENTRY_POINTS_PER_FLOOR {
            return Err(StoreError::new(
                "bound_exceeded",
                format!(
                    "floor {floor_id} has {} entry points, over the bound of {MAX_ENTRY_POINTS_PER_FLOOR}",
                    entry_points.len()
                ),
            ));
        }
        if entry_points.is_empty() {
            return Err(descent_violation(format!(
                "floor {floor_id} declares no entry point: doors and entry points are first-class architecture (D-1659)"
            )));
        }
        if let Some(doors) = floor.get("doors") {
            let doors = doors.as_array().ok_or_else(|| {
                StoreError::invalid_record(format!("floor {floor_id} doors must be an array"))
            })?;
            if doors.len() > MAX_DOORS_PER_FLOOR {
                return Err(StoreError::new(
                    "bound_exceeded",
                    format!(
                        "floor {floor_id} has {} doors, over the bound of {MAX_DOORS_PER_FLOOR}",
                        doors.len()
                    ),
                ));
            }
            for door in doors {
                let door_id = req_text(door, "id", MAX_ID_CHARS)?;
                let between = door["between"].as_array().ok_or_else(|| {
                    StoreError::invalid_record(format!(
                        "door {door_id} names the two chambers it joins"
                    ))
                })?;
                let sides: Vec<&str> = between.iter().filter_map(Value::as_str).collect();
                if sides.len() != 2 || !sides.iter().all(|s| chamber_ids.contains(*s)) {
                    return Err(descent_violation(format!(
                        "door {door_id} on floor {floor_id} does not resolve on both sides"
                    )));
                }
            }
        }
        floor_ids.push(floor_id);
    }
    // Stairs edges pair the floors into the descent's chain.
    let stairs: &[Value] = match layout.get("stairs") {
        Some(value) => value
            .as_array()
            .ok_or_else(|| StoreError::invalid_record("stairs must be an array of edges"))?,
        None => &[],
    };
    let mut paired: BTreeSet<(usize, usize)> = BTreeSet::new();
    for edge in stairs {
        let edge_id = req_text(edge, "id", MAX_ID_CHARS)?;
        let from = req_text(edge, "from_floor", MAX_ID_CHARS)?;
        let to = req_text(edge, "to_floor", MAX_ID_CHARS)?;
        let (Some(from_idx), Some(to_idx)) = (
            floor_ids.iter().position(|f| f == &from),
            floor_ids.iter().position(|f| f == &to),
        ) else {
            return Err(descent_violation(format!(
                "stairs edge {edge_id} names an undeclared floor"
            )));
        };
        if to_idx != from_idx + 1 {
            return Err(descent_violation(format!(
                "stairs edge {edge_id} does not pair adjacent floors in descent order: no floor is skipped"
            )));
        }
        if edge["direction"].as_str() != Some("both") {
            return Err(descent_violation(format!(
                "stairs edge {edge_id} is not two-way: the player descends and ascends between floors (D-1659)"
            )));
        }
        paired.insert((from_idx, to_idx));
    }
    for i in 0..floor_ids.len() - 1 {
        if !paired.contains(&(i, i + 1)) {
            return Err(descent_violation(format!(
                "no stairs edge pairs floor {} with floor {}",
                floor_ids[i],
                floor_ids[i + 1]
            )));
        }
    }
    // Progression gates and minigame placements reference
    // declared systems.
    for (field, declared, what) in [
        ("gates", dag_ids, "progression gate"),
        ("minigames", minigame_ids, "minigame placement"),
    ] {
        if let Some(list) = layout.get(field) {
            let list = list
                .as_array()
                .ok_or_else(|| StoreError::invalid_record(format!("{field} must be an array")))?;
            for entry in list {
                let entry_id = req_text(entry, "id", MAX_ID_CHARS)?;
                let system_id = req_text(entry, "system_id", MAX_ID_CHARS)?;
                if !declared.contains(&system_id) {
                    return Err(StoreError::new(
                        "graph_dangling_ref",
                        format!("{what} {entry_id} names undeclared system {system_id}"),
                    ));
                }
            }
        }
    }
    Ok(floors.len())
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

    // ---------- SYS-02..07: system_define per kind ----------

    fn system_args(system_id: &str, kind: &str, spec: Value) -> Value {
        let mut args = json!({"envelope": envelope(), "project_id": "first-light",
               "system_id": system_id, "kind": kind, "spec": spec});
        args["envelope"]["record_id"] = json!(format!("rec-{system_id}"));
        args
    }

    fn flags_spec() -> Value {
        json!({"flags": [
            {"id": "met-warden", "type": "bool", "initial": false},
            {"id": "depth", "type": "int", "initial": 1, "min": 0, "max": 12}
        ]})
    }

    fn with_flags(name: &str) -> (Store, PathBuf, PathBuf) {
        let (store, root, source) = scaffolded(name);
        store
            .define_system(&system_args("core-flags", "flags", flags_spec()))
            .unwrap();
        (store, root, source)
    }

    #[test]
    fn flags_round_trip_and_redefine_conflicts() {
        let (store, root, _source) = scaffolded("sys-flags");
        let body = store
            .define_system(&system_args("core-flags", "flags", flags_spec()))
            .unwrap();
        assert_eq!(body["status"], "candidate");
        assert_eq!(body["kind"], "flags");
        assert!(root.join("first-light/systems/core-flags.json").is_file());
        let err = store
            .define_system(&system_args("core-flags", "flags", flags_spec()))
            .unwrap_err();
        assert_eq!(err.code, "conflict");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_duplicate_flag_is_flag_duplicate() {
        let (store, root, _source) = scaffolded("sys-flag-dupe");
        let spec = json!({"flags": [
            {"id": "met-warden", "type": "bool", "initial": false},
            {"id": "met-warden", "type": "bool", "initial": true}
        ]});
        let err = store
            .define_system(&system_args("f", "flags", spec))
            .unwrap_err();
        assert_eq!(err.code, "flag_duplicate");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_an_int_flag_without_bounds_is_refused() {
        let (store, root, _source) = scaffolded("sys-flag-nobounds");
        let spec = json!({"flags": [
            {"id": "depth", "type": "int", "initial": 1}
        ]});
        let err = store
            .define_system(&system_args("f", "flags", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_an_int_initial_out_of_bounds_is_refused() {
        let (store, root, _source) = scaffolded("sys-flag-oob");
        let spec = json!({"flags": [
            {"id": "depth", "type": "int", "initial": 99, "min": 0, "max": 12}
        ]});
        let err = store
            .define_system(&system_args("f", "flags", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_float_flag_initial_is_refused() {
        let (store, root, _source) = scaffolded("sys-flag-float");
        let spec = json!({"flags": [
            {"id": "depth", "type": "int", "initial": 1.5, "min": 0, "max": 12}
        ]});
        let err = store
            .define_system(&system_args("f", "flags", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    fn machine_spec() -> Value {
        json!({"states": ["sealed", "open", "cleared"],
        "initial": "sealed", "terminal": ["cleared"],
        "transitions": [
            {"from": "sealed", "to": "open", "guard": "met-warden"},
            {"from": "open", "to": "cleared"}
        ]})
    }

    #[test]
    fn a_state_machine_round_trips() {
        let (store, root, _source) = with_flags("sys-machine");
        let body = store
            .define_system(&system_args("chamber", "state_machine", machine_spec()))
            .unwrap();
        assert_eq!(body["kind"], "state_machine");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_transition_to_an_undeclared_state_is_state_undeclared() {
        let (store, root, _source) = with_flags("sys-machine-undeclared");
        let mut spec = machine_spec();
        spec["transitions"][1]["to"] = json!("gone");
        let err = store
            .define_system(&system_args("chamber", "state_machine", spec))
            .unwrap_err();
        assert_eq!(err.code, "state_undeclared");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_guard_on_an_undeclared_flag_is_flag_undeclared() {
        let (store, root, _source) = with_flags("sys-machine-guard");
        let mut spec = machine_spec();
        spec["transitions"][0]["guard"] = json!("never-declared");
        let err = store
            .define_system(&system_args("chamber", "state_machine", spec))
            .unwrap_err();
        assert_eq!(err.code, "flag_undeclared");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_an_unreachable_non_terminal_state_is_graph_unreachable() {
        let (store, root, _source) = with_flags("sys-machine-island");
        let spec = json!({"states": ["sealed", "open", "side-room"],
               "initial": "sealed", "terminal": [],
               "transitions": [{"from": "sealed", "to": "open"}]});
        let err = store
            .define_system(&system_args("chamber", "state_machine", spec))
            .unwrap_err();
        assert_eq!(err.code, "graph_unreachable");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_unreachable_terminal_state_is_tolerated() {
        // The plan's contract: graph_unreachable names a
        // NON-terminal unreachable state. A terminal state no
        // path reaches is dead content, not a refusal.
        let (store, root, _source) = with_flags("sys-machine-terminal");
        let spec = json!({"states": ["sealed", "open", "cleared"],
               "initial": "sealed", "terminal": ["cleared"],
               "transitions": [{"from": "sealed", "to": "open"}]});
        store
            .define_system(&system_args("chamber", "state_machine", spec))
            .unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_progression_dag_chains_and_branches() {
        let (store, root, _source) = scaffolded("sys-dag");
        let spec = json!({"nodes": [
            {"id": "floor-1"},
            {"id": "floor-2", "requires": ["floor-1"]},
            {"id": "floor-3", "requires": ["floor-1"]},
            {"id": "boss", "requires": ["floor-2", "floor-3"]}
        ]});
        store
            .define_system(&system_args("descent", "progression_dag", spec))
            .unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_dag_cycle_is_graph_cycle() {
        let (store, root, _source) = scaffolded("sys-dag-cycle");
        let spec = json!({"nodes": [
            {"id": "a", "requires": ["b"]},
            {"id": "b", "requires": ["a"]}
        ]});
        let err = store
            .define_system(&system_args("descent", "progression_dag", spec))
            .unwrap_err();
        assert_eq!(err.code, "graph_cycle");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_dag_prerequisite_on_an_undeclared_node_is_dangling() {
        let (store, root, _source) = scaffolded("sys-dag-dangling");
        let spec = json!({"nodes": [
            {"id": "a", "requires": ["ghost"]}
        ]});
        let err = store
            .define_system(&system_args("descent", "progression_dag", spec))
            .unwrap_err();
        assert_eq!(err.code, "graph_dangling_ref");
        let _ = fs::remove_dir_all(&root);
    }

    fn economy_spec() -> Value {
        json!({"currencies": [{"id": "gold", "min": 0, "max": 9999}],
               "sources": [{"currency": "gold", "amount": 50}],
               "sinks": [{"currency": "gold", "amount": 20}]})
    }

    #[test]
    fn an_economy_with_a_source_and_sink_round_trips() {
        let (store, root, _source) = scaffolded("sys-economy");
        store
            .define_system(&system_args("market", "economy", economy_spec()))
            .unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_float_economy_amount_is_economy_float() {
        let (store, root, _source) = scaffolded("sys-economy-float");
        let mut spec = economy_spec();
        spec["sources"][0]["amount"] = json!(2.5);
        let err = store
            .define_system(&system_args("market", "economy", spec))
            .unwrap_err();
        assert_eq!(err.code, "economy_float");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_an_economy_entry_without_a_bound_is_economy_unbounded() {
        let (store, root, _source) = scaffolded("sys-economy-unbounded");
        let mut spec = economy_spec();
        spec["currencies"][0].as_object_mut().unwrap().remove("max");
        let err = store
            .define_system(&system_args("market", "economy", spec))
            .unwrap_err();
        assert_eq!(err.code, "economy_unbounded");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_sink_on_an_undeclared_currency_is_refused() {
        let (store, root, _source) = scaffolded("sys-economy-sink");
        let mut spec = economy_spec();
        spec["sinks"][0]["currency"] = json!("silver");
        let err = store
            .define_system(&system_args("market", "economy", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_daily_schedule_round_trips() {
        let (store, root, _source) = with_flags("sys-schedule");
        let spec = json!({"events": [
            {"id": "dawn", "tick": 0, "flag": "met-warden", "effect": "bells"},
            {"id": "dusk", "tick": 10, "flag": "met-warden", "effect": "lamps",
             "exclusive": true}
        ]});
        store
            .define_system(&system_args("day", "schedule", spec))
            .unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_negative_tick_is_refused() {
        let (store, root, _source) = with_flags("sys-schedule-negative");
        let spec = json!({"events": [
            {"id": "dawn", "tick": -1, "flag": "met-warden", "effect": "bells"}
        ]});
        let err = store
            .define_system(&system_args("day", "schedule", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_schedule_effect_on_an_undeclared_flag_is_flag_undeclared() {
        let (store, root, _source) = with_flags("sys-schedule-flag");
        let spec = json!({"events": [
            {"id": "dawn", "tick": 0, "flag": "never-declared", "effect": "bells"}
        ]});
        let err = store
            .define_system(&system_args("day", "schedule", spec))
            .unwrap_err();
        assert_eq!(err.code, "flag_undeclared");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_overlapping_exclusive_slots_are_refused() {
        let (store, root, _source) = with_flags("sys-schedule-overlap");
        let spec = json!({"events": [
            {"id": "dawn", "tick": 5, "flag": "met-warden", "effect": "bells",
             "exclusive": true},
            {"id": "dawn-2", "tick": 5, "flag": "met-warden", "effect": "horns",
             "exclusive": true}
        ]});
        let err = store
            .define_system(&system_args("day", "schedule", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    fn minigame_args(root: &Path) -> (Value, PathBuf) {
        let judge_dir = root.join("first-light/judges");
        fs::create_dir_all(&judge_dir).unwrap();
        let judge = judge_dir.join("coin.txt");
        fs::write(&judge, b"abc").unwrap();
        let spec = json!({"rules": "call the coin flip",
               "inputs": ["call"],
               "judge": {"path": "judges/coin.txt", "sha256": ABC_SHA}});
        (system_args("coin-flip", "minigame", spec), judge)
    }

    #[test]
    fn a_minigame_round_trips_and_the_census_verifies_its_judge() {
        let (store, root, _source) = scaffolded("sys-minigame");
        let (args, _judge) = minigame_args(&root);
        store.define_system(&args).unwrap();
        let census = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap();
        assert_eq!(census["counts"]["system_records"], 1);
        assert_eq!(
            check_named(&census, "system:coin-flip:judge")["status"],
            "verified"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_judge_without_a_hash_is_refused() {
        let (store, root, _source) = scaffolded("sys-minigame-nohash");
        let spec = json!({"rules": "call it",
               "judge": {"path": "judges/coin.txt"}});
        let err = store
            .define_system(&system_args("coin-flip", "minigame", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_judge_path_with_traversal_is_refused() {
        let (store, root, _source) = scaffolded("sys-minigame-traversal");
        let spec = json!({"rules": "call it",
               "judge": {"path": "../secret.txt", "sha256": ABC_SHA}});
        let err = store
            .define_system(&system_args("coin-flip", "minigame", spec))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_the_census_names_a_judge_hash_mismatch() {
        let (store, root, _source) = scaffolded("sys-minigame-mismatch");
        let (args, judge) = minigame_args(&root);
        store.define_system(&args).unwrap();
        fs::write(&judge, b"tampered").unwrap();
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("the census rides the refusal");
        assert_eq!(
            check_named(&body, "system:coin-flip:judge")["status"],
            "unverified"
        );
        let _ = fs::remove_dir_all(&root);
    }

    // ---------- SYS-08/09: scenes and the descent law ----------

    fn scene_args(scene_id: &str, graph: Value) -> Value {
        let mut args = json!({"envelope": envelope(), "project_id": "first-light",
               "scene_id": scene_id, "graph": graph});
        args["envelope"]["record_id"] = json!(format!("rec-scene-{scene_id}"));
        args
    }

    fn three_node_graph() -> Value {
        json!({"nodes": [{"id": "entry"}, {"id": "vault"}, {"id": "shrine"}],
               "edges": [{"from": "entry", "to": "vault"},
                         {"from": "vault", "to": "shrine"}],
               "links": [{"id": "l1", "from": "entry", "to": "shrine"}]})
    }

    #[test]
    fn a_three_node_scene_defines_and_validates() {
        let (store, root, _source) = scaffolded("sys-scene");
        let body = store
            .define_scene(&scene_args("gatehouse", three_node_graph()))
            .unwrap();
        assert_eq!(body["status"], "candidate");
        let verdict = store
            .validate_scene(&json!({"project_id": "first-light",
                                    "scene_id": "gatehouse"}))
            .unwrap();
        assert_eq!(verdict["valid"], true);
        assert_eq!(verdict["refusals"], json!([]));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_cycle_where_the_scene_declares_itself_acyclic_is_refused() {
        let (store, root, _source) = scaffolded("sys-scene-cycle");
        let mut graph = three_node_graph();
        graph["acyclic"] = json!(true);
        graph["edges"]
            .as_array_mut()
            .unwrap()
            .push(json!({"from": "shrine", "to": "entry"}));
        store.define_scene(&scene_args("loop", graph)).unwrap();
        let verdict = store
            .validate_scene(&json!({"project_id": "first-light",
                                    "scene_id": "loop"}))
            .unwrap();
        assert_eq!(verdict["valid"], false);
        assert!(
            verdict["refusals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "graph_cycle"),
            "verdict: {verdict}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_dangling_dialogue_link_is_refused() {
        let (store, root, _source) = scaffolded("sys-scene-dangling");
        let mut graph = three_node_graph();
        graph["links"][0]["to"] = json!("nowhere");
        store.define_scene(&scene_args("broken", graph)).unwrap();
        let verdict = store
            .validate_scene(&json!({"project_id": "first-light",
                                    "scene_id": "broken"}))
            .unwrap();
        assert_eq!(verdict["valid"], false);
        assert!(
            verdict["refusals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "graph_dangling_ref"),
            "verdict: {verdict}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_an_oversized_scene_graph_is_refused() {
        let (store, root, _source) = scaffolded("sys-scene-oversized");
        let nodes: Vec<Value> = (0..513).map(|i| json!({"id": format!("n{i}")})).collect();
        let graph = json!({"nodes": nodes, "edges": []});
        // Define refuses it outright (bound_exceeded)…
        let err = store
            .define_scene(&scene_args("huge", graph.clone()))
            .unwrap_err();
        assert_eq!(err.code, "bound_exceeded");
        // …and a hand-written oversized record (authored behind
        // the server's back) is refused by the validator.
        let dir = root.join("first-light/scenes");
        fs::create_dir_all(&dir).unwrap();
        let record = json!({"record_type": "meister_scene",
               "project_id": "first-light", "scene_id": "huge",
               "status": "candidate", "envelope": envelope(),
               "graph": graph});
        fs::write(dir.join("huge.json"), record.to_string()).unwrap();
        let verdict = store
            .validate_scene(&json!({"project_id": "first-light",
                                    "scene_id": "huge"}))
            .unwrap();
        assert_eq!(verdict["valid"], false);
        assert!(
            verdict["refusals"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r == "bound_exceeded"),
            "verdict: {verdict}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    fn floor(floor_id: &str, chambers: &[&str], doors: Value) -> Value {
        json!({"floor_id": floor_id,
               "chambers": chambers.iter().map(|c| json!({"id": c})).collect::<Vec<_>>(),
               "entry_points": [{"id": "landing"}],
               "doors": doors})
    }

    fn descent_layout() -> Value {
        json!({"floors": [
            floor("floor-1", &["entry-hall", "vault"],
                  json!([{"id": "d1", "between": ["entry-hall", "vault"]}])),
            floor("floor-2", &["crypt"], json!([])),
            floor("floor-3", &["deep-vault"], json!([]))
        ],
        "stairs": [
            {"id": "s1", "from_floor": "floor-1", "to_floor": "floor-2",
             "direction": "both"},
            {"id": "s2", "from_floor": "floor-2", "to_floor": "floor-3",
             "direction": "both"}
        ]})
    }

    fn level_args(layout: Value) -> Value {
        let mut args = json!({"envelope": envelope(), "project_id": "first-light",
               "level_id": "the-descent", "layout": layout});
        args["envelope"]["record_id"] = json!("rec-level-descent");
        args
    }

    #[test]
    fn a_synthetic_three_floor_descent_composes() {
        let (store, root, _source) = scaffolded("sys-descent");
        let body = store.compose_level(&level_args(descent_layout())).unwrap();
        assert_eq!(body["status"], "candidate");
        assert_eq!(body["floor_count"], 3);
        assert!(root.join("first-light/levels/the-descent.json").is_file());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_floor_with_no_entry_point_is_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-noentry");
        let mut layout = descent_layout();
        layout["floors"][1]["entry_points"] = json!([]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        assert!(err.message.contains("floor-2"), "message: {}", err.message);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_floor_with_no_chambers_is_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-nochamber");
        let mut layout = descent_layout();
        layout["floors"][0]["chambers"] = json!([]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_one_sided_door_is_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-door");
        let mut layout = descent_layout();
        layout["floors"][0]["doors"] =
            json!([{"id": "d-ghost", "between": ["entry-hall", "nowhere"]}]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        assert!(err.message.contains("d-ghost"), "message: {}", err.message);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_skipped_floor_is_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-skip");
        let mut layout = descent_layout();
        layout["stairs"] = json!([
            {"id": "s1", "from_floor": "floor-1", "to_floor": "floor-3",
             "direction": "both"}
        ]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_missing_stairs_pair_is_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-unpaired");
        let mut layout = descent_layout();
        layout["stairs"]
            .as_array_mut()
            .unwrap()
            .retain(|e| e["id"] != "s2");
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        assert!(err.message.contains("floor-3"), "message: {}", err.message);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_one_way_stairs_are_a_descent_violation() {
        // D-1659: the player descends AND ascends between floors.
        let (store, root, _source) = scaffolded("sys-descent-oneway");
        let mut layout = descent_layout();
        layout["stairs"][0]["direction"] = json!("down");
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_stairs_to_an_undeclared_floor_are_a_descent_violation() {
        let (store, root, _source) = scaffolded("sys-descent-ghost");
        let mut layout = descent_layout();
        layout["stairs"][1]["to_floor"] = json!("floor-99");
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn level_with_narrative_composes() {
        // NAR-04: dialogue at the camp (a chamber) and the
        // stairs (a trigger), an encounter placed in a declared
        // chamber — the narrative half is real at Rung 3.
        let (store, root, source) = scaffolded("nar-compose");
        store.add_cast(&cast_args(&source)).unwrap();
        store
            .author_dialogue(&json!({"envelope": envelope(), "project_id": "first-light",
                "dialogue_id": "gate-words",
                "lines": [{"id": "a", "speaker": "warden", "text": "Halt."}]}))
            .unwrap();
        let mut layout = descent_layout();
        layout["narrative"] = json!([
            {"floor_id": "floor-1", "chamber": "entry-hall",
             "dialogue_id": "gate-words"}
        ]);
        layout["encounters"] = json!([
            {"id": "e1", "floor_id": "floor-2", "chamber": "crypt"}
        ]);
        let mut args = level_args(layout);
        args["triggers"] = json!([{"id": "t1", "dialogue": "gate-words"}]);
        let body = store.compose_level(&args).unwrap();
        assert_eq!(body["floor_count"], 3);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_level_undeclared_dialogue_link_is_refused() {
        let (store, root, _source) = scaffolded("nar-compose-dangling");
        let mut layout = descent_layout();
        layout["narrative"] = json!([
            {"floor_id": "floor-1", "chamber": "entry-hall",
             "dialogue_id": "no-such-dialogue"}
        ]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "graph_dangling_ref");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_level_encounter_in_undeclared_room_is_refused() {
        let (store, root, _source) = scaffolded("nar-compose-room");
        let mut layout = descent_layout();
        layout["encounters"] = json!([
            {"id": "e1", "floor_id": "floor-1", "chamber": "nowhere"}
        ]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "descent_violation");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn census_sanitization_hit_locates_the_field() {
        let (store, root) = nar_store("san1");
        store
            .author_dialogue(&json!({"envelope": envelope(), "project_id": "first-light",
                "dialogue_id": "gate-words",
                "lines": [{"id": "a", "speaker": "warden",
                           "text": "The Qompass gate stands."}]}))
            .unwrap();
        fs::write(
            root.join("first-light/denylist.json"),
            "{\"terms\": [\"qompass\"]}",
        )
        .unwrap();
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        assert_eq!(err.code, "canon_unverified");
        let body = err.census.expect("census body rides the refusal");
        assert_eq!(body["counts"]["sanitization_hits"], 1);
        assert!(
            body["failures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|f| f.as_str().unwrap().contains("lines[0].text")),
            "the hit locates its field: {body}"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn census_differentiation_collision_and_overlap_report() {
        let (store, root, source) = scaffolded("nar-diff");
        store.add_cast(&cast_args(&source)).unwrap();
        // A second member with the same display name.
        let mut twin = cast_args(&source);
        twin["character_id"] = json!("warden-twin");
        twin["envelope"]["record_id"] = json!("rec-cast-twin");
        store.add_cast(&twin).unwrap();
        let err = store
            .canon_census(&json!({"project_id": "first-light"}))
            .unwrap_err();
        let body = err.census.expect("census body rides the refusal");
        assert!(
            body["failures"].as_array().unwrap().iter().any(|f| f
                .as_str()
                .unwrap()
                .contains("differentiation:warden:warden-twin")),
            "the collision is a named failure: {body}"
        );
        let pairs = body["differentiation"].as_array().unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0]["name_collision"], true);
        assert_eq!(pairs[0]["descriptor_overlap"], 1.0);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn adv_a_gate_on_an_undeclared_progression_system_is_dangling() {
        let (store, root, _source) = scaffolded("sys-descent-gate");
        let mut layout = descent_layout();
        layout["gates"] = json!([{"id": "g1", "system_id": "no-such-dag"}]);
        let err = store.compose_level(&level_args(layout)).unwrap_err();
        assert_eq!(err.code, "graph_dangling_ref");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_descent_with_declared_gates_and_minigames_composes() {
        let (store, root, _source) = scaffolded("sys-descent-full");
        store
            .define_system(&system_args(
                "descent-dag",
                "progression_dag",
                json!({"nodes": [{"id": "floor-1"},
                                 {"id": "floor-2", "requires": ["floor-1"]}]}),
            ))
            .unwrap();
        let judge_dir = root.join("first-light/judges");
        fs::create_dir_all(&judge_dir).unwrap();
        fs::write(judge_dir.join("coin.txt"), b"abc").unwrap();
        store
            .define_system(&system_args(
                "coin-flip",
                "minigame",
                json!({"rules": "call it",
                       "judge": {"path": "judges/coin.txt", "sha256": ABC_SHA}}),
            ))
            .unwrap();
        let mut layout = descent_layout();
        layout["gates"] = json!([{"id": "g1", "system_id": "descent-dag"}]);
        layout["minigames"] = json!([{"id": "m1", "system_id": "coin-flip"}]);
        let body = store.compose_level(&level_args(layout)).unwrap();
        assert_eq!(body["floor_count"], 3);
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

    // ---------- Rung 3: narrative (NAR-01..03) ----------

    fn nar_store(tag: &str) -> (Store, PathBuf) {
        let (store, root) = temp_store(&format!("nar-{tag}"));
        let source = source_file(&root);
        store.create_project(&create_args()).unwrap();
        store
            .define_system(&json!({"envelope": envelope(), "project_id": "first-light",
            "system_id": "core-flags", "kind": "flags",
            "spec": {"flags": [
                {"id": "met-warden", "type": "bool", "initial": false},
                {"id": "trust", "type": "int", "initial": 0, "min": 0, "max": 10}
            ]}}))
            .unwrap();
        store.add_cast(&cast_args(&source)).unwrap();
        (store, root)
    }

    fn dialogue_args(lines: Value) -> Value {
        json!({"envelope": envelope(), "project_id": "first-light",
               "dialogue_id": "gate-words", "lines": lines})
    }

    #[test]
    fn dialogue_branching_round_trips_unchanged() {
        let (store, _root) = nar_store("t0");
        let lines = json!([
            {"id": "open", "speaker": "warden", "text": "State your errand.",
             "choices": [
                 {"label": "Ask about the gate", "target": "gate"},
                 {"label": "Leave", "target": "leave"}],
             "effects": [{"flag": "met-warden", "set": true}]},
            {"id": "gate", "speaker": "warden", "text": "It opens for the proven.",
             "conditions": [{"flag": "met-warden", "equals": true}]},
            {"id": "leave", "speaker": "warden", "text": "Then go."}
        ]);
        let body = store
            .author_dialogue(&dialogue_args(lines.clone()))
            .unwrap();
        assert_eq!(body["line_count"], 3);
        let back = store
            .get_dialogue(&json!({"project_id": "first-light",
                                   "dialogue_id": "gate-words"}))
            .unwrap();
        assert_eq!(back["lines"], lines, "the record round-trips unchanged");
    }

    #[test]
    fn adv_dialogue_undeclared_flag_is_refused() {
        let (store, _root) = nar_store("t1");
        let err = store
            .author_dialogue(&dialogue_args(json!([
                {"id": "a", "speaker": "warden", "text": "Hi.",
                 "conditions": [{"flag": "never-declared"}]}
            ])))
            .unwrap_err();
        assert_eq!(err.code, "flag_undeclared");
    }

    #[test]
    fn adv_dialogue_unknown_speaker_is_refused() {
        let (store, _root) = nar_store("t2");
        let err = store
            .author_dialogue(&dialogue_args(json!([
                {"id": "a", "speaker": "ghost", "text": "Boo."}
            ])))
            .unwrap_err();
        assert_eq!(err.code, "speaker_unknown");
    }

    #[test]
    fn adv_dialogue_over_line_bound_is_refused() {
        let (store, _root) = nar_store("t3");
        let lines: Vec<Value> = (0..2049)
            .map(|i| json!({"id": format!("l{i}"), "speaker": "warden", "text": "x"}))
            .collect();
        let err = store
            .author_dialogue(&dialogue_args(json!(lines)))
            .unwrap_err();
        assert_eq!(err.code, "bound_exceeded");
    }

    #[test]
    fn adv_dialogue_loose_prose_is_refused() {
        let (store, _root) = nar_store("t4");
        let err = store
            .author_dialogue(&dialogue_args(json!(["just some prose"])))
            .unwrap_err();
        assert_eq!(err.code, "invalid_record");
    }

    #[test]
    fn reaction_locked_character_takes_a_profile() {
        let (store, root) = nar_store("t5");
        let source = source_file(&root);
        store.lock_cast(&lock_args(&source)).unwrap();
        let body = store
            .set_reaction(&json!({"envelope": envelope(), "project_id": "first-light",
                "character_id": "warden", "emotion": "defiance",
                "posture": "weight back, chin up", "gesture": "hand off the hilt",
                "stance": "feet planted wide"}))
            .unwrap();
        assert_eq!(body["status"], "candidate");
    }

    #[test]
    fn adv_reaction_unlocked_character_is_refused() {
        let (store, _root) = nar_store("t6");
        let err = store
            .set_reaction(&json!({"envelope": envelope(), "project_id": "first-light",
                "character_id": "warden", "emotion": "defiance",
                "posture": "weight back", "gesture": "hand off the hilt",
                "stance": "feet planted"}))
            .unwrap_err();
        assert_eq!(err.code, "identity_unlocked");
    }

    #[test]
    fn adv_reaction_unknown_character_is_refused() {
        let (store, _root) = nar_store("t7");
        let err = store
            .set_reaction(&json!({"envelope": envelope(), "project_id": "first-light",
                "character_id": "nobody", "emotion": "defiance",
                "posture": "weight back", "gesture": "hand off the hilt",
                "stance": "feet planted"}))
            .unwrap_err();
        assert_eq!(err.code, "not_found");
    }

    #[test]
    fn story_two_ending_arc_defines() {
        let (store, _root) = nar_store("t8");
        let body = store
            .define_story(&json!({"envelope": envelope(), "project_id": "first-light",
            "arc_id": "gate-arc",
            "beats": [
                {"id": "arrive", "choices": [
                    {"label": "Prove", "target": "proven"},
                    {"label": "Turn back", "target": "turned"}]},
                {"id": "proven", "kind": "ending"},
                {"id": "turned", "kind": "ending"}
            ]}))
            .unwrap();
        assert_eq!(body["beat_count"], 3);
    }

    #[test]
    fn adv_story_undeclared_scene_ref_is_refused() {
        let (store, _root) = nar_store("t9");
        let err = store
            .define_story(&json!({"envelope": envelope(), "project_id": "first-light",
                "arc_id": "gate-arc",
                "beats": [{"id": "arrive", "refs": {"scenes": ["no-such-scene"]}}]}))
            .unwrap_err();
        assert_eq!(err.code, "graph_dangling_ref");
    }

    #[test]
    fn adv_story_quest_undeclared_flag_is_refused() {
        let (store, _root) = nar_store("t10");
        let err = store
            .define_story(&json!({"envelope": envelope(), "project_id": "first-light",
                "arc_id": "gate-arc",
                "beats": [{"id": "arrive", "quests": [
                    {"id": "q1", "title": "Prove it", "objective_flag": "nope"}]}]}))
            .unwrap_err();
        assert_eq!(err.code, "flag_undeclared");
    }
}
