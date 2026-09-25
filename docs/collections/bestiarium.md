# Bestiarium — the dark bestiary

**Sheet subtitle (nine authored):** NINE BEASTS · THREE IN SAND · SIX IN WAX · THIRTY-FIVE STONES. After the first render round the weakest ring is dropped and the subtitle is recomputed from the eight that remain (Logan, 2026-09-24).

**What the collection shows:** the app's sculpting side. Hides and plumage are painted in metal millimetres on the bare-surface atlas (`core/skin.rs`), on factory stock and on keyframed and bypass bodies. Emblems are struck as true outlines (stamps v2: `core/outline.rs`, tiers, tops and rows). Every stone is in a made setting, and the creatures' weapons are CAD parts: fang, talon and tentacle claws, a twisted-sweep stinger, lofted feathers. Each ring is judged against its own `DraftSettings::process`.

**Sources.** This file is built from `docs/collections/source/plan.md` (the director's plan) and `docs/collections/source/brief-bestiary.md`. Where they differ, the plan's §1 verdicts, its base-allocation table and its §7 "Logan's answers" win. API names were checked against master at `30f0506` on 2026-09-24. Anything not checked is marked **unverified**.

**Paths.** `core/` = `crates/ringdesign-core/src/`, `ex/` = `crates/ringdesign-core/examples/`, `graph/` = `crates/ringdesign-graph/src/`, `wb/` = `crates/ringdesign-workbench/src/`. "Doctrine" is the project `CLAUDE.md`.

**Logan's binding decisions:**
- Four collections with eight rings each. The Bestiarium authors nine and drops the weakest after round 1.
- Mixed processes.
- The nine upright factory plans (004, 008, 009, 010, 011, 014, 018, 019, 020) are **lost wax only**. The sand master, which mirrors the upper half, is used only on 001, 002, 003, 005, 006, 007, 012, 013, 015, 016 and 017.
- Delft and Petrobond are both poured. Every Bestiarium sand ring is Delft.
- Harpyia returns with **lofted** feathers.
- Final renders are studio gold.
- Taste:
  - one theme from face to palm (Palisade was retired as "random elements placed in various spots");
  - figurative motifs are stamps with true outlines (painted moons "look like arrows");
  - made settings only (height-field prongs were "quite awful");
  - no faces and no snake eyes;
  - factory stock for signets;
  - reptiles land well, and the celestial Zenith did not;
  - "the most complex, well thought out, most insane rings we've done".

---

## Platform status (dependencies)

| ID | Capability | Status |
|---|---|---|
| P1 | Async template open with staged progress | **Landed** (batch 14, hardened in batch 15). `wb::templates::Template::open`, with the script engine attached. |
| P2 | CAD stones are stones | **Batch 15, merging now.** On master: `setstone::StoneSource::Cad { feature, copy }` (`core/setstone.rs:23`), `render::finished` (`core/render.rs:97`), and multi-source `Pattern` (`cad::pattern::Sources`, `core/cad/pattern.rs:475`). |
| P3 | Skin in core, plus `sand_master` | **Batch 15, merging now.** On master: `core/skin.rs` (`Atlas::of`, `Hide`, `Joints::{new, eccentric}`, `draft_clamp`, `hide_layer`) and `imported_base::sand_master` (`core/imported_base/sand_master.rs:54`). The atlas lands on the swept mesh to within 0.0006 mm on a keyframed band and 0.049 mm on a bypass (`skin.rs:531`). |
| P4 | Stamps v2 | **Batch 15, merging now.** On master: `Stamp::{tier, top}`, `StampTop::{Flat, Gable, Ridge, Cone, Dome, Taper}` (`core/setting.rs:927-953`), `core/outline.rs` (`keel`, `comb_lobe`, `lanceolate`, `quill`, `jaw`, `leaf`, `fork`, `spiral`, …), `Stamp::parting_monotone` (`setting.rs:2149`), `stamp_row` / `StampRow` / `RowPath` (`setting.rs:1547-1581`), `hull_and_bays` (`setting.rs:1490`). |
| P5 | Station-aware side-face gates | **Batch 16, not started.** `VGate::mask(v, ctx)` still has no θ (`core/field.rs:443`). |
| P6 | Claw styles | **Batch 16, not started.** `head.claw` takes only `prongs` and `wire_mm` (`core/cad/builders.rs:156`). |
| P7 | Template nodes and lift (`base.preset`, stamp nodes, `shank.key`) | **Batch 16, not started.** `keys` is still hidden on the shank node (`graph/nodes/shank.rs:52`). |
| P8 | Collection tooling (`render_collection.py`, `catalog_collection.py`, `collection_templates.rs`) | **Batch 16, not started.** |
| — | Starter gallery | Batch 16, not started. |
| C-B1..B3 | Bestiarium enablers (below) | Land inside this collection's batch. |

---

## The rings

| Ring | Epithet | Base | Process | Stones | Status |
|---|---|---|---|---|---|
| Draco | the wyvern displayed | factory 002 Kite, 13 × 19, sand master | Delft sand | none | in progress, `bestiarium-draco` |
| Arachne | the weaver | keyframed LowDome 5.6 × 2.4 | lost wax | onyx oval cab 9.75 × 7.8; garnet oval cab 5 × 3.5 | in progress, `bestiarium-arachne` |
| Basiliscus | king of serpents | factory 020 Escutcheon, 16 × 16.5, **unmirrored** | lost wax | tsavorite marquise 8 × 4 | buildable now |
| Fenrir | the wolf and the moon | factory 010 Trillion, 16 × 17, unmirrored | lost wax | moonstone round cab 10 | blocked on P6 (fallback: wire claws) |
| Phoenix | reborn | keyframed Flat 6.6 × 3.4 | Delft sand | fire-opal oval cab 8 × 6; 14 orange sapphire rounds 2.2 → 1.1 | buildable now with the re-keyed body; P5 preferred |
| Corvus | Huginn and Muninn | Bypass LowDome 6.0 × 2.8 | Delft sand | onyx oval cab 8 × 6 | buildable now |
| Kraken | the deep's grip | keyframed DShape 6.0 × 2.7 | lost wax | labradorite round cab 10 | blocked on P6 and C-B1 |
| Manticora | the tail that throws | keyframed HighDome 5.4 × 2.2 | lost wax | ruby oval 7 × 5 (pear after C-B2); about 12 black spinel princesses 2.0 | buildable now with the oval |
| Harpyia | the snatcher | Uniform Flat 7.2 × 3.8 | lost wax | sapphire round 8 | wings buildable now; talons blocked on P6 |

Stones: 0 + 2 + 1 + 1 + 15 + 1 + 1 + 13 + 1 = **35** across nine rings.

**Sheet layout.**
- The sand row is Draco, Phoenix and Corvus.
- The wax row is Arachne, Basiliscus, Fenrir, Kraken, Manticora and Harpyia, less whichever ring is dropped.
- Captions follow the Reptilia sheet: name, then epithet, then base and stone.

---

## Build order inside the collection

At most two lanes run at once. Each ring's lane owns only its example file and `showcase/bestiarium/<slug>/`. The registration files belong to the lead: `graph/templates.rs`, `wb/templates.rs`, the collection README and the sheet.

| Step | Lanes | Why this order |
|---|---|---|
| 0 | Draco, Arachne | Already in flight. |
| 1 | Phoenix, Basiliscus | Phoenix is the first painted keyframed body. It settles the side-face question (re-key or wait for P5) that Kraken and Manticora meet as well, and it writes `plumage::contour`, which Corvus reuses. Basiliscus is Draco's stock-painting method in wax and is the lowest-risk ring left. |
| 2 | Corvus, Manticora | Corvus reuses Phoenix's contour painter on a bypass atlas. Manticora needs only landed pieces (P3's `Joints::eccentric`, P4's rows); it ships with an oval ruby now and swaps to the pear when C-B2 lands. |
| 3 | Fenrir, Kraken | Both wait for P6 (batch 16, lane 1A). Kraken's lane also lands C-B1. |
| 4 | Harpyia | She needs P6's talons and is the highest risk. **Start her lane with a one-wing loft spike** (`ex/feather_loft_probe.rs`, wings only on the bare band) as soon as any lane frees: the spike decides whether she survives round 1. |
| 5 | Review and drop | Round-1 render sets for all nine, then one reviewer ranks them against the plan §5 checklist and the weakest is dropped. On a tie, drop a winged ring (Draco, Phoenix, Corvus, Harpyia), because the plan flagged wing overlap. Rounds 2–3 follow, then packaging (plan batch 3A). C-B2 (batch 3B) comes after packaging, and Manticora is re-rendered with the pear. |

---

## Collection enablers

### C-B1: CurveLayer per-point profile and bead rows (S, Kraken's lane)

- **Today:** `CurveLayer` has a single `taper` (a fraction of each end, 0..0.5) and uniform `width_mm` / `height_mm` (`core/curve.rs:62-76`). It has no per-point profile and no beads.
- **API sketch:**

```rust
pub struct CurveLayer {
    // existing: points, repeats_around, closed, width_mm, height_mm, profile, taper, mirror_v
    /// Per control point, × width_mm; empty is uniform. A short list repeats its last value.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widths: Vec<f64>,
    /// Per control point, × height_mm; empty is uniform.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub heights: Vec<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beads: Option<CurveBeads>,
}
pub struct CurveBeads {
    pub pitch_mm: f64,
    pub diameter_mm: f64,
    pub height_mm: f64,
    /// -1..1 across the wire: where the row runs on its width.
    pub offset: f64,
    /// Beads scale with the local width.
    pub graded: bool,
    /// Where along one pitch the first bead sits, 0..1.
    pub phase: f64,
}
pub const MAX_CURVE_BEADS: usize = 4096; // clamp the loop and break on the accumulated count (the MAX_CELLS pattern)
```

- **Rules:**
  - Empty fields leave every existing design bit-identical.
  - `feature_footprints` reports the smallest bead and the thinnest point of the wire, for both refinement seeding and DFM.
  - Pins:
    - an old design is bit-identical;
    - a graded wire's footprint equals its thinnest point;
    - bead counts cap.
- **Also in this lane:** `graph/nodes/layer.rs:272` (`layer.curve`). The struct-node coverage test (`graph/nodes/structs.rs:117`) fails when a field has no pin. A field hidden from the node rides as a `design.set` patch instead, which spends the template's patch budget. The plan gave the lane only `core/curve.rs`, so claim this file too.

### C-B2: true stone plans (M, batch 3B after packaging)

- **Today:** `setting::Plan` is a superellipse (`core/setting.rs:105-113`). A pear is seated as an ellipse (`plan_pow` 2.0), and a trillion, heart or half-moon as a 3.2 superellipse (`core/gem.rs:154-165`), while the preview draws the true facet meshes. The metal cut and the drawn stone differ, which Doctrine § "A seat is the stone's plan" forbids.
- **API sketch:**

```rust
#[derive(Clone, Copy, Debug)]
pub struct Plan { pub a: f64, pub b: f64, pub pow: f64, pub table: Option<&'static [f32; 256]> }
// `table`: polar radius per 1/256 turn, normalised to the half-extents, read from the bundled gem mesh's
// girdle for Pear, Trillion, Heart and HalfMoon. `point`, `normal`, `perimeter` and `claw_angles` read it when present.
```

- **Consumers:** `envelope`, `bur`, `collet`, `claw_angles`, `plan_half_extents_mm` (`core/field.rs:1302`), and the stones report.
- **Pin:** the preview stone and the metal cut agree to within 0.02 mm on every cut.
- **Bestiarium user:** Manticora's pear.

### C-B3: Bestiarium painters and SVG art (per ring, S each)

Code lives in the ring's own example. A painter is promoted to `core/plumage.rs` or `core/beast.rs`, following the `reptile.rs` pattern, only when a second ring uses it: `contour` (Phoenix and Corvus) will be. Painters return 0..1 heights in hide millimetres.

```rust
// plumage.rs
pub fn contour(along: f64, across: f64, pitch: f64) -> f64;             // chevron contour feather, point leading, drafted fall
pub fn remex(along: f64, across: f64, length: f64) -> f64;              // folded flight feather along the ring, shingled
pub fn tail_fan(along: f64, across: f64, feathers: u32) -> f64;         // rectrices, centre pair highest, stepping down outward
pub fn hackle(along: f64, across: f64, pitch: f64, aspect: f64) -> f64; // lanceolate hackle, rachis raised
pub fn hackle_scale(along: f64, across: f64, m: f64) -> f64;            // m 0 hackle .. 1 keeled snake scale, one skin
// beast.rs
pub fn fur_lock(along: f64, across: f64, pitch: f64) -> f64;            // S-curved lock with a centre groove
pub fn tergite(t: f64, across: f64) -> f64;                             // loaf plate: posterior lip, two carinae
// Harpyia only (CAD, not a painter)
pub struct FeatherSpec { pub root: [f64; 3], pub heading_deg: f64, pub bend_mm: f64, pub length_mm: f64,
                         pub width_mm: f64, pub thick_mm: f64, pub camber: f64, pub lift_mm: f64 }
pub fn feather_loft(spec: &FeatherSpec) -> cad::Operation; // Loft through five cambered Bézier lens sections
```

**SVG art to draw.** All of it uses the gradient-shaded family that holds the detail floor (the Serpentarium lesson), with strokes and gaps at least 0.3 mm in sand and 0.2 mm in wax:
- "Phoenix wing" in three tiers × 2;
- "Phoenix tail";
- Fenrir's "Gleipnir" braid;
- Harpyia's "Back feathers" and "Tail fan".

---

## Method every ring shares

- **One file per ring.**
  - The two in-progress rings are single files, `ex/bestiarium_draco.rs` and `ex/bestiarium_arachne.rs`. Follow that shape: `ex/bestiarium_<slug>.rs`, with CLI `-- [OUT_DIR] [--draft] [--verify]` and output defaulting to `showcase/bestiarium/<slug>/`.
  - Copy the `write()` gate block from Draco's branch: `git show bestiarium-draco:crates/ringdesign-core/examples/bestiarium_draco.rs`, lines 836–935.
- **Stock base.**
  - Sand: Draco's `base()` (same file, lines 419–460). Load the preset, `ImportedBase::attach(&mut d, sand_master(source)?)`, then `sand_envelope = true`.
  - Wax: the non-sand path of `ex/stock_masterworks.rs:317-411`. Attach the preset as loaded, with `sand_envelope = false`.
  - Either way, set `d.imported_base.chart = Some(SurfaceChart { profile, bore_radius_mm })` from this stock **before** painting.
- **Procedural base.** Set `d.shank.kind = ShankKind::Keyframes` with `ShankKey { theta_deg, width_scale, thickness_scale, crown_scale }` (`core/profile.rs:1747`). The first 16 keys are read (`profile.rs:2504`). Arachne's `key()` closure (its file, line 97) is the idiom.
- **Process.**
  - Sand: `mf::Recipe::sand(SandProcess::DelftClay)` gives 3.0°, 0.8 mm section and 0.30 mm detail (`core/castability.rs:178-187`).
  - Wax: `CastProcess::LostWax.apply(&mut d.draft)`, which writes a 0.5 mm section and 0.15 mm detail (`castability.rs:117`). Then set `d.draft.min_section_mm = 0.8` and `min_draft_deg = 0`, which is the plan's investment gate and what the stock-masterworks wax rings use.
  - Copy the recipe's floors into `d.draft`.
- **Keep the saved design lean.** Keep the `mf::Setup` in the example and pass it to `mf::inspect`, but do not store `d.manufacturing` or export build steps in the design. Arachne did this to cut its lift to four patches of its own (its commit `707f22c`).
- **Painting.**
  - `let a = skin::Atlas::of(&d, 2048, 768)?; let hide = skin::Hide::of(&a);`. Paint with `a.paint(name, |s| …)`.
  - In sand, pass each layer through `skin::draft_clamp(&a, &mut alpha, height)` and log its `ClampReport`; the gate is a bite of at most 0.05 mm.
  - Insert the alpha into the library as a portable 16-bit PNG: `lib.insert(Alpha::from_png16(name, &a.to_png16()?)?)`, the `portable()` idiom at `ex/stock_masterworks.rs:32`.
  - Add the layer with `skin::hide_layer(&d, name, height, window)`, which is one tile over the whole chart, `Blend::Max`.
  - Windows are `Window::around(centre, span)` with `fade_deg = 6`.
- **Stamps.** `setting::Stamp` has no `Default`. Every literal sets `tier: 0, top: StampTop::Flat` unless it wants otherwise. Draco's branch predates P4 and needs exactly this on rebase.
- **Stones.**
  - Seat pads: `SeatPadLayer { …, solid: SolidKind::{Flush, Bead, Bezel}, through, metal_true, .. }`, then `fit_stone(gem)` (`core/field.rs:1349`).
  - CAD stones: `cad::builders::stone_feature(id, gem, Placement::ring(θ, builders::stand_off_mm(key, gem)))` (`builders.rs:954`, `:504`), and heads through `builders::feature_on(id, name, key, stone_id, params)`.
  - Under sand the pattern leaves heads and seats out and raises `mark_mm` drill dots. Under wax both are cast in place.
- **Gates** (plan §5), at a draft build of 768 × 320 and an export build of 1536 × 448:
  - watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and on every made part; `built.solids.notes` empty; every stamp resolved;
  - `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)` judged against the ring's own process:
    - sand: **Castable** (not "with care"), and `mf::inspect` release at 0.100 mm and 0.075 mm with 0 obstructions and 0 unresolved rays;
    - wax: fill at 0.8 mm, and the author asserts the CAD land widths;
  - `dfm::findings_in(&d, &lib)` returns **0 findings** (Caiman carried 2);
  - the stone count in `stones::report` equals the preview count, and the crowding census is clean or explained;
  - `--verify`: a cold reload with an empty library gives identical vertices and faces.
- **Review** (plan §5), capped at three rounds; a ring still failing is cut, never shipped weak:
  - one theme;
  - figurative motifs as stamps;
  - no faces or eyes;
  - stones in visible made settings;
  - shoulder ornament not cut off at the face;
  - custom heads ≥ 13 mm;
  - no flat, blocky CAD;
  - factory walls keep their hard angles;
  - a distinct silhouette;
  - density and legibility at least equal to Caiman's hero, the Reptilia sheet and Logan's ZBrush sheet (`b15/zbrush_top.png`).
- **Stock folds** (Draco's lesson). Painted relief over the stock's own facet folds can make the mesh cross itself; Draco found folds on 002 at 181° and 357°. Search a draft build for self-crossings and cap the relief there, as Draco's `crossing_spots` and `fold_caps` do.

---

## Draco — *the wyvern displayed*

**Status:** in progress on `bestiarium-draco` (`af5304f`): factory 002 Kite at 13 × 19 as a Delft sand master, no stone. All nine gates pass at 1536 × 448 (clamp ≤ 0.033 mm, DFM 0, release clean). Left to do: rebase onto master (its `Stamp` literals need `tier` and `top`), review rounds, and the graph template.

## Arachne — *the weaver*

**Status:** in progress on `bestiarium-arachne` (`707f22c`): keyframed LowDome in lost wax, a spider at the hub of an orb web. Onyx and garnet oval cabochons sit in trimmed collets stored as parts, so the file is format 6. Eight jointed Twist legs are mirrored across the band. The gates pass. Its lift carries 7 `design.set` patches (4 of its own plus 3 from the CAD chain) against the budget of 4, so it waits on P7.

---

## Basiliscus — *king of serpents*

- **Status:** not started; buildable now (P3 and P4 are on master). A light template waits on P7.

- **Concept:** The cockatrice, the serpent-king hatched from a cock's egg. Rooster hackles pour off an heraldic escutcheon and down both shoulders, turning into a serpent's keeled scales and then its belly. Its comb, the crown that names it, runs down the shield's pale through a flush marquise.
  - **What changed from the brief:** Logan moved it to **lost wax on the unmirrored 020**, and the mirrored-020 spike is dropped. The plan also has the hackles own the table and both shoulders, so the ring reads as plumage before scales, since Cataphracta owns scale hides.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Face (table) | The comb runs **palewise**, across the band on the shield's own axis at θ 90. From the chief: two lobes, the marquise, two lobes. Hackles fan from the pale toward both shoulders. |
| Shoulders, 0–70° off the head | Hackles continue, lengthening. |
| Shoulders, 70–110° off the head | The same skin morphs from hackle to keeled serpent scale. |
| Head walls, cheeks, shank walls | Round scales blending into `reptile::shields` toward the palm. |
| Palm, 270 ± 55 | Ventral scutes. |
| Bore | Plain, comfort 0.1. |

- **Base:**
  - `PRESETS` "020" Escutcheon, native face 18 × 19 (`core/imported_base.rs:1116`), attached as loaded with **no** `sand_master` and `sand_envelope = false`.
  - `profile.apply_style(ProfileStyle::Flat)`, `profile.width_mm = 16.5` (across the finger), `shank.head.length_mm = 16.0` (along the ring).
  - `size = resize::size_from_bore(18.6)`, `edge_round_mm 0.3`, `comfort_fit_mm 0.1`.
  - The plan is **upright**: 4.1 mm asymmetric across z = 0 (plan F2). The chief, the cusped edge, lies toward one band edge and the point toward the other. Its mirror axis is the section at θ 90, so the heraldic pale runs across the band at θ 90, not along the parting line. That is why the comb is palewise here rather than along the ring as in the brief.

- **Process:** lost wax, Silver 925.
  - Upright plans are wax only, and the sand master would mirror the chief onto the point.
  - Recipe as the stock-masterworks wax path: `CastProcess::LostWax`, `sand: None`, `min_draft 0`, `min_detail 0.15`, `min_section 0.8`.
  - No draft clamp and no release gates. The field still reports pull statistics.

- **Stones:** one tsavorite marquise 8 × 4, `Gem { l_mm: 8.0, ..Gem::calibrated(GemCut::Marquise, 4.0) }`. Its `plan_pow` of 1.5 is native to the seat system (`core/gem.rs:163`), so stock, bur and stone agree without C-B2. It is flush-set with `SolidKind::Flush` and a through pilot, the made bur cast in place under wax.

- **Build, step by step:**
  1. Base as above, then take the `SurfaceChart`. Build `a = Atlas::of(&d, 2048, 768)` and `hide = Hide::of(&a)`.
  2. **Pale helper** in the example: `fn pale_at(a: &Atlas, z: f64) -> (f64, f64)` takes the atlas column at θ 90 (`x = width / 4`) and returns the `(theta, v)` of the row whose `p[2]` is nearest `z`. Stamps and the seat on the pale use it. Do **not** use `hide.crest_at`, which walks the parting line and would put the comb up to 2 mm off the shield's axis.
  3. **Fess point:** `z_fess` is the mean z of the table samples at θ 90 whose `p[1] ≥ a.top − 0.3`. Measure it; do not assume 0. Also record the table's chief and point ends, `z_chief` and `z_point`, where the rim roll begins.
  4. **Layer "Hackles into scales"** (0.75 mm, `hide_layer`, `window(90, 240)`). One painter, `hackle_scale(along, across, m)`, with `m = smootherstep(L70, L110, |along|)`, where `L70` and `L110` are `hide.along` at 70° and 110° off the head.
     - Each hackle is lanceolate, length : width 3.2 at `m = 0` falling to 1.2 at `m = 1`. Its shaft runs along ±`along` with the base on the pale and the tip toward the shoulder.
     - Rows are shingled so each feather overlaps the next one further out. A 0.1 mm overlap lip is legal in wax.
     - The raised rachis (0.08 over the vane) becomes a keel.
     - Pitch runs 1.4 mm at the pale, 2.2 mm at the table's ends, then 0.84 mm at `m = 1`, which is `reptile::snake`'s form (`core/reptile.rs:9`).
     - A flat "comb seat" strip, 1.6 mm wide, runs along the pale at 0.35 of the layer height, and a disc of the marquise's plan plus 0.6 mm is kept flat at the same level.
     - Every cast tip is at least 0.25 mm across.
  5. **Layer "Serpent flanks"** (0.40 mm, `window(180, 360)`, painted only where the sample faces the pull, `|n.z| > 0.6`, or lies past `hide.rim`). Round scales (copy `round_scales` from `ex/stock_masterworks.rs:1216`; it is private) blend into `reptile::shields(along, across)` (`reptile.rs:55`) toward the palm.
  6. **Layer "Ventral scutes"** (0.30 mm, `window(270, 110)`): `reptile::ventral(along / 2.0, across / 2.0)` (`reptile.rs:32`), closing on a joint at 270.
  7. **Layer "Graver's barbs and keels"**: `bench_only = true`, `Blend::Subtract`, 0.10 mm. Barbs off each rachis and keel lines on the scales.
  8. **Seat "Tsavorite, flush"**:
     - `SeatPadLayer { theta_deg, v_mm (from pale_at(&a, z_fess)), style: SeatStyle::Boss, crown: 0.15, blend_mm: 0.45, metal_true: true, solid: SolidKind::Flush, through: true, rot_deg: 90.0, .. }`;
     - then `fit_stone(gem)`, `height_mm = 0.6`, `Blend::Max`;
     - `rot_deg 90` lays the long axis palewise, and the 0.6 mm boss is the comb's central and tallest lobe.
  9. **Stamps "Comb lobe, 1..4"**:
     - outline `outline::comb_lobe(1.3, 1.1, 0.3)` (`core/outline.rs:226`);
     - `rot_deg 90`, `top: StampTop::Dome { crown_mm: 0.12 }`, heights 0.50 for the inner pair and 0.38 for the outer pair, sink 0.3, draft 2°;
     - centres at `z_fess ± (4.0 + 0.6 + 0.65)` and `± (4.0 + 0.6 + 0.65 + 1.45)`, placed with `pale_at`;
     - drop an outer lobe whose far end comes within 0.6 mm of `z_chief` or `z_point`, checking each side separately, because the fess point is not at the pale's middle.
  10. Run the gates. Print `thinnest_wall_mm` and its θ.

- **What it shows off:**
  - atlas painting in wax on an upright factory plan used as drawn;
  - two creatures handed over inside one painter;
  - domed comb lobes from the outline library;
  - a flush made setting with a through pilot;
  - heraldic stock.

- **Traps and how they are avoided:**
  - **The pale is not the parting line on an upright plan.** Place with the θ 90 column (step 2).
  - **Hand-overs must happen inside one skin.** Two layers side by side leave a visible seam where they hand over (`ex/stock_masterworks.rs` Caiman lesson). The morph is one painter.
  - **Hackles must read as plumage, not as a reptile.** If the round-1 review reads "another reptile", push the morph back to 80–120° off the head.
  - **DFM must be 0 findings** (F11). Barbs are bench-only, cast tips are ≥ 0.25 mm, and `dfm::findings_in` is run on every build.
  - **Stock folds.** Search a draft build for self-crossings (Draco's method).
  - **No faces.** Comb plus stone plus wattles would read as a rooster's head with the stone as its eye, so there are no wattles and nothing below the stone but hackles.
  - **Fill.** `thinnest_wall_mm` ≥ 0.8 at the shoulders, where the head's swell ends.

- **Needs:** P3 and P4 (landed); C-B3 `hackle_scale` (ring-local); P7 for a light template.

- **Template:**
  - Lift today with `Graph::from_design`:
    - an `/imported_base` patch of the unmodified 020, about 0.45 MB;
    - `alpha.png` × 4 at 2048 × 768 16-bit;
    - `layer.seat`;
    - a `/stamps` patch with 4 lobes;
    - `/draft/*` patches for process, section and detail.
  - Expect about 3 MB with 5–6 patches, over the ≤ 4 budget until P7's `base.preset` and stamp nodes replace the stock and stamp patches.
  - Expose US size only. Face length and width stretch the painted atlas until the deferred `alpha.hide` node exists.

- **Risk:** 3/5, medium-low.
  - If the palewise comb crowds the stone, keep one lobe each side and raise the boss to 0.7.
  - If the morph reads muddy, hand over hard along one scale joint running across the band.

---

## Fenrir — *the wolf and the moon*

- **Status:** not started; blocked on **P6** (Fang style, Jaws grouping). Everything else is buildable now, and the fallback is plain wire claws.

- **Concept:** Fenrir's line swallows the moon. Two toothed jaws close fore and aft on a moonstone, and four fangs curve over its dome. A fur ruff streams from the jaws over the head's walls and down both shoulders. At the palm the wolf is bound by Gleipnir, the silken fetter.
  - **What changed from the brief:** the jaws are drawn as jaws, arcs with tooth notches (`outline::jaw`), **not** as crescent moons (`moon_outline`). Two crescents round a moonstone read as celestial, which is what Logan disliked in Zenith.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Face | "Jaw, upper" and "Jaw, lower" stamps with their teeth pointing at the stone; four fang claws; the moonstone. |
| Head walls, shoulders, 90 ± 110 | Ruff of S-curved fur locks, with guard hairs on their crests. |
| Palm, 270 ± 35 | Gleipnir braid, knotted at 235 and 305 where it meets the fur. |
| Bore | Plain. Optional hollow under the head (step 10). |

- **Base:**
  - `PRESETS` "010" Trillion, native 18 × 20 (`core/imported_base.rs:1106`), attached unmirrored with `sand_envelope = false`.
  - Flat profile, `width_mm 17.0`, `head.length_mm 16.0`, bore 19.0, edge 0.3, comfort 0.1.
  - The plan is upright (7.7 mm, F2), and its wedge reads as the wolf's head in plan.

- **Process:** lost wax, Gold 18k (the alloy sets only shrink and weights; renders are studio gold).
  - The fangs overhang the stone, and the jaws' tooth notches face back across any parting line.
  - Recipe: `LostWax`, `min_draft 0`, `min_detail 0.15`, `min_section 0.8`.

- **Stones:** one moonstone round cabochon 10 mm, `Gem::cabochon(GemCut::Round, 10.0)` (`core/gem.rs:250`). A cabochon standing proud gets no bur; `builders::setting_features` skips it (`builders.rs:971`).

- **Build, step by step:**
  1. Base, chart, then `Atlas` and `Hide`.
  2. **Mask:** zero every painted layer inside the stone's radius plus 1.0 mm and inside the jaw outlines plus 0.3 mm.
  3. **Layer "Ruff"** (0.8 mm, `window(90, 220)`): S-curved lanceolate locks, each with a 0.08 mm centre groove, in three offset rows. They run from the table's rim down the walls (`hide.wall`) and back along the shoulders (`hide.along`). Painter `fur_lock`; tips at least 0.25 mm.
  4. **Layer "Guard hairs"** (0.35 mm, `Blend::SmoothMax`, `soft_mm 0.2`): finer strands riding the locks' crests.
  5. **Layer "Gleipnir"** (0.45 mm, `Window::around(270, 70)`): an **authored SVG braid** ("Gleipnir", gradient-shaded strands 0.45 mm wide with gaps ≥ 0.2 mm) in a `TilingLayer` whose `repeats_around` gives strands ≥ 0.3 mm. The builtin `Procedural::Braid` measured 0.04 mm gaps on the braided band (Doctrine § Manufacturing analysis), so use it only if `dfm::findings_in` stays empty.
  6. **Layer "Binding knots"** (0.4 mm): a `DecalLayer` of `Procedural::CelticKnot` with two `Decal { theta_deg: 235 / 305, size_mm: 4.0, .. }`. Check DFM at that size.
  7. **Layer "Graver's hair lines"**: `bench_only`, `Blend::Subtract`, 0.10 mm.
  8. **Stamps "Jaw, upper" and "Jaw, lower"**:
     - outline `outline::jaw(5.5, 1.4, 150.0, 5, 0.6)` (`core/outline.rs:419`: an arc 1.4 deep outside r 5.5, sweeping 150° about +y, five teeth 0.6 long pointing inward);
     - both centred on the stone's centre, height 0.7, sink 0.35, draft 2°;
     - turned so one arc wraps the stone fore and the other aft along the ring. The outline sweeps about +y, which is across the band in the stamp frame, so `rot_deg` is ±90. Confirm the turn in the stamp preview (**unverified**).
     - Tooth roots are ≥ 0.4 mm wide, because a stamp is read by granulometry (`dfm::STAMP`).
  9. **CAD document:**
     - F1 `Operation::Band`.
     - F2 `builders::stone_feature(2, gem, Placement::ring(90.0, builders::stand_off_mm(builders::CLAW, gem)))`.
     - F3 `builders::feature_on(3, "Fangs", builders::CLAW, 2, json!({"prongs": 4, "wire_mm": 1.5, "style": "Fang", "grouping": "Jaws", "tip": "Point"}))`, with `Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: 0.25 }`. The `style`, `grouping` and `tip` names are P6's plan and do not exist yet.
     - Until P6: `json!({"prongs": 4, "wire_mm": 1.5})`. The node stays the same and gains three params later.
  10. **Optional hollow** (**unverified**; drop it if the wall binds):
      - F4 `Operation::Plane { base: PlaneBase::Tangent { theta_deg: 90.0, across_mm: 0.0 }, offset_mm: -(head depth over the bore) }`;
      - F5 `Sketch` on it: the trillion plan scaled to ±5 mm along the ring and inset 1.2 mm;
      - F6 `Extrude` 2.0 mm outward with `Attach::Cut`.
      - Keep the pocket to ±5 mm along the ring: at ±8 mm the bore falls 4.4 mm below the tangent plane. The radial verdict cannot see axial webs, so read `thinnest_wall_mm` ≥ 0.8 and a section at θ 90.
  11. Run the gates.

- **What it shows off:**
  - the claw builder as literal fangs;
  - jaw outlines from the outline library, concave on purpose, which wax allows;
  - painted fur on factory stock;
  - a braid as a story element;
  - an optional CAD cut on imported stock.

- **Traps and how they are avoided:**
  - **Crescents would read as moons** (plan). Use `outline::jaw`.
  - **Fang tubes fold** where they bend tighter than the tube radius. P6 pins `self_crossings == 0` for 3–8 prongs. Check the built ring too, looping over every part as Arachne does.
  - **Fangs through the bore wall.** `setting::claw_head_within` refuses them (`setting.rs:697`).
  - **Cabochon crown.** Claws meet it partway up its dome; confirm the fang tips land on the dome, not above it, in the section pane.
  - **The braid fails DFM** (step 5).
  - **Stock folds** (Draco's method).

- **Needs:** P6 (critical); P3 and P4 (landed); C-B3 `fur_lock`; the "Gleipnir" SVG; P7 for the template.

- **Template:**
  - `/imported_base` patch of the unmodified 010, about 0.45 MB;
  - `alpha.png` × 3;
  - `alpha.svg` × 1 (braid);
  - `alpha.proc` × 1 (`CelticKnot`);
  - a `cad.feature` chain of 3 features (6 with the hollow), which carries 3 `/cad/*` patches as Arachne's did;
  - a `/stamps` patch with 2 jaws;
  - `/draft/*` patches.
  - About 2.5 MB. Over the patch budget until P7.
  - Expose US size and the stone's width (the stone builder's params).

- **Risk:** 3.5/5, medium; everything hangs on P6. The fallback ships four wire claws, and the jaws and ruff carry the wolf. Re-render when P6 lands.

---

## Phoenix — *reborn*

- **Status:** not started; buildable now with the **re-keyed** body (P3 and P4 on master). With P5 (batch 16) the brief's original keys could stand.

- **Concept:** A phoenix wraps the finger. Its flaming breast is at the top, round a fire opal. Graded embers run down its spine on both shoulders. Its wings enfold the band in opposite directions on the two side faces, and its tail streamers cross at the palm.
  - **What changed from the brief:** the keyframes, so the wing decals stay on the side faces without P5 (plan §1). The wings are also split into per-tier decals.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Top | Fire-opal oval cab in a made collet on the flared breast. |
| Crown, shoulders | Flame contour feathers: chevrons pointing toward the palm, with their points on the parting line. A graded bead-set sapphire run on the parting line down each shoulder. |
| Side faces | Low face: one wing sweeping from the breast toward 330. High face: the other sweeping toward 210. Coverts, secondaries and primaries in three stepped tiers. |
| Palm | Tail streamers crossing on both faces; small contour feathers on the crown. |
| Bore | Comfort 0.2. |

- **Base:**
  - `ProfileStyle::Flat`, 6.6 wide × 3.4 thick, then `flatten_sides()` (`core/profile.rs:548`). Square faces are where the wings go, and their usable height is thickness minus crown.
  - Bore 18.6, comfort 0.2, `ShankKind::Keyframes`, `amount 1.0`, ten keys.
  - The keys are re-keyed so `thickness_scale ≥ width_scale` at every key. That way no station's side face is a smaller share of its section than the reference's, and the reference-resolved `VGate` cannot spill onto the crown fillet.

| θ (°) | width | thickness | crown |
|---|---|---|---|
| 90 | 1.35 | 1.45 | 0.85 |
| 62, 118 | 1.28 | 1.36 | 0.90 |
| 28, 152 | 1.12 | 1.16 | 1.00 |
| 340, 200 | 1.00 | 1.00 | 1.00 |
| 300, 240 | 0.92 | 0.95 | 1.00 |
| 270 | 0.88 | 0.92 | 1.00 |

  - Before painting, run `cargo run -p ringdesign-core --release --example side_gate_probe` with these keys. That example measures how far side-face relief spills where a keyframed station departs from the reference. Every station under the wings (θ 330 → 90 → 210) must show its own side-face run covering the reference's.
  - If P5 has landed, the brief's keys (90: 1.45 w / 1.22 t) are allowed instead.

- **Process:** Delft sand, Silver 925, `mf::Recipe::sand(SandProcess::DelftClay)`. The collet and the bead heads stay out of the pattern, which carries `mark_mm` drill dots (Doctrine § Shown finished, exported as the pattern).

- **Stones:**
  - Fire opal: `Gem { l_mm: 8.0, ..Gem::cabochon(GemCut::Oval, 6.0) }`.
  - 14 orange sapphire rounds from `Gem::calibrated(GemCut::Round, 2.2)`, graded to about 1.1 mm in two runs of seven. The solver decides the count; record what it gives.

- **Build, step by step:**
  1. Body, then `Atlas::of` (the procedural path: one modulated section per column) and `Hide::of`.
  2. **Layer "Flame plumage"** (crown, 0.45 mm, `window(90, 250)`):
     - chevrons whose point rides the parting line and leads toward the palm, following the scute rule in `ex/stock_masterworks.rs:1202` (`fall = (0.45 / pitch).min(0.3)`);
     - each free edge a flame-tongue scallop;
     - plate joints from `Joints::eccentric(start, end, 2.2, 1.2)` along `hide.along` on each shoulder (2.2 mm at the breast to 1.2 at ±120°);
     - a flat "ember seat" strip, 1.8 mm wide, on the parting line under both runs, and the opal's plan plus 0.8 mm kept flat;
     - `draft_clamp`, then `hide_layer`.
  3. **Layers "Wing, low" and "Wing, high"** (side faces, 1.0 mm). The "Phoenix wing" SVG is split into three `SvgAlpha`s (coverts, secondaries, primaries), and each is a `DecalLayer` decal:
     - `Decal { theta_deg, v_mm, size_mm, height_mm: 1.0, flip }`, with `flip: true` on the high face because the chart reads true from −Z;
     - the entry's `window.v_gate = VGate::SideFaces(SideFacePick::Low)` or `(High)`;
     - each tier's SVG drawn squashed by `FieldContext::station_stretch(θ)` at its own centre (`core/field.rs:153`). A single 24 mm decal crosses stations whose stretch runs from 1.0 to about 1.4.
     - Tier centres: low face θ ≈ 60 / 30 / 355, high face θ ≈ 120 / 150 / 185, each tier about 9 mm wide.
  4. **Layer "Tail streamers"** (side faces at the palm, 0.8 mm): "Phoenix tail" SVG decals at θ 270 on both faces, the plumes crossing.
  5. **Layer "Barbs"**: `bench_only`, Subtract, 0.10 mm, vane striations on the crown feathers.
  6. **Seat "Fire opal, collet"**:
     - `SeatPadLayer { theta_deg: 90.0, v_mm: hide.crest_at(&a, 0.0).1, style: SeatStyle::Boss, crown: 0.3, blend_mm: 0.55, metal_true: true, solid: SolidKind::Bezel, .. }`;
     - then `fit_stone(gem)` and `height_mm = 0.7`.
  7. **Runs "Fire trail, east" and "Fire trail, west"**:
     - `SeatRunLayer { gem: Gem::calibrated(GemCut::Round, 2.2), taper: 0.5, taper_theta_deg: 90.0, bridge_mm: 0.75, seat: SeatPadLayer { style: GypsyMound, height_mm: 0.5, crown: 1.0, blend_mm: 0.5, solid: SolidKind::Bead, v_mm: crest, .. }, .. }`;
     - then `solve_spacing(&d.field_context())` (`field.rs:1999`);
     - windows `Window::around(40, 70)` and `around(140, 70)`, fade 6, so the gap at the top is the opal's. Neighbours share their beads.
  8. Run the gates. In sand, also read `castability::attribute_undercuts`' named culprit if the field is not clean.

- **What it shows off:**
  - keyframed sculpture;
  - atlas painting on a modulated body;
  - SVG plumage on side faces with flip and stretch handled;
  - a made collet;
  - graded bead-set runs;
  - a bench layer.

- **Traps and how they are avoided:**
  - **Side-face spill** (plan F4). Re-key and probe as above, or wait for P5. **Fallback:** paint the wings onto the atlas instead, projecting each sample that faces the pull (`|n.z| ≥ cos 10°`, from the atlas's own normals) into the wing's millimetre frame, the `project_cheek` pattern in `ex/stock_masterworks.rs:235`. That is station-aware by construction and costs one more PNG in the template.
  - **Walls facing round the ring lean on a widening section** (Doctrine § Two masterworks). The flare runs from ±28° to ±118°, so plumage stays ≤ 0.45 mm with drafted falls.
  - **A stone is sized to its face.** 2.2 mm plus 1.8 mm of gypsy stock is 4.0 mm, against a crown about 8 mm wide at the shoulders.
  - **Seat skirt DFM.** `blend_mm` is at least 0.5; a 0.4 mm skirt measured 0.34 against a 0.35 floor.
  - **Bridge at the small end.** Ask 0.75, as Diamondback does (`ex/serpentarium.rs:652`).
  - **A gem column must run on the parting plane.** Both runs do.
  - **A crown `VGate::Band` fade is a wall facing the crest.** The wings use only side-face gates.

- **Needs:** P3 and P4 (landed); P5 preferred; C-B3 `plumage::contour` (promote to core, since Corvus reuses it); the SVG art "Phoenix wing" (three tiers × 2) and "Phoenix tail".

- **Template:**
  - The procedural body lifts to nodes, plus a `/shank/keys` patch until P7 un-hides `keys`.
  - `alpha.png` × 2;
  - `alpha.svg` × 7;
  - `layer.seat`;
  - `layer.seatrun` × 2.
  - About 1.5–2 MB.
  - Expose size, width and thickness, but not the flare: the painted plumage is bound to the body it was painted on (the brief's `alpha.hide` enabler is deferred).

- **Risk:** 4/5, medium.
  - If the plumage still leans on the flare, lower the 90° key to 1.25 w / 1.35 t.
  - If decals misbehave, use the atlas-projected wings.

---

## Corvus — *Huginn and Muninn*

- **Status:** not started; buildable now. P3's atlas lands on a bypass to within a texel, and P4's `StampTop::Gable` gives the culmen.

- **Concept:** Odin's two ravens fly past each other on a bypass. Each arm is one bird, feathered from beak to tail. Their beaks are struck on the spine either side of a black onyx and point away from it: thought and memory sent out. Their tails cross at the palm.
  - **Plan instruction:** the beaks carry the whole identity. Render-review them in round 1 **before any feathers are painted**. If they do not read, grow them to 7 mm and drop the tail interleave.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Top | Onyx cab in a collet at the crossing. Beaks on the parting line at about ±22° from the top, pointing outward. |
| Crown | Contour feathers flowing from each beak back along its own arm, graded 0.9 → 2.2 mm. |
| Side faces | Folded primaries laid along the ring; the bypass's own seam channel at the crossing (`BYPASS_GROOVE_MM` 1.0, `core/profile.rs:2332`). |
| Palm | Two wedge tails interleaved at 270. |
| Bore | Comfort 0.2. |

- **Base:**
  - `ProfileStyle::LowDome`, 6.0 × 2.8, comfort 0.2, bore 18.6.
  - `ShankKind::Bypass` at `amount 1.0`. Each arm slides to ±`BYPASS_OFFSET` (0.45 of the half-width) over 100°→30° before the top and rounds to its tip over the 30° before `BYPASS_TIP_DEG` (35° past it) (`profile.rs:2321-2351`).

- **Process:** Delft sand, Silver 925. A bypass measured 0.0085% at −0.8° on a low dome (Doctrine § Bypass). The collet is a bench part under sand, and the pattern carries its drill dot.

- **Stones:** one black onyx oval cabochon 8 × 6, `Gem { l_mm: 8.0, ..Gem::cabochon(GemCut::Oval, 6.0) }`.

- **Build, step by step:**
  1. **Beak spike.** The bare body, the collet seat (step 8) and the two beak stamps (step 7) only; render the hero, face and side views. Proceed only if the beaks read.
  2. `Atlas::of` and `Hide::of`.
  3. **Arm ownership:** per column, `profile::bypass_span(off, k)` (`profile.rs:2351`) names the arm under each sample. Huginn's feathers flow from Huginn's beak, Muninn's from Muninn's.
  4. **Layer "Contour feathers"** (crown, 0.5 mm, `window(90, 320)`):
     - chevron rows pointing toward the palm, rachis on the crest;
     - joints from `Joints::eccentric` graded 0.9 mm at the heads to 2.2 on the backs;
     - a flat "head plate" under each beak, flat along the ring over the beak's footprint plus 0.6 mm;
     - `draft_clamp`.
  5. **Layer "Folded primaries"** (side faces, 0.8 mm): long remiges along the ring, shingled, tips toward the palm. Paint them on the atlas and choose the face per sample from its normal (`|n.z| ≥ cos 10°`), not with a `VGate`: the arms slide, and a reference-resolved gate rides a moving flank (Uraeus measured 2–4° of lean).
  6. **Layer "Crossed tails"** (palm, 0.5 mm, `window(270, 70)`): rectrices along the ring, shingled so the central pair is highest and each outer feather steps down. That is the only arrangement in which a feather's along-ring edge falls away from the parting line. Two tails interleave.
  7. **Stamps "Beak, Huginn" and "Beak, Muninn"**:
     - outline `outline::lanceolate(5.2, 2.0, 0.3)` (`core/outline.rs:264`), pointing outward: `rot_deg 0` on the +θ beak and 180 on the −θ beak;
     - `top: StampTop::Gable { rise_mm: 0.15, axis_deg: 0.0 }`, a ridge along the parting line that reads as the culmen;
     - height 0.45, sink 0.3, draft 4°;
     - placed at `hide.crest_at(&a, ±along(22°))`, where `along(22°)` is `hide.along` at the column 22° off the top;
     - assert `stamp.parting_monotone(&d).is_ok()` for both.
  8. **Seat "Onyx, collet"**:
     - `SeatPadLayer { theta_deg: 90.0, v_mm: crest, style: SeatStyle::GypsyMound, height_mm: 0.55, crown: 1.0, blend_mm: 0.45, metal_true: true, solid: SolidKind::Bezel, .. }` plus `fit_stone`;
     - Uraeus's crossing cab is the precedent (`ex/serpentarium.rs:339`): a cab has no pavilion, so the crossing wedge costs it nothing.
  9. **Layer "Graver's barbs"**: `bench_only`, Subtract.
  10. Run the gates, both at 384 × 192 and at the build resolution.

- **What it shows off:**
  - a bypass as two creatures;
  - painting on a sliding modulated body;
  - struck beaks with a gable culmen;
  - a made collet;
  - normal-chosen side-face painting.

- **Traps and how they are avoided:**
  - **The arms slide.** Paint everything in 3-D and draft-clamp it. Use no fixed-v rows and no bead lines: milgrain on a wandering crest measured 3% at 50°.
  - **The surface under a stamp may vary only across the band.** The union section changes along the ring at ±14–30°, so the head plate flattens it. Phantoms move with resolution while real obstructions converge (Caiman), hence the two resolutions.
  - **A bypass crossing needs room under a stone.** It is clean at 5.0 mm; this band is 6.0.
  - **Tails' along-ring edges** are legal only as outward-descending shingles.
  - **The gable ridge** must lie on the parting line. `parting_monotone` enforces it.
  - **No faces.** A beak stands alone: no eye, no nostril, no head outline around it.

- **Needs:** P3 and P4 (landed); C-B3 `plumage::{contour, remex, tail_fan}` (contour from Phoenix).

- **Template:**
  - a procedural body with a Bypass shank node (no keys, so no `/shank/keys` patch);
  - `alpha.png` × 4;
  - `layer.seat`;
  - a `/stamps` patch with 2 beaks until P7.
  - About 2–3 MB.
  - Expose size only. Width and thickness would stretch the painted skin.

- **Risk:** 3.5/5, medium; the beaks decide it.
  - Fallback 1: 7 mm beaks and no tail interleave.
  - Fallback 2: if the gable reads as a mask plate, `StampTop::Ridge { rise_mm, from, to, end_mm }` falling toward the tip, so the beak tapers.

---

## Kraken — *the deep's grip*

- **Status:** not started; blocked on **P6** (Tentacle style) and **C-B1**, which lands in this ring's lane. P5 is cosmetic here, because this ring is wax.

- **Concept:** Six arms wrap the finger, crossing over the crown. Their tips rise at the top and curl round a labradorite as its claws. The mantle's cellular skin covers both side faces.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Top | 10 mm labradorite cabochon in a six-tentacle claw head, raised on the flared mantle. |
| Crown, shoulders, palm | Six tapering tentacles (three curve layers, each mirrored across the band) crossing diagonally, with sucker rows on their inner curl. |
| Side faces | Mantle skin in fine Voronoi cells. |
| Bore | Comfort 0.2. |

- **Base:** `ProfileStyle::DShape` 6.0 × 2.7, comfort 0.2, bore 18.6, `Keyframes`, eight keys. The brief says nine, but its list is eight.

| θ (°) | width | thickness | crown |
|---|---|---|---|
| 90 | 1.50 | 1.30 | 0.90 |
| 60, 120 | 1.30 | 1.15 | 1.00 |
| 10, 170 | 1.05 | 1.02 | 1.00 |
| 310, 230 | 0.92 | 0.96 | 1.00 |
| 270 | 0.86 | 0.92 | 1.00 |

- **Process:** lost wax, Silver 925 or 14k.
  - Tentacles crossing the crown diagonally would lean in sand (off-crest rails measured 5.8% at 29°, Doctrine § Templates are code).
  - The claws overhang the stone.

- **Stones:** one labradorite round cabochon 10 mm, `Gem::cabochon(GemCut::Round, 10.0)`, sitting proud with no bur.

- **Build, step by step:**
  1. Body.
  2. **CAD document:**
     - F1 `Band`.
     - F2 `stone_feature(2, gem, Placement::ring(90.0, stand_off_mm(CLAW, gem)))`.
     - F3 `feature_on(3, "Tentacle claws", CLAW, 2, json!({"prongs": 6, "wire_mm": 1.4, "style": "Tentacle", "tip": "Point"}))`, with `Join`, `Cast` and `blend_mm 0.25`. The seam bead runs along every seam the boolean reports.
  3. **Claw feet.** For a round plan with six claws, `setting::Plan::claw_angles` starts at 30° (`setting.rs:139-147`): 30, 90, 150, 210, 270 and 330. Mirroring across the band pairs them 30↔330, 90↔270 and 150↔210, three pairs, which is what `mirror_v` needs.
     - Read each foot's world position off the built head and convert it to `(θ, v)` through the nearest `Atlas` sample.
     - A formula through `station_stretch` is the second choice.
  4. **Layers "Tentacles A / B / C"**: three `CurveLayer`s (C-B1 fields), each a `LayerEntry` with `blend: Blend::SmoothMax, soft_mm: 0.25`:
     - `repeats_around 1`, `mirror_v true`, `closed false`, `profile WireProfile::Round` (a cosine dome, since a circular section has a vertical wall);
     - `width_mm 2.6` and `height_mm 1.2`, with `widths` running 1.0 → 0.17 (2.6 → 0.45 mm) and `heights` 1.0 → 0.25 (1.2 → 0.3 mm);
     - up to 64 points (`MAX_CURVE_POINTS`, `curve.rs:17`), spiralling from the foot across the crown to the palm and round.
  5. **Suckers**, on each tentacle layer: `beads: Some(CurveBeads { pitch_mm: 0.75, diameter_mm: 0.55, height_mm: 0.25, offset: -0.6, graded: true, phase: 0.0 })`, offset toward the inside of the curl. The smallest bead must be ≥ 0.22 mm.
  6. **Layer "Mantle skin"**: a `TilingLayer` of `Procedural::Voronoi` with `window.v_gate = VGate::SideFaces(SideFacePick::Both)`, height 0.2.
     - The builtin carries 3 × 3 sites per tile, so the tile is 3.6 mm for 1.2 mm cells.
     - `repeats_around` is the integer nearest the side face's circumference over 3.6.
  7. Run the gates.

- **What it shows off:**
  - curve layers as anatomy;
  - tapered wires with bead rows;
  - a procedural Voronoi skin;
  - the tentacle claw head;
  - a ring made entirely of nodes with no painted atlas, the collection's lightest and most instructive template.

- **Traps and how they are avoided:**
  - **Tentacle claws curl tighter than their base.** That is legal only because the radius shrinks along the curl. P6's pin is bend radius ≥ local radius and `self_crossings == 0`; check the built ring too.
  - **Crossings under plain `Max`** leave knife valleys under 0.15 mm, hence SmoothMax.
  - **Sucker beads under 0.22 mm** fail granulometry.
  - **Claws must keep the bore wall** (`claw_head_within`).
  - **The graph node's coverage test** fails unless C-B1's fields get pins in `graph/nodes/layer.rs`.
  - **The feet must meet the claws.** Read them off the built head (step 3), never guess.

- **Needs:** P6 and C-B1 (both critical); P5 optional; P7 for keys.

- **Template:**
  - everything as nodes: profile, shank (plus a `/shank/keys` patch until P7), `layer.curve` × 3, `alpha.proc` Voronoi, and a `cad.feature` chain;
  - 3 `/cad/*` and 3 `/draft/*` patches, as in Arachne's lift;
  - about 30 kB.
  - Expose size, flare (the key-90 width), tentacle height and stone size. Exposing geometry is safe here because nothing is painted.

- **Risk:** 3/5, medium-high; the ring is only as good as P6 and C-B1.
  - Without P6: six wire claws, and the tentacles still end at the feet.
  - Without C-B1 there is no honest fallback for the suckers, so C-B1 is S-sized and lands first in this lane.

---

## Manticora — *the tail that throws*

- **Status:** not started; buildable now with an **oval** ruby. The pear waits for C-B2 (batch 3B).

- **Concept:** The ring is the manticore's scorpion tail. Its segments grow from the palm to a swollen venom bulb at the top, where a ruby sits in a collet like a drop of venom. The hooked stinger rises behind it and curls over the stone. Throwing quills bristle down both flanks, and a graded line of black spinels runs down each shoulder on the diagonal.
  - **What changed from the brief (plan §1):** use an oval until C-B2. Take the tergites from P3's `Joints::eccentric`. Strike the 40 quills as P4 rows, not as 40 hand-placed stamps.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Top | Ruby in a collet on the keyframed telson bulb; the twisted-sweep stinger hooked over it. |
| Crown | Graded tergites with two dorsal keels; a tilted, graded princess run on each shoulder. |
| Flanks | Quills, two per segment per side. |
| Side faces | Pleural folds aligned to the segment joints. |
| Palm | The smallest segments. |

- **Base:** `ProfileStyle::HighDome` 5.4 × 2.2, bore 18.6, `Keyframes`, ten keys (the brief's "±" angles are offsets from the top):

| θ (°) | width | thickness |
|---|---|---|
| 90 | 1.20 | 1.55 |
| 65, 115 | 1.16 | 1.48 |
| 30, 150 | 1.10 | 1.30 |
| 345, 195 | 1.04 | 1.14 |
| 300, 240 | 0.96 | 1.00 |
| 270 | 0.92 | 0.90 |

  `crown_scale` is 1.0 at every key. The 16-key cap means the keys cannot carry the segments, so the segments are relief.

- **Process:** lost wax, Gold 14k or 18k. The stinger overhangs.

- **Stones:**
  - Ruby oval 7 × 5, `Gem { l_mm: 7.0, ..Gem::calibrated(GemCut::Oval, 5.0) }`. Swap to `GemCut::Pear` after C-B2; today a pear is seated as an ellipse (`gem.rs:156`).
  - About 12 black spinel princesses, `Gem::calibrated(GemCut::Princess, 2.0)`, graded.

- **Build, step by step:**
  1. Body, `Atlas::of`, `Hide::of`.
  2. **Layer "Tergites"** (0.6 mm, `window(90, 350)`):
     - plates between `Joints::eccentric(start, end, 3.2, 1.7)` on each shoulder, along `hide.along` from the bulb to the palm; the series closes on an integer count at the pitches' geometric mean (`skin.rs:370`);
     - each plate a loaf with a posterior lip and two carinae either side of the crest, `beast::tergite`;
     - joints 0.35 mm deep, running across the band.
  3. **Layer "Pleural folds"** (0.3 mm): transverse wrinkles on the same joints, painted on the atlas where `|n.z| ≥ cos 10°`. A `VGate` would spill on this keyframed body; that is only cosmetic in wax, but the atlas avoids it.
  4. **Layer "Granulation"** (0.12 mm): granules ≥ 0.25 mm on the plates.
  5. **Layer "Graver's carina lines"**: `bench_only`, Subtract.
  6. **Stamp rows "Quill, left flank" and "Quill, right flank"**, each a `setting::StampRow`:
     - `stamp: Stamp { name: "Quill", outline: outline::quill(1.4, 0.45, 0.12), height_mm: 0.35, sink_mm: 0.25, draft_deg: 0.0, rot_deg: <pointing toward the palm>, tier: 0, top: StampTop::Taper { axis_deg: 0.0, tip_mm: 0.15 }, .. }`;
     - `path: RowPath::ChartV { v_mm: crest_v ± 1.5 }`, `from_deg: 112.0, to_deg: 250.0`, `count` equal to the tergite count on that arc, `taper: 0.4`, `mirror_shoulders: true`;
     - `d.stamps.extend(setting::stamp_row(&d, &row))` gives 2 flanks × 2 shoulders × about 10 = 40.
     - If the row's stations drift more than a quarter plate off the plates, place one stamp per plate from `Joints` instead.
  7. **Runs "Venom spinels, east" and "Venom spinels, west"**:
     - `SeatRunLayer { gem: princess 2.0, tilt_deg: 45.0, taper: 0.55, taper_theta_deg: 90.0, bridge_mm: 0.8, seat: SeatPadLayer { style: GypsyMound, height_mm: 0.45, solid: SolidKind::Bead, v_mm: crest, .. }, .. }`;
     - then `solve_spacing`, with windows `around(20, 90)` and `around(160, 90)`;
     - this is Diamondback's construction (`ex/serpentarium.rs:637`), which Logan loved.
  8. **CAD document:**
     - F1 `Band`.
     - F2 `stone_feature(2, ruby, Placement::ring(90.0, stand_off_mm(BEZEL, ruby)))`.
     - F3 `feature_on(3, "Venom collet", BEZEL, 2, json!({"wall_mm": 0.55, "lip": 0.35}))`, `Join`, `Cast`.
     - F4 `feature_on(4, "Seat bur", BUR, 2, json!({"through": false}))`, `Cut`, `Cast`.
     - F5 `Operation::Plane { base: PlaneBase::Tangent { theta_deg: 97.0, across_mm: 0.0 }, offset_mm: 0.0 }`.
     - F6 `Operation::Sketch` "Aculeus section": a keeled Bézier teardrop 1.8 × 1.3 mm (four `Geometry::Bezier`) on a `Workplane` square to the path's start. Give it the tangent frame at θ 97 explicitly (origin, x, y), as Arachne's leg sections do. Anchoring the sketch to F5 through `on_face` is **unverified** for plane features.
     - F7 `Operation::Twist { sketch: Profile::Feature { feature: 6 }, path: <inline Sketch>, degrees: 18.0, end_scale: 0.18 }`. The path is an inline `Sketch`, not a feature (`core/cad.rs:81-86`). It is drawn on the default workplane, world XY, which is the parting plane: a 1.2 mm radial rise at θ 97, then an `Arc` of r 3.6 over 120°, curving forward over θ 90 so the tip hangs 1.2 mm above the ruby's far end.
     - F7's component is `Attach::Separate`, `Stage::Cast`, `Placement::Free`. Add `cad::Joint { a: 1, b: 7, clearance_mm: 0.05, method: "Solder after the ruby is set", notes }` to the document's joints (`cad.rs:1284`). Only a separate stinger leaves the setter a clear collet to burnish.
  9. Run the gates, and check the stinger's clearance to the collet lip (≥ 0.3 mm) in the section pane.

- **What it shows off:**
  - a keyframed tail;
  - graded relief segments;
  - a tapered twisted sweep;
  - a separate part with a solder joint;
  - a tilted, graded, bead-set run;
  - stamp rows (the reel plays the family "Quill" as one beat).

- **Traps and how they are avoided:**
  - **A twisted sweep that crosses itself is refused** (`cad/twist.rs:498`). Ease the arc and keep `degrees` at 20 or less.
  - **Stinger tip under the 0.15 mm floor.** An `end_scale` of 0.18 leaves about 0.23 mm on the 1.3 mm section.
  - **A pear seated as an ellipse** leaves gaps at its shoulders and its point through the collet, hence the oval now.
  - **A tilted run at an oblique bearing leans** (5–6° at 30°, the showoff lesson), so keep it at 45°.
  - **`bridge_at` is within 3% of the solver.** Ask 0.8 against the 0.8 section floor.
  - **The separate stinger is its own casting.** It is left out of the ring's pattern and named so (`manufacturing::Casting::Ring`).

- **Needs:** P3 and P4 (landed); C-B2 for the pear; C-B3 `beast::tergite`.

- **Template:**
  - body nodes plus `/shank/keys`;
  - `alpha.png` × 4;
  - `layer.seatrun` × 2;
  - a `cad.feature` chain of 7 features, with a `/cad/joints` patch;
  - a `/stamps` patch with 40 quills until P7's `stamp.row` node, which turns it into two nodes.
  - About 1.5 MB.
  - Expose size and stone width. Exposing flare would stretch the painted tergites.

- **Risk:** 4/5, medium. If the twist refuses at any setting, drop to `degrees 0` (a tapered, untwisted sweep). A plain `Sweep` has no taper and is the last resort.

---

## Harpyia — *the snatcher*

- **Status:** not started. The lofted wings are buildable now (`Operation::Loft`, multi-source `Pattern`). The talons are blocked on **P6**, with six wire claws as the fallback. She is the highest-risk ring in the collection and one of nine; the weakest after round 1 is dropped.

- **What changed from the brief** (Logan, 2026-09-24):
  - She is **restored.** The plan had replaced her with Arachne; both now stay.
  - **Wings are lofted feathers.** Each feather is its own `Operation::Loft` through five cambered sections along a curved rachis. They overlap in three tiers, and one multi-source `Pattern` mirrors them all.
    - This replaces the brief's F6–F20: one work plane, one region sketch, eleven flat region extrudes at 1.55 / 1.15 / 0.95–0.75 mm with 2° draft, a kernel union and one mirror. Flat tiers read blocky, which was Logan's complaint about the Workshop set, and the review checklist forbids flat, blocky CAD.
    - The kernel union is gone, so the 500-face kernel-operand limit no longer matters: each loft is tessellated and joined to the band by `csg`.
    - The region sketch survives only as an optional plan guide and is not built.
  - **Unchanged:** the base, process, stone, talon head, cathedral legs, seat bur and the three height-field layers.

- **Concept:** A harpy perches at the top with her talons locked on a storm-blue sapphire and her legs standing as two arches. Her wings rise from beside the stone above the band, as Hypnos's rise from his temples, and sweep back along both side faces in overlapping lofted feathers. Her back feathers cover the crown behind her, her tail fans at the palm, and scaled legs run down the forward shoulder.

- **Theme face to palm:**

| Zone | What is there |
|---|---|
| Top | 8 mm sapphire; six talons (two feet of three); cathedral arches as legs. |
| Side faces, θ 104–228, both faces | Two wings of lofted feathers, the second a mirror of the first across the band. They rise up to about 3.5 mm above the crest at the wrist (θ ≈ 118). |
| Crown west, θ 105–225 | Contour back feathers between the wings. |
| Crown east, θ 345–75 | Scutellate leg scales under the forward arch. |
| Palm, 250 ± 25 | Tail fan. |
| Bore | Comfort 0.2. |

- **Base:** `ProfileStyle::Flat` 7.2 × 3.8, comfort 0.2, `flatten_sides()`, `ShankKind::Uniform`, bore 19.0.
  - The inner radius is 9.5 and the crest radius 13.3. The side faces are the planes z = ±3.6.
  - The band is uniform on purpose: every feather is placed in world coordinates (`Placement::Free`) and stays flush on a plane side face.

- **Process:** lost wax, Gold 18k. Wing plates standing beyond the band's radius have undersides facing the drag.

- **Stones:** one sapphire round 8 mm, `Gem::calibrated(GemCut::Round, 8.0)`.

- **Build, step by step:**
  1. Body. Keep `d.draft` as wax, and keep the recipe out of the design.
  2. **CAD, the head:**
     - F1 `Band`.
     - F2 `stone_feature(2, gem, Placement::ring(90.0, stand_off_mm(CLAW, gem)))`.
     - F3 `feature_on(3, "Talons", CLAW, 2, json!({"prongs": 6, "wire_mm": 1.3, "style": "Talon", "grouping": "Feet", "tip": "Point"}))`, `Join`, `Cast`, `blend_mm 0.25`. The `style`, `grouping` and `tip` params are P6 names; until P6, use `json!({"prongs": 6, "wire_mm": 1.3})`.
     - F4 `feature_on(4, "Legs", CATHEDRAL, 2, json!({"spread_deg": 34.0, "rise": 0.7, "wire_mm": 1.3}))`, `Join`, `Cast`, `blend_mm 0.2` (`builders.rs:188-192`: two wire arches rising from the band's shoulders to the head's gallery rail).
     - F5 `feature_on(5, "Seat bur", BUR, 2, json!({"through": true}))`, `Cut`, `Cast`.
  3. **Wing plan** (high face, z > 0), from the brief:
     - the root runs along r 10.2 from θ 104;
     - the leading edge rises to the wrist at r 16.8, θ 118;
     - primaries fall from θ 120 to 228;
     - the trailing edge returns at r 12.6 → 10.4.
     This plan is the envelope the feathers fill.
  4. **Feathers**, twenty per wing, one `Operation::Loft` each, built by the ring-local `feather_loft(&FeatherSpec)` (features F6–F25):

     | Tier | Count | Length | Width | Thickness | Lift over the side face (z top) | Roots |
     |---|---|---|---|---|---|---|
     | Coverts | 5 | 3.0–4.5 | 1.6–2.0 | 1.1 | 3.6 + 1.2 | r 10.4–12.6, θ 104–130 |
     | Secondaries | 6 | 5.0–7.0 | 1.8–2.2 | 0.95 | 3.6 + 0.9 | under the coverts, θ 110–150 |
     | Primaries | 9 | 7.0–11.0 | 1.8–2.4 | 0.85 | 3.6 + 0.6 | the wrist and the trailing plan, θ 118–228 |

     - **Rachis:** a planar arc parallel to the side face, curving back toward the palm. Its bend radius is at least 3 × the feather's width. There is no twist.
     - **Sections:** five, at rachis stations t = 0, 0.15, 0.5, 0.85 and 1.0.
       - Each lies on a `Workplane { origin, x, y, on_face: None }` whose normal is the rachis tangent at t, with `y` = +z (out of the face).
       - Each is a cambered lens of **four** `Geometry::Bezier` curves, with the same count and winding in every section and the start point always at the leading edge. The upper surface bulges 0.25 of the thickness outward, the underside is flatter, and a 0.1 mm rachis ridge runs on top.
       - Half-widths run from 0.5 mm at the root (the quill), to full width at 0.15, 0.9 w at 0.5, 0.6 w at 0.85, and a rounded 0.45 at the tip.
       - Thickness is at least 0.8 mm everywhere. That is the wax section floor, and the author asserts it, since the verdict does not measure free parts.
     - **Root:** sunk 0.4 mm into the side face (z 3.2).
     - **Overlap:** each feather overlaps its neighbours by 30–40% of its width. Successive tiers step 0.3 mm in z, so no two feather faces coincide.
     - **Component:** `Component { attach: Attach::Join, stage: Stage::Cast, blend_mm: 0.15, placement: Placement::Free, .. }` on every feather.
  5. **Mirror:** F26 `Operation::Pattern { sources: pattern::Sources((6..=25).collect()), kind: PatternKind::Mirror { plane: MirrorPlane::Band } }`, `Join`, `blend_mm 0.15`. The sources stay parts beside the copy (`cad.rs:124-130`).
  6. **Layer "Back feathers"**: the "Back feathers" SVG, a gradient contour-feather tile, in a `TilingLayer` over the crown only (a `VGate::Band` over the crown's v span), `window(165, 120)`, 0.4 mm.
  7. **Layer "Tail fan"**: the "Tail fan" SVG as one `DecalLayer` decal at θ 250 on the crown, 0.5 mm.
  8. **Layer "Scaled tarsus"**: `Procedural::ReptileShields` tiling, `window(30, 90)`, 0.3 mm, under the forward arch.
  9. Run the gates, plus a per-part `self_crossings` loop over all 46 made parts (Arachne's loop). Record the preview build time.

- **What it shows off:**
  - CAD lofting at its fullest: forty cambered lofts, a multi-source mirror, joins with seam beads, the cathedral builder, a through bur;
  - talon claws;
  - a node-only template, like Kraken.

- **Traps and how they are avoided:**
  - **Loft sections must be compatible.** 2–32 sections with matching curve counts and winding (`core/cad.rs:1902-1920`). All-polyline sections go to cadkernel's ruled `polygon_loft`; curved ones go to its NURBS `curved_loft` (cadkernel `src/brep/loft.rs:14`). A section with a hole is refused.
  - **A loft folds** where its sections tilt more than they stand apart, the same rule as a tube. Keep station spacing greater than section thickness and the rachis bend ≥ 3 × width.
  - **Coincident faces** between feathers, or between a root and the face, give degenerate booleans. Hence the 0.4 mm root sink and the 0.3 mm tier step. `csg` retries with a nudge, but avoid relying on it.
  - **Wing root against the talons and arches.** The root starts at θ 104, the talons stand within ±6° and the arches at ±34°. Check the section pane at θ 104 and 124 for ≥ 0.5 mm clearance.
  - **Build time.** Forty lofts plus the mirror join through `csg`. Arachne's 62 parts build fine at export, but measure the preview. If it is slow, cut the primaries to 7.
  - **No face.** She is read from wings, talons, tail and legs only.
  - **Free parts do not follow a resize.** `Placement::Ring` follows a resize and `Free` does not (`core/cad.rs:293`). A size change would leave the feathers off the side faces, so the template does **not** expose US size. The brief's `Placement::Face` enabler, which would fix this, is deferred.

- **Needs:** P6 (critical for the talons); P2 (landed: multi-source `Pattern`, CAD stones in the report); C-B3 `FeatherSpec` / `feather_loft`; the SVG art "Back feathers" and "Tail fan".

- **Template:**
  - profile node (Uniform), `alpha.svg` × 2, `alpha.proc` × 1;
  - a `cad.feature` chain of 26 features, with each loft's five section sketches inline;
  - about 150–250 kB, carrying 3 `/cad/*` and 3 `/draft/*` patches as Arachne's does, so it waits on P7.
  - Expose the stone size and the talon wire. Band width is safe only if the feathers are re-seated, so leave it out.

- **Risk:** 4.5/5, high.
  - If the Bézier lofts fail or look lumpy, use 24-point closed polylines with 7 sections (ruled).
  - If lofting fails altogether, each feather becomes an `Operation::Twist` of one cambered lens along its rachis with `end_scale 0.45` (tapered, uniform camber).
  - If the wing still reads flat in round 1, Harpyia is the ring dropped.

---

## Packaging (Reptilia format)

This is the lead's job, after the drop and rounds 2–3 (plan batch 3A).

**Per ring**, under `showcase/bestiarium/<slug>/`:
- `design.ring.json` and `editable-graph.ring.json`;
- `artwork/`, holding each painted PNG and each SVG as text;
- `finished-metal.stl`;
- the pattern:
  - sand: `casting-pattern.stl`, shrink-compensated, with no cuts or heads and with drill dots (`mesh::try_build_pattern`);
  - wax: the investment pattern;
- `reference-<stone>.stl`, marked do-not-cast;
- `report.json`, `mesh.json`, and `release-fine.json` for sand rings;
- `verification.json`: clamp bite, drag, template bytes, `design.set` patch count and pointers, open milliseconds from `graph/examples/template_open_probe.rs`, and cold design and graph reloads;
- `hero.png`, `face.png`, `palm.png`, Blender `studio*.png` in studio gold, and the reel.

**Collection level:**
- `showcase/bestiarium/README.md`: the table of the eight (form, surface, stone, approximate cast weight), what each folder holds, the verification summary, bench notes, and the reproduce commands under the memory guard;
- `index.html`;
- `Bestiarium-collection.png`, with the sand row and the wax row;
- `Bestiarium-final-renders.zip`.

**Reproduce commands for the README:**

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example bestiarium_<slug> -- showcase/bestiarium/<slug> [--draft]
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example bestiarium_<slug> -- showcase/bestiarium/<slug> --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example collection_templates -- bestiarium   # P8; until then a bestiarium_templates.rs copied from reptilia_templates.rs
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- showcase/bestiarium                                       # P8; until then render_bestiarium.py from render_reptilia.py
python3 tools/catalog_collection.py showcase/bestiarium                                                                                        # P8; until then from catalog_reptilia.py
```

**Blender renders.** Studio gold, stones set, from the exported finished meshes. There is no displacement and no geometry edit (the `render_reptilia.py` rule). P8 adds a stone material manifest, so the onyx, opal, moonstone, labradorite, ruby, spinel, sapphire, tsavorite and garnet each render as themselves.

**Graph templates:**
- Write `graphs/templates/<slug>-bestiarium.graph.json`. It is bundled automatically, because the directory is the family (`ringdesign-assets/build.rs:29`).
- Add a `pub static BESTIARIUM: &[TemplateGraph]` in `graph/templates.rs` beside `REPTILIA` (`:163`) and chain it into `catalog()` (`:170`).
- Budgets per plan §5, after P7: procedural ≤ 300 KB, stock ≤ 1 MB, and painted-atlas rings as measured, flagged above 3 MB. At most 4 `design.set` patches. Each template must evaluate cold to the design byte for byte, with an identical mesh.

**Menu:**
- In `wb/templates.rs`, placed after the starters, add `group("Bestiarium", &[<the eight slugs>], "Creatures told by hide, plumage and weapons, face to palm.")`.
- Add the eight previews to `preview_bytes` (`wb/templates.rs:501`).
- Make 160 px thumbnails from `render::finished` renders through `tools/template_thumbnails.py`, adding each source to `crates/ringdesign-workbench/assets/templates/sources.json`.
- Update the preview test count.

**Reel.**
- Name stamp families for `stamp_family` (`crates/ringdesigner-android/src/reel.rs:76`); "Comb lobe, 3" plays as "Comb lobe". Layer names are the captions.
- Record from the rdsmoke AVD with the screenrecord recipe.

**Phone.**
- Bump `crates/ringdesigner-android/Cargo.toml`'s version and add the `CHANGELOG.md` entry.
- Run `cargo test -p ringdesigner_android` and the `cargo ndk -t arm64-v8a check`.
- Open each template on the rdsmoke AVD.

**Sheet subtitle.** Recompute it from the eight that ship: beasts, sand count, wax count, stones. If the dropped ring is a wax ring, it reads "EIGHT BEASTS · THREE IN SAND · FIVE IN WAX · …".
