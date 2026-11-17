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
    /// The base profiles that carry this tool (see `PROFILES`):
    /// never empty, never a bundle, never `full`. Double entry with
    /// `PROFILES`; `check_profiles` holds the two in agreement.
    pub profiles: &'static [&'static str],
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

// ---- Surface profiles (EFLO-02) ----
//
// The catalog is served in profiles so a session loads the tools its
// task triggers, not the whole surface (COMPLIANCE.md §3, EFLO.md).
// Deterministic config decides what CAN load; a profile grants no
// authority — the lane gate and the handlers still decide what is
// permitted.

pub const PROFILE_CORE: &str = "core";
pub const PROFILE_CANON: &str = "canon";
pub const PROFILE_SYSTEMS: &str = "systems";
pub const PROFILE_NARRATIVE: &str = "narrative";
pub const PROFILE_ASSETS: &str = "assets";
pub const PROFILE_QA: &str = "qa";
pub const PROFILE_RELEASE: &str = "release";
pub const PROFILE_NIFLHEIM_FLOOR: &str = "niflheim-floor";
pub const PROFILE_FULL: &str = "full";

/// How a profile's served set is declared.
pub enum ProfileKind {
    /// The `core` tools plus the profile's own `tools`. `core` itself
    /// is the base profile whose own tools are the core tools.
    Base { tools: &'static [&'static str] },
    /// The union of the named base profiles. A bundle never names
    /// `release` (credential-adjacent, opt-in only), another bundle,
    /// or `full`.
    Bundle { members: &'static [&'static str] },
    /// Every catalog tool. For conformance and audit only.
    Full,
}

/// One declared surface profile.
pub struct Profile {
    /// The value `MEISTER_PROFILE` selects it by.
    pub name: &'static str,
    pub kind: ProfileKind,
}

/// The declared profiles, in serving order: `core` first (the
/// default), `full` last. Membership is the plan of record's
/// (docs/PATH-TO-1.0.md §5, EFLO-02) over the tools that exist.
pub const PROFILES: &[Profile] = &[
    // Four tools, all read-only.
    Profile {
        name: PROFILE_CORE,
        kind: ProfileKind::Base {
            tools: &[
                "meister_lane_check",
                "meister_project_list",
                "meister_project_inspect",
                "meister_canon_check",
            ],
        },
    },
    Profile {
        name: PROFILE_CANON,
        kind: ProfileKind::Base {
            tools: &[
                "meister_project_create",
                "meister_cast_add",
                "meister_cast_lock",
            ],
        },
    },
    // Group 3 plus `meister_level_compose` (its structural half
    // lands with the systems rung).
    Profile {
        name: PROFILE_SYSTEMS,
        kind: ProfileKind::Base {
            tools: &[
                "meister_scene_define",
                "meister_system_define",
                "meister_scene_validate",
                "meister_level_compose",
            ],
        },
    },
    // Groups 4–5 except `meister_level_compose`. The Rung 3 freeze
    // and emit tools join this profile when they exist.
    Profile {
        name: PROFILE_NARRATIVE,
        kind: ProfileKind::Base {
            tools: &[
                "meister_dialogue_author",
                "meister_dialogue_get",
                "meister_reaction_set",
                "meister_story_define",
                "meister_playtest_generate",
                "meister_verdict_record",
                "meister_emit",
            ],
        },
    },
    // Group 7.
    Profile {
        name: PROFILE_ASSETS,
        kind: ProfileKind::Base {
            tools: &[
                "meister_asset_intake",
                "meister_asset_route",
                "meister_asset_chain_get",
            ],
        },
    },
    // Groups 6 and 8.
    Profile {
        name: PROFILE_QA,
        kind: ProfileKind::Base {
            tools: &[
                "meister_rating_record",
                "meister_accessibility_check",
                "meister_content_summary",
                "meister_playtest_define",
                "meister_replay_check",
                "meister_defect_record",
            ],
        },
    },
    // Groups 9–10. Never in a bundle: credential-adjacent, opt-in.
    Profile {
        name: PROFILE_RELEASE,
        kind: ProfileKind::Base {
            tools: &[
                "meister_channel_list",
                "meister_release_prepare",
                "meister_preflight_run",
                "meister_account_register",
                "meister_account_inspect",
                "meister_account_verify",
            ],
        },
    },
    Profile {
        name: PROFILE_NIFLHEIM_FLOOR,
        kind: ProfileKind::Bundle {
            members: &[PROFILE_CORE, PROFILE_SYSTEMS, PROFILE_NARRATIVE],
        },
    },
    Profile {
        name: PROFILE_FULL,
        kind: ProfileKind::Full,
    },
];

