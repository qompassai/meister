//! The tool catalog: the full Meister surface as typed seams.
//!
//! Derived from README §"Tool groups" — ten groups mirroring the
//! maker surface's shape, re-homed to the SFW lane — plus the one
//! live tool, `meister_lane_check`. Every tool is born with a
//! complete input schema (explicit `properties`, always), an
//! output schema for its success shape, and behaviour annotations.
//! Handlers are seams until their rung lands (README §"The path to
//! 1.0"); the seam error names the rung that makes the tool real.
//! Rung 1 landed the Group 1–2 handlers (see `store`); the seam
//! count shrinks rung by rung.
//!
//! Record-carrying tools take an `envelope` argument — the lane
//! stamp of the record they carry (`record_id`, `origin_lane`,
//! `content_label`, `evidence`). The envelope's `content_label` is
//! deliberately NOT an enum in the schema: the lane gate, not the
//! schema validator, owns classification, so a bad label reaches
//! the gate and is refused with a typed lane code instead of
//! failing as a protocol error.

use std::sync::Arc;

use rmcp::model::{Tool, ToolAnnotations};
use serde_json::{Map, Value, json};

/// One catalog entry: everything dispatch and discovery need.
pub struct ToolSpec {
    /// Wire name (`meister_*`).
    pub name: &'static str,
    /// README tool group (1–10); 0 for the cross-cutting lane tool.
    pub group: u8,
    /// The rung that makes this tool real (0 = live at birth).
    pub rung: u8,
    /// True when the tool's input carries a record envelope and the
    /// lane gate runs before the handler.
    pub record_carrying: bool,
    /// One-line description, as served.
    pub description: &'static str,
    /// Complete input schema (object with explicit `properties`).
    pub input: Value,
    /// Output schema for the success shape.
    pub output: Value,
    /// Behaviour annotations, from birth.
    pub annotations: ToolAnnotations,
}

fn s() -> Value {
    json!({"type": "string"})
}

fn s_enum(values: &[&str]) -> Value {
    json!({"type": "string", "enum": values})
}

fn integer() -> Value {
    json!({"type": "integer"})
}

fn boolean() -> Value {
    json!({"type": "boolean"})
}

fn arr(items: Value) -> Value {
    json!({"type": "array", "items": items})
}

fn free_obj() -> Value {
    json!({"type": "object"})
}

/// Build an object schema from (name, schema) pairs and the names
/// of the required ones.
fn obj_schema(props: &[(&str, Value)], required: &[&str]) -> Value {
    let mut map = Map::new();
    for (name, schema) in props {
        map.insert((*name).to_string(), schema.clone());
    }
    let mut schema = json!({"type": "object", "properties": Value::Object(map)});
    if !required.is_empty() {
        schema["required"] = json!(required);
    }
    schema
}

/// The lane-stamp envelope schema shared by every record-carrying
/// tool. See the module docs for why `content_label` is a plain
/// string here.
fn envelope_schema() -> Value {
    obj_schema(
        &[
            ("record_id", json!({"type": "string", "minLength": 1})),
            ("origin_lane", s()),
            ("content_label", s()),
            ("evidence", s()),
        ],
        &["record_id", "origin_lane", "content_label", "evidence"],
    )
}

/// Input schema for a record-carrying tool: the envelope plus the
/// tool's own required/optional properties.
fn record_input(props: &[(&str, Value)], required: &[&str]) -> Value {
    let mut all: Vec<(&str, Value)> = vec![("envelope", envelope_schema())];
    all.extend_from_slice(props);
    let mut req: Vec<&str> = vec!["envelope"];
    req.extend_from_slice(required);
    obj_schema(&all, &req)
}

/// Input schema for a read tool (no record carried, no gate).
fn read_input(props: &[(&str, Value)], required: &[&str]) -> Value {
    obj_schema(props, required)
}

