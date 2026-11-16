# Changelog

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
