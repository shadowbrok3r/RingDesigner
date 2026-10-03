# Platform change: relief-aware sections, sturdier sculpts, and figures that grow out of the band

You are implementing, in RingDesigner's core, the changes the Cataphracta ring sessions asked for most (this repository; its CLAUDE.md is the design doctrine: search it for `dfm::part_sections`, `sculpt`, `Stock`, seam beads). Tonight the revivals of Sphenodon, Moloch, Basiliscus and Fenrir, and the new figurative rings (Ouroborus, Chamaeleo, Chelonia, Phrynosoma, Gekko), all need them.

The requests, with their sources (read the reports with `git show origin/claude/cataphracta-<slug>:cloud-report.md` for moloch, sphenodon and heloderma, and the examples `crates/ringdesign-core/examples/cataphracta_<slug>.rs` on those branches, which carry working code to lift):
1. **`dfm::face_sections`** (Sphenodon): `land_widths` had to re-implement `part_sections` to attribute thin faces to features. Expose the per-face reads, and fold `part_sections` over them.
2. **A relief-aware section census** (Moloch, Sphenodon, Heloderma, all three asked): pebbled or scaled hide over a thick body is reported as a 0.000 mm section, so every figurative lost-wax ring spends its time naming false thin sections. Lift Moloch's `land_census` logic into `dfm.rs`: skip rays that leave within a small angle of the local base surface's tangent plane, or read a relief's section in the plane normal to its own ridge line. Keep the plain measure as it is; the relief-aware one is a new function or an option.
3. **A decimation that reports where it crossed** (Moloch), so a caller can repair the field rather than fall back to the raw mesh.
4. **A relax that repairs its own folds** (Moloch's round-4 fix, lifted into `sculpt.rs`).
5. **A band-blended join for stored sculpts** (Moloch's proposal, not yet coded): a stored part flagged `fillet_into_band` is united with the band in the field domain, `sculpt::smin(part, stock.at(p), blend)` over the part's box, with `stock = sculpt::Stock::of(..)`, clipped to the part's footprint, so a sculpted tail, foot or head grows out of its ground with a real fillet instead of sitting on it. The reviews keep saying figures read as decals because of that seam. Opt-in per part, closed and uncrossed (`csg::self_crossings == 0`), with a test on a sculpted part on a factory stock and on a procedural band.

Do 2 and 5 first: they move scores. Then 1, 3 and 4.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from: `git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/core-relief-sculpt origin/master`. Read sources from other branches with `git fetch origin <branch>` and `git show origin/<branch>:<path>`; never merge a ring branch.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 22 GB.

## Rules
Keep each change as small as it can be. Every existing design, template and example must build byte for byte as before (the golden and template tests prove it): a fix may turn a refusal or a failure into a result, but where a build already produced a result, that result must not move. A new option defaults off, is skipped by serde when default, and is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers (`template_features_in_json`). Pin every fix with a test that fails without it, reproducing the reported failure first. Run the core tests (and the graph tests if you touch the graph crate) before pushing. Update CLAUDE.md only where the doctrine it states changes, in its own voice and briefly. The public repository never carries CrossGems internals in commit messages.

Ring sessions are building tonight and merge master at every round, so push as soon as a fix is solid, in several commits if that lands value sooner: `git push origin HEAD:claude/core-relief-sculpt`. End every commit message with:
Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DhhBfxWFhPAaJd3cVZw2HV

## Finish
Write `cloud-report.md` at the repository root: what changed, per request (fixed, worked around, or not done and why), the tests and their results, and one paragraph a ring author needs in order to use it. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag. The lead opens the pull request.
