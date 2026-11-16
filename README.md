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

## Genre categories

Every genre is its own category. A category pack is a named,
versioned bundle: the authoring verbs (groups 3–5 specialised),
the media profile (which lumen formats, at what budgets), the
systems templates, the playtest generators, and the gate set for
that genre. A project picks its pack at scaffolding (group 1) and
can adopt a second pack when a game genuinely spans genres — a
teaching RPG is Role Playing carrying the Educational category's
teaching layer, not a merged pack. Packs are data and emitters,
not code forks: the spine and the tool groups are shared; only
the pack differs.

Each category below names its **studied exemplars**: top games
researched across the eight tools this estate works with —
Unreal Engine, Unity, Aseprite, Bevy, Blender, RPG Maker, Godot,
and Ren'Py (researched 2026-11-16). Engine and art-tool layers
are recorded separately, and a tool is claimed only where a
source supports it. The gaps are findings, not omissions, and
they are stated where they fall.

### Action

Frame-data-first: combat and movement verbs as reusable kits
(parry, posture, finishers, style-ranked feedback), sprite or rig
animation with measured timing, hit-stop, screenshake, and
particle records tuned as data; world structure as ability-gated
graphs rather than hand-scripted triggers; behaviour authored as
state machines so artist-led teams can build enemies without
gameplay programmers; playtest bots that traverse, not teleport.
Media profile: pixel art or 3D at production density, animation
strips, SFX-forward audio, MIDI soundtrack. The jam-game family
is the proving ground: a pack is only real when a jam entry has
shipped through it.

Studied exemplars:

- **Sifu** (2022) — Unreal Engine 4: combat-verb kits (parry,
  posture meter, finisher states) belong in the pack as
  primitives, not per-game code.
- **Hi-Fi Rush** (2023) — Unreal Engine 4: a global beat clock
  that combat, animation, VFX, and SFX all stamp onto — the
  Action × Rhythm composition, proven at AAA scale.
- **Prince of Persia: The Lost Crown** (2024) — Unity: the
  ability-locked world graph as first-class level data.
- **Hollow Knight** (2017) — Unity: state-machine behaviour
  authoring (PlayMaker) let an artist-led team build every enemy
  and interactive.
- **Brotato** (2023) — Godot: wave, spawn, and economy loops as
  tunable data tables; a solo developer shipped ~10M copies.
- **Cuphead** (2017) — Unity; art is hand-drawn pencil, ink, and
  watercolour, outside the eight: the art pipeline must ingest
  frame-by-frame scanned art without imposing a house style.

### Adventure

Simulation-first world state before dialogue trees: a clockwork
world the player investigates beats scenery with conversation
bolted on. Scene composition, hotspots, inventory and state-flag
logic, dialogue trees with fail-forward design (a failed attempt
costs something authored, never a dead end the author didn't
choose). Media profile: painted or modelled backgrounds,
character sprites or 3D, voice, ambient effects. Deterministic
walkthrough generation must reach every ending from the authored
graph alone.

Studied exemplars:

- **Outer Wilds** (2019) — Unity: adventure support means a
  simulated world (a clockwork solar system) as the primary
  authored state, not dialogue trees with scenery.
- **Stray** (2022) — Unreal Engine 4: environmental storytelling
  density — the pack optimises for traversal and vista
  composition, not combat systems.
- **A Short Hike** (2019) — Unity + **Blender** (low-poly models,
  colour-atlas UV painting): the solo-dev stack is engine +
  Blender + a dedicated dialogue tool; route dialogue to a
  specialist format instead of reinventing it.
- **To the Moon** (2011) — RPG Maker XP (Unity for later ports):
  genre ≠ engine — an RPG maker's event system is also an
  adventure engine; packs are templates over shared systems.
- **Harold Halibut** (2024) — Unity; handmade physical models
  digitised by photogrammetry: a maker needs an asset-intake
  path for non-digital sources, with provenance per asset.

### Educational

