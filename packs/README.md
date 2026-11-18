# packs/

Genre packs (PK-01): **data plus emitters, never tools**. One
directory per pack, `packs/<slug>/pack.json`, compiled into the
binary with `include_str!` (the `COMPILED` list in `src/pack.rs`).
Every compiled pack is validated before the server serves anything;
a build carrying an invalid pack refuses to start
(`pack_catalog_invalid`). A `pack.json` on disk that is not in
`COMPILED` fails the test suite.

A pack adds no tool. The served tool count is the same with zero
packs and with one pack per genre; `src/pack.rs` pins this with
`catalog_invariant_tool_count_does_not_depend_on_packs`.

## Format `meister-pack/1`

Closed: every object below takes exactly the keys listed, and an
unknown key is `pack_unknown_field`. Ids are lowercase snake_case
(`[a-z][a-z0-9_]*`, at most 64 chars), unique across the pack's
kinds, templates, generators and gates.

```json
{
  "schema": "meister-pack/1",
  "slug": "<genre slug; equals the directory name>",
  "version": "MAJOR.MINOR.PATCH",
  "kinds": [
    {"kind": "<id>",
     "fields": [{"name": "<id>", "type": "string|integer|boolean|array|object",
                 "required": true, "ref": "<kind; optional>"}]}
  ],
  "systems_templates": [
    {"template_id": "<id>", "system_kind": "<core system kind | level>", "spec": {}}
  ],
  "media_profile": [
    {"format": "<lumen format>", "budget": {"<id>": 0}}
  ],
  "playtest_generators": [{"generator_id": "<id>", "description": "<text>"}],
  "gates": [{"gate_id": "<id>", "description": "<text>"}],
  "emitters": ["<an emitter this build carries>"]
}
```

All nine top-level keys are required; every list may be empty (a
data-only skeleton is a valid pack).

| Section | Rule |
|---|---|
| `slug` | One of the thirteen genre slugs. |
| `kinds` | At most 64; each declares 1..=64 fields. The kind's validator checks a record is an object, carries every required field, carries only declared fields, and each with its declared type. |
| `kinds[].fields[].ref` | Optional (RPG-01). Names a core system kind or a kind this pack declares; the field must be `string` or `array`. A record's value (or each array item, a string, at most 256) must be the `system_id` of a record of that kind in the project, else `graph_dangling_ref`. |
| `systems_templates` | At most 64; `system_kind` is one of `schedule`, `flags`, `progression_dag`, `economy`, `state_machine`, `minigame`, or `level` (RPG-01: the spec is a SYS-09 layout, instantiated through `meister_level_compose`); `spec` is an object. |
| `media_profile` | One entry per format at most; formats are lumen's: `still_2d`, `pixel`, `animation`, `model_3d`, `effect`, `audio`, `text`, `video`. Budgets: 1..=16 entries, non-negative integers. |
| `playtest_generators` | At most 32. |
| `gates` | At most 64. |
| `emitters` | Names from the emitters this build carries (`bevy-rust-content/1`, `bevy-shell/1`), no repeats. A pack names emitters; it never brings one. |

Bounds: a pack is at most 256 KiB and nests at most 16 levels.

## Refusals

