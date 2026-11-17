# Changelog

## Unreleased

- `two_model` form kind renamed `adult` -> `grown` (decision D12,
  Owner-ruled 2026-11-17). No stored-record migration: no live
  store carries the kind. PATH-TO-1.0 amended in the same pass
  (SYS-11a/b Rung 5 tasks; V1-01 title of record: Time Waits:
  Gear Game) and the COMPLIANCE S3 `niflheim-floor` EFLO waiver
  recorded as granted.

## 0.5.0 — 2026-11-17

- Rung 4 — asset routing and media (branch
  `pax/rung4-assets-20261116`, stacked on the Rung 3
  branch; unmerged and unpublished — the version names
  the branch state GATE-4 verified).
- Group 7 is real. `meister_asset_intake`: one media
  artifact from the nine-kind media map, its declared
  SHA-256 verified against the bytes when provenance
  names a source path, art-tool provenance required, and
  attribution recorded as verified or unverifiable —
  marked, never repeated as fact. An asset of a locked
  character records the standing lock hash.
- `meister_asset_chain_get` judges keeper chains: a
  missing chain is the typed state absent-unverified; an
  incomplete chain is refused naming the first missing
  field (`chain_incomplete` — the rung's published gate);
  the chain's final hash must equal the asset's.
  Admission is derived from the verdict chain (a chain
  claiming admission without a standing adoption is
  `canon_unadopted`; an admitted asset without a drift
  measurement is `drift_unmeasured`); a stale character
  lock is `lock_stale`; a human-touched finish node makes
  the asset custody-frozen, and re-intake against it is
  refused with the refusal recorded verbatim. No tool
  declares a visual pass.
- `meister_asset_route` records handoffs to lumen,
  eigenstyle, bloch, and fathom (spec-only: a route
  claiming the asset itself is `fathom_asset_refused`).
  The conformance run routes every media-map format.
- The genesis defaults ledger (AST-06): a project may be
  born with the seven defaults under a block of kind born
  or adopted, confirmed as a block through
  `meister_verdict_record` and read back through
  `meister_project_inspect`; emit refuses an unconfirmed
  block (`defaults_unconfirmed`). The ledger is
  append-only — it moves only through verdicts.
- AST-07 (the lumen-lane graduation amendments) is not
  Meister code and was not done in this lane; it is
  recorded as outstanding in COMPLIANCE.md §10.
- Twenty-one of 33 tools are live; twelve seams remain.

## 0.4.0 — 2026-11-17

- Rung 3 — narrative authoring, freeze, and emit (branch
  `pax/rung3-narrative-freeze-emit-20261116`; unmerged and
  unpublished — the version names the branch state GATE-3
  verified).
- Groups 4–5 are real: `meister_dialogue_author` /
  `meister_dialogue_get` (structured dialogue: bounded
  lines, declared-flag conditions and effects, resolving
  choice targets), `meister_reaction_set` (whole-body
  reaction profiles on identity-locked cast),
  `meister_story_define` (beats and quests with resolving
  references), and the narrative half of
  `meister_level_compose` (per-chamber scene/dialogue
  composition and encounters under the descent law).
- `meister_playtest_generate`: deterministic, bounded
  traversal of a dialogue or story graph — distinct
  root-to-ending playthroughs, endings reached, and
  endings never reached by id; two runs byte-identical.
- The census judges narrative now: sanitization hits
  against a project's out-of-band denylist and cast
  differentiation collisions are counted claims, and unit
  states (ungated / gated / sealed) are counted.
- Freeze: `meister_verdict_record` (new tool) records an
  Owner ruling on a verdict pack, every subject pinned by
  record id and current canonical hash. A record standing
  adopted refuses every edit path (`frozen_record`) until
  a later verdict thaws it. Project lifecycle is derived
  at read: scaffolded / authoring / frozen_slice /
  emitted.
- Emit: `meister_emit` (new tool) emits frozen records
  under a configured emit root — `bevy-rust-content/1`
  (one generated Rust module per record) and
  `bevy-shell/1` (a fresh edition-2024 shell on the estate
  nightly pin) — with an emit manifest carrying one
  SHA-256 per file; a second identical emit is
  byte-identical, and foreign files in a target are
  refused. EMT-04 compiled a frozen slice of Niflheim
  Floor 12 both ways.
- EFLO: narrative measures 2,166 of the 2,500-token
  budget; the niflheim-floor bundle measures 2,918 (over
  by 418 now that it carries freeze and emit) — a waiver
  for the bundle is requested in COMPLIANCE.md §3 and
  stands as an Owner decision. Eighteen of 33 tools are
  live; fifteen seams remain.

## 0.3.0 — 2026-11-17

- Rung 2 — EFLO profiles, the store guarantees, and systems
  authoring (branch `pax/rung2-systems-profiles-20261116`;
  unmerged and unpublished — the version names the branch
  state GATE-2 verified).
- EFLO: surface profiles selected at launch by
  `MEISTER_PROFILE` (core, canon, systems, narrative, assets,
  qa, release, the niflheim-floor bundle, and full for audit).
  `tools/list` serves the launched profile; a call outside it
  answers `group_not_loaded` after the lane gate, naming the
  profiles that carry the tool. The agent card carries one
  invocation entry per profile. EFLO.md measures every
  profile with a real tokenizer: niflheim-floor is 2,436 of
  the 2,500-token budget; full is audit-only and over, with
  the waiver recorded in COMPLIANCE.md §3.
- Store guarantees: a cross-process advisory lock serializes
  mutations across server processes (`store_busy` after a
  10 s bound, nothing written); the census carries a state
  hash and reports staleness against a caller-presented pin
  (`census_stale`); reference integrity (`canon_orphan`) and
  per-project record-id uniqueness (`record_id_duplicate`)
  are counted census claims.
- Group 3 is real: `meister_system_define` validates flags,
  state machines, progression DAGs, economies, schedules,
  and minigames over a shared bounded graph validator;
  `meister_scene_define` and `meister_scene_validate` cover
  scene graphs; `meister_level_compose` composes levels
  structurally under the descent law (D-1659) — entry points
  and doors are first-class, stairs pair adjacent floors in
  both directions — while narrative fields refuse
  `not_implemented_until_rung3` until Rung 3. Eleven of 31
  tools are live; twenty seams remain.
- SYS-10: a 51-fixture malformed-input corpus
  (`tests/fixtures/malformed/`), every fixture refused with
  its expected typed code and the store byte-identical.
- SYS-11: Niflheim Floor 12 authored end to end through the
  tools in a throwaway store (census 14/0). Two shape gaps
  are recorded for a later rung: recommended prerequisites
  are inexpressible in a progression DAG, and level chambers
  do not reference scene records.

## 0.2.0 — 2026-11-16

- Rung 1 — Scaffolding and canon (D-1660). Groups 1–2 are real
  implementations over an on-disk record store (`src/store.rs`):
  the root is `MEISTER_PROJECTS_ROOT`, else
  `$XDG_DATA_HOME/meister/projects`, else
  `$HOME/.local/share/meister/projects`. Records are JSON,
  written atomically after full validation — a refused call
  writes nothing. Seven of 31 tools are now live (the six
  Group 1–2 tools plus `meister_lane_check`); 24 seams remain.
- Group 1: project create/inspect/list. Genre pack selected at
  birth (the thirteen categories), optional secondary packs,
  and standing canon facts carried from birth, each naming its
  source artifact and that artifact's SHA-256.
- Group 2: cast identity records encoding the estate's canon
  governance (the Light Show model): governing references named
  inside the record; `two_model` means exactly one canonical
  base and one canonical adult form, every further design
  quarantined or superseded with a reason and a ruling
  reference; media enters through a keeper chain (born
  `candidate`; `admitted` only with a recorded verdict).
  Identity locks are hard gates in the record (D-1410):
  silhouette, palette, face/body proportions, and thumbnail
  recognisability, each sourced; a conflicting relock is
  refused, an identical relock stands.
- The canon census is server-side (`meister_canon_check`): every
  claim is recomputed from its source artifact and SHA-256
  (D-713). Any unverified claim fails the census closed with
  the full census in the error body — never a bare pass, never
  absorbed.
- SHA-256 is computed in-crate (`src/hash.rs`, FIPS 180-4,
  test-vector-verified): the fleet dependency set gains nothing
  for a hash.
- Gate (verdict pack `meister-rung1-gate-20261116`): a fresh
  Niflheim project (role-playing pack, bevy) scaffolded through
  the tools; the Delver and the Oathwarden cast from their live
  scene-branch material and identity-locked; canon carrying the
  descent structure (D-1659), the background embeddedness bar
  (D-1658), and Niflheim-teaches-Rust; census 44 checked / 0
  unverified. Adversarial probes: an unrecognisable-thumbnail
  lock refused; a tampered source failed the census closed.
- EFLO: served tools/list re-measured like-for-like (compact
  tools array): 24,283 B at Rung 0 → 25,706 B at Rung 1
  (+1,423 B, +5.9%), the delta entirely Group 1–2 schema
  growth. The waiver stays open; see COMPLIANCE §3.
- Bloch promotion records for the five tools whose served
  definitions changed under `records/rung1/`
  (`meister_project_list` is unchanged byte-for-byte and has
  none, on purpose).

## 0.1.0 — 2026-11-16

- Rung 0 — Birth (plan of record: README.md, D-1603/D-1604). The
  rmcp skeleton answers discovery in the MCP modern era only
  (2026-07-28, rmcp 3.8.0 fleet pin, toolchain nightly-2026-10-30)
  with the full tool surface as typed seams: 31 tools — three per
  README tool group (groups 1–10) plus the one live tool.
- The lane gate is live from this first commit: every
  record-carrying input is an envelope (`origin_lane`,
  `content_label`, `evidence`); only qompass + sfw proceeds.
  NSFW and other-lane input is refused `lane_mismatch`,
  unclassifiable input `lane_unclassified`, both typed and before
  any mutation. `meister_lane_check` exposes the gate's verdict
  without storing anything.
- Agent card corrected to spec-valid A2A v1.0.1 the same day
  (D-1607): the MCP interface is declared as a custom-binding
  entry in `supportedInterfaces`; the removed-in-1.0
  `stateTransitionHistory` field is dropped.
- Bloch birth records for all 31 schemas under `records/birth/`
  (bloch outcome-record envelope; see records/README.md).
- Agent card at `.well-known/agent-card.json` (A2A v1.0.1 shape):
  the fleet pilot for cards as canonical capability manifests
  (D-1605). The card declares no A2A endpoint — Meister is an MCP
  server over stdio — and carries its invocation facts in the
  qompass.ai MCP-interface extension. `server.json` and the card
  agree on name/version/title; `tests/manifest_agreement.rs`
  holds that invariant.
- Repo skills under `skills/` (build-and-gate, lane-law,
  tool-surface, genre-packs); build wiring in `docs/BUILDING.md`
  and the `justfile`, including SCIP index regeneration
  (generated, never committed).
- Gates at birth: cargo test 33 passed / 0 failed (21 unit,
  10 stdio conformance, 2 manifest agreement); clippy
  `--all-targets -D warnings` clean; fmt clean. Client-tolerance
  matrix (Inspector / rose / phlow) not yet run — named as not
  run in COMPLIANCE.md, not claimed.
