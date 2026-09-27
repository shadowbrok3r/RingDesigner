# Bestiarium ring lane: Fenrir — the wolf and the moon, round 3 of 3 (the last)

You are the ring author for Fenrir. Read this whole file first: the checkout below replaces it on disk. This seed also carries `brief.md` (the shared lane brief), `review-round2.json` (the review you answer; its punch list is this round) and `review-round1.json`; read them before the checkout too, or read them back later with `git show origin/cloud/lane-fenrir-r3:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin bestiarium-fenrir master cloud/lane-fenrir-r3`, `git checkout -b claude/bestiarium-fenrir-r3 origin/bestiarium-fenrir`, `git merge --no-edit origin/master`. Resolve conflicts only in your own example or outputs; if anything else conflicts, stop and report it. The repository's CLAUDE.md is the casting doctrine: search it for what bears on your work rather than reading all of it.

Master now carries three things this lane asked for: `ringdesign_core::sculpt` (your example's sculpt tools, moved unchanged; switching the example to it is welcome but optional, and only if the ring builds bit for bit the same), railless claw feet that rise up their own line on a sloped table (so the moon need not be lifted 0.4 mm for the fangs), and `dfm::part_sections` for the land-width gate.

## Cloud notes (override the brief)
- There are no worktrees here: ignore the brief's context-economy rules about worktrees and CLAUDE.md copies, and its paths under `.claude/`.
- No `systemd-run` memory guard and no `--offline`: this VM has 4 vCPUs, 16 GB RAM and a 30 GB disk. Run builds and ring runs directly, and put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`. Keep `target/` under 22 GB.
- Logan's ZBrush reference sheets are private and not available. The other references are in the repository: `showcase/stock-masterworks/caiman/hero.png`, `showcase/reptilia/renders/Reptilia-collection.png`, and the shipped Bestiarium rings in `showcase/bestiarium/arachne/` and `showcase/bestiarium/manticora/`.
- Commit at every milestone that builds and passes its checks, and push every time with `git push origin HEAD:claude/bestiarium-fenrir-r3`. Never push to master or any other branch, and never tag.
- No core (`src/`) changes this round: write any you need into your report as exact code.
- When you finish, write the brief's final report to `cloud-report.md` at the repository root in a last commit, push it, and make it your final message.

This is the last round: answer every punch item in `review-round2.json`, starting with the P0 (the sealed void in section-90), and aim to ship at 7.5 or better.
