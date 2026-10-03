# Cataphracta ring, second start: Ouroborus

You are the author of Ouroborus (`ouroborus`), and you run its whole review loop yourself in this one session. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed before the checkout, or read them back later with `git show origin/cloud/lane-ouroborus-2:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/cataphracta-ouroborus cloud/lane-ouroborus-2`, `git checkout -b claude/cataphracta-ouroborus-2 origin/claude/cataphracta-ouroborus`, then `git merge origin/master`. Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/lane-ouroborus-2 calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
**Step 0, the block-out.** An earlier session built three block-outs on the branch you start from; read its `cloud-report.md` and `showcase/cataphracta/ouroborus/read-test-*.json` first. Your block-out attempts are numbered 4, 5 and 6, and you get three of them. Build the ring's primary forms only: the base, the signature pattern or figure at its final scale and position, and the stone setting if the ring has one. Render at draft resolution, including `hero-300.png` and `face-300.png`. Then start a reviewer: a separate agent (the Agent tool) given only `target/review.md`, the ring's name and slug, "read-test mode", the attempt number, and the paths `review.md` lists. Never pass it your own notes or opinion. If `reads` is false, apply its changes and test again; at most three block-out attempts. If the third still does not read, stop and report: the subject needs rethinking, not detailing.

**Rounds 1 to 3.** Build the full detail. Run every gate in `brief.md` at draft and at export, and fix what fails before any review. Render, commit, and push with `git push origin HEAD:claude/cataphracta-ouroborus-2`. Then start a fresh reviewer agent (a new one every round) with `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Apply its punch list in the next round. Stop at **ship** (7.5 or more with every gate green), or after round 3, when a score under 7.5 is a **cut**.

The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Master moves tonight
The lead is landing platform work on master while you build: crisper stamp and relief edges in the renders, and the enablers C-B2 (true pear, heart, trillion and half-moon plans), C-V1 to C-V5 and C-T5 to C-T7. At the start of every round after the block-out, run `git fetch origin master` and `git merge origin/master`, then rebuild, re-run every gate and re-render. Use an enabler as soon as it reaches master, and say in the report which ones you used. If a merge breaks your example, fix your example; never edit `src/`.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include every read test and review verdict and score, and the number of rounds used. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/cataphracta.md` is "Ouroborus". Petrobond, as Logan confirmed. **The subject is the lizard the ring is named for: *Ouroborus cataphractus*, the armadillo girdled lizard, which takes its own tail in its mouth when threatened.** The earlier session was briefed wrongly, as a serpent, and its third block-out nearly read as a 'snake ring'. A snake ring would repeat Serpentarium's Ouroboros, the snake Logan loved. This one must read as a lizard biting its tail: four short legs tucked against the body (forelegs behind the head, hind legs at the hips), a broad, flat, triangular head armoured with spiny occipital plates, and the body and tail ringed with spiny girdles. Faces are allowed now (Logan's later decision overrides the section's 'plates only'), so give it eyes if they help it read. Apply the third read test's changes: girdles with rounded, overlapping trailing edges graded nape to tail (no square-topped castellations, which read as a cog), a real gape closing on a tapered tail tip, and a head about 1.4 times as long as wide. The template gate failed on size (3.89 MB against the 3 MB painted budget); bring it under budget. Block-out: the lizard curled into a ring, biting its tail. It must read 'lizard biting its tail' at 300 px.
