# Cataphracta — *Reptilia II*

> **Logan's decisions after this file was written (2026-09-24) — these override the text below:**
> - **Faces are allowed.** He only ruled them out because earlier attempts were poor: "feel free to impress me with a face." Skulls, creature heads and gargoyle faces are welcome when they read well; hold them to the render review's bar and cut any that do not.
> - **Viscum goes lost wax on the native factory 003 Clover** (no sand envelope, the factory lobes kept).
> - **Chelonia (007 Quatrefoil) and Phrynosoma (016 Star) go lost wax on the real factory stock**, not procedural heads: he prefers factory stock for its hard wall-to-face angles, and lost wax frees the carapace tiers and horns from the sand draft clamp. Cataphracta pours six rings in sand and two in wax.


**Sheet subtitle:** EIGHT HIDES · FIVE BANDS · THREE SIGNETS · THREE STONES · ALL POURED IN SAND

**Primary side of the app:** the height field used as a sculptor's layer stack under the sand's rules. Every hide is built from live, editable layers: masks, tie-exact `SmoothMax`, gradient-SVG alphas, warp, helix shear, graded tilings, keyframed bodies and terrace remaps. The draft rule is applied as a live clamp, and every figurative part is a struck stamp with a true outline. Every ring pours in two-part sand (six in Delft clay, two in Petrobond), carries at most one stone, and must come out **Castable** (not "Castable with care") with **zero DFM findings**.

*Cataphracta* means "the mail-clad", after *Ouroborus cataphractus*, the lizard that bites its own tail to become a ring.

This is collection 2 of 4 (Bestiarium, Cataphracta, Tenebrae, Vepres). In the director's plan it is **batch 4**. It starts after the Bestiarium has been packaged and reviewed by Logan (batch 3), and after this collection's own enablers C-R1..C-R8 have landed (batch 3C).

**Sources and precedence.** The source documents are in `docs/collections/source/` on branch `collections-handoff`:
- `plan.md`, the director's plan. Its section 1 verdicts, its base-allocation table and its section 7 ("Logan's answers") override the brief.
- `brief-reptilia2.md`, the original brief. It carries more prose per ring.

This file is written to stand alone. Where the brief and this file disagree, this file wins. The rings are Draco (replaced by Gekko), the Sphenodon epithet, the stone count, the base sizes and the enabler names.

The Bestiarium's own Draco (branch `bestiarium-draco`) and Arachne (branch `bestiarium-arachne`) are being authored now, and they are not part of this collection. Draco's author file is the best working template for a sand signet ring: `crates/ringdesign-core/examples/bestiarium_draco.rs` on that branch.

Snapshot: 2026-09-24, master at `30f0506`.

---

## The eight at a glance

Listed in build order.

