# Cataphracta revival: Heloderma

You are reviving Heloderma (`heloderma`). It was cut at 6.3 after 3 reviewed rounds, close to the 7.5 ship bar, and Logan wants rings that came close to ship. You get up to two more reviewed rounds. Read this whole file first: the checkout below replaces it on disk. Then read `brief.md`, `lessons.md` and `review.md` from this seed, or read them back later with `git show origin/cloud/revive-heloderma:<file>`.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from:
`git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master claude/cataphracta-heloderma cloud/revive-heloderma`, `git checkout -b claude/cataphracta-heloderma-revival origin/claude/cataphracta-heloderma`, then `git merge origin/master` (master has moved since the ring was cut; resolve any conflict in your example, never in `src/`). Copy the seed's reviewer files for later: `mkdir -p target && git archive origin/cloud/revive-heloderma calibration review.md | tar -x -C target`.

Cloud VM notes (override the brief and CLAUDE.md where they disagree): Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. No `systemd-run` memory guard and no `--offline`: run cargo and ring builds directly and let cargo fetch. Put anything longer than a few minutes in the background (a single command times out after 10 minutes). Set `CARGO_INCREMENTAL=0`; keep `target/` under 22 GB. Logan's private ZBrush sheets are not available; the references in the repository are.

## The loop
Read `cloud-report.md` and every review in `showcase/cataphracta/heloderma/` first. The last review's punch list is your brief. Its first item names the shape that keeps the ring from reading as its species; fix that before anything else, and check it against `lessons.md`.

**Round 4.** Apply the whole punch list. Rebuild at draft and at export, run every gate in `brief.md`, and fix what fails before any review. Render, zoom your own renders to 2x and fix any stair-step, comb or seam you can see, then commit and push with `git push origin HEAD:claude/cataphracta-heloderma-revival`. Start a fresh reviewer agent (the Agent tool) given only `target/review.md`, the ring's name and slug, "full-review mode", and the round number. Never pass it your own notes or opinion.

**Round 5**, only if round 4 did not ship: run `git fetch origin master` and merge it, apply the new punch list, run the gates, render, push, and start another fresh reviewer the same way.

Stop at **ship** (7.5 or more with every gate green), or after round 5, when a score under 7.5 is a **cut**. The reviewer's verdict stands. Do not argue with it, re-run it for a better score, or tell it what to find.

**After the last round,** shipped or cut, run the template gate from `brief.md` and record its numbers. Commit everything, and push.

## Master moves tonight
The lead is landing platform work on master while you build, including crisper stamp and relief edges in the renders. Merge `origin/master` at the start of round 5, then rebuild, re-run every gate and re-render.

## Finish
Write the brief's final report to `cloud-report.md` at the repository root. Include both new review verdicts and scores, beside the earlier ones. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag.

## Your ring
Your ring's section of `docs/collections/cataphracta.md` is "Heloderma". It is lost wax (Logan, 2026-09-27). The beaded one. It was cut five days ago, so master has moved a long way: merge it carefully and rebuild from scratch. It was cut on 2026-10-02, before the crisp-edge fix, and every reviewer marked it down partly for stair-steps, combing and smeared relief, which were mostly a render artifact (#248). **Round 4 starts with the crisp pass:** merge master, set `crisp_relief` where the ring carries steep height-field relief, use `StampTop::Pillow` for leaves and petals, and re-render every close-up framed. Then apply the round-3 punch list in full. This revival is an extension Logan granted on 2026-10-03, so write that into your ring's section of the collection doc before the first review.

**Logan's rules, 2026-10-03.**
- **Process:** where your ring's process is in question, or the sand gates are what hold it back, judge it as lost wax: 0.8 mm minimum section, no pull rule. If it also happens to pull from sand, report that in your report as a bonus. Record the decision in your ring's section of the collection doc.
- **Extra rounds:** any extra round Logan has granted must be written into your ring's section of the collection doc, one line with the date, before you start the reviewer. A reviewer applies the three-round cap unless the doc records an extension.

**Master is now `2e11632`.** Run `git fetch origin master` and merge it before you build. Since last night it has gained:
- **#248, crisp edges.** Most "stair-steps, combing and smearing" in past reviews were a render artifact. Render close-ups with `render::write_png_framed(path, &parts, render::yaw_facing(theta), pitch, render::Framing::new(centre, half_width_mm), 1600)`, never a cropped mesh. Set `d.crisp_relief = true` for steep height-field relief. Use `StampTop::Pillow { crown_mm }` for leaves, petals and any domed non-convex stamp.
- **#255, Textura and marks.** Real Textura lettering through `sketch.text` with `font: "Textura"`. `Component::mark` (set `mark: false` on bench parts).
- **#258, patterns along a path.**
- **#259, CAD fallbacks.** Revolves through real sketch arcs, drafted extrusions of Béziers and notched outlines, Brep minus Brep through csg, lofts in either order from any corner, and mirrored halves in one sketch (overlap them by height × tan(draft)). Draw what you mean instead of working around the kernel.
- **#257, true stone plans.** Pear, trillion, heart and half-moon seats sit on their true girdles.

One gap to know about: the graph lift cannot carry `crisp_relief` yet, so a design whose template gate needs the lift should leave it off and say so. A lane is fixing it today.
