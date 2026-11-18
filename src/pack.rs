//! Genre packs (PK-01): packs are data plus emitters, never tools.
//!
//! A pack is one JSON document, `packs/<slug>/pack.json`, compiled
//! into the binary with `include_str!` (see `COMPILED`). It holds:
//!
//! - specialised record `kind`s, each with a declared field table
//!   that the server-side validator (`Selection::validate`) checks
//!   records against;
//! - systems templates over the core system kinds;
//! - its media profile: lumen formats, each with integer budgets;
//! - playtest generators and its gate set (named, described);
//! - the emitters it uses, by name, from the emitters this build
//!   carries.
//!
//! A pack carries no code and has no channel to the tool catalog:
//! the format is closed (an unknown key is refused), a pack that
//! declares a tool anywhere is refused with `pack_declares_tool`,
//! and the catalog takes no input from packs. The served tool
//! count is therefore independent of the number of packs — the
//! catalog invariant this task's gate rests on, pinned by
//! `tests::catalog_invariant_tool_count_does_not_depend_on_packs`.
//!
//! A project's primary and secondary packs select which kinds'
//! validators run (`Registry::select`); a record of a kind no
//! selected pack declares is refused with `pack_kind_not_selected`.
//! The live call path is `meister_system_define` with kind `pack`
//! (RPG-01): one fixed enum value and one `pack_kind` input, the
//! same whatever packs are compiled in, so the invariant holds.
//!
//! A field may declare `ref`: its value (a string, or an array of
//! strings) names records of that kind — a core system kind or a
//! kind of the same pack — and each must resolve in the project, or
//! the record is refused with `graph_dangling_ref`. A systems
//! template may target `level`: its spec is a SYS-09 layout,
//! instantiated through `meister_level_compose`.
//!
//! Every check is bounded by the constants below, and every
//! refusal is typed. Validation is pure: no store, no files.

use std::collections::BTreeSet;

use serde_json::{Map, Value};

use crate::catalog;
use crate::store::{ASSET_KINDS, EMITTERS, GENRE_PACKS, SYSTEM_KINDS};

/// The pack format this build reads.
pub const PACK_SCHEMA: &str = "meister-pack/1";

/// The packs compiled into this build, as `(slug, pack.json)`.
/// A pack lands by adding its file under `packs/<slug>/` and one
/// line here, e.g.
/// `("role_playing", include_str!("../packs/role_playing/pack.json"))`.
/// PK-01 landed the mechanism; RPG-01 lands the first pack and
/// TEACH-01 the second.
const COMPILED: &[(&str, &str)] = &[
    (
        "role_playing",
        include_str!("../packs/role_playing/pack.json"),
    ),
    (
        "educational",
        include_str!("../packs/educational/pack.json"),
    ),
];

/// The `meister_system_define` kind that carries a pack record. A
/// pack kind may never take this name.
pub const PACK_SYSTEM_KIND: &str = "pack";

/// The template target that is a SYS-09 level layout rather than
/// a system spec.
pub const LEVEL_TEMPLATE: &str = "level";

/// The media-map formats lumen owns (README §"The media map";
/// AST-05's route map). `voice` is the one asset kind that is
/// not lumen's: voice style belongs to eigenstyle.
pub const LUMEN_FORMATS: &[&str] = &[
    "still_2d",
    "pixel",
    "animation",
    "model_3d",
    "effect",
    "audio",
    "text",
    "video",
];

/// Record kinds the core already owns, beyond the system and asset
/// kinds: the store's record types (`meister_<kind>`). A pack kind
/// may not reuse any of them, nor take the `meister_` prefix.
const CORE_RECORD_KINDS: &[&str] = &[
    "project",
    "cast_member",
    "system",
    "scene",
    "level",
    "dialogue",
    "reaction",
    "story_arc",
    "verdict",
    "emission",
    "asset",
    "handoff",
    "custody_refusal",
];

/// The namespace reserved for the core's own names.
const CORE_PREFIX: &str = "meister_";

/// Keys that carry a tool definition in any MCP-shaped document.
/// A pack holding one anywhere is declaring a tool.
const TOOL_KEYS: &[&str] = &["tool", "tools", "inputSchema", "input_schema"];

const MAX_PACK_BYTES: usize = 262_144;
const MAX_DEPTH: usize = 16;
const MAX_ID_CHARS: usize = 64;
const MAX_TEXT_CHARS: usize = 500;
const MAX_KINDS: usize = 64;
const MAX_FIELDS: usize = 64;
const MAX_TEMPLATES: usize = 64;
const MAX_GENERATORS: usize = 32;
const MAX_GATES: usize = 64;
const MAX_BUDGET_KEYS: usize = 16;
const MAX_SECONDARY_PACKS: usize = 12;
/// The most ids one `ref` array field may name.
const MAX_REF_ITEMS: usize = 256;

const TOP_KEYS: &[&str] = &[
    "schema",
    "slug",
    "version",
    "kinds",
    "systems_templates",
    "media_profile",
    "playtest_generators",
    "gates",
    "emitters",
];

/// A typed pack failure.
#[derive(Debug, PartialEq, Eq)]
pub struct PackError {
    /// Stable code: `pack_invalid`, `pack_unknown_field`,
    /// `pack_declares_tool`, `pack_kind_collision`,
    /// `pack_format_not_lumen`, `pack_too_large`,
    /// `pack_kind_not_selected`, `record_invalid`,
    /// `graph_dangling_ref`.
    pub code: &'static str,
    /// Human-readable reason.
    pub message: String,
}

impl PackError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new("pack_invalid", message)
    }
}

impl std::fmt::Display for PackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

pub type PackResult<T> = Result<T, PackError>;

/// The JSON type a declared field must carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldType {
    String,
    Integer,
    Boolean,
    Array,
    Object,
}

