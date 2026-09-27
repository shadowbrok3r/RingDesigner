# Cloud report: Cataphracta enablers C-R2, C-R4, C-R7, C-R8

Branch `claude/cataphracta-enablers-b`, on master `590921c`. Three commits hold the enablers: C-R2 and C-R4 share one because they touch the same fields and fences, then C-R8, then C-R7. This report is the last commit. None of the work touches `GroupLayer`, the seat-run layer, `castability.rs` or `dfm.rs`.

Every enabler is opt-in. Each new field is skipped when it holds its default, and when it does not, it fences the design at format 6 (`library::format_version_for`) and a graph at format 2 (`template_features_in_json`, checked by literal, by wire and by exposure). No version number was added. The golden and template suites pass unchanged (see Tests).

## C-R2: graded tilings

**What landed**
- `TilingLayer::grade: Option<TileGrade { taper, theta_deg, law, isotropic }>`, with `GradeLaw::{Cosine, Spiral { seam_deg }}` and `MAX_GRADE_TAPER` = 0.9.
- The lattice index runs on φ(u), a monotone map of the circle onto itself with φ(x+1) = φ(x)+1, so the integer count still closes.
- **Cosine** is the graded seat run's `eccentric_warp` (now `pub(crate)`) with c = √(1−taper). Its pitch is a raised cosine, largest at `theta_deg` and `1 − taper` of that opposite. The pitch ratio is exactly `SeatRunLayer::scale_at`'s law.
- **Spiral** shrinks the pitch geometrically from `seam_deg`, largest just after the seam and smallest just before it. It has closed-form forward and inverse maps and one kink, at the seam.
- `isotropic` scales `v` about `v_center_mm` by the local pitch over the largest, so the band narrows with the pitch.
- `cells()` draws graded cells in the unrolled editor.
- `TileGrade::{phi, x_of_phi, ratio, finest_over_nominal}` and `TilingLayer::finest_cell_size` give the smallest cell anywhere on the ring.
- The grade is exposed as the `grade` pin on `layer.tiling` (sparse) and as a "Grade" row in the GUI layer panel.

**Tests**
- `tiling::tests::a_grade_of_taper_zero_lays_every_cell_bit_for_bit`: heights and cells are compared by bit pattern, and `None` is not serialized.
- `a_graded_tiling_closes_and_its_seam_steps_like_its_interior`: run on both laws. The count closes, 24 resets fall round the ring, and the step at the seam is no larger than the steps inside it.
- `a_grade_runs_the_pitch_from_its_large_pole_to_its_small_one`: the Cosine pole ratio is 1/(1−t); the Spiral's cells fall monotonically from the seam; the cells cover the circumference; the grade is mirror-true about the large pole.
- `an_isotropic_grade_narrows_the_band_with_the_pitch`.
- Fencing: `library::…::a_graded_tiling_fences_the_design_and_a_graph_carrying_it` covers a tiling on its own, inside openwork and inside a group, plus the round trip. `graph::file::…::a_tiling_grade_fences_the_graph_by_literal_wire_or_exposure` covers the graph side.

**Not done**
- **The DFM part.** The plan asks for `tiling_finest_mm_at` × (1 − taper) at the small pole, and that function is in `dfm.rs`, which the other session owns. So DFM still measures the ungraded cell.
  - The fix is one line: use `t.finest_cell_size(ctx)` in place of `t.cell_size(ctx)` at the top of `tiling_finest_mm_at`.
  - The correct factor is √(1−taper) for Cosine and taper / −ln(1−taper) for Spiral, not the plan's (1 − taper). The isotropic `v` shrinks by (1 − taper).
- **Where the Spiral lattice starts.** The lattice keeps phase with the ungraded one (φ(anchor) = anchor), so a cell edge lands on the seam only when `seam_deg · repeats / 360` is a whole number. This is documented. For Ouroborus, `seam_deg` 90 lands on an edge for any count divisible by 4.

