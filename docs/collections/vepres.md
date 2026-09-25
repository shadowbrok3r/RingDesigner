# Vepres — *the thorned botanical collection*

**Sheet subtitle:** FOUR SIGNETS · FOUR VINES · FIFTY-TWO STONES
**Sheet footnote:** "Poured in sand where the thorn lies in the parting plane; lost to wax where it does not."
**House line:** Kings of Alchemy. Final renders in studio 18k yellow gold, stones set, recesses darkened.

**Primary side of the app: paths.** The collection shows the following:
- `cad::twist` sweeps: thorns, shoots, sepals.
- 3-D `Operation::Sweep`: canes, ivy stems.
- `Operation::Loft`: leaflets.
- `PatternKind::{Ring, About, Mirror}` arrays.
- `Component::blend_mm` seam beads.
- Leaves struck as `setting::Stamp` with true outlines.
- Stones in made settings: `head.claw`, `head.bezel`, `seat.bur`, `SolidKind`.

The height field carries only what the doctrine allows: side-face runners, bark on walls, and bench-only fine lines.

Every ring has one plant from face to palm. Vepres is the fourth collection in the plan's order (Bestiarium, Cataphracta, Tenebrae, Vepres). It is the last because it carries the most new geometry.

---

## 0. Read this first

**Sources.** This file was written from these sources:
- the director's plan, `docs/collections/source/plan.md`. Its section 1 verdicts, its base-allocation table and its section 7 "Logan's answers" override the brief;
- the botanical brief, `docs/collections/source/brief-botanical.md`;
- the code at master `30f0506`, 2026-09-24.

Paths used below:
- `core/` = `crates/ringdesign-core/src/`
- `ex/` = `crates/ringdesign-core/examples/`
- `graph/` = `crates/ringdesign-graph/src/`
- `wb/` = `crates/ringdesign-workbench/src/`

A `file:line` reference means the fact was checked in source on that date.

**Not in this file.** The Bestiarium's Arachne and Draco are being authored now, on branches `bestiarium-arachne` and `bestiarium-draco`. Arachne sets the working per-ring precedent that Vepres copies: `ex/bestiarium_arachne.rs`, output in `showcase/bestiarium/arachne/`.

### Logan's decisions (binding)

- **Collection size.** Four collections, eight rings each.
- **Process.** Process is mixed across the collection. Each ring is judged against its own `DraftSettings::process`.
- **Upright plans.** The nine upright factory plans are lost wax only: 004, 008, 009, 010, 011, 014, 018, 019, 020.
- **Sand master.** The sand master mirrors the upper half of a stock. It is used only on 001, 002, 003, 005, 006, 007, 012, 013, 015, 016 and 017.
- **Sands.** He pours Delft and Petrobond. Vepres sand rings use **Delft**, which is 3.0° draft, 0.8 mm section and 0.30 mm detail (`core/castability.rs:180`).
- **Taste.**
  - One theme from face to palm. Palisade was retired for "random elements placed in various spots".
  - Figurative motifs are stamps with true outlines. Painted moons "look like arrows".
  - Made settings only. Height-field prongs were "quite awful".
  - No faces and no eyes.
  - Factory stock for signets.
  - Reptiles landed; the celestial Zenith did not.
  - The target: "the most complex, well thought out, most insane rings we've done".

### Platform status (dependencies)

| ID | What | Status |
|---|---|---|
| P1 | Async template open with staged progress; expression engine attached (`wb/templates.rs:114`, `Evaluator::with_exprs`) | **Landed** (batch 14, hardened in batch 15) |
| P2 | CAD stones are stones. Pieces: `StoneSource::Cad{feature, copy}` (`core/setstone.rs:29`); multi-source `Pattern{sources}`; `Gem::preview_tint` (`core/gem.rs:234`); `render::finished` (`core/render.rs:97`) | Batch 15, **merged to master** (merge `d3dfa34`) |
| P3 | Skin in core: `core/skin.rs` (`Atlas`, `Hide`, `Joints`, `draft_clamp`, `hide_layer`) and `imported_base::sand_master` | Batch 15, **merged** (`537451a`) |
| P4 | Stamps v2. `Stamp::tier` and `StampTop` (`core/setting.rs:927-948`); `core/outline.rs`; `Stamp::parting_monotone` (`:2149`); `hull_and_bays` (`:1490`); `stamp_row` with `RowPath` (`:1547-1581`) | Batch 15, **merged** (`5fd9390`) |
| P5 | Station-aware side-face gates: `FieldContext::side_faces_at(θ)`, `VGate::mask(θ, v, ctx)`, `VGate::Draft{min_deg, fade_deg}` | Batch 16, not started |
| P6 | Claw styles: `head.claw`/`head.basket` gain `style: Wire\|Talon\|Fang\|Tentacle\|Thorn\|Sepal`, `grouping`, `tip` | Batch 16, not started. Today the claw schema has only `prongs` and `wire_mm` (`core/cad/builders.rs:157`). |
| P7 | Template-weight nodes and lift: `base.preset{id, sand, face_length, face_width, bore}`; stamp nodes `stamp`, `stamp.outline.*`, `stamp.row`, `design.stamps`; `shank.key`; `cad.feature` numeric and placement pins | Batch 16, not started. Today stamps lift as one `design.set` patch and `cad.feature` has only `design`, `operation` and `enabled` pins (`graph/nodes/cad.rs:63`). |
| P8 | Collection tooling: `tools/render_collection.py`, `tools/catalog_collection.py`, `graph/../examples/collection_templates.rs`, stone material manifest, `thumbnails!` macro | Batch 16, not started |
| Starter gallery | 20 stocks and 8 settings | Batch 16, not started |

Enablers borrowed from sibling collections all land before Vepres in the plan's order:
- **C-B1:** `CurveLayer` per-point widths and `phase`. Kraken's lane.
- **C-B2:** true pear plans. Bestiarium batch 3B.
- **C-R1:** live sand clamp on a group. Cataphracta.
- **C-R4:** hide-space tilings. Cataphracta.
- **C-V2:** `PatternKind::Along`. It lands in Tenebrae's batch with Porta.

Check each one on master before relying on it. Every ring below names a fallback for each.

### What the batch-15 spikes measured (commit `9ccac2b`)

These four examples print the numbers the collection is gated on. The commit message is the record; the raw output was not saved. Re-run any of them with `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example <name> -- /tmp/out`.

| Spike | Result | Consequence for Vepres |
|---|---|---|
| `ex/prickle_probe.rs` | "The in-plane prickle pulls clean on the crest and on 017; one obstruction from a 2 degree tilt of its plane." It tests Rubus's own LowDome 7.0 × 3.4 and the 14-key knuckled body. | **Rubus's sand claim stands.** `cant_deg` must be exactly 0, `across_mm` 0, and `spin_deg` 0 or 180. `tilt_deg` (a lean along the ring) keeps the hook in the plane. The clean result on the 017 master suggests the raw surface normal is already level on a mirrored master, so C-V1 `level` may be a guarantee rather than a fix (Prunus). |
| `ex/parting_stamp_probe.rs` | Tested on the **006 Square master**, which is Ilex's base. "A spine past 23.6° locks, a tip 0.05 mm off the parting line locks, the filled outline pulls." 23.6° is `asin(0.4/1.0)`, for a spine 0.8 wide and 1.0 tall. | **Ilex's face leaf stands.** Spine lean must stay under `asin(half-base/height)`. Place the stamp at `Hide::crest_at`'s interpolated `v`, never the nearest row. The fallback for a wild margin is the filled outline, with the bays cut at the bench. |
| `ex/side_gate_probe.rs` | "Reference-only side-face gates spill 0.1–0.8 mm onto the crown fillet and fail once full strength passes the station's face." Rubus's base is one of the two bodies it tests. | Rubus's keyframes need P5, or keys that never shrink the station's side-face share below the reference's (see Rubus). |
| `ex/stock_spike.rs` | "003 and 007 need 4.2 mm of sand envelope and are refused; 005 and 016 are rewritten by 0.8 and 0.44 mm; 013 to 16 mm needs a baked 130% step." | **Viscum's base, 003 Clover, is refused in sand.** The plan moved Viscum to 003 in sand before this was measured. See the decision below. |

### One decision Logan must make: Viscum's process

003 is a sculpted four-leaf clover whose lobes overhang the shank. The render is `scratchpad/b15/skin/renders/003-clover-master-hero.png` from the batch-15 session. The sand master would need 4.2 mm of fill, so it is refused. There are two routes:

1. **Recommended: lost wax on native 003.** It keeps the factory stock and its lobes, which are the point of the ring and of Logan's taste, and it needs no new enabler. Vepres then pours 3 in sand and 5 in wax. The plan's section heading says "5 sand, 3 wax", but its own per-ring verdicts give 4 and 4; with this change the count is 3 and 5.
2. **Sand on a procedural clover.** The bundled `CG Clover` outline (`bundled/outlines/CG Clover.outline.json`) goes on a lofted signet. `ShankStyle::suggest_dome` sends it to the cut dome, which fields 0.000% by construction. The cost is that the factory recipe replaces the factory stock.

This file writes Viscum for route 1 and lists the deltas for route 2.

---

## 1. The rings

| # | Ring | Epithet | Base | Process | Stones | Status |
|---|---|---|---|---|---|---|
| 1 | Rubus | *the bramble cane* | Procedural LowDome 7.0 × 3.4, `flatten_sides`, 14 keyframes | Sand (Delft) | 0 | Not started; **buildable now** (keys held to the no-spill rule until P5) |
| 2 | Sentis | *the briar thicket* | CAD only: four swept canes on a torus liner | Lost wax | 1 ruby round 4.0 | Not started; blocked on C-V4 and P6 (fallbacks exist); **build last** |
| 3 | Rosa mortua | *the dead rose* | Procedural `ShankKind::Bypass`, HighDome 3.6 × 2.7 | Lost wax | Ruby pear 7 × 5 (bud); **garnet round cabochon 6.0** (hip) | Not started; blocked on P6 (Sepal), C-B2 (pear) and C-V1 (`Relative`) for full fidelity |
| 4 | Hedera | *the strangling ivy* | Procedural `Uniform` DShape 5.5 × 2.0 host, plus a CAD vine | Lost wax | 7 black spinel round cabochons 2.2 | Not started; blocked on C-V2 for the rootlets (stamp fallback) |
| 5 | Ilex | *the Holly King's standard* | Factory **006 Square**, resized to 16 × 17, sand master | Sand (Delft) | 8 garnet round cabochons (2 × 2.0, 6 × 1.8) | Not started; **buildable now** |
| 6 | Viscum | *the golden bough* | Factory **003 Clover** 18 × 18, native (route 1) | Lost wax (route 1) | 21 moonstone round cabochons | Not started; **blocked on Logan's process decision**, then buildable now |
| 7 | Prunus | *Straif, the blackthorn* | Factory **012 Cushion** 10 × 10, sand master | Sand (Delft) | Onyx **round** cabochon 7.0 (the sloe); 4 diamonds 1.3 | Not started; buildable now at draft; final verdict gated on C-V1 `level` or a 012 prickle-probe row |
| 8 | Datura | *the thorn-apple* | Factory **011 Badge** 18 × 20, native | Lost wax | 8 black spinel round 1.5 | Not started; blocked on C-V1 (`Relative`) for a template that survives resize |

