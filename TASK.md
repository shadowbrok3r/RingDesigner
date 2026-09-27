# Platform change: core-seam-bead-radius

Read this whole file first: the checkout below replaces it on disk.

## Setup
This repository is a small seed. Work inside it, in `/home/user/repo`, which is the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/core-seam-bead-radius origin/master`. Then read the parts of `CLAUDE.md` that bear on your change (it is long: search it rather than reading all of it).

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

## Rules
Logan (the owner) approved this change knowing it moves the geometry of existing designs. Keep it to exactly what the request says. Run the core and graph tests. Where a pinned expectation moves because of this change (a golden hash, a measured figure, a template's mesh), update it and name it; never loosen a test for any other reason. A new saved field is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers when non-default, never a new version. Update CLAUDE.md where the doctrine it states changes, in its own voice and briefly.

In the pull request description, list every bundled template, showcase design and shipped Bestiarium ring (Arachne, Manticora) whose built geometry changes, with the measurable before and after (volume, a gate figure, the obstruction count). Say plainly if none change.

When done: commit, `git push origin HEAD:claude/core-seam-bead-radius`, open a pull request against master if your tools allow it, and write your final report to `cloud-report.md` at the repository root in a last commit pushed to the same branch. Your final message is that report.

## The change
23. **Seam bead radius from every part on the seam**: `parts.rs` `Chain::beads` takes a seam's radius
    from whichever part owns its first face, so a seam between a filleted and an unfilleted part gets
    a bead or none by face order. Take the largest `blend_mm` among the parts on the seam (code in
    Manticora's round 3 report).

The requester's code was in a report that is not available; implement it from the description: a seam's bead radius is the largest `blend_mm` among the parts that meet on it, whatever the face order. Pin it with a test where a filleted and an unfilleted part meet, in both orders.