| # | Ring | Epithet | Base | Process | Stone | Status |
|---|---|---|---|---|---|---|
| 1 | **Sphenodon** | *the parietal* | Procedural Flat 7.5 × 3.4, thickness-only keys | Delft | Peridot 3.0 round, flush on a gypsy mound | Not started. An ungraded draft can be built now; the final form needs C-R2 and C-R7 |
| 2 | **Heloderma** | *the beaded one* | Procedural HalfRound 8.0 × 3.2, keyframed fat-tail swell | Delft | Spessartite 3.0 round, flush on a gypsy mound | Not started. Needs C-R1, C-R2, C-R3, P5 and C-R7. A painted fallback exists |
| 3 | **Moloch** | *the thorn idol* | Procedural Flat 7.0 × 3.6, thickness-only hump | Petrobond | — | Not started. Can be built now ungraded (P4 has landed); grading needs C-R2 |
| 4 | **Gekko** | *the tokay* | Procedural Flat 7.0 × 3.4, thickness-only keys | Delft | — | Not started. Needs C-R1, C-R2 and C-R7. A painted fallback exists |
| 5 | **Ouroborus** | *the girdled wheel* | Procedural Flat 7.0 × 3.2, asymmetric head-to-tail keys | Petrobond | — | Not started. Needs P5 and C-R2's Spiral law; there is no workaround for either |
| 6 | **Chelonia** | *the carapace seal* | Factory 007 Quatrefoil, 16 × 17; **the base itself is in question** | Delft | — | Not started. **Blocked on a base decision** (the bare 007 master fields NotCastable), then on C-R1, C-R4, C-R7 and C-R8 |
| 7 | **Phrynosoma** | *the horned crown* | Factory 016 Star, 17 × 17; **the base itself is in question** | Delft | — | Not started. **Blocked on the bare 016 verdict** (NotCastable), then on C-R1, C-R4 and C-R7 |
| 8 | **Chamaeleo** | *the casque* | Factory 001 Cushion, 17 × 14.5 (the brief's 13 mm width is refused) | Delft | Alexandrite 5 × 4 cushion, flush boss | Not started. Needs C-R1, C-R4 and C-R7 |

Totals:
- 5 procedural bands and 3 factory signets.
- 6 rings in Delft and 2 in Petrobond.
- 3 stones.

Compared with the brief:
- The ruby is gone with the brief's Draco.
- Gekko carries no stone and has no wings.
- Sphenodon's epithet is "the parietal". Logan's no-eyes rule applies, so no copy anywhere may say "third eye".

---

## Platform status (the dependencies)

The IDs come from plan.md sections 3a and 3b. I checked every "landed" claim against git: all five `b15-*` branches are ancestors of master `30f0506`.

| ID | Capability | State | Used here by |
|---|---|---|---|
| P1 | Async template open, with staged progress | **Landed** (batch 14, hardened in batch 15). Templates open with the script engine attached (`Evaluator::with_exprs`, `crates/ringdesign-workbench/src/templates.rs:114`), so expression pins and script nodes work from the menu | every template |
| P2 | CAD stones are stones | **Landed** (batch 15). `render::finished` (`render.rs:97`) draws the metal and the stones for thumbnails | renders and thumbnails of the three stone rings |
| P3 | Skin in core, plus the sand master | **Landed** (batch 15). `core/skin.rs` provides `Atlas::of` (:52), `Hide::of` (:220), `Hide::crest_at` (:296), `Hide::folds` (:311), `Joints::{new, eccentric}` (:353, :370), `draft_clamp` (:408) and `hide_layer` (:444). `imported_base::sand_master` is in `imported_base/sand_master.rs:54` | the painted fallbacks, all three signets, and the engine under C-R1 |
| P4 | Stamps, second version | **Landed** (batch 15). In `core/setting.rs`: `Stamp::tier` and `Stamp::top` (:927, :930), `StampTop::{Flat, Gable, Ridge, Cone, Dome, Taper}` (:939), `RowPath` (:1547), `StampRow` (:1558), `stamp_row` (:1581), `hull_and_bays` (:1490), `Stamp::cast_as_hull` (:2209) and `Stamp::parting_monotone` (:2149). `core/outline.rs` provides `circle`, `keel`, `rounded_triangle`, `spiral`, `leaf`, `jaw`, `check` and others | Moloch, Gekko, Phrynosoma, Chamaeleo |
| P5 | Station-aware side gates, and `VGate::Draft` | Batch 16, **not started** | required by Ouroborus; its Draft gate is used by Heloderma and Sphenodon; optional for Moloch and Gekko |
| P6 | Claw styles | Batch 16, not started | not used |
| P7 | Template nodes and lift (`base.preset`, stamp nodes, `shank.key`) | Batch 16, **not started** | every template: keys, stamps and stock by reference |
| P8 | Collection tooling (render, catalog and template scripts, `thumbnails!`) | Batch 16, **not started** | packaging |
| — | Starter gallery | Batch 16, not started | — |

### Measured facts that bear on this collection

These are recorded in master `CLAUDE.md` under "The skin is core, and so is the sand master".

- **Bare sand masters, from `examples/stock_spike.rs`:**
  - **001 Cushion** fields Marginal, 0.10% at −1.6°.
  - **007 Quatrefoil** fields NotCastable, 14.7% at −64.1°, and "the envelope refus[es] to fill over 4 mm".
  - **016 Star** fields NotCastable, 2.9% at −7.6°. The envelope rewrites it by 0.44 mm.
  - "A sand ring on 003, 005, 007 or 016 answers its head's own lobes before any relief."
  - All of these were measured at the stock's own size.
- **The field verdict does not see the sand envelope.** `sand_envelope` is read only in the mesh build (`imported_base.rs:941`). The envelope can clean the ray pull, but `analyze_field` still judges the master's own surface plus relief. The collection gate is "field Castable", so the envelope cannot rescue 007 or 016.
- **Stock resize guard:** a face must stay within 70–130% of the master's face (`imported_base.rs:532-534`). Master faces: 001 is 20 × 20, 007 is 16.0 × 18.5, and 016 is 19 × 20 (`bases/signets/*.ringbase.json` calibration).
- **The reference-only side gate spills** (`examples/side_gate_probe.rs`):
  - A keyed station at 1.36× width and 1.00× thickness puts 0.51 mm of cells onto the crown fillet and fields NotCastable, 3.43% at −27.4°.
  - "Keys holding thickness at or above width stay clean."
- **Stamps on the parting line** (`examples/parting_stamp_probe.rs`):
  - Spines leaning ≤ 20° pull; past 23.6° they lock.
  - A tip 0.05 mm off the line locks, unless its ends are blunted by 0.09 mm.
  - `parting_monotone` is calibrated on this probe.
- **The atlas sits where the metal is.** It misses a keyframed band by 0.0006 mm and a bypass by 0.049 mm (pinned). The draft clamp is first-order exact. Painting on a procedural body is therefore trustworthy today.

---

## Build order inside the collection, and why

The plan allows at most two lanes in flight, and gives each lane one GitHub issue, one branch and one PR (Logan's workflow). Pairs below are listed in the order to run them.

1. **Sphenodon.** It is the safest ring: no stamps, no clamp, and thickness-only keys that keep the reference side gate on its face. The sail is where C-R2's cosine grade gets calibrated.
2. **Heloderma.** It brings in C-R1 (the live group clamp), C-R3 and P5's Draft gate, on a procedural body where the atlas is exact. The clamp is proven here before any signet depends on it.
3. **Moloch and Gekko** (paired).
   - Moloch exercises P4 at count: about 41 cone stamps, `stamp_row`, along-pull horns. It is also the first Petrobond ring.
   - Gekko reuses C-R1 and C-R2 and Sphenodon's `tubercle_rows`, and adds the helix shear.
4. **Ouroborus.** It waits for the Spiral law, the new maths added to C-R2, and for P5, which it cannot do without.
5. **Chelonia and Phrynosoma** (paired). They need C-R4's hide-space tilings, a proven clamp, and the base decisions below.
6. **Chamaeleo.** It is last because it reuses everything: the clamp, hide space, spiral and cone stamps, the warp, and Caiman's boss seat.

**Do this first, before batch 4 and without any new code:** run the three bare-base studies at the rings' own sizes (step 0 under Chelonia, Phrynosoma and Chamaeleo), and put the numbers in front of Logan. They decide two of the eight bases.

---

## Collection enablers (C-R1..C-R8)

These land in batch 3C, which waits for 3B (C-B2 true stone plans) because both touch `field.rs`. Files: `core/field.rs`, `core/tiling.rs`, `core/lib.rs` (`bake_clamps`), `core/dfm.rs`, `core/reptile.rs`, `core/castability.rs` and `core/imported_base.rs`.

| ID | Capability | Rings | Size | Depends on |
|---|---|---|---|---|
| C-R1 | Live sand clamp on a group | Heloderma, Gekko, Ouroborus, Chelonia, Phrynosoma, Chamaeleo | M | P3 (landed) |
| C-R2 | Graded tilings, laws `Cosine` and `Spiral` | Sphenodon, Heloderma, Moloch, Gekko, Ouroborus, Chelonia, Phrynosoma | M | — |
| C-R3 | Stone-less seat runs | Heloderma | S | — |
| C-R4 | Hide-space tilings and region masks | Chelonia, Phrynosoma, Chamaeleo | M | P3 |
| C-R5 | Drag attribution | all (diagnostic) | S | — |
| C-R6 | DFM reads remaps | Chelonia, Ouroborus (terrace treads) | S | — |
| C-R7 | Reptile SVG generators | all | S per generator | — |
| C-R8 | `plan_mask` from a factory plan | Chelonia | S | — |

### C-R1: live sand clamp on a group

```rust
// core/field.rs, GroupLayer (:566)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub clamp: Option<SandClamp>,

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SandClamp { pub resolution: [u32; 2] /* 2048 x 768 */, pub slack: f64 /* 1.0 */ }

// core/lib.rs: called by the bake pipeline after bake_sdfs (lib.rs:360)
impl RingDesign { pub fn bake_clamps(&self, lib: &mut AlphaLibrary) -> Vec<(String, skin::ClampReport)> }
```

What `bake_clamps` does, for each clamped group:
1. Rasterize the group's composite over `skin::Atlas::of(self, w, h)`.
2. Run `skin::draft_clamp`.
3. Store a derived ceiling `"{group}##clamp"`, beside `SDF_SUFFIX` (`alpha.rs:679`). The ceiling is never saved, and it only holds values where the rule bit, with a sentinel everywhere else.

The group's height becomes `min(composite, ceiling)`. The cache is keyed by the serialized group, profile, shank and base. `ClampReport { texels_cut, worst_mm }` goes to the report panel and the casting sheet.

The clamp cuts back and never fills, which is the opposite of the envelope in `pull.rs`. It is what makes `SmoothMax` and `Add` composites legal under the rule. Caiman's "layers joined by Max keep the guarantee" holds only for `Max`.

Pins:
- A clamped `Max` of two clamped layers is bit-identical to today's painted path.
- Taking the clamp off leaves the design unchanged.

P1's loader shows "clamping" as a bake stage.

### C-R2: graded tilings

```rust
// core/tiling.rs, TilingLayer (:114)
#[serde(default, skip_serializing_if = "Option::is_none")]
pub grade: Option<TileGrade>,
pub struct TileGrade { pub taper: f64, pub theta_deg: f64, pub law: GradeLaw, pub isotropic: bool }
pub enum GradeLaw { Cosine, Spiral { seam_deg: f64 } }
```

- The lattice index runs on φ(u).
- **`Cosine`** is `eccentric_warp` (`field.rs:1267`, today private; make it `pub(crate)`), the same law as `SeatRunLayer::theta_of_station` and `skin::Joints::eccentric`.
- **`Spiral`** is a monotone circle map with φ′ ∝ 1/pitch and one kink at `seam_deg`. The pitch runs monotonically from its largest value just after the seam to its smallest just before it.
- `isotropic` scales `v` about `v_center_mm` by the local pitch ratio, so rows converge the way a tail does.
- DFM: `dfm::tiling_finest_mm_at` (`dfm.rs:665`) × (1 − taper) at the small pole.

Pins:
- At taper 0 the layer is bit-identical.
- The seam step matches the interior step, in the `every_pattern_tiles_seamlessly` style.
- The integer count still closes, because φ maps the circle onto itself.

### C-R3: stone-less seat runs

`SeatRunLayer::bare: bool` (serde default false, `field.rs:1917`):
- Seats keep their authored plan (no `fit_stone`) and are spaced by the seat's own `plan_half_extents_mm`.
- `setstone::set_stones` skips them, and the report says "stock only".
- `gem` stays in the struct and is ignored.

A bare run keeps a fixed `v`, so it cannot follow a factory stock's parting line. On stock, use a domed `stamp_row` instead (Phrynosoma does).

### C-R4: hide-space tilings and region masks

```rust
// core/tiling.rs
#[serde(default)] pub space: ChartSpace,          // Chart (default) | Hide
// core/field.rs
impl FieldContext { pub fn hide(&self) -> Option<&HideChart> }  // cached per design signature, built from skin::Hide
// built-in mask names, resolved like "##sdf" entries:
// "##region:table" | "##region:rim" | "##region:cheek" | "##region:wall" | "##region:shoulder" | "##region:palm"
```

- `ChartSpace::Hide` samples the alpha at (`along_mm`, `across_mm`) from `skin::Hide`.
- The regions come from `Atlas::face`, `Atlas::cheek` and `Atlas::shoulder` (`skin.rs:135-160`).
- On a procedural band, hide space is `(u · crest_scale, (v − crest_v) · station_stretch)` (`field.rs:153`, `:161`).

This was chosen over the Bestiarium's deferred E6 `alpha.hide` painters.

### C-R5..C-R8

- **C-R5, drag attribution:** `castability::attribute_drag(&RingDesign, &AlphaLibrary, &FieldReport) -> Vec<DragShare { layer: String, marginal_mm2: f64, vertical_mm2: f64 }>`. It mutes each layer in turn, like `attribute_undercuts` (`castability.rs:1562`). The verdict turns Marginal when (marginal + vertical) / total exceeds `DRAG_FRACTION` = 0.12 (`castability.rs:31`, `:1249`).
- **C-R6, DFM reads remaps:** measure `remap.apply(shaped)` inside `tiling_finest_mm_at`. Today terrace treads are invisible to DFM.
- **C-R7, reptile SVG generators:** add `reptile::svg::{sail, tubercle_rows, paver, granules, bead_lattice, reticulation, rosette_thorn, lamella, granule_spots, whorl, whorl_spine, plate_voronoi, scute, shingle, plastron, fringe, rosette_tubercle, granule_voronoi, flat_tubercle}` to `core/reptile.rs` (76 lines today).
  - Each one is `fn(&Params) -> String`, with one pattern period per tile. This is the templates lesson: builtin tiles carry several periods and fall under the floor.
  - Each one gets a test asserting that `Alpha::min_feature_px` (`alpha.rs:494`) ink **and gaps** are ≥ 0.40 mm at its ring's tightest station.
  - Each one should also exist as a `script`-node twin, so a template can expose pitch, land and dome.
  - Write each generator in its ring's own module first, and promote it when a second ring uses it.
- **C-R8, `plan_mask`:** `imported_base::plan_mask(id: &str, w: usize, h: usize) -> Alpha`, built from `Preset::plan` (48 polar radii, `imported_base.rs:1069`).

Two items from the brief were dropped:
- E10, `kfold_phase`: Sphenodon's 44 teeth land a cell boundary on 90° without it.
- E16, the stamp footprint mask: it went with the brief's Draco.

---

## Shared construction (applies to every ring)

### The sand grammar, cited by number

| # | Where | What casts |
|---|---|---|
| G1 | Side faces (normal ∥ Z) | Anything at any height: 0.000% up to 1.6 mm |
| G2 | The crest line itself | Relief straddling the parting plane, such as bead rows, gables and fins whose flanks face ±Z |
| G3 | A domed crown off the crest | Only relief rising no faster than the local draft, walking away from the crest. Beads 1.9 mm off-crest lock at −37° over 4.8%; a hammer peen locks at 2.7% / −9° |
| G4 | Any crown or table | Anything that varies only along the ring: joints across the band, loaves, steps, chevrons whose point leads on the parting line |
| G5 | A factory table (zero draft) | Only tiers stepping **down** away from the parting line |
| G6 | Where a section changes **width** | Walls facing round the ring pick up an axial lean (flutes 1–10°, crest beads 45°) |
| G7 | v-gate or mask fades across the band | A fade is a wall facing the crest. Masks that vary only along `u` are free |
| G8 | Warped or sheared tilings | Must be gated to the side faces; ungated they spill (0.89% at −45°) |
| G9 | Stamps | `along_pull` on walls; nothing within 1 mm of a parting-line fold; the surface under a stamp may vary only across the band; strokes ≥ the floor (`dfm::stamp_finest_mm`, `dfm.rs:241`); walls `draft_deg` ≥ 4; a concave edge on a crown faces back (the crescent) |
| G10 | Tilted runs | 45° is clean; 30° leans 5–6° at every seat |
| G11 | Detail floor | `min_feature_px` thresholds the shaped alpha at 0.5 and ignores masks, windows and remaps. Lands between beads are gaps, and gaps are measured: Caiman failed on gaps |
| G12 | Drag | Marginal once marginal plus vertical area exceeds 12%. Steep along-ring walls near the crest lower n_z |

Sand numbers (`castability.rs:180-181`, as draft°, section mm, detail mm):
- Delft: 3.0°, 0.8 mm, 0.30 mm.
- Petrobond: 2.5°, 0.6 mm, 0.40 mm.

Design every feature to ≥ 0.40 mm at its tightest station, whatever the sand.

### Common set-up (verified names)

```rust
use ringdesign_core::{
    AlphaLibrary, BuildParams, ProfileStyle, RingDesign, ShankKind,
    castability::{CastProcess, SandProcess, Verdict},
    field::{Blend, Layer, LayerEntry, Remap, SeatPadLayer, SeatStyle, SideFacePick, VGate, Window, SIDE_FACE_MIN_DRAFT_DEG},
    gem::{Gem, GemCut}, manufacturing as mf, outline, profile::ShankKey, resize,
    setting::{self, RowPath, SolidKind, Stamp, StampRow, StampTop},
    skin::{self, Atlas, Hide}, svg::SvgAlpha, tiling::{TilingLayer, WarpField},
};

// Procedural body: set width and thickness, apply the style (which sets crown and edge from thickness), then override crown, then square the sides.
let mut d = RingDesign::default();
d.profile.width_mm = 7.5; d.profile.thickness_mm = 3.4;
d.profile.apply_style(ProfileStyle::Flat);          // profile.rs:530
d.profile.crown_mm = 1.2;
d.profile.flatten_sides();                          // profile.rs:548; side draft 0, fillet shrunk
d.profile.comfort_fit_mm = 0.15;
d.size = resize::size_from_bore(18.6)?;             // resize.rs:29
d.shank.kind = ShankKind::Keyframes; d.shank.amount = 1.0;
d.shank.keys = vec![ShankKey { theta_deg: 90.0, thickness_scale: 1.18, ..Default::default() } /* ... */];

// Sand set-up: copy bestiarium_draco.rs:464-492 (branch bestiarium-draco), swapping the sand per ring.
let mut setup = mf::Setup::default();
setup.recipe = mf::Recipe::sand(SandProcess::DelftClay);   // or SandProcess::Petrobond
setup.recipe.alloy = "Silver 925".into();
setup.recipe.shrink_pct = ringdesign_core::metal::find("Silver 925").unwrap().shrink_pct;
setup.sample_pitch_mm = 0.1; setup.auto_parting = false; setup.parting_mm = 0.0;
setup.flask.width_mm = 80.0; setup.flask.length_mm = 80.0;
// Channels: a gate from the palm's outer surface outward along −Y, then a sprue, as in Draco.
d.draft.process = setup.recipe.process; d.draft.sand = setup.recipe.sand;
d.draft.min_detail_mm = setup.recipe.min_detail_mm; d.draft.min_section_mm = setup.recipe.min_section_mm;
d.draft.min_draft_deg = setup.recipe.min_draft_deg;
d.manufacturing = Some(setup);
```

Other shared rules:
- **Keys** are per-station multiples of the reference profile, where the reference is 1.0. Keeping every thickness key ≥ 1.0 and every width key at 1.0 makes the reference the tightest station. The reference-only side gate is then safe without P5 (the probe above).
- **Side-face gates** live on the window: `e.window.v_gate = VGate::SideFaces(SideFacePick::Both)` (`field.rs:420`). They work with the angular window disabled.
- **SVG alphas:** `d.svgs.push(SvgAlpha { name, svg, invert: false })`, then `d.bake_all(&mut lib)` (`lib.rs:398`). A layer's `mask: Some(name)` is sampled over the whole unrolled band as (u/circ, v/len).
- **Stones:** put a `SeatPadLayer` at the crest with `solid: SolidKind::Flush` and `through: true`. The finished ring shows the bur and the stone. The pattern (`mesh::try_build_pattern`, `mesh.rs:411`) leaves the bur out and raises the `mark_mm` drill dot.
- **Hide painting** (the fallback for any clamped layer until C-R1):
  1. `let a = Atlas::of(&d, 2048, 768)?; let hide = Hide::of(&a);`
  2. `let mut alpha = a.paint(name, |s| f(hide.at(s)));`
  3. `skin::draft_clamp(&a, &mut alpha, h)?`
  4. `skin::hide_layer(&d, name, h, window)`

  Draco's `paint()` (`bestiarium_draco.rs:586`) is the idiom, including its `DRACO_DEBUG` bite map.

### Gates per ring (plan section 5, cap of three review rounds)

1. **Draft build** at 768 × 320, which must pass all of these:
   - watertight, with 0 degenerate faces;
   - `csg::self_crossings == 0`;
   - `built.solids.notes` empty and `built.solids.stamped == d.stamps.len()`;
   - `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)`, with process `SandTwoPart` and verdict **`Castable`**;
   - release through `mf::inspect` plus `mf::release::analyze`, with 0 obstructions and 0 unresolved rays at both 0.100 and 0.075 mm;
   - clamp bite ≤ **0.05 mm** (plan; the brief's 0.02 stays as a target);
   - `dfm::findings_in(&d, &lib)` empty;
   - for the three stone rings, `stones::report` count equal to the `gems::preview_mesh` stones.

   `bestiarium_draco.rs:904-1020` implements all of this.
2. **Render set** in studio gold (`render::GOLD`): hero, face, palm, side, stone close-up, and bare stock against finished.
3. **Review round.** A fresh reviewer scores the ring against Caiman's hero, the Reptilia sheet and `b15/zbrush_top.png`, using this checklist:
   - one theme from face to palm;
   - figurative parts as true-outline stamps;
   - no faces and no eyes;
   - stones in made settings;
   - shoulder ornament not cut off at the face;
   - no flat, blocky CAD;
   - the factory's hard wall-to-face angles kept;
   - a silhouette distinct from the other seven;
   - density and legibility equal to or greater than Caiman's.
4. **Export build** at 1536 × 448 with `--verify`: a cold reload with an empty library gives identical vertices.
5. **Template gate:**
   - the design evaluates byte for byte, with an identical mesh;
   - at most 4 `design.set` patches;
   - bytes and open milliseconds per phase recorded in `verification.json`, from `ringdesign-graph/examples/template_open_probe.rs`;
   - budgets after P7: procedural ≤ 300 KB, stock ≤ 1 MB, painted atlases flagged above 3 MB.
6. **Reel:** stamp families named for `stamp_family`, so "Thorn, crest 3" plays as "Thorn, crest". Layer names are the captions.

### Author layout

The plan specifies `crates/ringdesign-core/examples/cataphracta/`:
- `main.rs`, with the CLI `-- OUT_DIR [--draft] [--verify] [SLUG]`, which refuses to overwrite;
- `common.rs`, with the set-up, gates, `write_ring` and `paint`;
- one module per ring.

In practice each Bestiarium ring became one file (`bestiarium_draco.rs`, `bestiarium_arachne.rs`). Either layout works; fix one before the first lane starts. In both layouts each lane owns only its ring's files.

---

## Sphenodon — *the parietal*

- **Status:** not started.
  - An **ungraded draft can be built now**. The thickness-only keys keep the reference side gate clean, and every non-graded API exists.
  - The final form needs **C-R2**, since the sail's grade is what C-R2 is calibrated on, and **C-R7**; its generators can live in the ring module first.
  - **P5** is needed only for the fillet granules' Draft gate.
- **Concept:** The last of the beak-heads, older than the dinosaurs' fall. A serrated sail stands on the parting line, the one place where a fin is two side faces. Where the sail begins, the parietal stone, a peridot, sits on the spine. The copy must never call it an eye.
- **Theme face to palm:**
  - **Face (90°):** the peridot on the crest. The sail parts round its mound.
  - **Shoulders (crest):** the sail. Its teeth rise behind the stone, are tallest at 90 ± 22°, and grade down both shoulders to a low saw at the palm.
  - **Crown flanks:** a polished ribbon either side of the sail, so the sail reads. Only the crown's edge fillet carries fine granules.
  - **Side faces:** granular skin with wandering longitudinal rows of enlarged tubercles (warped). Toward the palm they hand over to squarish ventral scales.
  - **Palm:** a low saw on the crest, and ventral squares on the side faces.
  - **Bore:** a plain comfort fit.
- **Base:**
  - Profile: `ProfileStyle::Flat`, 7.5 × 3.4, `crown_mm` 1.2, `flatten_sides()`, `comfort_fit_mm` 0.15, bore 18.6.
  - Shank: `Keyframes`, `amount` 1.0, **thickness-only** keys. Thickness is 1.18 at 90°, 1.05 at 0° and 180°, and 1.00 at 270°; width and crown stay at 1.0.
  - The reference, at the palm, is the tightest station. The side face is 2.2 mm at the palm and about 2.8 mm at the top.
- **Process:** Delft two-part. The sail's teeth need the finer floor.
- **Stones:**
  - Peridot, `Gem { preview_tint: Some([0.50, 0.78, 0.12]), ..Gem::calibrated(GemCut::Round, 3.0) }`.
  - Seated on a `SeatPadLayer` at (90°, `ctx.crest_v_mm`) with `style: GypsyMound`, `crown` 1.0, `blend_mm` 0.45, `solid: Flush` and `through: true`. Call `fit_stone`, then set `height_mm` to 0.6.
  - The mound is about 4.8 mm, plus its skirt, which spans roughly ±13° at this crest radius (about 13.3 mm).
  - Blend `Max`, as a top-level entry after the sail.
- **Build, step by step** (stack order is reel order):
  1. Base and sand set-up as above.
  2. **"The sail":**
     - Alpha: `SvgAlpha "Sail"` from `reptile::svg::sail` (C-R7). One tooth per tile. The u-profile is a triangular tooth with a 0.35 mm blunted tip and a rounded 0.4 mm valley; the v-profile is a gable peaked on the tile's middle row.
     - Tiling: `TilingLayer::default_for("Sail", &ctx)` with `repeats_around` 44, `rows` 1, `v_center_mm = ctx.crest_v_mm`, `v_span_mm` 0.95, `feather_mm` 0.0, `height_mm` 1.30. Since 44 × 90/360 = 11, a cell boundary lands on 90°, so the teeth are symmetric about the stone.
     - Grade: `grade: Some(TileGrade { taper: 0.4, theta_deg: 90.0, law: Cosine, isotropic: false })` (C-R2).
     - Entry: `mask: Some("Sail height")`, a **u-only** gradient SVG (0 over 90 ± 13°, 1.0 at 90 ± 22°, 0.35 at 270°), with `Blend::Max`.
  3. **"Tubercle rows":**
     - Alpha: an SVG with three rows of small granules and one row of domed tubercles per period, lands ≥ 0.4.
     - Tiling: `t.fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG)` (`tiling.rs:221`, which sets `rows`, span, `mirror_v`, feather and repeats), then `height_mm` 0.45 and grade Cosine with taper 0.3.
     - Warp: `warp: Some(WarpField { points: 8 guide points [k/8, face_mid ± 0.4], strength: 0.8, falloff_mm: 2.5 })`.
     - Gate: `window.v_gate = VGate::SideFaces(Both)`, which is **mandatory** under a warp (G8).
     - Blend: `SmoothMax` with `soft_mm` 0.2, plus a u-only mask that is the complement of the ventral mask.
  4. **"Ventral squares":** rounded-square pavers (`reptile::svg::paver`), fitted to the side faces with height 0.40. Mask: a u-only bump around 270 ± 35°. Blend `SmoothMax`, soft 0.2, gate `SideFaces(Both)`.
  5. **"Flank granules":** fine granules (`granules`, lands ≥ 0.4), height 0.20, with `window.v_gate = VGate::Draft { min_deg: 30.0, fade_deg: 6.0 }` (P5). Until P5 lands, leave this layer out: the ring reads without it.
  6. **"Parietal peridot":** the seat.
- **What it shows off:**
  - The collection's proof that a fin on the parting line is legal: a u-only height-field sail.
  - u-only masks.
  - A warp under a side gate.
  - C-R2's cosine grade.
  - A `SmoothMax` handover.
  - A flush stone on the spine.
  - A thickness-only keyframed body.
- **Traps and how they are avoided:**
  - *Fill.* The sail is 0.95 mm thick, against Delft's `min_section_mm` of 0.8, and its tips are 0.35 mm, against `MIN_EDGE_MM` of 0.2.
  - *Drag (G12).* The tooth walls face round the ring, so they count as vertical. They cover about 110 mm² of roughly 1500 (about 7%), under 12%. Measure with C-R5, or read the change in marginal plus vertical area with the sail muted.
  - *Warp spill (G8).* The warped layer is gated.
  - *Granules on a low crown (G3).* They are confined by the Draft gate to where the fillet has ≥ 30° of draft.
  - *Crest-facing mask walls (G7).* Every mask on the crown is u-only.
  - *The sail running into the stone's skirt.* Its mask is zero to ±13°, not ±6° as the brief had. The brief's ±6° put the teeth on the mound's skirt.
  - *The side-face web* (doctrine, "Two things the model does not yet know"). All side-face relief is additive; nothing is carved there.
- **Needs:**
  - C-R2 and C-R7 (`sail`, `tubercle_rows`, `paver`, `granules`);
  - P5 (the Draft gate, optional);
  - C-R5 (optional);
  - P7 (`shank.key`, for the template).
- **Template:**
  - An authored builder: `band.profile` → `shank` → `layer.tiling` ×4 fed by `alpha.svg` (or `script` nodes emitting the SVG) → `layer.seat` with `gem.calibrated`.
  - There are no stamps, so everything lifts natively. The keys ride a `/shank` patch until P7's `shank.key`.
  - Expose "Sail height", "Teeth" (repeats) and "Sail grade" (taper).
  - About 25 KB.
  - Script nodes need `ringdesign_script::registry()`. Run the golden check where that crate is visible (the workbench, or `ringdesign graph eval` in the CLI), because `ringdesign-graph` cannot depend on it.
- **Risk:** medium difficulty, low risk. Fallbacks:
  - If C-R2 slips, ship the sail ungraded.
  - If P5 slips, drop the flank granules.
  - If drag passes 12%, cut the tooth count to 36 (a cell boundary still lands on 90°) or lower the sail to 1.1 mm.

---

## Heloderma — *the beaded one*

- **Status:** not started.
  - Blocked on **C-R1** (the group clamp), **C-R2**, **C-R3**, **P5** (`VGate::Draft`) and **C-R7**.
  - A fallback path exists today: the painted atlas plus `MilgrainLayer`. Its template would be heavy (atlas PNGs).
- **Concept:** The one venomous lizard wears beadwork: domed osteoderms in two heights over a tail swollen with stored fat. The Gila's black and salmon banding becomes high beads and low beads under a reticulation mask. One bead on the spine is a spessartite.
- **Theme face to palm:**
  - **Face (90 ± 40°):** the fat-tail swell. The dorsal bead row runs the crest, with the spessartite as its central bead.
  - **Shoulders and crown flanks:** the beadwork, in reticulated high and low bands. Beads lie as flattened shingles near the crest and stand as full domes toward the edges.
  - **Side faces:** none. This is a half-round dome, and the beadwork runs to the edge fillet.
  - **Palm (270 ± 55°):** belly pavers, square flat-topped beads in transverse rows. They fuse into single plates across the crest ribbon.
  - **Bore:** comfort fit 0.2.
- **Base:**
  - Profile: `ProfileStyle::HalfRound`, 8.0 × 3.2, `comfort_fit_mm` 0.2, and `edge_round_mm` 0.3 set after `apply_style`. Bore 18.6.
  - Shank: `Keyframes`, `amount` 1.0.

    | θ | width | thickness | crown |
    |---|---|---|---|
    | 90 | 1.22 | 1.28 | 1.05 |
    | 35, 145 | 1.08 | 1.10 | 1.0 |
    | 210, 330 | 0.96 | 0.96 | 1.0 |
    | 270 | 0.90 | 0.92 | 1.0 |

- **Process:** Delft. The palm beads need the 0.30 mm floor's margin.
- **Stones:**
  - Spessartite, `Gem { preview_tint: Some([0.95, 0.38, 0.06]), ..Gem::calibrated(GemCut::Round, 3.0) }`.
  - Seated on a `SeatPadLayer` at (90°, `crest_v`) with `GypsyMound`, `crown` 1.0, `height_mm` 0.55 (after `fit_stone`), `blend_mm` 0.45, `solid: Flush`, `through: true`, `Blend::Max`.
  - The mound is about 4.8 mm across on a 9.8 mm swell top: a stone is sized to its face (collection3).
- **Build, step by step:**
  1. **Group "Beadwork":** `Layer::Group(GroupLayer { clamp: Some(SandClamp { resolution: [2048, 768], slack: 1.0 }), .. })` (C-R1), window `around(90, 300)` with fade 12. It holds three layers:
     1. **"Low beads — salmon bands":**
        - Alpha: `reptile::svg::bead_lattice`, one 2 × 2 hex-staggered cell of radial-gradient domes. The gradient focus is offset toward the band edge, so each bead ramps gently on its crest side and drops steeply on its edge side. Bead diameter at the 0.5 iso-line is 0.58 × pitch, and the land is 0.42 × pitch.
        - Tiling: `mirror_v` true, with the lattice spanning crest to edge, and grade `Cosine { taper 0.30, theta 90, isotropic: true }`. The pitch is 1.40 at the top and 0.98 at the palm.
        - Height 0.24, `remap: Remap::cushion(0.24)` (`field.rs:1670`).
        - Gate: `window.v_gate = VGate::Draft { min_deg: 24, fade_deg: 6 }` (P5).
        - Blend `SmoothMax`, `soft_mm` 0.18.
     2. **"High beads — black bands":**
        - The same alpha and lattice, so the beads coincide. Height 0.36, no remap.
        - `mask: Some("Reticulation")`: an SVG of forking transverse bands with `feGaussianBlur` edges of 1.2 mm, one image over the whole unrolled band.
        - Blend `SmoothMax`, soft 0.18. The two skins tie on the lattice, so `smax` (`field.rs:386`) crossfades between them rather than stacking them.
     3. **"Belly pavers":**
        - Alpha: `reptile::svg::paver`, rounded squares in transverse rows, one period per tile, lands 0.45. Across the crest ribbon the alpha drops its along-ring joints, so the rows fuse into plates (G4).
        - `remap: Remap::Terrace { steps: 1, span_mm: 0.20, riser: 0.35 }`.
        - Window `around(270, 110)`, fade 20.
  2. **"Dorsal bead row":** `Layer::SeatRun`.
     - Seat: `GypsyMound`, `diameter_mm` 1.1, `height_mm` 0.42, `blend_mm` 0.15, `v_mm` = `crest_v`.
     - Run: `bare: true` (C-R3), `taper` 0.35, `taper_theta_deg` 90, `bridge_mm` 0.35.
     - Window `except(90, 26)`, which clears the mound's skirt (±13°). The brief's ±8° did not.
     - Until C-R3 lands, use `MilgrainLayer { v_mm: crest_v, bead_diameter_mm: 0.9, beads_around: N, height_mm: 0.4, .. }`, uniform rather than graded. Alternatively, strike `stamp_row(PartingLine)` with `outline::circle(1.1)` and `top: StampTop::Dome { crown_mm: 0.4 }`, at the cost of about 60 CSG stamps.
  3. **"Spessartite":** the seat, as a top-level `Max` entry after the group, so the clamp never shaves its stock.
  4. **"Graver: bead lands":** `bench_only`, `Blend::Subtract`, 0.06 mm. It deepens the lands on the flanks after the pour, and the verdict and DFM skip it.

  *Painted fallback until C-R1:* compose layers 1.1 to 1.3 per atlas sample into one alpha, clamp it with `skin::draft_clamp`, and show it with `skin::hide_layer`. The reticulation and SmoothMax become baked.
- **What it shows off:** masks; tie-exact `SmoothMax`; gradient SVG with an offset focus; isotropic grading; the Draft gate; the live group clamp; a stone-less graded run; a keyframed swell.
- **Traps and how they are avoided:**

  | Trap | Avoidance |
  |---|---|
  | Beads off the crest lock (G3: −37°, 4.8%) | The offset-focus beads have an inner flank ≤ 20°, the gate starts at 24° of base draft, and the clamp guarantees the rest. Target bite ≤ 0.02 mm; the gate is 0.05 |
  | `SmoothMax` does not keep the draft rule | Clamp the **group composite**, not each layer |
  | Mask fades are walls facing the crest (G7) | The reticulation fades are ≥ 1.2 mm, so the bead-height step is 0.12 mm over 1.2 mm (about 6°), under the 24° gate |
  | The swell changes width (G6) | The clamp walks the modulated sections (`Atlas::of` does, P3), and the Draft gate is per station (P5) |
  | DFM gaps (G11): Caiman's granules measured 0.05 mm | Lands are 0.42 × pitch, so ≥ 0.41 mm at the palm's 0.98 pitch. Assert it with `min_feature_px` in the generator's test |
  | Tight seats merge into a ridge (commissions) | Mound `blend_mm` 0.15, bridge 0.35 |
  | The stone gap is too narrow | `except(90, 26)` |

- **Needs:**
  - C-R1, C-R2, C-R3;
  - P5 (the Draft gate);
  - C-R7 (`bead_lattice`, `reticulation`, `paver`);
  - P7 (`shank.key`).
- **Template:**
  - An authored builder: `band.profile` → `shank` (keys) → `layer.group` with a clamp pin (the C-R1 lane adds the pin and its lift). The group holds three `layer.tiling` nodes fed by `alpha.svg`, with `remap.curve`, `remap.terrace` and 4 `window` nodes.
  - Then `layer.seatrun` with a `bare` pin (added by C-R3), `gem.calibrated` + `layer.seat`, and an `entry` node for the bench layer.
  - Patches: `/draft`, `/manufacturing` and `/build` (3 of the 4 allowed).
  - Expose "Bead pitch", "Bead grade" and "High bead height".
  - About 60 KB. The painted fallback is about 3 MB and is flagged.
- **Risk:** high difficulty, medium risk. The shingle slope against the gate calibration is the unknown: measure the bite per column (a bite map, as Draco's `DRACO_DEBUG` does) before tuning. Fallbacks, in order:
  1. Ungraded lattice.
  2. One bead height, with the reticulation cut at the bench.
  3. Milgrain for the crest row.

---

## Moloch — *the thorn idol*

- **Status:** not started.
  - **Can be built now ungraded.** P4's cone tops and `stamp_row` have landed, and the thickness-only hump keeps the reference side gate safe.
  - The graded rosettes need **C-R2**; the SVGs need **C-R7**.
- **Concept:** The thorny devil is named for the god that devoured children. Every scale is a thorn. A false head rises on its nape, and the grooves of its skin drink the dew and carry it to its mouth.
- **Theme face to palm:**
  - **Face (90°):** the false head, a hump that rises but does not widen. It is crowned by the largest crest thorn, and two great horns thrust along ±Z out of the side faces. It is a hump, not a face: no eyes or mouth.
  - **Crest (both shoulders):** a graded row of cone thorns on the parting line over a knife keel, shrinking to studs at the palm.
  - **Crown flanks:** capillary grooves straight across the band.
  - **Side faces:** thorn rosettes (a central cone ringed by six granules), graded.
  - **Palm:** the rosettes give way to plain granules through a u-only `SmoothMax` handover.
  - **Bore:** plain.
- **Base:**
  - Profile: `ProfileStyle::Flat`, 7.0 × 3.6, `crown_mm` 1.4, `flatten_sides()`, `comfort_fit_mm` 0.15, bore 18.6.
  - Shank: `Keyframes`, **thickness only**: 1.50 at 90°, 1.25 at 60° and 120°, 1.00 at 15° and 165°, 1.00 at 270°.
  - The reference is the palm.
- **Process:** **Petrobond**, which Logan confirmed he pours: 0.40 mm detail, 2.5° draft, 0.6 mm section, `mf::Recipe::sand(SandProcess::Petrobond)`. Every feature is ≥ 0.5 mm.
- **Stones:** none. The thorns are the jewel.
- **Build, step by step:**
  1. **"Thorn rosettes":**
     - Alpha: `reptile::svg::rosette_thorn`. The central cone is a linear radial gradient to a 0.3 mm flat tip, ringed by six granule domes; lands ≥ 0.45.
     - Tiling: `fit_to_side_faces(&ctx, SIDE_FACE_MIN_DRAFT_DEG)` (which sets `mirror_v` on this even pair), height 0.80, grade `Cosine { taper 0.45, theta 90 }` (C-R2).
     - Entry: gate `SideFaces(Both)`, `Blend::Max`, window `around(90, 330)` with fade 12, and `mask: Some("Rosette reach")`, the u-only complement of the palm mask.
  2. **"Palm granules":** SVG granules at 0.9 mm pitch (0.5 mm across, 0.45 mm lands), fitted to the side faces, `SmoothMax` with soft 0.2. The mask is `"Palm reach"`, a u-only bump centred on 270° (the idiom is `fade_svg`, `examples/showoff.rs:84`).
  3. **"Crest keel":** `Layer::Curve(CurveLayer { points: vec![[0.0, crest_v], [0.5, crest_v]], repeats_around: 1, closed: true, width_mm: 1.2, height_mm: 0.15, profile: WireProfile::Knife, taper: 0.0, mirror_v: false })`. *Unverified:* that a two-point closed path renders as a straight full-ring rail. `CurveLayer::preset_wave_rail` (`curve.rs`) is the closed-rail idiom to copy if it does not.
  4. **"Capillary grooves, fingertip flank" and "…, knuckle flank":**
     - Two `Layer::Flutes(FlutesLayer { count: 48, profile: FluteProfile::Vee, width_mm: 0.5, height_mm: 0.12, lean: 0.0, along: false })` entries, each with `Blend::Subtract`.
     - Each gets `window.v_gate = VGate::Band { .. }` over one crown flank, running from 1.3 mm off `crest_v` (clear of the keel and the thorn roots) to the fillet, with fade 0.3.
     - The brief had one crown-wide band. That runs grooves under the crest thorns, which breaks G9.
  5. **Stamps "Thorn, crest":**

     ```rust
     let proto = Stamp { name: "Thorn, crest".into(), theta_deg: 0.0, v_mm: 0.0, rot_deg: 0.0,
         outline: outline::circle(2.4), height_mm: 0.10, sink_mm: 0.3, draft_deg: 4.0,
         cut: false, bench: false, along_pull: false, tier: 0,
         top: StampTop::Cone { apex_mm: 1.2, at: [0.0, 0.0], tip_mm: 0.3 } };
     d.stamps.extend(setting::stamp_row(&d, &StampRow { stamp: proto, path: RowPath::PartingLine,
         from_deg: 90.0, to_deg: 262.0, count: 20, taper: 0.62, fold_clear_mm: 0.0, mirror_shoulders: true }));
     ```

     - This gives "Thorn, head" plus 19 per side, 39 in all, from 2.4 mm across down to about 0.9 mm.
     - Keep `at[1] = 0`. An apex off the parting line fails `parting_monotone`, which the `cone([0.0, 0.4])` test pins.
     - Assert `parting_monotone(&d).is_ok()` on every stamp.
  6. **Stamps "False-head horn, fingertip" and "…, knuckle":**
     - One per side face, at 90°. `v` is the middle of the station's own face: find it on `Atlas::of`, in the 90° column, among rows with |n.z| > 0.98. The reference face's middle also lands inside it, because the reference is the tightest station.
     - `outline::rounded_triangle(3.0, 2.6, 0.4)`, `along_pull: true`, `draft_deg` 4, `sink_mm` 0.3.
     - `top: StampTop::Cone { apex_mm: 1.6, at: [0.0, 0.2] /* tunable: leans the tip toward the crest */, tip_mm: 0.35 }`.
     - They stand 1.6 mm proud of the band along the finger, which is the ring's silhouette and close to Logan's Odin and Valkyrie spikes.
- **What it shows off:**
  - Cone-topped stamp rows on the parting line (P4 at count).
  - Along-pull horns, a silhouette the height field cannot make.
  - Graded side-face fields and a u-only handover.
  - A thickness-only keyframed hump.
  - Flutes as grooves, and a knife keel.
  - Petrobond.
- **Traps and how they are avoided:**
  - *Off-crest cones lock (G3).* Cones stand only on the crest, straddling it (G2), and on the side faces (G1).
  - *Grooves across a widening section lean (G6).* The hump never widens.
  - *Cone caps on a surface that curves both ways give 0.03–0.07 mm phantoms* (Caiman). The knife keel supplies a gable, the grooves are gated off the crest, and the hump's along-ring curvature is gentle (a 1.8 mm rise over ±30°). Verify at 384 × 192 as well as at the ring's own resolution.
  - *Groove ends (G7).* The outer end of a subtractive groove rises walking away from the crest: 0.12 mm over 0.3 mm, about 22°. Keep that end inside ≥ 25° of base draft, or run the grooves out through the fillet onto the side face, where they are free.
  - *Feather tips will not fill* (`MIN_EDGE_MM`). Every apex is a flat ≥ 0.3 mm.
  - *The side-face web.* All side-face relief is additive.
  - *Drag.* The squared faces have perfect draft, and crown 1.4 keeps the low-draft strip narrow. Measure it (C-R5).
  - *Build time.* About 41 stamps. Oriel's 37 stones resolved in 4.3 s at 1.38 M faces, so budget about 5 s at export.
- **Needs:**
  - P4 (landed);
  - C-R2 and C-R7 (`rosette_thorn`, `granules`);
  - C-R5 (optional);
  - P7 (`stamp.row` nodes).
- **Template:**
  - Tilings, flutes, the curve and the entries all lift natively.
  - Stamps ride a `/stamps` patch until P7. After P7 they become one `stamp.row` node, exposing "Thorn grade" (taper) and "Thorns per shoulder" (count).
  - About 40 KB after P7. Before it, the expanded stamps (about 41 × 75 points) run to roughly 150 KB.
  - Note: an older build silently drops `tier` and `top` from a `/stamps` patch.
- **Risk:** high difficulty, medium risk, from the stamp count and the CSG time. Fallbacks:
  - Ship the rosettes ungraded.
  - If a horn's cone self-crosses, lower `apex_mm` to 1.3 or move `at` back to [0, 0].
  - As a last resort, make the horns CAD features: sketch a circle, `Extrude` with taper, `Placement::Ring`, attach Join, stage Cast. Release inspection then has to judge them, because the field verdict does not.

---

## Gekko — *the tokay*

- **Status:** not started.
  - Blocked on **C-R1** (the crown clamp), **C-R2** (the graded lamellae) and **C-R7**.
  - Fallbacks today: the crown can be painted with `Atlas`/`draft_clamp`, and the lamellae painted as a series from `skin::Joints::eccentric`, which already has the cosine law.
  - **P5** is optional: the keys are thickness-only.
- **Concept:** The tokay climbs glass on lamellae. The ring grips the finger the way the gecko grips glass:
  - lamellae under the palm;
  - tubercle rows spiralling down the side faces;
  - a granular crown scattered with raised spots.

  It has no wings, no stone, and no eyes (a tokay's eyes are its best-known feature, and they stay out).
- **Theme face to palm:**
  - **Face and crown:** granules, with raised spots as terraced plateaus, clamped.
  - **Shoulders:** the granules and spots continue on the crown, and the tubercle rows carry on along the side faces.
  - **Side faces:** rows of enlarged tubercles on the helix shear, so the rows spiral round the ring.
  - **Palm:** lamellae, plates across the band that vary only along the ring. The pitch grades from 0.9 mm at 270° to 1.4 mm at their ends. At 270° sits the split lamella, a chevron with its point on the parting line.
  - **Bore:** plain.
  - **Bench:** the scale lines.
- **Base:**
  - Profile: `ProfileStyle::Flat`, 7.0 × 3.4, `crown_mm` 1.2, `flatten_sides()`, `comfort_fit_mm` 0.15, bore 18.6.
  - Shank: `Keyframes`, thickness only, all ≥ 1.0. The plan fixes only the top value of 1.2. Proposed keys: 1.20 at 90°, 1.10 at 30° and 150°, 1.02 at 210° and 330°, 1.00 at 270°.
- **Process:** Delft.
- **Stones:** none.
- **Build, step by step:**
  1. **Group "Crown granules and spots":** clamp (C-R1), window `except(270, 100)` with fade 20, `v_gate` `Band` over the crown between the fillets. It holds two layers:
     1. **"Granules":** `reptile::svg::granules` (lands ≥ 0.4), height 0.20.
     2. **"Spots":** `reptile::svg::granule_spots`, irregular round spots 1.5–3 mm across, a low-frequency mask. `remap: Remap::Terrace { steps: 2, span_mm: 0.30, riser: 0.35 }`, height 0.30, `SmoothMax` with soft 0.2.

     The clamp guarantees that nothing rises away from the crest faster than the draft allows (G3).
  2. **"Lamellae":**
     - Alpha: `reptile::svg::lamella`, one plate per tile, a u-sawtooth (a gentle loaf rising to a 0.4 mm trailing drop). It is constant across the band, which makes it legal on the crown by G4.
     - Tiling: `rows` 1, the v span covering the crown between the fillets, grade `Cosine { taper, theta 270 }` (C-R2) giving pitches of 0.9 → 1.4.
     - Window `around(270, 100)` with fade 20 (u-only), except the centre plate's own span (a second, inverted window entry or a u-only mask).
     - The trailing drops face away from 270° on both sides.
  3. **Stamp "Split lamella":**
     - At (270°, parting line), with a chevron outline whose apex sits on the parting line and leads. It is Saurian's pointed scute: the wall faces round the ring and away from the line.
     - Built with `outline::rounded_polygon(&[...], 0.2)`, a flat top, `height_mm` 0.3 and `draft_deg` 4.
     - Assert `parting_monotone`.
     - The lamellae are windowed off beneath it, so the surface under the stamp varies only across the band (G9).
  4. **"Tubercle rows":** Sphenodon's `tubercle_rows` alpha, `fit_to_side_faces`, `mirror_v`, `shear` 0.35 (`tiling.rs:162`; any value stays seamless), height 0.40, gate `SideFaces(Both)`. A shear, like a warp, moves the sampling, so the gate is mandatory (G8).
  5. **"Graver: scale lines":** `bench_only`, `Blend::Subtract`, 0.06 mm.
- **What it shows off:** the helix shear; graded lamellae (G4 on a crown); a clamped terrace-remapped group; a chevron stamp on the parting line; the bench stage.
- **Traps and how they are avoided:**
  - *Crown granules and spots off the crest (G3).* The group clamp. Design the spot plateaus (≤ 0.3 mm, terraced) so the clamp bites close to 0, and print the `ClampReport`.
  - *Column-wise clamp combing* (the Zenith lesson: the clamp combs every edge it bites). Keep the bite ≤ 0.05 mm, and inspect a bite map.
  - *Lamella drops are vertical walls (G12).* Measure drag. A 0.4 mm drop per 0.9–1.4 mm plate over ±50° is modest.
  - *The split lamella (G9).* The lamellae are windowed off under it, it passes `parting_monotone`, and a procedural band has no fold.
  - *DFM (G11).* Granule lands ≥ 0.4 mm, lamella drop 0.4 mm, smallest pitch 0.9 mm, all checked with `min_feature_px` in the generator tests.
- **Needs:**
  - C-R1 and C-R2;
  - C-R7 (`granules`, `granule_spots`, `lamella`, `tubercle_rows`);
  - P7 (`shank.key` and stamp nodes);
  - P5 (optional).
- **Template:**
  - An authored builder with the SVGs as text and a clamp pin on `layer.group`.
  - The one stamp rides `/stamps` until P7.
  - Expose "Lamella pitch", "Lamella grade", "Spot height" and "Tubercle shear".
  - About 40 KB.
- **Risk:** medium difficulty, medium risk. The spot terraces under the clamp are the unknown. Fallbacks:
  - Keep the spots to a lower 0.2 mm plateau on the upper flanks only.
  - Drop the chevron stamp and use a plain centre lamella.

---

## Ouroborus — *the girdled wheel*

- **Status:** not started.
  - Blocked on **P5**, which is **required**: the body runs both wider and narrower than the reference, so the reference-only gate spills.
  - Blocked on **C-R2's `Spiral` law**, also required.
  - Also needs **C-R1** (the head shield; a painted fallback exists) and **C-R7**, plus P7 for its keys as nodes.
  - There is no build of the side-face spines without P5.
- **Concept:** The lizard named for the ring. Threatened, it takes its own tail in its mouth and becomes a wheel of spines. The band is its body: an armoured head, girdled whorls graded from nape to tail, and the tail's tip meeting the head at the top.
  - It is a sequel to Serpentarium's Ouroboros (a snake in warped mail, which Logan loved); here the reptile is a lizard and the skin is whorls.
  - The head is plates only: no eyes, no nostrils, no teeth.
- **Theme face to palm, going round:**
  - **93° → 125°:** the head shield, terraced cephalic plates stepping down from the crest.
  - **125°, through the palm, to 78°:** the whorls. Each is a transverse girdle rising to a trailing edge. They grade from 2.6 mm at the nape to 1.0 mm at the tail tip, and the tail runs toward increasing θ.
  - **Crest:** a knife keel from the nape to the tail tip.
  - **Side faces:** each whorl's trailing edge breaks into triangular spines pointing tailward.
  - **Palm:** mid-tail, where the whorls are already small.
  - **At 90°:** the bite, where the head's front meets the tail tip.
  - **Bore:** plain.
- **Base:**
  - Profile: `ProfileStyle::Flat`, 7.0 × 3.2, `crown_mm` 1.3, `flatten_sides()`, bore 18.6.
  - Shank: `Keyframes`, asymmetric. The extra key at 102° tames the Catmull-Rom overshoot across the jump.

    | θ | width | thickness | crown |
    |---|---|---|---|
    | 93 | 1.25 | 1.35 | 0.9 |
    | 102 | 1.22 | 1.30 | 0.95 |
    | 130 | 1.05 | 1.10 | 1.0 |
    | 190 | 1.10 | 1.15 | 1.0 |
    | 280 | 0.95 | 1.00 | 1.0 |
    | 10 | 0.72 | 0.85 | 1.0 |
    | 82 | 0.55 | 0.70 | 1.0 |

  - Thickness stays at or above width at every key, but the tail runs thinner than the reference, which shrinks the side face (to about 0.94 mm at 82°). The reference-only gate therefore spills onto the fillet at the tail.
- **Process:** **Petrobond** (Logan confirmed). The whorls are ≥ 1 mm.
- **Stones:** none.
- **Build, step by step:**
  1. **"Whorls":**
     - Alpha: `reptile::svg::whorl`. One period is one girdle: a u-sawtooth rising to the trailing edge (a gentle loaf, then a steep 0.4 mm drop), constant across the crown (G4).
     - Grade: `TileGrade { law: Spiral { seam_deg: 90.0 }, .. }` (C-R2), pitch 2.6 → 1.0. The lattice kink lands under the bite.
     - Height 0.55.
     - Window `Window::around(281.5, 313)` with fade 6. That is 125° round to 438° (= 78°).
     - `v_gate` `Off`.
  2. **"Whorl spines":**
     - The same grade and `repeats_around` as the whorls, so the spines align with them by construction.
     - Alpha: `reptile::svg::whorl_spine`, triangular pyramids on the trailing edge pointing toward +u.
     - `fit_to_side_faces`, `mirror_v`, `window.v_gate = VGate::SideFaces(Both)`, which must be **station-aware (P5)**. Same window as the whorls.
  3. **Group "Head shield":**
     - Clamp (C-R1), window `around(109, 32)`.
     - Alpha: `reptile::svg::plate_voronoi`, polygonal plates from mirrored seeds.
     - `remap: Remap::Terrace { steps: 3, span_mm: 0.5, riser: 0.3 }` (`field.rs:1634`).
     - The terraces step down away from the crest, and the clamp takes whatever the fast width change leans (G6).
  4. **"Dorsal keel":** a knife `CurveLayer`, closed, at `crest_v`, width 1.0, height 0.18, window `except(109, 34)`.
  5. **"Scale divisions":** `bench_only`, `Blend::Subtract`, 0.08 mm. These are the along-ring lines dividing each girdle into scales on the crown. They would lock if cast (G3), so the graver cuts them.
- **What it shows off:**
  - An asymmetric keyframed body.
  - The new Spiral grade.
  - Aligned multi-layer grading.
  - A terrace remap.
  - The bench stage.
  - A knife keel.
  - Station-aware side gates, used for the first time.
- **Traps and how they are avoided:**
  - *The width jump at the bite (G6).* The whorls are windowed off the bite by about 12°, and the head is clamped.
  - *Catmull-Rom overshoot.* The key at 102°. Sweep `d.section_at(θ, …)` every 0.5° from 70° to 115°, and confirm every section stays single-crested with an edge ≥ `MIN_EDGE_MM`.
  - *The reference-only side gate* (plan F4; it spills 0.1–0.8 mm, and 3.43% at −27.4° was measured). P5 is required.
  - *Sawtooth drops are vertical walls (G12).* A 0.4 mm drop on a 1.0–2.6 mm pitch. Measure (C-R5).
  - *DFM at the tail tip (G11).* The smallest pitch is 1.0, with a 0.6 loaf and a 0.4 drop; the spines' base is 0.55 wide at the tail. All sit above Petrobond's 0.40.
  - *"A seam along the crest is a valley"* (the bypass lesson). The keel is a ridge, not a seam.
  - *The Spiral seam.* It lands at 90°, under the windowed-off bite, so its kink never shows.
- **Needs:**
  - P5 (required);
  - C-R2 with the `Spiral` law (required);
  - C-R1;
  - C-R7 (`whorl`, `whorl_spine`, `plate_voronoi`);
  - C-R6 (terrace treads) and C-R5 (optional);
  - P7 (`shank.key`).
- **Template:**
  - The keys become P7 `shank.key` nodes, exposed as "Head", "Body" and "Tail tip". Before P7 they ride a `/shank` patch.
  - The tilings and the group lift natively.
  - About 35 KB.
- **Risk:** high difficulty, medium risk. The Spiral law is new maths, but it is `eccentric_warp`'s idea with a monotone law in place of a symmetric one. Fallbacks:
  - If the Spiral law slips, use a Cosine grade centred on the nape (130°). It is seamless, but reads less like a tapering tail.
  - If P5 slips, drop the side-face spines. The whorls alone still read, but the wheel of spines is lost.

---

## Chelonia — *the carapace seal*

- **Status:** not started. **Blocked on a base decision.**
  - The planned base, the 007 Quatrefoil sand master, fields **NotCastable, 14.7% at −64.1°, bare**, and its envelope refuses to fill over 4 mm (master `CLAUDE.md`, from `examples/stock_spike.rs`). The field verdict does not see the envelope in any case.
  - After the base: C-R1, C-R4, C-R7 and C-R8 (C-R6 is nice to have). P3 has landed.
- **Concept:** The turtle is the seal. The quatrefoil's four lobes are its flippers and its middle is the shell. Every scute is ringed with the years it grew. The shoulders carry the hawksbill's overlapping tortoiseshell, and the palm is the plastron. No head is drawn: the table's two along-ring cusps are the neck and tail notches.
- **Theme face to palm:**
  - **Table:**
    - vertebral scutes on the parting line, 5 along the ring, straddling it, the tallest tier;
    - costal scutes stepping down on each side, 4 per side, their areolae on the parting-line edge;
    - marginals as the lowest tier at the rim;
    - 3 terraced growth annuli on every scute;
    - flipper-scale tiers on the four lobes, stepping down along each flipper.
  - **Cheeks (walls):** the shell's side, with vertical serration grooves under each marginal (G1).
  - **Shoulders:** hawksbill imbricate plates. Their rounded free edges lead toward the palm, graded 2.4 → 1.4 mm.
  - **Palm:** the plastron, broad plates with transverse seams (G4). The central seam is cut at the bench, because a seam on the crest is a valley.
  - **Bore:** plain.
- **Base, step 0 (decide before drawing anything):**
  - *Planned:* `PRESETS` "007" loaded through `sand_master`, with `sand_envelope = true`, `shank.head.length_mm` 16 and `profile.width_mm` 17 (0.92 of 18.5, inside the guard). Then bore 18.6, `apply_style(Flat)`, edge 0.3, comfort 0.1, and `chart = Some(SurfaceChart { .. })` set before any layer. `bestiarium_draco.rs:450-492` has the pattern.
  - *Measure* the bare master at 16 × 17: `examples/common/probe.rs::stock("007", true, Some((16.0, 17.0)))`, then `attributed_field_report` and `probe::pull`. The spike measured only the native size.
  - **If it is NotCastable (expected), Fallback A** (recommended; it keeps sand): a procedural lofted signet on the bundled factory outline **"CG Quatrefoil"** (`bundled/outlines/CG Quatrefoil.outline.json`):

    ```rust
    let o = ringdesign_core::library::list_outlines().into_iter().find(|o| o.name == "CG Quatrefoil").unwrap(); // library.rs:445
    d.shank.apply_signet(d.profile.width_mm);          // profile.rs:1853: kind Signet, loft 1.0
    let v = d.shank.adopt_outline(o);                  // returns SignetOutline::Custom(i)
    d.shank.head.outline = v;
    d.shank.head.length_mm = 16.0;
    d.shank.head.dome = d.shank.suggest_dome(v);       // profile.rs:1820; likely 1.0 for four lobes (unverified)
    ```

    - Build both `dome = suggest_dome(v)` and `dome = 0` (pure loft), and keep whichever reads better with the flippers.
    - Per the doctrine, every bundled outline fields 0.000% on a bare head.
    - The loft is CrossGems' own construction, within 0.04–0.05 mm of their cached meshes on the head. It is not the stock mesh, though: the wall-to-face angle becomes the loft's rim roll.
    - Ask Logan whether that meets his "factory stock for signets" rule.
  - **Fallback B:** the real 007 stock, unmirrored, in lost wax, the route the upright plans take. It keeps the exact mesh, but the collection's "all poured in sand" claim drops to seven rings.
  - Do not choose silently. Logan's standing rule is that signet preset choices go to him first.
  - Then **gate on drag** before a single scute: the bare table's (marginal + vertical) / total must be under 12%. If it is over, add `table_dome_mm` 0.7–0.8 (procedural head), which took a flat-table signet from 13–23% drag to 5.3% (Palisade), or raise the sand master's crown term (stock).
- **Process:** Delft.
- **Stones:** none. The carapace is the jewel.
- **Build, step by step** (hide space, everything in clamped groups):
  - First: `let a = Atlas::of(&d, 2048, 768)?; let hide = Hide::of(&a); let folds = hide.folds(&a, deg_per_mm);`. This gives along (mm along the parting line from the head's centre), across (mm from the line), and each column's rim and wall.
  1. **Group "Carapace":** clamp (C-R1), window `around(90, 110)`. It holds five layers:

     | Layer | Placement | Relief |
     |---|---|---|
     | "Vertebral scutes" | Hide-space tiling (C-R4, `space: Hide`), along pitch 3.1, across ±2.2. `reptile::svg::scute(areola: Centre)`, a hexagonal scute with a radial-gradient areola | `Remap::Terrace { steps: 3, span_mm: 0.18, riser: 0.35 }`; tier top 0.55 falling to 0.40 at the edge |
     | "Costal scutes" | Across 2.2 → 5.4, four per side, mirrored across the line in hide space. `scute(areola: CrestEdge)`, with the focus offset toward the line | Terrace, 3 steps, 0.30 → 0.15. The seam to the vertebrals is a **step down**, never a groove (G5) |
     | "Marginals" | Across ≥ rim − 1.2; rectangles with a serrated trailing edge toward the tail notch | 0.08 → 0 |
     | "Flippers" | Masked to the lobes by `plan_mask("007")` (C-R8), or by the `CustomOutline` table under Fallback A. Rows run across each flipper's axis | Stepping down away from the shell |
     | Costal seams | Across the band | 0.1 mm V grooves (G4) |

  2. **"Shell side":** a cheek tiling (mask `"##region:cheek"`, C-R4) with vertical grooves under the marginals, height 0.25, `Max`.
  3. **Group "Hawksbill plates":** clamp, window `except(90, 110)`, and a palm window taken out by a u-only mask.
     - Alpha: `reptile::svg::shingle` in hide space. Its free edges are U-shaped and lead palmward, with the U's apex on the parting line; the wall faces palmward and away from the line (G4).
     - Full width across the shank, graded `Cosine { taper 0.4 }` (C-R2).
  4. **"Plastron":** `reptile::svg::plastron`, with transverse seams, window `around(270, 100)`.
  5. **"Graver: growth striae and the plastron's seam":** `bench_only`, `Blend::Subtract`.

  *Fallback until C-R1 and C-R4 land:* Caiman's painted method. Paint each region from `hide.at(s)` in true millimetres, then `draft_clamp`, then `hide_layer`. It works today; the template is then about 10 MB and cannot be edited.
- **What it shows off:** the clamp as a live constraint on a table; hide-space tilings; terrace remaps as growth rings; a plan-derived mask; tiers.
- **Traps and how they are avoided:**
  - *The base's own undercut.* Step 0.
  - *The table rule (G5).* Every tier descends from the parting line, including the vertebral-to-costal seam. Growth rings centred on or beside the line descend with |z| by geometry.
  - *The fold.* The quatrefoil's along-ring cusps are where the parting line turns over the end walls. The nuchal and supracaudal scutes stop 1 mm short of `folds`.
  - *Column-wise clamp combing* (Zenith). Design the tiers so the clamp bites close to 0, and print each layer's `ClampReport`.
  - *DFM does not see remaps (G11).* Make the terrace treads ≥ 0.40 mm by hand: 3 steps on a 1.5 mm scute radius gives 0.5 mm treads. C-R6 fixes this properly.
  - *Flat-table drag (G12).* Measured in step 0.
  - *The central plastron seam.* It is cut at the bench.
- **Needs:**
  - the base decision;
  - C-R1, C-R4, C-R2;
  - C-R7 (`scute`, `shingle`, `plastron`);
  - C-R8;
  - C-R6 (nice to have);
  - P7 (`base.preset`, on the stock path).
- **Template:**
  - Stock path: `base.preset("007", sand)` (P7) → `layer.group(clamp)` → hide-space `layer.tiling` nodes → `remap.terrace`. Expose "Scute pitch", "Growth rings" and "Tier drop". About 50 KB after P7; before P7 the stock rides a ~3 MB `/imported_base` patch.
  - Fallback A path: `shank.signet` + `outline.library("CG Quatrefoil")`, with no imported base at all. About 50 KB even before P7.
- **Risk:** very high difficulty, medium-high risk. It depends on C-R1, C-R4, C-R8 and the base decision, and it is the ring most likely to become Logan's favourite. Fallbacks:
  - For the base: Fallback A, then B.
  - For C-R4: the painted method.
  - If the lobes cannot carry tiers cleanly: polish the flippers and keep the tiers to the shell.

---

## Phrynosoma — *the horned crown*

- **Status:** not started. **Blocked on the bare 016 verdict.**
  - The 016 Star sand master fields **NotCastable, 2.9% at −7.6°, bare**. Its envelope rewrites it by 0.44 mm, but only in the mesh build; the field verdict still reads the master itself.
  - After the base: C-R1, C-R4 and C-R7. P3 and P4 have landed.
  - It does **not** need C-R3: the crest tubercles are a domed `stamp_row` (see below).
- **Concept:** The horned lizard wears its crown at the back of its skull. The star's points become horns, the table its plated skull, and its fringe runs down both shoulders. The copy calls them "skull plates" and uses no eye words.
- **Theme face to palm:**
  - **Table:** cephalic plates. A midline row of large plates straddles the parting line, and flanking plates step down each side. They are polygonal and drafted, in 3 tiers.
  - **Star points (six):**
    - the across-band pair: the great occipital horns, 2.4 mm, thrust along ±Z;
    - the four diagonals: temporal horns, 1.6 mm;
    - the two along-ring points sit on the fold and stay low and polished.
  - **Cheeks:** enlarged tubercles among granules.
  - **Shoulders' walls:** the lateral fringe, pointed scales along each shoulder's rim, graded toward the palm.
  - **Shoulders' crest:** a graded row of round tubercles on the parting line.
  - **Palm:** fine ventral granules.
  - **Bore:** plain.
- **Base, step 0:**
  - *Planned:* `PRESETS` "016" through `sand_master`, `sand_envelope`, face 17 × 17 (0.89 × 0.85 of 19 × 20), bore 18.6, Flat chart.
  - *Measure* the bare master at 17 × 17 (probe as for Chelonia), and use the field notes to find where its undercut sits.
  - If it stays NotCastable, take **Fallback A**: a procedural lofted signet on **"CG Star"**. `suggest_dome` is likely 1.0 for a six-point star (unverified); build pure loft as well and compare.
  - **Fallback B:** 016 stock in lost wax.
  - Take the choice to Logan.
- **Process:** Delft.
- **Stones:** none. The horns are the crown.
- **Build, step by step:**
  1. **Group "Skull plates":** clamp (C-R1), hide space (C-R4).
     - Alpha: `reptile::svg::plate_voronoi`, with seeds mirrored across the line. Each plate is a drafted plateau whose height is set by its tier: 0.55, 0.35 or 0.15. Joints are 0.35 mm V-grooves, and tier edges are steps down (G5).
     - The seed layout is constrained so that same-tier neighbours meet only along across-band joints.
  2. **"Cheek tubercles":** mask `"##region:cheek"` (C-R4), `reptile::svg::rosette_tubercle` with lands ≥ 0.45, height 0.35.
  3. **"Lateral fringe":** a height-field tiling on the shank walls, mask `"##region:wall"`. Pointed triangular scales tipped palmward (`reptile::svg::fringe`), grade `Cosine { taper 0.4 }`. It is texture, not a figurative motif, so it is not a stamp. The walls face the pull (G1).
  4. **Stamps "Crest tubercle":**
     - `stamp_row(&d, &StampRow { stamp: /* outline::circle(1.2), top: StampTop::Dome { crown_mm: 0.35 }, height_mm: 0.05, sink_mm: 0.2, draft_deg: 4.0 */, path: RowPath::PartingLine, from_deg: /* the head's shoulder end, ~130 */, to_deg: 262.0, count: 12, taper: 0.42, fold_clear_mm: 1.0, mirror_shoulders: true })`.
     - They run from 1.2 mm across down to 0.7 mm.
     - This replaces the brief's bare seat run: a `SeatRunLayer` holds one `v` (`field.rs:1917`), and cannot follow a stock parting line.
  5. **Stamps "Horn, occipital, fingertip" and "…, knuckle", then "Horn, temporal, fingertip 1/2" and "…, knuckle 1/2":** six stamps on the cheeks below the star points.
     - `along_pull: true`.
     - Outline `outline::rounded_triangle(2.6, 2.0, 0.3)` for the temporal horns and `(3.0, 2.4, 0.3)` for the occipital ones.
     - `top: StampTop::Cone { apex_mm: 1.6 or 2.4, at: /* inside the root, offset toward the point */, tip_mm: 0.35 }`, `draft_deg` 4.
     - `rot_deg` aligns each root with its point.
     - Placement: the atlas sample with the highest `a.cheek(s)` whose plan bearing is nearest the point. The point bearings are the maxima of `Preset::plan`'s radii, or of the outline's table under Fallback A.
     - No horn stands at an along-ring point.
- **What it shows off:**
  - Along-pull cone horns, a silhouette the height field cannot make.
  - A clamped Voronoi tier mosaic on a table.
  - Region masks in hide space.
  - A domed stamp row on a factory shank's parting line.
- **Traps and how they are avoided:**
  - *The base's own undercut.* Step 0.
  - *Horns on the fold.* There are none at the along-ring points; Caiman measured 0.06 mm from a horn on the kink. The rows use `fold_clear_mm` 1.0.
  - *A leaning cheek tucks a square-struck stamp by 0.35 mm.* `along_pull`.
  - *An apex leaning past its root overhangs.* Keep `at` inside the outline, and check with `parting_monotone` and the release pull.
  - *Voronoi joints of every orientation on a zero-draft table (G5).* Only the tier descents cross columns, and the clamp guarantees the rest.
  - *DFM.* Plates ≥ 1.2 mm; joints 0.35 ≥ 0.30. Horns are measured by `stamp_finest_mm`, and their tips cost nothing.
- **Needs:**
  - the base decision;
  - C-R1, C-R4, C-R2;
  - C-R7 (`plate_voronoi`, `fringe`, `rosette_tubercle`);
  - P4 (landed);
  - P7 (stamp nodes, `base.preset`).
- **Template:**
  - `base.preset("016", sand)` (P7) or the Fallback A outline nodes, a clamped group, tilings, one `stamp.row` and six `stamp` nodes.
  - Expose "Horn length" (one scale on all six, through `StampTop::scaled`), "Plate tiers" and "Fringe grade".
  - About 40 KB after P7.
- **Risk:** high difficulty, medium risk, plus the base. Fallbacks:
  - Fallback A or B for the base.
  - Horns reduced to the two occipitals, if the diagonals crowd the plates.

---

## Chamaeleo — *the casque*

- **Status:** not started.
  - Blocked on **C-R1**, **C-R4** and **C-R7**. P3 and P4 have landed.
  - Two base facts first:
    - The brief's 17 × 13 face is **refused** by the stock guard. 13 / 20 = 0.65, below the 70% floor (`imported_base.rs:532`). Use **17 × 14.5**.
    - The bare 001 master reads **Marginal, 0.10% at −1.6°**, at native size. Measure it at 17 × 14.5 before any relief. The finished ring must reach Castable.
- **Concept:** The creature that changes colour carries the stone that does: an alexandrite set on its casque. Its prehensile tail is coiled on each cheek, and a crest of cones runs down its spine. There are no turret eyes.
- **Theme face to palm:**
  - **Table** (a long head, 17 along the ring by 14.5 across):
    - the alexandrite, flush on the parting line at the table's front third (along −4 mm);
    - behind it, the **casque**: a broad, smooth, helmet-shaped ridge 2.8 mm wide on the parting line. It peaks at the occiput (along +5 mm) and falls to the table's end;
    - temporal crests stepping down either side of it (tiers).
  - **Cheeks:** the coiled tail, one thick spiral stamp per cheek, ringed by heterogeneous tubercles.
  - **Shoulders:**
    - crest: the dorsal crest, small cones on the parting line, graded;
    - walls: heterogeneous granules in three sizes, with the **lateral stripe**, a warped band of larger flat tubercles, blended in with `SmoothMax`.
  - **Palm:** fine granules.
  - **Bore:** plain.
- **Base:**
  - `PRESETS` "001" through `sand_master`, `sand_envelope = true`.
  - `shank.head.length_mm` 17, `profile.width_mm` 14.5, bore 18.6.
  - Flat chart, edge 0.3, comfort 0.1, chart set before any layer.
- **Process:** Delft.
- **Stones:**
  - Alexandrite, `Gem { l_mm: 5.0, preview_tint: Some([0.18, 0.50, 0.38]), ..Gem::calibrated(GemCut::Cushion, 4.0) }`. A cushion's `aspect()` is 1.0 (`gem.rs:72`), so the 5 mm length is set by hand.
  - Seated on a `SeatPadLayer` at `hide.crest_at(&a, -4.0)` with `style: SeatStyle::Boss`, `crown` 0.15, `blend_mm` 0.4, `metal_true: true`, `solid: SolidKind::Flush`, `through: true`.
  - Call `fit_stone`, then set `height_mm` to the casque's base height.
  - It is a **top-level `Max` entry after the clamped group**, so the clamp never shaves its stock. This is Caiman's emerald idiom (`examples/stock_masterworks.rs:1723-1741`).
- **Build, step by step:**
  1. **Group "Casque and crests":** clamp (C-R1), hide space (C-R4). It holds two layers:
     - **"Casque":** an across-band gable (monotone from the line, G2) multiplied by an along profile rising from the stone to the occiput and falling at the table's end. Apart from its gable it varies only along the ring (G4). Height 0.9, width 2.8.
     - **"Temporal crests":** two tiers each side, stepping down (G5).
  2. **"Granules, three sizes":** `reptile::svg::granule_voronoi` (periodic, seeded, three radii, lands ≥ 0.4) on the cheeks and shank walls, with masks `"##region:cheek"` and `"##region:wall"`, height 0.35.
  3. **"Lateral stripe":** `reptile::svg::flat_tubercle` with a terraced cushion remap.
     - `warp` along a wavy guide on the shank walls.
     - Mask `"##region:wall"`, which is the factory equivalent of G8's side-face gate.
     - `SmoothMax` with soft 0.2, at the granules' height, so the stripe reads by its shape rather than by stacking.
  4. **"Alexandrite":** the seat.
  5. **Stamps "Tail coil, fingertip" and "…, knuckle":**
     - `outline::spiral(2.5, 0.6, 3.6, 0.45, 0.8)`. The brief's `w1` was 0.9, which leaves about 0.3 mm between the outer turns; 0.8 leaves about 0.4 mm. Run `outline::check` and `dfm::stamp_finest_mm` on it.
     - `along_pull`, `height_mm` 0.45, `draft_deg` 4, one at the centre of each cheek (the highest `a.cheek(s)`).
     - The spiral exceeds `PLAIN_MAX_STAMP_POINTS` (512), which writes the design at format 6. That is expected.
     - Its concave edges are fine on a cheek, whose walls stand along the pull (G9). The crescent rule applies only on a crown.
  6. **Stamps "Dorsal cone":**
     - `stamp_row` with `RowPath::PartingLine`, from the head's end (with `fold_clear_mm` 1.0) to 262°, `count` 10, `mirror_shoulders`, `taper` 0.54 (1.3 → 0.6 mm).
     - `outline::circle(1.3)` with `StampTop::Cone { apex_mm: 0.8, at: [0.0, 0.0], tip_mm: 0.3 }`.
     - The row stands on a knife keel. On a factory shank that has to be a hide-space gable layer (C-R4), because a chart-space `CurveLayer` cannot follow the stock's varying crest `v`.
- **What it shows off:** a crest-line casque on a factory table; heterogeneous granules; a warped stripe; spiral and cone stamps; a stone as a boss beside a clamped group.
- **Traps and how they are avoided:**
  - *The resize guard.* 14.5 mm, not 13.
  - *The base reads Marginal.* Measure it at size first.
  - *The table rule.* The casque is a crest feature (G2 + G4), and the temporal crests are tiers (G5).
  - *The spiral's stroke floor.* The inner turn is 0.45 mm; the gaps are ≥ 0.4 mm.
  - *Cheek curvature under a stamp (G9).* 001's cheeks are near-planar walls, and `along_pull` keeps the stamp's walls square to the pull.
  - *Cones near the fold.* `fold_clear_mm` 1.0.
  - *Caiman's granule-gap finding (G11).* Explicit lands.
  - *The boss inside a clamped group.* It stays a separate top-level entry.
- **Needs:**
  - C-R1, C-R4;
  - C-R7 (`granule_voronoi`, `flat_tubercle`);
  - P4 (landed);
  - P7 (template nodes).
- **Template:**
  - `base.preset("001", sand)` (P7), a clamped group, tilings, stamp nodes and the seat.
  - Expose "Casque height", "Coil turns" and "Cone grade".
  - About 45 KB after P7.
- **Risk:** high difficulty, medium risk. Fallbacks:
  - If the lateral stripe's warp leaves its mask, drop the warp and keep a straight stripe.
  - If the spiral self-crosses at the finished scale, use 2 turns.

---

## Packaging (the Reptilia format)

The lead packages the collection, following `showcase/reptilia/README.md`.

**Author:** `cargo run --release -p ringdesign-core --example cataphracta -- OUT_DIR [--draft] [--verify] [SLUG]`. It refuses to overwrite, and it writes each ring's gates into `report.json`.

**Per-ring folder, `showcase/cataphracta/<slug>/`:**
- `design.ring.json`, `editable-graph.ring.json`, and `artwork/*.svg` (text, not PNG);
- `finished-metal.stl` and `casting-pattern.stl`: the latter is shrink-compensated, with no cuts and with drill dots, from `mf::inspect(..).prepared.mesh`;
- for the three stone rings, `reference-<stone>.stl`;
- `report.json`, `mesh.json`, `release-fine.json`;
- `verification.json`, adding `clamp_bite_mm`, `drag_fraction`, `template_bytes` and the open milliseconds per phase;
- `hero.png`, `face.png` and `palm.png` from `render::write_png_parts` with `render::GOLD`;
- Blender `studio*.png` in gold;
- the build reel, recorded on the rdsmoke AVD.

Slugs: `sphenodon`, `heloderma`, `moloch`, `gekko`, `ouroborus`, `chelonia`, `phrynosoma`, `chamaeleo`.

**Collection level:**
- `README.md`, with the reproduce commands under the memory guard;
- `index.html`;
- `Cataphracta-collection.png`, carrying the title and subtitle above;
- a renders zip.

The shared tooling is P8 (`tools/render_collection.py` with a gold material, and `tools/catalog_collection.py --title --subtitle`). Today `tools/render_reptilia.py` has no gold material and `tools/catalog_reptilia.py` takes only a source directory. Either land P8 first or adapt those two scripts by hand.

**Graph templates:**
- Write `graphs/templates/<slug>-cataphracta.graph.json`. The directory is the bundle family (`ringdesign-assets/build.rs:29`).
- Add `pub static CATAPHRACTA: &[TemplateGraph]` in `crates/ringdesign-graph/src/templates.rs`, beside `REPTILIA` (:163), and chain it into `catalog()` (:170).
- Add a group "Cataphracta collection" in `crates/ringdesign-workbench/src/templates.rs`, placed after the starters. The Reptilia group at :479 is the pattern.
- Make 160 px thumbnails through `tools/template_thumbnails.py`, adding entries to `crates/ringdesign-workbench/assets/templates/sources.json` that point at `render::finished` studio-gold renders. Update the preview test count.

**Template packager:** P8's `ringdesign-graph/examples/collection_templates.rs -- cataphracta`, modelled on `reptilia_templates.rs`.
- Run `templates::refine_sources` before lifting. It drops embedded PNGs that shadow an SVG source, so the graph carries SVG text rather than rasters.
- Authored builders that use `script` nodes or expression pins must be golden-checked with `Evaluator::with_exprs(ringdesign_script::engine())`. That means placing the packager where `ringdesign-script` is visible (the workbench crate), or checking with `ringdesign graph eval` from the CLI.

**Reproduce commands for the README** (from the repository root, in a fresh directory):

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example cataphracta -- NEW_DIR --draft
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example cataphracta -- NEW_DIR --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example collection_templates -- cataphracta NEW_DIR
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- NEW_DIR --gold
python3 tools/catalog_collection.py NEW_DIR --title "Cataphracta" --subtitle "EIGHT HIDES · FIVE BANDS · THREE SIGNETS · THREE STONES · ALL POURED IN SAND"
```

**Phone:** bump `version` in `crates/ringdesigner-android/Cargo.toml` and add a `CHANGELOG.md` entry. Then verify with `cargo test -p ringdesigner_android`, run `cargo ndk -t arm64-v8a check -p ringdesigner_android`, and smoke-test on the rdsmoke AVD.

**Merge rule, per lane:** `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --workspace --offline` and the wasm check (`cargo check --no-default-features --target wasm32-unknown-unknown` for core and the configurator) must both pass.