impl FieldType {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "string" => Some(Self::String),
            "integer" => Some(Self::Integer),
            "boolean" => Some(Self::Boolean),
            "array" => Some(Self::Array),
            "object" => Some(Self::Object),
            _ => None,
        }
    }

    fn admits(self, value: &Value) -> bool {
        match self {
            Self::String => value.is_string(),
            Self::Integer => value.is_i64() || value.is_u64(),
            Self::Boolean => value.is_boolean(),
            Self::Array => value.is_array(),
            Self::Object => value.is_object(),
        }
    }
}

/// One declared field of a pack kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDef {
    pub name: String,
    pub field_type: FieldType,
    pub required: bool,
    /// The kind whose records this field's value names, if any.
    pub ref_kind: Option<String>,
}

/// One specialised record kind and its field table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindDef {
    pub kind: String,
    pub fields: Vec<FieldDef>,
}

/// One systems template over a core system kind.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub template_id: String,
    pub system_kind: String,
    pub spec: Value,
}

/// One media-profile entry: a lumen format and its budgets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaEntry {
    pub format: String,
    pub budget: Vec<(String, u64)>,
}

/// A named, described entry: a playtest generator or a gate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub description: String,
}

/// A validated pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub slug: String,
    pub version: String,
    pub kinds: Vec<KindDef>,
    pub systems_templates: Vec<Template>,
    pub media_profile: Vec<MediaEntry>,
    pub playtest_generators: Vec<Named>,
    pub gates: Vec<Named>,
    pub emitters: Vec<String>,
}

/// Every pack this build carries, validated together.
#[derive(Debug)]
pub struct Registry {
    packs: Vec<Pack>,
}

/// The packs a project selected: their kinds' validators run, and
/// only theirs.
#[derive(Debug)]
pub struct Selection<'a> {
    packs: Vec<&'a Pack>,
}

/// Validate the compiled-in packs. Run before anything is served:
/// a build carrying an invalid pack refuses to start.
pub fn check_compiled() -> PackResult<usize> {
    Registry::compiled().map(|r| r.packs.len())
}

impl Registry {
    /// The packs compiled into this build.
    pub fn compiled() -> PackResult<Self> {
        Self::from_sources(COMPILED)
    }

    /// Validate each `(slug, pack.json)` source and the set as a
    /// whole: slugs unique, and no kind declared by two packs.
    pub fn from_sources(sources: &[(&str, &str)]) -> PackResult<Self> {
        if sources.len() > GENRE_PACKS.len() {
            return Err(PackError::invalid("more packs than genre categories"));
        }
        let tool_names: BTreeSet<&'static str> =
            catalog::specs().iter().map(|spec| spec.name).collect();
        let mut packs: Vec<Pack> = Vec::with_capacity(sources.len());
        let mut kinds: BTreeSet<String> = BTreeSet::new();
        for (dir_slug, text) in sources {
            let pack = parse_pack(text, &tool_names)?;
            if pack.slug != *dir_slug {
                return Err(PackError::invalid(format!(
                    "pack under packs/{dir_slug}/ declares slug {}",
                    pack.slug
                )));
            }
            if packs.iter().any(|p| p.slug == pack.slug) {
                return Err(PackError::invalid(format!(
                    "pack {} is compiled in twice",
                    pack.slug
                )));
            }
            for def in &pack.kinds {
                if !kinds.insert(def.kind.clone()) {
                    return Err(PackError::new(
                        "pack_kind_collision",
                        format!(
                            "kind {} in pack {} is already declared by another pack",
                            def.kind, pack.slug
                        ),
                    ));
                }
            }
            packs.push(pack);
        }
        Ok(Self { packs })
    }

    /// The packs a project with this primary and these secondaries
    /// selects. A genre whose pack has not landed contributes no
    /// kinds; a slug that is no genre at all is refused.
    pub fn select(&self, primary: &str, secondaries: &[&str]) -> PackResult<Selection<'_>> {
        if secondaries.len() > MAX_SECONDARY_PACKS {
            return Err(PackError::invalid("secondary packs exceed their bound"));
        }
        let mut wanted: Vec<&str> = Vec::with_capacity(secondaries.len() + 1);
        for slug in std::iter::once(&primary).chain(secondaries.iter()) {
            if !GENRE_PACKS.contains(slug) {
                return Err(PackError::invalid(format!("{slug} is not a genre pack")));
            }
            if wanted.contains(slug) {
                return Err(PackError::invalid(format!("pack {slug} selected twice")));
            }
            wanted.push(slug);
        }
        let packs = self
            .packs
            .iter()
            .filter(|p| wanted.contains(&p.slug.as_str()))
            .collect();
        Ok(Selection { packs })
    }
}

impl Selection<'_> {
    /// Run the selected validator for `kind` against `record`: the
    /// record is an object, every required field is present, every
    /// present field is declared and carries its declared type, and
    /// every `ref` field's ids resolve. `resolves(kind, id)` answers
    /// whether the project holds a record `id` of `kind`; it is the
    /// caller's view of the store, so validation stays pure here.
    pub fn validate(
        &self,
        kind: &str,
        record: &Value,
        resolves: &dyn Fn(&str, &str) -> bool,
    ) -> PackResult<()> {
        let Some((slug, def)) = self
            .packs
            .iter()
            .flat_map(|p| p.kinds.iter().map(move |d| (p.slug.as_str(), d)))
            .find(|(_, d)| d.kind == kind)
        else {
            return Err(PackError::new(
                "pack_kind_not_selected",
                format!("no pack this project selected declares kind {kind}"),
            ));
        };
        let obj = record
            .as_object()
            .ok_or_else(|| PackError::new("record_invalid", "a pack record is an object"))?;
        for field in &def.fields {
            match obj.get(&field.name) {
                None if field.required => {
                    return Err(PackError::new(
                        "record_invalid",
                        format!("{kind} record is missing required field {}", field.name),
                    ));
                }
                Some(value) if !field.field_type.admits(value) => {
                    return Err(PackError::new(
                        "record_invalid",
                        format!("{kind} field {} must be {:?}", field.name, field.field_type),
                    ));
                }
                _ => {}
            }
        }
        if let Some(extra) = obj
            .keys()
            .find(|k| !def.fields.iter().any(|f| &f.name == *k))
        {
            return Err(PackError::new(
                "record_invalid",
                format!("{kind} declares no field {extra}"),
            ));
        }
        for field in &def.fields {
            let (Some(target), Some(value)) = (&field.ref_kind, obj.get(&field.name)) else {
                continue;
            };
            check_refs(kind, &field.name, target, value, resolves)?;
        }
        // The Educational pack's kinds carry the teaching law
        // beyond their field tables (TEACH-01).
        if slug == "educational" {
            crate::teach::validate(kind, record)?;
        }
        Ok(())
    }
}

