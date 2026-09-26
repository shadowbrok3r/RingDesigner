# core-dfm-texels-hide-lands — report

Branch: `claude/core-dfm-texels-hide-lands` (from master). Only `crates/ringdesign-core/src/dfm.rs` and `CLAUDE.md` changed. No saved field is added, so nothing needs fencing at design format 6 / graph format 2, and design, template and example bytes are unchanged. `fit_to_floor` was left as it was because regalia and patternbands build from it.

## What changed
- **#2 DFM on non-square texels.** When a tiling's texels are more than 5% from square, `tiling_finest_mm_at` now stretches the mask before granulometry: it repeats the coarser axis nearest-texel out to the finer pitch (no side goes past 4096). Within 5% of square the old path runs unchanged. I did not use the proposed box-average down to the coarse pitch: in testing it squashed an 8 px bar to 2 texels and read it 25% too wide.
- **#13 DFM for a hide layer.** A tiling with one repeat around the ring (a hide) is now judged at the tightest station where it stands at least half its height, checked within ±half a station. Before, it was judged across the whole window. If every inked station is thicker than the reference, it is judged at that thicker station, not at the reference.
- **#26 `dfm::part_sections(solid, up, floor_mm) -> (min_mm, under_floor_mm2)`.** Each face casts one ray along its inward normal, through the existing BVH, to where it leaves the metal. The function returns the thinnest section and the face area whose section is under the floor. With `up` (the part's axis), faces turned more than 45° toward its ends are skipped. **The signature differs from the request:** it takes `floor_mm`, because the area under the floor needs a floor.
- `CLAUDE.md`: a short addition to the DFM section, and Braid's template figure updated.

## Behaviour changes
- **Template findings:** of the shipped templates, only Braid's figure moves: 0.04 → 0.10 mm, still flagged against the 0.35 mm floor. Chevron is unchanged.
- **Solver no longer tight on tall texels:** `fitting_to_the_floor_silences_the_finding` said one repeat past the solve must flag. On Chevron's texels (1.38× taller than wide) the solve is 16 and findings start at 24. The test now asserts that the solve silences the finding, that findings return past it, and that the texels are non-square.

## Tests
New tests, each failing without its change:
- `a_mask_on_tall_texels_reads_each_axis_at_its_own_pitch`
- `a_hide_is_judged_where_it_carries_ink`
- `a_part_reads_its_sections_by_rays`: a 0.8 mm claw tip reads 0.8, a 0.8 mm collet wall reads 0.8, and a cone point shows as area under the floor.

`cargo test -p ringdesign-core -- --test-threads=4`: 791 passed, 0 failed, 16 ignored; integration test 1 passed; doc-tests 0. The graph crate is untouched, so its tests were not run.

## Not done
- The Bestiarium examples have no ray-based land check today: Manticora and Arachne write their lands from the design constants (`COLLET_WALL_MM`, `CLAW_MM`, the sweep radii). So `part_sections` is matched to those numbers on equivalent lathe solids in the test. It is not wired into the examples, and the examples were not re-run.