The teaching category, standalone. Curriculum as canon records;
lessons embedded as mechanics rather than interruptions;
assessment as authored encounters with measured outcomes;
progress state that a teacher or the learner can inspect. The
researched exemplars separate into four embedding patterns, and
the pack names all four as configurations: **lesson-as-mechanic**
(the simulation or construction is the subject), **lesson-as-gate**
(pass the embedded course to unlock the role or content),
**assessment-as-encounter** (checks carried by the genre's own
verbs — battles, puzzles), and the **narrative quiz frame** (a
question bank with explanatory feedback inside a story). The top
tier of the genre is lesson-as-mechanic; the quiz frame is the
honest fallback, not the default. Documentary and primary-source
media are a content type of their own, with an evidence-asset
workflow. Educational composes with every other category through
the teaching layer (below): Niflheim teaches Rust, Versemend
teaches Lua.

Studied exemplars:

- **America's Army** (2002) — Unreal Engine 2/3: lesson-as-gate
  — players pass virtual qualification courses (including combat
  first aid) to unlock roles and missions; the curriculum is the
  progression system.
- **Kerbal Space Program** (2015) — Unity: pure
  lesson-as-mechanic — a faithful orbital-mechanics simulation
  with legible failure needs no quiz layer at all.
- **Attentat 1942** (2017) — Unity: lesson-as-investigation —
  history assembled from filmed interviews and archival footage;
  knowledge is gathered as evidence, not lectured.
- **while True: learn()** (2019) — Unity: machine learning as a
  node-graph puzzle game; the concept roadmap maps onto a
  mechanic tech-tree.
- **Turing Complete** (2021) — Godot (the 2.0 rebuild moved to a
  custom engine): the purest lesson-as-mechanic — build a CPU
  from NAND gates; solved lessons compose as components in later
  ones, and lesson logic stays data-driven.
- **Learn Japanese To Survive! Hiragana Battle** (2016) — RPG
  Maker MV: assessment-as-combat — kana sets as enemy/lesson
  tables; a stock engine plus curriculum-as-data ships a course.
- **Learn to Code RPG** (2021) — Ren'Py: the narrative quiz frame
  done honestly — 600+ questions with explanatory feedback as
  data inside a visual-novel story.

### Fighting

Deterministic, fixed-timestep combat before everything: frame
data as records, hit/hurt interactions modelled on any entity
(projectile, wall, player), not hard-coded punch-versus-body.
Rollback scaffolding with state partitioning; training mode,
frame-data display, and simplified-input schemes as first-class
primitives (accessibility is a systems feature, not a difficulty
toggle); style as a transferable pipeline — toon shading and
limited-animation presets that travel across projects. Media
profile: rigged 3D or sprite fighters, effects records for
impact readability, music that carries round structure.

Studied exemplars:

- **Tekken 8** (2024) — Unreal Engine 5: fidelity is worthless
  if it adds input delay — determinism and a full asset-rebuild
  workflow outrank rendering features.
- **Guilty Gear -Strive-** (2021) — Unreal Engine 4: per-part
  forced shading with live material preview, and rollback
  optimised by manually partitioning what does and doesn't need
  rollback state.
- **Dragon Ball FighterZ** (2018) — Unreal Engine 4: a reusable
  cel-shading + limited-animation (pose-on-twos/threes) preset
  carried one house style across a licensed IP.
- **Fantasy Strike** (2019) — Unity: designed by a former Street
  Fighter lead to remove execution barriers while keeping depth
  — training, frame data, and simplified inputs are the product.
- **Your Only Move Is HUSTLE** (2023) — Godot: a turn-planning
  fighter whose product is deterministic, replayable simulation
  plus a mod/character loader (5,000+ community mods).

No sourced Aseprite or Blender pipeline exists in any top
fighter, and Bevy, RPG Maker, and Ren'Py have no versus fighter
of note — the genre clusters on Unreal at AAA and Unity/Godot at
the indie end.

### Narrative / visual novel

Scene-graph-first authoring: branching dialogue with relationship
state and continuity effects (group 4 at full stretch), whole-body
expression portraits, layered backgrounds with ambient life,
voice-per-line routing with locked timbres, deterministic
playthrough generation that walks every branch. The production
problem is branch volume: the pack must visualise and audit the
branch graph, and episodic release (chapter gating, save
continuity) is a first-class workflow. Media profile:
high-resolution stills, portrait sets, rendered video for openings
and endings. Authoring profile of record is Ren'Py; because
Ren'Py has no console support, the pack also carries the Unity
port path the canon itself used.

