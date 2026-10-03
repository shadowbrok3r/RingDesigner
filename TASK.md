# Tenebrae ring: Capsa

You are the author of Capsa (`capsa`), and you run its whole review loop yourself in this one session. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed before the checkout, or read them back later with `git show origin/cloud/tenebrae-capsa:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master cloud/tenebrae-capsa`, `git checkout -b claude/tenebrae-capsa origin/master`. Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/tenebrae-capsa calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
**Step 0, the block-out.** Build the ring's primary forms only: the base, the signature pattern or figure at its final scale and position, and the stone setting if the ring has one. Render at draft resolution, including `hero-300.png` and `face-300.png`. Then start a reviewer: a separate agent (the Agent tool) given only `target/review.md`, the ring's name and slug, "read-test mode", the attempt number, and the paths `review.md` lists. Never pass it your own notes or opinion. If `reads` is false, apply its changes and test again; at most three block-out attempts. If the third still does not read, stop and report: the subject needs rethinking, not detailing.

**Rounds 1 to 3.** Build the full detail. Run every gate in `brief.md` at draft and at export, and fix what fails before any review. Render, commit, and push with `git push origin HEAD:claude/tenebrae-capsa`. Then start a fresh reviewer agent (a new one every round) with `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Apply its punch list in the next round. Stop at **ship** (7.5 or more with every gate green), or after round 3, when a score under 7.5 is a **cut**.

The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Master moves tonight
The lead is landing platform work on master while you build: crisper stamp and relief edges in the renders, and the enablers C-B2 (true pear, heart, trillion and half-moon plans), C-V1 to C-V5 and C-T5 to C-T7. At the start of every round after the block-out, run `git fetch origin master` and `git merge origin/master`, then rebuild, re-run every gate and re-render. Use an enabler as soon as it reaches master, and say in the report which ones you used. If a merge breaks your example, fix your example; never edit `src/`.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include every read test and review verdict and score, and the number of rounds used. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/tenebrae.md` is "Capsa". Lost wax, two castings. Logan (2026-09-28): the relic on the floor inside the chest is a memento-mori **skull**, since faces are allowed. Model a true skull in relief, with cranium, orbits, nasal opening and a row of teeth, in place of the crossed bones under an hourglass. The lid stays a Separate cast lid on a bench hinge (`Joint`). C-T3's artwork is on master. Render one extra view, `lid-open.png`, with the lid lifted so the skull shows. Block-out: the chasse on its plinth, with the gabled lid, the blind arcades and the six cabochons. From the hero and face cameras at 300 px it must read as a Gothic reliquary casket on a ring.