## C-R4: hide-space tilings and region masks

**What landed**
- `TilingLayer::space: ChartSpace { Chart (default), Hide }`.
- `FieldContext::hide() -> Option<&HideChart>`, found on first use and held in `FieldContext::hide_cache`. `FieldContext::hide_uv(uv)` maps a chart point into hide space:
  - `u` is `circ/4 + along`, where `along` is measured along the crest from the head's centre at 90°.
  - `v` is `across`, which is 0 on the parting line.
  - `mirror_v` in hide space mirrors across the parting line.
- **Procedural band:** the analytic chart the plan specifies. `along` integrates `crest_scale` per degree, and `across` is (v − crest_v) · `station_stretch`.
- **Stock:** `skin::Hide::of` over the new `skin::Atlas::of_surface`, built on the stock's field surface (720 × 384). It is cached per `Arc<FieldSurface>`, which is itself cached per design signature. `Atlas::of` now shares that path, and its output is unchanged.
- **Region masks:** the built-ins are `"##region:table|rim|cheek|wall|shoulder|palm"`, defined in `skin::region`. The first three are `Atlas::face`, `cheek` and `shoulder`. The rest are drawn from the hide:
  - `rim` is the outer surface out to where each side turns to face the pull;
  - `wall` is the surface past the rim;
  - `palm` is where the surface faces within about 40° of straight down.
- The masks are painted on a 1024 × 256 atlas by `RingDesign::bake_regions`. The distance-field bake runs it, so `bake_all`, `bake_sdfs` and the observed bake all insert them. They are shared per band through `shared_bake`, keyed on the profile, the shank, the base fingerprint and chart, and the bore, and they are never embedded (`embed_alphas` skips them).
- The GUI rebakes the masks on edit, which costs one hash when the band has not moved.
- `space` is exposed as a select pin on `layer.tiling` (sparse) and in the GUI panel. Switching space keeps the band where it was by shifting `v_center_mm` by `crest_v`. The `entry` node's mask pin documents the region names.

**Tests** (in `skin::tests`)
- `hide_space_on_a_plain_band_is_the_chart_moved_to_the_parting_line`: agrees to 1e-9, with and without `mirror_v`.
- `hide_space_runs_true_millimetres_on_a_keyframed_band`: `along` is monotone round the ring and agrees with the atlas's own `Hide` within 2%. The chart runs short on the head.
- `hide_space_on_stock_starts_at_the_head_on_the_parting_line`: run on the 013 master; there is one chart per surface.
- `region_masks_bake_from_the_band_and_are_never_saved`: the masks mask the layer, are shared per band, rebake when the band changes, are never embedded, fence the design, and every region bakes.
- Graph fences for `space: Hide` and a `##region:` mask.

**Not done**
- **The back of the ring.** Hide space runs from the other shoulder past 270°. A hide tiling therefore closes there only where the crest arc totals the chart's (on a plain band it does). This is documented in CLAUDE.md: keep hide tilings on the head, or window them.
- **The layout editor.** `cells()` and `feature_footprints` still draw in chart space for a hide tiling. The editor's cell overlay is approximate in hide space.

## C-R7: reptile SVG generators

**What landed**
- `reptile::svg`: all 19 generators the plan names, each `fn(&Params) -> String`: sail, tubercle_rows, paver, granules, bead_lattice, reticulation, rosette_thorn, lamella, granule_spots, whorl, whorl_spine, plate_voronoi, scute, shingle, plastron, fringe, rosette_tubercle, granule_voronoi and flat_tubercle.
- `Params { pitch_mm, height_mm, land_mm, dome, seed, focus }`.
- Each generator draws one period per tile in mm, drawing wrap copies of any feature that reaches over an edge.
- `GENERATORS` lists each generator with the tile it is tested at and where that tile comes from.
- **Script twin:** `reptile_svg(name, pitch, height, land, dome)`, or `reptile_svg(name, #{…})` (which also takes seed and focus), and `reptile_skins()`. These are registered in `ringdesign-script`, so a `script` node can feed `alpha.svg` with pitch, land and dome exposed.

