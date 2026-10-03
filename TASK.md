# Tenebrae concept spike: Ogiva

Ogiva (`ogiva`) stopped at the block-out: three read tests failed, and its report says the concept, not the detail, has to change. Logan decides how it is rethought. Your job is to give him evidence to decide with: block out each rethink option below, have each one read-tested by an independent reviewer, and report which read. You build no detail, run no review rounds, and choose nothing for him. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed, or read them back later with `git show origin/cloud/spike-ogiva:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/tenebrae-ogiva cloud/spike-ogiva`, `git checkout -b claude/tenebrae-ogiva-spike origin/master`. Read the old attempt with `git show origin/claude/tenebrae-ogiva:cloud-report.md` and its read tests in `showcase/tenebrae/ogiva/` on that branch. Copy the seed's reviewer files: `mkdir -p target && git archive origin/cloud/spike-ogiva calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB.

## The spike
For each option below, build a block-out: the base and the primary forms at their final scale and position, plus the stones if the option carries any. Use one example file, `crates/ringdesign-core/examples/tenebrae_ogiva_spike.rs`, with the option as an argument. Render at draft resolution to `showcase/tenebrae/ogiva-spike/<option>/`: `hero.png`, `face.png`, `side.png`, `hero-300.png` and `face-300.png`. Then start a fresh reviewer agent for that option (the Agent tool), given only `target/review.md`, the ring's name and slug, "read-test mode", and the option's render paths. Never pass it your notes or the option's intended reading. Write its verdict to `showcase/tenebrae/ogiva-spike/<option>/read-test.json`.

You get one revision per option: if the first read fails, apply its changes once and read-test again with another fresh reviewer. Record which process each option could pour in (Delft sand or lost wax), and its main gate risk, from a quick draft check (field verdict for sand, `cad::measure::thickness` at 0.8 mm for wax). Commit and push after each option with `git push origin HEAD:claude/tenebrae-ogiva-spike`.

The options:
1. `arch`: an arch ring. A pointed-arch outline is cut and extruded along the pull as the face, so the arch stands in the outline the cameras see. Keep it distinct from Porta, the portal on factory 009: a single great arch with its keel and mouldings, not a doorway.
2. `keel-section`: the original keel, with the section view as its subject. Re-render it with `section.png` and an end-on `side.png` as the views to be read, and give the reviewer those two plus the hero and face. This tests whether the keel can be judged on its section, which is Logan's call.
3. `gargoyle`: another subject in the sand slot. Faces are now allowed, so try a gargoyle head on a factory sand-master signet (one of 001, 002, 005, 006, 012, 013, 015, 017), crouched and carved along the pull, poured in Delft. If no sand gate can hold it, say so and pour it in wax.

## Finish
Write `cloud-report.md` at the repository root with a table: option, what it is in one sentence, process, read test 1, read test 2, what the eye saw, and the main risk. Embed the paths of the 300 px renders. End with what you would build if it were your call, and why, marked plainly as a recommendation for Logan. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.