/// The environment variable that selects the served profile at
/// launch (COMPLIANCE.md §7).
pub const ENV_PROFILE: &str = "MEISTER_PROFILE";

/// The profile served when `MEISTER_PROFILE` is unset.
pub const DEFAULT_PROFILE: &str = PROFILE_CORE;

impl Profile {
    /// True when this profile serves `spec`. Every profile serves
    /// the core tools; `full` serves everything.
    pub fn serves(&self, spec: &ToolSpec) -> bool {
        let carried_by = |base: &&str| spec.profiles.contains(base);
        match self.kind {
            ProfileKind::Full => true,
            ProfileKind::Base { .. } => carried_by(&PROFILE_CORE) || carried_by(&self.name),
            ProfileKind::Bundle { members } => {
                carried_by(&PROFILE_CORE) || members.iter().any(carried_by)
            }
        }
    }
}

/// Look up one declared profile by name. Exact match only.
pub fn profile_named(name: &str) -> Option<&'static Profile> {
    PROFILES.iter().find(|profile| profile.name == name)
}

/// Resolve the launch profile from the raw `MEISTER_PROFILE` value:
/// unset selects the default; a set value must name a declared
/// profile exactly. Anything else — empty, mis-cased, padded, a
/// list, not UTF-8 — is an error carrying the value as given; the
/// caller refuses to start. There is no fallback.
pub fn select_profile(raw: Option<&std::ffi::OsStr>) -> Result<&'static Profile, String> {
    let Some(raw) = raw else {
        return Ok(profile_named(DEFAULT_PROFILE).expect("the default profile is declared"));
    };
    raw.to_str()
        .and_then(profile_named)
        .ok_or_else(|| raw.to_string_lossy().into_owned())
}

/// The profiles a client can launch to reach `spec`, in declared
/// order. `full` is left out: it carries every tool by definition
/// and is for conformance and audit only, so it is never the
/// answer to "where does this tool live".
pub fn carrying_profiles(spec: &ToolSpec) -> Vec<&'static str> {
    PROFILES
        .iter()
        .filter(|profile| !matches!(profile.kind, ProfileKind::Full) && profile.serves(spec))
        .map(|profile| profile.name)
        .collect()
}

/// A profile declaration the catalog refuses to serve under.
#[derive(Debug, PartialEq, Eq)]
pub enum ProfileError {
    /// `core` (a base profile) or `full` is not declared as such.
    MissingProfile(&'static str),
    DuplicateProfile(String),
    /// A base profile names a tool the catalog does not hold.
    UnknownTool {
        profile: String,
        tool: String,
    },
    /// A tool carried by no base profile.
    ToolWithoutProfile(String),
    /// A tool's `profiles` names something that is not a declared
    /// base profile.
    UnknownProfile {
        tool: String,
        profile: String,
    },
    /// A tool's `profiles` and `PROFILES` disagree on who carries it.
    MembershipMismatch {
        tool: String,
    },
    BundleCarriesRelease(String),
    /// A bundle member that is not a declared base profile.
    BundleMemberNotBase {
        bundle: String,
        member: String,
    },
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingProfile(name) => write!(f, "profile {name} is not declared"),
            Self::DuplicateProfile(name) => write!(f, "profile {name} is declared twice"),
            Self::UnknownTool { profile, tool } => {
                write!(f, "profile {profile} names unknown tool {tool}")
            }
            Self::ToolWithoutProfile(tool) => write!(f, "tool {tool} is in no profile"),
            Self::UnknownProfile { tool, profile } => {
                write!(f, "tool {tool} names undeclared base profile {profile}")
            }
            Self::MembershipMismatch { tool } => write!(
                f,
                "tool {tool}: its profiles and the profile declarations disagree"
            ),
            Self::BundleCarriesRelease(bundle) => {
                write!(f, "bundle {bundle} carries {PROFILE_RELEASE}")
            }
            Self::BundleMemberNotBase { bundle, member } => {
                write!(f, "bundle {bundle} names {member}, not a base profile")
            }
        }
    }
}

