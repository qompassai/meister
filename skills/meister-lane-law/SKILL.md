---
name: meister-lane-law
description: >-
  Meister's lane law: born-classified records (origin_lane qompass,
  content_label sfw, evidence from birth), the live lane gate and
  its typed refusals (lane_mismatch, lane_unclassified), fail-closed
  intake, and what never crosses lanes. Use when touching envelopes,
  records, intake paths, classification, birth records, or any code
  that creates, accepts, or moves content or records for meister.
license: Apache-2.0
---

# Meister — lane law

The law is in the plan of record (README §"Lane law (never
softened)") and is never softened. This skill is how it is
applied in code and records.

## The stamp

Every record Meister creates carries, from birth:

- `origin_lane: qompass`
- `content_label: sfw`
- `evidence` — what supports the label (a README section, a gate
  record, a ruling). A claimed label without evidence is not
  classified; it is unclassified.

Records are born classified, never classified after the fact.

## The gate

Every record-carrying tool input is an envelope
(`record_id`, `origin_lane`, `content_label`, `evidence`). The
shared guard in `src/lane.rs` runs BEFORE any handler logic:

- `content_label: nsfw` → refuse `lane_mismatch`, from any origin.
- A non-sfw label from `origin_lane: ntranch` → `lane_mismatch`.
- An sfw label claiming `origin_lane: ntranch` → `lane_mismatch`:
  records never cross lanes, in either direction, ever.
- Missing/unreadable label, unrecognised label or origin, or
  missing evidence → refuse `lane_unclassified`.
- Only qompass + sfw + evidence proceeds.

Unclassified defaults to the NSFW side — fail closed. Refusals
are typed JSON naming the offending record id, and happen before
any mutation: a refused run leaves inputs byte-identical, and
content questions that arrive unclassified are refused, never
investigated by processing them.

`meister_lane_check` exposes the gate's verdict for a candidate
envelope without storing anything; it is the demonstration and
debugging surface for this law.

## What never crosses

No content, no records, no canon move between the Qompass lane
and the NSFW lane — in either direction. The two maker servers
share a spine shape and group numbering (mechanics only). Do not
read, copy, or adapt material from the other lane's trees when
working on meister; meister's surface derives from its own plan
of record alone.

## Verdicts

No tool declares a visual or audio PASS. Measurements are
floors; the Owner's eye and ear are the verdict, recorded through
numbered verdict packs. An unverifiable number routes to REVIEW,
never to PASS (D-713).
