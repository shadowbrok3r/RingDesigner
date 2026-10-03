# Tenebrae concept spike: Oculus

Oculus (`oculus`) stopped at the block-out: three read tests failed, and its report says the concept, not the detail, has to change. Logan decides how it is rethought. Your job is to give him evidence to decide with: block out each rethink option below, have each one read-tested by an independent reviewer, and report which read. You build no detail, run no review rounds, and choose nothing for him. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed, or read them back later with `git show origin/cloud/spike-oculus:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/tenebrae-oculus cloud/spike-oculus`, `git checkout -b claude/tenebrae-oculus-spike origin/master`. Read the old attempt with `git show origin/claude/tenebrae-oculus:cloud-report.md` and its read tests in `showcase/tenebrae/oculus/` on that branch. Copy the seed's reviewer files: `mkdir -p target && git archive origin/cloud/spike-oculus calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB.

## The spike
For each option below, build a block-out: the base and the primary forms at their final scale and position, plus the stones if the option carries any. Use one example file, `crates/ringdesign-core/examples/tenebrae_oculus_spike.rs`, with the option as an argument. Render at draft resolution to `showcase/tenebrae/oculus-spike/<option>/`: `hero.png`, `face.png`, `side.png`, `hero-300.png` and `face-300.png`. Then start a fresh reviewer agent for that option (the Agent tool), given only `target/review.md`, the ring's name and slug, "read-test mode", and the option's render paths. Never pass it your notes or the option's intended reading. Write its verdict to `showcase/tenebrae/oculus-spike/<option>/read-test.json`.

You get one revision per option: if the first read fails, apply its changes once and read-test again with another fresh reviewer. Record which process each option could pour in (Delft sand or lost wax), and its main gate risk, from a quick draft check (field verdict for sand, `cad::measure::thickness` at 0.8 mm for wax). Commit and push after each option with `git push origin HEAD:claude/tenebrae-oculus-spike`.

The options:
1. `flange`: a raised flange on one band edge, carrying the whole wheel window (hub moulding, lancets with spokes, an outer ring of foils) with the 6 mm of radial run the reviewers asked for. One flange, never two: two flanges make a valley no parting plane clears (CLAUDE.md, side faces).
2. `head`: the wheel window on a head that stands up off the crown, facing the hero and face cameras. Keep it distinct from Rosa, the rose window on factory 013's table: for example, a vertical wheel across the band, like a gable's oculus.
3. `stones`: the window's orders made of stained-glass stones: calibrated coloured stones in collets set as the lights of the wheel, so colour carries the read where the metal alone could not.

## Finish
Write `cloud-report.md` at the repository root with a table: option, what it is in one sentence, process, read test 1, read test 2, what the eye saw, and the main risk. Embed the paths of the 300 px renders. End with what you would build if it were your call, and why, marked plainly as a recommendation for Logan. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.