/// Every id a `ref` field names resolves to a record of `target`.
/// The field's type (string or array) was checked by the caller.
fn check_refs(
    kind: &str,
    field: &str,
    target: &str,
    value: &Value,
    resolves: &dyn Fn(&str, &str) -> bool,
) -> PackResult<()> {
    let ids: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        single => vec![single],
    };
    if ids.len() > MAX_REF_ITEMS {
        return Err(PackError::new(
            "record_invalid",
            format!("{kind} field {field} names over {MAX_REF_ITEMS} records"),
        ));
    }
    for id in ids {
        let id = id.as_str().ok_or_else(|| {
            PackError::new(
                "record_invalid",
                format!("{kind} field {field} names records by string id"),
            )
        })?;
        if !resolves(target, id) {
            return Err(PackError::new(
                "graph_dangling_ref",
                format!("{kind} field {field} names undeclared {target} {id}"),
            ));
        }
    }
    Ok(())
}

/// Parse and validate one pack document.
fn parse_pack(text: &str, tool_names: &BTreeSet<&'static str>) -> PackResult<Pack> {
    if text.len() > MAX_PACK_BYTES {
        return Err(PackError::new(
            "pack_too_large",
            format!(
                "pack is {} bytes; the bound is {MAX_PACK_BYTES}",
                text.len()
            ),
        ));
    }
    let doc: Value = serde_json::from_str(text)
        .map_err(|e| PackError::invalid(format!("pack is not JSON: {e}")))?;
    // The tool scan runs first and over the whole document, so a
    // tool smuggled under an unknown key is still named for what
    // it is.
    scan_for_tools(&doc, 0)?;
    let top = closed_object(&doc, TOP_KEYS, "pack")?;
    let schema = req_str(top, "schema", "pack", MAX_ID_CHARS)?;
    if schema != PACK_SCHEMA {
        return Err(PackError::invalid(format!(
            "pack schema {schema} is not {PACK_SCHEMA}"
        )));
    }
    let slug = req_str(top, "slug", "pack", MAX_ID_CHARS)?;
    if !GENRE_PACKS.contains(&slug) {
        return Err(PackError::invalid(format!("{slug} is not a genre pack")));
    }
    let version = req_str(top, "version", "pack", MAX_ID_CHARS)?;
    if !is_semver(version) {
        return Err(PackError::invalid(format!(
            "version {version} is not MAJOR.MINOR.PATCH"
        )));
    }
    let mut ids = IdSpace::new(tool_names);
    let kinds = parse_list(top, "kinds", MAX_KINDS, |v| parse_kind(v, &mut ids))?;
    let systems_templates = parse_list(top, "systems_templates", MAX_TEMPLATES, |v| {
        parse_template(v, &mut ids)
    })?;
    let mut formats = BTreeSet::new();
    let media_profile = parse_list(top, "media_profile", LUMEN_FORMATS.len(), |v| {
        parse_media(v, &mut formats)
    })?;
    let playtest_generators = parse_list(top, "playtest_generators", MAX_GENERATORS, |v| {
        parse_named(v, "generator_id", &mut ids)
    })?;
    let gates = parse_list(top, "gates", MAX_GATES, |v| {
        parse_named(v, "gate_id", &mut ids)
    })?;
    check_ref_targets(&kinds)?;
    let mut seen_emitters = BTreeSet::new();
    let emitters = parse_list(top, "emitters", EMITTERS.len(), |v| {
        let name = v
            .as_str()
            .ok_or_else(|| PackError::invalid("an emitter is named by a string"))?;
        if !EMITTERS.contains(&name) {
            return Err(PackError::invalid(format!(
                "emitter {name} is not one this build carries; a pack names emitters, it never brings one"
            )));
        }
        if !seen_emitters.insert(name.to_string()) {
            return Err(PackError::invalid(format!("emitter {name} listed twice")));
        }
        Ok(name.to_string())
    })?;
    Ok(Pack {
        slug: slug.to_string(),
        version: version.to_string(),
        kinds,
        systems_templates,
        media_profile,
        playtest_generators,
        gates,
        emitters,
    })
}

/// Refuse a tool-carrying key anywhere in the document, bounded by
/// `MAX_DEPTH`.
fn scan_for_tools(value: &Value, depth: usize) -> PackResult<()> {
    if depth > MAX_DEPTH {
        return Err(PackError::invalid(format!(
            "pack nests deeper than {MAX_DEPTH} levels"
        )));
    }
    match value {
        Value::Object(map) => {
            if let Some(key) = map.keys().find(|k| TOOL_KEYS.contains(&k.as_str())) {
                return Err(PackError::new(
                    "pack_declares_tool",
                    format!("pack carries a {key} key; packs are data plus emitters, never tools"),
                ));
            }
            map.values().try_for_each(|v| scan_for_tools(v, depth + 1))
        }
        Value::Array(items) => items.iter().try_for_each(|v| scan_for_tools(v, depth + 1)),
        _ => Ok(()),
    }
}

