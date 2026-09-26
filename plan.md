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

