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