/// The pack-local name space: ids are unique across the pack's
/// kinds, templates, generators and gates, never a catalog tool's
/// name, and never a core kind.
struct IdSpace<'t> {
    tool_names: &'t BTreeSet<&'static str>,
    seen: BTreeSet<String>,
}

impl<'t> IdSpace<'t> {
    fn new(tool_names: &'t BTreeSet<&'static str>) -> Self {
        Self {
            tool_names,
            seen: BTreeSet::new(),
        }
    }

    fn claim(&mut self, id: &str, what: &str) -> PackResult<()> {
        if !is_id(id) {
            return Err(PackError::invalid(format!(
                "{what} {id:?} is not a lowercase snake_case id of at most {MAX_ID_CHARS} chars"
            )));
        }
        if self.tool_names.contains(id) {
            return Err(PackError::new(
                "pack_declares_tool",
                format!("{what} {id} is a catalog tool's name; packs never declare tools"),
            ));
        }
        if !self.seen.insert(id.to_string()) {
            return Err(PackError::invalid(format!("{what} {id} is declared twice")));
        }
        Ok(())
    }
}

/// Every `ref` names a core system kind or a kind this pack
/// declares: a reference no project could ever satisfy is a broken
/// pack, refused at load rather than at every record.
fn check_ref_targets(kinds: &[KindDef]) -> PackResult<()> {
    for def in kinds {
        for field in &def.fields {
            let Some(target) = &field.ref_kind else {
                continue;
            };
            let known =
                SYSTEM_KINDS.contains(&target.as_str()) || kinds.iter().any(|k| &k.kind == target);
            if !known {
                return Err(PackError::invalid(format!(
                    "{}.{} refs {target}, which is no core system kind and no kind of this pack",
                    def.kind, field.name
                )));
            }
        }
    }
    Ok(())
}

/// True when `kind` is a name the core owns.
fn is_core_kind(kind: &str) -> bool {
    kind == PACK_SYSTEM_KIND
        || kind.starts_with(CORE_PREFIX)
        || SYSTEM_KINDS.contains(&kind)
        || ASSET_KINDS.contains(&kind)
        || CORE_RECORD_KINDS.contains(&kind)
}

fn parse_kind(value: &Value, ids: &mut IdSpace<'_>) -> PackResult<KindDef> {
    let obj = closed_object(value, &["kind", "fields"], "kind")?;
    let kind = req_str(obj, "kind", "kind", MAX_ID_CHARS)?;
    ids.claim(kind, "kind")?;
    if is_core_kind(kind) {
        return Err(PackError::new(
            "pack_kind_collision",
            format!("kind {kind} is a core kind; a pack specialises, it never redefines"),
        ));
    }
    let mut names = BTreeSet::new();
    let fields = parse_list(obj, "fields", MAX_FIELDS, |v| {
        let f = closed_object(v, &["name", "type", "required", "ref"], "field")?;
        let name = req_str(f, "name", "field", MAX_ID_CHARS)?;
        if !is_id(name) {
            return Err(PackError::invalid(format!(
                "field name {name:?} is not an id"
            )));
        }
        if !names.insert(name.to_string()) {
            return Err(PackError::invalid(format!(
                "field {name} declared twice in {kind}"
            )));
        }
        let raw_type = req_str(f, "type", "field", MAX_ID_CHARS)?;
        let field_type = FieldType::parse(raw_type).ok_or_else(|| {
            PackError::invalid(format!("field {name} has unknown type {raw_type}"))
        })?;
        let required = f
            .get("required")
            .and_then(Value::as_bool)
            .ok_or_else(|| PackError::invalid(format!("field {name} needs a boolean required")))?;
        let ref_kind = match f.get("ref") {
            None => None,
            Some(_) => {
                let target = req_str(f, "ref", "field", MAX_ID_CHARS)?;
                if !matches!(field_type, FieldType::String | FieldType::Array) {
                    return Err(PackError::invalid(format!(
                        "field {name} refs {target} but is neither string nor array"
                    )));
                }
                Some(target.to_string())
            }
        };
        Ok(FieldDef {
            name: name.to_string(),
            field_type,
            required,
            ref_kind,
        })
    })?;
    if fields.is_empty() {
        return Err(PackError::invalid(format!(
            "kind {kind} declares no fields"
        )));
    }
    Ok(KindDef {
        kind: kind.to_string(),
        fields,
    })
}

fn parse_template(value: &Value, ids: &mut IdSpace<'_>) -> PackResult<Template> {
    let obj = closed_object(value, &["template_id", "system_kind", "spec"], "template")?;
    let template_id = req_str(obj, "template_id", "template", MAX_ID_CHARS)?;
    ids.claim(template_id, "template")?;
    let system_kind = req_str(obj, "system_kind", "template", MAX_ID_CHARS)?;
    if !SYSTEM_KINDS.contains(&system_kind) && system_kind != LEVEL_TEMPLATE {
        return Err(PackError::invalid(format!(
            "template {template_id} names {system_kind}, which is no core system kind nor level"
        )));
    }
    let spec = obj.get("spec").filter(|v| v.is_object()).ok_or_else(|| {
        PackError::invalid(format!("template {template_id} needs an object spec"))
    })?;
    Ok(Template {
        template_id: template_id.to_string(),
        system_kind: system_kind.to_string(),
        spec: spec.clone(),
    })
}