Stones: 0 + 1 + 2 + 7 + 8 + 21 + 5 + 8 = **52**. There are four signets (Ilex, Viscum, Prunus, Datura) and four vines (Rubus, Sentis, Rosa mortua, Hedera). Hawthorn is held back as a ninth, but its long straight spines repeat Prunus.

**Slugs:** `rubus-vepres`, `sentis-vepres`, `rosa-mortua-vepres` (not `rosa-vepres`, which is too close to Tenebrae's Rosa), `hedera-vepres`, `ilex-vepres`, `viscum-vepres`, `prunus-vepres`, `datura-vepres`.

## 2. Build order inside Vepres

Collection enablers come first:
- C-V1 is paired with C-V3.
- Then C-V4 and C-V5.
- C-V2 arrives from Tenebrae.

At most two lanes run at a time (the global rule). One branch, issue and PR per lane.

| Step | Ring(s) | Why this position |
|---|---|---|
| 1 | **Rubus** | Needs nothing new. The prickle probe already cleared its only new claim: a CAD part poured in sand. It calibrates seam beads, ring arrays on a keyframed body, and leaf stamps on side faces. Every later ring reuses these. |
| 2 | **Ilex** with **Prunus** | Both are sand stock rings on landed P3/P4. Ilex proves `parting_monotone` and `stamp_row` on the parting line. Prunus proves straight CAD spurs poured on a stock, and it is where C-V1 `level` is confirmed or shown unneeded. |
| 3 | **Viscum** | The heaviest stone count (21 seats on a stock), so it comes after the stock workflow is proven. It waits on Logan's process decision. |
| 4 | **Datura** | First ring that needs C-V1 `Relative` (spines seated in the capsule's frame). |
| 5 | **Rosa mortua** with **Hedera** | Both need the most enablers: P6 Sepal, C-B2 pear and C-V1 for Rosa; C-V2 and C-V3 for Hedera. Both are lost wax, so they cannot block a sand verdict. |
| 6 | **Sentis** | CAD-only csg with crossing sweeps is the riskiest assembly. It needs C-V4 (beads in CAD-only assembly) and P6 Thorn. The plan's fallback is three canes. |

## 3. Collection enablers (C-V1 to C-V5)

Each enabler has one lane that owns its files. Sizes: S is under one agent-day; M is one to three days; L is more.

### C-V1: placement `level`, `Relative`, `Side` (M)

Owned file: `core/cad.rs` (Placement section, `:392-502`).

Today `frame_on` (`:463`) reads the raw facet normal from `surface_hit`. Its frame:
- x runs along the finger (world −Z) squared to the normal;
- y = z × x, round the ring;
- `tilt_deg` turns about x, a lean along the ring;
- `cant_deg` turns about y, a lean across the ring;
- `spin_deg` turns about z.

```rust
pub enum Placement {
    Free,
    Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg,
           #[serde(default)] level: bool },   // zero the normal's axial part before squaring x, as Stamp::frame does (setting.rs frame section)
    Relative { part: Id, at: [f64; 3], rotation_deg: [f64; 3] },  // in `part`'s seated frame; `part` joins sources()
    Side { theta_deg: f64, radius_mm: f64, face: SideFacePick, height_mm: f64, spin_deg: f64, tilt_deg: f64 }, // z = ±Z on a side face
}
```

- **Who needs what.** Prunus needs `level`. Datura needs `Relative` (spines on the capsule). Rosa mortua needs `Relative` (sepal wisps on the hip). `Side` lets a part stand on a side face, which a radial `surface_hit` cannot reach; no Vepres ring requires it.
- **Serde and format.** Add every new field with `#[serde(default)]`. A `Relative` placement must list its `part` in `Feature::sources()` so ordering and skips are right.
- **Pins:**
  - `level` changes nothing on a mirrored master: rerun `prickle_probe` on 012 with and without it and expect equal results.
  - `Relative` follows its part when the part moves, as a resize test proves.

### C-V2: `PatternKind::Along` (M–L; lands in Tenebrae's batch with Porta)

Owned file: `core/cad/pattern.rs`.

```rust
PatternKind::Along {
    path: AlongPath,   // Crest { from_deg, to_deg } | Parting | Feature { id } | Chart { points: Vec<[f64; 2]> } | Curve { layer: usize }
    count: u32, pitch_mm: Option<f64>, phase: f64,
    alternate_deg: f64,   // spin added on every other copy
    roll_deg: f64,        // spin per copy (137.5 for phyllotaxis)
    scale: [f64; 2],      // start and end scale
    level: bool,
}
```

- **Frames.** Frames come from the path tangent and the surface normal. `Crest` reads `RingDesign::modulation_at`. `Parting` reads `skin::Hide`. `Feature` reads a Sweep or Twist centreline.
- **Scale needs non-rigid motions.** Motions are rigid today (`angles()` at `:126`), so the motion type must carry a scale. The count stays capped by `MAX_PATTERN_COUNT` = 120 (`:20`).
- **Vepres users:**
  - Hedera: rootlets, about 80 along the stem. Required, or use the stamp fallback.
  - Viscum: shoulder berries, if seats can read it. Otherwise pads.
  - Rosa mortua: prickles along the arm crests.
  - Sentis: thorns along the canes.
  - Prunus: graded spurs.

### C-V3: 3-D twisted sweeps with a scale law, and closed sweeps (M)

Owned files: `core/cad/twist.rs` and the Sweep arm of `core/cad.rs`.

Today a Twist path is one open planar `Sketch`: `chain()` refuses closed curves and more than 256 curves (`twist.rs:113-138`). A Sweep takes 2–128 stations (`cad.rs:1859-1861`) and hard-codes `closed: false` (about `:1877`).

```rust
Operation::Twist { sketch: Profile, path: TwistPath, degrees: f64, end_scale: f64,
                   #[serde(default)] scale: Vec<[f64; 2]>,   // (share, scale), share monotone; empty = linear to end_scale
                   #[serde(default)] closed: bool }
pub enum TwistPath { Sketch(Sketch), Points { points: Vec<[f64; 3]>, smooth: bool } }  // Catmull-Rom, ≤ 256 points
// and pass `closed` and SweepOptions { scale, twist } through Operation::Sweep (cadkernel already supports them).
```

- **Frames.** Use rotation-minimising frames, as `setting::tube` transports its frame. The planar path keeps its in-plane normal, so existing parts are bit-identical.
- **Laws.** Leaf law: 0.1 → 1.0 at 0.35 share → 0.05. Thorn law: 1.4 → 1.0 → 0.28.
- **Vepres users.** Sentis: closed canes, which remove the two-half joins, and tapered star shoots. Hedera: a tapering growing tip, and edge leaves. Rosa mortua: the withered leaf's taper.
- **Pin.** A closed sweep's mesh is watertight and has `self_crossings == 0`.

### C-V4: seam beads in CAD-only assembly (S)

Owned file: `core/parts.rs`.

`parts::assembled` (`:716`) never reads `blend_mm`. After each successful union there, call the same `blend::bead_seam` path that `resolve_with` uses for band joins (`:129-176`).

- **Needed by:** Sentis (thorn roots and shoot bases on canes).
- **Pin:** a CAD-only union of two cylinders with `blend_mm` 0.3 gains the analytic fillet volume within 5%.

### C-V5: graph path nodes (S–M)

Owned files: new `graph/nodes/path.rs` and its registry line.

- **Nodes.** `path.arc`, `path.helix`, `path.wreath(strands, crossings, wander, cane)`, `path.climb(turns, amplitude, surface)` and `path.crest(from, to, count)` return `List<Point3>`. Add `cad.features` (plural), which appends one feature per list item; today a list on `operation` makes N designs.
- **Depends on P7.** The component and placement pins on `cad.feature` come from P7, and they are what makes placement drivable.
- **Needed by:** Sentis (strands, crossings, amplitude), Hedera (the climb), Rubus (prickle count).
- **Until it lands.** A `script` node builds the operation JSON. Script nodes and expression pins both open from the menu now that P1 attached the engine.

---

## 4. Shared scaffolding and the gates every ring passes

**Layout** (the Arachne precedent):
- One example per ring: `ex/vepres_<slug>.rs`.
- CLI: `target/release/examples/vepres_<slug> [OUT_DIR] [--draft] [--verify]`.
- Default output: `showcase/vepres/<slug>/`.
- Shared code goes in `ex/common/vepres.rs`, pulled in with `#[path = "common/vepres.rs"] mod vepres;`, the way the probes use `common/probe.rs`.

The plan's `examples/vepres/` module layout is equivalent. Pick one and keep it.

**`common/vepres.rs` holds:**
- `stock(id, sand, face)` from `ex/common/probe.rs:45`.
- `sand_setup` / `wax_setup` / `cast_in` from `probe.rs:12-43`. For wax, set `min_section_mm` 0.8, `min_detail_mm` 0.15 and `min_draft_deg` 0, as `wax_setup` does.
- A `Skin` helper copied from `ex/stock_masterworks.rs:1135-1193`: `on_crest(arc)`, `on_cheek(rho, phi, side)`, `on_face(x, z)`. They map world places on a stock to chart `(theta, v)`; core has `Hide::crest_at` but not these.
- The gate and report writer, modelled on `bestiarium_arachne`'s `main`.
- Stone tints for `Gem::preview_tint`. Suggested sRGB: ruby [0.60, 0.05, 0.12], garnet [0.40, 0.04, 0.07], black spinel [0.03, 0.03, 0.04], moonstone [0.85, 0.88, 0.93], onyx [0.02, 0.02, 0.02], diamond none. Tune these in review.
- `ring_local_outlines`: ivy, vein comb and urn profile, below.

**Gates (plan §5, applied to every ring).** A red gate goes back to the author and does not use up a review round.
1. **Draft build at 768 × 320.**
   - Watertight, 0 degenerate faces.
   - `csg::self_crossings` is 0 on every made part (`core/csg.rs:1090`).
   - `built.solids.notes` and `built.parts.notes` are empty.
2. **Verdict.** Run `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)`, then `castability::judge_parts(&mut field, &d, &built)`. Judge against the ring's own `DraftSettings::process`.
   - **Sand:** `Castable`, not "with care". The ray release shows 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm (`probe::pull`). Clamp bite is ≤ 0.05 mm.
   - **Wax:** thinnest wall ≥ 0.8 mm.
3. **DFM.** `dfm::findings_in(&d, &lib)` returns **0 findings**. Caiman had 2.
4. **Stones.**
   - `stones::report` count equals the gem preview count (`gems::preview_vertices`).
   - No metal inside a stone (Arachne's `metal_in_stones` check).
   - The crowding census is clean or explained.
5. **Render set** in studio gold with stones set: hero, face, palm, side, a stone close-up, and bare stock against finished.
6. **Review.** A fresh reviewer scores the renders against Caiman, the Reptilia sheet and `b15/zbrush_top.png`. Cap: three rounds, then cut or defer.
7. **Export build** at 1536 × 448 with `--verify`: a cold reload with an empty library gives identical vertices.
8. **Template.**
   - The lift evaluates cold to the same design byte for byte.
   - At most 4 `design.set` patches.
   - Size and open time are recorded in `verification.json`.
   - Budgets after P7: procedural ≤ 300 KB; stock ≤ 1 MB; flag anything over 3 MB.
9. **Reel.** Stamp families name their steps: "Leaflet, 3" plays as "Leaflet".

**Format note.** A design is written at format 6 if any of these hold (`core/library.rs:56-66`):
- a stamp is not plain (tier > 0, a shaped top, or more than 512 points);
- a Pattern has several sources;
- a Revolve is `in_plane`.

That is expected for most Vepres rings. Say so in each README line.

**Outline units.** `outline::circle` takes a **diameter** (`core/outline.rs:166`); `Sketch::circle` takes a **radius** (`core/sketch.rs:282`).

---

## Rubus — *the bramble cane*

- **Status:** Not started. **Buildable now.**
  - P3 and P4 have landed, and the prickle probe is clean.
  - The side-face layers want P5 (batch 16). Until it lands, hold the keys to the no-spill rule below.
  - C-B1 `CurveLayer::phase` is optional.

- **Concept:** One bramble cane closed into a ring. It is knuckled at seven leaf nodes, and every hooked prickle along its back turns the same way round the ring. A fruiting runner arches down both side faces, carrying leaflets and blackberries. This is the collection's all-metal piece, and the first CAD part poured in sand.

- **Theme face to palm:**
  - **Crest, the top 206°** (θ 347° through 90° to 193°): 13 hooked prickles lying in the parting plane. Five large ones stand on the upper nodes; eight small ones stand in pairs between them.
  - **Side faces, all the way round:** the runner, one leaflet per internode, and one blackberry per node. The band is symmetric (`SideFaces::is_even()`), so both faces carry the pattern.
  - **Palm crest:** bare and polished; the nodes at 244° and 296° carry no prickles.
  - **Bore:** comfort fit.

- **Base:** This is exactly `prickle_probe.rs`'s `rubus()` (`ex/prickle_probe.rs:62-75`):
  - `RingDesign::default()`; `size = resize::size_from_bore(18.6)`.
  - `profile.width_mm` 7.0, `thickness_mm` 3.4, `apply_style(ProfileStyle::LowDome)`, `crown_mm` 0.6, `flatten_sides()` (`core/profile.rs:548`), `edge_round_mm` 0.3, `comfort_fit_mm` 0.1.
  - `ShankKind::Keyframes` with `amount` 1.0 and 14 `ShankKey`s. The cap is 16 (`core/profile.rs:1696`).
  - Nodes at θ = 90 + k·51.4286.
  - Internodes at node + 25.7143.
  - **With P5:** the brief's keys. Nodes `{w 1.05, t 1.15, c 1.10}`; internodes `{0.97, 0.93, 0.95}`.
  - **Without P5:** nodes `{w 1.00, t 1.15, c 1.10}`; internodes `{w 0.95, t 0.97, c 0.95}`.
    - The rule: a station's side-face share of its own surface arc must not fall below the reference's.
    - Thicker keeps the gate inside the face; wider or thinner spills it.
    - Add these keys as a row in `side_gate_probe` and confirm zero spill before painting.

- **Process:** Delft sand, `probe::cast_in(&mut d, &probe::sand_setup(0.1))`. Every proud element is either on a side face or lies wholly in the parting plane.

- **Stones:** None.

- **Build, step by step:**
  1. **Body.** The base above.
  2. **Runner.** `Layer::Curve(CurveLayer)` (`core/curve.rs:62`):
     - `WireProfile::Round` (a cosine dome), `width_mm` 0.8, `height_mm` 0.45, `repeats_around` 7, `mirror_v` true, `closed` false, `taper` 0.
     - Call `land_on_side_face(&ctx, 0.7)` (`:149`).
     - Entry window `VGate::SideFaces(SideFacePick::Both)` (`core/field.rs:420-438`).
     - The arches must crest at the nodes. Use C-B1 `phase` if it has landed; otherwise shift every control point's `x` by 0.75 of a cell, which works because x wraps.
  3. **Blackberries.**
     - One SVG alpha, "Drupelet cluster": seven radial-gradient drupelets 0.75 mm across in a 2.3 mm berry. Gradient SVG is the family that survives the floor (Serpentarium).
     - `Layer::Decals(DecalLayer { alpha, decals, feather_mm: 0.05, invert: false })`: 14 `Decal { theta_deg: node, v_mm: face centre, size_mm: 2.3, height_mm: 0.55, rotation_deg: 0, flip: <high face> }`.
     - Gated `SideFaces(Both)` with `Blend::Max`. `MAX_DECALS` is 64.
     - Set `flip` on the high face: the chart reads true from −Z (showcase lesson).
  4. **Leaflets.** 14 `setting::Stamp`s, one per internode per face (`core/setting.rs:897`):
     - `outline: outline::leaf(Margin::Serrate { teeth: 4, depth_mm: 0.22, lean_deg: 35.0 }, 3.2, 1.6)`
     - `height_mm` 0.42, `sink_mm` 0.25, `draft_deg` 3, `along_pull: true`, `top: StampTop::Gable { rise_mm: 0.12, axis_deg: 0.0 }` (a leaflet folded on its midrib).
     - Place each where the runner has swung to the far edge. Take `rot_deg` from the runner's tangent (`CurveLayer::sample_path`, `curve.rs:173`), mapped from cell x to θ and from v to v.
     - Gate each outline with `dfm::stamp_finest_mm(&outline, 0.30) ≥ 0.33`.
  5. **Withered leaflets.** On every third leaflet (4 per face):
     - a tier-1 cast cut bite: `outline::circle(1.8)` centred on the margin, `cut: true`, `tier: 1`, `height_mm` 0.6, `sink_mm` −0.02, `along_pull: true`. This is the crescent recipe: a floor 20 µm over the surface is never seen.
     - one tier-1 cast pit `outline::circle(0.6)`, `sink_mm` 0.3, `along_pull: true`. On a side face its walls stand parallel to the pull.
  6. **Veins.** One tier-1 **bench** cut per leaflet: a ring-local `vein_comb(len, veins: 4, groove: 0.16)` polygon (midrib plus four laterals), `cut: true, bench: true, tier: 1, height_mm` 0.1, `sink_mm` 0.12.
  7. **CAD document.** `d.cad = Some(Document)`:
     - **#1** `Operation::Band` (role Shank).
     - **#2** the large prickle, `Operation::Twist`:
       - `sketch: Sketch::circle(0.55)` (a radius, so a 1.1 mm round);
       - `path`: a sketch on `Workplane { x: [0,1,0], y: [0,0,1], .. }` with a 0.5 mm `Geometry::Line` radially out, then a 70° `Geometry::Arc` of radius 1.6 turning round the ring;
       - `degrees: 0.0, end_scale: 0.28`, giving a 0.31 mm tip.
       - Exact code: `ex/prickle_probe.rs:20-36`.
       - Component: `Placement::Ring { theta_deg: 347.1429, across_mm: 0.0, height_mm: -0.35, spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 }`, `Attach::Join`, `Stage::Cast`, `blend_mm` 0.35.
     - **#3** `Operation::Pattern { sources: [2], kind: PatternKind::Ring { count: 5, span_deg: 205.7143 } }`. A partial arc steps `span/(count−1)` = 51.43° (`core/cad/pattern.rs:133`), so the copies land on nodes 347°, 39°, 90°, 141° and 193°. Each copy is dropped onto the band at its own radius (`:251`).
     - **#4** small prickle A: circle 0.40, arc 60° at radius 1.1, `end_scale` 0.38, at θ 4.29.
     - **#5** small prickle B: the same, at θ 21.43.
     - **#6** `Pattern { sources: [4, 5], kind: Ring { count: 4, span_deg: 154.29 } }`. A multi-source pattern is format 6. It gives 8 small prickles between the upper nodes.
  8. Build, gate, render, review, export and verify, as in §4.

- **What it shows off:** a CAD part poured in two-part sand; ring arrays over a span on a keyframed body; seam beads; a side-face curve runner; leaf stamps with shaped tops and tiered cuts; decals; bench-only veins.

- **Traps and how they are avoided:**
  - **Off-crest proud relief locks** ("a bead row rides the crest line only"; off-crest rails 5.8% at 29°). Prickles sit at `across 0`, `cant 0` and `spin 0/180`. The probe shows one obstruction at a 2° cant.
  - **Crest-parallel ridges lock** (melon lobes 8.2% at −22°). Nothing runs round the ring on the crest; the runner is on the side faces.
  - **Reference-only gates spill on keyframes** (side-gate probe, 0.1–0.8 mm). Use P5, or the no-spill keys.
  - **Tips and teeth below the floor.** The tip is 0.31 mm against Delft's 0.30. The serration gate is `stamp_finest_mm ≥ 0.33`.
  - **Drupelets merge in the pour.** Accepted: size the berry so it still reads under the As-cast preview (`BuildParams::soften_mm`).
  - **Axial web.** The pits are 0.3 mm deep on a 7.0 mm band, so the web is ≥ 6.4 mm and the wall figure can be trusted.
  - **Wear.** Nothing proud stands on the palm crest.

- **Needs:**
  - Nothing that is not on master, if the no-spill keys are used.
  - P5 to use the brief's knuckle contrast.
  - C-B1 `phase` (optional).
  - P7 so the stamps lift as nodes.
  - Ring-local `vein_comb(len, veins, groove) -> Vec<[f64; 2]>`. Build it as a CCW polygon at `outline::STEP` spacing, validated with `outline::check`.

- **Template:**
  - `Graph::from_design` lift (`graph/lift.rs`).
  - Band, shank keyframes, `layer.curve` and `layer.decals` lift as nodes.
  - The six CAD features lift as a `cad.feature` chain (`nodes::cad::chain_document`), byte for byte.
  - Stamps lift as one `/stamps` patch until P7; draft settings as `/draft`.
  - Exposed: band width, thickness, "Shank taper" (keyframe `amount`), and prickle count. The count is a `script` node writing the Pattern JSON into `cad.feature`'s `operation` pin; with C-V5, `path.crest`.
  - Expect about 0.1–0.2 MB (one SVG, short outlines). Format 6.

- **Risk:** 2/5, low–medium.
  - If the keyframed prickles report obstructions at export, drop the small prickles to one per internode, then drop the keyframes.
  - If the knuckles read weak without P5, wait for P5 rather than widen the nodes.

---

## Sentis — *the briar thicket*

- **Status:** Not started. **Build last.**
  - Blocked on **C-V4** (seam beads in CAD-only assembly) and **P6** `style: Thorn`.
  - C-V3 (closed, tapered sweeps) and C-V5 (`path.wreath`) are upgrades.
  - Without C-V4 and P6 it can be built at reduced fidelity: crisp thorn roots and wire claws.

- **Concept:** Four briar canes wound into an open thicket round the finger. They cross over and under in a three-fold rhythm, with thorns all along them, and three cut shoots bristle up at the crown. Deep in the tangle, one ruby rose-hip is gripped by four thorn claws. This is the Sleeping Beauty hedge at ring scale, and the collection's unique silhouette.

- **Theme face to palm:**
  - **Crown:** the densest tangle, the three shoots and the hip.
  - **Shoulders:** crossings with thorns.
  - **Palm:** the tangle thins; there are no shoots and no hip, and the thorns there are smaller.
  - **Bore:** a smooth torus liner hidden inside the canes.

- **Base:** CAD only. There is no `Operation::Band`, so `Document::replaces_band()` holds (`core/cad.rs:1322`) and overlapping metal parts are united by `parts::assembled` (`core/parts.rs:716`). Bore 18.6. With no band surface, `Placement::Ring` falls back to the analytic `frame()` (`core/cad.rs:431`): radius = `inner_radius + profile.thickness_mm + height_mm`, z radial, x along the finger.

- **Process:** Lost wax, using `wax_setup`. No parting plane clears a weave ("a true helix locks"), and the thorns point every way.

- **Stones:** One ruby round 4.0 in a four-claw head, `Gem::calibrated(GemCut::Round, 4.0)` with a ruby tint.

- **Build, step by step:**
  1. **Heartwood liner** (#1). `Operation::Torus { major_mm: 9.85, minor_mm: 0.55 }` (inner 9.30 = the bore, outer 10.40), `Placement::Free`, Join, Cast.
  2. **Canes** (#2–#9). Four canes, each `Operation::Sweep { sketch: Sketch::circle(0.72).into(), path }` in `Placement::Free` world mm.
     - Path of cane i: r(θ) = 10.85 + 0.55·cos(3θ + iπ/2), z(θ) = 2.1·sin(3θ + iπ/2).
     - Opposite canes cross with centres 1.1 mm apart radially, so every crossing is a real 0.34 mm overlap, never a tangency.
     - The inner envelope is 9.58, inside the liner's 10.40, so every cane is united with the liner.
     - **Until C-V3:** each cane is two halves of 128 stations (the cap), θ 0–200° and 180–380°. The second half's radius is **0.74**, not 0.72. Identical tubes over the 20° overlap would be near-coincident faces, which trigger `Snag::Degenerate` retries and slivers; a 0.02 mm step removes that and hides under the thorns.
     - **With C-V3:** one closed sweep per cane, and a star section tapered 1.0 → 0.85 between crossings.
  3. **Thorns.** 12 hooked `Operation::Twist` prickles on one 120° period (base circle 0.40, 1.3 mm long, `end_scale` 0.4). Each is placed `Placement::Ring { theta_deg, across_mm: z_cane(θ), height_mm: r_cane(θ) + 0.72 − 0.25 − (9.3 + thickness_mm) }`, and `spin_deg` alternates 0/180 so the hooks read both ways. Then one `Pattern { sources: [12 thorn ids], kind: Ring { count: 3, span_deg: 360.0 } }` gives 36 thorns in one multi-source feature (P2). Thorns are separate small solids, so no copy overlaps another inside one Pattern part.
  4. **Shoots.** Three `Operation::Twist`:
     - section: a five-point star `Geometry::Polyline { closed: true }` 1.2 mm across (briar canes are angled; a twist shows only on a non-round section);
     - path: a 2.6 mm planar arc;
     - `degrees` 150, `end_scale` 0.5;
     - `Placement::Ring` at θ 72, 90 and 108, on the outermost cane at each angle.
  5. **Hip cup.** `Operation::Revolve` of a ring-local `urn_profile(Ø 4.6, h 3.0, flat top)` sketch on `Workplane { x: [1,0,0], y: [0,0,1] }`, with `pivot [0,0,0]`, `axis [0,0,1]`, `degrees 360`, and `in_plane: false`, so it stays format 5 for this feature. `Placement::Ring { theta_deg: 90, across_mm: 0, height_mm: h }` nests it among the canes. Join, Cast, `blend_mm` 0.3 (needs C-V4).
  6. **Ruby.** `builders::stone_feature(id, gem, Placement::Free)` (`core/cad/builders.rs:954`). Then write a `FaceSeat { face: <cup top FaceRef>, u_mm: 0, v_mm: 0, height_mm: stand_off, spin_deg: 0 }` into its params under the `"seat"` key (`core/cad.rs:511-535`), with `on: Some(cup_id)`.
  7. **Claws and seat.**
     - `builders::feature_on(id, "Thorn claws", "head.claw", ruby, json!({ "prongs": 4, "style": "Thorn", "tip": "Point" }))`. The style and tip keys come with P6; without P6 use `{ "prongs": 4 }`.
     - `feature_on(id, "Seat", "seat.bur", ruby, json!({ "through": false }))`.
     - Both are `Stage::Cast`. The claw feet stand on the cup, a kernel part with a planar face, never on a cane.
  8. Build, gate, render, review, export and verify. There are about 33 features, well under the 256 cap (`core/cad.rs:1301`).

- **What it shows off:** a whole ring built from CAD features; 3-D sweeps; csg union of many overlapping parts; a multi-source pattern; a stone seated on a part's face; claws (Thorn style).

- **Traps and how they are avoided:**
  - **Tangent contacts become `Snag::Degenerate`** and are nudged up to eight times ("Booleans are exact in topology"). Every crossing is designed as a 0.34 mm overlap, and the half-canes differ in radius.
  - **No beads in CAD-only assembly.** Land C-V4 first, or accept crisp roots.
  - **Folded tubes.** Thorn and shoot paths are straight–arc with the bend radius ≥ the local radius ("a swept tube folds through itself wherever its rings tilt more than they stand apart"). Gate: `self_crossings == 0`.
  - **Wax floors.** Canes Ø 1.44, thorn bases Ø 0.8 and tips 0.32; shoot tips ≥ 0.6 across.
  - **Build time.** About 45 solids through `csg::union_all`. Budget several seconds at export; P1's phase-two progress should show "parts j/M".

- **Needs:**
  - C-V4, P6 (Thorn).
  - C-V3 (closed canes, tapered star).
  - C-V5 `path.wreath(strands, crossings, wander, cane)`.
  - C-V2 `Along { Feature }`, which would fold the 12 thorn features into one array per cane.
  - Ring-local `urn_profile(dia, h) -> Sketch`.

- **Template:**
  - A `cad.source` → `cad.feature` chain of about 33 nodes, which lifts byte for byte. Tens of KB.
  - Each cane path is a 128-point JSON array. A `script` node can generate it into the `operation` pin, exposing "Strands", "Crossings", "Wander" and "Cane"; with C-V5 this becomes `path.wreath`.
  - Graph mode: Free (lost wax); confirm the lift picks Free for a CAD-only design (unverified).

- **Risk:** 3.5/5, medium–high (csg robustness).
  - Fallback 1: three canes (the plan's fallback) with the same period.
  - Fallback 2: canes as `TwistedRing` (`core/cad.rs:48`), with thorns only.

---

## Rosa mortua — *the dead rose*

- **Status:** Not started.
  - Buildable now at reduced fidelity: an oval bud, a plain five-claw head, and wisps placed in world coordinates.
  - Full fidelity needs **P6** (Sepal claws), **C-B2** (true pear plan) and **C-V1** `Relative` (wisps follow the hip).

- **Concept:** One rose stem closes round the finger and passes itself. One arm ends in a ruby bud that never opened, held in five sepals. The other ends in a garnet hip still crowned by its dried sepals. Hooked prickles run down both arms, a three-leaflet leaf springs below the bud, and a withered leaf hangs off the other arm.

- **Theme face to palm:**
  - **Face:** the bud (arm A) and the hip (arm B) flank the top.
  - **Shoulders:** seven prickles per arm, graded smaller toward the palm; the leaf below the bud; the withered leaf on arm B.
  - **Palm:** the plain stem.
  - **Bore:** smooth.

- **Base:**
  - Procedural `ShankKind::Bypass`, `ProfileStyle::HighDome` 3.6 × 2.7 (a round stem), bore 18.6.
  - Each arm slides to ±`BYPASS_OFFSET` = 0.45 of the half-width and ends `BYPASS_TIP_DEG` = 35° past the top, rounding over the last 30° (`core/profile.rs:2321-2332`).
  - `profile::bypass_span(off, k)` (`:2351`) gives each arm's centre and half-width.

- **Process:** Lost wax. The sepal claws and wisps overhang, and the prickles ride arm crests that slide along the finger ("the crest wanders in v": 3% at 50°).

- **Stones:**
  - Bud: ruby pear 7 × 5, `Gem::calibrated(GemCut::Pear, 5.0)` with `l_mm` 7. Until C-B2, the pear plan is an ellipse (`plan_pow` 2.0), so use an oval 7 × 5.
  - Hip: garnet **round** cabochon 6.0, `Gem::cabochon(GemCut::Round, 6.0)`. The plan changed it from an oval, which repeated the Toi et moi starter.

- **Build, step by step:**
  1. **Body.** The base above. Read each arm's crest per angle through `RingDesign::modulation_at`. Every placement below takes its `across_mm` from that, never from a constant.
  2. **Bud stone.** `stone_feature` at `Placement::Ring { theta_deg: 115.0, across_mm: +arm_A_centre(25°), height_mm: builders::stand_off_mm("head.claw", gem), tilt_deg: 20.0, .. }`, so the bud nods. Both stones sit at least 25° from the top, because a bypass crossing needs room under a stone (collection3).
  3. **Sepal head.** `feature_on(.., "head.claw", bud, json!({ "prongs": 5, "style": "Sepal" }))` (P6), then `seat.bur`, both Cast. Without P6: `{ "prongs": 5 }` with `wire_mm` 0.7.
  4. **Hip stone and bezel.** `stone_feature(Gem::cabochon(Round, 6.0))` at θ 65 on arm B's centre. Then `feature_on(.., "head.bezel", hip, json!({ "wall_mm": 0.45, "lip": 0.35 }))`. `BEZEL_SINK_MM` 0.25 (`core/cad/builders.rs:42`) wants metal under the collet's base, and the arm tip supplies it.
  5. **Dried sepal crown.**
     - One `Operation::Twist` wisp: a lens section 0.7 × 0.5 (above the 0.5 fill floor), a planar arc curling out 100°, `degrees` 60, `end_scale` 0.6.
     - Then `Pattern { sources: [wisp], kind: PatternKind::About { part: hip_stone, count: 5, span_deg: 360.0 } }` (`core/cad/pattern.rs:45`), round the stone's own table axis.
     - With C-V1, seat the wisp `Placement::Relative { part: hip_stone, at: [girdle radius, 0, 0.2], rotation_deg: [..] }` so it follows the hip on resize; until then use `Placement::Free` from the hip's seated frame.
  6. **Prickles.** 14 `Operation::Twist` prickles (the Rubus section and path, scaled 1.0 → 0.6 toward the palm).
     - Each is placed individually at `(θ_k, across_k)` read off its arm crest.
     - `spin_deg` alternates ±25° so the hooks lean down the stem.
     - `blend_mm` 0.25.
     - With C-V2, two `Along { path: Crest { .. }, scale: [1.0, 0.6], alternate_deg: 50 }` arrays replace the 14 features.
  7. **Leaf.** Three leaflets, each an `Operation::Loft` (2–32 sections, `core/cad.rs:1904`) of five lens sketches along a curling spine. Widths 0.6 / 1.9 / 2.2 / 1.4 / 0.5, each section turned progressively up to 35° so the leaflet cups. A twisted petiole (`Twist` of `Sketch::circle(0.36)`) joins them to arm A below the bud.
  8. **Withered leaf.**
     - A lens `Twist` with `degrees` 70, then `Operation::Boolean { a: leaf, b: bite_cylinder, kind: Boolean::Subtract }`.
     - The twist is a mesh, so the boolean runs through csg, not the kernel (the kernel refuses faceted operands over 500 faces).
     - With C-V3, use the leaf scale law in place of the boolean taper.

- **What it shows off:** a procedural bypass carrying CAD parts; an `About` array round a stone; lofts; a csg boolean inside the feature tree; claws and a bezel; seam beads.

- **Traps and how they are avoided:**
  - **Room under a stone at the crossing:** stones at ±25° from the top.
  - **Claw folds:** the Sepal path keeps straight–arc–straight with bend radius ≥ 0.8 wire.
  - **Wax floors:** wisps and petioles ≥ 0.6 thick.
  - **Sliding crest:** every prickle is placed from the modulated section.
  - **A head moves by its stone:** transform the stones, never the heads.

- **Needs:**
  - P6 (Sepal; the bud is the point of the ring).
  - C-B2 (pear).
  - C-V1 `Relative`.
  - C-V2 `Along { Crest }` (optional).
  - C-V3 scale law (optional).

- **Template:**
  - Band and shank nodes plus a chain of about 30 `cad.feature` nodes.
  - Exposed: stem width and thickness, bud and hip sizes. The stone builders' `params` are JSON pins; after P7, numeric pins.
  - Prickle placement stays in the feature tree until C-V2.
  - Under 200 KB.

- **Risk:** 3/5, medium.
  - Fallback: an oval bud in a plain five-claw head, wisps in world placement, and the withered leaf dropped.

---

## Hedera — *the strangling ivy*

- **Status:** Not started.
  - Blocked on **C-V2** for the rootlets; a stamp fallback is below.
  - C-V3 is an upgrade for the stem's growing tip and the edge leaves.
  - Ivy outlines are ring-local and need nothing new.

- **Concept:** A plain polished court band, the host, is strangled by an ivy stem climbing it from edge to edge. The stem grips with rows of aerial rootlets. Lobed juvenile leaves follow the creeping stem, heart-shaped adult leaves crowd the crown, and an umbel of black berries crowns it. This is botanically right: ivy fruits only on its adult, unlobed growth.

- **Theme face to palm:**
  - **Crown:** the umbel of seven black spinel berries and three adult cordate leaves.
  - **Shoulders:** the stem crosses edge to edge twice, with lobed juvenile leaves on petioles.
  - **Palm:** the stem's cut end, its rootlets, and one juvenile leaf.
  - **Side faces:** the stem rolls over each edge onto them for a few millimetres.
  - **Bore:** bare host metal.

- **Base:** Procedural `ShankKind::Uniform`, `ProfileStyle::DShape` 5.5 × 2.0, `comfort_fit_mm` 0.15, bore 18.6, polished.

- **Process:** Lost wax. The stem crosses the crown diagonally (off-crest rails measured 1.1% at −31°), and the rootlets and petioles overhang.

- **Stones:** Seven black spinel round cabochons 2.2, `Gem::cabochon(GemCut::Round, 2.2)`, in collets.

- **Build, step by step:**
  1. **Host.** The base above.
  2. **Atlas.** `skin::Atlas::of(&d, 2048, 512)?` samples the procedural body one modulated section per column (`core/skin.rs:52`).
     - Write the stem path in chart coordinates `(θ, v)`, then map each point to world with `Atlas::point(θ, v)` plus 0.12 mm along the sample normal. About 40% of the Ø 1.24 stem is sunk.
     - Near each edge, let `v` run past the crown into the side face so the stem rolls over the fillet.
  3. **Stem.** Two `Operation::Sweep { sketch: Sketch::circle(0.62).into(), path }`, 128 stations each, overlapping 12°, the second at radius 0.64 (the Sentis rule). `Placement::Free`, `Attach::Join`, `blend_mm` 0.3. The band is procedural, so the seam bead is laid today.
     - Path: θ from 250° over **330°**, with z = 2.0·sin(1.5(θ − 250°)).
     - The brief's 450° of travel ends exactly on the stem's own start at z = 0: 1.5 × 360 = 540°, so the second pass mirrors the first and meets it there.
     - Tune the phase so a crown crossing sits under the umbel at θ 90.
  4. **Rootlets.**
     - **With C-V2:** a tiny cone (`Operation::Extrude { sketch: Sketch::circle(0.11), height_mm: 0.6, draft_deg: 6 }`) under the stem, then `Pattern { kind: Along { path: Feature { id: stem }, pitch_mm: 0.6, alternate_deg: 180, .. } }`, about 80 copies (under the 120 cap).
     - **Fallback today:** about 80 tier-0 stamps on the band either side of the stem path, `outline::circle(0.3)` with `top: StampTop::Cone { apex_mm: 0.35, at: [0,0], tip_mm: 0.08 }`, `height_mm` 0.05. The stem joins over their roots, because parts resolve after stamps.
  5. **Leaves.** Nine stamps on the host (`sink_mm` 0.2, `height_mm` 0.45, `draft_deg` 3):
     - six juvenile, from a ring-local `ivy(lobes: 5, length: 3.5–4.5, width, sinus: 0.35)`;
     - three adult, from `ivy_cordate(5.0, 4.6)`;
     - each with a tier-1 raised midrib `outline::lanceolate(len·0.8, 0.5, 0.1)` at `height_mm` 0.15;
     - tier-1 cast vein cuts, `sink_mm` 0.12. They are allowed under wax at a 0.15 floor.
     - Every leaf must lie within the outer surface; `Stamp::stand` refuses one that "runs off the edge of the surface it stands on".
     - Leaves cannot sit on the stem: stamps are made against the band before parts join (`core/mesh.rs:322-324`).
  6. **Edge leaves.** Two leaves curl over the band edge as CAD parts: an `Operation::Loft` of five lens sections today, or with C-V3 a lens Twist with the leaf scale law.
  7. **Petioles.** Nine `Operation::Twist` of `Sketch::circle(0.36)` (Ø 0.72) along 60–90° planar arcs, from the stem to each leaf base. Join, `blend_mm` 0.2.
  8. **Umbel.**
     - Seven `stone_feature(Gem::cabochon(Round, 2.2))`, placed `Placement::Ring { theta_deg: 90 ± δ_k, across_mm: a_k, height_mm: h_k }` as a dome over the crown. Each is followed by `head.bezel { wall_mm: 0.4, lip: 0.3 }`.
     - Stalks: `Operation::Sweep` of `Sketch::circle(0.35)` along a two-point path, from the umbel point to each collet base, ending **inside** the collet (the `cad::examples` gallery pattern, `core/cad/examples.rs:159-203`).

- **What it shows off:** a 3-D sweep on a procedural band; about 30 CAD parts meeting one band through csg with seam beads; conforming stamps with tiered midribs; a cluster of collets on stalks; a path array (C-V2).

- **Traps and how they are avoided:**
  - **Section floor:** stalks and petioles are ≥ Ø 0.7 against the investment floor of 0.5.
  - **Collet with no metal under it:** the stalk ends inside the collet base.
  - **A self-meeting stem:** use 330° of travel, or lift the second pass one diameter where it crosses the first.
  - **Build load:** about 30 joined parts. Oriel's 37 made stones took 4.3 s at export; budget accordingly.

- **Needs:**
  - C-V2 (or the stamp fallback).
  - C-V3 (optional).
  - C-V5 `path.climb`.
  - Ring-local outlines. Both are polar, built at `outline::STEP` spacing and checked with `outline::check`:
    - `ivy(lobes, length, width, sinus) -> Vec<[f64; 2]>`: palmate, with a cordate notch at the stalk;
    - `ivy_cordate(length, width) -> Vec<[f64; 2]>`.
    - `outline::leaf`'s `Margin::Lobed` is pinnate, not palmate.

- **Template:**
  - Band nodes; stamps as a `/stamps` patch until P7; a chain of about 35 `cad.feature` nodes.
  - Exposed: host width and thickness, and berry size, which feeds the seven stone params through one `script` node.
  - The stem path is a `script` node until C-V5 `path.climb`.
  - About 150 KB.

- **Risk:** 3/5, medium.
  - Fallback: drop the edge leaves, and use stamp rootlets.

---

## Ilex — *the Holly King's standard*

- **Status:** Not started. **Buildable now.** P3, P4 and P2 have landed. The resize of 006 to 16 × 17 is untested (step 1).

- **Concept:** The Holly King's standard. A holly leaf lies across the face along the parting line, between two garnet berries. Holly sprays with berries fill both cheeks. A garland of small holly leaves rides the parting line down both shoulders. The palm is bare, like holly's smooth bark.

- **Theme face to palm:**
  - **Face:** the leaf and two berries on the line; the field polished.
  - **Cheeks:** a spray of three leaves and three berries each.
  - **Shoulders:** a garland of four leaves a side on the parting line, graded smaller toward the palm.
  - **Palm:** bare.
  - **Bore:** the stock's own.

- **Base:**
  - Factory 006 Square: native face 16 × 21 (`core/imported_base.rs` presets, `"006" => "Square", 16.0, 21.0`).
  - Build it as `stock_masterworks::base()` builds Aurelia, which is also 006 in sand (`ex/stock_masterworks.rs:317-380`): `ImportedBase::attach(&mut d, sand_master(source)?)`, `sand_envelope = true`, Flat profile, `profile.width_mm` 17.0, `shank.head.length_mm` 16.0, `size_from_bore(18.6)`, `edge_round_mm` 0.3, `comfort_fit_mm` 0.1.
  - Then set `SurfaceChart { profile, bore_radius_mm }` **from this stock** before drawing anything.

- **Process:** Delft sand: `mf::Recipe::sand(SandProcess::DelftClay)`, 18k yellow gold.

- **Stones:** Eight garnet round cabochons: two of 2.0 on the face line and six of 1.8 on the cheeks. All go in `SeatStyle::GypsyMound` pads with `solid: SolidKind::Flush` and `metal_true`. The sand pattern gets `mark_mm` drill dots.

- **Build, step by step:**
  1. **Stock and resize check.**
     - Build the bare stock at 16 × 17. If `mesh::try_build` refuses it, bake an intermediate step the way `stock_spike::bake` does (`ex/stock_spike.rs:117`); 013 needed 130% steps. 006 has already gone *up* to 20 × 26 for Aurelia, but this is a shrink across the band.
     - Record the envelope fill with `stock_spike`'s `fill_mm`; it must be ≤ 0.3 mm.
  2. **Skin.** `let a = skin::Atlas::of(&d, 2048, 768)?; let hide = skin::Hide::of(&a);` These are the parting probe's own sizes.
     - `hide.folds(&a, deg_per_mm)` gives the folds where the parting line turns over the head's end wall.
     - `hide.reach()` gives the run.
  3. **Face leaf.**
     - `Stamp { name: "Holly leaf", outline: outline::leaf(Margin::Holly { spines: 3, depth_mm: 0.35, lean_deg: 15.0 }, 8.0, 5.0), height_mm: 0.55, sink_mm: 0.3, draft_deg: 4.0, along_pull: false, top: StampTop::Gable { rise_mm: 0.25, axis_deg: 0.0 }, rot_deg: 0.0, .. }`.
     - Place it at `hide.crest_at(&a, 0.0)`. That is the interpolated `v` where the section crosses z = 0 (`core/skin.rs:296`); a tip 0.05 mm off the line locks.
     - The gable puts the midrib on the line and falls away from it on both sides.
     - Gate: `stamp.parting_monotone(&d).is_ok()` (`core/setting.rs:2149`). `outline::leaf` margins are single-valued along x by construction (`core/outline.rs:285`), so lean stays well under the probe's 23.6°.
  4. **Veins.** A tier-1 **bench** cut `vein_comb(7.2, 5, 0.16)`, `cut: true, bench: true, tier: 1, height_mm: 0.1, sink_mm: 0.15`. Bench stamps are never split on the parting plane and never enter the pattern.
  5. **Face berries.** Two `SeatPadLayer`s at `hide.crest_at(&a, ±5.9)`, using the `flush_seat` recipe (`ex/stock_masterworks.rs:1244-1262`): `style: GypsyMound`, `crown: 1.0`, `blend_mm` 0.45, `metal_true`, `solid: Flush`, `through: true`, `fit_stone(Gem::cabochon(Round, 2.0))`, mound Ø 2.8, `mark_mm` 0.6.
     - The mounds end 0.5 mm from the leaf tip, which is above the 0.30 floor, and 0.7 mm inside the face's end.
  6. **Cheek sprays.** For each cheek, place with `Skin::on_cheek(rho, phi, side)` (copied helper; keep only samples with `Atlas::cheek(s) > 0.9`):
     - three leaf stamps `outline::leaf(Margin::Holly { spines: 3, depth_mm: 0.3, lean_deg: 15.0 }, 5.0, 2.6)`, `along_pull: true`, `height_mm` 0.5, `draft_deg` 4, each with a tier-1 bench vein cut;
     - three garnet cabochons 1.8 in gypsy mounds (Ø gem + 0.9) at the spray's heart.
     - Keep every concave bay at least 1 mm below the rim (the crescent lesson). Cheeks face the pull.
  7. **Shoulder garland.** One `stamp_row` per the P4 API (`core/setting.rs:1558-1581`):
     `StampRow { stamp: <leaf 4.2 × 2.4, Gable top, height 0.45>, path: RowPath::PartingLine, from_deg: <face end + 1 mm clear>, to_deg: <that + ~32°>, count: 4, taper: 0.24, fold_clear_mm: 1.0, mirror_shoulders: true }`.
     - `taper` 0.24 grades 4.2 × 2.4 down to 3.2 × 1.8.
     - `fold_clear_mm` drops any station within 1 mm of a fold (Caiman: 0.06 mm).
     - Tips lead away from the head on both sides; the mirror flips `rot_deg`.
     - Check `parting_monotone` on every struck stamp.
  8. Build, gate, render, review, export and verify.

- **What it shows off:** the parting line used as a path; figurative stamps poured in sand on a zero-draft table; `parting_monotone`, gable tops, tiered bench cuts and `stamp_row` with mirrored graded shoulders; flush cabochons; the stock workflow.

- **Traps and how they are avoided:**
  - **Anything proud off the parting line undercuts** (Saurian). All face and shoulder relief lies on the line and is gated by `parting_monotone`.
  - **A stamp on the crest needs the crest exactly** (0.023 mm, Caiman). Use `crest_at`, never the nearest row.
  - **Under a stamp the surface may vary only across the band** (phantoms 0.03–0.07 mm). The table is flat. The garland stays off the folds; any garland leaf that still reports phantoms becomes a bench cut.
  - **Cheek lean** (a 0.35 mm tuck): `along_pull: true`.
  - **Cast dots off the line lean** (14°). The face berries are on the line, and the skirts are ≥ 0.45, because a seat's skirt is its finest DFM feature.
  - **Concave bays.** Safe on the line only through the monotone rule. The fallback is `hull_and_bays(&outline, 0.2)` (`core/setting.rs:1490`): cast the hull, bench-cut each bay. This is the parting probe's "filled outline pulls".

- **Needs:**
  - Nothing unlanded.
  - P7 (`base.preset`, stamp nodes) for a light template.
  - Ring-local `vein_comb`.

- **Template:**
  - Imported-base lift: the "Stock body" and "Stock face dimensions" nodes (`graph/lift.rs:336-382`) plus an `/imported_base` patch carrying the refined master mesh, about 3 MB until P7's `base.preset` replaces it.
  - Seats lift as `layer.seat` nodes. Stamps lift as a `/stamps` patch until P7.
  - No painted alphas. After P7, expect < 1 MB.

- **Risk:** 2/5, low–medium.
  - Fallback 1: `hull_and_bays` for the face leaf.
  - Fallback 2: if the 16 × 17 resize fights the master, native 16 × 21 with the leaf lengthened to 9.0.

---

## Viscum — *the golden bough*

- **Status:** Not started. **Blocked on Logan's process decision** (§0). After that it is buildable now: route 1 needs nothing unlanded.

- **Concept:** Aeneas's golden bough, his passport into the underworld, which Frazer identified as mistletoe. The factory clover's four lobes are the mistletoe's two crossed leaf pairs, each cut in intaglio at the bench like a seal. Three moonstone berries sit in the fork where the lobes meet. Forked twigs with paired leaves fill the cheeks. Graded moonstone berries run down both shoulders on the crest line, and the shank is the host oak, with bark on its walls.

- **Theme face to palm:**
  - **Face:** four lobes, each engraved as a leaf, joined by an engraved fork, with three proud berries at the centre.
  - **Cheeks:** Y-forked twigs, spatulate leaf pairs, and two berries each.
  - **Shoulders:** seven graded berries a side on the crest line.
  - **Shank walls:** oak bark.
  - **Palm crest:** bare.
  - **Bore:** the stock's own.

- **Base (route 1):**
  - Factory 003 Clover 18 × 18 at native size, **not** mirrored: `ImportedBase::attach(&mut d, source)` with `sand_envelope = false`.
  - Flat profile, `profile.width_mm` 18, `head.length_mm` 18, bore 18.6, `SurfaceChart` from this stock.
  - The recipe is set as `base()` sets non-sand stock (`ex/stock_masterworks.rs:379-385`): `CastProcess::LostWax`, `min_draft_deg` 0, `min_detail_mm` 0.15, `min_section_mm` 0.8.

- **Process:** Lost wax (route 1). The sand master needs 4.2 mm of envelope on 003 and is refused (`stock_spike`).

- **Stones:** 21 moonstone round cabochons: three of 2.5 on the face, four of 1.6 on the cheeks, and 14 graded 2.0 → 1.3 on the shoulders. All are gypsy-mound pads with `solid: Flush`, `metal_true` and `through` where the wall allows.

- **Build, step by step:**
  1. **Stock.** As above. Then `Atlas::of(&d, 2048, 768)`, `Hide::of(&a)` and the `Skin` helpers.
  2. **Find the lobes.** Take the four lobe axes from the face samples. `Preset::plan` for 003 has notches (r ≈ 0.64–0.70) at 0°, 90°, 180° and 270° of its own frame, so the lobes lie at 45° + k·90°. Confirm on `003-*-face.png` before cutting.
  3. **Intaglio leaves.** Four bench cut stamps, `outline::lanceolate(6.0, 2.2, 0.3)` (a mistletoe strap leaf), centred on each lobe with `rot_deg` along the lobe axis: `cut: true, bench: true, height_mm: 0.4, sink_mm: 0.25, top: StampTop::Gable { rise_mm: 0.12, axis_deg: 0.0 }`, so the cut floor is keeled like an engraved leaf. Confirm the sense of a cut's gable floor in P4's tests (unverified).
  4. **Fork.** One bench cut `outline::fork(4.0, 70.0, 0.5, 0.4)` at the centre, joining the leaves' stalks.
  5. **Face berries.** Three `SeatPadLayer`s in a triangle at the centre point (`Skin::on_face`): gypsy mound Ø 3.4, `fit_stone(Gem::cabochon(Round, 2.5))`, 0.4 mm gaps. Gypsy-skirted seats at column pitch merge into one ridge, so keep them apart.
  6. **Cheeks.** For each cheek:
     - a tier-0 twig stamp `outline::fork(5.0, 60.0, 0.6, 0.5)`;
     - three spatulate leaf pairs `outline::leaf(Margin::Entire, 3.6, 1.4)`, `along_pull: true`, `height_mm` 0.45;
     - two moonstones 1.6 flush in the fork.
     - Keep the crotch ≥ 1 mm below the rim.
  7. **Shoulder runs.** Seven pads a side at `hide.crest_at(&a, along_k)`:
     - Sizes follow a raised cosine from 2.0 to 1.3.
     - `along_k` accumulates `d_k/2 + 0.45 + d_{k+1}/2`, an equal 0.45 mm metal bridge. The graded-run principle holds the bridge constant, not the angle.
     - Mound Ø gem + 0.9. The lowest berry stays ≥ 1 mm off any fold (`hide.folds`).
     - `SeatRunLayer` cannot do this: its `v` is fixed (`core/field.rs` seat-run section) and the stock's crest wanders. C-V2 may later drive pads.
  8. **Oak bark.**
     - `a.paint("Oak bark", |s| a.cheek(s).max(a.shoulder(s)) * furrows(hide.at(s)))`, where the furrows run round the ring. Relief 0.35.
     - Show it with `skin::hide_layer(&d, "Oak bark", 0.35, window(270.0, 220.0))`, `Blend::Max`.
     - Under wax, `draft_clamp` is optional; run it anyway and record `ClampReport`, so route 2 is a switch.
     - If C-R4 (hide-space tilings) and C-R1 (clamped groups) have landed, use a procedural bark tiling in hide mm instead of a painted atlas. That keeps the template light.
  9. Build, gate, render, review, export and verify.

- **Route 2 deltas (sand, procedural):**
  - The base becomes a procedural signet: `apply_signet(width)` with `loft` 1.0; the outline is the imported `CG Clover` table through `CustomOutline::from_points`; `dome` is set from `suggest_dome`; 18 × 18 on a LowDome body 3.0 thick.
  - Delft recipe.
  - The bark must go through `draft_clamp` (bite ≤ 0.05).
  - The face berries and shoulder runs are already on the crest line.
  - The cheeks are the lofted head's walls. The reference side face is only the bottom strip of a lofted head's wall (Oriel lesson), so place cheek stamps from the Atlas `cheek` mask, never from `SideFaces`.

- **What it shows off:** bench intaglio as a first-class construction; many seats on a factory stock; equal-bridge graded stations along the crest line; a painted bark skin as support rather than subject; host and parasite as one theme.

- **Traps and how they are avoided:**
  - **Off-line face relief** (sand only). The seal is cut after the pour.
  - **Seats merging at column pitch:** 0.4 mm gaps.
  - **Crest ridges running round the ring:** the bark stays on the walls and the palm crest is bare.
  - **Concave cheek crotches:** ≥ 1 mm below the rim.
  - **Build time:** 21 seats and about 14 stamps at 1536 × 448. Expect several seconds; P1's phase-two progress should show it.

- **Needs:**
  - Logan's decision.
  - Nothing unlanded for route 1.
  - C-R1 and C-R4 would make the bark a light template.
  - P7.

- **Template:**
  - Imported-base lift with an `/imported_base` patch (about 3 MB) until P7.
  - 21 `layer.seat` nodes.
  - Painted bark embedded (about 3 MB) unless C-R4 makes it procedural.
  - Stamps as a patch until P7.
  - Flag anything over 3 MB.

- **Risk:** 3/5, medium.
  - Fallback: drop the cheek berries (−4 stones), and use a 1.5 mm uniform shoulder run.

---

## Prunus — *Straif, the blackthorn*

- **Status:** Not started. **Buildable now at draft.**
  - The final sand verdict is gated on C-V1 `level`, or on a 012 row added to `prickle_probe` showing the raw normal is level on the mirrored master (the 017 result suggests it is).

- **Concept:** The witches' tree. A black sloe, a round onyx cabochon, sits flush in the small cushion face. The parting line bristles with long straight blackthorn spurs down both shoulders. The walls are black-bark furrows starred with white blossom, because blackthorn flowers on bare black wood. Straif, the Ogham letter for blackthorn, is cut on the palm.

- **Theme face to palm:**
  - **Face:** the sloe; the rim polished.
  - **Shoulders:** seven spurs a side on the parting line, leaning alternately forward and back.
  - **Cheeks and shank walls:** black bark, with one or two blossoms per cheek.
  - **Palm crest:** Straif, bench-cut.
  - **Bore:** the stock's own.

- **Base:** Factory 012 Cushion 10 × 10 at native size, sand master (`probe::stock("012", true, None)`), Delft, bore 18.6. Its plan matches 001's to 0.016, and the batch-15 stock spike lists no rewrite for it.

- **Process:** Delft sand. The spurs lie in the parting plane, the sloe is a flush seat on the line, and the bark and blossoms are on the walls.

- **Stones:**
  - Black onyx **round** cabochon 7.0: `Gem::cabochon(GemCut::Round, 7.0)`, onyx tint. A sloe is round, which drops the pear dependency.
  - Four white diamonds 1.3 as blossom centres, `Gem::calibrated(Round, 1.3)`.

- **Build, step by step:**
  1. **Stock and skin.** `Atlas::of(&d, 2048, 768)`, `Hide::of(&a)`, `hide.folds(&a, ..)`.
  2. **Sloe.** Caiman's emerald recipe (`ex/stock_masterworks.rs:1723-1740`):
     - `SeatPadLayer` at `Skin::on_face(0.0, 0.0)`, `style: SeatStyle::Boss`, `crown: 0.15`, `blend_mm: 0.4`, `metal_true`, `solid: Flush`, `through: true`, `fit_stone(Gem::cabochon(Round, 7.0))`, `height_mm` 0.9, `mark_mm` 1.0.
     - On a 10 mm face this leaves about 1.1 mm of rim either side: the face *is* the sloe.
     - The brief's engraved twig round the stone is dropped; there is no room.
  3. **Long spur.** `Operation::Extrude { sketch: Sketch::circle(0.5).into(), height_mm: 2.6, draft_deg: 7.0 }`, tip Ø ≈ 0.36 (`core/cad.rs:59-65`, positive draft narrows the far end). Component: `Placement::Ring { theta_deg: <first station>, across_mm: 0.0, height_mm: -0.4, spin_deg: 0.0, tilt_deg: 38.0, cant_deg: 0.0 }` (+ `level: true` with C-V1), Join, Cast, `blend_mm` 0.3.
  4. **Short spur.** `Extrude(circle 0.5, 1.8, draft 8)` at `tilt_deg` −30.
  5. **One shoulder.**
     - `Pattern { sources: [long, short], kind: Ring { count: 4, span_deg: 36 } }` for the long spurs. Motions are rigid, so the short spurs interleave with their own source set in a second Ring pattern (count 3, span 30), phased by half a step.
     - Stations start ≥ 1 mm past the fold where the parting line turns over the end wall. Read the fold from `hide.folds`; 012's head is small, so measure it.
  6. **Other shoulder.** `Pattern { sources: [both ring patterns], kind: Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } }` (`core/cad/pattern.rs:52-66`).
  7. **Bark.** Painted on the walls only (`a.cheek(s).max(a.shoulder(s))`), with furrows running round the ring and lenticel dashes. Relief 0.35, through `draft_clamp(&a, &mut alpha, 0.35)`; the bite must be ≤ 0.05 mm. Show it with `hide_layer`. Or use C-R4 and C-R1 if they have landed.
  8. **Blossoms.** For each cheek:
     - `outline::blossom(5, d)` with d = 3.0 target and 2.4 minimum, sized to the measured cheek with a 0.3 mm margin; `along_pull: true`, `height_mm` 0.4, `top: StampTop::Dome { crown_mm: 0.15 }`;
     - a diamond centre `SeatPadLayer` `GypsyMound`, `solid: SolidKind::Bead`, `mark_mm` 0.6.
     - If two blossoms do not fit with 0.5 mm between them, use one (and three stones in total).
  9. **Straif.** One bench cut stamp on the palm's parting line: a 6 mm stem stroke plus five diagonal strokes 1.6 × 0.35 at 45°, as one comb polygon. Raised, a diagonal stroke's ends fail the monotone rule.
  10. Build, gate, render, review, export and verify.

- **What it shows off:** straight CAD parts poured in sand; a multi-source ring array and a section mirror; a single dramatic stone filling the face; bark skin as supporting texture; bench-cut Ogham.

- **Traps and how they are avoided:**
  - **A spur whose axis tips off the plane undercuts.** Parts do not correct for the raw normal the way stamps do (`frame_on`, `core/cad.rs:463-502`). Use C-V1 `level`, or prove the master's normal level with a probe row; the 017 run pulled clean.
  - **The fold** (Caiman: 0.06 mm). No spur within 1 mm of it.
  - **Raised Ogham locks.** It is bench-cut.
  - **Bark on the crest** (melon lobes). Walls only.
  - **Tips.** Draft chosen so tips are ≥ 0.36 mm.
  - **Graded spurs.** Rigid motions cannot grade; two sources, or C-V2 `scale`.

- **Needs:**
  - C-V1 `level`, or the probe row.
  - C-R1 and C-R4 (optional, for a light template).
  - P7.

- **Template:**
  - Imported-base lift with an `/imported_base` patch until P7.
  - Five `layer.seat` nodes.
  - A chain of 5–6 `cad.feature` nodes (two spurs, two rings, one mirror).
  - Bark embedded unless procedural.
  - Stamps as a patch until P7.
  - Multi-source patterns make it format 6.

- **Risk:** 3/5, medium.
  - Fallback 1: one spur size (a single Ring plus Mirror).
  - Fallback 2: if spurs on the small head report obstructions, move them to the shank, where the stock is a plain crest.

---

## Datura — *the thorn-apple*

- **Status:** Not started. Buildable now for a fixed size. It is **blocked on C-V1 `Relative`** for a template whose spines follow the capsule on resize.

- **Concept:** The devil's thorn-apple, a thorned nightshade. A spined seed capsule stands on the badge face as on a calyx. It is split into four gaping valves, with black seeds spilling in the splits. Sinuate datura leaves climb the cheeks, and the palm is the smooth stem. This is Logan's heaviest, most sculptural register, in plant form.

- **Theme face to palm:**
  - **Head:** the capsule on the badge calyx.
  - **Cheeks and shoulders:** two leaves a side with midribs and veins.
  - **Palm:** the polished stem.
  - **Bore:** the stock's own.

- **Base:** Factory 011 Badge 18 × 20 at native size, not mirrored (`sand_envelope = false`), with the lost-wax recipe as `base()` sets non-sand stock. Its sinuate outline reads as a calyx. Fallback base: 018 Butterfly, which is also wax-only.

- **Process:** Lost wax. Spines radiate from a dome; in two-part sand only about a quarter of them would release.

- **Stones:** Eight black spinel round 1.5, the seeds.

- **Build, step by step:**
  1. **Stock.** As above, plus the `Skin` helpers.
  2. **Capsule.**
     - `Operation::Revolve` of a half-egg `Geometry::Bezier` profile (base radius 4.2, height 5.6) on `Workplane { x: [1,0,0], y: [0,0,1] }`, with `pivot [0,0,0]`, `axis [0,0,1]`, `degrees 360`.
     - `Placement::Ring { theta_deg: 90, across_mm: 0, height_mm: -0.6 }`, Join, Cast, `blend_mm` 0.5. The bead flares the capsule into the face.
  3. **Valves.**
     - One `Operation::Extrude` of a cross-shaped slot sketch (two 0.9 mm slots, 9 mm long), placed over the capsule top, `height_mm` −3.4, `draft_deg` 6, so the slot narrows downward and the valves gape.
     - `Attach::Cut`. Cuts resolve after joins, so it carves the joined capsule and stops 2.4 mm above the table.
     - Never an `Operation::Boolean`: the kernel refuses faceted operands over 500 faces.
  4. **Spines.** Four rows at latitudes 18°, 38°, 58° and 76° on the capsule.
     - Each spine is `Extrude { circle 0.42, height 1.5 → 1.1 by row, draft 9 }`, tip Ø ≈ 0.36.
     - Each row source is placed `Placement::Relative { part: capsule, at: <point on the egg at that latitude>, rotation_deg: <normal> }` (C-V1). Until then, use `Placement::Free` from the capsule's seated frame (breaks on resize).
     - Each row is `Pattern { kind: PatternKind::About { part: capsule, count: 12 | 12 | 10 | 6, span_deg: 360 } }`, phased 15° so no spine lands in a slot. That makes 40 spines.
  5. **Seeds.** Eight `stone_feature(Gem::calibrated(Round, 1.5))`, two per slot arm, seated `Relative` to the capsule (or `Free`) on the slot walls, each with `seat.bur` (Cut, Cast).
  6. **Leaves.** For each cheek, two stamps:
     - `outline::leaf(Margin::Serrate { teeth: 3, depth_mm: 0.9, lean_deg: 30.0 }, 7.0, 4.2)`, `along_pull: true`, `height_mm` 0.5, `top: StampTop::Gable { rise_mm: 0.15, axis_deg: 0.0 }`;
     - a tier-1 raised midrib `outline::lanceolate(6.0, 0.45, 0.1)` at `height_mm` 0.15;
     - tier-1 cast vein cuts, `sink_mm` 0.12, at the wax floor of 0.15.
  7. Build, gate, render, review, export and verify.

- **What it shows off:** `Revolve`; `About` arrays round a part; a cut part through a joined part; seeds set by the bur; CAD sculpture on factory stock; relative placement (C-V1).

- **Traps and how they are avoided:**
  - **Kernel booleans on freeform bodies:** the valves are a Cut part resolved by csg.
  - **Weight.** The capsule is about 207 mm³, roughly 3.2 g in 18k. It stays solid, because `Shell` supports only boxes, cylinders and spheres.
  - **Wear.** Tips ≥ 0.3; no spines on the palm.
  - **Resize.** C-V1 `Relative`, or accept a fixed size.

- **Needs:** C-V1 `Relative`; P7.

- **Template:**
  - Imported-base lift plus a chain of about 30 `cad.feature` nodes.
  - Exposed: capsule height and diameter, spine length, seed size.
  - No painted alpha; under 1 MB after P7.

- **Risk:** 4/5, medium–high.
  - Fallback: two spine rows, not four, and seeds on the capsule's crown instead of in the slot walls.

---

## 5. Packaging (the Reptilia format)

The lead lane does this. It owns the registration files: `graph/templates.rs`, `wb/templates.rs`, `showcase/vepres/README.md` and the sheet.

**Per-ring folder** `showcase/vepres/<slug>/`:
- `design.ring.json` (written by `library::save_design_embedded`), `editable-graph.ring.json`, and `artwork/` (outline SVGs as text, plus any painted PNG).
- `finished-metal.stl`.
- The pattern:
  - sand: `casting-pattern.stl`, shrink-compensated by `metal::pattern_scale`, with no bench cuts or seats and with drill dots (`mesh::try_build_pattern`);
  - wax: the design is its own pattern.
- `reference-<stone>.stl` per stone kind.
- `report.json`, `mesh.json`, `release-fine.json` (sand), and `verification.json`. The verification file records clamp bite, drag, template bytes, open milliseconds from `graph/../examples/template_open_probe.rs`, patch pointers and node count, as Arachne's does.
- `hero.png`, `face.png`, `palm.png`, Blender `studio*.png`, and the reel.

**Collection level:**
- `showcase/vepres/README.md`: a table of the eight rings (form, surface, approximate cast 18k grams), the process per ring, what is bench work, and the reproduce commands.
- `index.html`.
- `Vepres-collection.png`.
- `Vepres-final-renders.zip`.

**Reproduce commands (README):**

```sh
for r in rubus sentis rosa_mortua hedera ilex viscum prunus datura; do
  systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example vepres_$r -- showcase/vepres/${r//_/-} --verify
done
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example collection_templates -- vepres   # P8; until then copy reptilia_templates.rs to vepres_templates.rs
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- showcase/vepres   # P8; until then adapt tools/render_reptilia.py
python3 tools/catalog_collection.py --collection vepres showcase/vepres   # P8; until then adapt tools/catalog_reptilia.py
```

**Templates:**
- `graphs/templates/<slug>.graph.json` is bundled automatically, because the directory is a family (`crates/ringdesign-assets/build.rs:29`).
- Add `pub static VEPRES: &[TemplateGraph]` in `graph/templates.rs` beside `REPTILIA` (`:163`), and chain it into `catalog()` (`:170`).
- Add `group("Vepres collection", &["rubus-vepres", "sentis-vepres", "rosa-mortua-vepres", "hedera-vepres", "ilex-vepres", "viscum-vepres", "prunus-vepres", "datura-vepres"], "Thorned botanicals: CAD thorns, vines and struck leaves; sand where the thorn lies in the parting plane.")` in `wb/templates.rs` after the starter groups (`:479-495`).
- 160 px thumbnails come from `render::finished` through `tools/template_thumbnails.py`, into `crates/ringdesign-workbench/assets/templates/<slug>.png`, with a `preview_bytes` arm (`wb/templates.rs:501`) or P8's `thumbnails!` macro.
- Update the preview test count (`every_authored_graph_and_collection_has_a_real_preview`, `:574`).

**Sheet:**
- Subtitle "FOUR SIGNETS · FOUR VINES · FIFTY-TWO STONES", with the footnote above.
- Use studio gold throughout, and draw each stone in its own tint through P8's stone material manifest.

**Phone:**
- Bump `crates/ringdesigner-android` `version` and add the `CHANGELOG.md` entry.
- Run `cargo test -p ringdesigner_android` and `cargo ndk -t arm64-v8a check -p ringdesigner_android`.
- Open all eight templates on the **rdsmoke** AVD (not the shared s26ultra), then publish with `publish-appstore.sh`.

**Reels:** record from rdsmoke with the known `adb shell screenrecord` recipe. Stamp families ("Leaflet", "Holly leaf", "Spur") and layer names ("Oak bark", "Runner") are the captions, so name them for the reader.
