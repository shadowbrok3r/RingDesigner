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