fn parse_media(value: &Value, formats: &mut BTreeSet<String>) -> PackResult<MediaEntry> {
    let obj = closed_object(value, &["format", "budget"], "media entry")?;
    let format = req_str(obj, "format", "media entry", MAX_ID_CHARS)?;
    if !LUMEN_FORMATS.contains(&format) {
        return Err(PackError::new(
            "pack_format_not_lumen",
            format!("media format {format} is not one lumen owns"),
        ));
    }
    if !formats.insert(format.to_string()) {
        return Err(PackError::invalid(format!(
            "media format {format} listed twice"
        )));
    }
    let budget_obj = obj
        .get("budget")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            PackError::invalid(format!("media format {format} needs a budget object"))
        })?;
    if budget_obj.is_empty() || budget_obj.len() > MAX_BUDGET_KEYS {
        return Err(PackError::invalid(format!(
            "media format {format} needs 1..={MAX_BUDGET_KEYS} budget entries"
        )));
    }
    let mut budget = Vec::with_capacity(budget_obj.len());
    for (key, v) in budget_obj {
        if !is_id(key) {
            return Err(PackError::invalid(format!(
                "budget key {key:?} is not an id"
            )));
        }
        let n = v.as_u64().ok_or_else(|| {
            PackError::invalid(format!(
                "budget {key} for {format} must be a non-negative integer"
            ))
        })?;
        budget.push((key.clone(), n));
    }
    Ok(MediaEntry {
        format: format.to_string(),
        budget,
    })
}

fn parse_named(value: &Value, id_key: &str, ids: &mut IdSpace<'_>) -> PackResult<Named> {
    let obj = closed_object(value, &[id_key, "description"], id_key)?;
    let id = req_str(obj, id_key, id_key, MAX_ID_CHARS)?;
    ids.claim(id, id_key)?;
    let description = req_str(obj, "description", id_key, MAX_TEXT_CHARS)?;
    Ok(Named {
        id: id.to_string(),
        description: description.to_string(),
    })
}

/// The value as an object whose keys are all in `allowed`.
fn closed_object<'v>(
    value: &'v Value,
    allowed: &[&str],
    what: &str,
) -> PackResult<&'v Map<String, Value>> {
    let obj = value
        .as_object()
        .ok_or_else(|| PackError::invalid(format!("{what} must be an object")))?;
    if let Some(key) = obj.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(PackError::new(
            "pack_unknown_field",
            format!("{what} has no field {key}; the pack format is closed"),
        ));
    }
    Ok(obj)
}

/// A required, non-empty string of at most `max` chars.
fn req_str<'v>(
    obj: &'v Map<String, Value>,
    key: &str,
    what: &str,
    max: usize,
) -> PackResult<&'v str> {
    let text = obj
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| PackError::invalid(format!("{what} needs a string {key}")))?;
    if text.is_empty() || text.chars().count() > max {
        return Err(PackError::invalid(format!(
            "{what} {key} must be 1..={max} chars"
        )));
    }
    Ok(text)
}

/// A required array of at most `max` items, each parsed by `item`.
fn parse_list<T>(
    obj: &Map<String, Value>,
    key: &str,
    max: usize,
    mut item: impl FnMut(&Value) -> PackResult<T>,
) -> PackResult<Vec<T>> {
    let arr = obj
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| PackError::invalid(format!("pack needs an array {key}")))?;
    if arr.len() > max {
        return Err(PackError::invalid(format!(
            "{key} has {} entries; the bound is {max}",
            arr.len()
        )));
    }
    arr.iter().map(&mut item).collect()
}

