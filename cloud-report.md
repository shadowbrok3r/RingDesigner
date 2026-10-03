# Cataphracta Gekko: cloud lane report

Branch `claude/cataphracta-gekko`, merged with master `2e11632`. Example: `crates/ringdesign-core/examples/cataphracta_gekko.rs`. Outputs: `showcase/cataphracta/gekko/`.

## Outcome: stopped at the block-out (no read in three attempts)

TASK.md allows three block-out attempts and says to stop if the third does not read. It did not, so **no review rounds were run (0 of 3)**. No ship or cut verdict applies. The subject needs rethinking before more detail goes on.

| Read test | Process | Reads | What the eye sees (reviewer, shortened) |
|---|---|---|---|
| 1 | Delft sand | no | "a lizard or newt"; legs "like staples or bracket clips"; hero "a beaded chain" |
| 2 | lost wax | no | "a lizard crawling across a pebbled band"; head "a small rounded knob"; beaded back reads as Heloderma; the figure melts into the ground; hero "a lump" |
| 3 | lost wax | no | "a lizard on a pebbled band"; the figure is now one animal in both views. The head reads as an arrowhead or snake; the spotted back still reads as Heloderma; the toe pads are the one gecko cue |

The full JSON is in `showcase/cataphracta/gekko/read-test-{1,2,3}.json`.

## Process decision (Logan's rule, 2026-10-03)

The ring was judged as **lost wax** from attempt 2 on: 0.8 mm minimum section, no pull rule. The decision is recorded in the Gekko section of `docs/collections/cataphracta.md`. The two-part undercut is reported as a number only. It does not pull from sand: 3.71% of the band and 6.93% with the part, of which 61.8 mm² is on the tokay.

The sand gates are what held it back. In Delft, a figure lying on the crown is pullable only if every pull line along Z meets it in one stretch from the crest. I built a "pull fill" (a column sweep over a cylindrical grid with an 8° lean), a parting-line split and draft edge-flips. With these, the Delft block-out got its undercut down to 0.18–0.3 mm² and its ray release to 0–1 obstructions of about 0.03 mm, but never to Castable. The fill also turned every bent limb and toe fan into a web. Clearing those webs needed bench cut parts, eyes soldered on at the bench, and drill and locating marks on the pattern. All of that code was removed when the ring moved to lost wax.

## Gates (attempt 3, draft 768 × 320, lost wax)

| Gate | Result |
|---|---|
| Finished mesh watertight, 0 degenerate, 0 self-crossings | pass (646,416 triangles) |
| Sculpted part uncrossed as placed; notes empty; part joined | pass |
| Nothing in the finger hole | pass |
| Lost-wax field verdict Castable (band and with the part judged); thinnest band wall 2.99 mm | pass |
| Every section under 0.8 mm named with its treatment | **FAIL**: body 0.60 mm (0.06 mm² under the floor) and head and neck 0.49 mm (0.76 mm² under) have no treatment. Toes 0.34 mm, pads, eyelids, tail and hide relief are named |
| 0 DFM findings | pass |
| Stones equal the preview | pass (no stone) |
| Within 2 M triangles; casting pattern closed | pass |
| Export 1536 × 448, `--verify`, 384 × 192 | not run: the ring never left the block-out |

## Template gate (attempt-3 design, class `painted`)

- `design.set` patches: **0**.
- Cold design and graph reload: identical. Mesh parity (vertices, faces and normals): identical, 1,479,262 triangles at 1536 × 448.
- Graph size: **4,747,403 bytes, over the 3 MB painted budget**, so `template_gate_passed: false` (size review required). Almost all of it is the 250 k-face stored sculpt mesh.
- Recorded in `showcase/cataphracta/gekko/template-verification.json`.

## What is built (attempt 3)

- **Base:** Procedural Flat, 7.0 × 3.4, `crown_mm` 1.2, crown exponent 3, `flatten_sides`, comfort 0.15, bore 18.6. I added width keys of 1.25 under the gecko (8.75 mm at 65–115°), with thickness held at or over width at every key, so the limbs reach the edges. **This departs from the section's thickness-only keys.**
- **"Tokay" (stored sculpt mesh, joined):** a distance field from `sculpt` primitives in a ring-following frame, meshed with `tetra_mesh`, relaxed, decimated to about 250 k faces and settled. It has:
  - a wedge head with jaw hinges and a mouth line;
  - lidded eyes with upright slit pupils;
  - a smooth torso with about 20 low rounded spots 1.5 mm apart;
  - four limbs bent at the elbow and knee, each foot with five toes and round pads;
  - a ringed tail that leaves the crest, crosses the near shoulder and runs down the side face.
- **Crown granules:** a painted Worley pebbling, 0.09 mm high at a 0.62 mm pitch, on the crown either side of the crest.
- **Tubercle rows:** C-R7 `tubercle_rows` on both side faces, sheared 0.35 and gated to the side faces.
- **Stones:** none.
- **Palm lamellae and split-lamella stamp:** not built, since they were never reached past the block-out.

## Enablers used

C-R7 (`reptile::svg::tubercle_rows`), the core `sculpt` toolkit, `skin::Atlas` painting, and stored CAD parts. In the sand attempt I also used C-R1-style `skin::draft_clamp` audits. From tonight's master I used `render::write_png_framed` and `yaw_facing` for the head close-up (`stones.png`) instead of a cropped mesh. I did not use C-B2, C-V1–5, C-T5–7, `crisp_relief`, `StampTop::Pillow` or Textura: none bears on a stoneless sculpted figure.

## What I could not do, and what the next attempt needs

- **The read.** In order of the reviewer's last punch list:
  - Blunt the snout to a radius of about 1 mm and widen the jaw hinge to 1.5× the neck. Make the eyes break the head's outline in the face view, at about 1.5 mm across and 0.6–0.8 mm proud.
  - Use 8–12 flat-topped spots of 1.0–1.5 mm, and give the figure a smooth halo against the pebbled ground.
  - Bring the near-side feet up so all four toe fans show in the face view, and halve the tail's rings.

  The deeper problem is scale. On a 7–8.75 mm band the tokay is a thin figure along the crown, and at 300 px its species cues (toe pads, eyes) are a few pixels each. A rethink could fill the face with the gecko seen from above, at the size of a signet face. That could be on a factory head, or with the gecko curled round the crown in plan, so the head and splayed feet take the camera's centre.
- **Sand.** A figurative gecko in two-part Delft sand needs the pull fill, bench web cuts and bench-set eyes described above, and it still left about 0.2 mm² of undercut from mesh-level facet noise. A heightfield mesh, or a facet-draft-aware decimator in `sculpt`, would make the fill exact. Neither is written.
- **Lands:** the body and head-and-neck readings under 0.8 mm need either thickening or a named treatment.

## Core changes wanted

None are needed for this lane's code; no `src/` file was touched. One request: a draft-aware decimation constraint, so a sand-pulled sculpt keeps the drafts its field guarantees. For example, `sculpt::decimate_with(&raw, target, cap, |face_normal, centroid| -> bool)`, which rejects a collapse when any resulting face fails the predicate.

## Commits

- `73eb5b7`: sand block-out attempt 1 and read test 1.
- `2548406`: lost wax, bent limbs and pads, pebbled crown; read test 2.
- `e188fa6`: block-out attempt 3; read test 3.
- The template gate, the doc status and this report follow.
