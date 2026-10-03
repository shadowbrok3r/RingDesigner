# Tenebrae enablers: C-T1, C-T3 and C-T4

Read this whole file first: the checkout below replaces it on disk.

## Setup
This repository is a small seed. Work inside it, in `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/tenebrae-enablers origin/master`. The repository's CLAUDE.md is the design doctrine: search it for what bears on your change rather than reading all of it.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

## The work
Implement C-T1 (tracery from a net, and `Profile::Regions`), C-T3 (Gothic cutter shapes, the harvested outline library and the artwork set) and C-T4 (DFM land width for CAD cuts) from `docs/collections/tenebrae.md`, section "The collection's enablers": each has its own subsection with the intended code, file locations and tests. The plan was written against an older master (its line numbers have moved, and P5 to P8 have landed since), so adapt it to master as it is now. Priority: C-T1, then C-T3, then C-T4. Two ring sessions (Oculus and Ogiva) are building at the same time in example files only; stay out of `crates/ringdesign-core/examples/tenebrae_*`.

C-T3 notes: Logan has since allowed faces, so the gargoyle may carry a face if it reads well (keep a silhouette-only variant too); `assets/User/Profiles/` is in the repository, harvest from it as the section says. New bundled assets go through `crates/ringdesign-assets` like every other family.

Rules: every enabler is opt-in, so every existing design, template, starter and showcase ring builds bit for bit as before (the golden and template tests prove it). A new saved field or variant is skipped when default and fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers when used, never a new version. Expose new operations wherever their siblings are exposed (graph nodes, MCP, the GUI's cutter shape list); the coverage tests will name any you miss. Pin each enabler with a test that fails without it. Run the core, graph and assets tests before pushing, and the wasm check (`cargo check --no-default-features --target wasm32-unknown-unknown -p ringdesign-core`). Add a short note to CLAUDE.md where it states the doctrine the enabler touches, in its own voice.

When done: commit (one commit per enabler is welcome), `git push origin HEAD:claude/tenebrae-enablers`, open a pull request against master if your tools allow it, and write your final report to `cloud-report.md` at the repository root in a last commit pushed to the same branch: per enabler, what landed, its test, and anything you could not do. Your final message is that report.
