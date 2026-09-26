# Platform change: core-graded-centre-phase

This small repository only carries this task. Setup: `git clone https://github.com/shadowbrok3r/RingDesigner.git /home/user/rd`, then `cd /home/user/rd` (master) and work there.

You are implementing one platform change in RingDesigner (this repository; its CLAUDE.md is the design doctrine) that ring authors asked for while building the Bestiarium collection. Work on a new branch `claude/core-graded-centre-phase` from master.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

Rules: keep the change minimal. Opt-in behaviour stays bit-identical at its default: every existing design, template and example must build byte for byte as before (the golden and template tests prove it). A non-default value that an older build would misread is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers, never a new version. Pin the new behaviour with a test that fails without it. Run the core tests (and the graph tests if you touch the graph crate) before pushing. Update CLAUDE.md only where the doctrine it states changes, in its own voice and briefly.

When done: commit, push the branch, and open a pull request against master if your tools allow it. Your final message: the branch, the commit, what changed, the test results, and anything you could not do.

The change (from `.claude/collection-review/core-change-requests.md`, which is not in the repository; its text follows):

1. **Mirror-true graded runs.** `SeatRunLayer` anchors station 0 at 0 deg, so a graded run is not
   symmetric about its taper centre. Proposed `centre_phase: Option<f64>` (0.0 stands a station on the
   centre, 0.5 straddles it; `None` keeps today's behaviour), used in the station phase. Workaround:
   the example solves c = tan(m*pi/2n) for its own count.