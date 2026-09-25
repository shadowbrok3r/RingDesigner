# Tenebrae — the Gothic cathedral collection

*The office of shadows, in which the candles go out one by one.*
Sheet subtitle: `FOUR SIGNETS · TWO BANDS · A SOLITAIRE · A RELIQUARY · TWENTY-SIX STONES OF GLASS`

**Primary side of the app: CAD modelling.** The collection shows sketches with regions on work planes and part faces, extrude and cut with draft, press-pull, revolve, loft, the twisted sweep, ring/About/mirror patterns of several parts, the builders (`head.basket`, `head.bezel`, `seat.bur`, `cutter.pierce`, `cutter.azure`, `shank.cathedral`), stones on part faces, Separate parts with a joint, bench stages and seam beads. Every ring is one building, and the stones are its stained glass: calibrated coloured stones in collets and baskets, most set *à jour* so light passes through them.

This file is self-contained. It was written on 2026-09-24 against `master` at `30f0506`. Where the source brief (`source/brief-gothic.md`) and the director's plan (`source/plan.md`) disagree, the plan and Logan's answers win, and this file already applies them. Every API name below was checked against that commit; `file:line` references are relative to `crates/ringdesign-core/src/` unless another crate is named. Anything not verified is marked **(unverified)**.

## The eight rings

| # | Ring | Epithet | Base | Process | Stones | Status |
|---|---|---|---|---|---|---|
| 1 | Oculus | the wheel | Procedural Flat 7.0 × 4.6, Uniform | Delft sand | 0 | Not started; buildable now |
| 2 | Ogiva | the keel | CAD-only revolved pointed-arch section | Delft sand | 0 | Not started; buildable now |
| 3 | Rosa | the west rose | Factory 013 Round at a 16 mm face (fallback 001) | Lost wax | 9 | Not started; buildable now with oval lights; full form waits on C-B2, C-T1, C-T3 |
| 4 | Sigillum | the chapter seal | Factory 005 Rosette, sand master | Delft sand + bench | 0 | Not started; blocked on C-T6 |
| 5 | Porta | the portal | Factory 009 Drop, unmirrored | Lost wax | 1 | Not started; buildable now in interim form; full form waits on C-V2, C-T3, C-T7 |
| 6 | Arcus | the flying buttress | Procedural Flat 3.2 × 2.3, Cathedral 0.8 | Lost wax | 1 | Not started; buildable now; one-number Spread waits on P7 |
| 7 | Capsa | the reliquary | Procedural Flat 4.6 × 2.3, Cathedral 0.5 | Lost wax, two castings | 6 cabochons | Not started; buildable now (native only) |
| 8 | Lanterna | the octagon lantern | Factory 015 Octagon 16 × 16 | Lost wax | 9 | Not started; built last |

Three sand rings, five lost-wax rings, 26 stones (9 + 1 + 1 + 6 + 9). Every sand ring is Delft clay; Petrobond belongs to Moloch and Ouroborus only. Nominal bore is 18.6 mm (r = 9.3) throughout. Final renders are studio gold throughout (Logan, 2026-09-24).

## Where this collection sits in the whole plan