/// Lowercase snake_case: `[a-z][a-z0-9_]*`, at most `MAX_ID_CHARS`.
fn is_id(text: &str) -> bool {
    let mut chars = text.chars();
    text.len() <= MAX_ID_CHARS
        && chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `MAJOR.MINOR.PATCH`, each a run of ASCII digits.
fn is_semver(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.len() <= 9 && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::sha256_hex;
    use serde_json::json;

    pub(super) const FULL: &str = include_str!("../tests/fixtures/packs/full/pack.json");
    pub(super) const SKELETON: &str = include_str!("../tests/fixtures/packs/skeleton/pack.json");

    pub(super) fn registry(sources: &[(&str, &str)]) -> PackResult<Registry> {
        Registry::from_sources(sources)
    }

    /// A resolver for a project that holds no records.
    pub(super) fn no_refs(_kind: &str, _id: &str) -> bool {
        false
    }

    pub(super) const ROLE_PLAYING: &str = include_str!("../packs/role_playing/pack.json");

    /// A synthetic pack for `slug` with one kind, one template, one
    /// media entry, one generator and one gate, all ids prefixed by
    /// the slug so packs never collide with each other.
    pub(super) fn synthetic(slug: &str) -> String {
        json!({
            "schema": PACK_SCHEMA,
            "slug": slug,
            "version": "1.0.0",
            "kinds": [{"kind": format!("{slug}_piece"),
                       "fields": [{"name": "id", "type": "string", "required": true}]}],
            "systems_templates": [{"template_id": format!("{slug}_flags"),
                                   "system_kind": "flags", "spec": {}}],
            "media_profile": [{"format": "still_2d", "budget": {"max_assets": 8}}],
            "playtest_generators": [{"generator_id": format!("{slug}_walk"),
                                     "description": "walks it"}],
            "gates": [{"gate_id": format!("{slug}_gate"), "description": "gates it"}],
            "emitters": ["bevy-shell/1"]
        })
        .to_string()
    }

    /// Count and hash of every profile's served definitions.
    fn served_surface() -> Vec<(&'static str, usize, String)> {
        catalog::PROFILES
            .iter()
            .map(|profile| {
                let defs = catalog::tool_definitions(profile);
                let text = serde_json::to_string(&defs).expect("tool definitions serialize");
                (profile.name, defs.len(), sha256_hex(text.as_bytes()))
            })
            .collect()
    }

    #[test]
    fn the_pack_schema_validates_a_full_pack() {
        let reg = registry(&[("rhythm", FULL)]).expect("full fixture validates");
        let pack = &reg.packs[0];
        assert_eq!(pack.slug, "rhythm");
        assert_eq!(pack.version, "0.1.0");
        assert_eq!(pack.kinds.len(), 1);
        assert_eq!(pack.kinds[0].fields.len(), 5);
        assert_eq!(pack.systems_templates[0].system_kind, "economy");
        assert_eq!(pack.media_profile.len(), 2);
        assert_eq!(
            pack.media_profile[0].budget,
            vec![
                ("max_assets".to_string(), 64),
                ("max_bytes".to_string(), 67_108_864)
            ]
        );
        assert_eq!(pack.playtest_generators[0].id, "full_chart_pass");
        assert_eq!(pack.gates[0].id, "chart_sync");
        assert_eq!(pack.emitters, vec!["bevy-rust-content/1"]);
    }

    /// The PK-02 guard in miniature: a data-only skeleton with
    /// every section empty validates. Nothing genre-specific is
    /// required by the format.
    #[test]
    fn a_data_only_skeleton_validates() {
        let reg = registry(&[("strategy", SKELETON)]).expect("skeleton validates");
        assert!(reg.packs[0].kinds.is_empty());
    }

    #[test]
    fn the_compiled_packs_validate() {
        assert_eq!(check_compiled(), Ok(COMPILED.len()));
    }

    /// Every `packs/<slug>/pack.json` on disk is compiled in, and
    /// every compiled pack has its file: a pack cannot sit in the
    /// tree unvalidated.
    #[test]
    fn packs_on_disk_and_compiled_agree() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("packs");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .expect("packs/ exists")
            .filter_map(Result::ok)
            .filter(|e| e.path().join("pack.json").is_file())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        on_disk.sort();
        let mut compiled: Vec<String> = COMPILED.iter().map(|(s, _)| s.to_string()).collect();
        compiled.sort();
        assert_eq!(on_disk, compiled);
    }

    /// Every lumen format and every emitter this build carries is
    /// nameable by a pack.
    #[test]
    fn a_pack_may_name_every_lumen_format_and_emitter() {
        let media: Vec<Value> = LUMEN_FORMATS
            .iter()
            .map(|f| json!({"format": f, "budget": {"max_assets": 1}}))
            .collect();
        let doc = json!({"schema": PACK_SCHEMA, "slug": "action", "version": "2.3.4",
            "kinds": [], "systems_templates": [], "media_profile": media,
            "playtest_generators": [], "gates": [], "emitters": EMITTERS})
        .to_string();
        let reg = registry(&[("action", &doc)]).expect("every format and emitter is nameable");
        assert_eq!(reg.packs[0].media_profile.len(), LUMEN_FORMATS.len());
        assert_eq!(reg.packs[0].emitters.len(), EMITTERS.len());
    }

    #[test]
    fn lumen_formats_are_the_media_map_minus_voice() {
        let expected: Vec<&str> = ASSET_KINDS
            .iter()
            .copied()
            .filter(|k| *k != "voice")
            .collect();
        assert_eq!(LUMEN_FORMATS, expected.as_slice());
    }

    #[test]
    fn primary_and_secondary_packs_select_validators() {
        let a = synthetic("role_playing");
        let b = synthetic("educational");
        let reg = registry(&[("role_playing", &a), ("educational", &b)]).unwrap();
        let rec = json!({"id": "x"});
        let both = reg.select("role_playing", &["educational"]).unwrap();
        assert_eq!(both.validate("role_playing_piece", &rec, &no_refs), Ok(()));
        assert_eq!(both.validate("educational_piece", &rec, &no_refs), Ok(()));
        // A genre whose pack has not landed selects nothing, and is
        // not an error.
        let none = reg.select("platformer", &[]).unwrap();
        assert!(none.packs.is_empty());
    }

    #[test]
    fn a_record_of_a_selected_kind_validates() {
        let reg = registry(&[("rhythm", FULL)]).unwrap();
        let sel = reg.select("rhythm", &[]).unwrap();
        let required_only = json!({"chart_id": "c1", "bpm": 120, "notes": []});
        assert_eq!(sel.validate("beat_chart", &required_only, &no_refs), Ok(()));
        let with_optional = json!({"chart_id": "c1", "bpm": 120, "notes": [1, 2],
            "practice_only": true, "timing_window": {"ms": 40}});
        assert_eq!(sel.validate("beat_chart", &with_optional, &no_refs), Ok(()));
    }

    /// RPG-01: the Role Playing pack carries its whole v1 surface —
    /// FOE/encounter/genesis/media/migration kinds, floor and region
    /// templates (SYS-09 level layouts), a FOE behaviour table, the
    /// media profile, generators, gates — and every kind carries the
    /// version-migration fields.
    #[test]
    fn the_role_playing_pack_loads_with_its_full_surface() {
        let reg = Registry::compiled().expect("compiled packs validate");
        let pack = reg
            .packs
            .iter()
            .find(|p| p.slug == "role_playing")
            .expect("role_playing is compiled in");
        let kinds: Vec<&str> = pack.kinds.iter().map(|k| k.kind.as_str()).collect();
        assert_eq!(
            kinds,
            [
                "foe",
                "encounter",
                "encounter_table",
                "enemy_genesis_hook",
                "portrait_set",
                "combat_effect",
                "music_cue",
                "data_migration"
            ]
        );
        for def in pack.kinds.iter().filter(|k| k.kind != "data_migration") {
            let field = |name: &str| def.fields.iter().find(|f| f.name == name);
            let rev = field("schema_rev").expect("schema_rev");
            assert!(
                rev.required && rev.field_type == FieldType::Integer,
                "{}",
                def.kind
            );
            let from = field("migrated_from").expect("migrated_from");
            assert!(
                !from.required && from.field_type == FieldType::Object,
                "{}",
                def.kind
            );
        }
        let targets: Vec<(&str, &str)> = pack
            .systems_templates
            .iter()
            .map(|t| (t.template_id.as_str(), t.system_kind.as_str()))
            .collect();
        assert!(targets.contains(&("foe_behaviour", "state_machine")));
        assert!(targets.contains(&("floor_cave", LEVEL_TEMPLATE)));
        assert!(targets.contains(&("region_descent", LEVEL_TEMPLATE)));
        let formats: Vec<&str> = pack
            .media_profile
            .iter()
            .map(|m| m.format.as_str())
            .collect();
        assert_eq!(
            formats,
            ["pixel", "model_3d", "still_2d", "audio", "effect"]
        );
        assert!(
            pack.playtest_generators
                .iter()
                .any(|g| g.id == "floor_traversal")
        );
        assert_eq!(pack.emitters, ["bevy-rust-content/1"]);
    }

    #[test]
    fn a_ref_field_resolves_through_the_projects_records() {
        let reg = registry(&[("role_playing", ROLE_PLAYING)]).unwrap();
        let sel = reg.select("role_playing", &[]).unwrap();
        let held = |kind: &str, id: &str| kind == "foe" && (id == "wisp" || id == "warg");
        let rec = json!({"foes": ["wisp", "warg"], "weight": 3, "schema_rev": 1});
        assert_eq!(sel.validate("encounter", &rec, &held), Ok(()));
        let hook = json!({"foe": "wisp", "variant_table": "roguelike:wisp-variants",
            "schema_rev": 1});
        assert_eq!(sel.validate("enemy_genesis_hook", &hook, &held), Ok(()));
    }

    /// RPG-01's carrier keeps the invariant: `meister_system_define`
    /// gains one fixed enum value, never a pack's kind names, and no
    /// compiled pack kind appears anywhere in any served definition.
    #[test]
    fn the_carrier_enum_is_fixed_and_names_no_pack_kind() {
        let specs = catalog::specs();
        let spec = specs
            .iter()
            .find(|s| s.name == "meister_system_define")
            .expect("carrier exists");
        let mut expected: Vec<&str> = SYSTEM_KINDS.to_vec();
        expected.push(PACK_SYSTEM_KIND);
        assert_eq!(spec.input["properties"]["kind"]["enum"], json!(expected));
        let served = serde_json::to_string(&catalog::tool_definitions(
            catalog::PROFILES.last().expect("full profile"),
        ))
        .unwrap();
        let reg = Registry::compiled().unwrap();
        for def in reg.packs.iter().flat_map(|p| p.kinds.iter()) {
            assert!(
                !served.contains(&format!("\"{}\"", def.kind)),
                "{}",
                def.kind
            );
        }
    }

    /// The catalog invariant (PK-01's gate): the served surface —
    /// tool count and every definition's bytes, in every profile —
    /// is the same with zero packs and with one pack per genre,
    /// while the packs themselves really did grow.
    #[test]
    fn catalog_invariant_tool_count_does_not_depend_on_packs() {
        let baseline = served_surface();
        let full_count = catalog::specs().len();
        let texts: Vec<(&str, String)> = GENRE_PACKS
            .iter()
            .filter(|slug| !COMPILED.iter().any(|(s, _)| s == *slug))
            .map(|slug| (*slug, synthetic(slug)))
            .collect();
        let mut last_kinds = 0;
        for n in 0..=texts.len() {
            let mut sources: Vec<(&str, &str)> = COMPILED.to_vec();
            sources.extend(texts[..n].iter().map(|(s, t)| (*s, t.as_str())));
            let reg = registry(&sources).expect("synthetic packs validate");
            assert_eq!(reg.packs.len(), COMPILED.len() + n);
            let kinds: usize = reg.packs.iter().map(|p| p.kinds.len()).sum();
            assert!(n == 0 || kinds > last_kinds, "packs grew at n={n}");
            last_kinds = kinds;
            assert_eq!(catalog::specs().len(), full_count, "n={n}");
            assert_eq!(served_surface(), baseline, "n={n}");
        }
        assert_eq!(texts.len() + COMPILED.len(), GENRE_PACKS.len());
    }
}