/// The own tools of the base profile `name`, if one is declared.
fn base_tools(profiles: &[Profile], name: &str) -> Option<&'static [&'static str]> {
    profiles.iter().find_map(|p| match p.kind {
        ProfileKind::Base { tools } if p.name == name => Some(tools),
        _ => None,
    })
}

/// Check a profile declaration against a tool list of
/// (name, profiles) pairs. The first violation is returned.
///
/// Laws: `core` is a base profile and `full` is the full profile;
/// profile names are unique; a base profile names only catalog
/// tools; a bundle names only base profiles and never `release`;
/// every tool is carried by at least one base profile; and the two
/// entries of the double entry — each tool's `profiles` and the
/// base profiles naming it — are the same set.
pub fn check_profiles(tools: &[(&str, &[&str])], profiles: &[Profile]) -> Result<(), ProfileError> {
    if base_tools(profiles, PROFILE_CORE).is_none() {
        return Err(ProfileError::MissingProfile(PROFILE_CORE));
    }
    if !profiles
        .iter()
        .any(|p| p.name == PROFILE_FULL && matches!(p.kind, ProfileKind::Full))
    {
        return Err(ProfileError::MissingProfile(PROFILE_FULL));
    }
    for (index, profile) in profiles.iter().enumerate() {
        if profiles[..index].iter().any(|p| p.name == profile.name) {
            return Err(ProfileError::DuplicateProfile(profile.name.to_string()));
        }
        match profile.kind {
            ProfileKind::Base { tools: own } => {
                for tool in own {
                    if !tools.iter().any(|(name, _)| name == tool) {
                        return Err(ProfileError::UnknownTool {
                            profile: profile.name.to_string(),
                            tool: (*tool).to_string(),
                        });
                    }
                }
            }
            ProfileKind::Bundle { members } => {
                for member in members {
                    if *member == PROFILE_RELEASE {
                        return Err(ProfileError::BundleCarriesRelease(profile.name.to_string()));
                    }
                    if base_tools(profiles, member).is_none() {
                        return Err(ProfileError::BundleMemberNotBase {
                            bundle: profile.name.to_string(),
                            member: (*member).to_string(),
                        });
                    }
                }
            }
            ProfileKind::Full => {}
        }
    }
    for (tool, tagged) in tools {
        if tagged.is_empty() {
            return Err(ProfileError::ToolWithoutProfile((*tool).to_string()));
        }
        for tag in *tagged {
            if base_tools(profiles, tag).is_none() {
                return Err(ProfileError::UnknownProfile {
                    tool: (*tool).to_string(),
                    profile: (*tag).to_string(),
                });
            }
        }
        // Double entry: the base profiles naming this tool are
        // exactly the tool's own `profiles`.
        let declared = profiles.iter().filter(|p| match p.kind {
            ProfileKind::Base { tools: own } => own.contains(tool),
            _ => false,
        });
        let mut declared_count = 0usize;
        for profile in declared {
            declared_count += 1;
            if !tagged.contains(&profile.name) {
                return Err(ProfileError::MembershipMismatch {
                    tool: (*tool).to_string(),
                });
            }
        }
        if declared_count != tagged.len() {
            return Err(ProfileError::MembershipMismatch {
                tool: (*tool).to_string(),
            });
        }
    }
    Ok(())
}

/// Check the shipped declaration: `PROFILES` against `specs()`.
/// Run at startup, before anything is served.
pub fn check_declared_profiles() -> Result<(), ProfileError> {
    let all = specs();
    let tools: Vec<(&str, &[&str])> = all.iter().map(|s| (s.name, s.profiles)).collect();
    check_profiles(&tools, PROFILES)
}

