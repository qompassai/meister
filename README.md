# Meister — Qompass SFW game maker MCP

Commissioned by the Owner, 2026-11-16 (decision D-1604), as the plan of
record for building Meister to 1.0. Name ruled by the Owner, 2026-11-16
(decision D-1603): **Meister** — the guild master, the rank earned by
submitting a masterwork. Its own ancestor *magister* was the runner-up.

Meister is the SFW counterpart to [Ardor](https://github.com/NTRanch/ardor),
the NTRanch lane's NSFW maker MCP. The pair share a spine and a shape —
one maker surface per lane — and nothing else crosses: no content, no
records, no canon. Ardor makes games of passion; Meister makes games of
craft.

## Purpose

One Qompass-lane maker surface for building SFW games end to end:
the teaching games (Niflheim, Versemend), the puzzle line (light-show),
narrative games, and every future Qompass title. Meister is an
authoring and project-state server. It is not a runtime or
orchestration replacement for phlow, not a media engine (that is
lumen), and not a publishing surface (that is slipway).

The worked examples already exist. Niflheim and Versemend were built
by hand through the process this server formalises: decisions became
gated, deterministic production. Meister exists to make that way of
working the default for every SFW title — scaffold, canon, authored
content, media, playtest, and release handoff, all as structured game
data with provenance, all reproducible.

## Why a server

Meister holds authored project state across sessions and enforces
canon and lane decisions through a decide()-class gate before
canonical writes. State plus enforcement meets the estate's server
graduation rule; thin one-shot capabilities remain skills. The SFW
content pipes currently homed in lumen as tooling and gate scripts
(genesis, canon census, keeper chains) are Meister's seed: the three
graduation candidates named in the lumen pipes definitions move
server-side here, on the demonstrated enforcement need this
commission records. Everything else in lumen stays in lumen —
Meister routes media work to it and records the handoff.

## Technology contract

- Rust server on rmcp, MCP modern era only (2026-07-28),
  stateless/discover-first, per the fleet standardisation ruling.
- Tiger Style MCP discipline: bounded tool surface, complete
  schemas, annotations from birth, explicit error, timeout, and
  security contracts; the tool surface stays inside the fleet EFLO
  token budget or carries a gated waiver with evidence.
- Kotlin MCP SDK companion for Android-side authoring, preview, and
  playtest bridges, following the established fleet pattern;
  Android-facing pieces are feature-gated and compiled out of
  release/store artifacts unless explicitly enabled.
- Pure Rust core where it owns game data and project state; edition
  2024 and the estate nightly pin apply to Rust game code.
- A TUI of fleet-standard capability is part of the 1.0 surface,
  per the fleet TUI programme: plan/auto modes, live progress,
  media preview at the highest fidelity the terminal stack allows.
- No containers anywhere in the build or deployment path. Local
  validation only; the estate runs no CI by design.

## Lane law (never softened)

- Every record Meister creates carries `origin_lane: qompass` and
  `content_label: sfw` from birth, with evidence. Meister is a
  Qompass-lane system; its records are born classified, never
  classified after the fact.
- Intake accepts only `content_label: sfw`. NSFW input is refused
  with a typed `lane_mismatch`; missing or other labels are refused
  as `lane_unclassified`. Unclassified defaults to the NSFW side —
  fail closed. Refusals are typed, name the offending ids, and
  happen before any mutation: a refused run leaves inputs
  byte-identical.
- Nothing NSFW is authored, stored, or processed in a Qompass tree.
  Content questions that arrive unclassified are refused, never
  investigated by processing them.
- No tool declares a visual or audio PASS. Measurements are floors;
  the Owner's eye and ear are the verdict, recorded through numbered
  verdict packs. A pipeline stage that cannot produce its gate
  evidence has not happened.

## The spine

Every Meister pipeline is the lumen content-factory spine applied to
one kind of state:

**Describe → Propose → Freeze → Emit → Prove**

- **Describe** — the canon record is written first. Generation
  against an unwritten record is forbidden.
- **Propose** — a candidate is produced against the record. A
  proposal is not an asset; it is born `candidate` and stays
  quarantined from canon.
- **Freeze** — adoption is a ruling, not a side effect. Canon is
  adopted or changed only by the Owner's ruling on a numbered
  verdict pack; the freeze pins a hash.
- **Emit** — named, versioned, deterministic emitters turn frozen
  records into engine-ready artifacts. Same record in, byte-
  identical artifacts out.
- **Prove** — every stage leaves a conformance run record with its
  gate evidence, or it did not happen. Numbers in a gate report
  cite their run artifacts; an unverifiable number routes to
  REVIEW, never to PASS.

## Tool groups

The surface mirrors Ardor's ten groups, re-homed to the SFW lane.
Group numbers match Ardor's deliberately, so the pair can be read
side by side; where a group's content is lane-specific, the SFW
form is specified here and the NSFW form stays in Ardor, unshared.

1. **Project scaffolding** — create and inspect Qompass game
   projects with the estate conventions already in place: engine
   shell (Bevy first; Godot, Ren'Py, Unity, and Unreal targets as
   emitter profiles), metadata, test layout, lane stamps, and the
   project's genre pipeline pack (below) selected at birth.
2. **Cast and canon** — character identity records, identity locks
   (silhouette, palette, face and body proportions, thumbnail
   recognisability), appearance and rig references, canon checks,
   and versioned changes to cast data. A character is identity-
   locked at intake, before any detail work — the lock is a hard
   gate, not a style preference.
3. **Scenes and systems authoring** — dialogue and scene graphs,
   schedules, flags, progression DAGs, economies, state machines,
   and minigame definitions: the game-systems authoring verbs every
   genre pack builds on.
4. **Dialogue and narrative creation** — author dialogue as
   structured game data, not loose prose: lines, speakers, choices,
   conditions, per-character reaction profiles driven by
   relationship state, prior choices, and scene history;
   prerequisites and continuity effects; branching; and cues for
   animation, voice, and sound. Expression is whole-body: a
   reaction record names posture, gesture, and stance, not only a
   face. This is Ardor group 4's SFW form — the adult state
   machinery (arousal, conditioning, kink actions, haptics) does
   not exist here and is never imported.
5. **Story, plot, and level making** — create and validate story
   arcs, beats, quests and objectives, character arcs, relationship
   progression, branching endings, scenes, encounters, and levels.
   Level tools cover layout and scene composition, triggers,
   schedules, progression gates, minigame placement, and
   deterministic playtest generation from the authored graph.
6. **Content standards and accessibility** — first-class
   scaffolding and checks, never progression-gated: content tags
   and rating metadata (ESRB, PEGI, IARC), age-appropriateness
   review records, accessibility surfaces (photosafety — flash and
   transition-rate gates are hard limits, not options; colour-
   vision-safe palettes; remappable input; subtitle and text-
   scaling contracts), and plain-language summaries of what a game
   contains. Where Ardor scaffolds safety for adults, Meister
   scaffolds appropriateness for everyone, including classrooms.
7. **Asset routing and provenance** — intake, hash, provenance,
   and routing records for every media artifact a project uses.
   Specialist work stays with its owner (see the media map below):
   lumen for media conversion and the sprite-to-3D factory,
   eigenstyle for voice style, bloch for lessons, fathom for
   clean-room study of existing games. Meister calls or records
   handoffs; it re-implements nothing.
8. **Playtest and QA** — scripted playthrough definitions,
   deterministic replay and hash checks, screenshot specifications
   for verdict packs, and defect records tied to authored project
   state. Test design follows the estate split: half validation,
   half adversarial, by construction.
9. **Build and release handoff** — prepare channel artifacts and
   hand them to slipway for Play and F-Droid certification, and to
   the channel executors below. Meister never forks slipway's
   publish gates or definitions. This group carries a
   **channel-target registry for SFW distribution**: Google Play
   (the SFW lane's home channel; adult content is prohibited
   platform-wide, which is why this registry is Meister's and not
   Ardor's), F-Droid, itch.io (pipeline via butler; desktop, web,
   and Android artifacts), Steam, GOG, Epic Games Store, Humble,
   Apple App Store, Microsoft Store, console portals (Nintendo,
   PlayStation, Xbox — curated-pitch class), and direct web release
   (own site, GitHub releases). Each target records platforms,
   content-policy notes, automation class (pipeline /
   prepared-submission / curated-pitch), artifact requirements, and
   its handoff executor. Every channel is verified live at build
   time; a channel that fails verification is flagged with
   evidence, never given invented requirements. Whether the estate
   ships on a given channel for a given title remains the Owner's
   per-title decision; the registry records capability, not intent.
10. **Channel credentials and sessions** — auth, API key, token,
    and login management for every channel in the group-9
    registry, under Ardor group 10's hard rule, adopted verbatim:
    Meister manages credential **references and status, never
    values**. No tool stores a raw secret in project state, returns
    one in its output, or writes one to a log or report. A
    channel-account record holds the channel, the account identity,
    the credential kind, where the value lives (a primo `pass` path
    or a Secure Vault entry), scopes, expiry, and last-verified
    time. Tools register and inspect channel accounts, run
    per-channel verification probes where the channel supports one,
    track expiry and rotation, and run a publish **preflight** that
    refuses a handoff when the target channel's auth is missing or
    unverified. Channels with no API get honest login-reference
    records, never invented token flows.

## The media map — what lumen provides

Meister authors game data; lumen is the estate's deterministic
media-conversion engine, and every media artifact in a Meister
project is produced or converted through lumen's formats and
recorded in group 7. The applicable lumen media surface:

- **2D stills** — sprites, sprite sheets, tilesets, character
  portraits and whole-body expression portraits, backgrounds, key
  art, UI art. PNG canonical; Aseprite-native pipelines for pixel
  work; every still carries its keeper chain (governing reference,
  candidate history, gate reference) from birth.
- **Pixel art** — Aseprite `.aseprite` sources, deterministic
  batch exports, palette discipline, sprite-sheet packing.
- **Animation** — sprite animation strips and GIF assembly;
  idle/breathe cycles and companion idle strips; and the estate's
  shipped-animation form: code-as-animation. A frozen, hashed scene
  spec compiles through a timeline compiler to rig/curve data and
  renders deterministically — double-rendered byte-identical.
  Diffusion image-to-video is reference only and never ships.
- **3D** — lumen's parametric sprite-to-3D factory: a 2D sprite is
  identity-locked, projected onto the canonical rigged base
  (Mixamo-compatible skeleton, morph-target expression set), and
  exported glTF/GLB-first, with VRM and thin per-engine adapters
  for Blender, Unity, Unreal, and Godot. One canonical model per
  character, never N independently generated models.
- **Effects** — `lumen-fx/1` effect records (particles, flashes,
  ambient life: drifting motes, twinkle fields, light beams) with
  replay hashes and the photosafety flash gate applied at the
  format level.
- **Audio** — MIDI-first music generation and arrangement,
  rendered to WAV/OGG; sound-effect records and synthesis;
  per-character synthetic voices with locked, versioned reference
  timbres (a voice swap is a re-approval, never a silent change).
  Audio is produced and verified as files; nothing is auditioned
  by automation on the Owner's hardware.
- **Text and data** — the canon-record format, dialogue records,
  scene specs, and the deterministic converters between them
  (JSON/RON and engine-native forms). Text is a media format in a
  maker: it is versioned, hashed, gated, and emitted like any
  other asset. All shipped text is English-canonical.
- **Video** — deterministic renders of animated scenes to MP4/WebM
  for trailers, storefront pages, and in-game cutscenes, produced
  from the same frozen scene specs as the shipped animation, so a
  trailer frame and a game frame are the same frame.

Formats lumen does not own stay with their owners: haptics belong
to the NSFW lane's stack and do not exist in Meister; corpus
screening belongs to fathom and neemu's lanes respectively;
lessons belong to bloch.

## Genre pipelines

A genre pipeline pack is a named, versioned bundle: the authoring
verbs (groups 3–5 specialised), the media profile (which lumen
formats, at what budgets), the systems templates, the playtest
generators, and the gate set for that genre. A project picks its
pack at scaffolding (group 1) and can adopt a second pack when a
game genuinely spans genres. Packs are data and emitters, not code
forks: the spine and the tool groups are shared; only the pack
differs.

### Teaching RPG / dungeon crawler — the flagship pack

The estate's home genre (Niflheim, Versemend): floors or regions
as authored graphs; encounters, enemy genesis with canonical
variant tables, and deterministic battle simulation; dialogue that
teaches — lessons embedded as mechanics, with the curriculum as a
canon record beside the cast; quizzes and trials as minigame
definitions. Media profile: pixel-art sprites and tilesets, portrait
sets, MIDI-first soundtrack, effects records for combat feel.
Worked example of record: a Niflheim floor produced end to end
through the pack.

### Narrative / visual novel

Scene-graph-first authoring: branching dialogue with relationship
state and continuity effects (group 4 at full stretch), whole-body
expression portraits, layered backgrounds with ambient life,
voice-per-line routing with locked timbres, deterministic
playthrough generation that walks every branch. Media profile:
high-resolution stills, portrait sets, rendered video for openings
and endings, Ren'Py and Bevy emitter profiles. The teaching layer
applies here too — a visual novel whose subject is a curriculum is
a pack configuration, not a new pack.

### Puzzle

Level graphs with provable solvability: the pack requires a
deterministic solver or constructive proof per level family, and
playtest generation covers the solution space's edges, not only
its middle. Difficulty is an authored curve with measured rungs.
Media profile: minimal and exact — tilesets, kinetic UI (text as
animation), effects records, short musical motifs. light-show's
fiber-optic puzzles are the worked example of record.

### Platformer / action

Frame-data-first: sprite animation strips with measured timing,
tilesets and parallax layers, hit-stop, screenshake, and particle
records tuned as data; levels as composition plus trigger graphs;
playtest bots that traverse, not teleport. Media profile: pixel
art at production density, animation strips, SFX-forward audio,
MIDI soundtrack. The jam-game family is the proving ground: a pack
is only real when a game jam entry has shipped through it.

### Simulation / management

Schedules, economies, and NPC state machines as the primary
authored state; long-horizon deterministic simulation with seeded
replay, so a balance claim is a hash-checked run, not an anecdote.
Media profile: portrait and sprite sets, ambient-life effects,
generative-but-frozen music beds. Town, farm, shop, and school
framings are configurations of the same systems templates.

### Strategy / tactics

Grid and graph theatres, unit canon records, deterministic battle
simulation with replay hashes, AI opponents as authored policies
with bounded lookahead. Media profile: tilesets and unit sprites,
3D via the lumen factory for titles that want it, effects records
for combat readability. Clean-room studies of classic tactics and
dungeon design feed this pack through fathom, as spec and never as
extracted assets.

### Adventure / point-and-click

Scene composition, hotspots, inventory and state-flag logic,
dialogue trees with fail-forward design (a failed attempt costs
something authored, never a dead end the author didn't choose).
Media profile: painted backgrounds, character sprites or 3D,
voice, ambient effects. Deterministic walkthrough generation must
reach every ending from the authored graph alone.

### Roguelike / procedural

The genesis pack: seeded generation under canon constraints —
enemy genesis, variant sweeps, item and economy generators — where
the generator is itself a frozen, hashed record and every run is
replayable from its seed. Procedural never means unaccountable:
the census tools verify that generated families stay inside their
canonical tables. Media profile: tilesets and sprite families,
generative MIDI with frozen renders.

### Rhythm / music

Beat maps as authored data against MIDI-first compositions;
input windows, scoring curves, and difficulty as measured records;
photosafety gates at their strictest (flash and transition limits
are format-level). Media profile: music as the primary asset,
kinetic UI, effects records slaved to the beat grid. Deterministic
replay verifies a chart is passable at its stated difficulty.

### The teaching layer (cross-genre)

Not a genre: a layer any pack can carry. Curriculum as canon
records; lessons embedded as mechanics rather than interruptions;
assessment as authored encounters with measured outcomes; progress
state that a teacher or the learner can inspect. Niflheim teaches
Rust, Versemend teaches Lua — the layer is what
makes a Meister game a Qompass teaching game, whatever its genre.

## The path to 1.0

Rungs land in order; a rung is done when its gates pass on the
machine, with evidence, not when its code exists. Gates follow the
estate pattern: strict clippy with warnings denied, theodolite LSP
validation per file type with a planted-error control (a zero
without a control is unverified), conformance run records for every
pipeline stage, and the Owner's verdict wherever a gate is an eye
or an ear.

- **Rung 0 — Birth.** This plan; the repository; the rmcp skeleton
  (modern era only) answering discovery with the full annotated
  tool surface as typed seams; bloch birth records for every
  schema; the lane gate live from the first commit — Meister
  refuses NSFW and unclassified input before it can author
  anything at all.
- **Rung 1 — Scaffolding and canon.** Groups 1–2 real: project
  birth with genre pack selection, cast records with identity
  locks, canon checks. The lumen pipes' canon census graduates
  server-side here. Gate: a fresh project scaffolds, locks a cast,
  and passes census with zero unverified numbers.
- **Rung 2 — Systems authoring.** Group 3 real: scene graphs,
  schedules, flags, progression DAGs, economies, state machines,
  minigames — all as validated, bounded records with typed
  refusals for malformed input (malformed is the normal case, not
  the edge case; local models author these records).
- **Rung 3 — Narrative.** Groups 4–5 real: dialogue records with
  reaction profiles and continuity, story arcs and quests, level
  composition with triggers and gates, deterministic playtest
  generation from the authored graph. Gate: a branching narrative
  slice whose every ending is reached by generated playthroughs.
- **Rung 4 — First genre packs.** Teaching RPG, narrative/visual
  novel, and puzzle packs at v1, each proven by its worked example
  of record produced end to end through Meister. Gate: the worked
  examples' conformance records verify, and their verdict packs
  carry the Owner's rulings.
- **Rung 5 — Asset routing and media completeness.** Group 7 real;
  the remaining lumen pipe graduations (genesis ledger,
  keeper-chain registry) server-side; every format in the media
  map routable with provenance. Gate: an asset with an incomplete
  keeper chain is refused as a keeper, by the server, in a test.
- **Rung 6 — Standards, accessibility, QA.** Groups 6 and 8 real:
  rating metadata emitted per channel, photosafety and
  accessibility checks as gates, scripted playtests with replay
  hashes, defect records tied to project state. Gate: a project
  that fails photosafety cannot reach release handoff, proven by
  adversarial fixture.
- **Rung 7 — Remaining genre packs and the TUI.** Platformer,
  simulation, tactics, adventure, roguelike, and rhythm packs at
  v1, each with a worked example; the teaching layer as a formal
  cross-pack feature; the TUI at fleet-standard capability.
- **Rung 8 — Release.** Groups 9–10 real: the SFW channel registry
  verified live entry by entry, credential references with
  preflight, slipway handoff for Play and F-Droid. Gate: a worked
  example completes a full preflight against at least one pipeline
  channel and one prepared-submission channel, with secrets never
  present in any Meister state or output.
- **Rung 9 — 1.0.** The definition below, verified end to end,
  then publication: crates.io, the MCP Registry under the Qompass
  namespace, and the publish-flow rule — the published artifact is
  reinstalled and the landed binary's own version report probed.
  A publish is not complete until the installed thing answers as
  the new version.

### What 1.0 means

Meister 1.0 is true when a new SFW game, in any genre with a v1
pack, can be taken from scaffold to release handoff entirely
through Meister and its routed specialists — canon locked,
content authored as data, media produced through lumen with
provenance, playtests deterministic, channels preflighted — and
the worked examples prove it, one per pack, with conformance
records the Owner has gated. Anything less is a version on the
way, honestly numbered: the fleet ships 0.x without embarrassment
and 1.0 only when it is earned.

## Boundaries

- phlow remains the agent runtime and workflow orchestrator.
- lumen remains the media engine and the home of SFW pipe tooling
  that has not graduated; Meister consumes and graduates, it does
  not fork.
- slipway remains the Android and publish-certification surface.
- fathom remains the clean-room reverse-engineering surface;
  studied games enter Meister as spec, never as assets.
- bloch remains the lessons surface; eigenstyle the voice-style
  surface; tackle the editor broker.
- Ardor remains the NSFW lane's maker. The lanes share a spine
  shape and this README's structure — mechanics only. Content,
  records, and canon never cross, in either direction, ever.
- Qompass lane only. Registry publication follows the fleet's
  standardised route under a Qompass namespace when the build is
  conformance-green.

## Name

Ruled by the Owner (D-1603, 2026-11-16): **Meister** — the master
of a craft guild, the rank a journeyman reaches by submitting a
masterwork to the masters' judgment. The estate's SFW games are
its masterworks in progress; this server is where they are made.
The crates.io name `meister` and the GitHub name `qompassai/meister`
were both verified unclaimed on 2026-11-16.

Earlier candidates, retained as history: **fable** and **fabulist**
(the story-form), **mythopoeia** (Tolkien's word for myth-making),
**gamewright**, **clockwright**, and **gamesmith** (the maker
compounds), **ilmarinen** (the Kalevala's eternal smith), **regin**,
**eitri**, **weland**, and **brigid** (the smiths of myth),
**wootz**, **masterwork**, and **siderurgy** (the material, the
credential, and the craft), **magister** and **mastro** (Meister's
Latin ancestor; Geppetto's title). All verified unclaimed on
crates.io at the 2026-11-16 check, with one exception: the bare
name **fable** is taken — its fleet-pattern form `fable-mcp` is
the unclaimed one.