**Tests**
- `reptile::tests::every_reptile_skin_holds_the_detail_floor_at_its_tightest_station`:
  - each generator is rasterized as a design bakes it (`SvgAlpha::rasterize`);
  - `min_feature_px` ink and gaps are asserted ≥ 0.40 mm at the tile's mm per texel;
  - the seam steps are checked against the interior steps, round the ring and across.
- All 19 pass. The tightest measured values are:
  - ink: rosette_thorn and rosette_tubercle at 0.407 mm, then sail at 0.469;
  - gaps: plastron at 0.401 mm.
- `a_skin_answers_its_land`.
- `ringdesign-script`: `a_script_twins_every_reptile_skin`.

**Deviations and flags**
- **Generators live in `core/reptile.rs`, not in the ring modules.** No ring module exists yet. Move each one when its ring lands if you prefer the plan's "ring module first" rule.
- **Tightest tiles I chose.** Sail, bead_lattice, paver, granules, lamella, whorl and the Chelonia scute pitch are derived from the doc's own numbers. The rest are my reading of each ring's text, recorded in each `Generator::station`, and should be checked against the rings as they are built.
- **Lands drawn at 0.42.** Drawn lands measure about 0.01 mm narrow through the 1024-px raster. So lamella, whorl, shingle, plate_voronoi and scute are tested with 0.42 mm lands.
- **Phrynosoma's 0.35 mm plate joints**, as its text specifies, are under the collection's own 0.40 floor.
- **rosette_thorn and rosette_tubercle need tiles of 3.0 mm or more.** Six granules of at least 0.4 mm around the centre, with 0.45 lands, will not fit in less. Moloch's flat 7.0 × 3.6 band may have a side face narrower than that at the palm, so measure it when the ring is built.
- **Spines are blunt wedges.** whorl_spine and fringe have a flat tip of 0.8 × base, not true points: at 0.6 × base the Ouroborus tail-tip spine reads 0.399 mm.
- **plate_voronoi draws every plate at full height.** Tiers are the layer's remap or a second layer.
- **paver has no crest-ribbon variant.** It does not drop its along-ring joints across the crest ribbon as Heloderma's text describes; that fusing belongs in the layer (a mask or window), or in a later parameter.

## C-R8: `plan_mask`

**What landed**
- `imported_base::plan_mask(id, w, h) -> Option<Alpha>`. It fills the preset's 48 polar radii as a polygon with a 4 × 4 supersampled edge.
- Columns run along the head's length (bearing 0) and rows across its width. The plan's bounding box fills the raster, so the mask sits on the table of a face of `face_mm`.
- It returns `Option` rather than the plan's bare `Alpha`, so an unknown id is `None` rather than a silent all-zero mask.

**Test:** `imported_base::tests::a_plan_mask_fills_the_factory_table`. Each of the 20 presets fills its own polygon's share of the box to within 0.004. 013 fills π/4. 007's four notches and its corners are bare.

**Not done:** there is no graph node for it. The plan asks only for the function.

## Tests run

Rust was updated to 1.98.1, because egui 0.36 requires 1.95. Everything ran on this branch's head:

- `cargo test -p ringdesign-core -- --test-threads=4`: 825 passed, 0 failed (golden corpus and template tests included)
- `cargo test -p ringdesign-graph -- --test-threads=4`: 129 passed, 0 failed (struct coverage and showcase templates included)
- `cargo test -p ringdesign-script`: all pass.
- `cargo check --tests` on `ringdesign-gui` and `ringdesign-workbench`: clean. `ringdesigner-android` is not a workspace member and was not built

CLAUDE.md has a note for each enabler, in its own voice:
- C-R2 under "Seamlessness";
- C-R4 under "The skin is core";
- C-R7 under per-layer DFM;
- C-R8 under the stock presets.
