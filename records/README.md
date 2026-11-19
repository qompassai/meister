# records/

Server records Meister keeps about itself, born classified
(`origin_lane: qompass`, `content_label: sfw`, with evidence) per
the lane law in the plan of record (../README.md).

## birth/

One birth record per tool schema, written at Rung 0 when the
schema was born. Format: the bloch outcome-record envelope
(`OutcomeRecord` in `src/outcome.rs` of qompassai/bloch — the
estate's one finished-run record schema): `outcome_id`, `server`,
`asked`, `produced` (path + SHA-256 of the artifact claimed),
`result`, `verification`, `cost`, `skills`, the lane stamp, and
`recorded_at_epoch`. Each record's `produced` entry names
`tools/list#<tool>` with the SHA-256 of that tool's canonical
served definition (sorted keys, compact separators), so a later
surface can be diffed against its birth state.

## rung1/

One promotion record per tool whose served definition changed
at Rung 1, in the same envelope and with the same `produced`
convention: the SHA-256 of the tool's canonical served
definition after the rung landed, diffable against its birth
record. Five of the six Group 1–2 tools changed (their schemas
grew with the live implementations). `meister_project_list` is
absent on purpose: its schema did not change — its Rung 1
definition hash is byte-identical to its birth record — only
its handler became real.

## rung2/

One promotion record per tool whose served definition changed
at Rung 2, in the same envelope and with the same `produced`
convention. First entry: `meister_canon_check` (STORE-02 — the
census gained `state_hash` and `stale` outputs and the optional
`prior_state_hash` input). Further records join this section as
the rung's clusters land; a tool whose definition did not
change gets no record.

One vocabulary note, recorded rather than smoothed over: bloch's
intake gate parses `origin_lane` as `general|restricted`, while
Meister's plan of record stamps `origin_lane: qompass`. These
records follow bloch's envelope shape and field discipline with
Meister's own lane vocabulary, as the plan of record rules. If
they are ever fed to bloch intake, that mapping is the one
translation required, and it belongs to the intake side.

## rung3/

No promotion records: no Rung 0–2 tool's served definition
changed at Rung 3 (handlers landed; schemas were born
complete). The rung's two new tools were born at Rung 3 and
their birth records live in `birth/` with the rest:
`meister_verdict_record` (group 8) and `meister_emit`
(group 5).

## rung4/

One promotion record per tool whose served definition
changed at Rung 4, same envelope and convention:
`meister_asset_intake` (optional `chain` and `drift`
inputs), `meister_asset_route` (optional `mode` input),
`meister_project_create` (optional `genesis` input), and
`meister_project_inspect` (the `genesis` output). The
asset handlers themselves landed at this rung against
schemas born at Rung 0.

## rung5/

Birth records for schemas born at Rung 5 that are not tool
schemas, same envelope. `meister-pack-1.json` (PK-01) is the
birth of the pack format `meister-pack/1`: its `produced`
entries pin the defining source (`src/pack.rs`) and the format
document (`packs/README.md`) by SHA-256. No tool's served
definition changed at PK-01 (packs add no tools; the catalog
invariant test pins this), so no promotion record is written.

RPG-01 adds three records here. `meister_system_define.json` is
the promotion of that tool's served definition: the kind enum
gains the fixed value `pack` and the input the optional
`pack_kind` (the pack carrier). `meister-pack-1-rpg01.json` is an
additive amendment of `meister-pack/1` (optional field `ref`; the
`level` template target; `pack` reserved), pinning the amended
`src/pack.rs` and `packs/README.md`. `role-playing-pack-1.json` is
the birth of the first compiled pack, pinning
`packs/role_playing/pack.json`.

One promotion record per tool whose served definition
changed at Rung 5, same envelope and convention:
`meister_scene_define` (the optional `stage` input — the
STAGE scene idiom's part tables, camera rig, and room
composition) and `meister_scene_validate` (the
`stage_findings` output — the embeddedness verdict rows).
STAGE-01's checks transcribe the ruled embeddedness bar
(D-1658), the removed standing panel (D-1672), and the
Niflheim scene rounds' convicted and cured forms (C1–C7,
S15–S18). Rung 5 items that change no served definition
get no record.
