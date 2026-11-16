# Spec compliance — meister

Status: version 0.1.0 (Rung 0 — Birth, plan of record README.md).
Basis: stdio behaviour below is exercised by the in-repo suites
(unit, stdio conformance, manifest agreement) run on primo at
the Rung 0 gate; per-client tolerance (MCP Inspector, rose,
phlow) has NOT been run at this rung and is not claimed.

## 1. Lifecycle and version negotiation

Modern era only. The server pins `supported_protocol_versions`
to `2026-07-28` and is stateless and discover-first: clients use
`server/discover`, `tools/list`, and `tools/call`, each carrying
the modern-era `_meta` probe (`io.modelcontextprotocol/
protocolVersion`). A stdio era guard owns the transport end:
`initialize` is refused with a typed protocol error `-32022`
naming 2026-07-28, and `tools/list` without the `_meta` probe is
refused the same way. Stateless operation: no session state is
kept; the guard forwards only discover, ping, tools/list, and
tools/call.

## 2. Transport — stdio

JSON-RPC over stdio, one JSON object per line, 4 MiB line bound.
Notifications (requests without an id) are dropped unexecuted.
Parse errors answer `-32700`; unknown methods `-32601`;
oversized lines `-32600`. Stderr is diagnostics only; stdout
carries protocol frames only.

## 3. Tools

31 tools served, all declared in `src/catalog.rs`: three per
README tool group (groups 1–10) plus `meister_lane_check`
(group 0, the live lane gate). 30 are typed seams at Rung 0 —
each returns a structured `not_implemented` tool error naming
the tool, its group, and the rung that makes it real.
`meister_lane_check` is fully live (§4).

Completeness at birth, asserted by the conformance suite over
the wire: every tool carries a complete input schema (explicit
`properties` on every schema, zero misses) and a published
output schema, and annotations on 31/31 tools. Reads declare
`readOnlyHint`; writes do not. `openWorldHint` is true only
where the tool's work reaches outside the server by design
(`meister_preflight_run`, `meister_account_verify`).

EFLO record (fleet metadata-budget rules): the 25-tool soft
budget is exceeded — 31 tools — and the measured served
`tools/list` payload is 26,229 bytes (≈6,557 tokens at the
4-bytes-per-token estimate), over the ≈2,500-token budget. This
carries a gated waiver with evidence, which the plan of record
itself sanctions (README §"Technology contract"): the full
ten-group surface at Rung 0 is the commission — every group
present as typed seams from birth, plus the live gate, so no
group's contract is designed later under pressure from an
implemented neighbour. Mitigation of record: trigger-scoped
loading by group (Rule 8) — the catalog's groups are cleanly
separable, and client configs generated from the agent card can
scope a session to the groups a task triggers. The waiver is
re-measured at every rung gate; per-tool payload is dominated by
the repeated lane envelope on record-carrying tools, which is a
lane-law cost, not decoration.

## 4. Lane law enforcement

The gate is live from this commit (`src/lane.rs`). Every
record-carrying input is an envelope (`record_id`,
`origin_lane`, `content_label`, `evidence`); the gate runs
before any handler logic and before any mutation:

- nsfw label → typed refusal `lane_mismatch`, any origin;
- non-sfw label or an sfw label claiming `origin_lane: ntranch`
  → `lane_mismatch` (records never cross lanes, either way);
- missing/unreadable label or origin, unrecognised values, or
  missing evidence → `lane_unclassified`;
- only qompass + sfw + evidence proceeds (to the seam at
  Rung 0).

Refusals are `isError` tool results whose structured body names
the offending record ids. The envelope's `content_label` is
deliberately not a schema enum: the gate owns classification, so
a bad label reaches the gate and is refused with a lane code
rather than dying as a protocol error. `meister_lane_check`
returns the gate's verdict for a candidate envelope and stores
nothing. No tool declares a visual or audio PASS; the Owner's
eye/ear are the verdict (D-713/D-221).

## 5. Errors

Execution failures are tool results (`isError` + structured
JSON: lane refusals, `not_implemented` seams, `unknown_tool`);
protocol errors are reserved for protocol faults (era refusal,
unknown method, malformed JSON, oversized line). A client can
therefore always distinguish "the server refused this work" from
"the request was not protocol".

