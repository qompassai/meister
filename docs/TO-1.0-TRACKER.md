# Meister — To-1.0 Programme Tracker

Owner ruling 2026-11-17 (Matt): this file is the live tracker for the
road from v0.5.0 (Rungs 0-4 on main, `0117079`) to Meister 1.0. The
plan of record is `docs/PATH-TO-1.0.md`; this tracker is its working
state. Every lane reads this file before starting work and updates
its rows as work moves.

## How to work this tracker

- Statuses: `TODO` -> `CLAIMED` (add lane + date) -> `IN-PROGRESS`
  -> `DONE-PENDING-GATE` -> `DONE` (Pax flips this after the gate)
  or `BLOCKED` (add the reason in the row).
- Claim before you build. One lane per task. If a task is CLAIMED by
  someone else, pick another row or ask Pax.
- Commit tracker updates with the work they describe, on your task
  branch. Never commit to `main`; merges to main are the Owner's
  word, relayed by Pax.
- Gates are Pax's: tests + clippy + fmt by exit code (never through
  a pipe), theodolite where the rung calls for it, verdict packs for
  Owner-gated items. A gate number cites its run artifact (D-713).
- Lane key: PAX (the coordinating agent), CLAUDE (Opus lane),
  ASTRA (Codex lane), AZOTH (Grok; research/dossiers only),
  LOCAL (rose/granite local models; bounded batches, proposals
  only, Pax-gated), OWNER (Matt's act or verdict).

## Phase 0 — queued rulings and hygiene

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| P0-1 | `two_model` rename `adult` -> `grown` (schemas, records, adopted canon store; branch; full gates + census) | PAX | DONE-PENDING-GATE | Owner ruling D12, 2026-11-17. Executed on branch `pax/phase0-rulings-20261117` (gates 213/0, clippy/fmt rc=0); landing returns for per-push confirmation. |
| P0-2 | Floor 12 real verdict append (Niflheim proof store, after the NIGHT-FIXTURE entry; census stays 0 unverified) | PAX | DONE | Owner adopted Floor 12 as canon 2026-11-17. Appended 2026-11-17 as pack `pack-emt04-owner`, seq 2, ruling adopt, 7 subjects; the fixture record is untouched; canon census re-run under the 0.5.0 binary: 55 checks, all verified, pass. |
| P0-3 | EFLO waiver into COMPLIANCE S3 (2,926 vs 2,500 tokens, GATE-4 measurement) | PAX | DONE-PENDING-GATE | Recorded in COMPLIANCE S3 on branch `pax/phase0-rulings-20261117`; lands with that branch. |
| P0-4 | PATH-TO-1.0 amendment: SYS-11 recommended-prerequisite edges + chamber->scene refs as Rung 5 tasks; V1-01 title recorded (P0-5) | PAX | DONE-PENDING-GATE | SYS-11a/SYS-11b added as Rung 5 tasks and the V1-01 title recorded in PATH-TO-1.0, on branch `pax/phase0-rulings-20261117`; lands with that branch. |
| P0-5 | V1-01 title of record: **Time Waits: Gear Game** (platformer) | OWNER | DONE | Named by Matt 2026-11-17 08:52 PST. |
| P0-6 | Reinstall Meister from merged main `0117079` on primo; probe installed binary version (D-1543) | PAX | DONE | Reinstalled 2026-11-17 from merged main `0117079`; the replaced binary was v0.2.0; the installed binary's own discover response reports meister 0.5.0 (D-1543 probe). |

## Rung 5 — The flagship pack: Role Playing x Educational

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| PK-01 | Pack mechanism: packs as data plus emitters, never tools; invariant test | CLAUDE | DONE 2026-11-17 | Pax gate PASS: branch `pax/rung5-pack-mechanism-20261117` @ `df6c108`, tests 231/0 (18 new: 9 validation + 9 adversarial), clippy/fmt rc=0, theodolite copy-route PASS. Selection not yet on a live call path — carrier tool is an RPG-01 decision; the dead-code debt is `expect`-guarded so it cannot rot silently. Branch push awaits Owner word. |
| RPG-01 | Role Playing pack v1 data | CLAUDE | TODO | Depends on PK-01. |
| RPG-02 | RPG playtest generators (traverse floors as a player would) | CLAUDE + LOCAL | TODO | LOCAL runs traversal batches; results are evidence, not verdicts. |
| TEACH-01 | Teaching layer v1 | CLAUDE | TODO | Both kinds land in the Educational secondary pack. |
| SYS-11a | progression_dag recommended-prerequisite edges | CLAUDE | TODO | From P0-4 amendment. |
| SYS-11b | chamber->scene record references checked at compose | CLAUDE | TODO | From P0-4 amendment. |
| PK-02 | Second-pack smoke: data-only Puzzle pack skeleton against PK-01 schema | ASTRA | TODO | Depends on PK-01. No code changes to the mechanism. |
| STAGE-01 | STAGE scene record (ruled scene idiom as data) | CLAUDE | DONE-PENDING-GATE 2026-11-18 | Gate conditions all satisfied 2026-11-18: (1) Owner's S0 verdict on the R3-S6 pack = PASS 2026-11-18 08:28 ("this is way better it does capture the volume") — the R3-S5-verdict gate is satisfied via the R3-S6 cure; (2) R3-S6 tree disposition settled — cure committed `bf36c49` on `local/scene-r3s6-20261117`, shell rung `41e54e4` and boundaries `1198476` committed on top in the same worktree; (3) landing riders (GUI title string, cast-record re-stamp) RULED OUT of STAGE-01's scope by Owner ruling 2026-11-18 (Decision 15) — no title-string work, no re-stamp in this task (the re-stamp was discharged separately by the 2026-11-18 census re-lock, 55/0). Idiom transcribed from the R3-S6 cure as built: one projected vault (one-point perspective, VP room-local (624, -42), s(y) = 1.45*(y+42)/554, figures/doors at s(y)/1.30), one warm light pool at the camp stone (ember tone, stone light capped 0.85x), full-room framing (cam scale 1.00), room luminance 42.8 / 29.5 / 108.9 against the D-1676 reference's 33.1 / 22.0 / 107.6, doors as dark leaves with slate plaques, the standing panel replaced by the on-demand Guide pop-up (D-1672). Known misses enter as embeddedness check rows flagged below-bar: S16 architecture:figure ratio ~1.09-1.15, S17 no foreground occluder plane, S18 Guide plate parchment/sans (not dark bracketed serif). Branch `pax/rung5-stage01-20261118`. LANDED on branch `pax/rung5-stage01-20261118` (implementation `f7066d6`): the optional stage block on `meister_scene_define` (define-time shape only), the embeddedness verdict on `meister_scene_validate` (refusal codes merged into refusals; `stage_findings` carries the C1-C7 / D-1658 refusal rows and the S15-S18 below-bar flags with measured milli values). Gates, lane-run (logs in the lane gates dir): cargo test 230/0 (baseline 213 + 17 new: 7 validation, 10 adversarial), clippy --all-targets -D warnings clean, cargo fmt --check clean, theodolite copy-route on src/store.rs + src/catalog.rs 0 -> planted E0308 caught -> byte-identical restore -> 0. Promotion records records/rung5/meister_scene_define.json + meister_scene_validate.json (served hashes 75a63cca..., f93d8ce9...). NO PUSH, NO MERGE - Pax gate pending. |
| NIF-02 | Worked example: one Niflheim floor end to end through Meister | PAX + CLAUDE | TODO | Owner gates the floor's verdict pack. |
| GATE-5 | Rung 5 gate (NIF-02, PK-01 invariant, PK-02, EFLO re-measure) | PAX | TODO | EFLO re-measure run by LOCAL as a gated batch. |

## Rung 6 — Standards, accessibility, QA

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| QA-01 | `meister_rating_record` | ASTRA | TODO | Spec-tight, test-first. |
| QA-02 | `meister_content_summary` | ASTRA | TODO | |
| QA-03 | `meister_accessibility_check` | ASTRA | TODO | |
| QA-04 | `release_eligible(project)` predicate | ASTRA | TODO | |
| QA-05 | `meister_playtest_define` | ASTRA | TODO | |
| QA-06 | `meister_replay_check` | ASTRA | TODO | Niflheim-first per plan. |
| QA-07 | `meister_defect_record` | ASTRA | TODO | |
| QA-08 | Screenshot specs for verdict packs | ASTRA | TODO | |
| GATE-6 | Rung 6 gate | PAX | TODO | |

## Rung 7 — Remaining packs, teaching layer, TUI

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| RS-7 | Genre dossiers for the remaining pack categories (13-category taxonomy minus RPG/Puzzle/Platformer): one dossier per category, eight reference tools, no fabrication — flag unverifiable | AZOTH | TODO | SFW research; dossiers gate through Pax before any build. |
| PK-7a | Platformer pack (priority: V1-01 rides on it) | ASTRA | TODO | After its dossier gates. |
| PK-7b | Remaining pack builds from gated dossiers | ASTRA | TODO | One pack per commission. |
| CONF-7 | Per-pack conformance dry-runs | LOCAL | TODO | Batches; Pax gates results. |
| TEACH-7 | Teaching layer completion | CLAUDE | TODO | |
| TUI-7 | Meister TUI (initial build — none exists; pulled forward from Rung 7 by Owner ruling 2026-11-17) | PROGRAMME | CLAIMED 2026-11-17 | Built by the fleet TUI completion programme (coordinator lane pax-tui-rung2) on branch `pax/meister-tui-20261117`; full pattern per DESIGN D2. |
| GATE-7 | Rung 7 gate | PAX | TODO | |

## Rung 8 — Release

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| RS-8 | Channel-requirements dossiers: Google Play, F-Droid, itch.io, Steam (current rules, fees, review mechanics; primary sources) | AZOTH | TODO | Feeds REL-01/02 design. |
| REL-01 | Channel registry data (`meister_channel_list`) | CLAUDE | TODO | |
| REL-02 | Live verification per channel | CLAUDE | TODO | |
| REL-03 | `meister_account_register` / `meister_account_inspect` | CLAUDE | TODO | |
| REL-04 | `meister_account_verify` | CLAUDE | TODO | |
| REL-05 | `meister_preflight_run` | CLAUDE | TODO | |
| REL-06 | `meister_release_prepare`: slipway handoff | CLAUDE | TODO | |
| REL-07 | Secret canary | CLAUDE | TODO | |
| KT-01 | Kotlin MCP companion (with slipway) | CLAUDE | TODO | Owner ruling D11, 2026-11-17. |
| GATE-8 | Release worked example: light-show shipped through Meister | PAX + OWNER | TODO | Owner ruling 2026-11-17: GATE-8's example is light-show. Store-side acts are the Owner's. |

## Rung 9 — 1.0

| ID | Task | Lane | Status | Notes |
|---|---|---|---|---|
| V1-01 | **Time Waits: Gear Game** (platformer) born in Meister: project_create -> genesis -> bevy-shell -> cast lock -> content -> assets -> freeze -> emit -> QA -> preflight -> handoff, conformance record at every stage | CLAUDE + ASTRA | TODO | Depends on PK-7a. Split streams: engine-side / content-side. Owner gates the title. |
| V1-02 | Worked-example audit (~13 Owner-gated examples across the packs) | PAX | TODO | The 1.0 bar stands as written (Owner ruling D10). |
| V1-03 | Surface closure | CLAUDE | TODO | |
| V1-04 | Publication | PAX | TODO | |
| V1-05 | Publish-flow proof: reinstall + version probe of the published build (D-1543) | PAX | TODO | |
| GATE-9 | 1.0 gate | PAX + OWNER | TODO | |

## Owner gates on the critical path

- R3-S5 verdict — LANDED via the R3-S6 cure: Owner S0 PASS 2026-11-18 08:28; STAGE-01 unblocked and claimed (see its row).
- NIF-02 floor verdict (GATE-5).
- GATE-8 store-side acts; V1-01 title gate (GATE-9).
