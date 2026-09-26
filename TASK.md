# Platform change: core-claw-foot-lift

This small repository only carries this task. Setup: `git clone https://github.com/shadowbrok3r/RingDesigner.git /home/user/rd`, then `cd /home/user/rd` (master) and work there.

You are implementing one platform change in RingDesigner (this repository; its CLAUDE.md is the design doctrine) that ring authors asked for while building the Bestiarium collection. Work on a new branch `claude/core-hide-steadied` from master.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

Rules: keep the change minimal. Opt-in behaviour stays bit-identical at its default: every existing design, template and example must build byte for byte as before (the golden and template tests prove it). A non-default value that an older build would misread is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers, never a new version. Pin the new behaviour with a test that fails without it. Run the core tests (and the graph tests if you touch the graph crate) before pushing. Update CLAUDE.md only where the doctrine it states changes, in its own voice and briefly.

When done: commit, push the branch, and open a pull request against master if your tools allow it. Your final message: the branch, the commit, what changed, the test results, and anything you could not do.

Request 29 (from the Fenrir lane, round 2): railless claws on a sloped table. Today a claw foot is refused as "stands clear" (`HeadSnag::Floats`) when the head's base starts more than 1.0 mm down in the metal, so Fenrir had to lift its moon 0.4 mm for its railless Fang claws to resolve. Proposed, in `crates/ringdesign-core/src/setting.rs` inside the claw foot search (near master line 1124), after `let reached = met.reached;`:
```rust
            // A foot whose own base already lies deeper in the metal than the scan accepts is raised up its own line
            // until it sinks FOOT_SINK_MM, instead of being called free.
            if let Some(metal) = floor(inner(own)).filter(|metal| own < metal - FOOT_SINK_MM - 0.5) {
                let lifted = (metal - FOOT_SINK_MM).min(start[1] - 0.4);
                if keeps(lifted) {
                    base_z = lifted;
                    met.reached += 1;
                    z = own - CLAW_REACH_MM;
                }
            }
```
Adapt it to the code as it is on master (names may differ). The test to go with it: a railless Fang claw head in the Jaws grouping over a floor sloping 4 degrees now resolves, with every foot sunk `FOOT_SINK_MM`. Every head that resolves today must build bit for bit as before (only feet refused as floating change), so the existing claw, starter-template and graph tests must still pass unchanged.