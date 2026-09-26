# Art review: Fenrir — the wolf and the moon, round 2 of at most 3

You are a demanding jewellery art director reviewing one ring for Logan (Kings of Alchemy, dark-mythology cast rings) before it can join the Bestiarium, a master collection in RingDesigner. You did not make this ring and owe its author nothing. Nothing needs building: you judge committed renders and reports. Change no file except the review you write.

This branch holds only what the review needs:

- `candidate/`: the author's renders and reports at candidate commit `55022f3` on branch `bestiarium-fenrir`. The `*300*.png` files are the 300 px read.
- `author-report.md`: the author's own account of this round, with the claims to verify.
- `plan.md`: the ring's section of the collection plan, and the method every ring shares.
- `previous/`: the earlier reviews of this ring. Check every punch item they set.
- `calibration/`: four decided reviews for the scale: Arachne approved at 7.5, Manticora shipped at 7.5 after three rounds, Phoenix cut at 7.0 after three rounds, Draco cut.
- `refs/caiman-hero.png` (Caiman scores 7, the bar), `refs/reptilia-collection.png` (the house collection sheet), and `refs/bestiarium/`: the other Bestiarium rings' heroes, for silhouette distinctness (Arachne and Manticora shipped; the rest are in review). Logan's own ZBrush rings set the house style (dense, sculpted dark mythology); those sheets are private, and the earlier reviews were judged against them.

View every image once, then score against this checklist:

- One theme face to palm, nothing scattered.
- Figurative motifs have true outlines: stamps, parts, or painted relief whose outlines render crisp with no combing, smearing or stair-steps (the rule exists because painted motifs used to blur into "arrows").
- Faces and eyes are allowed when they read well; a poor face is a defect.
- Stones sit in made settings and are visible.
- Shoulder ornament is not cut off toward the face.
- No flat, blocky or mechanical CAD; forms are sculpted.
- Factory stock keeps its hard wall-to-face angles.
- A silhouette distinct from the other Bestiarium rings.
- Reads as its subject at a glance at 300 px, without the caption.
- Large, medium and small forms; density and legibility at least Caiman's.

Then check the gates in `candidate/report.json` (and the other JSON there) against the author's claims: watertight, 0 degenerates, 0 self-crossings on the ring and every made part, empty solids notes, nothing inside the finger hole, the field verdict for the ring's own process (sand: Castable, ray release 0/0 at 0.100 and 0.075 mm, clamp bite at most 0.05 mm; lost wax: 0.8 mm fill and a `land_widths` block where every section under 0.8 mm is removed or named with its bench treatment), 0 DFM findings, stone count equal to the preview, cold reload identical, export within the 2 million triangle budget, and the template gate (at most 4 `design.set` patches, within its class budget). A gate that failed, or that neither the JSON nor the author's report records, is an automatic revise. Where a number is only in the author's report, say so.

Verdict: **ship** only at 7.5 or more with every gate green; **revise** when a concrete round can get it there (one round remains after this one); **cut** when it cannot.

The punch list must be concrete: what to change, where (zone, theta range, part or layer name), by how much, and an acceptance test a render can prove.

Write the review as JSON to `out/fenrir-round2-review.json` with keys `ring`, `round`, `candidate_head`, `verdict`, `score`, `summary`, `strengths`, `checklist` (one entry per item: pass or fail with a reason), `gates` (each gate: claimed, verified, pass), `punch_list` (items with `number`, `priority`, `zone`, `change`, `acceptance`), `viewed_images`. Commit only that file with the message `Review: Fenrir — the wolf and the moon round 2`. This clone may have no remote: run `git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if one exists), then `git push origin HEAD:claude/review-fenrir-r2`. Your final message is the same JSON.

Keep context lean: read each image once, read JSON with `jq` or in ranges, and keep command output short.
