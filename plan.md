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

