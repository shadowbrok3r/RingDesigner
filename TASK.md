# Tenebrae ring, rethought: Arcus

You are the author of Arcus (`arcus`), and you run its whole review loop yourself in this one session. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed before the checkout, or read them back later with `git show origin/cloud/tenebrae-arcus-rethink:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/tenebrae-arcus cloud/tenebrae-arcus-rethink`, `git checkout -b claude/tenebrae-arcus-rethink origin/master`. Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/tenebrae-arcus-rethink calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
**Step 0, the rethink block-outs.** An earlier session built Arcus as the plan wrote it and stopped after three failed read tests; read its `cloud-report.md` and read tests with `git show origin/claude/tenebrae-arcus:<path>` first, and reuse its example (`crates/ringdesign-core/examples/tenebrae_arcus.rs` on that branch) as your starting point. Block out each option below at draft resolution, with renders under `showcase/tenebrae/arcus/<option>/` including `hero-300.png` and `face-300.png`. For each, start a fresh reviewer (the Agent tool) given only `target/review.md`, the ring's name and slug, "read-test mode", and that option's render paths; never pass it your notes or the option's intended reading. Each option gets one revision if its first read fails. Then take the option that reads (if several do, the one whose reviewer named the subject most plainly) as the ring, and record the others in the report. If none reads, stop and report.

The options:
1. `wide`: a lower, wider head (a bezel or a low basket) with the piers spread 60 to 70 degrees, so each flyer spans 6 mm or more with a deep arch, and the dark void under it is the dominant shape at 300 px.
2. `aisles`: the shoulders become the aisle roofs: raise the swell and give it sloped crests under the flyers, so the elevation reads nave, aisle and buttress.
3. `wide-aisles`: both together.
In every option, keep the Gothic pinnacles that already read in the last attempt, and use fewer, larger side-face bays (about 14 a side, 2 mm tall, under a string course) instead of 30 narrow lancets that read as reeding. Number the read tests 1, 2, 3 and so on across all options, and name the option in each JSON.

**Rounds 1 to 3.** Build the full detail. Run every gate in `brief.md` at draft and at export, and fix what fails before any review. Render, commit, and push with `git push origin HEAD:claude/tenebrae-arcus-rethink`. Then start a fresh reviewer agent (a new one every round) with `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Apply its punch list in the next round. Stop at **ship** (7.5 or more with every gate green), or after round 3, when a score under 7.5 is a **cut**.

The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Master moves tonight
The lead is landing platform work on master while you build: crisper stamp and relief edges in the renders, and the enablers C-B2 (true pear, heart, trillion and half-moon plans), C-V1 to C-V5 and C-T5 to C-T7. At the start of every round after the block-out, run `git fetch origin master` and `git merge origin/master`, then rebuild, re-run every gate and re-render. Use an enabler as soon as it reaches master, and say in the report which ones you used. If a merge breaks your example, fix your example; never edit `src/`.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include every read test and review verdict and score, and the number of rounds used. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/tenebrae.md` is "Arcus". Lost wax. Every builder exists; P6 and P7 have landed.
