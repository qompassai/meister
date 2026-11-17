# Meister — the path to 1.0, task by task

Companion to README §"The path to 1.0", which stays the plan of record's
front door. The README states what each rung *is*; this file breaks each
rung into tasks a builder lane can pick up one at a time without
inventing scope.

Commissioned by the Owner: D-1663 (plan, Niflheim-first) and D-1665
(the plan's home is this repo's docs). Planning pass of 2026-11-16. The
plan was read against:

- the README at `main` `bcdc63a`;
- Rung 0 at `pax/rung0-skeleton-20261116` `c4a8568`;
- Rung 1 on `pax/rung1-scaffolding-canon-20261116`, first read as an
  in-flight working tree (without modifying it), then re-checked
  against its landed commits `dd8c0e0` and `3e6e39d` (gate record);
- lumen `docs/pipes/PIPES.md` at `36575af` and lumen `EFLO.md`;
- Niflheim's local branches, read-only. Niflheim's content never enters
  this public repo.

**Status of this file.** The task lists are specified. The re-sequencing
in §2 is a **proposal awaiting the Owner's ruling** (decision D1, §13).
Until that ruling, the published rung order in the README stands. Every
task has a stable ID that does not depend on rung order, so if the
Owner reorders rungs, whole sections move and no ID changes.

Contents:

1. Where things stand
2. Re-sequencing for Niflheim, with reasons
3. How to read a task
4. Rung 1 — Scaffolding and canon (landed on its branch)
5. Rung 2 — Systems and floor structure, with surface profiles
6. Rung 3 — Narrative, Freeze, Emit
7. Rung 4 — Asset routing and media
8. Rung 5 — The flagship pack: Role Playing × Educational
9. Rung 6 — Standards, accessibility, QA
10. Rung 7 — The remaining packs, the teaching layer, the TUI
11. Rung 8 — Release
12. Rung 9 — 1.0
13. Open decisions for the Owner
14. Risks and unknowns

---

## 1. Where things stand

### 1.1 Rung 0 — landed (`c4a8568`)

There are 31 tools: one live, 30 typed seams. Each seam answers
`not_implemented`, naming its group and the rung that makes it real
(`src/catalog.rs`).

| Group | Tools | State | Rung (published) |
|---|---|---|---|
| 0 lane | `meister_lane_check` | **live** | 0 |
| 1 scaffolding | `meister_project_create`, `meister_project_inspect`, `meister_project_list` | seam at Rung 0; real on the Rung 1 branch (`dd8c0e0`) | 1 |
| 2 cast/canon | `meister_cast_add`, `meister_cast_lock`, `meister_canon_check` | seam at Rung 0; real on the Rung 1 branch (`dd8c0e0`) | 1 |
| 3 systems | `meister_scene_define`, `meister_system_define`, `meister_scene_validate` | seam | 2 |
| 4 dialogue | `meister_dialogue_author`, `meister_dialogue_get`, `meister_reaction_set` | seam | 3 |
| 5 story/level | `meister_story_define`, `meister_level_compose`, `meister_playtest_generate` | seam | 3 |
| 6 standards | `meister_rating_record`, `meister_accessibility_check`, `meister_content_summary` | seam | 6 |
| 7 assets | `meister_asset_intake`, `meister_asset_route`, `meister_asset_chain_get` | seam | 5 |
| 8 QA | `meister_playtest_define`, `meister_replay_check`, `meister_defect_record` | seam | 6 |
| 9 release | `meister_channel_list`, `meister_release_prepare`, `meister_preflight_run` | seam | 8 |
| 10 credentials | `meister_account_register`, `meister_account_inspect`, `meister_account_verify` | seam | 8 |

Other Rung 0 facts:

- 16 tools carry records. Each takes the lane `envelope`, and the lane
  gate runs before any handler.
- Tests: `cargo test` 33 passed / 0 failed. Clippy and fmt are clean.
  Theodolite copy-route: PASS, with its control.
- The client-tolerance matrix (Inspector / rose / phlow) has **not been
  run** (COMPLIANCE.md §10).
- EFLO: the served `tools/list` is **26,229 bytes**. COMPLIANCE.md §3
  converts this to ≈ 6,557 tokens with a 4-bytes-per-token *estimate*,
  not a tokenizer, so that token count is unverified.
- The fleet budget is **≤ 2,500 tokens** (cl100k_base; lumen `EFLO.md`,
  "fleet EFLO, torori reference").
  - This is a token budget. At lumen's measured ratio (12,224 characters
    for 2,826 tokens) it is about 10–11 KB, not 2.5 KB.
  - Either way the surface is two to three times over.
  - An open waiver covers Rungs 0 and 1 only.

### 1.2 Rung 1 — landed on its branch, not yet merged to `main`

Rung 1 landed on its own branch while this plan was being written:
`dd8c0e0` (the build) and `3e6e39d` (the gate record). That branch is
the source of the facts below.

- **The record store (`src/store.rs`)**:
  - Layout: `<root>/<project_id>/project.json` and
    `<root>/<project_id>/cast/<character_id>.json`.
  - Records are JSON with `schema_version: 1`, written atomically
    (temp file, then rename) only after every validation passes.
  - The root is `MEISTER_PROJECTS_ROOT`, falling back to
    `$XDG_DATA_HOME/meister/projects`, then
    `~/.local/share/meister/projects`.
  - Mutations are serialised by an **in-process** `Mutex`.
  - Bounds are named constants: ids ≤ 64 chars, cast ≤ 256, canon facts
    ≤ 128, records ≤ 1 MiB, census sources ≤ 32 MiB.
- **Group 1**:
  - The project record carries the engine, a primary pack, up to 12
    secondary packs, and canon facts (`id`, `statement`, `value`,
    `source_artifact`, `source_sha256`).
  - The only status is `scaffolded`.
  - **No engine shell or test layout is emitted.** Rung 1 writes records
    only.
- **Group 2**: Light Show's canon model, encoded.
  - `model_scheme` is `two_model` (exactly one canonical `base` form and
    one canonical `adult` form) or `single_model` (exactly one canonical
    form).
  - Forms are tied to governing references.
  - A non-canonical form needs a quarantine record with a reason and a
    ruling ref.
  - `media_chain` entries are `candidate`, `admitted` (with a recorded,
    non-pending verdict), or `quarantined`.
- **Identity locks (D-1410)**:
  - Silhouette, face and body proportions, palette, and thumbnail
    recognisability, each with a source artifact and its SHA-256.
  - The lock hash is taken over canonical JSON.
  - Re-locking with an identical lock is idempotent. A different lock
    fails with `identity_lock_conflict`.
- **Census (`meister_canon_check`)**:
  - Recomputes every claim from its source file's SHA-256.
  - Fails closed: `canon_unverified`, with the full census in the error
    body.
  - Verdict is `pass` or `review`.
  - It does **not** yet implement four parts of lumen census 1.0:
    orphans, differentiation, sanitization, and staleness.
- **Error codes**: `invalid_input`, `invalid_record`, `not_found`,
  `conflict`, `identity_lock_conflict`, `canon_unverified`, `io`.
  Version 0.2.0.
- **The Rung 1 gate** (a fresh Niflheim project scaffolds, locks its
  cast, and passes census with zero unverified numbers) is recorded as
  passed in `3e6e39d`: 51 tests green across 4 runs, clippy and fmt
  clean, theodolite copy-route PASS, and a Niflheim census of 44
  checked / 0 unverified (verdict pack `meister-rung1-gate-20261116`).
  This plan cites that record and did not re-run it.
- **EFLO**: Rung 1's COMPLIANCE.md still carries the Rung 0
  measurement (26,229 bytes). The Group 1–2 schemas that Rung 1 widened
  have not been re-measured, which EFLO-01 does first.

### 1.3 Three structural gaps in the published plan

1. **Freeze and Emit have no tools.**
   - The spine is Describe → Propose → Freeze → Emit → Prove.
   - None of the 31 tools records an Owner ruling on a numbered verdict
     pack, and none runs an emitter.
   - For Niflheim, emission is the payoff: Niflheim's content is Rust.