/// The full catalog, in group order. Count: 33 tools (32 group
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
            profiles: &[PROFILE_CANON],
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
            profiles: &[PROFILE_CORE],
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
            profiles: &[PROFILE_CORE],
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
            profiles: &[PROFILE_CANON],
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
            profiles: &[PROFILE_CANON],
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
            profiles: &[PROFILE_CORE],
            description: "Run the canon census over a project: every claim recomputed from its source artifact and SHA-256 (D-713); any unverified claim fails the census closed with the full census in the error body. The census records the state_hash it ran against; pass an earlier census's hash as prior_state_hash and a project that has changed since answers census_stale. Declares no visual PASS; the Owner's eye is the verdict.",
            input: read_input(
                &[
                    ("project_id", s()),
                    (
                        "prior_state_hash",
                        json!({"type": "string", "pattern": "^[0-9a-f]{64}$"}),
                    ),
                ],
                &["project_id"],
            ),
            output: out_schema(&[
                ("project_id", s()),
                ("state_hash", s()),
                ("stale", boolean()),
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
            profiles: &[PROFILE_SYSTEMS],
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
            profiles: &[PROFILE_SYSTEMS],
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
            profiles: &[PROFILE_SYSTEMS],
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
            profiles: &[PROFILE_NARRATIVE],
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
            profiles: &[PROFILE_NARRATIVE],
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
            profiles: &[PROFILE_NARRATIVE],
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
            profiles: &[PROFILE_NARRATIVE],
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
            profiles: &[PROFILE_SYSTEMS],
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
            profiles: &[PROFILE_NARRATIVE],
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
            profiles: &[PROFILE_QA],
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
            profiles: &[PROFILE_QA],
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
            profiles: &[PROFILE_QA],
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
            profiles: &[PROFILE_ASSETS],
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
            profiles: &[PROFILE_ASSETS],
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
            profiles: &[PROFILE_ASSETS],
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
        // ---- Freeze (Rung 3, FRZ-01): verdict records ----
        ToolSpec {
            name: "meister_verdict_record",
            group: 8,
            rung: 3,
            record_carrying: true,
            profiles: &[PROFILE_NARRATIVE],
            description: "Record an Owner ruling on a numbered verdict pack: the subjects it pins, by record id and current hash, and the ruling. The server records rulings; it never authors them.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("pack_id", s()),
                    ("decision_ref", s()),
                    ("subjects", arr(free_obj())),
                    ("ruling", s_enum(&["adopt", "send_back", "quarantine"])),
                    ("notes", s()),
                ],
                &[
                    "project_id",
                    "pack_id",
                    "decision_ref",
                    "subjects",
                    "ruling",
                ],
            ),
            output: out_schema(&[("pack_id", s()), ("ruling", s()), ("subjects", integer())]),
            annotations: write_ann(),
        },
        // ---- Group 8: Playtest and QA (Rung 6) ----
        ToolSpec {
            name: "meister_playtest_define",
            group: 8,
            rung: 6,
            record_carrying: true,
            profiles: &[PROFILE_QA],
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
            profiles: &[PROFILE_QA],
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
            profiles: &[PROFILE_QA],
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
        // ---- Emit (Rung 3, EMT-01) ----
        ToolSpec {
            name: "meister_emit",
            group: 5,
            rung: 3,
            record_carrying: true,
            profiles: &[PROFILE_NARRATIVE],
            description: "Emit frozen records through a named emitter into an emit-root directory: generated files plus an emit manifest with one SHA-256 per file. It never touches git.",
            input: record_input(
                &[
                    ("project_id", s()),
                    ("emitter", s()),
                    ("subjects", arr(s())),
                    ("out_dir", s()),
                ],
                &["project_id", "emitter", "subjects", "out_dir"],
            ),
            output: out_schema(&[
                ("emitter", s()),
                ("file_count", integer()),
                ("out_dir", s()),
            ]),
            annotations: write_ann(),
        },
        // ---- Group 9: Build and release handoff (Rung 8) ----
        ToolSpec {
            name: "meister_channel_list",
            group: 9,
            rung: 8,
            record_carrying: false,
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_RELEASE],
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
            profiles: &[PROFILE_CORE],
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

/// The tool definitions served under `profile`, built from the
/// catalog, in catalog order.
pub fn tool_definitions(profile: &Profile) -> Vec<Tool> {
    specs()
        .into_iter()
        .filter(|spec| profile.serves(spec))
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
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_is_complete_and_bounded() {
        let all = specs();
        assert_eq!(all.len(), 33);
        // Every group 1..=10 carries at least two tools.
        for group in 1..=10u8 {
            let count = all.iter().filter(|s| s.group == group).count();
            assert!(count >= 2, "group {group} has {count} tools");
        }
        // Names are unique and prefixed.
        let mut names: Vec<&str> = all.iter().map(|s| s.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 33);
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

    // ---------- profiles (EFLO-02): validation ----------

    /// The served set a profile DECLARES, resolved from `PROFILES`
    /// alone (never from the tools' own `profiles`), so it is an
    /// independent oracle for the double entry.
    fn declared_set(name: &str) -> BTreeSet<&'static str> {
        let own = |base: &str| -> BTreeSet<&'static str> {
            base_tools(PROFILES, base)
                .expect("a declared base profile")
                .iter()
                .copied()
                .collect()
        };
        let profile = PROFILES
            .iter()
            .find(|p| p.name == name)
            .expect("a declared profile");
        let mut set = own(PROFILE_CORE);
        match profile.kind {
            ProfileKind::Base { .. } => set.extend(own(name)),
            ProfileKind::Bundle { members } => {
                for member in members {
                    set.extend(own(member));
                }
            }
            ProfileKind::Full => set.extend(specs().iter().map(|s| s.name)),
        }
        set
    }

    fn catalog_tags() -> Vec<(&'static str, &'static [&'static str])> {
        specs().iter().map(|s| (s.name, s.profiles)).collect()
    }

    #[test]
    fn declared_profiles_pass_the_catalog_check() {
        assert_eq!(check_declared_profiles(), Ok(()));
        let names: Vec<&str> = PROFILES.iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "core",
                "canon",
                "systems",
                "narrative",
                "assets",
                "qa",
                "release",
                "niflheim-floor",
                "full"
            ]
        );
    }

    #[test]
    fn every_tool_is_in_core_or_a_non_full_profile() {
        let non_full: Vec<&str> = PROFILES
            .iter()
            .filter(|p| !matches!(p.kind, ProfileKind::Full))
            .map(|p| p.name)
            .collect();
        for spec in specs() {
            assert!(
                non_full.iter().any(|p| declared_set(p).contains(spec.name)),
                "{} is served by no non-full profile",
                spec.name
            );
        }
    }

    #[test]
    fn full_equals_the_catalog_and_bundles_are_unions() {
        let catalog: BTreeSet<&str> = specs().iter().map(|s| s.name).collect();
        assert_eq!(declared_set(PROFILE_FULL), catalog);
        let mut union = declared_set(PROFILE_CORE);
        union.extend(declared_set(PROFILE_SYSTEMS));
        union.extend(declared_set(PROFILE_NARRATIVE));
        assert_eq!(declared_set(PROFILE_NIFLHEIM_FLOOR), union);
        // Every bundle is a union of declared base profiles.
        for profile in PROFILES {
            if let ProfileKind::Bundle { members } = profile.kind {
                for member in members {
                    assert!(base_tools(PROFILES, member).is_some(), "{member}");
                }
            }
        }
    }

    #[test]
    fn profile_membership_is_the_plan_of_record() {
        // Served-set sizes: core, then core + the profile's own.
        let sizes: Vec<(&str, usize)> = PROFILES
            .iter()
            .map(|p| (p.name, declared_set(p.name).len()))
            .collect();
        assert_eq!(
            sizes,
            [
                ("core", 4),
                ("canon", 7),
                ("systems", 8),
                ("narrative", 11),
                ("assets", 7),
                ("qa", 10),
                ("release", 10),
                ("niflheim-floor", 15),
                ("full", 33)
            ]
        );
        // Core is four tools, all read-only.
        let core = declared_set(PROFILE_CORE);
        for spec in specs() {
            if core.contains(spec.name) {
                assert_eq!(spec.annotations.read_only_hint, Some(true), "{}", spec.name);
                assert!(!spec.record_carrying, "{}", spec.name);
            }
        }
        // Group placement: 3 → systems, 4–5 → narrative (except
        // level_compose → systems), 7 → assets, 6 and 8 → qa,
        // 9–10 → release.
        for spec in specs() {
            let expected: &[&str] = match (spec.group, spec.name) {
                (_, "meister_level_compose") | (3, _) => &[PROFILE_SYSTEMS],
                // The Rung 3 freeze and emit tools join the
                // narrative profile (its declaration says so),
                // whatever their groups.
                (_, "meister_verdict_record") => &[PROFILE_NARRATIVE],
                (4 | 5, _) => &[PROFILE_NARRATIVE],
                (7, _) => &[PROFILE_ASSETS],
                (6 | 8, _) => &[PROFILE_QA],
                (9 | 10, _) => &[PROFILE_RELEASE],
                _ => continue,
            };
            assert_eq!(spec.profiles, expected, "{}", spec.name);
        }
    }

    // ---------- profile selection (EFLO-03): validation ----------

    #[test]
    fn each_profile_serves_exactly_its_declared_set() {
        // The serving path reads the tools' own `profiles`; the
        // oracle reads `PROFILES`. They must agree for every profile.
        for profile in PROFILES {
            let served: BTreeSet<&str> = specs()
                .iter()
                .filter(|spec| profile.serves(spec))
                .map(|spec| spec.name)
                .collect();
            assert_eq!(served, declared_set(profile.name), "{}", profile.name);
            let listed: BTreeSet<String> = tool_definitions(profile)
                .iter()
                .map(|tool| tool.name.to_string())
                .collect();
            let declared: BTreeSet<String> = declared_set(profile.name)
                .iter()
                .map(|name| (*name).to_string())
                .collect();
            assert_eq!(listed, declared, "{}", profile.name);
        }
    }

    #[test]
    fn selection_defaults_to_core_and_takes_every_declared_name() {
        assert_eq!(select_profile(None).unwrap().name, "core");
        for profile in PROFILES {
            let raw = std::ffi::OsString::from(profile.name);
            assert_eq!(select_profile(Some(&raw)).unwrap().name, profile.name);
        }
    }

    #[test]
    fn carrying_profiles_name_where_a_tool_lives() {
        let carriers = |name: &str| carrying_profiles(&spec_named(name).unwrap());
        assert_eq!(
            carriers("meister_scene_define"),
            ["systems", "niflheim-floor"]
        );
        assert_eq!(carriers("meister_cast_lock"), ["canon"]);
        assert_eq!(carriers("meister_account_verify"), ["release"]);
        // A core tool lives in every launchable profile.
        assert_eq!(carriers("meister_lane_check").len(), PROFILES.len() - 1);
    }

    // ---------- profiles (EFLO-02): adversarial ----------

    mod adversarial {
        use super::*;

        #[test]
        fn a_tool_with_no_profile_fails() {
            let mut tools = catalog_tags();
            tools.push(("meister_orphan", &[]));
            assert_eq!(
                check_profiles(&tools, PROFILES),
                Err(ProfileError::ToolWithoutProfile("meister_orphan".into()))
            );
        }

        #[test]
        fn a_profile_naming_an_unknown_tool_fails() {
            // The Rung 3 tools the plan places in `narrative` do not
            // exist yet: declaring one early is exactly this failure.
            let profiles = [
                Profile {
                    name: PROFILE_CORE,
                    kind: ProfileKind::Base {
                        tools: &["meister_lane_check"],
                    },
                },
                Profile {
                    name: PROFILE_NARRATIVE,
                    kind: ProfileKind::Base {
                        tools: &["meister_emit"],
                    },
                },
                Profile {
                    name: PROFILE_FULL,
                    kind: ProfileKind::Full,
                },
            ];
            let tools: [(&str, &[&str]); 1] = [("meister_lane_check", &[PROFILE_CORE])];
            assert_eq!(
                check_profiles(&tools, &profiles),
                Err(ProfileError::UnknownTool {
                    profile: "narrative".into(),
                    tool: "meister_emit".into()
                })
            );
        }

        #[test]
        fn a_bundle_containing_release_fails() {
            let profiles = [
                Profile {
                    name: PROFILE_CORE,
                    kind: ProfileKind::Base {
                        tools: &["meister_lane_check"],
                    },
                },
                Profile {
                    name: PROFILE_RELEASE,
                    kind: ProfileKind::Base {
                        tools: &["meister_channel_list"],
                    },
                },
                Profile {
                    name: "ship-it",
                    kind: ProfileKind::Bundle {
                        members: &[PROFILE_CORE, PROFILE_RELEASE],
                    },
                },
                Profile {
                    name: PROFILE_FULL,
                    kind: ProfileKind::Full,
                },
            ];
            let tools: [(&str, &[&str]); 2] = [
                ("meister_lane_check", &[PROFILE_CORE]),
                ("meister_channel_list", &[PROFILE_RELEASE]),
            ];
            assert_eq!(
                check_profiles(&tools, &profiles),
                Err(ProfileError::BundleCarriesRelease("ship-it".into()))
            );
        }

        #[test]
        fn a_tool_whose_profiles_disagree_with_the_declaration_fails() {
            // Tagged into a profile that does not name it.
            let mut tools = catalog_tags();
            let index = tools
                .iter()
                .position(|(name, _)| *name == "meister_cast_add")
                .unwrap();
            tools[index].1 = &[PROFILE_QA];
            assert_eq!(
                check_profiles(&tools, PROFILES),
                Err(ProfileError::MembershipMismatch {
                    tool: "meister_cast_add".into()
                })
            );
            // Tagged into a profile nobody declared, a bundle, or full.
            for bad in ["nope", PROFILE_NIFLHEIM_FLOOR, PROFILE_FULL] {
                let tags = [bad];
                let mut tools: Vec<(&str, &[&str])> = catalog_tags();
                tools[index].1 = &tags;
                assert_eq!(
                    check_profiles(&tools, PROFILES),
                    Err(ProfileError::UnknownProfile {
                        tool: "meister_cast_add".into(),
                        profile: bad.into()
                    })
                );
            }
        }

        #[test]
        fn selection_refuses_anything_but_an_exact_declared_name() {
            for bad in [
                "",
                "nope",
                "Core",
                "CORE",
                " core",
                "core ",
                "core,canon",
                "core\n",
            ] {
                let raw = std::ffi::OsString::from(bad);
                assert_eq!(
                    select_profile(Some(&raw)).map(|p| p.name),
                    Err(bad.to_string()),
                    "{bad:?} must not select a profile"
                );
            }
        }

        #[cfg(unix)]
        #[test]
        fn selection_refuses_a_non_utf8_value() {
            use std::os::unix::ffi::OsStrExt;
            let raw = std::ffi::OsStr::from_bytes(b"core\xff");
            assert!(select_profile(Some(raw)).is_err());
        }

        #[test]
        fn no_launchable_profile_but_release_carries_a_release_tool() {
            for spec in specs() {
                if spec.group == 9 || spec.group == 10 {
                    assert_eq!(carrying_profiles(&spec), ["release"], "{}", spec.name);
                }
            }
        }

        #[test]
        fn malformed_declarations_fail() {
            let tools: [(&str, &[&str]); 1] = [("meister_lane_check", &[PROFILE_CORE])];
            let core = || Profile {
                name: PROFILE_CORE,
                kind: ProfileKind::Base {
                    tools: &["meister_lane_check"],
                },
            };
            let full = || Profile {
                name: PROFILE_FULL,
                kind: ProfileKind::Full,
            };
            assert_eq!(
                check_profiles(&tools, &[full()]),
                Err(ProfileError::MissingProfile(PROFILE_CORE))
            );
            assert_eq!(
                check_profiles(&tools, &[core()]),
                Err(ProfileError::MissingProfile(PROFILE_FULL))
            );
            assert_eq!(
                check_profiles(&tools, &[core(), core(), full()]),
                Err(ProfileError::DuplicateProfile("core".into()))
            );
            // A bundle of a bundle, or of an undeclared profile.
            let nested = Profile {
                name: "nested",
                kind: ProfileKind::Bundle {
                    members: &[PROFILE_FULL],
                },
            };
            assert_eq!(
                check_profiles(&tools, &[core(), nested, full()]),
                Err(ProfileError::BundleMemberNotBase {
                    bundle: "nested".into(),
                    member: "full".into()
                })
            );
        }
    }
}
