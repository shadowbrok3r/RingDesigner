# Bestiarium revival: Fenrir

You are reviving Fenrir (`fenrir`). It was cut at 7.0 after 3 reviewed rounds, close to the 7.5 ship bar, and Logan wants rings that came close to ship. You get up to two more reviewed rounds. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed, or read them back later with `git show origin/cloud/revive-fenrir:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/bestiarium-fenrir-r3 cloud/revive-fenrir`, `git checkout -b claude/bestiarium-fenrir-revival origin/claude/bestiarium-fenrir-r3`, then `git merge origin/master` (master has moved since the ring was cut; resolve any conflict in your example, never in `src/`). Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/revive-fenrir calibration previous review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
Read every earlier review in `target/previous/` (and `cloud-report.md` if the branch has one) first. The last review's punch list is your brief. Its first item names the shape that keeps the ring from reading as its species; fix that before anything else, and check it against `lessons.md`.

**Round 4.** Apply the whole punch list. Rebuild at draft and at export, run every gate in `brief.md`, and fix what fails before any review. Render, zoom your own renders to 2x and fix any stair-step, comb or seam you can see, then commit and push with `git push origin HEAD:claude/bestiarium-fenrir-revival`. Start a fresh reviewer agent (the Agent tool) given only `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Never pass it your own notes or opinion.

**Round 5**, only if round 4 did not ship: run `git fetch origin master` and merge it, apply the new punch list, run the gates, render, push, and start another fresh reviewer the same way.

Stop at **ship** (7.5 or more with every gate green), or after round 5, when a score under 7.5 is a **cut**. The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Master moves tonight
The lead is landing platform work on master while you build, including crisper stamp and relief edges in the renders. Merge `origin/master` at the start of round 5, then rebuild, re-run every gate and re-render.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include both new review verdicts and scores, beside the earlier ones. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/bestiarium.md` is "Fenrir". It is lost wax. Face-on it reads as a bulldog or a gargoyle mask (punch items 1 to 3). A wolf's muzzle is long and tapering, with the nose well ahead of the eyes and a lean jaw tied to the skull, and its upper lips are broken at the fangs, not one rolled horseshoe round the moon. Make the face name a wolf at 300 px first, then stop the ruff from combing (punch item 4). Build the head with `ringdesign_core::sculpt` so it is deterministic.
