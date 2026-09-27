# Cataphracta enablers: C-R1, C-R3, C-R5 and C-R6

Read this whole file first: the checkout below replaces it on disk.

## Setup
This repository is a small seed. Work inside it, in `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/cataphracta-enablers-a origin/master`. The repository's CLAUDE.md is the design doctrine: search it for what bears on your change rather than reading all of it.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

## The work
Implement C-R1, C-R3, C-R5 and C-R6 from `docs/collections/cataphracta.md`, section "Collection enablers (C-R1..C-R8)": each has its own subsection with the intended code, file locations and tests. The plan was written against an older master (it calls the P5-P8 dependencies "not started"; batch 16 has since landed them, and the CAD and parts work has moved line numbers), so adapt it to master as it is now. Another session is implementing C-R2, C-R4, C-R7 and C-R8 at the same time on its own branch; stay out of `core/tiling.rs` and `core/reptile.rs`, which it owns, to keep the two merges clean.

Rules: every enabler is opt-in, so every existing design, template, starter and showcase ring builds bit for bit as before (the golden and template tests prove it). A new saved field is skipped when default and fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers when non-default, never a new version. Expose each new field wherever its layer's other fields are exposed (the graph node's struct coverage test will name any you miss). Pin each enabler with a test that fails without it. Run the core and graph tests before pushing. Add a short note to CLAUDE.md where it states the doctrine the enabler touches, in its own voice.

When done: commit (one commit per enabler is welcome), `git push origin HEAD:claude/cataphracta-enablers-a`, open a pull request against master if your tools allow it, and write your final report to `cloud-report.md` at the repository root in a last commit pushed to the same branch: per enabler, what landed, its test, and anything you could not do. Your final message is that report.