2. **The published order inverts dependencies.**
   - Rung 4 (first packs, "produced end to end", "verdict packs carry
     the Owner's rulings") needs Rung 5's keeper chains.
   - Rung 6's gate ("cannot reach release handoff") names a capability
     that lands in Rung 8.
3. **Group 1 promises an engine shell and test layout at scaffold.**
   Rung 1 writes records only, so the shell becomes an emitter
   (EMT-03).

---

## 2. Re-sequencing for Niflheim, with reasons

Niflheim is Meister's first job (D-1660). Its build state shapes the
order:

- Floors 1–12 of 18 are landed as code, five steps per floor.
- Every floor is built structure-first: Step 1 is scaffolding.
- Content is Rust data.
- The STAGE scene idiom is mid-verdict: the R3-S5 pack awaits the
  Owner's eye. The standing bars are D-1658 (embeddedness) and D-1659
  (the descent law: cave/dungeon floors, doors and entry points per
  floor, descend/ascend).
- Dynamic combat has its design and R1 core landed. Its presentation
  waits on the scene verdict.

### 2.1 Proposed order (awaiting ruling D1)

| Rung | Content | Published | Move |
|---|---|---|---|
| 2 | Group 3 + the structural half of `meister_level_compose`; surface profiles (EFLO); cross-process store lock | 2 (+ part of 3) | extended |
| 3 | Groups 4–5; **Freeze** (`meister_verdict_record`); **Emit** (`meister_emit`) | 3 (+ new) | extended |
| 4 | Group 7; the genesis ledger and keeper-chain registry graduate | 5 | earlier |
| 5 | Flagship pack Role Playing × Educational, teaching layer v1; worked example: a Niflheim floor | 4 (part) | later, split |
| 6 | Groups 6 and 8 | 6 | stands (gate re-worded) |
| 7 | Puzzle and Narrative first, then the other ten packs; the teaching layer formal across packs; the TUI | 4 (part) + 7 | Puzzle/Narrative later |
| 8 | Groups 9–10 | 8 | stands |
| 9 | 1.0 | 9 | stands |

### 2.2 The moves

**M1. The structural half of `meister_level_compose` moves to Rung 2.**
- Why: Niflheim builds floors structure-first.
- Why: the D-1659 descent law is a property of floor structure, and
  Rung 2 can check it as a graph invariant. At Rung 3 it would bite too
  late.
- Cost: one more tool goes live in Rung 2.

**M2. Freeze and Emit join Rung 3.**
- Why: without them Meister hands Niflheim nothing it can run.
- Why: Rung 3 is the first rung where a floor's structure and its words
  both exist.
- Why not Rung 2: emitting structure alone would interleave
  half-generated floor modules with hand edits.
- Cost: Rung 3 becomes the largest rung, so it has three sub-gates.
- Depends on decision D4, which adds two tools.

**M3. Asset routing moves ahead of the first pack.**
- Why: a pack's worked example must be "produced end to end", which
  includes media. An asset without a complete keeper chain is not a
  keeper (lumen Pipe 4).
- In the published order, Rung 4 cannot pass honestly without Rung 5.

**M4. Published Rung 4 splits.**
- RPG × Educational, the Niflheim pack, becomes Rung 5.
- Puzzle and Narrative move to the head of Rung 7.
  - Why: nothing on Niflheim's path needs them.
  - Why: Puzzle's worked example, light-show, already ships.
  - Why: Narrative has no named worked example (D9).
- Cost: the pack mechanism is first proven on one genre and could
  overfit. Mitigation: the Rung 5 second-pack smoke (PK-02).

**M5. Teaching layer v1 moves from Rung 7 to Rung 5.**
- Why: a Niflheim floor cannot be produced without curriculum records
  and an assessment encounter (the FOE gate's hidden suite).
- Rung 7 keeps the cross-pack generalisation.

**M6. Surface profiles land in Rung 2.**
- Why: the EFLO waiver covers Rungs 0–1 only.
- Why: Rung 2 is the first rung to grow the live surface past
  Groups 1–2.

### 2.3 What stands, and why

- **Rung 2 stays first.** Systems records (flags, state machines, DAGs)
  are what dialogue conditions and level gates refer to.
- **Rung 6 stays where it is.** Niflheim already enforces its own
  determinism: combat replays its transcript byte-for-byte, and twin
  renders are checked. Group 8 adds records; it does not unblock
  Niflheim.
- **Rungs 8–9 stand.** Release is last by nature. Niflheim is
  local-only and never pushed, so the Rung 8 example is likely
  light-show (D10).
- **The TUI stays in Rung 7.** Niflheim has its own.

### 2.4 Pace: shadow mode (decision D2)

Niflheim floors 9–12 landed between 2026-11-10 and 2026-11-15, roughly
one a day. Floors 13–18 could be finished by hand before Rung 3 can
emit.

The recommendation is **shadow mode**:
- From Rung 2 on, each new floor's records are authored in Meister
  alongside the hand build.
- The census checks the hand build against those records.
- The first floor *produced* through Meister is whichever floor is next
  when EMT-04 passes.

---

## 3. How to read a task

Each task carries:

- **ID**: stable, prefixed by area (`EFLO`, `STORE`, `SYS`, `NAR`, `FRZ`,
  `EMT`, `AST`, `PK`, `RPG`, `TEACH`, `QA`, `TUI`, `REL`, `V1`, `NIF`,
  `GATE`).
- **Build**: the tools, record kinds, or files the task makes real.
- **Contract**: inputs, bounds, typed failure codes (Tiger Style).
  Bounds marked *proposed* are design values. The task's own tests
  confirm or revise them, and the revision is recorded.
- **Tests**: written first, half validation and half adversarial. The
  adversarial half lives in a `mod adversarial` block, or a test name
  prefixed `adv_`, so the gate can count the split.
- **Done when**: verifiable criteria.
- **Needs**: other task IDs, and any Rung 1 assumption (`A#`, §4.2).
- **[NIF]**: the task directly serves Niflheim's build.
- **LS**: a Light Show carry-forward that applies, if any.

Every task's definition of done also includes the standing gate
discipline of README §"The path to 1.0":

- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
  and `cargo test`, with counts taken from the run;
- the theodolite copy-route on changed files, with its planted-error
  control;
- a bloch birth record (or successor evidence) for every new or changed
  schema;
- a conformance run record for every pipeline stage;
- **every refusal is pre-mutation**: each refusal test hashes the store
  tree before and after the call and asserts they are identical.

**Niflheim content never enters this repo.** Fixtures are synthetic.
Niflheim-shaped gate evidence stays on primo under
`~/.local/share/pax/meister-gate-<date>/`, the same pattern as the
Rung 0 theodolite record.

---

## 4. Rung 1 — Scaffolding and canon (landed on its branch; another lane)

These tasks are recorded as fact. **Do not pick them up.** They belong
to the Rung 1 lane. The merge to `main` is that lane's act.

### 4.1 Rung 1 tasks

| ID | Task | State |
|---|---|---|
| R1-01 | Record store: layout, atomic writes, bounds, env-root contract | landed `dd8c0e0` |
| R1-02 | `meister_project_create` / `_inspect` / `_list` real, with pack selection and canon facts at birth | landed `dd8c0e0` |
| R1-03 | `meister_cast_add` (governance validation) and `meister_cast_lock` (D-1410) real | landed `dd8c0e0` |
| R1-04 | `meister_canon_check`: per-claim, source-hash census, fail-closed | landed `dd8c0e0` |
| R1-05 | Gate: fresh Niflheim scaffold, cast locked, census zero unverified **[NIF]** | PASS per gate record `3e6e39d` (44 checked / 0 unverified) |

### 4.2 What later rungs assume from Rung 1, and the fallback for each

Each later task that depends on one of these names it as `A#`.

| # | Assumption | If Rung 1 lands differently |
|---|---|---|
| A1 | Store layout as in §1.2. Later rungs add sibling directories: `systems/`, `scenes/`, `levels/`, `dialogue/`, `story/`, `verdicts/`, `assets/`, `emits/`, `genesis/`, `qa/`, `release/` | Later tasks bind to `Store` methods keyed by logical id, never to paths. A layout change costs one module. |
| A2 | The store root comes from the environment and sits outside every game repo | Unchanged; decision D7 picks the root |
| A3 | Each record embeds its envelope verbatim | If envelopes move to a sidecar, the census lane-stamp check moves with them |
| A4 | The census lacks orphans, staleness, sanitization, and differentiation | STORE-02/03 (Rung 2) and NAR-05 (Rung 3) add them. Any part Rung 1 already lands is dropped from those tasks. |
| A5 | Absolute `source_artifact` paths are accepted (needed to cite files in a game worktree) | If refused, STORE-04 adds snapshot-into-project, with the copy hashed |
| A6 | `media_chain` is `candidate`/`admitted`/`quarantined`, with verdict *text* | AST-02 adopts the landed shape; FRZ-01's verdict record becomes the structured verdict the chain cites |
| A7 | The only project status is `scaffolded` | FRZ-04 adds the lifecycle |
| A8 | No engine shell is emitted at scaffold | EMT-03 delivers it |
| A9 | *Resolved by `3e6e39d`, which records the cast as locked.* Niflheim's cast can be identity-locked at the R1-05 gate. Its figures are still placeholder part sets. | If not, NAR-02 is blocked for those characters only. Dialogue proceeds with candidate speakers. |
| A10 | One minor version per rung (Rung 1 → 0.2.0) | Cosmetic |
| A11 | *Resolved: Rung 1's COMPLIANCE.md documents `MEISTER_PROJECTS_ROOT` as the only environment input.* | GATE-2 adds `MEISTER_PROFILE` to the same paragraph |
| A12 | The store lock is in-process only | STORE-01 adds a cross-process lock either way |
| A13 | `envelope.record_id` is neither bound nor checked for uniqueness | STORE-03 adds per-project uniqueness |
| A14 | *Resolved: R1-05 is recorded as passed.* | **No Rung 2 task starts on an open Rung 1.** If R1-05 fails, Rung 2 waits. |
| A15 | D-1658 and D-1659 can be transcribed into validator rules | Any rule that is an eye-judgement stays a verdict-pack item, not a validator |
| A16 | The `two_model` form kinds are named `base`/`adult` (age forms, from Light Show) | Decision D12; a rename is a Rung 1 lane change |

---

## 5. Rung 2 — Systems and floor structure, with surface profiles

Rung 2 makes Group 3 real, plus the structural half of
`meister_level_compose`. It lands surface profiles (the EFLO fix) and a
cross-process store lock.

**Published gate (README):** validated, bounded records with typed
refusals for malformed input. Malformed input is the normal case, not
the edge case.

### EFLO — trigger-scoped loading (in Rung 2 because the waiver ends at Rung 1)

The doctrine:

- deterministic config decides what tools **can** load;
- the triggered skill decides what **should** load;
- `decide()` and the lane gate decide what is **permitted**.

A skill never grants authority.

#### EFLO-01 — Real-tokenizer measurement
- **Build:** `scripts/eflo_measure.py` and a `just eflo` recipe.
  - The script captures `tools/list` from the built binary over stdio,
    once per profile.
  - It serialises compact JSON with sorted keys and counts tokens with
    cl100k_base (`tiktoken`), the method of lumen `EFLO.md`.
  - It writes `EFLO.md` with a per-profile and per-tool table.
  - Run it with `python -I`.
- **Contract:** if `tiktoken` is unavailable on primo, the task stops
  and reports a blocker. **No estimate is substituted.**
- **Tests:**
  - Validation: measuring the same binary twice gives byte-identical
    `EFLO.md` tables.
  - Adversarial: a binary answering malformed JSON is reported as an
    error, not as 0 tokens.
- **Done when:** `EFLO.md` records the real token count of the full
  31-tool surface. That count replaces the estimate in COMPLIANCE.md §3.

#### EFLO-02 — Profile membership in the catalog
- **Build:**
  - `ToolSpec` gains `profiles: &'static [&'static str]`.
  - A `PROFILES` constant declares the profile list:
    - `core` (default): `meister_lane_check`, `meister_project_list`,
      `meister_project_inspect`, `meister_canon_check`. Four tools, all
      read-only.
    - `canon`: core + `meister_project_create`, `meister_cast_add`,
      `meister_cast_lock`.
    - `systems`: core + Group 3 + `meister_level_compose`.
    - `narrative`: core + Groups 4–5 (except `level_compose`) +
      `meister_verdict_record` + `meister_emit`.
    - `assets`: core + Group 7.
    - `qa`: core + Groups 6 and 8.
    - `release`: core + Groups 9–10. Never in a bundle; it stays opt-in
      because it is credential-adjacent.
    - Bundle `niflheim-floor` **[NIF]**: core ∪ `systems` ∪ `narrative`.
    - `full`: every tool. For conformance and audit only.
- **Tests:**
  - Validation: every tool is in `core` or in at least one non-`full`
    profile. `full` equals the catalog. Bundles are unions of declared
    profiles.
  - Adversarial: a tool with no profile, a profile naming an unknown
    tool, or a bundle containing `release` each fails a catalog unit
    test.
- **Done when:** the catalog tests pass, and membership is listed in
  COMPLIANCE.md §3.

#### EFLO-03 — Profile selection at launch
- **Build:**
  - `MEISTER_PROFILE` (default `core`) picks exactly one declared
    profile or bundle.
  - An unknown value makes the server **refuse to start**: a non-zero
    exit and a typed message on stderr. It never falls back silently.
  - `tools/list` serves only the profile's tools.
  - A `tools/call` to a catalog tool outside the profile returns
    `isError` with code `group_not_loaded`, naming the profile or
    profiles that carry the tool. It never returns `unknown_tool`.
- **Tests:**
  - Validation: each profile's `tools/list` matches its declared set,
    over stdio.
  - Adversarial:
    - An unknown profile exits non-zero before serving anything.
    - Calling an out-of-profile tool returns `group_not_loaded`.
    - An out-of-profile *record-carrying* call with an NSFW envelope is
      still refused by the lane gate first: the lane gate precedes the
      profile check, so NSFW input is never told where a tool lives.
- **Done when:** the conformance suite covers all declared profiles.

#### EFLO-04 — Agent card and server.json per profile
- **Build:**
  - `.well-known/agent-card.json` lists one invocation entry per profile
    under the qompass MCP custom-binding extension. The card is the
    canonical capability manifest (D-1605).
  - `server.json` stays single-identity.
- **Tests:** `tests/manifest_agreement.rs` is extended.
  - Validation: the card's profile list equals `PROFILES`.
  - Adversarial: a card profile missing from the catalog fails, and so
    does a card with a duplicate profile.
- **Done when:** a client config can be generated per profile from the
  card alone.

#### EFLO-05 — Trigger skills
- **Build:**
  - New repo skills: `meister-systems-authoring`, `meister-narrative`,
    `meister-assets`, `meister-qa`, `meister-release`. Each one's
    operational core names the profile to launch.
  - `skills/meister-tool-surface` §Budget is updated: profiles are the
    standing mitigation, and the rule "packs add zero tools" is added.
  - `skills/README.md` gets the new table rows.
- **Tests:** none in code. Review check: every non-`full` profile has
  exactly one triggering skill, and no skill text claims to grant a
  permission.
- **Done when:** the skills exist and each names a declared profile.

#### EFLO-06 — Budget gate
- **Build:** `EFLO.md` (via EFLO-01) for every profile and bundle at the
  Rung 2 tree. The COMPLIANCE.md §3 waiver is rewritten.
- **Done when:**
  - `core` measures ≤ 2,500 tokens.
  - Every other profile and bundle is either ≤ 2,500 tokens, or split,
    or carries a waiver record holding its measurement and the Owner's
    ruling.
  - `full` carries a standing waiver (audit use only).
  - Descriptions are **not** shortened to chase the number (the lumen
    precedent: enforcement semantics stay in descriptions).
  - The re-measure is repeated at every later rung gate.

### STORE — integrity across processes and time

#### STORE-01 — Cross-process store lock
- **Build:**
  - An advisory `flock` on `<root>/.meister.lock`, held for every
    mutation, in addition to the in-process `Mutex` (A12).
  - Lock wait is bounded (*proposed* 10 s). On timeout the call returns
    `store_busy`.
- **Why now:** profiles mean several Meister processes can share one
  store.
- **Tests:**
  - Validation: two server processes run 50 interleaved creates each;
    zero torn records, zero lost writes, every record parses.
  - Adversarial: a third process holding the lock past the bound makes
    the other processes' writes fail with `store_busy`, and nothing is
    written.
- **Done when:** both tests pass on primo's store filesystem. The
  filesystem's `flock` semantics are recorded as evidence.

#### STORE-02 — State hash and census staleness
- **Build:**
  - `state_hash`: SHA-256 over the canonical JSON of every record in a
    project, in sorted path order.
  - The census records the `state_hash` it ran against.
  - `meister_canon_check` gains the field `stale: bool`, and a census
    run against an older hash is reported as `census_stale`. This is
    lumen census 1.0's staleness rule.
- **Needs:** A1, A4.
- **Tests:**
  - Validation: the same state gives the same hash, and two runs give
    byte-identical census bodies.
  - Adversarial: any write after a census makes that census stale. A
    reordered record set gives the same hash.
- **Done when:** both suites pass. The output schema change goes
  through the contract gate (meister-tool-surface skill).

#### STORE-03 — Orphans and record-id uniqueness
- **Build:** the census gains two claims:
  - every id referenced by any record resolves (`canon_orphan`);
  - `envelope.record_id` is unique per project (`record_id_duplicate`,
    A13).
- **Tests:**
  - Validation: a fully resolved project passes.
  - Adversarial: a dangling cast reference, a dangling flag reference,
    and a duplicate `record_id` each produce a named failure in the
    census body.
- **Done when:** the census reports both checks with counts.

#### STORE-04 — Source snapshot (conditional)
- **Build only if A5 fails:** a census source outside the project
  directory is copied into `<project>/sources/<sha256>` and the copy is
  verified.
- **Done when:** censuses that cite worktree files pass again.

### SYS — Group 3 made real

#### SYS-01 — Shared graph validator
- **Build:** `src/graph.rs`, pure functions:
  - bounds;
  - cycle detection (where the kind demands a DAG);
  - reachability from declared roots;
  - dangling-reference detection.
- **Contract** (*proposed* bounds): nodes ≤ 512, edges ≤ 2,048,
  traversal steps ≤ nodes + edges.
- **Typed codes:** `graph_cycle`, `graph_unreachable`,
  `graph_dangling_ref`, `bound_exceeded`.
- **Tests:**
  - Validation: a chain, a diamond, and a forest with declared roots.
  - Adversarial: a self-loop, a 2-cycle, an unreachable island, a
    dangling edge, 513 nodes, a 10,000-node input refused before any
    traversal.
- **Done when:** unit tests pass. No I/O.

#### SYS-02 — `system_define` kind `flags` [NIF]
- **Build:** declared flags (`id`, `type ∈ {bool, int}`, `initial`, and
  bounds for `int`).
- **Contract:** flags ≤ 1,024 per project (*proposed*). Code
  `flag_duplicate`.
- **Tests:**
  - Validation: bool and int flags round-trip.
  - Adversarial: a duplicate id, an int without bounds, an initial value
    out of bounds, a float.
- **Done when:** the kind works over the wire. Records are born
  `candidate`.
- **LS:** records are born `candidate` and stay quarantined from canon,
  the keeper discipline generalised to records.

#### SYS-03 — kind `state_machine` [NIF]
- **Build:** states, transitions (`from`, `to`, guard over declared
  flags), initial state, and terminal states. Niflheim's chamber and
  FOE progressions (three states each) are the shape targets, written as
  synthetic fixtures.
- **Contract:** states ≤ 128, transitions ≤ 1,024 (*proposed*). Codes
  `state_undeclared`, `flag_undeclared`, `graph_unreachable` (a
  non-terminal unreachable state).
- **Tests:**
  - Validation: the two Niflheim-shaped machines (synthetic names).
  - Adversarial: a transition to an undeclared state, a guard on an
    undeclared flag, an unreachable state, no initial state.
- **Done when:** the kind works over the wire.

#### SYS-04 — kind `progression_dag` [NIF]
- **Build:** unlock nodes with prerequisites, which must form a DAG
  (SYS-01). Niflheim's stairs gates and skill prerequisites are the
  shape target.
- **Tests:**
  - Validation: a linear chain and a branching unlock.
  - Adversarial: a cycle, a prerequisite on an undeclared node, a root
    that is never declared.
- **Done when:** the kind works over the wire.

#### SYS-05 — kind `economy` [NIF]
- **Build:** currencies and resources, sources and sinks, and declared
  integer bounds. **Integers only.** Niflheim's combat law is "integers
  and booleans; no floats".
- **Codes:** `economy_float`, `economy_unbounded`.
- **Tests:**
  - Validation: one currency with a source and a sink.
  - Adversarial: a float value, a missing bound, a sink on an undeclared
    resource.
- **Done when:** the kind works over the wire.

#### SYS-06 — kind `schedule`
- **Build:** time-slotted events over integer ticks. Each event names a
  flag condition and an effect.
- **Tests:**
  - Validation: a daily cycle.
  - Adversarial: overlapping exclusive slots, a negative tick, an effect
    on an undeclared flag.
- **Done when:** the kind works over the wire.

#### SYS-07 — kind `minigame` [NIF]
- **Build:** a minigame record names its rules, its inputs, and its
  **judge** as an artifact path plus SHA-256. Meister never runs the
  judge. Niflheim's puzzle judges are the shape target.
- **Tests:**
  - Validation: a judge reference whose hash the census verifies.
  - Adversarial: a judge without a hash, a hash mismatch (the census
    names it), a path with traversal.
- **Done when:** the kind works over the wire, and the census covers the
  judge reference.

#### SYS-08 — `meister_scene_define` and `meister_scene_validate`
- **Build:** the scene graph: nodes, edges, dialogue-link placeholders
  (resolved in Rung 3), and composition slots. `scene_validate` runs
  SYS-01 plus orphan checks and returns `{valid, refusals[]}`, each
  refusal with its typed code.
- **Tests:**
  - Validation: a 3-node scene.
  - Adversarial: a cycle where the scene declares itself acyclic, a
    dangling link, an oversized graph, a missing envelope, an NSFW
    envelope (lane refusal first).
- **Done when:** both tools are real. Their seam errors are removed and
  their birth records gain successor evidence.

#### SYS-09 — `meister_level_compose`, structural half, with the descent law [NIF]
- **Build:**
  - A level record: floors, rooms, doors, entry points, stairs edges,
    triggers, progression gates (referencing SYS-04), and minigame
    placement (referencing SYS-07).
  - The descent-law validator (D-1659):
    - every floor declares ≥ 1 entry point;
    - every door resolves on both sides;
    - each floor's descend exit pairs with the next floor's ascend
      entry;
    - no floor is skipped.
  - **The rule set is transcribed from the D-1659 ruling text, not from
    this paraphrase (A15).**
- **Contract:** floors ≤ 64, rooms per floor ≤ 64, doors per floor
  ≤ 128 (*proposed*). Code `descent_violation`, naming the offending
  id.
- **Tests:**
  - Validation: a synthetic 3-floor descent passes.
  - Adversarial: a floor with no entry, a one-sided door, a skipped
    floor, a missing ascend, a stairs edge to an undeclared floor.
- **Done when:** the tool is real for its structural fields. The
  narrative fields still answer `not_implemented_until_rung3` (typed).

#### SYS-10 — Malformed-input corpus
- **Build:** `tests/fixtures/malformed/` holds at least 6 malformed
  fixtures per system kind, per scene, and per level, **taken from real
  local-model output**. Generate the candidates on primo, then curate
  them. Shapes: wrong types, missing keys, extra nesting, cycles,
  oversized arrays, stringified JSON.
- **Tests:** an adversarial, table-driven test feeds every fixture over
  the wire and asserts the expected typed code and a byte-identical
  store.
- **Done when:** the corpus has at least 48 fixtures and every one is
  refused with its expected code.

#### SYS-11 — Floor 12 shape proof (primo only) [NIF]
- **Build:** author the *structure* of hand-built Niflheim Floor 12 as
  Meister records, in a primo store.
  - The structure covers chambers, doors, stairs, chamber state
    machines, and the FOE machine.
  - The census sources pin the hash of the Floor 12 content source.
- **Done when:** every record validates, the census passes, and the
  evidence (records plus census) sits under
  `~/.local/share/pax/meister-gate-<date>/`. **Nothing is committed to
  this repo.**
- If a shape does not fit, that is a finding against SYS-02…09, fixed
  there before GATE-2.

#### NIF-01 — Shadow-mode authoring begins [NIF]
- **Build:** under decision D2(a), each new floor's structure is
  authored in Meister alongside its hand build.
- **Done when:** the floor's records validate, and its census pins the
  hand build's content hash.

#### CT-01 — Client-tolerance matrix, first run
- **Build:** run MCP Inspector, rose, and phlow against `core` and
  `niflheim-floor`.
- **Done when:** each client's result is recorded in COMPLIANCE.md as
  pass, fail, or not run, with evidence. This has been "not run" since
  Rung 0; profiles change discovery.

#### GATE-2 — Rung 2 gate
- **Includes:**
  - the standing gates;
  - SYS-10 green;
  - SYS-11 evidence on primo;
  - EFLO-06 within budget or waived;
  - STORE-01 on primo;
  - CT-01 recorded;
  - COMPLIANCE.md (§3 surface, §7 env contract, A11), CHANGELOG, and
    version 0.3.0;
  - birth records for every changed schema.
- **Needs:** A14 (Rung 1 closed).

---

## 6. Rung 3 — Narrative, Freeze, Emit

Rung 3 makes Groups 4–5 real and adds the Freeze and Emit steps of the
spine.

**Published gate (README):** a branching narrative slice whose every
ending is reached by generated playthroughs.

### NAR — Groups 4 and 5

#### NAR-01 — `meister_dialogue_author` and `meister_dialogue_get` [NIF]
- **Build:** structured dialogue:
  - lines (`speaker` = `character_id`, text, cues for animation, voice,
    and sound);
  - choices;
  - conditions over declared flags and states;
  - continuity effects on declared flags;
  - prerequisites.
- **Contract:** lines ≤ 2,048 per record, choices ≤ 16 per line, text
  ≤ 500 chars (*proposed*). Codes `flag_undeclared`, `speaker_unknown`,
  `bound_exceeded`.
- **Needs:** SYS-02, SYS-03, and A9 (speakers may be `candidate` cast).
- **Tests:**
  - Validation: a branching exchange round-trips through `_get`
    unchanged.
  - Adversarial: a condition on an undeclared flag, an unknown speaker,
    2,049 lines, loose prose with no structure.
- **Done when:** both tools are real.
- **LS:** canon governance; speakers must resolve to the cast.

#### NAR-02 — `meister_reaction_set`
- **Build:** whole-body reactions per emotion (posture, gesture,
  stance), conditioned on relationship state, prior choices, and scene
  history. **This is detail work, so it requires the character to be
  `identity_locked` (D-1410).** The adult state machinery of the other
  lane does not exist here and is never imported.
- **Needs:** A9.
- **Code:** `identity_unlocked`.
- **Tests:**
  - Validation: a locked character takes a profile.
  - Adversarial: an unlocked character is refused, an emotion with only
    a face and no posture is refused, and an unknown character is
    refused.
- **Done when:** the tool is real.
- **LS:** identity-lock discipline: lock before detail.

#### NAR-03 — `meister_story_define`
- **Build:** arcs, beats, quests and objectives, character arcs,
  relationship progression, and branching endings. Beats reference
  scenes, dialogue, and progression nodes.
- **Tests:**
  - Validation: a two-ending arc.
  - Adversarial: an ending no beat leads to (reported, not refused at
    define time), a beat referencing an undeclared scene, a quest whose
    objective names an undeclared flag.
- **Done when:** the tool is real.

#### NAR-04 — `meister_level_compose`, narrative half [NIF]
- **Build:** scene composition per room (the dialogue links of SYS-08
  resolved), encounter placement, schedules.
- **Tests:**
  - Validation: a floor with dialogue at its camp and stairs.
  - Adversarial: a link to an undeclared dialogue, an encounter in an
    undeclared room.
- **Done when:** the `not_implemented_until_rung3` path from SYS-09 is
  removed.

#### NAR-05 — Sanitization and differentiation in the census
- **Build:**
  - A project denylist record (no studio name, no real project, no real
    person in shipped text: the teaching games' law). The scan runs over
    every shippable text field, and hits produce `sanitization_hit` with
    the field location. They are never silently edited.
  - A differentiation check per declared cast pair: name equality is a
    collision outright, and descriptor overlap is reported as a number.
    **The machine reports; the Owner decides.**
- **Needs:** A4.
- **Tests:**
  - Validation: a clean project passes.
  - Adversarial: a denylisted term in a dialogue line, a choice label,
    and a quest title each produce a located hit. Two cast members with
    the same name produce `differentiation_collision`.
- **Done when:** the census reports both checks.
- **LS:** canon governance.

#### NAR-06 — `meister_playtest_generate`
- **Build:** a deterministic, bounded traversal of the authored graph
  (story, dialogue, levels, flags).
  - Output: playthroughs, `endings_reached`, and **`endings_unreached`
    by id**.
  - It never asserts coverage it did not observe.
- **Contract:** playthroughs ≤ 10,000, path length ≤ 4,096
  (*proposed*). Code `playtest_bound_exceeded`. A bound hit is reported
  with what was covered so far.
- **Tests:**
  - Validation: the published slice reaches every ending, and two runs
    are byte-identical.
  - Adversarial: the twin slice has one ending made unreachable, and
    exactly that id is reported. A cyclic dialogue does not loop
    forever. A graph over the bounds is refused before traversal.
- **Done when:** GATE-3.1 passes.

### FRZ — Freeze (needs decision D4)

#### FRZ-01 — `meister_verdict_record` (new tool; group 8; profile `narrative`)
- **Build:** a record-carrying tool that records an Owner ruling on a
  numbered verdict pack:
  - `pack_id`;
  - `decision_ref` (D-number);
  - `subjects[{record_id, sha256}]`;
  - `ruling ∈ {adopt, send_back, quarantine}`;
  - `notes`.
- **The server records rulings. It never authors them.** No code path
  sets `adopt` without a supplied ruling.
- **Codes:** `verdict_hash_mismatch` (a subject's current hash differs),
  `subject_unknown`.
- **Tests:**
  - Validation: adopting a 3-subject pack pins all three.
  - Adversarial: a stale subject hash, an unknown subject, an empty
    subject list, a ruling outside the enum, an NSFW envelope.
- **Done when:** the tool has its schema, annotations, and birth record.
  The EFLO-06 re-measure includes it.
- **LS:** verdict-pack gates: adoption only by the Owner's ruling on a
  numbered pack.

#### FRZ-02 — Frozen-record enforcement and thaw
- **Build:**
  - An adopted record is **frozen**: every write tool refuses changes
    to it with `frozen_record`.
  - A later `send_back` or `quarantine` ruling on the same subject
    thaws it.
  - Freeze and thaw are appended to the record's chain (custody).
- **Tests:**
  - Validation: freeze, thaw, edit, freeze again.
  - Adversarial: editing a frozen record through every write tool of
    Groups 1–5 is refused, and the store stays byte-identical.
- **Done when:** the refusal coverage is complete across the write
  tools.
- **LS:** keeper chains (custody).

#### FRZ-03 — Content unit states (lumen Pipe 3)
- **Build:** each content unit (scene, dialogue, level, arc) carries a
  state:
  - `ungated` → `gated` (with the run id that gated it) → `sealed` (when
    frozen).
  - Any edit returns the unit to `ungated`.
  - Unlock conditions are declared data (`unlock_undeclared`).
- **Graduation:** the refusal traffic from FRZ-02 and NAR-06 is the
  evidence for graduating Pipe 3. The graduation is filed in the lumen
  lane as a PIPES.md amendment (**lumen-lane task**).
- **Tests:**
  - Validation: a unit passes through all three states.
  - Adversarial: an edit after gating resets the unit, an undeclared
    unlock is refused.
- **Done when:** the census reports a count of units in each state.

#### FRZ-04 — Project lifecycle
- **Build:** `scaffolded → authoring → frozen_slice → emitted`, driven
  by events from FRZ-01, FRZ-02, and EMT-01 (A7).
- **Tests:**
  - Validation: the lifecycle follows events.
  - Adversarial: no transition happens without its triggering event.
- **Done when:** `meister_project_inspect` reports the status.

### EMT — Emit (needs decisions D3 and D4)

#### EMT-01 — `meister_emit` (new tool; profile `narrative`)
- **Build:** inputs are `project_id`, `emitter` (`name@version` from an
  emitter registry), `subjects` (every subject must be frozen), and
  `out_dir`.
  - `out_dir` must resolve under an emit root the Owner configured.
    Traversal or symlink escape fails with `emit_path_refused`.
  - The tool writes the files, then `emit-manifest.json` (one SHA-256
    per file plus the subject-set hash), and records the run.
  - **It never touches git.**
- **Codes:** `subject_unfrozen`, `emitter_unknown`, `emit_path_refused`,
  `emit_conflict` (the target holds files Meister did not write).
- **Tests:**
  - Validation: double emit is byte-identical.
  - Adversarial: an unfrozen subject, a `../` path, a symlink escape, an
    unknown emitter, a target holding hand-written files.
- **Done when:** the tool is real and annotated as a write
  (`openWorldHint` false).

#### EMT-02 — Emitter `bevy-rust-content/1` [NIF]
- **Build:** frozen structure, systems, and narrative records become
  generated Rust modules (decision D3(a)), consts and tables in
  Niflheim's content-as-data idiom.
  - Each generated file starts with a header naming the emitter, its
    version, and the subject-set hash.
  - Output passes rustfmt and is edition 2024.
- **Tests:**
  - Validation: golden-file tests over a synthetic floor.
  - Adversarial: unicode, quotes, and `*/` in dialogue text are escaped
    so they cannot break the Rust syntax. An empty subject set emits
    nothing and returns `subject_empty`.
- **Done when:** the golden tests pass and generated code compiles in a
  fixture crate.

#### EMT-03 — Emitter `bevy-shell/1`
- **Build:** the Group 1 promise: an engine shell with estate
  conventions, test layout, lane stamps, and toolchain pin (A8).
  Niflheim, an adopted project, does not need it. New projects (V1-01)
  do.
- **Tests:**
  - Validation: the emitted shell passes `cargo check` and
    `cargo test`.
  - Adversarial: emitting over a non-empty directory fails with
    `emit_conflict`.
- **Done when:** a fresh project emits a compiling shell.

#### EMT-04 — Compile proof in a Niflheim copy (primo only) [NIF]
- **Build:** emit the Floor 12 shape-proof records (SYS-11, after
  adoption via FRZ-01) into a **throwaway copy** of a Niflheim worktree,
  then run `cargo check`. **Never the live tree.**
- **Done when:** the `cargo check` record and the emit manifest are kept
  as gate evidence on primo.

#### GATE-3 — Rung 3 gate
- **G3.1:** the NAR-06 slice. Every ending reached, and the twin's
  unreached ending reported by id.
- **G3.2:** FRZ-02 coverage.
- **G3.3:** EMT-01, EMT-02, and EMT-04.
- **Also:** EFLO re-measure, birth records for the 2 new tools,
  COMPLIANCE, CHANGELOG, version 0.4.0.
- **Needs:** GATE-2, D3, D4.

---

## 7. Rung 4 — Asset routing and media (published Rung 5)

Rung 4 makes Group 7 real. Two lumen pipes graduate here: the
keeper-chain registry (c) and the genesis defaults ledger (a).

**Published gate:** an asset with an incomplete keeper chain is refused
as a keeper, by the server, in a test.

#### AST-01 — `meister_asset_intake`
- **Build:** intake records:
  - `asset_id`, `sha256`;
  - `kind` from the media map: `still_2d`, `pixel`, `animation`,
    `model_3d`, `effect`, `audio`, `voice`, `text`, `video`;
  - provenance, including **art-tool provenance per asset**, with
    `attribution: verified | unverifiable`. Unverifiable attributions
    are marked, never repeated as fact.
- **Tests:**
  - Validation: one asset of each kind.
  - Adversarial: a hash that doesn't match the bytes, an unknown kind,
    provenance missing its tool, an asset of an undeclared character.
- **Done when:** the tool is real.

#### AST-02 — Keeper-chain registry and `meister_asset_chain_get`
- **Build:** the full keeper-chain field set (lumen art bible §1.1):
  - asset id;
  - record id;
  - canon references with hashes;
  - verbatim prompt pair;
  - model checkpoint and hash, with sampler parameters;
  - conditioning models and weights;
  - seed;
  - ordered finish operations;
  - final SHA-256;
  - verdict with ruling source.
- A missing session record is the typed state `absent-unverified`
  (A6).
- **Codes:** `chain_incomplete`, `chain_hash_mismatch`.
- **Tests:**
  - Validation: a complete chain reports `chain_complete: true`.
  - Adversarial: removing each required field in turn yields
    `chain_incomplete`, naming the field.
- **Done when:** this test is the published gate, and it passes.

#### AST-03 — Keeper admission, drift, custody
- **Build:**
  - `candidate → admitted` happens only through `meister_verdict_record`
    (FRZ-01).
  - The drift measurement (palette-role comparison, prompt-block
    conformance) is attached as numbers. **No tool declares a visual
    PASS**; the phone-size side-by-side is the Owner's eye.
  - Custody: lifted nodes that a human has touched are frozen. A
    proposal touching one is refused, and the refusal is recorded
    verbatim.
- **Codes:** `canon_unadopted`, `drift_unmeasured`, `custody_frozen`.
- **Tests:**
  - Validation: admission with a ruling.
  - Adversarial: admission without a ruling, admission without a drift
    measurement, a proposal against a frozen node.
- **Done when:** all of these refusals are pre-mutation.
- **LS:** keeper chains (candidate → admitted with the verdict
  recorded); drift check against the locked palette; custody.

#### AST-04 — Lock staleness
- **Build:** an asset of a character records that character's
  `lock_hash`. If the lock has changed since, the asset fails with
  `lock_stale`.
- **Needs:** A16. The lock hash shape is from Rung 1.
- **Tests:**
  - Validation: an asset with the current lock is accepted.
  - Adversarial: a re-lock under a new ruling makes the old asset stale.
- **LS:** identity-lock discipline.

#### AST-05 — `meister_asset_route`: one route per media-map format
- **Build:** handoff records to the owning specialist. Meister records
  each handoff and re-implements nothing.
  - **lumen**: stills, pixel/Aseprite, animation (code-as-animation),
    3D (sprite-to-3D factory, GLB-first), effects (`lumen-fx/1`), audio
    (MIDI-first), text converters, video.
  - **eigenstyle**: voice style.
  - **bloch**: lessons.
  - **fathom**: clean-room study. Spec only, never assets.
- **Tests:**
  - Validation: one route record per format.
  - Adversarial: a route to an unknown target, a route for an asset
    whose record is missing (`record_missing`), a fathom route that
    claims an extracted asset (`fathom_asset_refused`).
- **Done when:** every media-map format has at least one route record in
  the conformance run.

#### AST-06 — Genesis defaults ledger (lumen graduation (a))
- **Build:** a genesis record per project:
  - the seven defaults: platforms, release model, outward-reference
    posture, language, art/audio pipeline, accessibility gate,
    packaging (owned-Gradle default, D-829);
  - each item starts `inherited`;
  - confirmation happens as a block via `meister_verdict_record`;
  - the ledger is append-only.
- **No new tool:** `meister_project_create` gains a `genesis` field, and
  the genesis record is read back through `meister_project_inspect`.
- **Codes:** `defaults_unconfirmed`, `name_conditional`,
  `clearance_collision`. The name-clearance scan stays a skill
  procedure.
- **Niflheim [NIF]:** gets a genesis record of kind `adopted`, holding
  its existing defaults with ruling refs. The ledger never pretends
  Niflheim was born in Meister.
- **Tests:**
  - Validation: confirming the block.
  - Adversarial: downstream work (EMT-01) against an unconfirmed ledger
    fails with `defaults_unconfirmed`, and an edit to a ledger entry is
    refused (append-only).
- **Done when:** the ledger gates emit.

#### AST-07 — Lumen graduation records (lumen-lane task)
- **Build:** file amendments in the lumen lane marking the census (b),
  keeper chain (c), and genesis ledger (a) scripts as superseded by
  Meister. This prevents two validators diverging.
- **Not Meister code.**
- **Done when:** the amendments exist and are referenced from
  COMPLIANCE.md.

#### GATE-4 — Rung 4 gate
- **Includes:** AST-02 (the published gate), AST-03 through AST-06,
  EFLO re-measure, COMPLIANCE, CHANGELOG, version 0.5.0.
- **Needs:** GATE-3.

---

## 8. Rung 5 — The flagship pack: Role Playing × Educational (from published Rung 4)

**Published gate (as it applies to this pack):** the worked example's
conformance records verify, and its verdict packs carry the Owner's
rulings.

#### PK-01 — Pack mechanism
- **Build:** packs as **data plus emitters, never tools**.
  - Location: `packs/<slug>/pack.json`, compiled in with
    `include_str!`.
  - A pack holds: specialised record `kind`s with server-side
    validators, systems templates, its media profile (lumen formats and
    budgets), playtest generators, and its gate set.
  - A project's primary and secondary packs (Rung 1) select which
    validators run.
- **Tests:**
  - Validation: the pack schema validates.
  - Adversarial: a pack declaring a tool, a pack kind colliding with a
    core kind, and a pack whose media profile names a format lumen does
    not own are each refused by a unit test.
  - **Catalog invariant:** the tool count does not depend on the number
    of packs.
- **Done when:** the invariant test passes.

#### RPG-01 — Role Playing pack v1 data [NIF]
- **Build:**
  - floor and region templates (SYS-09 shapes);
  - FOE behaviour tables as `state_machine` records (Niflheim combat R1
    keeps a FOE behaviour table as data);
  - encounter tables;
  - enemy-genesis hooks (a canonical variant-table reference only;
    genesis proper is Roguelike's, in Rung 7);
  - media profile: pixel sprites or 3D via the lumen factory, portrait
    sets, MIDI-first, effects records for combat feel;
  - **version migration** fields, because auteur RPGs run for a decade.
- **Tests:**
  - Validation: the templates instantiate.
  - Adversarial: a FOE table with an undeclared state, an encounter
    referencing an undeclared FOE.
- **Done when:** the pack loads and its validators run on an RPG
  project.

#### RPG-02 — RPG playtest generators [NIF]
- **Build:** generators that traverse floors as a player would (no
  teleporting), built on NAR-06.
- **Tests:**
  - Validation: full traversal of a 3-floor synthetic descent.
  - Adversarial: a floor reachable only by teleport is reported
    unreachable.

#### TEACH-01 — Teaching layer v1 [NIF]
- **Build:**
  - `curriculum` canon kind: domain, codex principle, failure class.
  - `assessment_encounter` kind: names the gate suite (path plus
    SHA-256) and records its validation/adversarial split. **Meister
    never runs player code; the game does.**
  - Progress-state spec that a teacher or learner can inspect.
  - Embedding pattern per encounter: `lesson_as_mechanic`,
    `lesson_as_gate`, `assessment_as_encounter`, or
    `narrative_quiz_frame`.
- **Tests:**
  - Validation: a curriculum and encounter pair validates.
  - Adversarial: a suite whose recorded split is unequal fails with
    `split_unbalanced`, a suite without a hash is refused, and a quiz
    frame claiming `lesson_as_mechanic` is refused.
- **Done when:** both kinds are in the Educational secondary pack.

#### STAGE-01 — STAGE scene record (decision D8) [NIF]
- **Gated on the Owner's R3-S5 verdict.** Do not start before it.
- **Build:** the ruled STAGE idiom as a scene record (part tables, camera
  rig, room composition), plus the D-1658 embeddedness checks
  transcribed from the ruling (A15).
- **Without the verdict:** the task does not ship in pack v1, and
  floors use the panel shell, as most of Niflheim's floors do today.

#### PK-02 — Second-pack smoke
- **Build:** a data-only Puzzle pack skeleton: a level family with a
  declared solver reference. No worked example.
- **Done when:** it validates against PK-01's schema with no
  RPG-specific fields. This is the guard against the mechanism being
  Niflheim-shaped.

#### NIF-02 — Worked example: one Niflheim floor, end to end (primo only) [NIF]
- **Build:** the floor chosen under D2, produced entirely through
  Meister:
  - records authored (structure, systems, narrative, curriculum,
    assessment);
  - cast locked;
  - assets routed through lumen with complete keeper chains;
  - frozen by verdict;
  - emitted with `bevy-rust-content/1`;
  - Niflheim's own gates pass on the emitted floor: its test suite,
    clippy, and the whole-floor playthrough (Step 5).
- **Done when:** the Owner's ruling on the floor's verdict pack is
  recorded, and the conformance records verify on primo. The commit
  into Niflheim is the Niflheim lane's act, not Meister's.
- **LS:** verdict-pack gates; identity locks on the cast; canon
  governance over curriculum.

#### GATE-5 — Rung 5 gate
- **Includes:** NIF-02, PK-01 invariant, PK-02, EFLO re-measure
  (unchanged tool count), version 0.6.0.
- **Needs:** GATE-4, D2, D6, D7. STAGE-01 only if the R3-S5 verdict has
  landed.

---

## 9. Rung 6 — Standards, accessibility, QA

Rung 6 makes Groups 6 and 8 real.

**Published gate, re-worded** (release handoff does not exist until
Rung 8): a project that fails photosafety is not release-eligible,
proven by an adversarial fixture. GATE-8 then repeats the same fixture
through the real preflight.

#### QA-01 — `meister_rating_record`
- **Build:** ESRB, PEGI, and IARC metadata with descriptors, and
  age-appropriateness review records.
- **Tests:**
  - Validation: one rating per scheme.
  - Adversarial: an unknown scheme, a descriptor the scheme does not
    define (the descriptor tables come from each scheme's primary
    source, cited).

#### QA-02 — `meister_content_summary`
- **Build:** a plain-language summary and content tags, computed from
  project state.
- **Tests:**
  - Validation: the summary reflects the tags.
  - Adversarial: a summary that contradicts the rating fails with
    `summary_inconsistent`.

#### QA-03 — `meister_accessibility_check`
- **Build:**
  - **Photosafety:** hard limits on flash and transition rate, read
    from `lumen-fx/1` records and scene specs. The limit values come
    from the cited standard. lumen already enforces them at the format
    level; Meister re-checks at the project level.
  - Colour-vision-safe palette check over the locked palettes.
  - Subtitle and text-scaling contracts.
  - Remappable-input declaration.
- **None of this is progression-gated.**
- **Tests:**
  - Validation: a compliant project.
  - Adversarial: a flash over the limit, a palette pair that collapses
    under deuteranopia, missing subtitles on a voiced line.
- **LS:** photosafety. light-show (light and flash puzzles) is the
  estate's strongest exposure. The Owner's eye is the verdict for visual
  accessibility.

#### QA-04 — The `release_eligible(project)` predicate
- **Build:** the predicate holds only when all of these are true:
  - accessibility clean;
  - rating recorded;
  - census `pass` and not stale;
  - genesis confirmed.
- It returns typed reasons. Rung 8's preflight must call it.
- **Tests:**
  - Validation: an eligible project.
  - Adversarial: photosafety failure → `photosafety_failed`, a stale
    census → `census_stale`.
- **Done when:** the published gate (re-worded) passes.

#### QA-05 — `meister_playtest_define`
- **Build:** scripted playthroughs, half validation and half adversarial
  **by construction**. A script whose split is unequal fails with
  `split_unbalanced`.
- **Tests:**
  - Validation: a balanced script.
  - Adversarial: an unequal split, a script referencing an undeclared
    scene.

#### QA-06 — `meister_replay_check` [NIF]
- **Build:** compares a replay hash recorded by the game against
  Meister's recorded hash. **Meister is not a runtime.** Niflheim's
  combat transcript replay is the first consumer.
- **Tests:**
  - Validation: matching hashes.
  - Adversarial: a divergence is reported with both hashes, and a
    missing replay artifact produces `replay_missing`.

#### QA-07 — `meister_defect_record`
- **Build:** a defect tied to a `state_ref` that must resolve.
- **Tests:**
  - Validation: a defect against a dialogue line.
  - Adversarial: a dangling `state_ref`.

#### QA-08 — Screenshot specs for verdict packs
- **Build:** screenshot specifications (scene, camera, tick, size)
  attached to `meister_verdict_record` packs. **This is a spec, not a
  capture.** Capture belongs to the game's tooling.
- **Tests:**
  - Validation: a spec resolves to a declared scene.
  - Adversarial: a spec naming an undeclared camera.

#### GATE-6 — Rung 6 gate
- **Includes:** QA-04 (the published gate), the rest of QA-01…08, EFLO
  re-measure, version 0.7.0.
- **Needs:** GATE-5.

---

## 10. Rung 7 — The remaining packs, the teaching layer, the TUI

**Gate:** every pack's worked example has conformance records the Owner
has gated.

Most worked-example titles **do not exist yet** (risk K1). Each pack
task therefore starts with a sub-task: **"Owner names the worked
example"**. Until a title is named, the pack can reach `v1-data` (its
data validates) but not `v1`.

Every PK task follows the same steps:

1. Write the pack data (PK-01 shape).
2. Write its validators, tests first, half validation / half
   adversarial.
3. Write its playtest generator.
4. Write its media profile.
5. Produce the worked example end to end.
6. Get the Owner's verdict.

Below, each pack's *specific* primitives are taken from its README
category text.

| ID | Pack | Pack-specific primitives to build (from README) | Worked example |
|---|---|---|---|
| PK-PUZ | Puzzle | A level-family record with a **required solver or constructive proof**; playtests cover the edges of the solution space; a difficulty curve with measured rungs; a puzzle = scene + validation model | light-show's fiber-optic puzzles (README). **LS:** the homecoming: two_model governance, `_base` media, keeper chains |
| PK-NAR | Narrative / VN | Branch-graph audit and visualisation data; episodic chapter gating with save continuity; voice-per-line routing with locked timbres; a Ren'Py emitter profile plus the Unity port path | **D9**: none named |
| PK-ACT | Action | Combat-verb kits (parry, posture, finishers) as primitives; a global beat clock; ability-gated world graphs; hit-stop and screenshake as data; traversal bots | to be named; README: a jam entry |
| PK-ADV | Adventure | A clockwork world-state simulation; hotspots; inventory and state flags; fail-forward dialogue (a failure costs something authored) | to be named |
| PK-EDU | Educational (standalone) | All four embedding patterns as configurations; an evidence-asset workflow for documentary and primary-source media | to be named |
| PK-FGT | Fighting | Fixed-timestep frame data as records; hit/hurt on any entity; rollback state partitioning; training mode and simplified input as primitives | to be named |
| PK-PLT | Platformer | Movement-kit feel parameters; room/tile levels; parallax stacks; checkpoints; assist modes as standard parts | to be named |
| PK-RHY | Rhythm | A chart format against MIDI; calibration/latency offset; timing from the audio clock, never frame count; strictest photosafety; deterministic passability replay | to be named |
| PK-RGL | Roguelike | The generator as a frozen, hashed record; runs replayable from their seed; enemy genesis and variant sweeps; composable effect records; census of generated families against canonical tables | to be named |
| PK-SHT | Shooter | Ballistics and hit models as data; scoring and feedback components; performance-budget profiles; data that survives engine migration | to be named |
| PK-SIM | Simulation | Schedules, economies, and NPC machines as primary state; a director as weighted event tables; agent budgets and tick dials; production-chain graphs | to be named |
| PK-STR | Strategy | Grid and graph theatres; unit canon records; deterministic battle simulation with replay hashes; bounded-lookahead AI policies; swarm task assignment; composable faction traits; fathom specs only | to be named |

Other Rung 7 tasks:

- **TEACH-02 — The teaching layer, formal across packs.**
  - TEACH-01's kinds become available to any primary pack.
  - Worked example: Versemend (teaches Lua).
  - Tests: a non-RPG pack carrying the layer validates; the layer cannot
    be selected as a *primary* pack.
- **TUI-00 — Obtain the fleet TUI programme's gate criteria** (unknown
  U3). TUI-01…n are specified from those criteria.
- **TUI-01 — The TUI.** Plan and auto modes, live progress, and media
  preview at the highest fidelity the terminal stack allows, all over
  the same profiles (EFLO-03). It is a client of Meister, never a second
  write path.
- **GATE-7.** Every pack task reaches `v1` with an Owner verdict, TUI
  criteria met, EFLO re-measure (tool count unchanged by packs),
  version 0.8.0.

---

## 11. Rung 8 — Release

Rung 8 makes Groups 9–10 real.

**Published gate:** a worked example completes a full preflight against
at least one pipeline channel and one prepared-submission channel. No
secret is ever present in any Meister state or output.

#### REL-01 — Channel registry data (`meister_channel_list`)
- **Build:** entries for Google Play, F-Droid, itch.io (butler), Steam,
  GOG, Epic Games Store, Humble, Apple App Store, Microsoft Store, the
  Nintendo / PlayStation / Xbox portals (curated-pitch class), and
  direct web (own site, GitHub releases).
  - Each entry records: platforms, content-policy notes, automation
    class (`pipeline` / `prepared_submission` / `curated_pitch`),
    artifact requirements, handoff executor.
  - **The registry records capability, not intent.**
- **Tests:**
  - Validation: every entry has all fields.
  - Adversarial: an entry without a verification record is flagged
    `unverified`, never filled in with invented requirements.

#### REL-02 — Live verification per channel
- **Build:** verify each entry against the channel's primary
  documentation at build time. Evidence is recorded per entry: source
  URL and date. A channel that fails verification is flagged with
  evidence.
- **Done when:** every entry is `verified` or `flagged`.

#### REL-03 — `meister_account_register` and `meister_account_inspect`
- **Build:** channel-account records holding **references only**:
  channel, account identity, credential kind, value location (a primo
  `pass` path or a Secure Vault entry), scopes, expiry,
  last-verified. **No value is ever accepted, stored, or returned.**
- **Tests:**
  - Validation: a register/inspect round trip.
  - Adversarial: an input that looks like a secret value (a token
    pattern or high-entropy string) in any field fails with
    `credential_value_refused`.

#### REL-04 — `meister_account_verify`
- **Build:** a per-channel probe where the channel supports one.
  Channels with no API get honest login-reference records. **No token
  flows are invented.**
- **Tests:**
  - Validation: a probe against a channel's sandbox where one exists.
  - Adversarial: a channel without an API returns
    `verification_unsupported`, not `verified`.

#### REL-05 — `meister_preflight_run`
- **Build:** refuses if the channel's auth is missing or unverified, or
  if `release_eligible` (QA-04) is false.
- **Tests:**
  - Validation: a fully preflighted project.
  - Adversarial: **the QA-04 photosafety fixture is refused here** (the
    re-worded Rung 6 gate, now proven end to end); also expired auth and
    a missing account.

#### REL-06 — `meister_release_prepare`: slipway handoff
- **Build:** channel artifacts handed to slipway for Play and F-Droid
  certification, and to the other executors. **Meister never forks
  slipway's publish gates.**
- **LS:** light-show's shipped release practice (`fastlane/`, `fdroid/`,
  listing assets) is the reference. Store listing art carries keeper
  chains (AST-02).

#### REL-07 — Secret canary
- **Build:** a test that plants canary secret values in the `pass`
  store fixture, runs the full Rung 8 flow, and greps every byte of
  Meister state, logs, and outputs.
- **Done when:** zero canaries are found.

#### KT-01 — Kotlin MCP companion (decision D11)
- **Build:** Android-side authoring, preview, and playtest bridges,
  following the fleet pattern. Feature-gated and compiled out of
  release and store artifacts unless explicitly enabled.
- **Done when:** a release artifact built without the feature flag
  contains no companion symbols.

#### GATE-8 — Rung 8 gate
- **Includes:**
  - the published gate, run on the D10 release example (recommended:
    light-show) against one pipeline channel and one
    prepared-submission channel;
  - REL-05's photosafety refusal;
  - REL-07 clean;
  - version 0.9.0.
- **Needs:** GATE-6, GATE-7 (the release example's pack), D10, D11.

---

## 12. Rung 9 — 1.0

**The 1.0 definition (README §"What 1.0 means"):**

- A *new* SFW game, in any genre with a v1 pack, is taken from scaffold
  to release handoff entirely through Meister and its routed
  specialists:
  - canon locked;
  - content authored as data;
  - media produced through lumen with provenance;
  - playtests deterministic;
  - channels preflighted.
- The worked examples prove it, **one per category pack**, with
  conformance records the Owner has gated.

#### V1-01 — The new-game proof
- **Build:** a title **born in Meister**. Niflheim does not qualify: it
  is adopted, not new.
  - Steps: `meister_project_create` → genesis confirmed →
    `bevy-shell/1` → cast locked → content → assets → freeze → emit →
    QA → preflight → handoff.
  - The README's own suggestion is a jam entry through a pack.
- **Done when:** every stage has its conformance record and the Owner
  gates the title.
- The **Owner names the title** (decision D10).

#### V1-02 — The worked-example audit
- **Build:** a table of all 13 packs, each linked to its worked
  example's conformance records and the Owner's verdict ref.
- **Done when:** 13 of 13 rows are Owner-gated. Anything less ships as
  an honestly numbered 0.x.

#### V1-03 — Surface closure
- **Build:**
  - the final EFLO re-measure;
  - every profile within budget or waived by ruling;
  - client tolerance (CT-01) re-run on the 1.0 binary;
  - COMPLIANCE.md and CHANGELOG at 1.0.0.

#### V1-04 — Publication
- **Build:**
  - crates.io `meister`;
  - the MCP Registry as `ai.qompass/meister`, with `server.json`
    packages populated with **real** URLs and hashes (never invented,
    COMPLIANCE §8);
  - an agent card version bump that keeps manifest agreement.
- Publication is an outward act. **It needs the Owner's explicit
  authorization at the time.**

#### V1-05 — Publish-flow proof
- **Build:** reinstall the published artifact, then probe the installed
  binary's own version report.
- **Done when:** the installed binary answers `1.0.0`. A publish is not
  complete until then.

#### GATE-9 — 1.0
- **Includes:** V1-01 through V1-05.

---

## 13. Open decisions for the Owner

Each decision lists options with their costs, then a recommendation.
None is decided here.

| # | Decision | Options (cost) | Recommendation |
|---|---|---|---|
| D1 | Adopt the §2 re-sequencing | (a) all of M1–M6 (README path changes by ruling); (b) M3 and M6 only (fixes the inversions, Niflheim value comes later); (c) keep the published order (Rung 4 can't pass honestly) | (a) |
| D2 | Niflheim's pace against Meister | (a) shadow mode, NIF-01 (double authoring for 1–3 floors, which become regression oracles); (b) hold floors 13+ until Rung 3 (stalls Niflheim); (c) make STAGE and combat presentation the worked example instead (blocked on the R3-S5 verdict) | (a) |
| D3 | Emit target for Niflheim | (a) generated Rust modules (fits content-as-Rust, checked at compile time; costs a regeneration step); (b) RON plus a runtime loader (changes the architecture, adds runtime parse failures) | (a) |
| D4 | Tools for Freeze and Emit (none in the plan of record) | (a) `meister_verdict_record` + `meister_emit` (+2 tools, in the `narrative` profile); (b) emit as a CLI outside MCP (saves EFLO, splits the surface); (c) fold Freeze into `canon_check` (a read tool that writes) | (a) |
| D5 | EFLO | (a) profiles in Rung 2, `core` as default (§5); (b) extend the waiver to 1.0 (unbounded growth); (c) shorten descriptions (loses enforcement semantics). Also: confirm the budget unit as 2,500 **tokens** | (a) |
| D6 | Niflheim floors 1–12 | (a) forward-only; (b) import canon and cast only (keepers, FOEs, codex principles as sourced canon facts) for continuity; (c) import every floor in full (a large one-off cost) | (b) |
| D7 | Where Niflheim's Meister records live | (a) a dedicated local-only git repo under the store root (versioned, never pushed); (b) inside the Niflheim worktree (several worktrees drift); (c) a plain store (no history) | (a) |
| D8 | When to encode STAGE | (a) after the R3-S5 verdict; (b) now, as provisional (freezes a guess) | (a) |
| D9 | Narrative/VN pack worked example | (a) the Owner names a title; (b) the pack waits | (b) unless a title is named |
| D10 | The 1.0 bar, and the titles for V1-01 and GATE-8 | (a) as written, 13 Owner-gated examples; (b) 1.0 covers the packs that have examples, the rest ship as 0.x previews | (a); light-show for GATE-8 |
| D11 | The Kotlin companion (no rung owned it) | (a) Rung 8 with slipway; (b) Rung 7 with the TUI; (c) after 1.0 | (a) |
| D12 | `two_model` form kind `adult` (Light Show's age form) | (a) keep; (b) rename (e.g. `grown`) so no reader or lane heuristic confuses an SFW record with the other lane | Owner's call; this is a Rung 1 lane change |

---

## 14. Risks and unknowns

- **K1. The 1.0 bar needs about 13 worked examples, and most of the
  titles don't exist.** This is the largest schedule risk (D10).
- **K2. Pace.** Niflheim may finish floors 13–18 before Rung 3 can emit
  (D2).
- **K3. Emit is greenfield.** Bevy has no commercial canon. Generated
  Rust must be byte-stable under the nightly pin.
- **K4. Content could leak into this public repo** through fixtures,
  birth records, or evidence. Mitigation: synthetic fixtures only, and
  evidence kept on primo.
- **K5. Multi-process store races** once profiles land (STORE-01).
- **K6. Malformed records from local models** are the normal case.
  SYS-10's corpus must come from real model output.
- **K7. The real token count is unmeasured** (EFLO-01). It may be worse
  than the estimate.
- **K8. Scene-verdict dependency.** STAGE-01 and Niflheim's combat
  presentation wait on the Owner's eye. The schedule does not try to
  control that.
- **K9. Two censuses could diverge** until the AST-07 amendments land in
  lumen.
- **Unknowns:**
  - U1: the exact text of D-1658 and D-1659.
  - U2: the merge of Rung 1 to `main` (the gate record exists on its branch).
  - U3: the fleet TUI programme's criteria.
  - U4: client tolerance (never run).
  - U5: `flock` semantics on primo's store filesystem.
