# Bestiarium ring lane: Harpyia — the snatcher, round 3 of 3 (the last)

You are the ring author for Harpyia — the snatcher. This small repository is only your brief; the real work happens in a full clone of RingDesigner.

## Setup
1. `git clone https://github.com/shadowbrok3r/RingDesigner.git /home/user/rd`, then `cd /home/user/rd && git checkout bestiarium-harpyia && git merge --no-edit origin/master`. Resolve conflicts only in your own example or outputs; if anything else conflicts, stop and report it.
2. Read `brief.md` here (the shared lane brief), then the cloud notes below, which override it where they disagree.
3. Read `review-round2.json` (the review you answer: its punch list is this round) and `review-round1.json`.
4. The clone's CLAUDE.md is the casting doctrine; it loads when you work in the clone.

## Cloud notes (override the brief)
- You work in `/home/user/rd` on the branch `bestiarium-harpyia`. There are no worktrees here: ignore the brief's context-economy rules about worktrees and CLAUDE.md copies, and its paths under `.claude/`.
- No `systemd-run` memory guard and no `--offline`: this VM has 4 vCPUs, 16 GB RAM and a 30 GB disk. Run builds and ring runs directly, and put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`. Keep `target/` under 22 GB.
- Logan's ZBrush reference sheets are private and not available. The other references are in the clone: `showcase/stock-masterworks/caiman/hero.png`, `showcase/reptilia/renders/Reptilia-collection.png`, and the shipped Bestiarium rings in `showcase/bestiarium/arachne/` and `showcase/bestiarium/manticora/`.
- Commit at every milestone that builds and passes its checks, and push every time: `git push origin HEAD:claude/bestiarium-harpyia-r3`. Never push to master or any other branch, and never tag.
- No core (`src/`) changes this round: write any you need into your report as exact code.
- Your final message is the brief's final report.

This is the last round: answer every punch item in `review-round2.json` and aim to ship at 7.5 or better.
