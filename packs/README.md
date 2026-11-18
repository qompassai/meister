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
                 "required": true}]}
  ],
  "systems_templates": [
    {"template_id": "<id>", "system_kind": "<core system kind>", "spec": {}}
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
| `systems_templates` | At most 64; `system_kind` is one of `schedule`, `flags`, `progression_dag`, `economy`, `state_machine`, `minigame`; `spec` is an object. |
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