Studied exemplars:

- **Doki Doki Literature Club!** (2017) — Ren'Py (the *Plus!*
  rebuild, 2021, is Unity): Ren'Py's Python layer enables
  meta/file-system storytelling — and its console edition is the
  port lesson above, learned by the canon itself.
- **Slay the Princess** (2023) — Ren'Py; hand-pencilled art by
  Abby Howard: thousands of hand-drawn frames behind a choice
  graph — branch-graph tooling is the pack's centre of gravity.
- **Katawa Shoujo** (2012) — Ren'Py: a volunteer international
  team shipped a landmark novel; accessibility is both its
  subject and its craft.
- Cross-reference: **Long Live the Queen** (Ren'Py) is studied
  under Simulation — the same engine carrying a systems game.

### Platformer

The movement kit is the product: run, jump, wall-kick, slide,
and air control exposed as tuned feel parameters, not buried
constants. Room- and tile-based level design, parallax layer
stacks, checkpoint/save systems as built-in modules, and assist
modes as standard parts (accessibility is authored, not patched
in). Co-op scaffolding (split-screen/netplay) where the design
wants it. Media profile: pixel art or 3D, animation strips with
measured timing, tilesets and parallax layers, SFX-forward audio.

Studied exemplars:

- **Celeste** (2018) — engine outside the eight (XNA/FNA);
  **Aseprite** on secondary sourcing (sprite artist Pedro
  Medeiros's workflow): per-screen room design, pixel-art import,
  and the landmark Assist Mode.
- **Ori and the Will of the Wisps** (2020) — Unity: the parallax
  layer stack and checkpoint system as platformer modules.
- **It Takes Two** (2021) — Unreal Engine 4: a new mechanic
  nearly every level — fast throwaway-mechanic prototyping and
  co-op scaffolding beat feature breadth.
- **Psychonauts 2** (2021) — Unreal Engine 4: Double Fine chose
  Unreal over a custom engine to save time and money — maker
  tools win on iteration speed.
- **Pseudoregalia** (2023) — Unreal Engine 5: a solo-dev breakout
  built around an expressive movement kit — expose feel
  parameters directly.

### Puzzle

Level graphs with provable solvability: the pack requires a
deterministic solver or constructive proof per level family, and
playtest generation covers the solution space's edges, not only
its middle. Difficulty is an authored curve with measured rungs.
A puzzle is modelled as data — a scene plus a validation model —
so deduction and placement puzzles verify mechanically. Media
profile: minimal and exact — tilesets, kinetic UI (text as
animation), effects records, short musical motifs. light-show's
fiber-optic puzzles are the worked example of record.

Studied exemplars:

- **Monument Valley** (2014) — Unity: one-week playable
  prototypes of a spatial mechanic, with a WYSIWYG editor and a
  fast build-test loop, are how the concept survived.
- **Tetris Effect** (2018) — Unreal Engine 4/5: a simple
  deterministic core can carry heavy audiovisual layering when
  materials/particles and VR + flatscreen preview sit side by
  side.
- **The Case of the Golden Idol** (2022) — Godot + **Aseprite**
  (the 2024 *Redux* ported to Unity): the puzzle as a data-driven
  scene with slot/word-bank validation; logic and art split
  cleanly between two brothers.
- **Unpacking** (2021) — Unity + **Aseprite**: greybox in the art
  tool itself — placement grids drawn in Aseprite, rebuilt in the
  engine, nodes placed over them; a maker should round-trip that
  grid.
- **OneShot** (2016) — RPG Maker XP: metafictional puzzles that
  use the player's OS as a mechanic — an event/script system
  needs an escape hatch to OS-level actions.
- **Return of the Obra Dinn** (2018) — Unity; modelled in Maya
  LT, explicitly **not** Blender (the developer considered
  Blender and chose otherwise): a custom post-process path (1-bit
  dithering) and a logbook that is a validated data model,
  confirmed in batches of three fates.


### Rhythm

Beat maps as authored data against MIDI-first compositions;
input windows, scoring curves, and difficulty as measured records;
photosafety gates at their strictest (flash and transition limits
are format-level). A chart format plus calibration/latency offset
are first-class data, and note timing is a function of the audio
clock, never frame count — with a low-level audio-clock escape
hatch exposed, because stock engine audio has proven insufficient
for serious rhythm work. Media profile: music as the primary
asset, kinetic UI, effects records slaved to the beat grid.
Deterministic replay verifies a chart is passable at its stated
difficulty.

Studied exemplars:

- **Beat Saber** (2019) — Unity: VR's best-selling title runs on
  a data-driven chart pipeline plus per-player latency
  calibration.
- **Muse Dash** (2018) — Unity: a two-button, two-lane core
  sustained by song-pack throughput — content pipeline, not more
  mechanics.
- **Rhythm Doctor** (2025) — Unity + **Aseprite** (the team's own
  tool list): one button, deep charts, and a JSON level editor
  (`.rdlevel`) that the Workshop community authors against.
- **Hi-Fi Rush** (2023) — Unreal Engine 4: a global beat clock
  that any animation can be retimed to land on, regardless of
  BPM; cutscenes animated to a click track.
- **Project Heartbeat** (2020) — Godot: the developers bypassed
  stock audio with a custom C++/miniaudio engine and sub-frame
  input — the escape-hatch requirement above is this game's
  lesson.

No shipped rhythm game exists on Bevy or Ren'Py (Ren'Py has an
open-source rhythm minigame engine — minigame support is the
opportunity), and RPG Maker's rhythm presence is plugins only.

### Roguelike

The genesis pack: seeded generation under canon constraints —
enemy genesis, variant sweeps, item and economy generators — where
the generator is itself a frozen, hashed record and every run is
replayable from its seed. Generation assembles authored pieces
(rooms, biome chunks, encounters) rather than generating from
nothing; items and powers are composable, stackable effect
records, so content volume comes from combination, not asset
volume. Procedural never means unaccountable: the census tools
verify that generated families stay inside their canonical tables.
Media profile: tilesets and sprite families, generative MIDI with
frozen renders.

Studied exemplars:

- **Enter the Gungeon** (2016) — Unity: procedural generation as
  a layout/connection system over a library of hand-designed,
  playtested rooms.
- **Risk of Rain 2** (2020) — Unity: items as composable,
  stackable effect records — synergy emerges from the data model
  and its proc-order rules.
- **Caves of Qud** (2024) — Unity hosting a custom C# simulation
  core: entities as bags of composable properties, not class
  hierarchies — the model its team credits for its depth.
- **Returnal** (2021) — Unreal Engine 4: the die → reshuffle →
  meta-progress loop at AAA scale, with biome chunks, boss gates,
  and permanent unlocks as data.
- **Path of Achra** (2024) — Godot: a solo-dev traditional
  roguelike whose depth is build-crafting — every power
  data-defined and freely combining.

### Role Playing

The estate's home genre, and the flagship pack. Floors or regions
as authored graphs; encounters, enemy genesis with canonical
variant tables, and deterministic battle simulation; dialogue
systems with relationship state; long-lived project support with
version migration, because auteur RPGs run for a decade. Templates
accelerate, they don't finish: the pack ships strong stock rigs,
recipes, and systems expecting full customisation on top, and it
is scoped for tiny teams, not only studios. Media profile:
pixel-art sprites and tilesets or 3D via the lumen factory,
portrait sets, MIDI-first soundtrack, effects records for combat
feel. Worked example of record: a Niflheim floor produced end to
end through the pack (Role Playing × Educational).

Studied exemplars:

- **Clair Obscur: Expedition 33** (2025) — Unreal Engine 5: a
  small team's breakout hit — faces start from a stock rig
  (MetaHuman) and are then 100% customised; the template is the
  starting point, never the finish.
- **Octopath Traveler** (2018) — Unreal Engine 4: HD-2D is a
  named, repeatable art-direction recipe (sprite scale + diorama
  depth + lighting), not just an engine choice.
- **Sea of Stars** (2023) — Unity (its pixel-art tool is publicly
  unsourced): even famous pixel RPGs often have no public
  art-pipeline record — Meister records art-tool provenance at
  intake instead of inheriting the gap.
- **Cassette Beasts** (2023) — Godot: a two-person team shipped a
  full monster-taming RPG — scope the pack for tiny teams.
- **OMORI** (2020) — RPG Maker MV: over a million copies on an
  event/database model that carried a decade-long auteur project.
- **Eastward** (2021) — custom engine; **Aseprite** for the pixel
  art: engine and art tool are independent choices — the art
  pipeline outlived and out-travelled the bespoke engine.

### Shooter

Ballistics and hit models as data; scoring and feedback systems
(style meters, rank announcements, hit confirmation) as reusable
components; procedural and destructible terrain as a pack module
where the design wants it; co-op scaffolding. Performance-budget
profiles — explicit minimum-spec targets — are part of the pack
from day one, and gameplay data must survive engine-version
migration under a live game. Media profile: 3D via the lumen
factory or sprite-based for retro forms, effects records for
tracer/impact readability (photosafety gates apply), SFX-forward
audio.

Studied exemplars:

- **Fortnite** (2017) — Unreal Engine 4 → 5 under a live game:
  design for engine upgrades; gameplay data survives migration.
- **Valorant** (2020) — Unreal Engine 4 → 5.3: engineered for
  high frame rates on minimum-spec hardware — performance budgets
  are a design input, not a launch-week surprise.
- **Deep Rock Galactic** (2020) — Unreal Engine 4, gameplay in
  Blueprints: visual scripting plus fully destructible procedural
  caves as the co-op shooter's core module.
- **Remnant 2** (2023) — Unreal Engine 5: procedural biome/world
  assembly that is seeded and reproducible, not purely random.
- **ULTRAKILL** (2020) — Unity: a solo-dev retro FPS whose
  style-rank combat system is exactly the kind of scoring/feedback
  component a pack should ship.
- **Cruelty Squad** (2021) — Godot: an uncompromising solo-dev
  vision on an open engine — the maker must not enforce visual or
  structural norms; weird pipelines are a feature.

No notable shooter exists on Bevy, RPG Maker, or Ren'Py.

### Simulation

Schedules, economies, and NPC state machines as the primary
authored state; long-horizon deterministic simulation with seeded
replay, so a balance claim is a hash-checked run, not an anecdote.
The director/storyteller is a data product: weighted event tables
plus agent traits and needs as composable records. Agent budgets
and simulation tick rates are explicit dials — simulation scales
by simplifying agents, not by cutting count — and production
chains are directed graphs (recipes, rates, power) that the pack
emits and validates, not just buildings it places. Media profile:
portrait and sprite sets, ambient-life effects, generative-but-
frozen music beds. Town, farm, shop, and school framings are
configurations of the same systems templates.

Studied exemplars:

- **RimWorld** (2018) — Unity: the AI storyteller is weighted
  event tables + pawn records; design the save/mod format before
  the graphics.
- **Cities: Skylines** (2015) — Unity: a 13-person studio
  simulated a million citizens' routines by simplifying agents,
  not cutting their count.
- **Satisfactory** (2024) — Unreal Engine 4 → 5: the leading
  factory sim; its production chains are a solvable data model.
  (A Blender art credit is UNVERIFIED — a job posting's
  proficiency requirement is not pipeline evidence.)
- **Thrive** (2021) — Godot (C#): an open-source evolution sim
  sustained by a volunteer-scale team through stage-based design,
  JSON-defined content, and editor-based modding.
- **Long Live the Queen** (2012) — Ren'Py: a raising/political
  simulation whose engine is a schedule + stat-check + event-flag
  script — Ren'Py's label/jump/flag grammar *is* the simulation.

Kerbal Space Program (Unity) is studied under Educational, where
its simulation is the teacher.

### Strategy

Grid and graph theatres, unit canon records, deterministic battle
simulation with replay hashes, AI opponents as authored policies
with bounded lookahead. Tactics are data: unit stats, abilities,
cover and hit-chance rules in editable tables, with a mod/emit
path from day one. Swarm systems — hundreds of simple units with
task assignment — are a first-class system, and faction identity
is composable trait sets, not fixed factions. Media profile:
tilesets and unit sprites, 3D via the lumen factory for titles
that want it, effects records for combat readability. Clean-room
studies of classic tactics and dungeon design feed this pack
through fathom, as spec and never as extracted assets.

Studied exemplars:

- **XCOM 2** (2016) — Unreal Engine 3.5: the defining modern
  tactics game shipped an official SDK and became a modding
  landmark — tactics as editable data is the lesson.
- **Triangle Strategy** (2022) — Unreal Engine 4: the HD-2D
  preset — 2D sprite actors in a lit 3D scene — as a repeatable,
  freezable style recipe (the same recipe as Octopath, under
  Role Playing).
- **Pikmin 4** (2023) — Unreal Engine 4: Nintendo's first internal
  Unreal title is a swarm game — hundreds of simple units with
  task assignment beat a few smart units.
- **Humankind** (2021) — Unity: 4X as territory + district data
  on a hex grid, with civ identity as composable per-era traits.
- **Songs of Conquest** (2024) — Unity + **Aseprite** ("all of our
  pixel art is created with Aseprite," per its art director;
  tiles excepted, in PyxelEdit): `.aseprite` sources as first-class
  pipeline assets — sprite intake, billboard-into-3D emission, a
  lighting pass over pixel art.
- **Symphony of War: The Nephilim Saga** (2022) — RPG Maker,
  heavily customised: a breakout SRPG from a tool that defaults
  to another genre — expose the data model, not just the template.

No top-tier strategy exemplar was found on Godot or Bevy, and
Blender has no evidenced production use in this set.

### What the research says (cross-category)

Across sixty-plus studied titles, the engine map is consistent:
Unreal owns the AAA end of Action, Fighting, and Shooter and
carries the HD-2D recipe; Unity is the broadest base everywhere;
Godot's proven beachhead is the solo and small-team hit (Brotato,
Cruelty Squad, Cassette Beasts, Path of Achra); RPG Maker and
Ren'Py each escape their home genre at least once (Symphony of
War, To the Moon, OneShot; Long Live the Queen). **Bevy has no
commercial canon in any category** — Meister's Bevy emitters are
greenfield by necessity, and the estate's own Bevy titles are
their worked examples of record.