/// Output schema: an object with the named properties, all required
/// unless the caller says otherwise by construction.
fn out_schema(props: &[(&str, Value)]) -> Value {
    let required: Vec<&str> = props.iter().map(|(name, _)| *name).collect();
    obj_schema(props, &required)
}

fn read_ann() -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(true)
        .destructive(false)
        .idempotent(true)
        .open_world(false)
}

fn read_open_ann() -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(true)
        .destructive(false)
        .idempotent(true)
        .open_world(true)
}

fn write_ann() -> ToolAnnotations {
    ToolAnnotations::new()
        .read_only(false)
        .destructive(false)
        .idempotent(false)
        .open_world(false)
}

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

/// The full catalog, in group order. Count: 31 tools (30 group
/// seams + the live lane check). The >25 count carries its written
/// reason in COMPLIANCE.md (Tiger Style MCP Rule 1).
pub fn specs() -> Vec<ToolSpec> {
    vec![
        // ---- Group 1: Project scaffolding (Rung 1) ----
        ToolSpec {
            name: "meister_project_create",
            group: 1,
            rung: 1,
            record_carrying: true,
            description: "Create a Qompass game project with estate conventions and its genre pipeline pack selected at birth.",
            input: record_input(
                &[
                    ("name", s()),
                    (
                        "engine",
                        s_enum(&["bevy", "godot", "renpy", "unity", "unreal"]),
                    ),
                    ("genre_pack", s_enum(GENRE_PACKS)),
                    ("project_id", s()),
                    ("secondary_genre_packs", arr(s_enum(GENRE_PACKS))),
                    ("canon", arr(free_obj())),
                ],
                &["name", "engine", "genre_pack"],
            ),
            output: out_schema(&[
                ("project_id", s()),
                ("engine", s()),
                ("genre_pack", s()),
                ("secondary_genre_packs", arr(s())),
                ("canon_fact_count", integer()),
                ("status", s()),
                ("project_path", s()),
            ]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_project_inspect",
            group: 1,
            rung: 1,
            record_carrying: false,
            description: "Inspect one project record: engine shell, metadata, lane stamps, and cast and canon counts computed from the record set.",
            input: read_input(&[("project_id", s())], &["project_id"]),
            output: out_schema(&[
                ("project_id", s()),
                ("name", s()),
                ("engine", s()),
                ("genre_pack", s()),
                ("secondary_genre_packs", arr(s())),
                ("origin_lane", s()),
                ("content_label", s()),
                ("status", s()),
                ("cast_count", integer()),
                ("canon_fact_count", integer()),
                ("project_path", s()),
            ]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_project_list",
            group: 1,
            rung: 1,
            record_carrying: false,
            description: "List projects, bounded: limit defaults to 25, maximum 100, with a continuation cursor.",
            input: read_input(
                &[
                    (
                        "limit",
                        json!({"type": "integer", "minimum": 1, "maximum": 100, "default": 25}),
                    ),
                    ("cursor", s()),
                ],
                &[],
            ),
            output: out_schema(&[
                ("projects", arr(free_obj())),
                ("count", integer()),
                ("next_cursor", json!({"type": ["string", "null"]})),
            ]),
            annotations: read_ann(),
        },
        // ---- Group 2: Cast and canon (Rung 1) ----
        ToolSpec {
            name: "meister_cast_add",
            group: 2,
            rung: 1,
            record_carrying: true,
            description: "Add a character identity record to a project's cast, born candidate and lane-stamped.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("character_id", s()),
                    ("identity", free_obj()),
                ],
                &["project_id", "character_id", "identity"],
            ),
            output: out_schema(&[
                ("project_id", s()),
                ("character_id", s()),
                ("status", s()),
                ("governing_reference_count", integer()),
                ("record_path", s()),
            ]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_cast_lock",
            group: 2,
            rung: 1,
            record_carrying: true,
            description: "Identity-lock a character at intake: silhouette, palette, face and body proportions, thumbnail recognisability. A hard gate before any detail work.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("character_id", s()),
                    ("lock", free_obj()),
                ],
                &["project_id", "character_id", "lock"],
            ),
            output: out_schema(&[
                ("project_id", s()),
                ("character_id", s()),
                ("locked", boolean()),
                ("lock_hash", s()),
                ("record_path", s()),
            ]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_canon_check",
            group: 2,
            rung: 1,
            record_carrying: false,
            description: "Run the canon census over a project: every claim recomputed from its source artifact and SHA-256 (D-713); any unverified claim fails the census closed with the full census in the error body. Declares no visual PASS; the Owner's eye is the verdict.",
            input: read_input(&[("project_id", s())], &["project_id"]),
            output: out_schema(&[
                ("project_id", s()),
                ("checked", integer()),
                ("unverified", integer()),
                ("verdict", s()),
                ("counts", free_obj()),
                ("checks", arr(free_obj())),
                ("failures", arr(s())),
            ]),
            annotations: read_ann(),
        },
        // ---- Group 3: Scenes and systems authoring (Rung 2) ----
        ToolSpec {
            name: "meister_scene_define",
            group: 3,
            rung: 2,
            record_carrying: true,
            description: "Define a scene graph record: nodes, dialogue links, and composition for one scene.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("scene_id", s()),
                    ("graph", free_obj()),
                ],
                &["project_id", "scene_id", "graph"],
            ),
            output: out_schema(&[("scene_id", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_system_define",
            group: 3,
            rung: 2,
            record_carrying: true,
            description: "Define a game-systems record: schedule, flags, progression DAG, economy, state machine, or minigame, as a validated bounded record.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("system_id", s()),
                    (
                        "kind",
                        s_enum(&[
                            "schedule",
                            "flags",
                            "progression_dag",
                            "economy",
                            "state_machine",
                            "minigame",
                        ]),
                    ),
                    ("spec", free_obj()),
                ],
                &["project_id", "system_id", "kind", "spec"],
            ),
            output: out_schema(&[("system_id", s()), ("kind", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_scene_validate",
            group: 3,
            rung: 2,
            record_carrying: false,
            description: "Validate an authored scene record: malformed shapes are typed refusals, never crashes.",
            input: read_input(
                &[("project_id", s()), ("scene_id", s())],
                &["project_id", "scene_id"],
            ),
            output: out_schema(&[
                ("scene_id", s()),
                ("valid", boolean()),
                ("refusals", arr(s())),
            ]),
            annotations: read_ann(),
        },
        // ---- Group 4: Dialogue and narrative creation (Rung 3) ----
        ToolSpec {
            name: "meister_dialogue_author",
            group: 4,
            rung: 3,
            record_carrying: true,
            description: "Author dialogue as structured game data: lines, speakers, choices, and conditions — never loose prose.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("dialogue_id", s()),
                    ("lines", arr(free_obj())),
                ],
                &["project_id", "dialogue_id", "lines"],
            ),
            output: out_schema(&[("dialogue_id", s()), ("line_count", integer())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_dialogue_get",
            group: 4,
            rung: 3,
            record_carrying: false,
            description: "Read one authored dialogue record with its branching and continuity effects.",
            input: read_input(
                &[("project_id", s()), ("dialogue_id", s())],
                &["project_id", "dialogue_id"],
            ),
            output: out_schema(&[("dialogue_id", s()), ("lines", arr(free_obj()))]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_reaction_set",
            group: 4,
            rung: 3,
            record_carrying: true,
            description: "Set a character's whole-body reaction profile for an emotion: posture, gesture, and stance — expression is never only a face.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("character_id", s()),
                    ("emotion", s()),
                    ("posture", s()),
                    ("gesture", s()),
                    ("stance", s()),
                ],
                &[
                    "project_id",
                    "character_id",
                    "emotion",
                    "posture",
                    "gesture",
                    "stance",
                ],
            ),
            output: out_schema(&[("character_id", s()), ("emotion", s()), ("status", s())]),
            annotations: write_ann(),
        },
        // ---- Group 5: Story, plot, and level making (Rung 3) ----
        ToolSpec {
            name: "meister_story_define",
            group: 5,
            rung: 3,
            record_carrying: true,
            description: "Define a story arc record: beats, quests and objectives, character arcs, and branching endings.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("arc_id", s()),
                    ("beats", arr(free_obj())),
                ],
                &["project_id", "arc_id", "beats"],
            ),
            output: out_schema(&[("arc_id", s()), ("beat_count", integer())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_level_compose",
            group: 5,
            rung: 3,
            record_carrying: true,
            description: "Compose a level record: layout and scene composition, triggers, schedules, progression gates, and minigame placement.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("level_id", s()),
                    ("layout", free_obj()),
                    ("triggers", arr(free_obj())),
                ],
                &["project_id", "level_id", "layout"],
            ),
            output: out_schema(&[("level_id", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_playtest_generate",
            group: 5,
            rung: 3,
            record_carrying: false,
            description: "Generate deterministic playthroughs from the authored graph: every ending reached or the gap reported, never asserted.",
            input: read_input(
                &[("project_id", s()), ("graph_ref", s())],
                &["project_id", "graph_ref"],
            ),
            output: out_schema(&[("playthroughs", integer()), ("endings_reached", integer())]),
            annotations: ToolAnnotations::new()
                .read_only(false)
                .destructive(false)
                .idempotent(true)
                .open_world(false),
        },
        // ---- Group 6: Content standards and accessibility (Rung 6) ----
        ToolSpec {
            name: "meister_rating_record",
            group: 6,
            rung: 6,
            record_carrying: true,
            description: "Record rating metadata for a project under one scheme (ESRB, PEGI, IARC) with its content descriptors.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("scheme", s_enum(&["esrb", "pegi", "iarc"])),
                    ("rating", s()),
                    ("descriptors", arr(s())),
                ],
                &["project_id", "scheme", "rating"],
            ),
            output: out_schema(&[("project_id", s()), ("scheme", s()), ("rating", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_accessibility_check",
            group: 6,
            rung: 6,
            record_carrying: false,
            description: "Check a project's accessibility surfaces: photosafety flash and transition-rate hard limits, colour-vision-safe palettes, subtitle and text-scaling contracts.",
            input: read_input(&[("project_id", s())], &["project_id"]),
            output: out_schema(&[
                ("project_id", s()),
                ("photosafety", s()),
                ("palette", s()),
                ("subtitles", s()),
                ("findings", arr(s())),
            ]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_content_summary",
            group: 6,
            rung: 6,
            record_carrying: false,
            description: "Read a project's plain-language summary of what the game contains, with its content tags.",
            input: read_input(&[("project_id", s())], &["project_id"]),
            output: out_schema(&[("project_id", s()), ("summary", s()), ("tags", arr(s()))]),
            annotations: read_ann(),
        },
        // ---- Group 7: Asset routing and provenance (Rung 5) ----
        ToolSpec {
            name: "meister_asset_intake",
            group: 7,
            rung: 5,
            record_carrying: true,
            description: "Intake one media artifact: hash, provenance (including art-tool provenance per asset), and routing record. Specialist work stays with its owner.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("asset_id", s()),
                    (
                        "sha256",
                        json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
                    ),
                    ("kind", s()),
                    ("provenance", free_obj()),
                ],
                &["project_id", "asset_id", "sha256", "kind", "provenance"],
            ),
            output: out_schema(&[("asset_id", s()), ("sha256", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_asset_route",
            group: 7,
            rung: 5,
            record_carrying: true,
            description: "Record a handoff of one asset to its owning specialist: lumen (media), eigenstyle (voice style), bloch (lessons), fathom (clean-room study). Meister re-implements nothing.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("asset_id", s()),
                    (
                        "target",
                        s_enum(&["lumen", "eigenstyle", "bloch", "fathom"]),
                    ),
                    ("purpose", s()),
                ],
                &["project_id", "asset_id", "target", "purpose"],
            ),
            output: out_schema(&[("asset_id", s()), ("target", s()), ("handoff_id", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_asset_chain_get",
            group: 7,
            rung: 5,
            record_carrying: false,
            description: "Inspect one asset's keeper chain: governing reference, candidate history, gate reference. An incomplete chain is not a keeper.",
            input: read_input(
                &[("project_id", s()), ("asset_id", s())],
                &["project_id", "asset_id"],
            ),
            output: out_schema(&[
                ("asset_id", s()),
                ("chain_complete", boolean()),
                ("chain", free_obj()),
            ]),
            annotations: read_ann(),
        },
        // ---- Group 8: Playtest and QA (Rung 6) ----
        ToolSpec {
            name: "meister_playtest_define",
            group: 8,
            rung: 6,
            record_carrying: true,
            description: "Define a scripted playthrough: half validation, half adversarial, by construction (the estate test split).",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("playtest_id", s()),
                    ("script", free_obj()),
                ],
                &["project_id", "playtest_id", "script"],
            ),
            output: out_schema(&[("playtest_id", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_replay_check",
            group: 8,
            rung: 6,
            record_carrying: false,
            description: "Check a deterministic replay against its recorded hash: same inputs, byte-identical run, or the divergence reported.",
            input: read_input(
                &[("project_id", s()), ("playtest_id", s())],
                &["project_id", "playtest_id"],
            ),
            output: out_schema(&[
                ("playtest_id", s()),
                ("replay_hash", s()),
                ("matches", boolean()),
            ]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_defect_record",
            group: 8,
            rung: 6,
            record_carrying: true,
            description: "Record a defect tied to authored project state, so a failure names the record it belongs to.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("defect_id", s()),
                    ("summary", s()),
                    ("state_ref", s()),
                ],
                &["project_id", "defect_id", "summary", "state_ref"],
            ),
            output: out_schema(&[("defect_id", s()), ("status", s())]),
            annotations: write_ann(),
        },
        // ---- Group 9: Build and release handoff (Rung 8) ----
        ToolSpec {
            name: "meister_channel_list",
            group: 9,
            rung: 8,
            record_carrying: false,
            description: "List the SFW channel-target registry: platforms, content-policy notes, automation class, and handoff executor per channel. Capability, not intent.",
            input: read_input(&[], &[]),
            output: out_schema(&[("channels", arr(free_obj())), ("count", integer())]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_release_prepare",
            group: 9,
            rung: 8,
            record_carrying: true,
            description: "Prepare a channel handoff for a project and hand it to slipway or the channel executor. Meister never forks slipway's publish gates.",
            input: record_input(
                &[("project_id", s()), ("channel", s())],
                &["project_id", "channel"],
            ),
            output: out_schema(&[("project_id", s()), ("channel", s()), ("handoff", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_preflight_run",
            group: 9,
            rung: 8,
            record_carrying: false,
            description: "Run a publish preflight for one channel: refuses the handoff when the channel's auth is missing or unverified.",
            input: read_input(
                &[("project_id", s()), ("channel", s())],
                &["project_id", "channel"],
            ),
            output: out_schema(&[("channel", s()), ("auth_status", s()), ("verdict", s())]),
            annotations: read_open_ann(),
        },
        // ---- Group 10: Channel credentials and sessions (Rung 8) ----
        ToolSpec {
            name: "meister_account_register",
            group: 10,
            rung: 8,
            record_carrying: true,
            description: "Register a channel account by credential REFERENCE: channel, account identity, credential kind, and where the value lives (a pass path or Secure Vault entry). Values are never accepted, stored, or returned.",
            input: record_input(
                &[
                    ("channel", s()),
                    ("account_id", s()),
                    ("credential_kind", s()),
                    ("value_location", s()),
                ],
                &["channel", "account_id", "credential_kind", "value_location"],
            ),
            output: out_schema(&[("channel", s()), ("account_id", s()), ("status", s())]),
            annotations: write_ann(),
        },
        ToolSpec {
            name: "meister_account_inspect",
            group: 10,
            rung: 8,
            record_carrying: false,
            description: "Inspect a channel account record: scopes, expiry, and last-verified time. References and status only, never values.",
            input: read_input(
                &[("channel", s()), ("account_id", s())],
                &["channel", "account_id"],
            ),
            output: out_schema(&[
                ("channel", s()),
                ("account_id", s()),
                ("scopes", arr(s())),
                ("expiry", s()),
                ("last_verified", s()),
            ]),
            annotations: read_ann(),
        },
        ToolSpec {
            name: "meister_account_verify",
            group: 10,
            rung: 8,
            record_carrying: false,
            description: "Run the channel's verification probe for one account, where the channel supports one; channels with no API get honest login-reference records instead.",
            input: read_input(
                &[("channel", s()), ("account_id", s())],
                &["channel", "account_id"],
            ),
            output: out_schema(&[
                ("channel", s()),
                ("verified", boolean()),
                ("checked_at", s()),
            ]),
            annotations: read_open_ann(),
        },
        // ---- Cross-cutting: the lane gate, live at Rung 0 ----
        ToolSpec {
            name: "meister_lane_check",
            group: 0,
            rung: 0,
            record_carrying: false,
            description: "Run the lane gate over a candidate record envelope and return its verdict (allow, or refuse with the typed lane code). Stores nothing; this is the Rung 0 gate, demonstrable.",
            input: read_input(
                &[
                    ("record_id", s()),
                    ("origin_lane", s()),
                    ("content_label", s()),
                    ("evidence", s()),
                ],
                &["origin_lane", "content_label", "evidence"],
            ),
            output: out_schema(&[
                ("verdict", s_enum(&["allow", "refuse"])),
                (
                    "refusal_code",
                    json!({"type": ["string", "null"], "enum": ["lane_mismatch", "lane_unclassified", null]}),
                ),
                ("reason", s()),
            ]),
            annotations: read_ann(),
        },
    ]
}

/// The served tool definitions, built from the catalog.
pub fn tool_definitions() -> Vec<Tool> {
    specs()
        .into_iter()
        .map(|spec| {
            let Value::Object(input_map) = spec.input else {
                unreachable!("catalog input schemas are objects");
            };
            let Value::Object(output_map) = spec.output else {
                unreachable!("catalog output schemas are objects");
            };
            let mut tool = Tool::new(spec.name, spec.description, input_map)
                .with_annotations(spec.annotations);
            tool.output_schema = Some(Arc::new(output_map));
            tool
        })
        .collect()
}

/// Look up one catalog entry by wire name.
pub fn spec_named(name: &str) -> Option<ToolSpec> {
    specs().into_iter().find(|spec| spec.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_complete_and_bounded() {
        let all = specs();
        assert_eq!(all.len(), 31);
        // Every group 1..=10 carries at least two tools.
        for group in 1..=10u8 {
            let count = all.iter().filter(|s| s.group == group).count();
            assert!(count >= 2, "group {group} has {count} tools");
        }
        // Names are unique and prefixed.
        let mut names: Vec<&str> = all.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 31);
        assert!(names.iter().all(|n| n.starts_with("meister_")));
    }

    #[test]
    fn every_schema_is_an_object_with_properties() {
        for spec in specs() {
            for schema in [&spec.input, &spec.output] {
                assert_eq!(schema["type"], "object", "{} schema", spec.name);
                assert!(
                    schema.get("properties").is_some_and(Value::is_object),
                    "{} schema lacks explicit properties",
                    spec.name
                );
            }
        }
    }

    #[test]
    fn record_carrying_tools_require_the_envelope() {
        for spec in specs() {
            if spec.record_carrying {
                let required = spec.input["required"].as_array().expect("required list");
                assert!(
                    required.iter().any(|r| r == "envelope"),
                    "{} is record-carrying but does not require envelope",
                    spec.name
                );
            }
        }
    }
}
