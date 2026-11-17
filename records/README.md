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
