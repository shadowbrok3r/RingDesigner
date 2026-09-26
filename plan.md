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

