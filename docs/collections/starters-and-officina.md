# Starters and Officina: handoff plan

This file is the plan for the File > New from template starter gallery (Starter bands, Stone settings, Starter signets), the six-lesson Officina collection that replaces the Workshop collection, and platform enablers P5–P8, which are built in the same batch. It is self-contained. A reader with this file and the repo can build every entry without the session that wrote it.

- **Repo:** `/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner`. Written against branch `collections-handoff` at `30f0506`. Batch 15's merge commits (`b15-cad-stones`, `b15-skin`, `b15-stamps`, `b15-open-speed`) are already on that branch.
- **Sources:** `docs/collections/source/plan.md`, the director's plan. Its section 1 verdicts, section 3 enabler table and section 7 answers override the brief. The brief is `docs/collections/source/brief-starters.md`. The project `CLAUDE.md` is the doctrine.
- **Path shorthand:** `core/` = `crates/ringdesign-core/src/`, `graph/` = `crates/ringdesign-graph/src/`, `wb/` = `crates/ringdesign-workbench/src/`, `gui/` = `crates/ringdesign-gui/src/`, `phone/` = `crates/ringdesigner-android/src/`.
- **Line references:** every `file:line` below was checked against `30f0506`. Anything not checked is marked **(unverified)**.
- **Memory guard:** run every test under `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test ...`. This machine has no swap.

---

## 0. What binds this file

### Logan's decisions (2026-09-24)

- **Process:** the collections use a mix of sand and lost wax, and each ring is judged against its own `DraftSettings::process`. He pours both **Delft clay** and **Petrobond**. Every sand entry in this file is Delft.
- **Upright factory plans (004, 008, 009, 010, 011, 014, 018, 019, 020) are lost wax only.** The sand master mirrors the upper half of the stock, so it is used only on 001, 002, 003, 005, 006, 007, 012, 013, 015, 016 and 017. Bare starters follow the same split: sand-safe plans open as Delft sand, and upright plans open as lost wax with an "upright" badge. No off-centre parting study will be done.
- **Taste:**
  - One theme from face to palm. Palisade was retired as "random elements placed in various spots".
  - Figurative motifs are stamps with true outlines. The painted moons "look like arrows".
  - Made settings only. Height-field prongs were "quite awful".
  - No faces, no eyes.
  - Signets start from factory stock.
  - Final renders are in studio gold.
- **Starter decisions from plan §1:**
  - Court band and Braided band stay and are re-rendered.
  - Wishbone wave leaves the menu until `ShankKey::slide` (C-S6) lands. Do not ship a "Saddle wave", because the name collides with `ShankKind::Saddle`.
  - Split shank (a true Y, lost wax) and Split gallery (sand) both ship.
  - Eight stone settings.
  - Twenty bare factory stocks, badged by plan type.
  - Shouldered cushion stays. Heart and Waved hexagon leave the menu.
- **Officina decisions from plan §1:**
  - Six lessons: Rivet, Sigil, Keystone, Aile, Torsade and Fenestra.
  - Sigil moves from stock 004 to **013 at its native size**, as a 7-feature lesson. Sigillum in Tenebrae is the masterwork version of the same seal.
  - Lantern becomes **Fenestra** and drops `shank.cathedral` and `cutter.azure`. Arcus (Tenebrae) and the Cathedral solitaire starter already carry that chain.

### Platform status

| ID | Capability | State | What it means here |
|---|---|---|---|
| P1 | Async template open with staged progress | **Landed** (batch 14, hardened in batch 15) | `wb/templates.rs` `Template::open`, `Stage`, `Opened::library_for`, `Slot`. Expression pins run through `Evaluator::with_exprs`. A new `Source` arm only needs a match arm in `opened()` (`wb/templates.rs:110`). |
| P2 | CAD stones are stones | Batch 15, **merging now** | `StoneSource::Cad { feature, copy }` (`core/setstone.rs:29`), `render::finished` (`core/render.rs:97`), multi-source `Pattern` (`core/cad/pattern.rs:475`), `Gem::preview_tint`. Stones show in reports, stone maps and thumbnails. |
| P3 | Skin in core, plus `sand_master` | Batch 15, **merging now** | `core/skin.rs` and `imported_base::sand_master` (`core/imported_base/sand_master.rs:54`). The stock starters use `sand_master`. |
| P4 | Stamps v2 | Batch 15, **merging now** | `Stamp::tier` / `StampTop` (`core/setting.rs:927-950`), `core/outline.rs`, `stamp_row` (`:1581`), `parting_monotone` (`:2149`). Not used by this file's entries. |
| P5 | Station-aware side-face gates | Batch 16, **not started** | No entry here needs it. Section 3 carries its full spec. |
| P6 | Claw styles | Batch 16, **not started** | No entry here needs it. Section 3. |
| P7 | Template-weight nodes and lift | Batch 16, **not started** | Makes the stock starters and Sigil into light graphs, and `shank.key` replaces the `/shank/keys` patch on the split starters. Section 3. |
| P8 | Collection tooling | Batch 16, **not started** | Officina's renders, catalog and template packaging go through it. Section 3. |
| Starter gallery | This file, part 1 | Batch 16, **not started** | Lane 1E owns it (plan §4). |

### Process floors (`core/castability.rs:117-185`)

| Process | Draft | Min section | Min detail | Set with |
|---|---|---|---|---|
| Delft clay | 3.0° | 0.8 mm | 0.30 mm | `SandProcess::DelftClay.apply(&mut d.draft)` |
| Petrobond | 2.5° | 0.6 mm | 0.40 mm | `SandProcess::Petrobond.apply(&mut d.draft)` |
| Lost wax | not gated | 0.5 mm | 0.15 mm | `CastProcess::LostWax.apply(&mut d.draft)` |

`SandProcess::apply` does not touch `process`. It stays `SandTwoPart` unless something set it to wax. Write the process with `apply`, never by assigning `d.draft.process` alone: the old `workshop_collection.rs::base()` did that and left the sand floors in place under wax.

### Gates every entry passes (plan §5, adapted to starters and lessons)

- **Geometry:**
  - Watertight with 0 degenerate faces.
  - `csg::self_crossings == 0` on every made part.
  - `built.solids.notes` empty.
  - No CAD feature reporting `Failed` or `Skipped`.
- **Process:** `castability::judge::judged_field_report(.., Some(&built))` (`core/castability/judge.rs:118`), judged against the entry's own process.
  - **Sand:** Castable, meaning not "castable with care", with 0 obstructions and 0 unresolved rays in `manufacturing::inspect`. The only exception is Keystone, whose lesson *is* the "castable with care" verdict (section 2).
  - **Wax:** fill ≥ the investment section.
- **DFM:** `dfm::findings_in` returns 0 findings, or the finding is named in the entry's blurb. Court band and Braided band are kept as they are (see their entries).
- **Stones:** `stones::report` agrees with the gem preview count, and the crowding census is clean or explained.
- **Lessons (Officina) and bands:** studio-gold renders from `render::finished`, reviewed against the plan §5 checklist. The "no flat, blocky CAD" and "one theme" rules apply.

---

## 1. The starter gallery

**Subtitle:** "Blanks a new user reaches for first."

**Primary side of the app:** File > New from template on desktop and phone (`wb/templates.rs` `collections()` and `menu()`, shared by both apps). It covers three construction families: the procedural band with height-field layers, CAD builders on a procedural band, and imported factory stock.

**Base size:** every starter except the stocks is built on `RingDesign::default()`, which is size 7 with a **17.3 mm bore** (`RingSize(7.0)`, circumference 36.5 + 2.55·7). That is the starters' convention; the collections use 18.6 mm. Stocks take the master's calibrated bore through `ImportedBase::attach` (`core/imported_base.rs:474`).

### All entries

| Group | Entry | Epithet | Base | Process | Stones | Status |
|---|---|---|---|---|---|---|
| Bands | Court band | the blank | LowDome 4.0 × 2.0 | sand (defaults) | — | exists; re-render |
| Bands | Braided band | cords on the side faces | Flat 7.5 × 2.4 | sand (defaults) | — | exists; re-render |
| Bands | Split shank | the Y, in wax | LowDome 2.4 × 1.8, Keyframes | lost wax | — | not started; buildable now (v0), C-S2 for the builder |
| Bands | Split gallery | daylight along the pull | LowDome 4.2 × 1.8, Keyframes | Delft | — | not started; buildable now (v0), C-S3 for the builder |
| Bands | Wishbone wave | a V at the top | DShape 3.6 × 1.9, Keyframes + slide | Delft | — | off the menu; blocked on C-S6 |
| Settings | Cathedral solitaire | six claws on two arches | DShape 2.1 × 1.8, ReverseTaper 0.45 | lost wax | Round 6.5 | not started; buildable now |
| Settings | Bezel solitaire | shown finished, poured plain | LowDome 2.4 × 1.7 | Delft (collet at the bench) | Oval 7 × 5 | not started; buildable now |
| Settings | Halo | melee round a cushion on a fine shank | DShape 2.2 × 1.8, ReverseTaper 0.2 | lost wax | Cushion 6.0, ~16 × 1.2 melee, 2 × pavé runs of 1.2 | not started; buildable now |
| Settings | Trilogy | three stones, one line | DShape 2.2 × 1.9, ReverseTaper 0.3 | lost wax | Round 5.5, 2 × Oval 5 × 3.5 (pears after C-B2) | not started; buildable now with ovals |
| Settings | Toi et moi | two stones on crossing arms | LowDome 3.0 × 1.9, Bypass 1.0 | lost wax | Oval 7 × 5 (pear after C-B2), Oval 6 × 4 | not started; buildable now with ovals |
| Settings | Split-shank basket | a cage on two rails | the Split shank | lost wax | Oval 8 × 6 | not started; after Split shank |
| Settings | Half eternity | a bead-set row on the parting line | LowDome 3.0 × 2.1 | Delft | Round 1.8 × ~13 | not started; buildable now |
| Settings | Gypsy trio | three stones flush in the dome | LowDome 6.0 × 2.4 | Delft | Round 4.0 + 2 × Round 2.5 | not started; buildable now |
| Signets | Shouldered cushion signet | ornament off the table | lofted Cushion 14.5 × 2.2 | sand (defaults) | — | exists; re-render |
| Signets | 20 factory stocks | hard angles, ready for a theme | presets 001–020 | 11 Delft, 9 lost wax | — | not started; C-S1 is this lane's first task |

### Build order and why

The work runs in two parts.

**Part 1 (batch 16, lane 1E).** It owns `core/templates.rs`, the new `core/templates/settings.rs` and `core/templates/fixtures.rs`, `wb/templates.rs` (starter groups and the `thumbnails!` macro), `core/../tests/golden.rs`, `crates/ringdesign-py/tests/test_smoke.py`, and the fixture call sites. The order:

1. **C-S4 fixtures.** Move every retired builder behind `templates::fixture(name)` and rewire the call sites (list in 1.4). This lands in the same commit as the removals, or about 15 tests break.
2. **C-S1 stock starters.** 20 menu entries, no new geometry, and the largest visible gain. Its first-build cost is the only risk, so measure it here with `template_open_probe`.
3. **C-S5 per-template camera.** Needed before any thumbnail is final.
4. **Settings in risk order:**
   - Cathedral solitaire and Bezel solitaire: existing builders; `cad::examples` "claw-solitaire" at `core/cad/examples.rs:204` is most of Cathedral already.
   - Half eternity and Gypsy trio: height-field seats with `solid`, measured patterns.
   - Halo, then Trilogy, then Toi et moi: crowding and claws on thin shanks.
5. **Split gallery v0** (Extrude + Mirror), then **Split shank v0** (Loft), then **Split-shank basket**. The loft v0 is the riskiest step of part 1.
6. **Re-render** Court, Braided and Shouldered cushion. Regenerate every thumbnail and the golden corpus.

