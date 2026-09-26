# Bestiarium ring lane: Corvus — Huginn and Muninn, round 3 of 3 (the last)

You are the ring author for Corvus — Huginn and Muninn. This small repository is only your brief; the real work happens in a full clone of RingDesigner.

## Setup
1. `git clone https://github.com/shadowbrok3r/RingDesigner.git /home/user/rd`, then `cd /home/user/rd && git checkout bestiarium-corvus && git merge --no-edit origin/master`. Resolve conflicts only in your own example or outputs; if anything else conflicts, stop and report it.
2. Read `brief.md` here (the shared lane brief), then the cloud notes below, which override it where they disagree.
3. Read `review-round2.json` (the review you answer: its punch list is this round) and `review-round1.json`.
4. The clone's CLAUDE.md is the casting doctrine; it loads when you work in the clone.

## Cloud notes (override the brief)
- You work in `/home/user/rd` on the branch `bestiarium-corvus`. There are no worktrees here: ignore the brief's context-economy rules about worktrees and CLAUDE.md copies, and its paths under `.claude/`.
- No `systemd-run` memory guard and no `--offline`: this VM has 4 vCPUs, 16 GB RAM and a 30 GB disk. Run builds and ring runs directly, and put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`. Keep `target/` under 22 GB.
- Logan's ZBrush reference sheets are private and not available. The other references are in the clone: `showcase/stock-masterworks/caiman/hero.png`, `showcase/reptilia/renders/Reptilia-collection.png`, and the shipped Bestiarium rings in `showcase/bestiarium/arachne/` and `showcase/bestiarium/manticora/`.
- Commit at every milestone that builds and passes its checks, and push every time: `git push origin HEAD:claude/bestiarium-corvus-r3`. Never push to master or any other branch, and never tag.
- You own one core change this round (enabler C-B7): request 28 in `core-request-28.md`, the faired bypass arm union. Implement it in core (opt-in; fenced at design format 6 through `library::format_version_for` when non-zero), expose it where the other shank parameters are exposed, pin it with a test, run `cargo test -p ringdesign-core -- --test-threads=4` (and the graph tests if the shank node's struct coverage needs the field), then use it on Corvus (about 4 degrees) to remove the skull fold at its source.
- Your final message is the brief's final report.

This is the last round: answer every punch item in `review-round2.json` and aim to ship at 7.5 or better.
