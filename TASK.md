# Platform change: core-comfort-apex

Read this whole file first: the checkout below replaces it on disk.

## Setup
This repository is a small seed. Work inside it, in `/home/user/repo`, which is the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/core-comfort-apex origin/master`. Then read the parts of `CLAUDE.md` that bear on your change (it is long: search it rather than reading all of it).

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 20 GB.

## Rules
Logan (the owner) approved this change knowing it moves the geometry of existing designs. Keep it to exactly what the request says. Run the core and graph tests. Where a pinned expectation moves because of this change (a golden hash, a measured figure, a template's mesh), update it and name it; never loosen a test for any other reason. A new saved field is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers when non-default, never a new version. Update CLAUDE.md where the doctrine it states changes, in its own voice and briefly.

In the pull request description, list every bundled template, showcase design and shipped Bestiarium ring (Arachne, Manticora) whose built geometry changes, with the measurable before and after (volume, a gate figure, the obstruction count). Say plainly if none change.

When done: commit, `git push origin HEAD:claude/core-comfort-apex`, open a pull request against master if your tools allow it, and write your final report to `cloud-report.md` at the repository root in a last commit pushed to the same branch. Your final message is that report.

## The change
16. **Comfort fit on an off-centre section locks the bore's sand** (castability defect, sand).
    On Bypass (and Wave, Twist, upright signet heads), `sample_spaced` centres the comfort dome on
    the section's sliding mid-plane, not on the parting plane, so the bore's own drafted flanks face
    the wrong mould half: 46 ray-release obstructions up to 1.2 mm deep on Corvus with comfort 0.2.
    The field verdict skips bore samples and cannot see it. Proposed (`profile.rs` `sample_spaced`,
    the `bore_r` closure): put the dome's apex on the parting plane,
    `let apex = 0.0_f64.clamp(b_lo, b_hi);` and
    `inner_r + lift + comfort * ((z - apex) / reach.max(1e-9)).powi(2)` with `reach` the span on
    that side of the apex. Identical whenever `z_center_frac == 0`. Workaround: comfort 0. Also
    consider teaching the field verdict (or a gate) to read bore samples on off-centre sections.

Do the apex fix and pin it with a test on a bypass band with comfort fit: ray-release obstructions on the bore before and after (the request measured 46 on Corvus at comfort 0.2; after, none attributable to the bore). Teaching the field verdict to read bore samples is out of scope; say in the report whether it looks worth doing.