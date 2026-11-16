---
name: meister-tool-surface
description: >-
  Tiger Style MCP rules for meister's tool surface: adding a tool,
  changing a schema, or promoting a seam to a real handler at its
  rung. Covers schema completeness (explicit properties, always),
  output schemas, annotations from birth, the EFLO budget and its
  waiver, the lane-gate-first dispatch order, error taxonomy, and
  birth records for new schemas. Use for any change to src/catalog.rs,
  src/server.rs dispatch, or the served tools/list.
license: Apache-2.0
---

# Meister — tool surface

The estate's Tiger Style MCP skill governs; this skill is its
meister-specific application. The surface is the contract: draft
the schema before the implementation, always.

## Adding a tool

1. Place it in its README tool group (1–10) and name it
   `meister_<object>_<verb>`-style, consistent with the existing
   catalog in `src/catalog.rs`. One server, one domain: a tool
   from a second domain means a split server or no tool.
2. Write the full input schema in the same commit as the tool:
   an object with an explicit `properties` key (zero misses —
   an absent `properties` is the estate's known defect class D1),
   required fields named, bounds in the schema (list limits,
   string patterns) not just in prose.
3. Publish the output schema for the success shape, and enforce
   it in tests; never ship a decorative schema.
4. Annotate from birth: `readOnlyHint`, `destructiveHint`,
   `idempotentHint`, `openWorldHint` set deliberately, and
   declared access must match detected access in both directions.
5. If the tool carries a record, it takes the `envelope`
   argument and the lane gate runs before its logic (see the
   meister-lane-law skill). The envelope's `content_label` is
   never an enum in the schema — the gate owns classification,
   so a bad label is refused with a typed lane code, not a
   protocol error.
6. Errors: execution failure is an `isError` tool result with a
   structured body; protocol errors are reserved for protocol
   faults. Seam state is the typed `not_implemented` error naming
   the tool, its group, and its rung.
7. Write the tool's bloch birth record under `records/birth/`
   (see records/README.md) in the same change.
8. Gate it: catalog unit tests (count, properties, envelope
   requirement) and the stdio conformance assertions must pass;
   update COMPLIANCE.md's surface numbers from the run.

## Budget (EFLO)

Soft budget: 25 tools and ≈2,500 tokens of served tools/list
payload. Meister's Rung 0 surface is 31 tools by plan of record
(the full ten groups as seams + the live gate) and carries a
gated waiver with the measured payload in COMPLIANCE.md.
New tools beyond the plan of record need the same treatment: a
written reason before the release is gated, and group-scoped
loading (the README groupings) is the standing mitigation —
keep groups cleanly separable so a session can load one group
without dragging the rest.

## Promoting a seam to real

A tool becomes real at its rung (README §"The path to 1.0"),
when the rung's own gate passes — not before, and the seam error
is removed only by the change that implements the handler and
its tests (half validation, half adversarial). The promotion
updates: the handler, the catalog entry's behaviour (schemas
change only through the contract gate, never silently), the
birth record's successor evidence, and COMPLIANCE.md.