The art-tool layer is under-documented industry-wide. Aseprite is
evidenced on Songs of Conquest, The Case of the Golden Idol,
Unpacking, Rhythm Doctor, and Eastward (Celeste on secondary
sourcing); Blender is evidenced on A Short Hike, and otherwise
only on a mod pipeline (Kerbal Space Program's UGC). The famous
claim that Dead Cells used Blender is a misattribution — its
artist's own deep dive names 3DS Max — which is why Meister
records art-tool provenance per asset at intake and marks
unverifiable attributions instead of repeating them.

### The teaching layer (cross-genre)

Not a genre: the Educational category's exportable layer, which
any pack can carry. Curriculum as canon records; lessons embedded
as mechanics rather than interruptions; assessment as authored
encounters with measured outcomes; progress state that a teacher
or the learner can inspect. Niflheim teaches Rust, Versemend
teaches Lua — the layer is what makes a Meister game a Qompass
teaching game, whatever its genre.

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
- **Rung 4 — First genre packs.** Role playing (the flagship
  pack, carrying the teaching layer — Niflheim its worked
  example), narrative/visual novel, and puzzle packs at v1, each
  proven by its worked example
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
- **Rung 7 — Remaining genre packs and the TUI.** Action,
  adventure, educational, fighting, platformer, rhythm,
  roguelike, shooter, simulation, and strategy packs at v1 —
  completing the thirteen categories — each with a worked
  example; the teaching layer as a formal cross-pack feature; the
  TUI at fleet-standard capability.
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
the worked examples prove it, one per category pack, with conformance
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
