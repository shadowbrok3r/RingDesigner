# Cloud report: core-stamp-fine-cap

Opt-in fine cap pitch is done and pushed to `claude/core-stamp-fine-cap`, PR #233.

## What changed
- `Stamp::fine_cap` is a new saved field. Serde defaults it to off and skips it when it is off. When on, the cap grid pitch is `(reach / 28.0).clamp(0.1, 0.2)`; when off, it is today's `(reach / 14.0).clamp(0.12, 0.35)`. The choice is made in `Stamp::cap_pitch` in `setting.rs`.
- **Design fence:** `is_plain` now requires `!fine_cap`, so `library::format_version_for` writes design format 6, the same way it does for tier and top.
- **Graph fence:** `template_features_in_json` treats `"fine_cap": true` as fenced, so graph writers and presets write graph format 2.
- **Exposure:** the graph `stamp` node has a sparse `fine_cap` pin, next to tier and top. The GUI Node inspector and the MCP graph tools read node pins, so both get it. The workbench's own small stamp window does not show tier or top, so I left it alone.
- **CLAUDE.md:** the stamp section has a fine-cap bullet, the format-5 paragraph lists `fine_cap`, and the graph-fence sentence covers it.

## Geometry changes
None. No bundled template, showcase design, Arachne or Manticora sets `fine_cap`. With it off, the pitch and the frame choice are unchanged, so those builds are bit for bit what they were, including the pinned plain-stamp hashes. No pinned expectation moved.

## Tests
- New test `a_fine_cap_carries_about_three_times_the_cap_points_on_a_large_dome`: a 10 × 4 mm dome goes from 267 to 846 cap points (3.17×). The test also checks the fence and the round-trip.
- New graph fence test, and the existing stamp-node test now round-trips the `fine_cap` pin.
- `ringdesign-core`: 810 passed, 0 failed.
- `ringdesign-graph`: every suite passes.
- **Not compiled here:** the GUI, workbench and android crates. Their dependencies need a newer rustc than 1.94.1, which is what this VM has. Their only change is adding `fine_cap: false` to struct literals.

## Branches and paths
- TASK.md said to work in `/home/user/repo`. That directory does not exist, so I used the checkout at `/home/user/RingDesigner`.
- Everything was pushed to `claude/core-stamp-fine-cap`, as TASK.md said. The session's default branch, `claude/task-autonomous-work-hekjoj`, was left untouched.