## 6. Resources, prompts, other surfaces

None at Rung 0: the guard answers resource and prompt methods
`-32601` (method not found) rather than advertising an empty
surface. Capabilities beyond tools are added by rung, with the
same contract discipline, when the plan of record calls for
them.

## 7. Security posture

No secrets are read, stored, or served: group 10 manages
credential REFERENCES and status by schema design (a
`credential_ref` string pattern, never a value), and the server
takes no CLI arguments and reads no environment configuration.
At Rung 0 the server writes nothing to disk at all — handlers
are seams behind the lane gate, and the conformance suite
asserts refused and seam calls leave the working directory
untouched. Input bounds live in the schemas (list limits, id
patterns) and in the guard's line bound.

## 8. Packaging, registry, and the agent card

`server.json` carries the registry identity `ai.qompass/meister`,
version 0.1.0, repository and website. Packages are OMITTED:
nothing is published yet, and no package URL or hash is invented;
packages are added at Rung 9 publication.

The agent card (`.well-known/agent-card.json`, A2A v1.0.1
shape) is the fleet pilot for cards as canonical capability
manifests (D-1605). Honesty invariant: Meister is an MCP server
over stdio and has NO A2A endpoint, so `supportedInterfaces` is
empty — no field asserts an endpoint that does not exist. The
MCP interface is declared via the
`https://qompass.ai/extensions/mcp-interface/v1` capability
extension, whose params carry the provisioning facts
(implementation rmcp, registry name, and `invocation`:
stdio transport, command `~/.local/bin/meister`, no args,
protocol era 2026-07-28) so a client config can be generated
from the card alone.

Maintained invariant: card and server.json agree exactly on the
join keys — card `name` == server.json `title`, card `version`
== server.json `version`, extension `registryName` ==
server.json `name`. The invariant is held by
`tests/manifest_agreement.rs`; any change to one file that
breaks it fails the gate.

## 9. Bloch birth records

One birth record per tool schema under `records/birth/`, in the
bloch outcome-record envelope (qompassai/bloch `src/outcome.rs`:
`outcome_id`, `server`, `asked`, `produced` with SHA-256,
`result`, `verification`, `cost`, `skills`, lane stamp,
`recorded_at_epoch`), each stamped `origin_lane: qompass`,
`content_label: sfw`, with the README section it derives from as
evidence, and the served schema's canonical SHA-256 as its
produced artifact. Vocabulary note (also in records/README.md):
bloch's intake parses `origin_lane` as `general|restricted`;
Meister's records use its own plan-of-record vocabulary
(`qompass`). The envelope shape and field discipline are bloch's;
the lane value is Meister's, by its README.

## 10. Test evidence at Rung 0

- `cargo test`: 33 passed, 0 failed — 21 unit (era guard 6,
  lane gate 7, catalog 3, dispatch 5), 10 stdio conformance
  (half validation, half adversarial: era refusal, no-probe
  refusal, lane refusals over the wire, no-write assertions,
  malformed-input survival), 2 manifest agreement.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo fmt --check`: clean.
- SCIP: `rust-analyzer scip` ran on primo at the Rung 0 gate and
  produced a 146,522-byte `index.scip` whose header identifies
  the rust-analyzer SCIP emitter; the index was verified
  gitignored and deleted after the smoke (generated, never
  committed). No `scip` CLI exists on primo, so no `scip print`
  pass was available; header check is the parse evidence.
- Theodolite LSP validation: RUN by Pax after the build gate
  (2026-11-16), copy-route protocol per docs/BUILDING.md —
  byte-identical copy of this commit in a throwaway git repo,
  bacon-ls producer first, rust-analyzer diagnostics: all five
  src files at zero errors; planted-error control in
  src/lane.rs seen as exactly one E0308 while an untouched
  file stayed at zero; witness restored byte-identical
  (sha256 44a6dcd27ef28a452273c4507e80b3bb1adc7997848c34d27360068d551afe6c)
  and all files zero again after a fresh-file refresh trigger.
  Verdict: PASS. Record: primo
  ~/.local/share/pax/meister-gate-20261116/gates/theodolite-record.json.
- Client tolerance (Inspector / rose / phlow): NOT RUN; named,
  not claimed.
