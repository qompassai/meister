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

One vocabulary note, recorded rather than smoothed over: bloch's
intake gate parses `origin_lane` as `general|restricted`, while
Meister's plan of record stamps `origin_lane: qompass`. These
records follow bloch's envelope shape and field discipline with
Meister's own lane vocabulary, as the plan of record rules. If
they are ever fed to bloch intake, that mapping is the one
translation required, and it belongs to the intake side.
