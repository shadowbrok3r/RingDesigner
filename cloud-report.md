# core-fold-errors-name-the-point — report

- **Branch:** `claude/core-fold-errors-name-the-point` (from master)
- **Commit:** `6f4d184` (the change); this report is committed on top of it.

## What changed
- `blend::station_at(p)` (new): names a point in the ring's frame as
  `"85.0° round the ring, r 10.65 z -0.08 mm in its section, point (0.93, 10.61, -0.08) mm"` —
  ring angle (atan2(y, x), 0–360°), section position (radius from the axis, height along the finger), and the point in mm.
- `blend.rs`: the "the bead folds at N of M stations at its smallest radius" error now ends with
  `, first at <station_at>` for the first crossing station.
- `blend::Bead` gains `min_at: P3`, the seam point of the station laid at the smallest radius.
- `parts.rs`: the "its fillet pinches to X mm at N of M stations" note now ends with `, smallest at <station_at>`.
- `CLAUDE.md`: two lines under `Component.attach / stage / blend_mm` stating the rule.
- These were the only producers of the two messages (searched the crates for `folds at` and `pinch`; the
  other hits are comments, touch gestures, or tests). Messages only — no geometry or format change,
  so no format fencing is needed.

## Tests
- New `blend::tests::a_fold_names_the_station_it_is_found_at`: exact `station_at` output, plus a figure-eight
  seam that crosses itself at the top of a 10 mm ring; the fold error must name ~90°, r 10.0, z 0.00.
- `a_wire_lying_on_the_court_band_clamps_the_bead_where_the_wedge_closes` now asserts `min_at` lies near 90°, |z| < 1.
- `parts::tests::a_wire_lying_on_the_dome_beads_or_says_it_pinches_and_the_build_stands` now requires the
  pinch note to name the angle (within 15° of 90°) and the section/point. Observed:
  `wire: its fillet pinches to 0.02 mm at 55 of 86 stations, smallest at 85.0° round the ring, r 10.65 z -0.08 mm in its section, point (0.93, 10.61, -0.08) mm`.
- The new/strengthened tests do not compile or pass without the change.
- `cargo test -p ringdesign-core -- --test-threads=4`: lib 789 passed, 0 failed, 16 ignored; golden 1 passed; doc-tests 0.
- Graph crate not touched, so its tests were not run.

## Push and PR
- Pushed to `origin/claude/core-fold-errors-name-the-point`; PR: https://github.com/shadowbrok3r/RingDesigner/pull/225
  (the first push attempts were refused by the git proxy until the repository was added to the session with push access).

## Not done
- Nothing outstanding. `Junction` (the `fillet_junction_report` summary) was left without a location, since it produces no message.
