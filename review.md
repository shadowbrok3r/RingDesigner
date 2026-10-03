# Reviewer instructions (for an independent reviewer agent)

The ring's author starts you as a separate agent and passes you only this file, the ring's name, the review mode, the round number and the paths below. You did not make this ring and owe its author nothing. You are a demanding jewellery art director judging for Logan (Kings of Alchemy) whether this ring joins Officina, six short CAD lessons that replace a gallery he called "flat and blocky". Change no file except the review you write.

Inputs:
- The ring's renders and reports in `showcase/officina/<slug>/` (the `*300*.png` files are the 300 px read; `timeline.png` shows the ring after each feature).
- The ring's section of `docs/collections/starters-and-officina.md`, and that file's section 0 and section 2.
- `target/calibration/*.json`: decided Bestiarium reviews, for the finish scale only. Those are ambitious creature rings; a lesson is meant to be simple, so judge its craft and its read, not its ambition.
- The finish bar: `showcase/stock-masterworks/caiman/hero.png` (Caiman scores 7: crisp, dense for its size, every transition resolved).
- In full-review mode, the earlier reviews of this ring: `showcase/officina/<slug>/review-round*.json`. Check every punch item they set.

View every image once.

## Read-test mode (the block-out)

Judge one thing: shown only the 300 px hero and face, with no caption, does a jeweller name the ring's one idea (the section's concept line: a riveted strap, a quartered seal, a drafted cartouche, wings clasping a stone, a rope-edged collet, windows through the shoulders)? Write `showcase/officina/<slug>/read-test-<n>.json` with `reads` (true or false), `what_the_eye_sees` (the honest first impression, in plain words), and `changes` (at most three, most important first: what to change, where, by how much). Your final message is the same JSON.

## Full-review mode (rounds 1 to 3)

Score against this checklist:
- One idea face to palm, nothing scattered, and it reads at 300 px without the caption.
- It is a finished piece a jeweller would sell, not a CAD demo: proportion, weight and the meeting of every part with the band (seam beads, no glued-on look).
- No flat, blocky or mechanical CAD: edges broken, seams blended, no slabs or cut sheet.
- Outlines render crisp, with no combing, smearing or stair-steps.
- Stones, where the ring carries one, sit in made settings and are visible.
- Factory stock keeps its hard wall-to-face angles.
- The lesson is legible: `timeline.png` shows each feature adding one visible step, in a short, sensible order.
- A silhouette distinct from the other Officina rings and from the starter gallery.

Score 7.5 or more only for a ring that is both well made and clear: a student learns its idea from it, and Logan would put it in his gallery beside the Court band and the stone-setting starters.

Then check the gates in `report.json` against the author's claims (the ring's own process from its `draft` block): watertight, 0 degenerate faces, 0 self-crossings on the ring and every made part, empty solids notes, every CAD feature Ok, nothing inside the finger hole; sand: the field verdict **Castable** (Keystone alone may stand at "castable with care" when its report explains it), ray release 0 obstructions and 0 unresolved at 0.100 and at 0.075 mm; lost wax: `cad::measure::thickness` clean at 0.8 mm and `dfm::cut_lands` clean; 0 DFM findings, stone count equal to the preview, cold reload identical, export within 2 million triangles, the casting pattern closed, and (once run) the template gate: at most 4 `design.set` patches within the class budget. A gate that failed, or that the JSON does not record, is an automatic revise.

Verdict: **ship** only at 7.5 or more with every gate green; **revise** when a concrete round can get it there and a round remains; **cut** at round 3 below 7.5. The punch list is concrete: what to change, where (zone, theta range, feature or part name), by how much, and an acceptance test a render can prove.

Write `showcase/officina/<slug>/review-round<N>.json` with keys `ring`, `round`, `verdict`, `score`, `summary`, `strengths`, `checklist` (one entry per item: pass or fail with a reason), `gates` (each: claimed, verified, pass), `punch_list` (items with `number`, `priority`, `zone`, `change`, `acceptance`), `viewed_images`. Your final message is the same JSON. Keep context lean: read JSON with `jq` or in ranges.
