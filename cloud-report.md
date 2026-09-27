# Cloud report: core-collet-bearing-slope (request 20)

The collet's bearing ledge now follows the pavilion's full slope. Before, it sat at 0.9 of the slope, so its inner edge cut into faceted stones. Core and graph tests pass. PR: https://github.com/shadowbrok3r/RingDesigner/pull/235

## The change
- `setting.rs` `collet_named`: `slope = p / plan.b.max(1e-6)` for faceted stones. It was `0.9 * p / …`. Cabochons stay at `0.0`.
- New test `a_collet_bears_on_the_pavilion_without_entering_the_stone`: across every faceted cut at 1.3–10 mm, the collet no longer enters the stone. Cabochon collets are pinned bit for bit to master.
- One exception is named in the test. A 1.3 mm trillion still grazes at its corners: 3e-4 mm³, down from 6e-4. The cause is separate: the ledge's inward offset becomes a scale, which at a triangle's corners does not inset evenly. I left it unfixed because it is outside this request.
- CLAUDE.md: one sentence added to the collet paragraph.
- No new saved field, so there is no format fence.

## Built geometry that changed
- **Manticora:** volume 1098.675 → 1098.513 mm³, 17.12 → 17.11 g. The ruby seat went from 0.021 mm / 0.060 mm³ of metal in the stone (96 vertices) to none. With-parts undercut went from 10.571 to 10.555 %. All 14 gates pass.
- **Bezel solitaire template:** volume 239.7995 → 239.6374 mm³. Thinnest wall 1.212 mm and 0 obstructions, both unchanged.
- **Halo template:** volume 226.3093 → 226.2581 mm³, undercut 18.371 → 18.364 %. 26 obstructions, unchanged.
- **Unchanged:** Arachne, Oriel (a cabochon collet), and all other showcases.

## Pinned figures updated
- Golden corpus rows for the Bezel solitaire and Halo templates.
- Manticora's `report.json` (timings kept as committed) and `stinger-section-90.svg`.
- The Bestiarium README weight.
- PNG renders were not regenerated; the change is below render resolution.

## Tests
- `ringdesign-core`: 810 passed, 16 ignored, golden passed.
- `ringdesign-graph`: all suites passed.

## Deviation from TASK.md
I pushed to the session's required branch `claude/execute-task-md-cqi2aj`, not `claude/core-collet-bearing-slope`. The work was done in `/home/user/RingDesigner`, the only checkout present. TASK.md was removed from the tree because the branch was reset to origin/master.