/// The refusal half (`adv_`): every refusal is typed.
#[cfg(test)]
mod adversarial {
    use super::tests::{FULL, ROLE_PLAYING, no_refs, registry, synthetic};
    use super::*;
    use serde_json::json;

    fn code(text: &str, slug: &str) -> &'static str {
        registry(&[(slug, text)]).expect_err("must be refused").code
    }

    /// The full fixture with one edit applied.
    fn edited(edit: impl FnOnce(&mut Value)) -> String {
        let mut doc: Value = serde_json::from_str(FULL).unwrap();
        edit(&mut doc);
        doc.to_string()
    }

    #[test]
    fn adv_a_pack_declaring_a_tool_is_refused() {
        let top = edited(|d| {
            d["tools"] = json!([{"name": "rhythm_score", "description": "x"}]);
        });
        assert_eq!(code(&top, "rhythm"), "pack_declares_tool");
        // Buried in a template spec, under an MCP-shaped key.
        let nested = edited(|d| {
            d["systems_templates"][0]["spec"]["inputSchema"] = json!({"type": "object"});
        });
        assert_eq!(code(&nested, "rhythm"), "pack_declares_tool");
    }

    #[test]
    fn adv_a_pack_id_naming_a_catalog_tool_is_refused() {
        let kind = edited(|d| d["kinds"][0]["kind"] = json!("meister_emit"));
        assert_eq!(code(&kind, "rhythm"), "pack_declares_tool");
        let gate = edited(|d| d["gates"][0]["gate_id"] = json!("meister_lane_check"));
        assert_eq!(code(&gate, "rhythm"), "pack_declares_tool");
    }

    #[test]
    fn adv_a_pack_kind_colliding_with_a_core_kind_is_refused() {
        for core in ["state_machine", "still_2d", "dialogue", "meister_floor"] {
            let doc = edited(|d| d["kinds"][0]["kind"] = json!(core));
            assert_eq!(code(&doc, "rhythm"), "pack_kind_collision", "{core}");
        }
    }

    #[test]
    fn adv_two_packs_declaring_one_kind_are_refused() {
        let other = synthetic("puzzle").replace("puzzle_piece", "beat_chart");
        let err = registry(&[("rhythm", FULL), ("puzzle", &other)]).unwrap_err();
        assert_eq!(err.code, "pack_kind_collision");
    }

    #[test]
    fn adv_a_media_format_lumen_does_not_own_is_refused() {
        for format in ["haptics", "voice"] {
            let doc = edited(|d| d["media_profile"][0]["format"] = json!(format));
            assert_eq!(code(&doc, "rhythm"), "pack_format_not_lumen", "{format}");
        }
    }

    #[test]
    fn adv_a_kind_no_selected_pack_declares_is_refused() {
        let reg = registry(&[("rhythm", FULL)]).unwrap();
        let rec = json!({"chart_id": "c1", "bpm": 120, "notes": []});
        // The pack exists, but this project did not select it.
        let sel = reg.select("puzzle", &["strategy"]).unwrap();
        let err = sel.validate("beat_chart", &rec, &no_refs).unwrap_err();
        assert_eq!(err.code, "pack_kind_not_selected");
        // A slug that is no genre cannot be selected at all.
        assert_eq!(
            reg.select("rhythm", &["haptic_pack"]).unwrap_err().code,
            "pack_invalid"
        );
    }

    #[test]
    fn adv_a_record_breaking_its_kind_is_refused() {
        let reg = registry(&[("rhythm", FULL)]).unwrap();
        let sel = reg.select("rhythm", &[]).unwrap();
        for bad in [
            json!({"chart_id": "c1", "notes": []}),
            json!({"chart_id": "c1", "bpm": 120.5, "notes": []}),
            json!({"chart_id": "c1", "bpm": 120, "notes": [], "cheat": 1}),
            json!(["not", "an", "object"]),
        ] {
            let err = sel.validate("beat_chart", &bad, &no_refs).unwrap_err();
            assert_eq!(err.code, "record_invalid", "{bad}");
        }
    }

    #[test]
    fn adv_a_pack_bringing_its_own_emitter_is_refused() {
        let doc = edited(|d| d["emitters"] = json!(["rhythm-native/1"]));
        assert_eq!(code(&doc, "rhythm"), "pack_invalid");
    }

    /// A `ref` no project could satisfy (an unknown kind), or on a
    /// field that cannot hold an id, is a broken pack.
    #[test]
    fn adv_a_ref_to_an_undeclared_kind_is_refused() {
        let doc: Value = serde_json::from_str(ROLE_PLAYING).unwrap();
        let mut unknown = doc.clone();
        unknown["kinds"][1]["fields"][0]["ref"] = json!("dragon");
        let err = registry(&[("role_playing", &unknown.to_string())]).unwrap_err();
        assert_eq!(err.code, "pack_invalid");
        assert!(err.message.contains("dragon"), "{}", err.message);
        let mut on_int = doc;
        on_int["kinds"][0]["fields"][2]["ref"] = json!("foe");
        let err = registry(&[("role_playing", &on_int.to_string())]).unwrap_err();
        assert_eq!(err.code, "pack_invalid");
        // The carrier's own kind name is reserved.
        let pack_kind = edited(|d| d["kinds"][0]["kind"] = json!(PACK_SYSTEM_KIND));
        assert_eq!(code(&pack_kind, "rhythm"), "pack_kind_collision");
    }

    #[test]
    fn adv_a_dangling_or_malformed_ref_in_a_record_is_refused() {
        let reg = registry(&[("role_playing", ROLE_PLAYING)]).unwrap();
        let sel = reg.select("role_playing", &[]).unwrap();
        let held = |kind: &str, id: &str| kind == "foe" && id == "wisp";
        let dangling = json!({"foes": ["wisp", "ghost"], "weight": 1, "schema_rev": 1});
        let err = sel.validate("encounter", &dangling, &held).unwrap_err();
        assert_eq!(err.code, "graph_dangling_ref");
        assert!(err.message.contains("ghost"), "{}", err.message);
        // Same id, wrong kind: a FOE id is not a behaviour table.
        let foe = json!({"display_name": "Wisp", "behaviour": "wisp", "tier": 1,
            "schema_rev": 1});
        assert_eq!(
            sel.validate("foe", &foe, &held).unwrap_err().code,
            "graph_dangling_ref"
        );
        let not_ids = json!({"foes": [7], "weight": 1, "schema_rev": 1});
        assert_eq!(
            sel.validate("encounter", &not_ids, &held).unwrap_err().code,
            "record_invalid"
        );
        let too_many: Vec<&str> = vec!["wisp"; MAX_REF_ITEMS + 1];
        let flood = json!({"foes": too_many, "weight": 1, "schema_rev": 1});
        assert_eq!(
            sel.validate("encounter", &flood, &held).unwrap_err().code,
            "record_invalid"
        );
    }

    #[test]
    fn adv_malformed_packs_are_refused_with_typed_codes() {
        let unknown = edited(|d| d["scripts"] = json!(["build.rs"]));
        assert_eq!(code(&unknown, "rhythm"), "pack_unknown_field");
        let not_genre = edited(|d| d["slug"] = json!("haptic_pack"));
        assert_eq!(code(&not_genre, "haptic_pack"), "pack_invalid");
        assert_eq!(code(FULL, "puzzle"), "pack_invalid", "slug/dir mismatch");
        let schema = edited(|d| d["schema"] = json!("meister-pack/2"));
        assert_eq!(code(&schema, "rhythm"), "pack_invalid");
        let float_budget = edited(|d| d["media_profile"][0]["budget"]["max_assets"] = json!(1.5));
        assert_eq!(code(&float_budget, "rhythm"), "pack_invalid");
        let mut deep = json!(0);
        for _ in 0..=MAX_DEPTH + 1 {
            deep = json!([deep]);
        }
        let too_deep = edited(|d| d["systems_templates"][0]["spec"]["x"] = deep);
        assert_eq!(code(&too_deep, "rhythm"), "pack_invalid");
        let huge = " ".repeat(MAX_PACK_BYTES + 1);
        assert_eq!(code(&huge, "rhythm"), "pack_too_large");
        assert_eq!(code("{not json", "rhythm"), "pack_invalid");
    }
}
