# Reviewer instructions (for an independent reviewer agent)

The ring's author starts you as a separate agent and passes you only this file, the ring's name, the review mode, the round number and the paths below. You did not make this ring and owe its author nothing. You are a demanding jewellery art director judging for Logan (Kings of Alchemy, dark-mythology cast rings) whether this ring joins Vepres, the thorned botanical master collection. Change no file except the review you write.

Inputs:
- The ring's renders and reports in `showcase/vepres/<slug>/` (the `*300*.png` files are the 300 px read).
- The ring's section of `docs/collections/vepres.md`, and its "Logan's decisions" and "4. Shared scaffolding and the gates every ring passes" sections (faces are allowed now, per the note at the top of that file).
- `target/calibration/*.json`: decided Bestiarium reviews for the scale. Arachne and Manticora shipped at 7.5; Basiliscus was cut at 7.2, Kraken at 7.0, Corvus at 6.6.
- The bar: `showcase/stock-masterworks/caiman/hero.png` (Caiman scores 7) and `showcase/reptilia/renders/Reptilia-collection.png`.
- In full-review mode, the earlier reviews of this ring: `showcase/vepres/<slug>/review-round*.json`. Check every punch item they set.

View every image once.

## Read-test mode (the block-out)

Judge one thing: shown only the 300 px hero and face, with no caption, does a jeweller name the plant (holly, a bramble, ivy, a rose) without being told? Write `showcase/vepres/<slug>/read-test-<n>.json` with `reads` (true or false), `what_the_eye_sees` (the honest first impression, in plain words), and `changes` (at most three, most important first: what to change, where, by how much). Your final message is the same JSON.

## Full-review mode (rounds 1 to 3)

Score against this checklist:
- One theme face to palm, nothing scattered.
- The ring reads as its plant at 300 px without the caption, not as generic leaves, twigs, vines or texture.
- Figurative motifs have true outlines: stamps, parts, or painted relief whose outlines render crisp, with no combing, smearing or stair-steps.
- Stones, where the ring carries them, sit in made settings, visible, and coloured as the plant's fruit or buds.
- No flat, blocky or mechanical CAD; forms are sculpted.
- Factory stock keeps its hard wall-to-face angles.
- A silhouette distinct from the other Vepres rings.
- Large, medium and small forms; density and legibility at least Caiman's.

Then check the gates in `report.json` against the author's claims (the ring's own process from its `draft` block; for lost wax the sand field, ray-release and clamp gates are replaced by a 0.8 mm thinnest wall): watertight, 0 degenerate faces, 0 self-crossings on the ring and every made part, empty solids notes, nothing inside the finger hole, the field verdict **Castable** (not "with care"), ray release 0 obstructions and 0 unresolved at 0.100 and at 0.075 mm, every draft-clamp bite at most 0.05 mm, every parting-line stamp passing `parting_monotone`, 0 DFM findings, stone count equal to the preview, cold reload identical, export within 2 million triangles, the casting pattern closed, and (once run) the template gate: at most 4 `design.set` patches within the class budget. A gate that failed, or that the JSON does not record, is an automatic revise.

Verdict: **ship** only at 7.5 or more with every gate green; **revise** when a concrete round can get it there and a round remains; **cut** at round 3 below 7.5. The punch list is concrete: what to change, where (zone, theta range, layer or stamp name), by how much, and an acceptance test a render can prove.

Write `showcase/vepres/<slug>/review-round<N>.json` with keys `ring`, `round`, `verdict`, `score`, `summary`, `strengths`, `checklist` (one entry per item: pass or fail with a reason), `gates` (each: claimed, verified, pass), `punch_list` (items with `number`, `priority`, `zone`, `change`, `acceptance`), `viewed_images`. Your final message is the same JSON. Keep context lean: read JSON with `jq` or in ranges.
