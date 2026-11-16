---
name: meister-build-and-gate
description: >-
  Build and gate meister on primo: the cargo discipline (env, build
  lock, nice), the gate sequence (fmt, strict clippy, the full test
  split), theodolite copy-route LSP validation for this tree, and
  SCIP index regeneration. Use when compiling meister, running or
  interpreting its tests, validating a change before calling it
  done, or refreshing index.scip.
license: Apache-2.0
---

# Meister — build and gate

Meister builds on primo only, in `~/.GH/Qompass/meister`, under
the estate discipline. Full text: `docs/BUILDING.md` in this
repo. This skill is the operational core.

## Build discipline

```sh
cd /home/phaedrus/.GH/Qompass/meister
. "$HOME/.cargo/env"
export RUSTC_WRAPPER=
flock -w 7200 /home/phaedrus/.local/share/pax/night-locks/build.lock \
  nice -n 10 cargo <verb>
```

Every cargo verb runs inside that shape: env sourced, wrapper
emptied, `nice -n 10`, the shared build lock held. Target dirs
stay on `/home`, never `/tmp`. Toolchain is pinned by
`rust-toolchain.toml` (nightly-2026-10-30, matching the fleet);
never substitute the system toolchain for a gate.

## Gate sequence

In order; a change is done when all pass, with numbers from the
run output:

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test` — unit (era guard, lane gate, catalog), stdio
   conformance (half validation / half adversarial), manifest
   agreement. Report passed/failed counts per suite.
4. Theodolite validation for changed Rust files (below).
5. Optional: `just scip` to regenerate the SCIP index when the
   surface changed materially.

A gate not run is named as not run. Numbers never come from
memory or from a previous run's output.

## Theodolite copy-route (this tree)

The bridge's bacon-ls roots through git, so a session on the
repo itself reads vacuous zeros. Use the copy route:

1. Byte-identical copy of the tree in its own throwaway git
   repo; sha256 manifest proves identity.
2. Producer first: `bacon --headless -j bacon-ls` in the copy so
   `.bacon-locations` exists and is current before the session
   starts.
3. Control round-trip in that session: clean witness = 0 →
   planted error (E0308) seen → restored = 0. A zero without the
   control is unverified; a failed control voids its zeros.
4. Read verdicts only after the control passes. Delete the
   throwaway copy at the end.

## SCIP

`just scip` runs `rust-analyzer scip . --output index.scip`:
the code-intelligence index agents use for symbol navigation.
Generated on demand, gitignored, never committed (estate rule
for generated/index files). Regeneration is an optional gate
step, not a substitute for any gate above.
