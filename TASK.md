# Platform change: core-dfm-texels-hide-lands

This small repository only carries this task. Setup: `git clone https://github.com/shadowbrok3r/RingDesigner.git /home/user/rd`, then `cd /home/user/rd` (master) and work there.

You are implementing one platform change in RingDesigner (this repository; its CLAUDE.md is the design doctrine) that ring authors asked for while building the Bestiarium collection. Work on a new branch `claude/core-dfm-texels-hide-lands` from master.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

Rules: keep the change minimal. Opt-in behaviour stays bit-identical at its default: every existing design, template and example must build byte for byte as before (the golden and template tests prove it). A non-default value that an older build would misread is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers, never a new version. Pin the new behaviour with a test that fails without it. Run the core tests (and the graph tests if you touch the graph crate) before pushing. Update CLAUDE.md only where the doctrine it states changes, in its own voice and briefly.

When done: commit, push the branch, and open a pull request against master if your tools allow it. Your final message: the branch, the commit, what changed, the test results, and anything you could not do.

The change (from `.claude/collection-review/core-change-requests.md`, which is not in the repository; its text follows):

2. **DFM on non-square texels.** `dfm.rs` `tiling_finest_mm_at` measures with a round disc in pixels
   and converts with `scale = (cw/w).min(ch/h)`, so anisotropic masks read too thin. Proposed:
   resample the finer axis to the coarser pitch before `min_feature_px` when `(sx/sy - 1).abs() > 0.05`.
   Workaround: paint on square texels.

13. **DFM for a hide layer**: judge it at the tightest station among columns that carry ink, not the
    whole window (a table-only texture got the 50 deg station's 0.43 scale). Workaround: narrow the
    window. Needs dfm internals.

26. **Land widths in core**: `dfm::part_sections(solid, up: Option<P3>) -> (min_mm, under_floor_mm2)`
    so lost-wax lanes stop writing their own ray versions of the `land_widths` gate.

For 26, return the thinnest section and the area under the floor for one made part (claws, collets, loft parts), measured by rays through the solid, so a lost-wax ring's `land_widths` gate can call it instead of each ring writing its own. Look at how the Bestiarium examples (`crates/ringdesign-core/examples/bestiarium_*.rs`, search for land_widths) measure it today and keep their numbers.