**Part 2 (batch 17, after P6 merges; plan's batch 2).** P6 owns `core/cad/builders.rs`, so the new builders wait for it.

1. **C-S2 `cutter.split` and C-S3 `cutter.window`** in `core/cad/builders/cutters.rs` plus their `SPECS` rows. Swap both split starters and the basket from v0 to the builders.
2. **C-S6 `ShankKey::slide`** (optional). It brings Wishbone wave back.

### Enablers carried by the starters (C-S1 to C-S6)

| ID | What | Size | Lands in |
|---|---|---|---|
| C-S1 | Stock starters with process chosen by plan symmetry | S (≤ 1 day) | part 1 |
| C-S2 | `cutter.split` builder | M | part 2 |
| C-S3 | `cutter.window` builder | M | part 2 |
| C-S4 | Fixtures for retired templates | S | part 1 |
| C-S5 | Per-template thumbnail camera | S | part 1 |
| C-S6 | `ShankKey::slide` | M, optional | part 2 |

**C-S1: stock starters.** Two pieces of core API and one workbench arm.

```rust
// core/imported_base.rs, beside PRESETS (:1095)
impl Preset {
    /// The plan's two halves agree to 0.6 mm across the parting plane, so the sand master
    /// (which mirrors the upper half) does not rewrite it. Measured in plan.md F2.
    pub fn sand_safe(&self) -> bool {
        matches!(self.id, "001"|"002"|"003"|"005"|"006"|"007"|"012"|"013"|"015"|"016"|"017")
    }
}

// core/templates.rs (new fn; the workbench, CLI and tests all call it)
pub fn stock(preset: &'static Preset) -> anyhow::Result<RingDesign> {
    let sand = preset.sand_safe();
    let source = preset.load()?;
    let mut d = RingDesign::default();
    ImportedBase::attach(&mut d, if sand { sand_master(source)? } else { source })?;
    d.imported_base.as_mut().unwrap().sand_envelope = sand;
    // From here, mirror crates/ringdesign-core/examples/common/probe.rs:45-60 (`stock`):
    // keep the face (length, width) attach set, Flat style, edge_round 0.3, comfort 0.1,
    // then the chart taken from THIS stock before any layer is drawn.
    ...
    if sand { SandProcess::DelftClay.apply(&mut d.draft) } else { CastProcess::LostWax.apply(&mut d.draft) }
    d.name = stock_name(preset); // "Octagon signet · 015"; the cushions carry "(20 mm)" / "(10 mm)"
    Ok(d)
}

// wb/templates.rs:26: one more Source arm, one more match arm in opened() (:110)
enum Source { Graph(..), Document(..), Starter(..), Design(..), File(..), Stock(&'static Preset) }
Source::Stock(p) => { set(Stage::Reading); ringdesign_core::templates::stock(p)? }
```

- **Sand-master naming:** `sand_master` renames the source to `"{name} / drafted workshop master"` (`core/imported_base/sand_master.rs`, near its end). Anything that matches a design back to its stock must accept both names: the test below, and P7's lift recognition.
- **Pin the symmetry list with a test.** Re-measure, for every preset, the maximum |z⁺ − z⁻| over head vertices per 0.5 mm station, and assert `sand_safe()` exactly when that value is ≤ 0.6 mm. Plan F2's numbers: 004 7.8, 008 6.3, 009 2.5, 010 7.7, 011 3.7, 014 9.0, 018 1.7, 019 7.9, 020 4.1; the rest are ≤ 0.6.
- **Fields added to `wb::templates::Template`:**
  - `badge: Option<&'static str>`: "sand-safe plan" or "upright · lost wax", drawn as a weak label at the end of the menu row.
  - `family: Option<&'static str>`: `menu()` draws a weak heading whenever it changes inside a collection's scroll area.

**C-S2: `cutter.split`.** A slot through the band from the outer surface to the bore, so the band parts into two rails.

```rust
pub const SPLIT: &str = "cutter.split";
// SPECS row: Spec { key: SPLIT, label: "Split", role: ComponentRole::Shank, attach: Attach::Cut,
//                   reference: false, on_stone: false, hint: "A slot from crest to bore that parts the band into two rails" }
// schema:
//   theta_deg      0..360, default 90   (the centre of the split)
//   spread_deg     20..80, default 48   (half the split's arc; it closes to a point at theta ± spread)
//   gap_mm         0.3..6, default 2.6  (the slot's width at the centre)
//   rail_round_mm  0..0.6, default 0.3
//   tip            "Point" | "Round"
```

- **Construction:** a closed loft of radial rectangles at ≤ 2° per station. Each rectangle spans from 0.8 mm inside the bore to 0.8 mm past the crest, read per station through `Bore::of` (`core/cad/builders.rs:585/598`). The width follows a smootherstep from 0 at the tips to `gap_mm` at `theta_deg`, and the ends are capped.
- **Refusal:** it refuses by station name wherever a rail (half of the band's width at that station minus half the gap) falls below `MIN_EDGE_MM`.
- **Stage:** Cast under wax. Under sand it refuses to be Cast ("a true split is two crests; judge it for lost wax"); it does not silently stage itself Bench.

**C-S3: `cutter.window`.** A window through the band along the pull, the sand-legal read of a split.

```rust
pub const WINDOW: &str = "cutter.window";
// SPECS row: role Shank, attach Cut, on_stone false
// schema: from_deg, to_deg, rail_in_mm (0.5..3, 1.0), rail_out_mm (0.5..3, 1.0),
//         draft_deg (0..5, 2), tip_round_mm (0..1, 0.35)
```

- **Outline:** per station, r_in(θ) = the bore crossing + `rail_in_mm` and r_out(θ) = the outer crossing − `rail_out_mm`, both from `Bore::crossings`. The tips round by `tip_round_mm`.
- **Solid:** two drafted frusta from the parting plane out through each side face. They overlap by 0.1 mm across z = 0 rather than meeting face to face, because coincident faces are a degenerate boolean (doctrine: "a coincident floor is a degenerate boolean").
- **Stage:** `cutters::cut_stage([0.0, 0.0, 1.0], sand)` (`core/cad/builders/cutters.rs:1067`), which is Cast in sand, because the window runs along the pull.

**C-S4: fixtures.** `core/templates/fixtures.rs`:

```rust
/// A retired starter kept for tests: not in the menu, not bundled as a graph.
pub fn fixture(name: &str) -> Option<RingDesign>;
// "Heart signet", "Waved hexagon signet", "Cathedral solitaire stock",
// "Toi et moi (two heads)", "Split channels" (the old ShankKind::Split starter), "Wishbone wave"
```

The builders move out of `TEMPLATES` unchanged, so every test keeps its geometry. The call sites are in 1.4.

**C-S5: per-template camera.**

- Add `view: (f64, f64)` (yaw, pitch) to `core::templates::Template`, default `(0.55, 1.12)`. `core/examples/template_shots.rs` reads it in place of the constant.
- For stocks, template_shots gets a second loop over `PRESETS` that calls `templates::stock` and uses `(0.55, 1.12)`.
- For Officina, the views live in P8's per-collection `collection.json`.
- Values: Braided band `(0.55, 0.6)`, Split shank `(0.0, 1.35)` (from above, so the Y reads), Split gallery `(0.25, 0.35)` (side-on, so the daylight shows), Toi et moi `(0.55, 1.25)`, Half eternity and Gypsy trio `(0.55, 1.0)`.

**C-S6: `ShankKey::slide`** (`core/profile.rs:1747`).

```rust
pub struct ShankKey { pub theta_deg: f64, pub width_scale: f64, pub thickness_scale: f64, pub crown_scale: f64,
                      #[serde(default)] pub slide: f64 } // along the finger, fraction of the half-width, clamped ±0.6
```

- Keyframes interpolates `slide` with the other keys (periodic Catmull-Rom) into `ShankMod::z_center_frac`, using the crest bias Wave already uses so the crest stays on the parting plane. Wave's own term is at `core/profile.rs:2623`.
- The ±0.6 cap is Wave's measured cap, where the undercut converges to phantom scale.
- A band whose slides are all 0 is bit-identical to today (pin it on the golden corpus).
- P7's `shank.key` node carries `slide` once this lands.

### 1.1 Starter bands

## Court band — *the blank*

- **Status:** exists (`core/templates.rs:68`). Re-render only.
- **Concept:** a plain comfort-fit court band. It is the blank that every other band starts from, and the row icon for Starter bands.
- **Theme face to palm:** plain throughout. Bore comfort fit.
- **Base:** `ProfileStyle::LowDome`, 4.0 × 2.0, size 7 (17.3 mm bore).
- **Process:** sand with `DraftSettings::default()`. Leave it there: the MCP test pins "the Court band built by tools alone evaluates to the code template byte for byte". Naming Delft would change its draft and break that pin for no gain.
- **Stones:** none.
- **Build, step by step:** unchanged.
- **What it shows off:** the procedural band.
- **Traps and how they are avoided:** none.
- **Needs:** C-S5 (camera `(0.55, 1.12)`), and the studio-gold re-render through `render::finished` (P2, merging).
- **Template:** already bundled (`graphs/templates/court-band.graph.json`, 1 KB).
- **Risk:** none.

## Braided band — *cords on the side faces*

- **Status:** exists (`core/templates.rs:114`). Re-render only.
- **Concept:** braid cords on both squared side faces, with milgrain riding the crest.
- **Theme face to palm:** the braid runs the full circle on both side faces. A crest milgrain of 130 beads × 0.5 mm. The palm carries the same.
- **Base:** Flat 7.5 × 2.4, `flatten_sides`, size 7.
- **Process:** sand, defaults.
- **Stones:** none.
- **Build, step by step:** unchanged.
- **What it shows off:** the side-face doctrine (relief there pulls straight out), and milgrain on the crest line.
- **Traps and how they are avoided:** it carries a known DFM finding. The doctrine measures the Braid alpha's gaps at 0.04 mm at this tile size: the cords merge in the pour. Either open the gaps through the layer's bias and contrast until `dfm::findings_in` is clean (preferred), or say "cords merge softly in the pour" in the blurb. A bigger tile will not fix it: it would need about 7× the cell.
- **Needs:** C-S5 (pitch 0.6, so the braid on the side face reads), and a re-render.
- **Template:** bundled already (3.5 KB).
- **Risk:** low.

## Split shank — *the Y, in wax*

- **Status:** not started. Buildable now as v0 on existing operations. The final form uses C-S2.
- **Concept:** a plain band that parts into two rails from the shoulders to the top, open to the air between them. Seen from above it is a Y at each shoulder. It replaces the old `ShankKind::Split` starter, which widened the band and grooved both side faces. The thumbnail camera looked down at the crest, so that starter read as "not even sure what I'm looking at".
- **Theme face to palm:**
  - Top: two rails, 2.0 mm each, with a 2.6 mm gap.
  - Shoulders: the rails close to a point at 42° and 138°.
  - Palm: plain 2.4 mm.
  - Bore: comfort fit 0.1.
- **Base:** size 7 (17.3 mm). `ProfileStyle::LowDome` 2.4 × 1.8, `comfort_fit_mm` 0.1. `ShankKind::Keyframes`, `amount` 1.0, with these keys (`ShankKey`, `core/profile.rs:1747`):

  | θ | width_scale | thickness_scale |
  |---|---|---|
  | 270, 200, 340 | 1.0 | 1.0 |
  | 150, 30 | 1.35 | 1.0 |
  | 125, 55 | 2.0 | 1.05 |
  | 90 | 2.75 (6.6 mm) | 1.1 |

- **Process:** lost wax, set with `CastProcess::LostWax.apply(&mut d.draft)`. A true split is two crests, "a valley no single parting plane clears" (doctrine). The judge reports each rail's inner wall as an undercut, which under wax does not gate.
- **Stones:** none.
- **Build, step by step:** the CAD document comes from `core::templates::settings::split_doc(&RingDesign) -> cad::Document`. The name keeps the settings module the one home for starter documents.
  1. `Operation::Band`.
  2. **v0:** `Operation::Loft { sections }` through 7 inline rectangle sketches.
     - Each lies on a radial plane at θ = 42, 58, 74, 90, 106, 122, 138: `Sketch { plane: Workplane { origin: [0,0,0], x: [cosθ, sinθ, 0], y: [0,0,1], on_face: None }, .. }`.
     - Each spans x = 7.85 to 11.45 (0.8 mm inside the 8.65 bore radius and 0.8 mm past the 10.63 crest radius at the top).
     - The y half-gaps are 0.025, 0.55, 1.05, 1.3, 1.05, 0.55, 0.025. The full gaps are 0.05, 1.1, 2.1, 2.6, 2.1, 1.1, 0.05.
     - `Component { attach: Attach::Cut, stage: Stage::Cast, blend_mm: 0.25, .. }`: the seam bead rounds the rims.
  3. **Final:** replace step 2 with `cutter.split { theta_deg: 90, spread_deg: 48, gap_mm: 2.6, rail_round_mm: 0.3, tip: "Point" }` (C-S2).
- **What it shows off:** Keyframes, a CAD cut part on a procedural band, the seam bead, and the verdict reporting an undercut that does not gate under lost wax.
- **Traps and how they are avoided:**
  - The loft needs matching curve counts, 2–32 sections (**(unverified)**: the brief cites `core/cad.rs:1893-1910`).
  - The 0.05 mm end sections are slivers. If the loft refuses them, drop them and let the 58° and 122° sections close into a round tip with a 0.3 mm end radius. If that fails too, wait for C-S2.
  - Chord sag between 16° sections is about 0.11 mm at r 11.45, covered by the 0.8 mm padding.
  - Fill: rails at the top are 2.0 × 1.98 mm, far over the investment's 0.5 mm.
- **Needs:**
  - C-S2 for the final form.
  - C-S5 (camera `(0.0, 1.35)`).
  - P7's `shank.key` so the graph needs no `/shank/keys` patch.
- **Template:** a code starter plus a graph builder in `graph/templates.rs`: `band.profile` → `shank` → `/shank/keys` patch (until P7) → `nodes::cad::chain_document(&mut g, last, &doc)` (`graph/nodes/cad.rs:254`). Set `g.next_id` above the feature ids first, as the lift does. It is pinned byte for byte against the code starter by the existing `BUNDLED`-vs-code test (`graph/templates.rs:846`). About 6 KB.
- **Risk:** medium on v0, low with C-S2. Fallback: ship with C-S2 only and hold the entry back from part 1.

## Split gallery — *daylight along the pull*

- **Status:** not started. Buildable now as v0. The final form uses C-S3.
- **Concept:** seen side-on, the top of the band opens into two rails, an outer rail arching over an inner one, with daylight between. The window runs through the band along the finger axis, which is the pull direction, so it casts in sand. This teaches the pull doctrine next to the wax Y.
- **Theme face to palm:**
  - Top: a crescent window between two 1.0 mm rails, over θ 38–142.
  - Palm: a plain 1.8 mm band.
  - Bore: plain.
- **Base:** size 7. LowDome 4.2 × 1.8. Keyframes, `width_scale` 1.0 throughout, with thickness_scale:

  | θ | thickness_scale |
  |---|---|
  | 270, 200, 340 | 1.0 |
  | 150, 30 | 1.35 |
  | 120, 60 | 1.85 |
  | 90 | 2.3 (top 4.1 mm deep) |

  The keys follow from the window: thickness(θ) = (r_out(θ) + 1.0 − 8.65) / 1.8, where r_out is the outer arc's polar radius. At 90° that gives (11.75 + 1.0 − 8.65) / 1.8 = 2.28. At 60° it gives (10.99 + 1.0 − 8.65) / 1.8 = 1.86.
- **Process:** Delft, set with `SandProcess::DelftClay.apply`. The window is the bore's case in the doctrine, a through-hole along the pull. Its walls are drafted 2° either way from the parting plane, so each mould half lifts its own half of the sand core.
- **Stones:** none.
- **Build, step by step (v0):**
  1. `Operation::Band`.
  2. `Operation::Plane { base: PlaneBase::Parting, offset_mm: -0.05 }` (`core/cad/pattern.rs:70-80`).
  3. `Operation::Sketch` "Window" on 2, `Workplane { on_face: Some(FaceAnchor { feature: 2, face: FaceRef::bare(0) }), .. }`. A sketch anchored to a work plane resolves through `pattern::on_plane` (`core/cad.rs:1604-1605`); the face ordinal is ignored. The sketch is a lens of two `Geometry::Arc`s:
     - Inner arc: centre (0, 0), r 9.65, from θ 38 to θ 142. The tips are (±7.604, 5.941).
     - Outer arc: through both tips and (0, 11.75). That is centre (0, 3.868), R 7.882, computed here; the brief's "≈ 8.1 about a centre ≈ 3.6 up" was approximate.
  4. `Operation::Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: 2.65, draft_deg: -2.0 }`. A negative draft widens toward the +Z face. `attach: Cut`, `stage: Cast`. It runs from z = −0.05 through the +Z face at 2.1.
  5. `Operation::Pattern { sources: [4], kind: PatternKind::Mirror { plane: MirrorPlane::Band } }` (`core/cad/pattern.rs:52-62`), Cut, Cast. The two cuts overlap by 0.1 mm across z = 0, so no faces coincide.
  6. **Final:** replace 2–5 with `cutter.window { from_deg: 38, to_deg: 142, rail_in_mm: 1.0, rail_out_mm: 1.0, draft_deg: 2, tip_round_mm: 0.35 }` (C-S3).
- **What it shows off:** the pull doctrine made visible, keyframed thickness, draft either side of the parting plane, a mirrored cut, and `cut_stage` staying Cast along the pull.
- **Traps and how they are avoided:**
  - **(a) The verdict cannot see the rails.** "The verdict is radial and cannot see the axial web" (doctrine). The template's test must measure the rails with `manufacturing::inspect` (`core/manufacturing/mod.rs:568`) and assert each is ≥ 0.8 mm (Delft section).
  - **(b) Sand strength.** The core is a slab about 2.1 × 15 × 4.2 mm. The blurb and the report say "mould trial required", because the slot checks do not compute sand strength.
  - **(c) Tip rounding.** v0 has sharp tips, so round them by adding a 0.35 mm `Arc` fillet at each tip in the sketch.
  - Walls facing round the ring do not arise: every window wall is extruded along Z.
- **Needs:** C-S3 for the final form, C-S5 (camera `(0.25, 0.35)`), and P7 `shank.key`.
- **Template:** a code starter plus a graph builder, as for Split shank. About 5 KB.
- **Risk:** low with C-S3, medium on v0 (the arc fitting and the mirror boolean). Fallback: raise the plane offset overlap to 0.2 mm if `Snag::Degenerate` still fires after the eight nudges.

## Wishbone wave — *a V at the top* (off the menu)

- **Status:** removed from the menu in part 1 and kept as `fixture("Wishbone wave")`. Blocked on C-S6.
- **Why it is off:** `ShankKind::Wave` with `waves = 1` sets `z_center_frac = 0.6·k·sin(θ − 90°)` (`core/profile.rs:2623`). One sine period puts each edge on a *planar* circle, tilted about 4°, so the band reads as a plain band seen at an angle. No camera fixes it.
- **Return plan:** DShape 3.6 × 1.9, Keyframes, all scales 1.0, with `slide` −0.5 at θ 90, 0 at 65 and 115, and 0 at 270. Delft. It is a V dip at the top only, to nest against a solitaire's head. It counts as a 58th menu entry when it returns.
- **Traps:** keep |slide| ≤ 0.6 (Wave's measured cap). Assert the field is 0.000% or a crest-line phantom, and that a top view shows the V.

### 1.2 Stone settings

**Subtitle:** "Eight made settings · three pour in sand, five in lost wax".

Rules common to all eight:

- **Made settings only.** Stones go through the CAD builders (`core/cad/builders.rs:14-35`, `SPECS` at `:60`), or through `SeatPadLayer::solid` (`core/field.rs:1188`, `SolidKind` at `core/setting.rs:21`). No height-field prongs.
- **Placement:** `builders::stone_feature(id, gem, Placement::ring(90.0, builders::stand_off_mm(key, gem)))` (`core/cad/builders.rs:954`, `:504`). `stand_off_mm` returns the bezel's depth for `"head.bezel"`/`"bezel"`, and pavilion + 0.2 mm culet clearance for anything else.
- **Orientation:** a part's x axis runs along the finger (`core/cad.rs:441`). An elongated stone therefore lies north-south at `spin_deg` 0, and along the ring at `spin_deg` ±90.
- **Heads:** use `builders::feature_on(id, name, key, stone, params)` (`:966`) or `builders::setting_features(key, stone, gem, sand, &mut next)` (`:971`). Its keys are `claw4`, `claw6`, `bezel`, `basket` and `halo`. Under sand it stages the head and the bur `Bench`; under wax, `Cast`.
- **Other builders:** `cutters::cathedral_feature` (`core/cad/builders/cutters.rs:1176`) and `cutters::azure_feature` (`:1167`). Their `head` parameter is the head feature's id (`builders::HEAD`).
- **Code shape:** each document comes from `core::templates::settings::<ring>(&RingDesign) -> anyhow::Result<cad::Document>`. The code starter and the graph builder (through `chain_document`) share it, so they stay equal byte for byte. Follow the "claw-solitaire" example at `core/cad/examples.rs:204-213`.
- **Tests:** each is judged with `judged_field_report(.., Some(&built))`, with `built.solids.notes` empty. P2's `stones::report` count must equal `gems::preview_mesh`'s stone count.
- **Pears:** a pear's `plan_pow` is 2.0 (`core/gem.rs`), an ellipse. True pear plans are C-B2 (plan §3b, batch 3B). Until then, Trilogy and Toi et moi carry ovals of the pear's size, `Gem { cut: GemCut::Oval, w_mm, l_mm, ..Gem::calibrated(GemCut::Oval, w_mm) }`. Swap them to `GemCut::Pear` when C-B2 lands.

## Cathedral solitaire — *six claws on two arches*

- **Status:** not started. Buildable now.
- **Concept:** a carat round in six claws, carried on cathedral arches over a fine tapered band. It is the plain member of the cathedral chain; Arcus (Tenebrae) is the rich one. It replaces "Cathedral solitaire stock", whose height-field prongs Logan rejected and whose thumbnail showed no stone.
- **Theme face to palm:**
  - Head: stone and claws.
  - Shoulders: the two arches and four teardrop azures under the stone.
  - Side faces, palm and bore: plain.
- **Base:** size 7. DShape 2.1 × 1.8, `ShankKind::ReverseTaper`, `amount` 0.45. That gives 1.67 mm wide at the top: width_scale = 1 − 0.45·k·(1 − d) (`core/profile.rs:2657`).
- **Process:** lost wax.
- **Stones:** Round 6.5, `Gem::calibrated(GemCut::Round, 6.5)` (1.00 ct).
- **Build, step by step:**
  1. `Band`.
  2. `stone_feature(2, gem, Placement::ring(90.0, stand_off_mm("claw6", gem)))`.
  3. `feature_on(3, "Six-claw head", CLAW, 2, json!({"prongs": 6}))`, stage Cast.
  4. `cathedral_feature(4, 2, 3, Stage::Cast)`, then set its params `spread_deg` 30 and `rise` 0.7 (schema defaults are 32 and 0.6, `core/cad/builders.rs:188-192`).
  5. `azure_feature(5, 2, Some(3), 4, Stage::Cast)`. The shape defaults to Teardrop.
  6. `feature_on(6, "Seat bur", BUR, 2, json!({"through": true}))`, Cast.
- **What it shows off:** the whole claw-head builder chain in the timeline, in dependency order.
- **Traps and how they are avoided:**
  - The claw feet must reach metal (`FOOT_SINK_MM`). The arches are what carry the head over a 1.67 mm top.
  - The culet sits 0.2 mm clear (`CULET_CLEAR_MM`, `:44`).
  - The azures take `head` so their windows clear the claws.
- **Needs:** P2 (merging) for the stones in the report and thumbnail. P7's `cad.op.*` nodes would expose Stone, Claws and Spread as graph controls.
- **Template:** code starter plus graph builder via `chain_document`. About 8 KB.
- **Risk:** low. The claw-solitaire example is this without the arches and azures.

## Bezel solitaire — *shown finished, poured plain*

- **Status:** not started. Buildable now.
- **Concept:** an oval in a collet on a plain band. It is the one starter that teaches the two-stage model:
  - The ring is shown finished, but the pattern exports the bare band.
  - The verdict says the collet is a bench part: under sand, bench parts are not judged.
  - Switching the process to lost wax flips both parts to Cast.
- **Theme face to palm:** the collet at the top; plain everywhere else.
- **Base:** size 7. LowDome 2.4 × 1.7, `ShankKind::Uniform`.
- **Process:** Delft. The collet is soldered and the seat cut after the pour. Plan §1 kept the brief's choice.
- **Stones:** Oval 7 × 5, north-south (`spin_deg` 0): `Gem { l_mm: 7.0, ..Gem::calibrated(GemCut::Oval, 5.0) }`. The default oval aspect is 1.6 (`core/gem.rs:81`), so `l_mm` is set explicitly.
- **Build, step by step:**
  1. `Band`.
  2. `stone_feature(2, gem, Placement::ring(90.0, stand_off_mm("head.bezel", gem)))`.
  3. and 4. `setting_features("bezel", 2, gem, true, &mut next)`. That yields `head.bezel` (Bench) and `seat.bur {through: false}` (Bench) (`core/cad/builders.rs:971-998`).
- **What it shows off:** the finished-versus-pattern split; `manufacturing::prepare` leaving bench parts out of the sand pattern; the process switch.
- **Traps and how they are avoided:**
  - The collet overhangs a 2.4 mm band. That is fine for a soldered head; `under_wall` reads the metal under it (`core/cad/builders.rs:512`).
  - Test that the exported pattern equals the bare band's mesh.
- **Needs:** P2 (merging).
- **Template:** code plus graph builder. About 5 KB.
- **Risk:** low.

## Halo — *melee round a cushion on a fine shank*

- **Status:** not started. Buildable now.
- **Concept:** a cushion in four claws, ringed by melee in small collets, with bead-set pavé running down each shoulder.
- **Theme face to palm:**
  - Head: the stone and its halo.
  - Shoulders: one bead-set pavé run each side, on the crest line.
  - Side faces, palm and bore: plain.
- **Base:** size 7. DShape 2.2 × 1.8, ReverseTaper 0.2, about 2.0 mm at the top. The brief had DShape 2.0 × 1.8 at ReverseTaper 0.3, but that leaves 1.3 mm pavé on a ~1.75 mm crest with 0.2 mm either side, right at `MIN_EDGE_MM`.
- **Process:** lost wax.
- **Stones:** Cushion 6.0 centre; about 16 melee of 1.2 (the halo solves its own count when `count` is 0); two pavé runs of Round 1.2.
- **Build, step by step:**
  1. `Band`.
  2. `stone_feature(2, Gem::calibrated(GemCut::Cushion, 6.0), Placement::ring(90.0, stand_off_mm("claw4", gem)))`.
  3. to 5. `setting_features("halo", 2, gem, false, ..)`: a four-claw head, `halo`, and `seat.bur {through: true}`. Then set the halo's params to `{"melee_mm": 1.2, "gap_mm": 0.3, "bridge_mm": 0.25, "style": "Bezel"}`.
  6. Two `LayerEntry::new("Shoulder pavé, right|left", Layer::SeatRun(SeatRunLayer { seat, gem, bridge_mm: 0.35, .. }))`:
     - `seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.2, crown: 1.0, blend_mm: 0.3, solid: SolidKind::Bead, .. }`, followed by `seat.fit_stone(gem)`, then `run.solve_spacing(&ctx)` (`core/field.rs:1999`).
     - Windows: `Window { fade_deg: 1.0, ..Window::around(90.0 ± 50.0, 32.0) }`, covering 34° to 66° off the top.
- **What it shows off:** CAD builders and height-field seat runs in one ring, and P2's crowding census across both.
- **Traps and how they are avoided:**
  - **The brief's windows collided with the halo.** `Window::around(90 ± 42, 44)` starts the pavé 20° off the top. The halo reaches about 3.0 (the cushion's half-length) + 0.3 (gap) + 1.2 (melee) + 0.3 (collet wall) ≈ 4.8 mm, which is about 27° at a crest radius near 10.4 mm. Measure the built halo's reach and keep at least 1 mm of bare band before the first pavé seat.
  - "A bead row rides the crest line only" (doctrine), which is why the runs sit at `crest_v_mm`.
  - A halo on a 2 mm shank is exactly what the crowding census exists to check. It must be clean.
- **Needs:** P2 (merging), so that CAD melee appears in the census.
- **Template:** code plus graph builder; the runs lift as layer nodes. About 12 KB.
- **Risk:** medium. Fallback: drop the pavé runs and ship halo plus plain shank.

## Trilogy — *three stones, one line*

- **Status:** not started. Buildable now with ovals; pears after C-B2.
- **Concept:** a round centre with two side stones pointing toward it, each in its own claw head.
- **Theme face to palm:** three heads over the top 56°; plain elsewhere.
- **Base:** size 7. DShape 2.2 × 1.9, ReverseTaper 0.3.
- **Process:** lost wax.
- **Stones:** Round 5.5 at θ 90. Two side stones at θ 62 and θ 118, `spin_deg` ±90 so their long axes lie along the ring:
  - v1: `Gem { l_mm: 5.0, ..Gem::calibrated(GemCut::Oval, 3.5) }`.
  - After C-B2: `GemCut::Pear` 5 × 3.5 with the points toward the centre. Which sign of `spin_deg` points the tip inward is **(unverified)**: check it on the first render.
- **Build, step by step:**
  1. `Band`.
  2. Centre stone.
  3. `head.claw {prongs: 4}` on the centre stone.
  4. `seat.bur {through: true}` on the centre stone.
  5. to 10. The same three features for each side stone, with `head.claw {prongs: 3}` (**(unverified)**: that 3 claws hold an oval of this size well; fall back to 4).
- **What it shows off:** computed spacing, and the pairwise crowding census on CAD stones.
- **Traps and how they are avoided:**
  - Spacing, from the brief: girdle radii are about 12.95 mm (centre) and 12.15 mm (sides). At ±28° the centre-to-side distance is 6.12 mm, against 2.75 + 2.5 + claw ≈ 5.95 mm needed. At ±21° they collide (5.70 mm). Keep ±28°.
  - The claw feet on a 1.9 mm top may land beside the band. Check the claw census (`built.solids.notes`). The fallback is `shank.cathedral` under the centre head only, or width 2.6.
- **Needs:** P2 (merging); C-B2 for the pears.
- **Template:** code plus graph builder. About 12 KB.
- **Risk:** medium.

## Toi et moi — *two stones on crossing arms*

- **Status:** not started. Buildable now with ovals.
- **Concept:** a bypass band whose two arms each end under a stone, the stones lying along the ring past each other. It replaces the old two-signet-head Toi et moi, which carried no stones.
- **Theme face to palm:** the two stones over the crossing; the arms' seam channel on the side faces (Bypass's own); plain palm.
- **Base:** size 7. LowDome 3.0 × 1.9, `ShankKind::Bypass`, `amount` 1.0. The arms sit at ±0.45 of the half-width (`BYPASS_OFFSET`, `core/profile.rs:2321`).
- **Process:** lost wax.
- **Stones:** both lie along the ring (`spin_deg` 90).
  - Arm A: Oval 7 × 5 (a pear 7 × 5 after C-B2) at θ 107, `across_mm` +0.7.
  - Arm B: Oval 6 × 4 at θ 73, `across_mm` −0.7.
- **Build, step by step:**
  1. `Band`.
  2. Stone A, with `Placement::Ring { theta_deg: 107.0, across_mm: 0.7, height_mm: stand_off_mm("claw4", a), spin_deg: 90.0, .. }`.
  3. `head.claw {prongs: 4}` on stone A.
  4. `seat.bur {through: true}` on stone A.
  5. Stone B at θ 73, across −0.7.
  6. `head.claw {prongs: 4}` on stone B.
  7. `seat.bur {through: true}` on stone B.
- **What it shows off:** the bypass plan outline, stones placed off the centre line, and claws on a crossing.
- **Traps and how they are avoided:**
  - At 17° past the top both arms are present and the section is 4.35 mm wide, so there is metal under both heads. At 14° the stones would collide.
  - Which arm rides +Z at θ 107 is **(unverified)**. Sample `modulation_at(107)` and put each stone over its own arm's crest; if they are swapped, negate both `across_mm`.
  - Rosa mortua (Vepres) was moved to a round hip so it does not repeat this pairing (plan §2).
- **Needs:** P2 (merging); C-B2 for the pear.
- **Template:** code plus graph builder. About 9 KB.
- **Risk:** medium, from the claw feet on the crossing.

## Split-shank basket — *a cage on two rails*

- **Status:** not started. Buildable after Split shank v0; final form after C-S2.
- **Concept:** a 1.5 ct oval held in a four-claw basket whose feet stand on the two rails of the Split shank.
- **Theme face to palm:** stone and basket over the split top; rails to the shoulders; plain palm.
- **Base:** the Split shank's design (same keys and cut).
- **Process:** lost wax.
- **Stones:** Oval 8 × 6, north-south (`spin_deg` 0): `Gem { l_mm: 8.0, ..Gem::calibrated(GemCut::Oval, 6.0) }`.
- **Build, step by step:**
  1. `Band`.
  2. The split cut (the Loft v0, or `cutter.split` later).
  3. `stone_feature(3, gem, Placement::ring(90.0, stand_off_mm("basket", gem)))`.
  4. `feature_on(4, "Basket", BASKET, 3, json!({"prongs": 4, "rails": 2}))`.
  5. `seat.bur {through: true}`.
- **What it shows off:** a head standing on a CAD-cut band; the basket builder.
- **Traps and how they are avoided:**
  - The claws sit at the plan's diagonals, about ±2.1–2.8 mm across. The rails run from ±1.3 to ±3.3, so the feet land on metal. If the census says a foot came down in the gap, turn the stone 45° or use 6 prongs.
  - The basket's base rail straddles the gap. Render-check it before signing off.
- **Needs:** Split shank (and C-S2 for its final form); P2 (merging).
- **Template:** code plus graph builder. About 9 KB.
- **Risk:** medium.

## Half eternity — *a bead-set row on the parting line*

- **Status:** not started. Buildable now.
- **Concept:** a row of small rounds bead-set along the top half of the ring, on the crest line, cast in sand as stock with drill-start dots.
- **Theme face to palm:** the row over the top 160°; the plain band finishes the circle.
- **Base:** size 7. LowDome 3.0 × 2.1. The brief had 2.8 × 2.1, but `fit_stone` gives a Boss 1.2 mm wider than its stone (`core/field.rs:1335-1340`), and a 2.0 stone then needs a 3.2 mm pad on a 2.8 mm band.
- **Process:** Delft.
- **Stones:** Round 1.8 × about 13 (from `solve_spacing`).
- **Build, step by step:**
  1. `let ctx = d.field_context();`
  2. `let mut seat = SeatPadLayer { v_mm: ctx.crest_v_mm, style: SeatStyle::Boss, height_mm: 0.2, crown: 1.0, blend_mm: 0.35, solid: SolidKind::Bead, mark_mm: 0.7, .. }; seat.fit_stone(gem); seat.diameter_mm = 1.8 + 0.9;`. The last assignment is the commissions rule "spot mounds carry gem + 0.9", so seats at column pitch do not merge into a ridge.
  3. `SeatRunLayer { seat, gem, count: 13, bridge_mm: 0.5, taper: 0.0, taper_theta_deg: 90.0, shared_prong_mm: 0.0, tilt_deg: 0.0 }`, then `solve_spacing(&ctx)`.
  4. `LayerEntry { window: Window { fade_deg: 1.0, ..Window::around(90.0, 160.0) }, .. }`.
- **What it shows off:** a stone column along the parting plane; sand patterns with raised drill dots (`mark_mm`); `pattern_parts` leaving the bead solids out.
- **Traps and how they are avoided:**
  - "A gem column must run along the parting plane" (doctrine: commissions). The crest line is that plane.
  - A pit locks, so the drill mark is raised.
  - **Culet bridge:** loss ≈ p·pavilion / r = 2.3 × 0.78 / 10.75 ≈ 0.17 mm, so a 0.5 mm girdle bridge leaves about 0.33 mm at the culet. That is over Delft's 0.30 detail floor. The brief's 0.35 would leave about 0.18.
  - The seat skirt is its finest DFM feature (read at 0.85 metal scale). A 0.35 mm blend reads about 0.30: check `dfm::findings_in`, and raise it to 0.4 if flagged.
- **Needs:** nothing new.
- **Template:** code plus graph builder (layer nodes only). About 4 KB.
- **Risk:** low.

## Gypsy trio — *three stones flush in the dome*

- **Status:** not started. Buildable now.
- **Concept:** three stones flush-set in low gypsy mounds on a wide dome.
- **Theme face to palm:** three mounds on the crest line at the top; a plain dome elsewhere.
- **Base:** size 7. LowDome 6.0 × 2.4.
- **Process:** Delft.
- **Stones:** Round 4.0 at θ 90; Round 2.5 at θ 72 and θ 108.
- **Build, step by step:** three `SeatPadLayer`s on `ctx.crest_v_mm`, each `style: GypsyMound, crown: 1.0, solid: SolidKind::Flush, through: true`, then `fit_stone`:
  - Centre: `height_mm` 0.55, `blend_mm` 0.6.
  - Sides: `height_mm` 0.4, `blend_mm` 0.5.
  - Add `mark_mm` 0.8 on the centre and 0.6 on the sides.
  - This follows Palisade's column (`core/examples/atelier_masterworks.rs:167-171`).
- **What it shows off:** flush settings, and stock cast in sand with the bur cut at the bench.
- **Traps and how they are avoided:**
  - A GypsyMound carries 1.8 mm of stock, so the 4.0 centre's mound is 5.8 mm across on a 6.0 mm band ("a stone is sized to its face, not to its band"). Check the edge clearance in `stones::report`. If it is under `MIN_EDGE_MM`, drop the centre to 3.5, or widen to 6.5.
  - Everything sits on the crest line, which is the parting plane.
- **Needs:** nothing new.
- **Template:** code plus graph builder. About 4 KB.
- **Risk:** low.

### 1.3 Starter signets

## Shouldered cushion signet — *ornament off the table*

- **Status:** exists (`core/templates.rs:99`). Re-render; move it to first place in the group.
- **Concept:** a lofted cushion signet with Chevron ornament on the shoulders, windowed off the table.
- **Base:** `signet(SignetOutline::Cushion, 14.5, 2.2)`, lofted (`apply_signet` sets `loft = 1.0`), size 7.
- **Process:** sand (defaults).
- **Traps:** the doctrine records a DFM finding here: Chevron at 0.03 mm gaps on 7.6 × 0.6 mm shoulder cells, at 185°. Re-measure with `dfm::measured_tests::the_templates_measured -- --nocapture`. Either raise the cell height by lowering `repeats_around` from 9 to about 6 and re-check, or name it in the blurb.
- **Template:** bundled (2.8 KB), unchanged. Its thumbnail is the group's row icon.

## The twenty factory stocks — *hard angles, ready for a theme*

- **Status:** not started. C-S1 is part 1's first task after the fixtures.
- **Concept:** every decoded factory signet, bare, as a starting blank. These presets keep the hard angles where wall meets face, which is Logan's standing preference for signets. Each opens ready for one theme face to palm.
- **Theme face to palm:** bare stock. The theme is the user's.
- **Base:** `templates::stock(&PRESETS[i])` (C-S1 above), at the master's calibrated bore and face.
- **Process:** chosen by plan symmetry.
  - **Sand-safe (11):** `sand_master` + envelope + Delft.
  - **Upright (9):** raw stock + lost wax. The sand envelope "fills an off-centre plateau toward z = 0, so the bare stock would change shape" (plan §1).
- **Stones:** none.
- **Names and slugs:**
  - Names like "Octagon signet · 015"; the two cushions are "Cushion signet · 001 (20 mm)" and "Cushion signet · 012 (10 mm)".
  - Slugs like `stock-015-octagon`.
  - Description: `Preset::label()` (for example "Octagon · 16 × 16 mm · 015") + " · factory stock, hard angles where wall meets face; bare, ready for a theme".

| Family | Id | Plan | Face mm (`PRESETS`) | Asymmetry (F2) | Opens as | Badge | Already used by |
|---|---|---|---|---|---|---|---|
| Round and square | 013 | Round | 10 × 10 | ≤ 0.6 | Delft | sand-safe plan | Saurian, Ophidian, Rosa (Tenebrae), Officina Sigil |
| | 012 | Cushion | 10 × 10 | ≤ 0.6 | Delft | sand-safe plan | Prunus (Vepres) |
| | 001 | Cushion | 20 × 20 | ≤ 0.6 | Delft | sand-safe plan | Solstice, Chamaeleo (Cataphracta) |
| | 017 | Tonneau | 16 × 12 | ≤ 0.6 | Delft | sand-safe plan | Zenith, Varanus |
| | 006 | Square | 16 × 21 | ≤ 0.6 | Delft | sand-safe plan | Aurelia, Ilex (Vepres) |
| | 015 | Octagon | 16 × 16 | ≤ 0.6 | Delft | sand-safe plan | Nocturne, Caiman, Lanterna (Tenebrae) |
| Shields | 004 | Shield | 18 × 18 | 7.8 | lost wax | upright · lost wax | none |
| | 014 | Heater | 15 × 18 | 9.0 | lost wax | upright · lost wax | none |
| | 020 | Escutcheon | 18 × 19 | 4.1 | lost wax | upright · lost wax | Basiliscus (Bestiarium, wax) |
| | 011 | Badge | 18 × 20 | 3.7 | lost wax | upright · lost wax | Datura (Vepres) |
| | 019 | Jewel | 19 × 19 | 7.9 | lost wax | upright · lost wax | Vesper |
| Lobed | 003 | Clover | 18 × 18 | ≤ 0.6 | Delft | sand-safe plan | Viscum (Vepres) |
| | 007 | Quatrefoil | 16 × 18.5 | ≤ 0.6 | Delft | sand-safe plan | Chelonia (Cataphracta) |
| | 005 | Rosette | 18 × 21 | ≤ 0.6 | Delft | sand-safe plan | Sigillum (Tenebrae) |
| | 016 | Star | 19 × 20 | ≤ 0.6 | Delft | sand-safe plan | Phrynosoma (Cataphracta) |
| | 018 | Butterfly | 20 × 17 | 1.7 | lost wax | upright · lost wax | none |
| | 008 | Heart | 19 × 19 | 6.3 | lost wax | upright · lost wax | none |
| Pointed | 002 | Kite | 14 × 25 | ≤ 0.6 | Delft | sand-safe plan | Draco (Bestiarium) |
| | 009 | Drop | 17 × 22.3 | 2.5 | lost wax | upright · lost wax | Porta (Tenebrae) |
| | 010 | Trillion | 18 × 20 | 7.7 | lost wax | upright · lost wax | Fenrir (Bestiarium) |

- **Build, step by step:**
  1. `templates::stock(preset)`.
  2. Menu row, with `badge` and `family` set.
  3. Thumbnail from template_shots' `PRESETS` loop: `render::finished(&d, &lib, BuildParams { theta_steps: 512, profile_steps: 192, .. })` → `write_png_parts(dir/stock-<id>-<name>.png, &f.parts(render::GOLD), 0.55, 1.12, 700)`, then `tools/template_thumbnails.py` (add 20 rows to `wb/../assets/templates/sources.json`).
- **What it shows off:** imported factory stock; the sand master and envelope; the process chosen by the plan's own geometry.
- **Traps and how they are avoided:**
  - **First-build cost:** a sand-safe stock's first build runs `sand_master` (refined to 0.55 mm edges, up to 400k faces) and then `pull::build`. Measure both with `graph/examples/template_open_probe.rs` before promising "instant".
  - **The phone:** its "Imported signet base..." entry (`phone/app/files.rs:123-137`) opens only `PRESETS[0]`. Remove it: the shared menu (`files.rs:139`) now carries all twenty.
- **Needs:** C-S1; P7 for graph form.
- **Template:** code starters now (`Source::Stock`), with no bundled graph. A lift today carries a 3 MB `/imported_base` patch per stock (Caiman's graph is 12.7 MB; 3.0 MB of that is this patch). After P7, "Convert to graph" lifts each stock to about 10 nodes with `base.preset` and the five stock controls exposed (size, face width, palm thickness, face length, face rise). Only then consider bundling them, at under 20 KB each.
- **Risk:** low. Fallback if a sand master fails to build clean: open that one plan as lost wax too, and record it in plan F2.

### 1.4 Menu, tests and retirements

**Menu (`wb/templates.rs:467` `collections()`).**

- **Order:** Starter bands, Starter signets, Stone settings, Officina, Reptilia collection, Stock masterworks, Atelier designs, Original masterwork signets. Starters go first, because a new user reaches for a blank before a masterwork. New collections (Bestiarium and the rest) go after Officina.
- **Row icons:** the first template of each group, so Court band, Shouldered cushion, Cathedral solitaire and Rivet.
- **Group contents:**
  - Starter bands: `court-band`, `braided-band`, `split-shank`, `split-gallery`.
  - Starter signets: `shouldered-cushion-signet`, then the 20 stocks in the family order above.
  - Stone settings: `cathedral-solitaire`, `bezel-solitaire`, `halo`, `trilogy`, `toi-et-moi`, `split-shank-basket`, `half-eternity`, `gypsy-trio`.
  - Officina: `rivet-officina`, `sigil-officina`, `keystone-officina`, `aile-officina`, `torsade-officina`, `fenestra-officina` (all `Source::Graph`).
- **Removed:** the Workshop collection group (`wb/templates.rs:481-486`); `heart-signet`, `waved-hexagon-signet`, `cathedral-solitaire-stock` and `wishbone-wave`; the old `toi-et-moi`, whose slug is reused by the new entry.
- **Previews:** replace the `preview_bytes` match (`wb/templates.rs:501-536`) with a `thumbnails!{ "slug", … }` macro that emits both `preview_bytes` and a `SLUGS` list.

  ```rust
  macro_rules! thumbnails { ($($slug:literal),* $(,)?) => {
      pub const SLUGS: &[&str] = &[$($slug),*];
      pub fn preview_bytes(slug: &str) -> Option<&'static [u8]> {
          Some(match slug { $($slug => include_bytes!(concat!("../assets/templates/", $slug, ".png")),)* _ => return None })
      }
  }}
  ```

**Counts, and where they are pinned.**

| Where | Today | After part 1 | Note |
|---|---|---|---|
| `wb/templates.rs:589` `assert_eq!(slugs.len(), 31)` | 31 | **57** | 4 bands + 21 signets + 8 settings + 6 Officina + 5 Reptilia + 7 stock masterworks + 4 atelier + 2 originals. 58 when Wishbone returns (C-S6). Each later collection adds its own. |
| Core `TEMPLATES` (`core/templates.rs:66`, `[Template; 9]`) | 9 | **13** | Court, Braided, Split shank, Split gallery, Shouldered cushion, and the 8 settings. The stocks are not in `TEMPLATES`; they come from `PRESETS`. |
| Graph `BUNDLED` (`graph/templates.rs:126-136`, `build()` at `:377`) | 9 | **13** | Pinned equal to the code starters (`graph/templates.rs:846`). Regenerate with `RD_WRITE_TEMPLATE_GRAPHS=1 cargo test -p ringdesign-graph write_template_graphs`. |
| `crates/ringdesign-py/tests/test_smoke.py:25` `len(names) == 9` and "Heart signet" | 9 | **13**, with "Heart signet" replaced by "Cathedral solitaire" | Line 81 uses "Cathedral solitaire stock": switch it to "Cathedral solitaire" and assert `stones()` counts 1. |
| Golden corpus (`crates/ringdesign-core/tests/golden.rs:82`, `tests/golden/corpus.json`) | 9 `template/` rows | 13 `template/`, 6 `fixture/` (keeping `ShankKind::Split` as "Split channels"), 20 `stock/001`…`stock/020` | Rewrite with `RD_WRITE_GOLDEN=1`. |

**Retired-name call sites (C-S4).** Switch each to `templates::fixture(..)`. Lines are current at `30f0506`; the brief's numbers had drifted.

| File:line | Uses |
|---|---|
| `core/castability/judge.rs:674` | Heart signet |
| `core/cad.rs:3612` | Heart signet |
| `core/parts.rs:1117`, `:1562` | Heart signet (the second iterates "Court band", "Heart signet") |
| `core/cad/pattern.rs:970` | Heart signet |
| `core/setting.rs:3013` | Heart signet |
| `core/dfm.rs:399` | Heart signet |
| `core/interaction/pick.rs:982` | Cathedral solitaire stock |
| `gui/pattern_tests.rs:380` | Heart signet |
| `wb/touch/parts.rs:306` | Cathedral solitaire stock |
| `crates/ringdesign-core/examples/bead_probe.rs:241,251`, `pick_probe.rs:57`, `join_probe.rs:55` | retired names |
| `crates/ringdesign-py/tests/test_smoke.py:25,81` | as above |
| `wb/templates.rs:598` (open test) iterates `["aster-atelier", "court-band", "aster-workshop"]` | replace `aster-workshop` with `aster-botanical`, the remaining `Source::Design` |

- `core/profile.rs:1258` (`ShankKind::Split => "Split shank"`) is the shank kind's label, not a template. Leave it.
- `crates/ringdesign-configurator/src/compose.rs:45-55` uses the names as labels only. It is unaffected.

**Tests to add.**

- `every_stock_starter_opens_clean`: all 20 at 192 × 96, under the memory guard. For each:
  - The source name is `preset.stock_name()` (upright) or `"{stock_name} / drafted workshop master"` (sand-safe).
  - `sand_envelope == sand_safe()`.
  - The process is Delft sand or lost wax as the badge says.
  - Watertight, 0 degenerate faces.
  - The verdict is not NotCastable.
- The core starter test (`core/templates.rs:215`) switches to `judged_field_report(.., Some(&built))` and asserts `built.solids.notes` is empty.
- The GUI test (`gui/ui_tests.rs:260`, `file_menu_has_preview_collections_and_opens_the_selected_template`): open "Octagon signet · 015" through the loader and step until it lands; `imported_base` must be `Some`.

**Workshop retirement.**

- Drop the four `DESIGNS` rows at `crates/ringdesign-assets/build.rs:39-42`, their previews and `sources.json` entries.
- Replace `crates/ringdesign-core/examples/workshop_collection.rs` with `examples/officina/`.
- Retire `tools/catalog_workshop_collection.py` and `tools/check_workshop_collection.py`.
- This also ends the three-"Aster" collision.
- Leave the `showcase/workshop-collection/` folder in place until Logan says otherwise.

---

## 2. Officina

**Subtitle:** "Six lessons in the CAD workspace · three pour in sand, three in lost wax".

**Primary side of the app:** the CAD workspace. It covers the feature timeline and its rollback marker (`Document::through`, `core/cad.rs:1297`; `wb/timeline.rs:174`, `Action::RollTo` at `:212`), sketches and regions, builders, and `cad.feature` graph chains. The graph and the timeline chips list the same features in the same order. The README tells the reader to drag the rollback marker from feature 1 to the end.

**Format:** each ring is a short feature history, one idea per ring. Depth belongs to Tenebrae.

**Bore:** 18.2 mm, keeping `workshop_collection.rs`'s `setup()` recipe, flask and channels (`:15-57`). Set the process with `apply` (section 0), not with that file's `base()`.

| # | Ring | Epithet | Base | Process | Stones | Features | Status |
|---|---|---|---|---|---|---|---|
| 1 | Rivet | a riveted strap | LowDome 6.0 × 2.1 | Delft | — | 5 | not started; buildable now |
| 2 | Sigil | a quartered seal on the round | stock 013, native size | Delft (sand master) | — | 7 | not started; buildable now after a spike; light template needs P7 |
| 3 | Keystone | a drafted cartouche | Flat 6.0 × 2.0, `flatten_sides` | Delft | — | 8 | not started; buildable now |
| 4 | Aile | wings clasping a bezel | DShape 2.4 × 1.8, ReverseTaper 0.4 | lost wax | Oval 7 × 5 | 12 | not started; buildable now |
| 5 | Torsade | a rope-edged collet | Flat 4.4 × 1.8, `flatten_sides` | lost wax | Round cabochon 7.0 | 9 | not started; buildable now |
| 6 | Fenestra | windows along the pull | Flat 3.2 × 2.4, `flatten_sides`, Cathedral 0.8 | lost wax | Oval 7 × 5 | 8 | not started; buildable now |

### Build order and why

This is batch 17 (the plan's batch 2), run in parallel with the Bestiarium, after P7 and P8 merge.

1. **Rivet.** It needs nothing new, so it proves the `examples/officina/` scaffold, the P8 packaging and the template lift end to end.
2. **Keystone.** It carries the native-fillet risk, and its fallback (Chamfer) is known.
3. **Torsade.** Twist plus csg join time along a 58 mm rope, which has never been measured.
4. **Sigil.** It needs the CAD-on-stock spike first, and its light template needs P7 `base.preset`.
5. **Fenestra.** The builder chain, plus side-face piercing.
6. **Aile.** Last: Bezier regions and csg union are the highest risk, and it is the most taste-sensitive (a Hypnos homage; a lesson-sized winged piece beside the Bestiarium's winged beasts).

### Enablers

Officina has no C-* enabler of its own. It depends on:

- P2 (merging), for CAD stones in reports and renders.
- P7, for `base.preset` (Sigil's template under 1 MB) and `cad.op.*`/`placement` pins (exposed lesson controls).
- P8, for `render_collection.py`, `catalog_collection.py` and `collection_templates.rs -- officina`.

**Optional, not scheduled in the plan:** `Feature::note: String` (serde default), shown in the timeline chip's hover, for per-step lesson text. Until then, the lesson text lives in the README.

**Author example:** `crates/ringdesign-core/examples/officina/main.rs`, with one module per ring. It has a CLI with `--draft`, `--verify` and `SLUG`, and a `write_ring` like the Reptilia author. It saves `showcase/officina/<slug>/design.ring.json`. It also adds the six to `cad::examples::NAMES` (`core/cad/examples.rs:3`), keeping the existing six, which `wb/timeline.rs:827,875` iterate. Check that those tests still pass with twelve.

## Rivet — *a riveted strap*

- **Status:** not started. Buildable now.
- **Concept:** a strap band with sixteen large rivet heads on the crest line and a small one between each pair, as if the band were riveted round the finger.
- **Theme face to palm:** alternating large and small rivets on the crest the whole way round, 32 in all. The side faces and the bore are plain.
- **Base:** LowDome 6.0 × 2.1, bore 18.2 mm.
- **Process:** Delft.
- **Stones:** none.
- **Build, step by step:**
  1. `Band`.
  2. `Sphere { radius_mm: 0.9 }`, `Placement::ring(90.0, -0.35)`, `Attach::Join`, `blend_mm` 0.15. Its centre sits 0.35 mm inside the crest, so it stands 0.55 mm proud.
  3. `Pattern { sources: [2], kind: PatternKind::Ring { count: 16, span_deg: 360.0 } }`, Join, blend 0.15.
  4. `Sphere { radius_mm: 0.5 }`, `Placement::ring(101.25, -0.2)` (half a pitch on, 0.3 mm proud), Join, blend 0.12.
  5. `Pattern Ring { count: 16 }` of 4.
- **What it shows off:**
  - Placement and stand-off.
  - Join against Separate.
  - The seam-bead radius.
  - Why integer counts close seamlessly: a full turn steps span/count (`core/cad/pattern.rs:133`).
- **Traps and how they are avoided:**
  - The beads sit on the crest line, where they split cleanly between cope and drag. Every point of a sphere centred on z = 0 faces its own mould half. The same row 1.9 mm off the crest locks at −37° (doctrine: showcase). So `across_mm` stays 0.
  - The gap between large and small beads is 11.25° × r 11.2 ≈ 2.2 mm pitch, minus visible radii of 0.83 and 0.46, which leaves about 0.9 mm. That is over the 0.30 floor.
- **Needs:** nothing new.
- **Template:** lift into a `cad.feature` chain. About 10 KB. Expose the two sphere radii once P7 adds numeric pins.
- **Risk:** low.

## Sigil — *a quartered seal on the round*

- **Status:** not started. The design is buildable now after one spike. The light template waits on P7.
- **Concept:** a signet seal cut at the bench into factory stock 013: a quartered field with two quarters sunk, inside a sunk bordure. It is the lesson-sized seal; Sigillum (Tenebrae, on 005) is the full one.
- **Theme face to palm:**
  - Table: the seal.
  - Shoulders, walls and palm: bare stock with its hard angles.
  - Bore: plain.
- **Base:** stock 013 Round (face 10 × 10), at its calibrated bore. Use `templates::stock(&PRESETS[013])`, which gives the sand master with the envelope on. It is "native", meaning not resized; Rosa (Tenebrae) uses 013 at 16 mm.
- **Process:** Delft. 013 is sand-safe. Every cut is `Stage::Bench`, so the pattern is the bare stock.
- **Stones:** none.
- **Build, step by step:**
  1. `Band` (the stock is the band; `Band` is the anchor).
  2. `Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: 0.0 }`. The plane is dropped onto the built mesh, which is the stock (`core/cad/pattern.rs:646-656`).
  3. `Sketch` "Field" on 2 (anchored as in Split gallery):
     - Four `Arc`s of r 4.0 about (0, 0), each 90°, with endpoints at N, E, S and W.
     - Two `Line`s, N–S and E–W, whose ends are those arc endpoints.
     - Every junction is an endpoint, so the four quadrants are four regions (`core/sketch/region.rs:57`, `RegionRef { entity, at }`).
  4. `Extrude { sketch: Profile::Region { feature: 3, region: RegionRef { entity: <NE arc>, at: [1.5, 1.5] } }, height_mm: -0.30, draft_deg: 0.0 }`, `Attach::Cut`, `Stage::Bench`.
  5. The same for the SW quadrant (`at: [-1.5, -1.5]`).
  6. `Sketch` "Bordure" on 2: two `Circle`s about (0, 0), r 4.15 and r 4.6, making one region with a hole. That leaves a 0.15 mm land between field and bordure, so the two bench cuts never share a face.
  7. `Extrude { sketch: Profile::Feature { feature: 6 }, height_mm: -0.20 }`, Cut, Bench.
- **What it shows off:**
  - A work plane on factory stock.
  - T-junction regions and picking one by `RegionRef`.
  - A region with a hole.
  - A negative extrude as a cut.
  - The two-stage model: the seal is in the render, not the pattern, and the verdict skips it ("The verdict judges the pour, not the bench").
- **Traps and how they are avoided:**
  - **Spike first:** assert that feature 2's origin sits within 0.05 mm of the stock's table at θ 90 and that its normal is radial within 1°. `surface_hit` reads the built mesh, so this should hold, but it has not been tried on imported stock.
  - Whether two concentric `Circle`s form one region with a hole is **(unverified)**. The fallback is two loops of four arcs each.
  - Bench cuts on a sand-master stock: `pull::build` envelopes only the cast stack, and CAD cuts resolve after it, so the envelope cannot fill them. Check the render.
- **Needs:** the spike; P7 `base.preset`.
- **Template:** lift into a `cad.feature` chain. Today that carries a 3 MB `/imported_base` patch; after P7 it is `base.preset {id: "013", sand: true}` + 7 `cad.feature`s, under 1 MB (the plan §5 stock budget). Do not bundle before P7 unless Logan accepts the size.
- **Risk:** medium. Fallback: if the tangent plane will not seat on stock, a `Plane { Section { theta_deg: 90 } }` rotated by hand is **not** a fallback, because it stands on the wrong axis. Instead, ship Sigil on a procedural lofted round signet (`signet(SignetOutline::Oval, 10.0, 1.8)` at aspect 1) and note that.

## Keystone — *a drafted cartouche*

- **Status:** not started. Buildable now.
- **Concept:** a raised cartouche on a flat band, pressed taller, its rim filleted by edge signature, with a bright-cut facet in its top.
- **Theme face to palm:** the cartouche at the top; a plain squared band elsewhere.
- **Base:** Flat 6.0 × 2.0, `flatten_sides`, bore 18.2 mm (crest r 11.1).
- **Process:** Delft.
- **Stones:** none.
- **Build, step by step:**
  1. `Band`.
  2. `Plane { base: Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: -0.5 }`. Sinking the plane keeps the boss joined where the band curves away: the sag over the cartouche's half-length is 3² / (2 × 11.1) ≈ 0.41 mm.
  3. `Sketch` "Cartouche" on 2: a rounded rectangle 6.0 round the ring × 4.8 across, drawn with 4 `Line`s and 4 corner `Arc`s of r 0.8.
  4. `Extrude { Profile::Feature { feature: 3 }, height_mm: 1.4, draft_deg: 7.0 }`, `Join`, `blend_mm` 0.25.
  5. `PressPull { source: 4, face: FaceRef::signed(body, top, &frame), distance_mm: 0.4 }`. Author the `FaceRef` after one evaluation, reading the body's planar top face (`core/cad.rs:1054`).
  6. `Fillet { source: 5, edges: <the 8 top-rim edges as EdgeRef::signed(..)>, radius_mm: 0.35 }` (`core/cad.rs:1020`).
  7. `Sketch` "Facet" on feature 6's top face: `Workplane { on_face: Some(FaceAnchor { feature: 6, face: <top> }) }`. A lozenge 4.0 × 2.8 plus one diagonal, making 2 regions.
  8. `Extrude { Profile::Region { feature: 7, region: <one triangle> }, height_mm: -0.35 }`, Cut, Bench. This is the bright-cut facet.
- **What it shows off:** draft, press-pull, a sketch on a face, and edges named by signature rather than position. Changing step 5's distance keeps the fillet attached.
- **Traps and how they are avoided:**
  - **The verdict is the lesson.** The walls facing round the ring and the flat top are zero-draft to a Z pull, the same case as a signet's table (doctrine: "A flat table is a zero-draft plane…"). The fillet adds draft at the rim. Expect "castable with care", and state it in the README. This is the one sand entry exempt from the "Castable" gate. Record whatever is measured.
  - Native `fillet_edges` on a drafted prism is a risk (**(unverified)**; the brief cites `core/cad.rs:1941-1953`). Fall back to `Chamfer { source: 5, edges, base_face, distance_mm: 0.3 }`.
- **Needs:** nothing new.
- **Template:** lift into a `cad.feature` chain. About 15 KB.
- **Risk:** medium.

## Aile — *wings clasping a bezel*

- **Status:** not started. Buildable now.
- **Concept:** an oval in a collet, clasped by two three-feather wings that lift off the band toward their tips. It is a lesson-sized homage to Logan's Hypnos. The plan keeps it beside the Bestiarium's winged beasts (plan §2).
- **Theme face to palm:** the stone and wings over the top 70°; a plain tapering shank to the palm.
- **Base:** DShape 2.4 × 1.8, `ShankKind::ReverseTaper` 0.4, bore 18.2 mm.
- **Process:** lost wax. In sand, wings off the crest lean or overhang (doctrine: off-crest rails).
- **Stones:** Oval 7 × 5, north-south: `Gem { l_mm: 7.0, ..Gem::calibrated(GemCut::Oval, 5.0) }`.
- **Build, step by step:**
  1. `Band`.
  2. `stone_feature(2, gem, Placement::ring(90.0, stand_off_mm("head.bezel", gem)))`.
  3. `head.bezel` on 2, Cast.
  4. `seat.bur {through: false}` on 2, Cast.
  5. `Plane { Tangent { theta_deg: 58.0 }, offset_mm: -1.0 }`.
  6. `Sketch` "Wing" on 5 (the plane's x runs round the ring, y along the finger):
     - The outer contour runs from the root at x 2.0 against the collet, out to the tip at about (−6.5, +2.2), and back along the leading edge.
     - Two internal `Bezier`s run from the root to the trailing edge, sharing their endpoints with the contour, which makes 3 feather regions (primaries).
  7. to 9. `Extrude` each region: heights 1.8, 1.6 and 1.4, `draft_deg` 10. At the tangency they stand 0.8, 0.6 and 0.4 mm proud. The band falls away under the plane (sag about 1.9 mm at x −6.5), so the tips lift off the band.
  10. `Boolean { a: 7, b: 8, kind: Boolean::Union }`.
  11. `Boolean { a: 10, b: 9, kind: Union }`, `Join`, `blend_mm` 0.12. Mesh operands go through csg.
  12. `Pattern { sources: [11], kind: Mirror { plane: MirrorPlane::Section { theta_deg: 90.0 } } }`, Join.
- **What it shows off:** building round a stone; regions from T-junctions; one sketch with three heights; union into one part; mirroring round the ring.
- **Traps and how they are avoided:**
  - **Flat tiers read "flat and blocky".** That was Logan's complaint about the Workshop set, and why Harpyia returns with lofted feathers. The brief's extrusions had 0° draft; the 10° draft above is this file's change, to soften the steps. If the round-1 render still reads blocky, the upgrade is Harpyia's lofted-feather construction, once the Bestiarium proves it.
  - Feather tips must be ≥ 0.15 mm detail and 0.5 mm section under investment.
- **Needs:** nothing new.
- **Template:** lift into a `cad.feature` chain. About 25 KB, because the Bezier sketch is inline JSON.
- **Risk:** medium, from Bezier regions and csg union. Fallback: two feathers instead of three.

## Torsade — *a rope-edged collet*

- **Status:** not started. Buildable now.
- **Concept:** a flat band whose two edges carry twisted ropes, which butt into a cabochon's collet at the top.
- **Theme face to palm:** ropes along both arrises from the collet round the palm and back; the collet at the top. Plain side faces and bore.
- **Base:** Flat 4.4 × 1.8, `flatten_sides`, bore 18.2 mm. The side faces are at z = ±2.2 and the crest at r 10.9.
- **Process:** lost wax. "A true helix locks in the sand" (doctrine).
- **Stones:** `Gem::cabochon(GemCut::Round, 7.0)` (`core/gem.rs:250`).
- **Build, step by step:**
  1. `Band`.
  2. `Plane { base: Parting, offset_mm: 2.05 }`.
  3. `Sketch` "Path" on 2: one `Arc` about (0, 0), r 10.75, from θ 112 the long way round to θ 68 (316°, about 59 mm). Whether `Arc` runs counter-clockwise from start to end is **(unverified)**: if it is clockwise, swap `start` and `end`.
  4. `Plane { base: Section { theta_deg: 112.0 } }`.
  5. `Sketch` "Section" on 4: a diamond 1.1 mm across, centred on the path's start (x 10.75, y 2.05), square to the path, as `twist::sweep` requires (`core/cad/twist.rs:360-380`).
  6. `Twist { sketch: Profile::Feature { feature: 5 }, path: <feature 3's Sketch, inline, plane anchored to 2>, degrees: 7200.0, end_scale: 1.0 }`, Join, blend 0.08. `Twist::path` is an inline `Sketch`, not a feature id (`core/cad.rs:81-86`).
  7. `Pattern { sources: [6], kind: Mirror { plane: MirrorPlane::Band } }`, Join. This gives the opposite lay on the −Z arris.
  8. `stone_feature(8, gem, Placement::ring(90.0, stand_off_mm("head.bezel", gem)))`.
  9. `head.bezel` on 8, Cast. Its lip comes from the cabochon's dome ("The bezel stands on its stone"). The rope ends at ±22° butt into the collet wall.
- **What it shows off:** paths against sections, the twist parameters, mirroring across the band's mid-plane, and a cabochon collet.
- **Traps and how they are avoided:**
  - **The brief buried the rope.** At r 10.6 and z 1.55, a 1.0 mm diamond stands only 0.2 mm proud of the 10.9 crest and stops 0.15 mm short of the side face. At r 10.75 and z 2.05, 1.1 mm across, it stands 0.4 mm proud and 0.4 mm past the face, wrapping the arris.
  - Stations: 7200° / `TWIST_STEP_DEG` 3 = 2400, under `MAX_STATIONS` 8192. About 40k triangles, under `MAX_TRIANGLES` 2 M.
  - A twist is a mesh, so fillet and press-pull refuse it by name (doctrine).
- **Needs:** nothing new.
- **Template:** lift. About 15 KB.
- **Risk:** medium. The csg join time along the 59 mm rope has not been measured. Fallback: 3600° with a 1.3 mm section.

## Fenestra — *windows along the pull*

- **Status:** not started. Buildable now.
- **Concept:** a basket solitaire whose swelling shoulders are pierced right through their side faces by drop windows along the finger. The windows are arrayed down each shoulder and mirrored to the other.
- **Theme face to palm:**
  - Head: stone and basket.
  - Shoulders: three drop windows each side, through both side faces.
  - Palm: plain.
- **Base:** Flat 3.2 × 2.4, `flatten_sides`, `ShankKind::Cathedral` 0.8 (the procedural swell, not the `shank.cathedral` builder). That gives 4.6 × 2.9 mm at the top, bore 18.2 mm.
  - The brief's 2.4 mm band relied on the cathedral arches to carry the basket. Without them, the claw feet at about ±2.1–2.4 mm across need a top at least 4.3 mm wide.
- **Process:** lost wax. Switching to sand is itself the lesson:
  - The basket and bur flip to Bench.
  - The side-face windows stay Cast, because `cut_stage` sees a cut within 10° of the pull (`core/cad/builders/cutters.rs:1067`).
- **Stones:** Oval 7 × 5 along the ring (`spin_deg` 90).
- **Build, step by step:**
  1. `Band`.
  2. `stone_feature(2, gem, Placement::Ring { theta_deg: 90.0, height_mm: stand_off_mm("basket", gem), spin_deg: 90.0, .. })`.
  3. `feature_on(3, "Basket", BASKET, 2, json!({"prongs": 4, "rails": 2}))`.
  4. `seat.bur {through: true}`.
  5. `let at = cutters::pierce_at(&d, 48.0, 0.0, Some((p, [0.0, 0.0, 1.0])), Shape::Drop)?`, where `p` is a point mid-face on the +Z side face at θ 48. Then `cutters::pierce_feature(5, Shape::Drop, &at)`. The function is at `core/cad/builders/cutters.rs:1099`, `pierce_feature` at `:1156`. The +Z normal selects side-face mode: `cant −90`, through along the finger.
  6. `Pattern { sources: [5], kind: Ring { count: 3, span_deg: -16.0 } }`, which places windows at 48°, 40° and 32°. A partial span steps span / (count − 1) (`core/cad/pattern.rs:133`).
  7. `Pattern { sources: [5], kind: Mirror { plane: Section { theta_deg: 90.0 } } }`.
  8. `Pattern { sources: [6], kind: Mirror { plane: Section { theta_deg: 90.0 } } }`.
- **What it shows off:** the basket builder; side-face piercing along the pull; per-part stages; arrays and mirrors of cuts.
- **Traps and how they are avoided:**
  - `pierce_at` sizes the window from the face at θ 48 (0.45 of the face, capped at 3 mm). The copies keep that size, so check that the face at 32° still clears `MIN_EDGE + 0.2` either side.
  - Once the first render shows the gap to the basket, the array can start nearer the head than the brief's 48°/32°/16°, which was set for arches that are gone.
  - Steps 7 and 8 stay two single-source mirrors on purpose. One mirror of `[5, 6]` would be a pattern of several parts, which `library::format_version_for` writes at format 6 (`core/library.rs:56-66`), so older builds would refuse the file.
- **Needs:** nothing new.
- **Template:** lift. About 12 KB. Ship with `Document::through = Some(3)`, so the timeline opens mid-lesson.
- **Risk:** medium. Fallback: if a basket foot lands off the band, widen to 3.6 mm before shrinking the stone.

---

## 3. Platform enablers P5–P8 (batch 16)

These are built in the same batch as the starters: lanes 1A–1D, run at most two at a time, in the pairing order 1A P6 → 1C P7 → 1B P5 → 1D P8, with 1E starters alongside.

- **Branches:** one GitHub issue, branch and PR per lane.
- **Merge rule:** a lane merges when `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --workspace --offline` and `cargo check --no-default-features --target wasm32-unknown-unknown` (for core and the configurator) both pass.
- **File ownership:** each lane owns its files exclusively. `lib.rs` `mod` lines are append-only one-liners, and the second merger resolves them.

### P5: station-aware side-face gates (lane 1B)

- **Problem (plan F4):** `VGate::mask(&self, v, ctx)` (`core/field.rs:443`) has no θ. It resolves side faces on the reference section through `ctx.side_faces_std()` (`:253`). On a keyframed or bypass body whose station is wider or thinner than the reference, side-face relief spills onto the crown fillet. `crates/ringdesign-core/examples/side_gate_probe.rs` (landed in 0C) measures the spill.
- **Needed by:** Phoenix, Rubus, Heloderma, Moloch, Sphenodon, Gekko, and above all Ouroborus. Kraken only cosmetically. No entry in this file needs it.
- **API sketch:**

  ```rust
  // core/field.rs
  impl FieldContext {
      /// The side-face runs of the section at `theta_deg`, as shares of the reference `v` chart;
      /// exactly `side_faces_std()` on an unmodulated band.
      pub fn side_faces_at(&self, theta_deg: f64) -> Option<SideFaces>;
  }
  pub enum VGate {
      Off,
      Band { center_mm: f64, span_mm: f64, fade_mm: f64 },
      SideFaces(SideFacePick),
      /// Where the station's own base draft clears `min_deg`, faded over `fade_deg`.
      Draft { min_deg: f64, fade_deg: f64 },
  }
  impl VGate { pub fn mask(&self, theta_deg: f64, v: f64, ctx: &FieldContext) -> f64; }
  // field.rs:534: the only caller, Window::mask, already holds uv: pass ctx.theta_of_u(uv.u).
  pub side_faces: Option<Arc<Vec<SideFaces>>>, // new FieldContext field: one per degree, None when unmodulated
  ```

- **Table construction:** build the per-station table in `RingDesign::station_tables` (`core/lib.rs:527`), beside `stretch` and `crest_scale`. It uses the same 360 stations, the same `modulation_at` sections (`core/lib.rs:452`) and the same cache key. For each station, walk the modulated section's draft inward from each bore edge (the `side_faces` walk at `core/field.rs:264`), then express the run in reference-`v` shares: the chart is the section's arc normalized.
- **Boundary:** P5 changes only the gate. The other readers of `side_faces_std` stay reference-based, because they place things in the chart: `TilingLayer::fit_to_side_faces`, `pave.rs`, `curve.rs`, `setting.rs`, `stones.rs`, `dfm.rs`, the GUI and phone layer panels, and MCP. A follow-up may move the DFM worst-station read onto it.
- **Owned files:** `core/field.rs` (VGate and FieldContext) and `core/lib.rs` (`station_tables` only).
- **Done when:**
  - The golden corpus is bit-identical, since every unmodulated band takes the old path.
  - A keyframed spill test fails without P5 and passes with it: the side_gate_probe's worst case, relief on the crown fillet at a station wider than the reference.
  - A per-sample performance check: the Court band's `analyze_field` at 192 × 128 stays about 5 ms, and a keyframed band pays one table lookup per sample.
- **Size:** S–M. **Risk:** low to medium (the per-sample cost).

### P6: claw styles (lane 1A)

- **Needed by:** Fenrir, Kraken, Rosa mortua, Sentis; Arcus optionally. This file needs it only indirectly: part 2's cutters wait for this lane, because both touch `core/cad/builders.rs`.
- **API sketch:**

  ```rust
  // core/cad/builders.rs schema (:147), CLAW and BASKET gain three choices; the stone's own `form` (:155) is untouched (plan F12):
  Param "style":    Wire | Talon | Fang | Tentacle | Thorn | Sepal   (default Wire)
  Param "grouping": Even | Feet | Jaws                               (default Even)
  Param "tip":      Dome | Point                                     (default Dome)

  // core/setting.rs
  pub enum ClawStyle { Wire, Talon, Fang, Tentacle, Thorn, Sepal }
  pub enum ClawGrouping { Even, Feet, Jaws }
  pub fn claw_head_within(gem, prongs, wire_mm, rails, floor, wall, style: ClawStyle, grouping: ClawGrouping, tip: ClawTip) -> Result<Named, HeadSnag>; // (:697)
  pub fn tube_oval(path: &[P3], radii: &[(f64, f64)], around: usize, dome: (bool, bool)) -> Solid; // beside tube (:359), for Sepal
  ```

- **What changes:** only the 2-D path and radii each claw feeds to `tube()`: the `line` and `rs` vectors at `core/setting.rs:813-827`. The foot search above them, the rails and the stone notch stay as they are.
- **Styles:**
  - Wire: today's path, bit-identical.
  - Talon: one tighter hooked arc tapering to a point.
  - Fang: a straight cone leaning inward.
  - Tentacle: an S path with falling radii.
  - Thorn: a straight cone with a hooked tip.
  - Sepal: an oval-section tube, flattened, curling over the crown.
- **Groupings:**
  - Even: equal spacing.
  - Feet: claws in pairs or triples like a bird's foot.
  - Jaws: two opposed sets along the ring.
- **Rules, both of which cost a failure each (doctrine):** every bend radius ≥ the local tube radius, and straight–arc–straight paths with no spline kinks.
- **Owned files:** `core/setting.rs` (claw section) and `core/cad/builders.rs` (schema).
- **Done when:**
  - `csg::self_crossings == 0` for every style at 3–8 prongs, on a round and an oval.
  - Wire is bit-identical to today (claw-solitaire volume pinned).
  - The notch still cuts the girdle bite.
- **Size:** M. **Risk:** medium.

### P7: template-weight nodes and lift (lane 1C)

- **Problem:** the lift carries whatever nodes cannot express as `design.set` patches (`graph/lift.rs:427-452`). Stock rings carry a 3 MB `/imported_base` patch. Stamped rings carry `/stamps`. Keyframed bodies carry `/shank/keys`, because `shank` hides `keys` (`graph/nodes/shank.rs:52`). CAD builders are opaque operation JSON on `cad.feature` (`graph/nodes/cad.rs:63`).
- **Needed by:** every template. Here: the split starters (`shank.key`), the stock starters and Sigil (`base.preset`), and exposed lesson controls (`cad.op.*`).
- **API sketch:**

  ```rust
  // graph/nodes/base.rs (new)
  "base.preset": in design; id: select "001".."020"; sand: bool (default Preset::sand_safe());
                 face_length_mm, face_width_mm, bore_mm (optional overrides)
                 eval: PRESETS load → sand_master if sand → ImportedBase::attach → sand_envelope = sand
                       → chart as core::templates::stock does; out design

  // core/imported_base.rs + core/library.rs (format ladder)
  // ImportedBase serializes a bundled source by reference: {"preset": "015", "sand_master": true}
  // in place of the whole Source; load resolves it through PRESETS (+ sand_master).
  // format_version_for (core/library.rs:56) returns 7 for a design carrying a preset reference;
  // older builds refuse it by name. A design with an embedded source is written exactly as today.

  // graph/nodes/stamp.rs (new)
  "stamp":            name, theta_deg, v_mm, rot_deg, outline (List<Point>), height_mm, sink_mm, draft_deg,
                      cut, bench, along_pull, tier, top (select + params) → Stamp   (core/setting.rs:898-930)
  "stamp.outline.*":  one node per core/outline.rs family (keel, lanceolate, leaf, blossom, fork, spiral,
                      rounded triangle, comb lobe, quill, moon) → List<Point>
  "stamp.row":        StampRow (RowPath::{PartingLine, ChartV, SideFace}, taper, fold_clear, mirror) → List<Stamp> via stamp_row
  "design.stamps":    design + List<Stamp> → design

  // graph/nodes/shank.rs
  "shank.key": theta_deg, width_scale, thickness_scale, crown_scale (+ slide after C-S6) → ShankKey
  // un-hide "keys" in shank's .hidden(..) and add a `keys` list pin

  // graph/nodes/cad.rs
  "cad.op.<key>": generated per builder from builders::schema (core/cad/builders.rs:147), typed pins, → operation JSON
  "cad.feature":  gains `placement` (Json) and numeric pins; the existing `operation` pin (:63) takes cad.op output

  // graph/lift.rs
  // emits base.preset when source.name matches preset.stock_name() or "{stock_name} / drafted workshop master"
  // and the source fingerprint matches; shank.key per station; stamp/stamp.row/design.stamps for /stamps;
  // cad.op.* for Builder features — each in place of its design.set patch.
  ```

- **Owned files:** new `graph/nodes/base.rs` and `graph/nodes/stamp.rs`; `graph/nodes/shank.rs`, `graph/nodes/cad.rs`, `graph/lift.rs`, `graph/file.rs`, `core/imported_base.rs` and `core/library.rs`.
- **Done when:**
  - Caiman re-lifts at under 3 MB with no `/imported_base` or `/stamps` patch.
  - Every bundled template still round-trips byte for byte, with at most 4 `design.set` patches (the existing `lift.rs:491` assertion).
  - A format-6 design still opens in a format-6 build.
- **Size:** L. **Risk:** medium (the byte-for-byte lift and the migration).

### P8: collection tooling (lane 1D)

- **Needed by:** every collection's packaging step, including Officina.
- **API sketch:**

  ```text
  tools/render_collection.py      generalised from tools/render_reptilia.py
      blender -b --factory-startup -P tools/render_collection.py -- DIR --collection NAME [--draft] [--resume]
      reads DIR/collection.json (slugs, titles, views) and a per-ring stones.json
      (stone mesh → material: tint from Gem::preview_tint, IOR, dispersion) — the stone material manifest
  tools/catalog_collection.py     generalised from tools/catalog_reptilia.py
      python3 tools/catalog_collection.py DIR --title "Officina"  → <Title>-collection.png, index.html, renders zip
  crates/ringdesign-graph/examples/collection_templates.rs   generalised from reptilia_templates.rs
      cargo run -p ringdesign-graph --release --example collection_templates -- <collection> [SOURCE_DIR]
      per slug: load design → refine_sources → lift → reload → evaluate cold (empty library) → assert source and
      vertices/faces/normals identical → write graphs/templates/<slug>-<collection>.graph.json,
      editable-graph.ring.json, verification.json (+ template bytes, open ms per phase from the open probe)
  ```

- **Macro:** the `thumbnails!` macro belongs to lane 1E, which owns `wb/templates.rs` (section 1.4). P8 uses it.
- **Owned files:** `tools/render_collection.py`, `tools/catalog_collection.py` and `crates/ringdesign-graph/examples/collection_templates.rs`.
- **Done when:** it regenerates the Reptilia sheet identically: `collection_templates -- reptilia` reproduces the five bundled Reptilia graphs byte for byte, and `catalog_collection.py` reproduces `Reptilia-collection.png`.
- **Size:** S. **Risk:** low.

---

## 4. Packaging

### Starters

The starters have no showcase folder. Their deliverables are:

- **Thumbnails:**
  - Starters and settings: `cargo run -p ringdesign-core --release --example template_shots -- target/template-library-review/starter-renders`. It renders `render::finished` in studio gold, stones set, at each template's C-S5 view, plus the 20 stocks.
  - Pad to 160 px with `python3 tools/template_thumbnails.py` after adding every new slug to `crates/ringdesign-workbench/assets/templates/sources.json`.
- **Review sheet for Logan:** `python3 tools/catalog_collection.py target/template-library-review/starter-renders --title "Starters"` (P8).
- **Graphs:** regenerate the 13 bundled starter graphs with `RD_WRITE_TEMPLATE_GRAPHS=1`. Delete `heart-signet`, `waved-hexagon-signet`, `cathedral-solitaire-stock` and `wishbone-wave` from `graphs/templates/`: the menu test fails on any authored graph that is not in the menu.
- **Golden corpus:** regenerate with `RD_WRITE_GOLDEN=1`.
- **Phone:**
  - Bump `version` in `crates/ringdesigner-android/Cargo.toml` and add the `CHANGELOG.md` entry.
  - Run `cargo test -p ringdesigner_android`, and `cargo ndk -t arm64-v8a check -p ringdesigner_android` from the crate dir.
  - Smoke-test on the rdsmoke AVD: open one sand stock and one upright stock from the menu.

### Officina (Reptilia format)

**Per-ring folder `showcase/officina/<slug>/`:**

- `design.ring.json` and `editable-graph.ring.json`.
- `finished-metal.stl`.
- The pattern:
  - Sand (Rivet, Sigil, Keystone): `casting-pattern.stl`, shrink-compensated, bench cuts out.
  - Wax (Aile, Torsade, Fenestra): the investment pattern.
- `reference-<stone>.stl` for Aile, Torsade and Fenestra.
- `report.json`, `mesh.json`, `release-fine.json` (sand only) and `verification.json` (template bytes, open ms).
- `hero.png`, `face.png`, `palm.png`, `top.png` and `side.png`.
- Blender `studio*.png` in studio gold.
- The reel. Record the build reel (Tools > Play build reel) with feature names as captions, from the rdsmoke AVD (`adb shell screenrecord`).

**Collection level:**

- `showcase/officina/README.md`: the lesson text per ring; Keystone's expected "castable with care"; Sigil's bench-only seal; the timeline instruction; the reproduce commands below.
- `showcase/officina/collection.json`: slugs, titles, views.
- `index.html`, `Officina-collection.png` and `Officina-final-renders.zip`.

**Reproduce, in a fresh directory from the repo root:**

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example officina -- NEW_DIRECTORY
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example officina -- NEW_DIRECTORY --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example collection_templates -- officina NEW_DIRECTORY
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- NEW_DIRECTORY --collection officina
python3 tools/catalog_collection.py NEW_DIRECTORY --title "Officina"
```

**Templates:**

- `graphs/templates/<slug>-officina.graph.json` is bundled automatically, because the directory is the family (`crates/ringdesign-assets/build.rs:29`).
- Add `pub static OFFICINA: &[TemplateGraph]` to `graph/templates.rs` and chain it into `catalog()` (`:170`).
- Add the "Officina" group in `wb/templates.rs` directly after the starters.
- 160 px thumbnails from `render::finished` through `tools/template_thumbnails.py`.
- The preview test count, which is already included in the 57.
- **Size budgets (plan §5):** procedural lessons ≤ 300 KB (expected 10–25 KB); Sigil ≤ 1 MB after P7.

**Menu group:** "Officina", with the description "Six lessons in the CAD workspace · three pour in sand, three in lost wax". Its row icon is Rivet's thumbnail.
