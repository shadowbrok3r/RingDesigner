# Cataphracta ring: Heloderma — the beaded one

You are the author of Heloderma — the beaded one (`heloderma`), and you run its whole review loop yourself in this one session. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed before the checkout, or read them back later with `git show origin/cloud/lane-heloderma:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master cloud/lane-heloderma`, `git fetch origin claude/cataphracta-heloderma`, `git checkout -b claude/cataphracta-heloderma origin/claude/cataphracta-heloderma`, `git merge --no-edit origin/master`. Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/lane-heloderma calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
**Step 0, the block-out.** Build the ring's primary forms only: the base, the signature pattern or figure at its final scale and position, and the stone setting if the ring has one. Render at draft resolution, including `hero-300.png` and `face-300.png`. Then start a reviewer: a separate agent (the Agent tool) given only `target/review.md`, the ring's name and slug, "read-test mode", the attempt number, and the paths `review.md` lists. Never pass it your own notes or opinion. If `reads` is false, apply its changes and test again; at most three block-out attempts. If the third still does not read, stop and report: the subject needs rethinking, not detailing.

**Rounds 1 to 3.** Build the full detail. Run every gate in `brief.md` at draft and at export, and fix what fails before any review. Render, commit, and push with `git push origin HEAD:claude/cataphracta-heloderma`. Then start a fresh reviewer agent (a new one every round) with `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Apply its punch list in the next round. Stop at **ship** (7.5 or more with every gate green), or after round 3, when a score under 7.5 is a **cut**.

The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include every read test and review verdict and score, and the number of rounds used. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/cataphracta.md` is "Heloderma — the beaded one". **Third start, lost wax, a figure this time (Logan's standing rule: keep going).** Two sessions have stopped at the block-out: in sand, then in lost wax, every read test saw a sea-urchin shell, granulation or caviar, never a Gila monster. Read `cloud-report.md` and every `read-test-*.json` on your branch first. The lesson from the whole pilot: texture alone never names the animal. Sphenodon's reviewer saw "a reptile at once" the one time a lizard head was on the ring. So build **the animal itself**: a Gila monster's head with its blunt snout and eye, and its forelegs, gripping the crown beside the stone, its thick beaded body and fat tail wrapping round the band, the black-and-salmon banding carried on the body as the Gila's own skin. Sculpt the head and legs as parts or stamps with true outlines (`ringdesign_core::sculpt` is on master for sculpted parts). The ring stays lost wax: `CastProcess::LostWax.apply(&mut d.draft)`, then `min_section_mm = 0.8` and `min_draft_deg = 0`; the lost-wax gates apply (the 0.8 mm fill, a `land_widths` block via `dfm::part_sections`, every section under 0.8 mm removed or named), and the two-part undercut is a reported number, not a gate. Tell every reviewer you start that the ring is lost wax. The block-out count starts again at 1.
