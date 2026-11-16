# Building meister

All building and validation happens on primo, in this repository
(`~/.GH/Qompass/meister`), under the estate's build discipline.
No containers; no CI (the estate runs none, by design).

## Primo build discipline

```sh
cd /home/phaedrus/.GH/Qompass/meister
. "$HOME/.cargo/env"
export RUSTC_WRAPPER=
flock -w 7200 /home/phaedrus/.local/share/pax/night-locks/build.lock \
  nice -n 10 cargo build
```

Every cargo invocation runs the same way: the cargo environment
sourced, `RUSTC_WRAPPER` emptied (no wrapper cache in the gate
path), `nice -n 10`, and the shared build lock held so primo
never runs two estate builds at once. Target directories stay on
`/home` (the repo's `target/`), never `/tmp`.

## Gate sequence

A change is done when all of these pass on the machine, in this
order (also `just gate`):

1. `cargo fmt --check`
2. `cargo clippy --all-targets -- -D warnings`
3. `cargo test` — unit tests (era guard, lane gate, catalog),
   the stdio conformance suite (half validation, half
   adversarial), and the manifest-agreement tests.
4. Theodolite LSP validation for changed Rust files (below).
5. Optional: regenerate the SCIP index (§SCIP) when the surface
   changed materially, so downstream code intelligence is fresh.

Numbers in a gate report cite their run output. A gate that was
not run is named as not run, never implied.

## Theodolite copy-route validation

Rust diagnostics for this tree are validated through the
theodolite LSP bridge using the estate's copy-route protocol
(the bridge's bacon-ls roots through git, so a session opened on
this nested workspace would read vacuous zeros):

1. Make a byte-identical copy of the tree in its own throwaway
   git repository; prove identity with a sha256 manifest of the
   source files.
2. Producer first: start `bacon --headless -j bacon-ls` in the
   copy so the `.bacon-locations` file exists and is current
   BEFORE the theodolite session starts.
3. Open the session on the copy and run the control round-trip in
   that session: clean witness file reads 0 diagnostics → plant a
   known error (e.g. an E0308 type mismatch) → the planted error
   is seen → restore → 0 again. A zero without the control is
   unverified, and a failed control voids the zeros it guards.
4. Only then read verdicts for the files under change. Remove
   the throwaway copy when done.

## Registering with diver (snippet only)

Diver-side registration follows the Owner's live-first workflow
(live `~/.config/nvim` first, validated, then mirrored into the
Diver repo) and is NOT part of any meister commit. When that step
happens, the entry for `lua/ai/mcp/declared_servers.lua` is:

```lua
    {
        -- Local binary (estate layout): cargo-installed meister,
        -- MCP modern era only (2026-07-28) over stdio. No secrets:
        -- group 10 manages credential references, never values, so
        -- no secrets block belongs here.
        name = 'meister',
        command = '/home/phaedrus/.local/bin/meister',
        args = {},
        enabled = true,
    },
```

The same facts live in the agent card's invocation extension
(`.well-known/agent-card.json`), which is the canonical source
the snippet is derived from.

## SCIP

`index.scip` is the SCIP code-intelligence index for this
repository: symbol and reference data that agents and tooling
across the estate use for precise navigation (definitions,
references, implementations) without re-parsing the tree.

Regenerate it with:

```sh
just scip
# which runs: rust-analyzer scip . --output index.scip
```

(under the same build discipline: cargo env sourced, build lock
held). The index is a generated artifact: it is regenerated on
demand, listed in `.gitignore`, and never committed — the estate
treats index files the way the bloch commit-scope ruling treats
generated files: produced when needed, kept out of the tree.

## Agent cards and client configs (direction)

Per D-1605, per-server agent cards are the fleet's canonical
capability manifests, and client configs for the estate's agent
runtimes (diver's declared servers, Claude Code, Codex) are to be
GENERATED from card + server.json pairs. That generator is a
follow-on programme, not Rung 0 scope. Rung 0's obligation is
that Meister's pair is sufficient and in exact agreement: the
agreement invariant is stated in COMPLIANCE.md §8 and held by
`tests/manifest_agreement.rs`.
