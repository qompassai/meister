---
name: meister-genre-packs
description: >-
  Meister's genre categories and packs: the thirteen standalone
  categories, what a category pack is (specialised authoring verbs,
  media profile, systems templates, playtest generators, gate set),
  pack selection at project birth, the worked-example rule, and the
  teaching layer as the Educational category's cross-genre export.
  Use when scaffolding a project, selecting or adopting a genre
  pack, adding a pack, or reasoning about genre coverage.
license: Apache-2.0
---

# Meister — genre packs

Source of record: README §"Genre categories" and §"The teaching
layer (cross-genre)". Every genre is its own category.

## The thirteen categories

Action · Adventure · Educational · Fighting · Narrative / visual
novel · Platformer · Puzzle · Rhythm · Roguelike · Role Playing ·
Shooter · Simulation · Strategy.

Slugs as served in `meister_project_create`'s `genre_pack` enum:
`action`, `adventure`, `educational`, `fighting`, `narrative`,
`platformer`, `puzzle`, `rhythm`, `roguelike`, `role_playing`,
`shooter`, `simulation`, `strategy`.

## What a pack is

A category pack is a named, versioned bundle: the authoring
verbs (tool groups 3–5 specialised), the media profile (which
lumen formats, at what budgets), the systems templates, the
playtest generators, and the gate set for that genre. Packs are
data and emitters, not code forks: the spine
(Describe → Propose → Freeze → Emit → Prove) and the tool groups
are shared; only the pack differs.

A project picks its pack at scaffolding (group 1) and can adopt
a second pack when a game genuinely spans genres — a teaching
RPG is Role Playing carrying the Educational category's teaching
layer, not a merged pack.

## The worked-example rule

A pack is only real when a worked example of record has been
produced end to end through it, with conformance records the
Owner has gated. Standing worked examples: Role Playing ×
Educational → a Niflheim floor (Niflheim teaches Rust); Puzzle →
light-show's fiber-optic puzzles; Educational's teaching layer →
Versemend (teaches Lua). Rung 4 lands the first packs (role
playing flagship, narrative, puzzle); Rung 7 completes the
thirteen.

## Research discipline behind the packs

Each category's studied exemplars are recorded in the README
with engine and art-tool layers kept separate; a tool is claimed
for an exemplar only where a source supports it, and
unverifiable attributions are marked, never repeated (the Dead
Cells/Blender claim is a misattribution — 3DS Max, per its
artist). Two findings shape pack work:

- Bevy has no commercial canon in any category: Meister's Bevy
  emitters are greenfield, and the estate's own Bevy titles are
  their worked examples of record.
- Meister records art-tool provenance per asset at intake
  (group 7) instead of inheriting the industry's gaps.

## The teaching layer

Not a genre: the Educational category's exportable layer, which
any pack can carry — curriculum as canon records, lessons
embedded as mechanics (the top tier; the narrative quiz frame is
the honest fallback), assessment as authored encounters with
measured outcomes, progress state a teacher or learner can
inspect. The four embedding patterns are lesson-as-mechanic,
lesson-as-gate, assessment-as-encounter, and the narrative quiz
frame.
