# Platform change: core-collet-bearing-slope

Read this whole file first: the checkout below replaces it on disk.

## Setup
This repository is a small seed. Work inside it, in `/home/user/repo`, which is the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/core-collet-bearing-slope origin/master`. Then read the parts of `CLAUDE.md` that bear on your change (it is long: search it rather than reading all of it).

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

## Rules
Logan (the owner) approved this change knowing it moves the geometry of existing designs. Keep it to exactly what the request says. Run the core and graph tests. Where a pinned expectation moves because of this change (a golden hash, a measured figure, a template's mesh), update it and name it; never loosen a test for any other reason. A new saved field is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers when non-default, never a new version. Update CLAUDE.md where the doctrine it states changes, in its own voice and briefly.

In the pull request description, list every bundled template, showcase design and shipped Bestiarium ring (Arachne, Manticora) whose built geometry changes, with the measurable before and after (volume, a gate figure, the obstruction count). Say plainly if none change.

When done: commit, `git push origin HEAD:claude/core-collet-bearing-slope`, open a pull request against master if your tools allow it, and write your final report to `cloud-report.md` at the repository root in a last commit pushed to the same branch. Your final message is that report.

## The change
20. **Collet bearing on the pavilion's slope** (`setting.rs` `collet_named`): `let slope = match
    gem.form { GemForm::Faceted => p / plan.b.max(1e-6), GemForm::Cabochon => 0.0 };` so the
    ledge's inner edge stays out of the stone (0.021 mm, 0.06 mm^3 measured).

Pin it with a test that a faceted stone's collet ledge no longer enters the stone (the request measured 0.021 mm and 0.06 mm^3 of overlap) and that a cabochon's collet is unchanged.