- Tenebrae is **batch 5** of plan section 4: after the Bestiarium (batches 2–3, with a stop for Logan's review at the end of batch 3) and Cataphracta (batch 4), and before Vepres. Nothing here starts before that review unless Logan reorders.
- The Bestiarium's Draco and Arachne are being authored on the branches `bestiarium-draco` and `bestiarium-arachne`; they are not part of this file.
- Global rules: at most **two lanes in flight**; one GitHub issue, branch and PR per lane; a lane merges when `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --workspace --offline` and the wasm check (`cargo check --no-default-features --target wasm32-unknown-unknown` for core and the configurator) pass. The lead alone edits registration files (`ringdesign-graph/src/templates.rs`, `ringdesign-workbench/src/templates.rs`, `showcase/tenebrae/README.md`, the sheet).

## Platform status (the IDs every ring's Needs line uses)

| ID | What | State on 2026-09-24 | What Tenebrae uses it for |
|---|---|---|---|
| P1 | Async template open with staged progress, `Arc<Alpha>` library, script engine on open | **Landed** (batch 14, hardened in batch 15). `workbench::templates::Template::open` evaluates with `Evaluator::with_exprs(ringdesign_script::engine())` (`ringdesign-workbench/src/templates.rs:114`) | Every Tenebrae template is 20–40 CAD features plus csg resolves; the clusters may use expression pins |
| P2 | CAD stones are stones: `StoneSource::Cad` (`setstone.rs:23`), multi-source `Pattern` (`cad/pattern.rs:475`, `Sources(pub Vec<Id>)`), stone `tint` (`cad/builders.rs:269`), `render::finished` (`render.rs:97`) | **Merged on master** (batch 15, merge `d3dfa34`) | Rosa's and Lanterna's arrays of stone + head + bur; every stone in the report, sheet, stone map, thumbnails; the glass colours |
| P3 | `skin.rs` (`Atlas::of` `:52`, `Hide`, `crest_at`), `imported_base::sand_master` (`imported_base/sand_master.rs:54`) | **Merged** (batch 15, merge `537451a`) | Sigillum's sand master; locating a stock cheek's `v` for wall stamps |
| P4 | Stamps v2: `Stamp::tier`, `StampTop` (`setting.rs:939`), `outline.rs` families, `Stamp::parting_monotone` (`setting.rs:2149`), `hull_and_bays` (`:1490`), `stamp_row` (`:1581`) | **Merged** (batch 15, merge `5fd9390`) | Wall arcades on stock; `parting_monotone` checks Sigillum's cast stamps |
| P5 | Station-aware side-face gates | Batch 16, not started | Only cosmetic here (Arcus's nave arcade on a Cathedral swell, in wax) |
| P6 | Claw styles (`style`, `grouping`, `tip: Dome\|Point`) | Batch 16, not started | Optional pointed claw tips on Arcus |
| P7 | Template nodes and lift: `base.preset`, stamp nodes, `cad.feature` numeric and placement pins | Batch 16, not started | Every template's controls and size; removes the ~3 MB `/imported_base` patch from the four stock templates |
| P8 | Collection tooling: `tools/render_collection.py`, `tools/catalog_collection.py`, `ringdesign-graph/examples/collection_templates.rs`, stone manifest for Blender, `thumbnails!` | Batch 16, not started | Packaging |
| — | Starter gallery | Batch 16, not started | Nothing directly (the Cathedral solitaire starter is Arcus's plain cousin) |
| C-B2 | True stone plans (pear, trillion, heart, half-moon) | Batch 3B, not started | Rosa's pear lights (ovals until then) |
| C-V2 | `PatternKind::Along` / `Line` | **Lands in this batch** (Porta's lane owns it; Vepres reuses it) | Porta's crockets; optional for Capsa |
| C-T1..T7 | Tenebrae's own enablers | Land inside batch 5 | Below |

The brief's enabler numbers map as follows: E1 → P1, E2 → P2, E8 → P2 (tint), E5 → P7, E13 → P7, E14 → P6, E6 → C-V2, E3/E3b → C-T1, E4 → C-T2, E7/E15 → C-T3, E11 → C-T4, E12 → C-T5, E10 → C-T6, E16 → C-T7.

## Build order inside the collection

Following plan section 4, batch 5:

1. **Enablers, in pairs.** (C-T1 tracery and `Profile::Regions`) with (C-T6 Textura and text-to-sketch), then (C-T3 cutter shapes and the outline and artwork library) with (C-V2 `Along`/`Line`), then C-T4, C-T5 and C-T7. The clusters (C-T2) are owned by their own lane and land before the templates are packaged.
2. **Oculus and Ogiva.** They need no enabler, both prove the along-the-pull rule in Delft sand, and they give the sheet a fast first row. Under the two-lane cap the lead may start them beside the first enabler pair.
3. **Rosa.** Exercises C-T1 and P2's About array of stone + collet + bur; its base resize is already spiked.
4. **Sigillum.** Needs C-T6 for its legend and C-T5 for its bench marks.
5. **Porta.** Its lane owns C-V2, and it needs C-T3's lancet cutter and fleur-de-lis.
6. **Arcus.** Every builder it uses exists; it waits only on artwork from C-T3.
7. **Capsa.** Native shell, press-pull, stones on part faces and a Separate lid. Artwork from C-T3.
8. **Lanterna.** The most complex feature tree in the library (5/5). It reuses everything above, so it goes last.

## The collection's enablers

Sizes: S is under one agent-day, M is one to three days, L is more than three.

### C-T1 — Tracery from a net, and `Profile::Regions` (S–M)

Files: `sketch/edit.rs`, `cad.rs` (`Profile`, `regions_of` at `:1655`), `library.rs` (format fence).

```rust
// sketch/edit.rs
pub struct Tracery {
    /// Each light's new closed loop, in the order the net's cells were found.
    pub lights: Vec<Vec<Id>>,
    /// Cells whose inward offset would fold, with the reason; left out, never half-made.
    pub skipped: Vec<(RegionRef, String)>,
}
impl Sketch {
    /// Split the net at its crossings, offset every closed cell inward by bar_mm / 2, mark the net construction.
    pub fn tracery(&mut self, net: &[Id], bar_mm: f64) -> Result<Tracery>;
}
// cad.rs: a new untagged arm, placed before `Feature` (untagged serde tries arms in order).
pub enum Profile {
    Region { feature: Id, region: RegionRef },
    Regions { feature: Id, regions: Vec<RegionRef> },
    Feature { feature: Id },
    Inline(Sketch),
}
```

- It composes existing calls: `split_at_intersections` (`sketch/edit.rs:192`), then `profile_regions` (`sketch/region.rs:138`; a branched sketch returns its cells), then `offset(cell, −bar/2)` (`sketch/edit.rs:433`, which refuses a fold), then marking the net `construction`.
- **Fence `Regions`.** An older build reads `{feature, regions}` as `Profile::Feature`, because untagged serde ignores the unknown key, and would sweep every region. Put it behind a format step in `library::format_version_for`, as `Revolve { in_plane }` already is.
- `Regions` is needed only when one branched sketch carries several depths. Keeping one Sketch feature per depth avoids it. Sigillum and Porta both work today that way.
- Test: a 24-cell polar net at bar 0.9 gives 24 loops, and each loop's gap to its neighbour is 0.9 ± 1e-6.

### C-T2 — The Gothic clusters (M)

Files: `graphs/clusters/*.cluster.json`, `ringdesign-graph/src/templates.rs` (`BUNDLED_CLUSTERS`, `:226`), a cluster test. Each cluster wraps a `script` node (`ringdesign-script/src/node.rs:20`). The script declares typed pins in its header, computes the light loops **analytically** (so it needs neither split nor offset), and emits the **Sketch feature's operation** as `Json`. That JSON wires into a `cad.feature` node's `operation` pin (`ringdesign-graph/src/nodes/cad.rs:24-46`). The downstream Extrude keeps its own params and names the sketch by node id; on a driven design, feature ids are node ids.

| Cluster | Inputs (exposed) | Outputs | Used by |
|---|---|---|---|
| `wheel-window` | `size` (US, via `band.size` → `bore_radius_mm`), `lights` (Int, 24), `bar_mm` 0.9, `inner_rail_mm` 1.0, `outer_r_mm` 12.4, `head_drop_mm` 0.35 | `sketch_op: Json`, `lights: Int` (feeds the flute count) | Oculus |
| `pointed-arch-section` | `size`, `width_mm` 5.6, `thickness_mm` 3.6, `keel` 0..1, `fillet_mm` 0.3, `comfort_mm` 0.12 | `sketch_op: Json` (on `Workplane::section()`) | Ogiva |
| `rose-tracery` | `lights` (Int, 8), `r_in`, `r_out`, `bar_mm`, `head` ("trefoil" / "pointed" / "mouchette") | `sketch_op: Json`, `count: Int` (for the About pattern via `json.set`) | Rosa, Lanterna's windows |
| `lancet-arcade` | `bays`, `width_mm`, `height_mm`, `bar_mm`, `cusps` | `sketch_op: Json`, `outline: List<Number>` (for P7 stamp nodes later) | Rosa, Sigillum, Porta and Lanterna wall arcades; Capsa's bays |

```text
// in: bore_r: Number = 9.3, lights: Int = 24, bar: Number = 0.9, rail: Number = 1.0, outer: Number = 12.4, drop: Number = 0.35
// out: sketch_op: Json, count: Int
let r0 = bore_r + rail;            // inner circle
... one closed loop per light: inner arc, two radial sides bar/2 off the mullions, two head arcs meeting `drop` under `outer`
let sketch_op = #{ "Sketch": #{ "sketch": #{ "points": pts, "entities": ents, "plane": plane } } };
let count = lights;
```

- The rhai engine caps are 200,000 operations, map size 1024 and strings of 64 KB (`ringdesign-script/src/lib.rs:24-57`). 24 lights of about 6 points and 6 entities each fit. If a cluster ever hits a cap, the fallback is a native `sketch.gothic.*` node in `ringdesign-graph/src/nodes/`.
- The Json must be the exact serde shape of `sketch::Sketch`. The test deserializes every cluster output into `Sketch` and holds each to the example's own feature byte for byte.
- `band.size` gives `bore_radius_mm` (`ringdesign-graph/src/nodes/band.rs:129-135`), so a resize moves the architecture. That is the fix for Oculus's and Ogiva's absolute radii.
- Clusters that use only script nodes need no expression engine. Any expression pin needs P1's `with_exprs`, which is already on the open path.

### C-T3 — Gothic cutter shapes, the harvested outline library and the artwork set (M)

Files: `cad/builders/cutters.rs` (`Shape` `:101`, `PIERCE_SHAPES` `:16`), `tools/harvest/`, `crates/ringdesign-assets/build.rs` (a new family), `library.rs` (listing).

```rust
pub enum Shape { Round, Oval, Marquise, Heart, Drop, Lancet, Ogee, Trefoil, Quatrefoil, Mouchette }
pub const PIERCE_SHAPES: &[&str] = &["Round", "Oval", "Marquise", "Heart", "Drop", "Lancet", "Ogee", "Trefoil", "Quatrefoil", "Mouchette"];
// ringdesign-assets/build.rs FAMILIES:
Family { ident: "SKETCHES", dir: "bundled/sketches", suffix: ".svg" },
// library.rs, overlaid like list_outlines (library.rs:445): user dir over bundled, by name
pub fn list_sketches() -> Vec<(String, crate::sketch::Sketch)>;
```

- The bright cut must **inset** concave cusps instead of pushing along normals (`outline(.., g)`, `cutters.rs:147`), or a quatrefoil's cusps fold.
- Harvest from `assets/User/Profiles/`, all viewed and usable per the brief:
  - Under Gallery Cuts 001 (ogee), 002 (quatrefoil), 003 (cusped lozenge);
  - Jalis 000, 002, 010, 016 (tileable tracery nets, as centre lines for C-T1);
  - Ornaments 027 (quatrefoil ring) and 028 (fleur-de-lis, a B-rep).
- Write them to `bundled/sketches/gothic/*.svg` in `import_svg`-clean form (`sketch/exchange.rs:146`): `width`/`height` with units, a uniform viewBox, no `transform`, no S/T shorthand.
- Authored artwork, as SVG text in the same folder:
  - crocket leaf;
  - fleur-de-lis;
  - fleur cresting strip;
  - nave arcade tile (3 bays);
  - gargoyle **silhouette** (no eye, no mouth detail: Logan's no-faces rule);
  - crossed bones under an hourglass (Capsa's memento mori without a face);
  - a cross pattée (Sigillum's legend cross).

### C-T4 — DFM land width for CAD cuts (S–M)

Files: `dfm.rs`. It closes the tangential-bar half of "Two things the model does not yet know".

```rust
pub const CUT_LAND: &str = "cut land";
/// For every Cut part (and every copy a Pattern of one makes), the narrowest metal between two of its
/// regions, between its copies, and to the band's or its host part's edge, against `floor_mm`.
pub fn cut_lands(design: &RingDesign, built: &crate::mesh::BuildResult, floor_mm: f64) -> Vec<Finding>;
```

- Distances are measured in each profile's sketch plane between region outlines, with Pattern copies carried by their copy motions.
- A finding reads, for example: "Cut #4 'Pierce the lights': 0.62 mm between lights 3 and 4 (floor 0.8)".
- Until this lands, every author script asserts its lands from its own sketch numbers.

### C-T5 — Bench marks for engraved cuts (S)

Files: `castability/marks.rs`, `cad.rs` (`Component`, and its serde twin `ComponentWire`, `:285-380`).

- **Verify first.** Today a sand pattern stands a raised `LocatingMark` where a bench part meets the band ("`Cut` a drill mark", `castability/marks.rs:26-44`), and moves a dot the field blames onto the parting line (`on_parting_line`).
- Sigillum's three bench cuts are all centred on the table's centre, which lies on the parting line. If their marks already field clean there, C-T5 shrinks to a regression test.
- Otherwise, add an opt-out, because an engraving needs no drill-start dot:

```rust
pub struct Component { .., #[serde(default = "yes")] pub mark: bool }   // and the same field on ComponentWire
```

### C-T6 — Textura and text as sketch (M)

Files: `text.rs` (`TextFont`, `:25`), a new `sketch/text.rs`, `assets/fonts/` (plus `OFL.txt`), and a `sketch.text` graph node.

```rust
pub enum TextFont { Serif, Script, Textura }        // Textura = UnifrakturMaguntia, SIL OFL
pub struct TextArc { pub radius_mm: f64, pub start_deg: f64, pub clockwise: bool }
/// Glyph outlines as closed cubic loops; counters become holes by even-odd nesting.
pub fn text(font: TextFont, text: &str, cap_mm: f64, tracking: f64, arc: Option<TextArc>) -> Result<Sketch>;
```

- Outlines come from `ttf-parser` 0.25. It is already in `Cargo.lock` through fontdue; add it as a direct dependency. Map quadratic segments to `Geometry::Bezier { points: [Id; 4] }` (`sketch.rs:121`).
- One sketch holds at most `MAX_ITEMS` = 1024 points (`sketch.rs:30`). A long legend is split into one sketch per word.
- The same font sets the sheet's title.

### C-T7 — Sweep along a sketch curve (S)

Files: `cad.rs` (`Operation::Sweep`, `:77`; evaluation at `:1858`, 2–128 stations).

```rust
#[serde(untagged)]
pub enum SweepPath { Points(Vec<[f64; 3]>), Sketch { feature: Id, entity: Id, lift_mm: f64 } }
```

- The path samples `entity` every 0.3 mm (at most 128 stations) in the sketch's plane, lifted by `lift_mm` along its normal. A moulding then follows an edit to the arch it runs along.
- Old files keep reading as `Points`. Fence the new form with a format step.
- Parts modelled at the origin and seated already follow a **resize** (house rule 2). C-T7 is for **parametric edits**.

### C-V2 — Patterns along a line and along a curve (M–L; owned here by Porta's lane)

Files: `cad/pattern.rs` (`PatternKind`, `:37`).

```rust
pub enum PatternKind {
    Ring { .. }, About { .. }, Mirror { .. },
    Line  { along: [f64; 3], count: u32, pitch_mm: f64 },
    Along { path: Id, entity: Id, count: u32, pitch_mm: Option<f64>, alternate: bool, roll_deg: f64, scale: [f64; 2] },
}
```

- The copy motions carry scale. A scaled copy of a stone scales its gem's `w_mm`/`l_mm` with it; if that is not wanted, refuse scale on stone sources by name.
- Older builds refuse an unknown variant loudly. Bump the format anyway, so the message names the version.
- Vepres reuses this enabler (Hedera, Viscum and graded spurs).

## House rules for all eight

1. **The CAD form of the side-face guarantee.** Draw on the parting plane or on a side face, extrude along the pull, and draft each half from z = 0: the result casts by construction. A sketch extruded along a surface normal off the crown locks and belongs to the bench.
   - Side-face piercing measured 0.000 mm² and staged Cast; crown piercing locked 8.63 mm² at −88° (`PLAN.md`).
   - `cutters::cut_stage` stages a cut Cast within `ALONG_PULL_DEG` = 10° of the pull (`cad/builders/cutters.rs:24`, `:1067`).
2. **Model at the origin and seat once.** Build every part in the part frame and seat it with `Placement::Ring` (`cad.rs:392`), which follows a resize. The part frame has x along the finger, y round the ring and z out of the metal.
   - A `Box` and a `Cylinder` are **centred on their origin** (`cad.rs:1726`). Seat one with `height_mm = half its z − the sink`: a 2.4 mm pier sunk 0.3 takes `height_mm: 0.9`, not −0.3.
   - A `Tangent` work plane has **x round the ring and y along the finger** (`cad/pattern.rs:73-78`), which is the part frame turned 90° about z. Swap coordinates, or use `spin_deg: 90`.
3. **Keep a part a kernel body while you still need its faces.** Sketch-on-face, `FaceSeat` stones, press-pull, native `Fillet`/`Chamfer` and `Shell` refuse meshes (`cad.rs:1588-1606`). Builders, patterns, the twisted sweep, csg booleans and stored meshes all produce meshes. Do face work first and mesh operations last, or leave the union to the band join: Join parts are clustered into one tool (`parts.rs:17`).
   - A sketch on a work plane uses `on_face: Some(FaceAnchor { feature: <plane id>, face: FaceRef::bare(0) })`. A work plane holds a frame and no body (`cad.rs:1603-1606`).
4. **Stained glass is set *à jour*.** Every faceted stone gets `seat.bur { "through": true }`, a pilot to open air past the bore (`cad/builders.rs:163`). Colours go in the stone's `tint` param (linear RGB, `cad/builders.rs:269`). Suggested tints:

   | Stone | Tint |
   |---|---|
   | Chartres-blue sapphire | `[0.02, 0.06, 0.45]` |
   | Ruby | `[0.45, 0.01, 0.04]` |
   | Amethyst | `[0.22, 0.05, 0.38]` |
   | Citrine | `[0.75, 0.42, 0.03]` |
   | Garnet | `[0.30, 0.02, 0.03]` |
5. **The author measures bars; the verdict does not.** Tracery bars are at least 0.8 mm. That is Delft's `min_section_mm` (`castability.rs:180`) and the investment recipe `stock_masterworks` uses (`examples/stock_masterworks.rs:379-385`).
   - `CastProcess::LostWax.apply` writes 0.5 (`castability.rs:117`). Raise it to 0.8 after applying it.
   - The field verdict is radial and cannot see a tangential bar or an axial web. Every author script asserts its lands until C-T4 exists.
6. **Split before regions.** Curves that cross or touch are refused (`sketch/region.rs:133-137`). Draw overlapping, run `split_at_intersections`, and let T-junctions close the cells. `Profile::Feature` sweeps only unbranched loops (`cad.rs:1657`); take cells with `Profile::Region`, or C-T1's `Regions`.
7. **No coincident faces.**
   - Through-cuts overshoot the far face by 0.3 mm.
   - Drafted halves overlap 0.05 mm across z = 0.
   - `CUT_CLEAR_MM` (0.05, `cad.rs:2034`) already lifts a cut's entry side.
8. **Native only.** OpenCascade has no Windows build, and a `Stored` mesh forces format 6 and goes stale on upstream edits. No Tenebrae ring uses it (plan verdict on Capsa).
9. **The feature tree is the lesson.** Feature names are sentences ("Pierce the eight spandrel lights to the bore"). The CAD workspace's rollback (`Document::through`, `cad.rs:1297`) steps through them.
10. **No flat, blocky CAD** (Logan's complaint about the Workshop set).
    - Every joined part carries a seam bead (`blend_mm` 0.1–0.35).
    - Piers and plinths get a chamfer or a moulding.
    - A render that reads as stacked boxes fails the review round.
11. **No faces and no eyes** (Logan). A gargoyle is a silhouette with no eye; Capsa's memento mori is bones and an hourglass, not a skull.

## How every ring is judged (plan section 5, applied)

Iterate each ring for at most three review rounds. A ring still failing after round 3 is cut or deferred, never shipped weak.

1. **Draft build at 768 × 320, then automatic gates.** A red gate returns the ring to its author without spending a review round.
   - **Geometry:** watertight; 0 degenerate faces; `csg::self_crossings` (`csg.rs:1090`) 0 on every made part; `built.solids.notes` empty.
   - **Sand rings:** `attributed_field_report(…, 256, 128)` reads **Castable**, not "with care". The release study (`manufacturing::inspect`, `manufacturing/mod.rs:568`) shows 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm. `dfm::findings_in` returns 0 findings.
   - **Wax rings:** fill ≥ 0.8 section. The author asserts the CAD lands (C-T4 later) and calls `cad::measure::thickness(&mesh, 0.8)` (`cad/measure.rs:50`) on the built mesh, because the field cannot see walls under CAD cuts.
   - **Stones:** the P2 stone record's count equals the gem preview's; the crowding census is clean or explained.
2. **Render set in studio gold with stones set:** hero, face, palm, side, a stone close-up, bare stock against finished; `section.png` for Oculus and Ogiva.
3. **Review round.** A fresh reviewer scores the renders against Caiman's hero, the Reptilia sheet and Logan's ZBrush sheet (`b15/zbrush_top.png`):
   - one theme face to palm;
   - figurative motifs as stamps with true outlines;
   - no faces or eyes;
   - stones in made settings and visible;
   - shoulder ornament not cut off before the face;
   - custom heads ≥ 13 mm;
   - no flat blocky CAD;
   - factory stock keeps its hard wall-to-face angles;
   - a silhouette distinct from the other seven;
   - density and legibility at least Caiman's.
4. **Export build** at 1536 × 448 with `--verify`: a cold reload with an empty library gives identical vertices.
5. **Template gate.** The template evaluates cold to the same design byte for byte, with the same mesh, using at most 4 `design.set` patches. Record its size and per-phase open time in `verification.json`. Budgets after P7: procedural ≤ 300 KB, stock ≤ 1 MB.
6. **Reel.** Name stamps by family ("Wall arcade, 2" plays as "Wall arcade"), and name features as sentences.

---

## Oculus — *the wheel*

- **Status:** Not started. Buildable now: every API exists. The bore-driven template waits on C-T2.
- **Concept:** Seen along the finger, a ring's side face is an annulus, which is the plan of a wheel window, and the finger is its oculus. Twenty-four lancet lights pierce the band from side face to side face along the pull, so light crosses the ring through the wheel. The crown carries the wheel's voussoirs.
- **Theme face to palm:**
  - **Side faces (both):** the wheel. 24 pointed lancet lights through the band, each half drafted 2° from the parting plane, with 24 trefoil-cusped blind spandrels between the heads.
  - **Crown:** 24 voussoir reeds across the crown on the mullions, and one milgrain row on the crest line.
  - **Shoulders and palm:** the same wheel all the way round, because the building is circular.
  - **Bore:** the oculus itself, plain, comfort 0.1.
- **Base:**
  - `ProfileStyle::Flat`, width 7.0, **thickness 4.6** (the brief had 4.0), `flatten_sides()`, `edge_round_mm` 0.25, `comfort_fit_mm` 0.1, `ShankKind::Uniform`, bore 18.6.
  - The brief's outer circle at r 12.4 on a 4.0 band leaves 0.3 mm of rail over each light at the band's edge, because the side-face run ends near 9.3 + 4.0 × 0.85 = 12.7. At 4.6 the run ends at 13.21, and the rail is 0.81, which clears Delft's 0.8 section.
- **Process:** Delft two-part sand (`SandProcess::DelftClay`: 3.0° draft, 0.8 section, 0.30 detail, `castability.rs:180`), Silver 925, `Recipe::sand`, `auto_parting = false`, `parting_mm = 0`. Everything is cut along the pull on the side faces, so it pours by construction. Silver ≈ 18 g.
- **Stones:** none.
- **Build, step by step:**
  1. `Operation::Band`, "Procedural shank".
  2. "High side face": `Operation::Plane { base: PlaneBase::Parting, offset_mm: 3.5 }`. Its origin sits on the finger axis, so every circle below is centred on the bore by construction.
  3. "Wheel": `Operation::Sketch` on plane 2.
     - **Construction net:** circles at r 10.3 (bore + 1.0: the plan's bore-side rail) and r 12.4, and one radial mullion.
     - **One light:** inner arc, two sides 0.45 off the mullions, and a pointed head of two arcs meeting 0.35 under r 12.4.
     - **Array it:** `Sketch::pattern(ids, &Pattern::Polar { centre: [0.0, 0.0], count: 24, sweep_deg: 360.0 })` (`sketch/edit.rs:755`).
     - **Tree lesson:** draw the net, `split_at_intersections()`, `offset(cell, −0.45)` per cell, and mark the net construction. C-T1's `tracery(net, 0.9)` does this in one call.
     - **Robust route:** draw each light's closed loop directly, as the `wheel-window` cluster does.
     - **Widths:** the light is 1.8 wide at r 10.3 and 2.35 at r 12.4, and 2.1 tall.
  4. "Cut the lights to the parting plane": `Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: −3.55, draft_deg: 2.0 }`, `Attach::Cut`, `Stage::Cast`. Positive draft narrows the far end, so each window narrows toward z = 0 and stops 0.05 past it.
  5. "The drag half": `Operation::Pattern { sources: Sources(vec![4]), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }`, `Attach::Cut`.
  6. "Cusps": a second sketch on plane 2 with 24 trefoil-cusped triangles in the spandrels between the heads, each at least 0.8 from every light.
  7. `Extrude −0.30, draft 5°`, Cut.
  8. Mirror across `Band`, Cut.
  9. **Field layer "Voussoirs":** `FlutesLayer { count: 24, profile: Round, width_mm: 1.3, height_mm: 0.2, lean: 0.0, along: false }` (`field.rs:1846`), gated to the crown with `Window { enabled: false, v_gate: VGate::Band { center_mm: band_v_len/2, span_mm: <crown arc>, fade_mm: <across the edge round> }, .. }`.
     - The flutes must not reach the side face where the wheel is cut, or they leave a skin over the lights.
     - Rotate the wheel sketch so the mullions land on flute centres. Read the flute cell's origin in `FlutesLayer::height` first **(unverified which u the first cell centres on)**.
  10. **Field layer "Crest beads":** `MilgrainLayer { v_mm: band_v_len/2, bead_diameter_mm: 0.5, beads_around: 96, height_mm: 0.2, mirror: false }` (`field.rs:2207`).
- **What it shows off:** a parting-plane work plane with an offset; a polar sketch pattern about the finger axis; cuts drafted from z = 0 and mirrored across the band; CAD and the height field in one ring; the sand verdict on CAD cuts (`castability::judge_parts`); a section view of a pierced band.
- **Traps and how they are avoided:**
  - **Perfect draft exists only on the side-face run** (`FieldContext::side_faces(80°)`; doctrine "Side faces are where ornament goes"). Its width is thickness − crown. Flat spends 15% of the thickness on the crown, and the lights stay inside the run.
  - **Rails.** The bore rail is 1.0 (plan). The outer rail over the lights at the side-face corner is 0.81 (the base fix above). The author asserts both.
  - **Sand slots.**
    - Minimum `min_sand_web_mm` is 0.6 (`manufacturing/mod.rs:58`).
    - A 1.8 mm mouth narrows to about 1.56 at z = 0 under 2° over 3.5 mm.
    - Each half-window is a 3.5 mm pin of sand hanging from its own mould half (aspect about 2.3). That is the **named physical-trial item** in the README (plan).
  - **Exit faces:** each half stops 0.05 past z = 0 and the halves overlap (house rule 7).
  - **Reeds stay straight.** Lean 0 measures 0.000%; lean 0.2 already gives 0.37% (`field.rs:1856-1866`). The shank stays `Uniform`, because relief whose walls face round the ring leans wherever the section changes width (doctrine "Two masterworks").
  - **Beads ride the crest line only** (doctrine "The showcase is the measured tour").
  - **A v-gate's fade is a wall facing the crest** (doctrine "Two masterworks"). Let the flutes' fade run over the edge round, then confirm 0.000%.
  - **Axial web.** The blind cusps leave 7.0 − 0.6 = 6.4 mm of web. The verdict cannot see an axial web, so the README states the number.
  - **Resize.** The wheel's radii are absolute. The `wheel-window` cluster reads `band.size`.
- **Needs:**
  - Nothing new to build it.
  - C-T2 `wheel-window` for the template.
  - C-T1 to teach the net route.
  - C-T4 later; the author asserts lands until then.
  - Optionally a one-line `FlutesLayer::phase_deg` (`#[serde(default)] pub phase_deg: f64`, added to u before the cell lookup) to lock reeds to mullions without rotating the sketch.
  - The example carries:

```rust
fn wheel(bore_r: f64, lights: u32, bar: f64, rail: f64, outer: f64, drop: f64) -> Sketch   // closed light loops
fn assert_lands(s: &Sketch, bar: f64) -> Result<()>                                        // light to light and to both rails
```

- **Template:** The lift (`ringdesign-graph/src/lift.rs:317`, `lift::from_design`) gives profile and shank nodes, 2 layer entries and 8 `cad.feature` nodes (`nodes::cad::chain_document`). Expect 60–150 KB, with no `design.set` patch beyond perhaps the draft settings.
  - **Controls:** Lights, Bar, Width, Thickness. Lights feeds both the cluster and `FlutesLayer::count`, so reeds and mullions stay aligned.
  - It lifts cleanly.
- **Risk:** 2/5, low. The open item is the sand-core trial. If the release study reports unresolved rays in the windows, fall back to blind lights: a 1.5 mm pocket from each side, 4.0 of web. That gives up the see-through and keeps the wheel.

## Ogiva — *the keel*

- **Status:** Not started. Buildable now. The resizable template waits on C-T2.
- **Concept:** The ring *is* a pointed arch: its cross-section is a blunt lancet, 5.6 wide and 3.6 thick, standing on the bore. Thirteen crockets climb its keel over the top of the hand, and thirty-six blind lancet niches are cut into each foot. Every feature is an extrusion along the pull, so it pours in two-part sand. It is the library's first parts-only sand ring.
- **Theme face to palm:**
  - **Crest:** the keel, a 155° crease on the parting plane, with 13 crockets over the top 144°.
  - **Flanks and feet:** 36 blind lancet niches each side, cut along the pull into the near-vertical feet.
  - **Palm:** the keel only, with no crockets to snag.
  - **Bore:** a comfort arc whose tightest point is the nominal 9.3.
- **Base:** CAD-only, with **no `Band` feature**, so the document is the whole ring (`mesh.rs:397-404`). Bore 18.6.
- **Process:** Delft two-part sand, Silver 925, about 11 g. The route is stated in the README (plan): no field verdict, no DFM layers, no stamps or cutters. `cutters.rs:281` and `:1100` refuse "this ring is its parts alone".
  - The ring is judged by `manufacturing::inspect`'s ray release: 0 obstructions and 0 unresolved at 0.100 and 0.075 mm, as in `examples/workshop_collection.rs:293-333`.
  - Its local wall comes from `cad::measure::thickness`, which `inspect` computes on its own when the ring is parts alone (`manufacturing/mod.rs:586`).
  - Setup: `auto_parting = false`, `parting_mm = 0`.
- **Stones:** none.
- **Build, step by step:**
  1. "Section plane": `Plane { base: PlaneBase::Section { theta_deg: 0.0 } }`, with x radial and y along the finger. Or put the sketch on `Workplane::section()` (`sketch.rs:77`).
  2. "Pointed-arch section": a constrained sketch.
     - **Recomputed from the brief's numbers.** The brief put the springers on r 9.3 with an inward comfort sagitta, which makes the tightest bore 9.18, about a quarter size small.
     - **Points:**
       - springers A (9.42, −2.8) and B (9.42, 2.8);
       - apex C (12.9, 0);
       - arc centres L (9.42, +0.7625) and R (9.42, −0.7625), radius 3.5625;
       - a bore arc A → (9.30, 0) → B (sagitta 0.12, comfort).
     - **Constraints:** `Constraint::Symmetry { a: A, b: B, center: O }`, and `Constraint::Distance` L–A = L–C = R–B = R–C = 3.5625 (`sketch.rs:143-164`).
     - **Corners:** `fillet_corner(A, 0.3)` and `fillet_corner(B, 0.3)` (`sketch/edit.rs:678`), above `MIN_EDGE_MM` 0.2.
     - **Result:** each flank leaves the apex 12.4° off the pull, a 155° keel. The feet are vertical at the springers, which makes them side faces.
  3. "Revolve the ring": `Revolve { sketch: Profile::Feature { feature: 2 }, pivot: [0.0; 3], axis: [0.0, 0.0, 1.0], degrees: 360.0, in_plane: false }`, the move `examples/workshop_collection.rs:149-173` makes.
  4. "Parting plane": `Plane { base: Parting, offset_mm: −0.03 }`.
  5. "Crocket": a curled leaf in the plane's XY at θ 18°, sunk 0.25 into the keel, hooking round the ring (`Geometry::Bezier`). Every stroke of the leaf is ≥ 0.8 wide in plan, and the hook's gap is ≥ 0.6.
  6. "Cope half": `Extrude { 5, +0.45, draft 3° }`, running z −0.03 → +0.42. The brief's +0.33 made a 0.6 mm fin, under Delft's 0.8 section; this one is 0.84 thick.
  7. Mirror across `Band` for the drag half.
  8. Boolean Union 6 + 7: the csg route, since 7 is a mesh (`cad.rs:1912-1924`).
  9. "Thirteen crockets": `Pattern { sources: [8], kind: Ring { count: 13, span_deg: 144.0 } }`. A partial span steps `span / (count − 1)` = 12°, so the crockets sit at θ 18 … 162 (`cad/pattern.rs:133`).
  10. "Foot plane": `Plane { Parting, offset_mm: 2.85 }`, above the springer.
  11. "Niche": a pointed niche 0.9 wide × 1.2 tall centred at r 10.4, then `Pattern::Polar` × 36 about [0, 0]. The pitch is 1.815, so the bar is 0.9.
  12. "Cut the niches": `Extrude { 11, −0.95, draft 3° }`, floor at z = 1.9. The surface there stands at |z| 2.43–2.78, so each niche is 0.5–0.9 deep.
  13. Mirror 12 across `Band`.
  14. Boolean Union 3 + 9.
  15. Boolean Subtract 12.
  16. Boolean Subtract 13.
- **What it shows off:** a constrained sketch; revolving a whole ring; features drawn in the parting plane and extruded along the pull; ring arrays; kernel versus mesh booleans; a sand verdict on a parts-only ring; the local-wall measure.
- **Traps and how they are avoided:**
  - **No chart means no field tools.** It is judged by ray release and local wall only, and the README says so.
  - **The keel sits on z = 0 by construction**, with the parting pinned (`auto_parting = false`).
  - **The section is a single-crest monotone drop** with vertical feet: the superellipse guarantee by another road (doctrine "The castability guarantee lives in profile.rs").
  - **Crocket hooks** would lock if extruded along a normal. Drawn in the parting plane and extruded along the pull, they cannot (house rule 1).
  - **Drafted halves meeting on z = 0 share a face** (`Snag::Degenerate`), so each half starts at −0.03.
  - **Crest phantom.** The keel has 12.4° of real draft either side, so facet noise has draft to be measured against. The release study still runs at both resolutions.
  - **Resize.** A revolved section does not follow the ring size, and `design.resize` does not move it. The `pointed-arch-section` cluster takes the bore from `band.size`.
- **Needs:**
  - Nothing new to build it.
  - C-T2 `pointed-arch-section` for a resizable template.
  - An `EqualRadius` constraint would make the arch a one-number edit (small, `sketch.rs:143`; optional).
  - The example carries:

```rust
fn pointed_arch(bore_r: f64, width: f64, thickness: f64, fillet: f64, comfort: f64) -> Sketch  // solves L/R centres
```

- **Template:** `nodes::cad::from_document` (`ringdesign-graph/src/nodes/cad.rs:215`) gives `cad.source` plus 16 `cad.feature` nodes. Expect 40–100 KB.
  - **Controls:** Width, Thickness, Keel, Crockets (`json.set` into feature 9's count), Niches.
  - It lifts cleanly.
- **Risk:** 3/5, medium.
  - If the csg union of the 13-crocket pattern with the revolved ring fails, join each crocket separately (13 unions).
  - If 72 niche subtractions are slow at export, subtract each side's pattern once, before the mirror.

## Rosa — *the west rose*

- **Status:** Not started. P2 has landed, so the arrayed stones are honest. The planned form waits on C-B2 (true pear plans), C-T1 (the tracery lesson) and C-T3 (quatrefoil oculi and mouchette lights). It is buildable now with oval lights, analytic light loops and round oculi. Template size waits on P7.
- **Concept:** The rose window of a west front on a round signet. A ruby oculus sits in a revolved roll moulding, and eight sapphire lights radiate from it like the lights of a rose. Eight pierced mouchettes turn between them, all leaning one way, so the rose spins like a flamboyant rose.
- **Theme face to palm:**
  - **Face (table):** the rose. Ruby oculus in a collet inside a roll moulding; 8 sapphire lights in collets, points outward; 8 pierced mouchette spandrels. The table's own filleted rim is the rose's outer order (doctrine "The head has one edge").
  - **Head walls:** a "gallery of kings" blind three-lancet arcade on each ±z cheek and at each end of the head, recessed 0.35. It is empty: no figures.
  - **Shoulders:** 4 pierced oculi each side, graded 1.6 → 0.9, dimming toward the palm like the rose's light fading.
  - **Palm:** plain, polished.
  - **Bore:** the à jour pilots and through-lights open here; every edge is broken 0.2 at the bench.
- **Base:**
  - Factory **013 Round** taken to a **16 × 16 mm face** (plan). The 0C spike settled the route (`examples/stock_spike.rs:185-235`): straight from the native 10 mm to 16 mm is refused, and it works through a **baked 130% step**.
    - Attach the preset.
    - Set `d.shank.head.length_mm` and `d.profile.width_mm` to 1.3 × native.
    - `bake(&d, "Signet 013 at 13 mm")` (`examples/stock_spike.rs:117`) into a new `Source`.
    - Attach that, and set 16 × 16.
  - No sand master and no envelope (`sand_envelope = false`), as Nocturne and Vesper are. Bore 18.6.
  - Base setup: copy `examples/stock_masterworks.rs:317-411`.
  - **Fallback:** 001 Cushion at 18 × 18, the brief's base. That also restores its four corner mouchette pairs.
- **Process:** lost wax, Gold 18k: `CastProcess::LostWax.apply(&mut d.draft)`, then `min_section_mm = 0.8`, `min_detail_mm = 0.15`. The through-lights, the proud collets and the crown piercing all lock in sand.
- **Stones:** 9 in all, each with a `head.bezel` collet and a `seat.bur { through: true }`.

  | Stone | Size | Tint | Collet |
  |---|---|---|---|
  | Ruby oculus, Round | 4.0 | ruby | wall 0.45, lip 0.3 |
  | Sapphire lights, 8 × Oval | w 1.8 × l 3.0 | sapphire | wall 0.35, lip 0.3 |

  The lights become Pear 3.0 × 1.8 once C-B2 lands. Pear's plan is an ellipse today (`GemCut::plan_pow`, `gem.rs:156`), so a pear collet would be wrong until then.
- **Layout at 16 mm.** These are starting numbers; the author asserts every land.

  | Element | Radius from the table centre (mm) |
  |---|---|
  | Ruby collet, outer edge | ≈ 2.48 |
  | Oculus moulding: a half-round of radius 0.3 revolved about the stone axis | 2.75 |
  | Oval lights, centred | 5.1 (spanning 3.6–6.6; collet to 6.95) |
  | Mouchettes, 1.0 wide, at 22.5° between the lights | 6.1–7.1 |

  - The mouchettes' land to the neighbouring collets must be ≥ 0.8 (at r 6.1 it is about 0.8, which is why the lights are 1.8 and not 2.0 wide).
  - The table is flat to about r 7.2; the rim rolls 0.6–0.9 below the plane.
- **Build, step by step:**
  1. `Band`.
  2. "Table": `Plane { base: Tangent { theta_deg: 90.0, across_mm: 0.0 } }`. Sketch x runs round the ring and y along the finger.
  3. "Rose net": a sketch on the Table.
     - Draw the 8 mouchette loops, drawn once and `Pattern::Polar` × 8 about [0, 0].
     - Tree lesson: the net of circles r 3.6/7.1 plus 8 radial mullions, split, and offset by C-T1.
     - Robust route: the `rose-tracery` cluster's closed loops.
  4. "Pierce the eight mouchettes to the bore": `Extrude { Profile::Feature { feature: 3 }, height_mm: −7.0, draft_deg: 0.0 }`, `Attach::Cut`.
     - The table stands about 1.75 over the bore at its centre and more toward its edge round the ring, so −7.0 reaches the bore everywhere under the rose.
     - Measure the real figure on the resized stock from `FieldReport::thinnest_wall_mm`.
  5. "Oculus moulding": an inline sketch in the part's x–z plane (a 0.3 half-round at 2.75 from the axis, plus a 0.15 cove), `Revolve { pivot: [0; 3], axis: [0, 0, 1], degrees: 360 }`, seated `Placement::ring(90.0, 0.0)`, `Attach::Join`, `blend_mm` 0.12.
  6. "Oculus stone": `builders::stone_feature(id, ruby, Placement::ring(90.0, builders::stand_off_mm("bezel", ruby)))` (`cad/builders.rs:954`, `:504`), with its `tint`.
  7. "Oculus collet": `builders::feature_on(id, "Oculus collet", "head.bezel", 6, json!({ "wall_mm": 0.45, "lip": 0.3 }))` (`cad/builders.rs:966`).
  8. "Oculus seat": `feature_on(.., "seat.bur", 6, json!({ "through": true }))`.
  9. "First light":
     - `stone_feature(id, sapphire, Placement::Ring { theta_deg: 90.0, across_mm: 5.1, height_mm: stand_off_mm("bezel", sapphire), spin_deg: 0.0, tilt_deg: 0.0, cant_deg: 0.0 })`.
     - It sits up the finger, and the stone's length axis is the part's x, along the finger, so it points radially. Check which end is the point once C-B2 lands, and use `spin_deg` 180 if needed.
     - Then its collet (wall 0.35) and its bur (through).
  10. "Eight lights": `Operation::Pattern { sources: Sources(vec![9, collet, bur]), kind: PatternKind::About { part: 6, count: 8, span_deg: 360.0 } }`.
      - These are rigid world turns about the ruby's axis (`cad/pattern.rs:44-50`). They stay on the table only because the table is a plane.
      - P2 keeps a `gem` on every copy.
  11. "Oculi of the nave": `cutter.pierce` (`{ "shape": "Round", "width_mm": w, "length_mm": w, "through": true }`) at θ 90 − (44, 54, 64, 74), with widths 1.6, 1.4, 1.2, 0.9.
      - `cutters::pierce_at` sizes to the local band and refuses near an edge (`cad/builders/cutters.rs:1099`).
      - Mirror with `Pattern { Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } }`.
      - The shape becomes Quatrefoil with C-T3.
- **Stamps** (`setting::Stamp`, `setting.rs:897`; the walls are stock mesh, not kernel faces):
  - four "Wall arcade" stamps: a three-lancet-on-sill outline 6.0 × 2.4, ≤ 512 points (`PLAIN_MAX_STAMP_POINTS`, `setting.rs:1252`, keeps format 5), `cut: true`, `sink_mm: 0.35`, `height_mm: 0.3`, `along_pull: true`;
  - two on the ±z cheeks at θ 90, and two at the head's ends (θ ≈ 90 ± 38), where the wall faces round the ring;
  - find each cheek's `v` with `skin::Atlas::of(&d, w, h)` (`skin.rs:52`) and `Atlas::cheek`.
  - `Stamp` has no `flip`. A symmetric arcade needs none; an asymmetric outline on the −z cheek is reflected in x with its point order reversed, to stay counter-clockwise.
- **What it shows off:** a work plane over stock; net → cells → offsets → one multi-loop cut; a revolve at the origin, then seated; the stone/collet/bur trio; an About array of three parts at once (P2); stamps on stock; the pierce builder mirrored through a section.
- **Traps and how they are avoided:**
  - **Crossing circles are refused** (`sketch/region.rs:133-137`). Split first, and name each light by an entity on its outer loop so edits keep it (`sketch/region.rs:57-75`).
  - **The rim rolls 0.6–0.9 mm under the plane** (doctrine "The head has one edge"). Nothing is placed past r 7.2.
  - **The pilot and the nearest opening share metal.** The ruby bur's pilot and the lights' pilots must keep 0.8 between them. Overlapping cut tools are clustered into one (`parts.rs:17`) and would merge silently rather than fail, so the author asserts the distance.
  - **Revolves in world coordinates drift on resize.** Both revolves are modelled at the origin and seated (house rule 2).
  - **Stamps on the head's walls** use `along_pull`, and no wall straddles z = 0 (`setting.rs:1369`). That is harmless in wax and keeps the file honest.
  - **The baked 130% source is not a bundled preset**, so the template carries it inline (next).
- **Needs:**
  - C-B2 (or ovals), C-T1, C-T3, C-T4, P7.
  - The example carries:

```rust
fn stock_at(id: &str, face_l: f64, face_w: f64, bore: f64, sand: bool) -> Result<RingDesign>   // stock_masterworks base()
fn bake_step(d: &RingDesign, scale: f64) -> Result<Arc<Source>>                                 // stock_spike.rs:117
fn rose_lights(lights: u32, r_in: f64, r_out: f64, bar: f64) -> Sketch                          // mouchette loops
```

- **Template:** The lift gives stock nodes, an `/imported_base` patch, a `/stamps` patch (until P7's stamp nodes) and about 20 `cad.feature` nodes.
  - **Size:** the baked source travels inline, about 3 MB even after P7, unless P7's `base.preset` also carries a `pre_scale` step: `base.preset { id: "013", pre_scale: 1.3, face_length, face_width, bore }`, applied deterministically on load. Ask the P7 lane for it.
  - **Controls:** Lights (one number into the `rose-tracery` cluster and, through `json.set`, the About pattern's `count`), Bar, Oculus stone, Light size.
- **Risk:** 4/5, medium. About 20 csg cuts go through the head; an export Subtract costs 10–331 ms each, so a build takes seconds (P1 already covers the open).
  - If the About pattern of three sources misbehaves on seated stock, place the 8 lights by hand (24 features).
  - If the 16 mm bake reads soft in the render, fall back to 001 at 18 × 18.

## Sigillum — *the chapter seal*

- **Status:** Not started. Blocked on C-T6 (the legend). C-T5 needs verifying (see its section). The cast part and the quatrefoil and fleur can be built now.
- **Concept:** A cathedral chapter's seal ring on the rosette stock, whose cusped frame is the seal's own cusped border. The seal is **cut at the bench**, which is what a seal is: mirrored intaglio with drafted walls, so wax releases the impression. What can be cast is cast, along the pull: the chapter-house arcade in the cheeks and quatrefoil roundels down the shoulders' side faces. This is the collection's honest sand-plus-bench ring.
- **Theme face to palm:**
  - **Face (table), bench intaglio, mirrored:** a cusped quatrefoil field (−0.30), a fleur-de-lis (−0.70), and a Textura legend round the rim, ✠ SIGILLVM CAPITVLI (−0.40).
  - **Cheeks:** a cast, recessed three-bay lancet arcade (stamps, `along_pull`, cut 0.35).
  - **Shoulders:** 3 graded quatrefoil roundels on each shoulder's ±z side faces (12 stamps).
  - **Palm and bore:** plain.
- **Base:**
  - Factory **005 Rosette** (plan). Native face 18 round the ring × 21 across; resize to **16 × 18.7** (native aspect).
  - Through `sand_master` (`imported_base/sand_master.rs:54`): `ImportedBase::attach(&mut d, sand_master(PRESETS … "005" .load()?)?)`, then `sand_envelope = true`. Bore 18.6.
  - **Spike fact:** on bare 005 the sand master fills up to **0.8 mm** (`stock_spike` "envelope fill"; commit `9ccac2b`). First step: render the bare master and confirm the rosette's cusps still read.
  - **Fallback:** 017 Tonneau, which Zenith proved clean in Delft, with the seal field drawn as an oval track.
- **Process:** Delft two-part sand, Silver 925. Nothing is proud on the table. The bench cuts are `Stage::Bench`, so they are in the finished ring and left out of the pattern (`mesh::try_build_pattern`, `mesh.rs:411`; `Document::leave_bench_parts_out`, `cad.rs:1347`).
- **Stones:** none.
- **Build, step by step.** Cast stamps first.
  1. Two "Chapter-house arcade" stamps on the ±z cheeks: three lancets, 7.0 × 2.2, `cut: true`, `sink_mm: 0.35`, `along_pull: true`.
     - Stroke and gap widths ≥ 0.30 (Delft detail), judged by granulometry (`dfm::findings`, `stamp_finest_mm`).
     - Locate each cheek's `v` with `skin::Atlas`.
  2. Twelve "Shoulder roundel" stamps: cusped quatrefoils Ø 2.2 / 1.8 / 1.4, `cut: true`, `sink_mm: 0.25`, `along_pull: true`.
     - They sit on the **side faces** of the shoulders (their ±z walls), within the first ~25° past the head's end, where the wall is ≥ 2.6 mm tall.
     - **Not on the shoulder's crown:** a recess on a radially facing surface locks both mould halves (the bur-dimple lesson, doctrine "commissions"). This corrects the brief, which left the side unstated.
  3. Check every cast stamp with `Stamp::parting_monotone(&d)` (`setting.rs:2149`), then with the field and the release study.

  CAD features:

  4. `Band`.
  5. "Table": `Plane { Tangent { 90, 0 } }`.
  6. "Seal field": a sketch on the Table. The cusped quatrefoil (4 arcs plus cusps, or C-T3's harvested Under Gallery Cut 002), 9.0 across, then `Sketch::mirror(all, y_axis_line)` (`sketch/edit.rs:738`) so the impression reads true.
  7. "Cut the field": `Extrude { Profile::Feature { feature: 6 }, −0.30, draft_deg: 15.0 }`, `Attach::Cut`, `Stage::Bench`.
  8. "Fleur": a sketch on the Table (C-T3's `fleur-de-lis.svg` through `import_svg`, 5.0 tall), mirrored.
  9. `Extrude { 8, −0.70, draft 12° }`, Bench Cut.
  10. "Legend": `sketch::text(TextFont::Textura, "SIGILLVM CAPITVLI", cap 1.2, tracking to close the circle, Some(TextArc { radius_mm: 6.2, .. }))`, plus the cross pattée from C-T3, mirrored.
      - One Sketch feature per word if the 1024-point cap bites.
      - One Sketch feature per depth, so no `Profile::Regions` is needed.
  11. `Extrude { 10, −0.40, draft 12° }`, Bench Cut.
- **What it shows off:** bench-stage cut features; `draft_deg` on a cut; mirrored sketches; text as geometry; finished ring against pattern; stamps on stock with the sand envelope; bench locating marks.
- **Traps and how they are avoided:**
  - **Nothing is cast on the table.** On a signet's face, anything proud off the parting line is an undercut (doctrine "One theme per ring, on the factory's signets"). The seal is all bench.
  - **Bench marks can lock.** A dot off the parting line on the zero-draft table leans 14° (doctrine, the sketches lesson). All three bench cuts are centred on the table's centre, which is on the parting line; verify their `LocatingMark`s land there (C-T5).
  - **The envelope must be on**, or single-sample obstructions appear on bare stock (doctrine, Zenith notes).
  - **The chart reads true from −Z**, so an asymmetric outline on the −z cheek is reflected (see Rosa's stamps).
  - **Depth over the bore:** the deepest cut, 0.70, must leave ≥ 0.8. Read the table-over-bore figure off the resized stock's field report (about 1.75 on factory signets); if it is under 1.5, the fleur goes to −0.60.
  - **Textura hairlines** are about 0.1 mm at a 1.2 mm cap. That is fine, because the legend is cut by a graver and never judged by the sand floor.
- **Needs:**
  - C-T6 (blocking), C-T5 (verify, or the `mark` opt-out), C-T3 (fleur, quatrefoil, cross), P7 (stamp nodes and `base.preset { id: "005", sand: true }`).
  - The example carries:

```rust
fn seal_sketches(face: [f64; 2], legend: &str) -> Result<[Sketch; 3]>   // field, fleur, legend; each mirrored
fn shoulder_roundels(d: &RingDesign) -> Result<Vec<Stamp>>              // side-face v from skin::Atlas
```

- **Template:** stock nodes (about 3 MB inline until P7, then under 1 MB), a `/stamps` patch until P7, and 8 `cad.feature` nodes.
  - **Controls:** Legend (a `sketch.text` node from C-T6 into feature 10's operation), Seal depth (a script node scaling the three extrude heights), Face size.
- **Risk:** 3/5, medium. The risks are the 0.8 mm fill on 005 and the marks. If C-T6 slips, the legend becomes a `bench_only` `TextAlpha` layer on the table (the old engraving route), and the text control is lost.

## Porta — *the portal*

- **Status:** Not started. Buildable now in interim form: 6 hand-placed crockets, Marquise shoulder piercings, and roll mouldings swept along points computed in the example. The full form waits on C-V2 (owned by this lane), C-T3 (lancet cutter, fleur, crocket leaf) and C-T7.
- **Concept:** A cathedral's west portal cut into a drop-shaped signet. Four archivolts step down into the head like the splay of a real portal, each carrying a swept roll moulding. A ruby sits in the tympanum, and the twin doors are split by a trumeau. An ogee hood-mould with crockets rises to a fleur-de-lis at the drop's point, and a pierced trefoil crypt window fills the round end.
- **Theme face to palm:**
  - **Face:** the portal, 8.5 round the ring × 12.0 along the finger.
    - 4 archivolts on a depth ladder, each with a roll moulding.
    - Tympanum with a ruby, à jour; door leaves and trumeau.
    - An ogee hood (+0.4) with 6 crockets and a fleur-de-lis finial.
    - A crypt trefoil pierced through.
  - **Head walls:** blind arcades (stamps, `along_pull`, cut).
  - **Shoulders:** 4 pierced lancets each side, diminishing.
  - **Palm:** plain.
- **Base:**
  - Factory **009 Drop** (plan: 002 went to Draco). Native face 17 round the ring × 22.3 across; resize to **14.5 × 19**. Its long axis runs across the band, so the portal stands upright along the finger.
  - Unmirrored: 009 is upright (2.5 mm asymmetry across the parting plane, plan F2), so it is lost wax only (Logan's answer 1).
  - The portal's apex, hood and finial go toward the drop's point, and the crypt toward its round end. Read which band edge the point is on from the stock (`Preset::plan`, 48 radii) and mirror the sketch in y if needed.
  - Bore 18.6.
- **Process:** lost wax, Gold 18k, investment section 0.8. The crockets and hood stand proud of a zero-draft table, which is lost-wax-only work.
- **Stones:** 1, a Round 3.0 ruby in the tympanum, with a `head.bezel` and a `seat.bur { through: true }`.
- **Build, step by step:**
  1. `Band`.
  2. "Table": `Plane { Tangent { 90, 0 } }`.
  3. "Portal": a sketch on the Table.
     - An equilateral pointed arch (two arcs) 8.5 wide on a straight threshold, 12.0 tall.
     - `offset(loop, −0.65)` four times gives 4 nested loops, and the innermost opening is 3.3 wide.
     - A lintel line at 60% of the innermost height, and a trumeau pair 0.6 apart from threshold to lintel, as T-junctions.
     - Regions (`profile_regions`): 4 rings, the tympanum, 2 leaves, the trumeau.
  4. "Archivolt i" (i = 1..4): `Extrude { Profile::Region { feature: 3, region: ring_i }, −step·i }`, Cut.
     - **Ladder.** The table stands about 1.75 over the bore at its centre, and the investment section is 0.8, so the deepest floor is 0.9: `step = (t_bore − 0.85) / 5 ≈ 0.18`.
     - Archivolts at −0.18, −0.36, −0.54 and −0.72; tympanum and leaves at −0.9; trumeau at −0.72.
     - This corrects the brief's −1.0, which left 0.75.
  5. "Tympanum" and "Door leaves": Extrude of those regions at −0.9, Cut.
  6. "Roll moulding i": `Sweep { sketch: Sketch::circle(0.2).into(), path }`, with the path along ring i's inner arch edge on its floor, sunk 40%.
     - Sampled every 0.3 mm; at most 128 stations (`cad.rs:1858-1862`).
     - Until C-T7, model it at the origin in the Table's frame and seat it `Placement::ring(90, 0)` with `spin_deg` 90 (house rule 2). `Attach::Join`, `blend_mm` 0.08.
  7. "Ruby": `stone_feature` at the tympanum centre, seated `Placement::Ring { theta_deg: 90, across_mm: y_tymp, height_mm: −0.9 + stand_off }`.
  8. Its bezel.
  9. Its bur, through.
  10. "Hood": an ogee drip strip 0.5 wide, two `Geometry::Bezier` S-curves rising from the arch springers to an ogee point 2.0 above the apex. `Extrude +0.4`, Join, blend 0.1.
  11. "Crocket": the C-T3 leaf, `Extrude +0.3`.
  12. `Pattern { sources: [11], kind: Along { path: 10, entity: <ogee curve>, count: 6, alternate: true, .. } }` (C-V2). Interim: 6 hand-placed copies.
  13. "Finial": the fleur-de-lis, `Extrude +0.5`, at the ogee point.
  14. "Crypt trefoil": a trefoil 1.6 across at the round end, `Extrude −7.0` Cut, through.
  15. `cutter.pierce` × 4 each side, Marquise (Lancet after C-T3), lengths 2.2 → 1.4, at θ 90 ± (42, 52, 62, 72).
  16. Mirror through `Section { 90 }`.
  17. **Stamps:** 4 wall arcades as in Rosa.
- **What it shows off:** offsets as architecture; depth ladders by region; a sweep along an arch; Bezier ogees; region picks that survive edits; a pattern along a curve (C-V2's first user).
- **Traps and how they are avoided:**
  - **Wall over the bore:** the ladder stops at 0.9, and the author asserts `cad::measure::thickness(&mesh, 0.8)` under the portal. The field's `thinnest_wall` does not see CAD cuts.
  - **`offset` refuses folds** (`sketch/edit.rs:430-433`). A pointed apex offsets inward cleanly; the hood's concave ends are trimmed.
  - **Sweep paths are points.** A path modelled at the origin and seated follows a resize, but not an edit to the arch; C-T7 fixes the edit case.
  - **The drop's width at its point** limits the hood and finial. Check the plan radii before fixing the ogee's height.
- **Needs:**
  - C-V2 (this lane owns it), C-T3, C-T7, P7.
  - The example carries:

```rust
fn portal(width: f64, height: f64, steps: u32, step_mm: f64) -> Sketch    // nested offsets, lintel, trumeau
fn arch_edge_points(s: &Sketch, ring: usize, floor: f64, pitch: f64) -> Vec<[f64; 3]>   // ≤ 128
```

- **Template:** stock nodes, a stamps patch until P7, and about 25 `cad.feature` nodes.
  - **Controls:** Step depth (a script node: one number × i into features 4–5), Steps, Stone size.
  - Stock ≤ 1 MB after P7.
- **Risk:** 3/5, medium. If the sweeps fold at the arch apex, split each moulding at the apex into two sweeps.

## Arcus — *the flying buttress*

- **Status:** Not started. Buildable now: every builder exists. One-number Spread (pier placement tracking the builder) waits on P7's placement pins. Gothic pointed claw tips wait on P6 (optional). Artwork comes from C-T3.
- **Concept:** A cathedral solitaire that means it. The sapphire sits in an openwork basket, the choir. The app's own `shank.cathedral` arches are the flyers, each springing from a pinnacled pier that stands on the band exactly where the builder's arch lands. Six teardrop azures under the stone are the crypt windows, and the shank's side faces carry the nave's blind arcade to the palm.
- **Theme face to palm:**
  - **Head:** an emerald-cut sapphire in a four-claw basket with 2 rails, and 6 teardrop azures under it.
  - **Shoulders:** two flyers (`shank.cathedral`, spread 34°). Each springs from a pier with a lofted spire, 4 crockets, a revolved knop finial and a gargoyle spout in silhouette.
  - **Side faces:** the nave, a blind lancet arcade (SVG tiling) from the piers to the palm.
  - **Palm:** the apse, plain.
- **Base:** `ProfileStyle::Flat` 3.2 × 2.3, `flatten_sides()`, `ShankKind::Cathedral` amount 0.8 (the shoulders swell in width), bore 18.6.
- **Process:** lost wax, Gold 18k, investment section 0.8. The basket, azures and overhanging flyers are investment work. In wax the builders' parts are `Stage::Cast` (`builders::setting_features` makes them Bench only under sand, `cad/builders.rs:982`).
- **Stones:** 1, an Emerald-cut sapphire, w 6 × l 8.
- **Build, step by step:**
  1. `Band`.
  2. "Stone": `stone_feature(id, Gem { cut: Emerald, w_mm: 6.0, l_mm: 8.0, .. } + sapphire tint, Placement::ring(90, stand_off_mm("basket", gem)))`.
  3. "Choir": `feature_on(.., "head.basket", 2, json!({ "prongs": 4, "wire_mm": 0.9, "rails": 2 }))`.
  4. `seat.bur { through: true }`.
  5. "Crypt windows": `feature_on(.., "cutter.azure", 2, json!({ "shape": "Teardrop", "count": 6, "head": 3 }))` (`cutters.rs:18`; the `head` param names the head feature, `cad/builders.rs:33`).
  6. "Flyers": `feature_on(.., "shank.cathedral", 2, json!({ "spread_deg": 34.0, "rise": 0.75, "wire_mm": 1.0, "head": 3 }))`.
  7. "Pier", at the origin: `Box { size: [1.6, 1.6, 2.4] }`.
  8. "Pier foot": `Chamfer { source: 7, edges: <the four bottom edges>, base_face: <bottom face>, distance_mm: 0.3 }` (`cad.rs:100`). Seam bead v1 blends no three-edge corner (`blend.rs:15-16`), and the chamfer keeps the bead off the pier's corners.
  9. `Plane { Face { feature: 8, face: <top> } }`.
  10. Sketch: a square 1.6 on plane 9.
  11. Sketch: a square 0.25 on the same plane, offset +2.2.
  12. "Spire": `Loft { sections: [Profile::Feature { feature: 10 }, Profile::Feature { feature: 11 }] }`, with matching curve counts.
  13. Boolean Union 8 + 12: a kernel boolean under 500 faces, so it stays B-rep.
  14. "Crocket": the C-T3 leaf sketched on one sloped planar face of 13 (`FaceAnchor`), `Extrude +0.35`.
  15. `Pattern { [14], About { part: 13, count: 4 } }`, about the pier's own z. **Faces first:** after 15 the spire is a mesh.
  16. "Knop": a ball-and-collar profile, `Revolve` about local z at the spire tip.
  17. "Gargoyle": `sketch::exchange::import_svg(gargoyle.svg)` on an inline workplane in the local y–z plane, so the spout projects outward and is seen side-on. `Extrude` ±0.45 (plane offset −0.45, height 0.9).
  18. "Pinnacle": Union chain 13 + 15 + 16 + 17 (csg, since 15 is a mesh), seated `Placement::Ring { theta_deg: 124.0, height_mm: 1.2 − 0.3 = 0.9 }`, `Attach::Join`, `blend_mm` 0.3.
      - **124 = 90 + spread.**
      - The Box is centred on its origin, so the brief's `height_mm: −0.3` would have buried it 1.5 mm.
  19. `Pattern { [18], Mirror { Section { theta_deg: 90 } } }`.
  20. **Field layer "Nave arcade":** `TilingLayer { alpha: "Nave arcade" (SVG, 3 bays a tile), repeats_around: 36, height_mm: 0.25, .. }`, with `Window { enabled: true, theta_deg: 270.0, span_deg: 280.0, fade_deg: 6.0, invert: false, v_gate: VGate::SideFaces(SideFacePick::Both) }` (`field.rs:399-438`). The span runs from the piers through the palm.
- **What it shows off:** builders and hand-built parts meeting at one number; loft; a sketch on a part's face; About arrays; an imported SVG outline; a native chamfer; a seam bead; a mirror through a section; the full basket + azure + cathedral chain, which is Arcus's alone in the catalogue (Officina's ring became Fenestra).
- **Traps and how they are avoided:**
  - **The flyer builder has preconditions.** `shank.cathedral` needs a head, and a stone standing out of the crown. It refuses an arch that runs back into the band with "give it more rise, or less spread" (`cutters.rs:1038`), which is why rise is 0.75.
  - **The pier must track the spread.** Until P7's placement pins, the template cannot wire Spread into feature 18's placement.
    - Interim: carry the spread as a `Transform { source: 18, rotation_deg: [0, 0, spread − 34] }` fed by `json.set`. This is valid because a Cathedral shank keeps its thickness.
    - **(Unverified** whether `Transform` turns a seated part about the world z axis or its own frame; `cad.rs:1556` builds the rotation from Euler angles.) Otherwise leave Spread unexposed until P7.
  - **Faces before meshes:** the crocket's sketch-on-face precedes the About pattern (house rule 3).
  - **The gargoyle SVG** needs units on width and height, a uniform viewBox, no transforms and no S/T. It is a silhouette with no eye or mouth (house rule 11). If a render reads it as a face, swap it for a leaf spout.
  - **Wall over the finger:** claws are floored by the bore (`builders::build_in`, `cad/builders.rs:744-745`), and azures refuse a stone too small by name. An 8 × 6 clears both.
  - **Side gates on a Cathedral swell** are resolved on the reference section (plan F4), so a little of the arcade may spill onto the crown fillet at the shoulders. That is cosmetic in wax, and P5 fixes it.
- **Needs:**
  - C-T3 (crocket leaf, gargoyle silhouette, nave arcade tile), P7 (placement pins), P6 optional (`tip: Point` for pointed claws).
  - The example carries:

```rust
fn pinnacle(doc: &mut Document, ids: &mut impl FnMut() -> Id, theta: f64, spread: f64) -> Result<Id>
```

- **Template:** The lift gives profile and shank nodes, one tiling layer with its SVG alpha node, and about 19 `cad.feature` nodes.
  - Procedural ≤ 300 KB; the SVG is text.
  - **Controls:** Stone size, Spread, Rise, Pier height (`json.set` into the Box size).
- **Risk:** 4/5, medium-high: csg union chains of small parts, and a builder's arch meeting a hand-built pier.
  - If the union chain fails, seat each pinnacle part at the same placement and let the band join cluster them (`parts.rs:17`).
  - If the arch and the pier do not meet cleanly, widen the pier to 1.8.

## Capsa — *the reliquary*

- **Status:** Not started. Buildable now as the plan orders it: **native only** (a `Chamfer` on the eaves instead of the OpenCascade fillet, format 5). The artwork comes from C-T3; C-V2 is optional for the bays.
- **Concept:** A Gothic chasse lies along the finger, in the coffin-ring convention of memento-mori rings. It stands on a plinth, its long walls arcaded and set with cabochons like Limoges enamels. Its gabled roof is a **separate cast lid**, hinged at the bench and crested with fleurs-de-lis. Lift it: crossed bones under an hourglass lie on the relic floor.
- **Theme face to palm:**
  - **Head:** a chest 13.0 along the finger × 9.4 round the ring × 3.8 tall, walled 1.1, on a plinth. Three-bay blind arcades on both long walls, and a cabochon in each centre bay and each gable.
  - **Lid (Separate):** a gable roof prism with fleur-de-lis cresting on the ridge, two revolved finials, chamfered eaves, and a cabochon on each roof slope.
  - **Inside:** crossed bones under an hourglass in relief (+0.4) on the relic floor.
  - **Shoulders:** 3 graded quatrefoil piercings each side (Round until C-T3).
  - **Palm and bore:** plain.
- **Base:** `ProfileStyle::Flat` 4.6 × 2.3, `ShankKind::Cathedral` amount 0.5, bore 18.6.
- **Process:** lost wax, Gold 18k, investment section 0.8, **two castings**: `manufacturing::Casting::Ring` and `Casting::Part(lid)` (`manufacturing/mod.rs:301-307`). A Separate part is its own casting, poured only when chosen (doctrine "The ring is the casting").
- **Stones:** 6 oval cabochons, w 1.8 × l 2.3 (set `l_mm` explicitly; `Gem::cabochon` defaults the aspect to at most 1.25, `gem.rs:250`).
  - 4 garnets: the long walls' centre bays and the two roof slopes.
  - 2 sapphires: the gables.
  - Each has `head.bezel { wall_mm: 0.35, lip: 0.3 }`; a cabochon gets no bur (`setting_features`, `cad/builders.rs:991`).
  - This corrects the brief. Its 3.0 × 4.0 cabochons do not fit a bay on a 13 mm wall with 0.8 posts (bay 3.27 × about 2.8), and "4 in the centre bays" of two three-bay walls counted 2.
- **Build, step by step:**
  1. `Band`.
  2. "Chest": `Box { size: [13.0, 9.4, 3.8] }` at the origin (Free for now).
  3. "Hollow the chest": `Shell { source: 2, open_faces: [<top>], thickness_mm: 1.1 }`. Native shell works only on an analytic box, cylinder or sphere (`cad.rs:1988`), so it comes before any cut.
  4. "Relic floor": `PressPull { source: 3, face: <inner floor>, distance_mm: 0.4 }`.
  5. "Arcade, +y wall": a sketch on the +y wall's outer face (`FaceAnchor { feature: 4, face }`), three pointed bays between 0.8 posts (the `lancet-arcade` cluster, `bays: 3`).
  6. `Extrude −0.30`: 1.1 − 0.3 leaves the 0.8 section behind each niche. The plan asked for 1.0 walls; 1.1 with 0.3 niches is the version that holds 0.8.
  7. Boolean Subtract, a kernel boolean that keeps the chest B-rep.
  8. The −y wall.
  9. The same.
  10. The same.
  11. "Memento mori": `import_svg(bones-and-hourglass.svg)` sketched on the relic floor face.
  12. `Extrude +0.4`.
  13. Kernel Union.
  14. "Plinth": `Box [11.0, 9.0, 1.7]`, seated `Placement::ring(90, 0.85 − 1.1)`, so it sinks 1.1 into the crest and its ends round the ring still meet the band (sagitta 0.87 at ±4.5 on r ≈ 11.6). `Attach::Join`, blend 0.35. The parts cluster unites it with the chest.
  15. Seat the chest (13) with `Placement::ring(90, 0.6 + 1.9)` (plinth top + half the chest), `Attach::Join`, blend 0.3.
  16. "Cabochons": `cad::stone_on_face(id, cab, on: 13, &FaceSeat { face: <niche floor or gable face>, u_mm, v_mm, height_mm: 0.0, spin_deg })` (`cad.rs:663`) for the 4 chest stones, **while 13 is still a kernel body**, each with its bezel.
  17. "Lid": a pentagon sketch (eaves 9.8 wide, 0.5 walls, ridge 2.0 above the eaves) on a workplane at the chest's end, `Extrude 13.2` along the finger.
      - `Attach::Separate`, `Stage::Cast`.
      - Seated at the chest's top with 0.05 clearance.
      - Its 2 roof cabochons go on its slope faces now, before any mesh operation.
  18. "Eaves": `Chamfer { source: 17, edges: <the four eaves edges>, base_face: <underside>, distance_mm: 0.2 }`. It goes here, while the lid is B-rep, instead of the OpenCascade fillet as a last step.
  19. "Cresting": `import_svg(fleur-cresting.svg)` on the ridge's mid-plane, `Extrude ±0.3`, kernel Union.
  20. "Finial": a Revolve at one ridge end, Union.
  21. `Pattern { [20], Mirror { plane: Plane { feature: <chest mid-plane> } } }`, Union: csg, last.
  22. `Joint { a: <chest>, b: <lid>, clearance_mm: 0.05, method: "Three-knuckle hinge, 0.8 mm pin, soldered at the bench", notes: "" }` (`cad.rs:1284`) in `Document::joints`.
  23. `cutter.pierce` × 3 via `pierce_at`, at θ 90 ± (48, 60, 72).
  24. Mirror through `Section { 90 }`.
- **What it shows off:** native shell; press-pull; sketches on a part's faces; B-rep booleans kept B-rep; stones on part faces; a native chamfer; a Separate part as its own casting; a joint; SVG outlines.
- **Traps and how they are avoided:**
  - **Seat the stones before the chest becomes a mesh** (house rule 3). The chest stays B-rep through its kernel booleans; the mirror in step 21 is the only mesh step, and it comes last.
  - **The chest floats at its ends round the ring.** Its 9.4 width drops about 0.95 over ±4.7 on r ≈ 11.6; the sunk plinth fills the gap, and a seam bead closes the join.
  - **Height.** Plinth 0.6 + chest 3.8 + lid 2.5 is about 7 mm over the crest, plus finials. The review round decides; coffin rings run about 6.
  - **The overhang along the finger:** the chest is 13 mm on a band about 5.8 wide, so its underside floats about 1.2 mm over the finger. That is normal for a coffin ring, and lost wax only.
  - **Two castings:** export both, `finished-metal.stl` and `lid-pattern.stl`.
  - **Shoulder piercings on a swelling section:** `pierce_at` sizes to the local band and refuses near the edge.
  - **No faces** (house rule 11): the brief's skull is replaced by bones and an hourglass. See Open questions.
- **Needs:**
  - C-T3 (cresting strip, fleur, bones and hourglass SVGs, quatrefoil), C-V2 optional (a `Line` pattern for bays), C-T6 optional (MEMENTO MORI inside the lid, as a bench cut).
  - The example carries:

```rust
fn chest(doc: &mut Document, ids: &mut impl FnMut() -> Id, size: [f64; 3], wall: f64) -> Result<Id>
fn lid(doc: &mut Document, ids: &mut impl FnMut() -> Id, chest: Id, eaves: f64, ridge: f64) -> Result<Id>
```

- **Template:** band nodes plus about 30 `cad.feature` nodes. It stays **format 5**, with no `Stored`. Procedural ≤ 300 KB.
  - **Controls:** Chest size (`json.set` into the Box), Wall (into the Shell), Cabochon size.
- **Risk:** 4/5, medium-high: the two-casting path and stones on part faces.
  - If the lid's chamfer is refused on the eaves edges, try native `Fillet { radius_mm: 0.2 }` (`cad.rs:1950`); failing that, leave the eaves square and break them at the bench.

## Lanterna — *the octagon lantern*

- **Status:** Not started. Every API it needs exists (P2's multi-source Pattern has landed), but by plan it is built **last**. C-T1 eases its star vault.
- **Concept:** Ely's octagon, seen from beneath. A star vault of ribs is raised on the octagonal table and gathers into a revolved boss carrying an Asscher amethyst. Eight lancet windows pierce the table between the ribs, and at the eight corners stand pinnacles with twisted spirelets, each carrying a citrine lamp on its outward face.
- **Theme face to palm:**
  - **Face (table):** a star vault of 8 ribs and 8 tiercerons, 0.6 wide and raised 0.45; the boss with the amethyst in a collet, à jour; 8 pierced lancet lights.
  - **Corners:** 8 pinnacles, each a shaft, a twisted spirelet (45°, taper 0.08), 4 crockets and a knop finial, with a Round 1.5 citrine lamp on the outward face.
  - **Walls (the octagon's 8):** paired blind lancets (stamps, `along_pull`, cut 0.3).
  - **Shoulders:** clasping buttresses with two set-offs, mirrored.
  - **Palm:** plain.
- **Base:** factory **015 Octagon** at its native 16 × 16, bore 18.6, no sand master (wax). 015 is also Caiman's (sand); the octagon is this ring's subject.
- **Process:** lost wax, Gold 18k, investment section 0.8.
- **Stones:** 9.
  - An Asscher **5.0** amethyst, in `head.bezel` with `seat.bur { through: true }`. The brief's 6.0 has a half-diagonal of about 3.8, plus its collet wall, so its corners overhung the brief's 3.0 boss by about 1.4. The boss is now r 3.7.
  - 8 Round 1.5 citrines, in `head.bezel { wall_mm: 0.3 }` with blind burs.
- **Build, step by step:**
  1. `Band`.
  2. "Table": Tangent 90.
  3. "Star vault": one closed star outline.
     - Ribs run from the boss circle (r 3.7) to the corners, and tiercerons to the mid-sides, drawn once, `Pattern::Polar` × 8, split and trimmed into a single loop with no crossings and no holes (C-T1 or by hand).
     - Read the corners' bearings from the stock (`Preset::plan`) rather than assuming 22.5°.
  4. `Extrude +0.45`, Join, `blend_mm` 0.12.
  5. "Lantern windows": a lancet 1.1 × 2.4 centred at r 5.6, between each rib and tierceron, `Pattern::Polar` × 8 (the `rose-tracery` cluster with `head: "pointed"`).
  6. `Extrude −6.0` Cut, through.
  7. "Boss": `Cylinder { radius_mm: 3.7, height_mm: 1.2 }`, seated `Placement::ring(90, 0.6 − 0.2)`.
  8. "Raise the boss": `PressPull { source: 7, face: <top>, distance_mm: 0.3 }`. This is the direct-edit lesson.
  9. "Boss moulding": a revolved roll at r 3.7–4.1, seated with the boss, Join.
  10. "Amethyst": `cad::stone_on_face(id, asscher, on: 8, &FaceSeat { face: <top>, .. })`.
  11. Its collet.
  12. Its bur, through.
  13. **The first pinnacle, seated at the first corner** (θ and `across_mm` from the plan, 1.0 inside the outline; `spin_deg` = the corner's bearing):
      - "Shaft": `Box [1.3, 1.3, 1.8]`, `height_mm` 0.9 − 0.2.
      - "Spirelet": `Twist { sketch: <rectangle 1.3 on the shaft's top face>, path: <straight 2.4>, degrees: 45.0, end_scale: 0.08 }` (`cad.rs:81`; `cad/twist.rs`).
      - "Crocket": a leaf on a shaft face, `Extrude +0.3`, then `Pattern About { part: shaft, count: 4 }`.
      - "Knop": a Revolve at the spirelet tip.
      - "Lamp": `stone_on_face(.., citrine, on: shaft, FaceSeat { face: <outward face> })`, its collet (wall 0.3) and its blind bur.
      - **Seat every part at the corner placement and let the band join cluster them.** Do not Boolean-union them into one placed part: a stone on a face of an operand of a *placed* Boolean is **unverified**. P2 follows a stone through a Boolean in the record (commit `4bb2977`), but whether the Boolean's own placement moves it is not pinned.
  14. "Eight pinnacles": `Pattern { sources: Sources(vec![shaft, spirelet, crockets, knop, lamp, lamp collet, lamp bur]), kind: About { part: 10, count: 8 } }`. These are rigid turns about the amethyst's axis; they stay on the table only because it is flat (`cad/pattern.rs:44-50`).
  15. "Buttress": a sketch on a `Parting` plane at θ 130 (just past the head's end), an elevation with two weathered set-offs.
  16. `Extrude` ±0.9 (plane offset −0.9, height 1.8), Join, blend 0.3.
  17. `PressPull` on the upper set-off, −0.25.
  18. `Mirror { Section { 90 } }`.
  19. **Stamps:** 8 wall stamps (paired lancets, `cut: true`, `sink_mm: 0.3`, `along_pull: true`), one per octagon wall.
- **What it shows off:** the twisted sweep with a taper; revolve; press-pull twice; a stone on a part's face; an About array of a whole seven-part assembly (P2); a parting-plane elevation; stamps on stock. It is the most complex CAD tree in the library.
- **Traps and how they are avoided:**
  - **Seat each lamp on the shaft before anything meshes it** (house rule 3).
  - **The twist is a mesh.** Fillet, press-pull and sketch-on-face refuse it by name, so the crockets go on the shaft, not the spirelet. Its section must stand square to the path at the start (`cad/twist.rs:27`).
  - **The rim rolls:** pinnacle feet stand 1.0 inside the outline and sink 0.2.
  - **Face budget:** `MAX_PATTERN_FACES` is 2,000,000 (`cad/pattern.rs:22`), and eight twisted spirelets at the 0.015 mm export chord count toward it. The author prints the count.
  - **Windows against ribs:** keep ≥ 0.8 of metal between each window and its ribs and the boss. The windows run 5+ mm deep near the corners, which investment accepts.
  - **Height:** each pinnacle stands about 4.8 over the table's corner. The review round judges the snag risk; the fallback is shaft 1.4 and spirelet 2.0.
- **Needs:**
  - C-T1 (vault), C-T4, C-T3 (crocket leaf, lancet), P7.
  - The example carries:

```rust
fn corner_frames(d: &RingDesign) -> Result<Vec<(f64, f64, f64)>>   // (theta, across, bearing) from the stock plan
fn pinnacle_parts(doc: &mut Document, ids: &mut impl FnMut() -> Id, at: Placement) -> Result<Vec<Id>>
```

- **Template:** stock nodes, a stamps patch until P7, and about 30 `cad.feature` nodes. Stock ≤ 1 MB after P7.
  - **Controls:** Pinnacle height (the twist path's length), Twist, Lamp size, Boss stone.
- **Risk:** 5/5, high. If the seven-source About pattern is refused or slow, pattern the pinnacle's metal parts and hand-place the 8 lamps (24 features). If the face budget bites, lower the spirelet's twist to 30°.

---

## Packaging (the Reptilia format)

The lead owns packaging, after every ring passes the gates.

**Author program.** `crates/ringdesign-core/examples/tenebrae/main.rs`, with `common.rs` and one module per ring (the plan's layout; the Bestiarium lanes shipped as single files, `examples/bestiarium_<ring>.rs`, and either layout works).
- **CLI:** `NEW_DIRECTORY [SLUG] [--draft] [--verify]`.
- **Gates it asserts:** lands, rails, walls, sand slots, and zero obstructions for the sand rings at 0.100 and 0.075 mm.
- **Helpers:** `write_ring` and the gates, and `stock_at` copied from `examples/stock_masterworks.rs:317-411`.
- It refuses to overwrite an existing folder.

**Per-ring folder** `showcase/tenebrae/<slug>/`:
- `design.ring.json`, `editable-graph.ring.json`;
- `sketches/*.svg`: every sketch exported with `sketch::exchange::svg` (`sketch/exchange.rs:8`). This is the artwork, as text;
- `finished-metal.stl`;
- the pattern, one of:
  - `casting-pattern.stl` for the sand rings (shrink-compensated, bench cuts out, drill and locating dots in);
  - `investment-pattern.stl` for the wax rings;
  - and also `lid-pattern.stl` for Capsa;
- `reference-<n>-<colour>.stl` per stone;
- `report.json`, `mesh.json`, `release-fine.json` (sand), and `verification.json` (clamp bite, drag, local wall, lands, template bytes, open milliseconds per phase);
- `hero.png`, `face.png`, `palm.png`, `section.png` (Oculus and Ogiva), Blender `studio*.png`, and the reel.

**Collection level:**
- `showcase/tenebrae/README.md`: a table of ring, form, surface, process and approximate weight; the process notes (Oculus's sand-core trial; Ogiva's parts-only route; Sigillum's bench seal and marks; Capsa's two castings and hinge); and the reproduce commands below.
- `index.html`, `Tenebrae-collection.png` with a Textura title (C-T6), and a renders zip.

**Reproduce**, from the repository root in a fresh directory. The collection-generic tools are P8's; until P8 lands, write `ringdesign-graph/examples/tenebrae_templates.rs` (a copy of `reptilia_templates.rs:1-40` over the eight slugs, adding the cluster wiring) and `tools/render_tenebrae.py` / `tools/catalog_tenebrae.py` from their Reptilia versions.

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example tenebrae -- NEW_DIRECTORY
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example tenebrae -- NEW_DIRECTORY --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example collection_templates -- tenebrae NEW_DIRECTORY
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- tenebrae NEW_DIRECTORY
python3 tools/catalog_collection.py tenebrae NEW_DIRECTORY
```

**Graph templates.**
- `graphs/templates/<slug>-tenebrae.graph.json`, bundled automatically, because that directory is the `GRAPHS` family (`crates/ringdesign-assets/build.rs:29`). The Gothic clusters go in `graphs/clusters/`, listed in `BUNDLED_CLUSTERS`.
- A `pub static TENEBRAE: &[TemplateGraph]` in `ringdesign-graph/src/templates.rs`, chained into `catalog()` (`:170`).
- `group("Tenebrae collection", &[…], "Eight Gothic buildings as CAD feature histories, with stained-glass stones.")` in `ringdesign-workbench/src/templates.rs` (beside `:479-495`), placed after the starters.
- The template gate (plan section 5, item 5): a byte-for-byte cold evaluation, the same mesh, ≤ 4 `design.set` patches, and the size and open time recorded.

**Thumbnails.** 160 px menu previews from `render::finished` (`render.rs:97`): studio gold, stones set, CAD parts included.
- Add each render to `crates/ringdesign-workbench/assets/templates/sources.json`, run `tools/template_thumbnails.py`, and add an `include_bytes!` arm to `preview_bytes` (`ringdesign-workbench/src/templates.rs:501`).
- `every_authored_graph_and_collection_has_a_real_preview` requires the arm, and the preview test's count goes up by eight.

**Phone.** Bump `crates/ringdesigner-android/Cargo.toml`'s version and add the `CHANGELOG.md` entry. Verify with `cargo test -p ringdesigner_android`, the ndk check (`cargo ndk -t arm64-v8a check -p ringdesigner_android`), and on the rdsmoke AVD: open each Tenebrae template and watch the staged progress plate.

## Open questions still with Logan

1. **Capsa's inner motif.** The brief's memento-mori skull is a face, and the review rule is "no faces, no eyes". This file substitutes crossed bones under an hourglass. Confirm, or name another relic.
2. **Capsa's lid.** Should it be a real bench hinge (the default here: a Separate cast lid with a `Joint`), or cast shut with the relic seen through pierced gables? (Brief question 3; still unanswered.)
3. **Alloys.** Renders are studio gold throughout. For the sheet's weights, this file defaults to Silver 925 for the three sand rings and Gold 18k for the five wax rings, as `stock_masterworks` does; the sheet prints every alloy anyway. (Brief question 5.)
4. **Oculus's weight.** The 4.6 mm thickness needed for a 0.8 rail makes a heavy band, about 18 g of silver. The alternative is 16 lights on a 4.0 band with the outer circle pulled in to r 11.9, which gives shorter, wider lights.