| Code | When |
|---|---|
| `pack_declares_tool` | A `tool`, `tools`, `inputSchema` or `input_schema` key anywhere, or any id equal to a catalog tool name. |
| `pack_kind_collision` | A kind that is a core kind (a system kind, an asset kind, a core record kind, or any `meister_` name), or a kind two packs both declare. |
| `pack_format_not_lumen` | A media format lumen does not own (e.g. `voice`, which is eigenstyle's, or `haptics`). |
| `pack_unknown_field` | A key the format does not define. |
| `pack_too_large` | Over 256 KiB. |
| `pack_invalid` | Anything else malformed: bad JSON, wrong schema, unknown slug, slug and directory disagree, bad id or version, unknown emitter, over a bound, too deep. |

## Selection

A project's primary pack and its secondary packs select which
kinds' validators run. A record of a kind that no selected pack
declares is `pack_kind_not_selected`; a record that breaks its
kind is `record_invalid`. A genre whose pack has not landed yet
selects no kinds.

## The carrier (RPG-01)

Pack records travel on `meister_system_define` with `kind: "pack"`
and `pack_kind: "<kind>"`; the record is the `spec`. The selection is
the project's own `genre_pack` plus `secondary_genre_packs`. The
record lands in the project's `systems/` family, born `candidate`,
with `pack_kind` recorded beside `kind`, so freeze, census, the state
hash and level-compose references (`layout.encounters[].system_id`)
treat it as any system record. The tool gained one fixed enum value
and one optional input, the same whatever packs are compiled in:
the catalog invariant still holds, and `pack` is reserved (a pack
kind may not take the name).

## Compiled packs

| Slug | Version | Contents |
|---|---|---|
| `role_playing` | 1.0.0 (RPG-01) | Kinds `foe` (behaviour refs a `state_machine` FOE behaviour table), `encounter` (refs `foe`), `encounter_table` (refs `encounter`), `enemy_genesis_hook` (a canonical variant-table reference only; genesis proper is Roguelike's), `portrait_set`, `combat_effect`, `music_cue` (MIDI required, streamed only as fallback), `data_migration`. Every kind but `data_migration` carries `schema_rev` (required) and `migrated_from` (optional) for version migration. Templates: `foe_behaviour` (state machine), `region_unlock` (progression DAG), `party_purse` (economy), `floor_cave` and `region_descent` (SYS-09 level layouts). Media: pixel, model_3d, still_2d portraits, MIDI-first audio, effect records; budgets are proposed design values. Emitter `bevy-rust-content/1`. |
| `educational` | 0.1.0 (TEACH-01) | Kinds `curriculum` (canon: `domain`, `codex_principle`, `failure_class`, every field non-empty text) and `assessment_encounter` (gate suite pinned by path plus SHA-256 with an equal recorded validation/adversarial split; the embedding pattern must match its form — a quiz frame is `narrative_quiz_frame` and nothing else). The teaching law beyond the field tables lives in `src/teach.rs`, run by `Selection::validate` after the field table passes: typed refusals `split_unbalanced`, `suite_unhashed`, `embedding_mismatch`. Template: `learner_progress` (state machine). Selected as a secondary pack. Emitter `bevy-rust-content/1`. |

### `educational` (TEACH-01, teaching layer v1)

The Educational pack, selected as a secondary pack (e.g. primary
`role_playing`, secondary `educational`). Its two kinds carry the
teaching law in `src/teach.rs`, run by `Selection::validate` after
the field table passes:

| Kind | Fields | Law |
|---|---|---|
| `curriculum` (canon) | `curriculum_id`, `domain`, `codex_principle`, `failure_class` | Every field is non-empty text. |
| `assessment_encounter` | `encounter_id`, `curriculum_id`, `embedding_pattern`, `gate_suite`, optional `quiz` | `embedding_pattern` is one of `lesson_as_mechanic`, `lesson_as_gate`, `assessment_as_encounter`, `narrative_quiz_frame`. `gate_suite` is closed: `path`, `sha256`, `validation_count`, `adversarial_count`. Meister never runs player code; the game does. |

Typed refusals beyond the mechanism's: `split_unbalanced` (the
recorded validation and adversarial counts differ), `suite_unhashed`
(no SHA-256 on the suite), `embedding_mismatch` (an encounter
carrying a `quiz` claims anything but `narrative_quiz_frame`, or
`narrative_quiz_frame` without a quiz). Anything else malformed is
`record_invalid`.

The progress-state spec a teacher or learner inspects is the
`learner_progress` systems template, a core `state_machine`:
`unseen -> introduced -> practiced -> assessed -> mastered`, with
`assessed -> practiced` for a failed assessment.

Not yet checked (needs a carrier tool, an RPG-01 decision):
that an encounter's `curriculum_id` names a curriculum record of
the same project, and canon governance over curriculum records.
