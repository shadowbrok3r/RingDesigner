# Ogiva concept spike: report

Three rethinks of Ogiva (`ogiva`) were blocked out and read-tested. **Only the arch reads.** It passed its first read test on its outline alone, so it needed no revision. The keel judged on its section, and the gargoyle on factory 015, both failed their first test and their one revision.

Nothing here is detailed and no review rounds ran. The choice is Logan's.

- Example: `crates/ringdesign-core/examples/tenebrae_ogiva_spike.rs <arch|keel-section|gargoyle> [OUT_DIR] [--rev]`
- Outputs: `showcase/tenebrae/ogiva-spike/<option>/`. The first block-outs of the two revised options are kept in `round1/`.
- Read tests: `read-test.json` (test 1) and `read-test-2.json` (test 2) in each option folder.
- Quick checks: `check.json` (and `check-2.json` for a revision) in each option folder.

## Results

| Option | What it is | Process | Read test 1 | Read test 2 | What the eye saw | Main risk |
|---|---|---|---|---|---|---|
| `arch` | Seen along the finger, the ring is one equilateral pointed arch: a keeled extrados, two chamfered orders and a sunk mouth in the head, imposts at the springers, straight piers on a flat sill. All of it is drawn on the parting plane and extruded along the pull. | **Delft sand**, CAD-only, about 8.1 g silver | **reads** | not needed | "A pointed lancet arch at once, with nested archivolt orders stepping in at the apex and two impost blocks… 'Gothic arch' or 'church window' without a caption." The hero alone is weaker: "a teardrop or Reuleaux triangle" band. | **Local wall at the keel crease.** `cad::measure::thickness` at 0.8 has 93 of 384 samples below, because the keel is a knife edge (it needs a ~0.3 mm round). Ray release at 0.100 mm: 0 obstructions, 0 unresolved. The field verdict does not apply to a CAD-only ring ("with care"). 1 degenerate face to clear. |
| `keel-section` | The original keel: a lancet section revolved round the finger. Test 1 judged the plain keel on `section.png` and the end-on `side.png` (plus hero and face). Test 2 added 13 crockets, 24 lancet niches a flank and a side-turned hero. | **Delft sand**, CAD-only, about 11.5 g | does not read | does not read | Test 1: "a plain smooth gold band… The section and side views show a fine lancet… but that shape lives only in cross-section." Test 2: "a gear or sprocket ring… square cog teeth… radial slots." | **The read, not a gate.** Ray release is clean (0/0). Thin-wall samples come from the niche floors and the crocket roots (32 of 383 below 0.8 in the revision). |
| `gargoyle` | A crouched winged gargoyle drawn on the parting plane and carved in layers along the pull, on factory **015 Octagon's** Delft sand master. Test 2: rounded layers, the head thrust 2 mm past the table like a spout, a pinnacle behind the wing, and a blind lancet arcade on both head walls. | **Delft sand** holds it. Field **Castable**, ray release 0 obstructions and 0 unresolved, 0 DFM. About 26–29 g (stock included). | does not read | does not read | Test 1: "a novelty animal-topper signet… a griffin, a dragon or a winged dog… a flat cut-out silhouette." Test 2: "a winged dragon or griffin statuette… 'dragon signet ring', possibly 'gargoyle'… a quartz crystal or obelisk… seven small slots." | **Flatness.** Along-the-pull carving gives a stack of plates edge-on, and both reviewers failed it as a cut-out or badge. Sculpting it in the round means lost wax. Weight is also high. |

300 px renders:

- arch: `showcase/tenebrae/ogiva-spike/arch/hero-300.png`, `showcase/tenebrae/ogiva-spike/arch/face-300.png`
- keel-section, test 1: `showcase/tenebrae/ogiva-spike/keel-section/round1/hero-300.png`, `showcase/tenebrae/ogiva-spike/keel-section/round1/face-300.png`, with `round1/section.png` and `round1/side.png`
- keel-section, test 2: `showcase/tenebrae/ogiva-spike/keel-section/hero-300.png`, `showcase/tenebrae/ogiva-spike/keel-section/face-300.png`, with `section.png` and `side.png`
- gargoyle, test 1: `showcase/tenebrae/ogiva-spike/gargoyle/round1/hero-300.png`, `showcase/tenebrae/ogiva-spike/gargoyle/round1/face-300.png`
- gargoyle, test 2: `showcase/tenebrae/ogiva-spike/gargoyle/hero-300.png`, `showcase/tenebrae/ogiva-spike/gargoyle/face-300.png`

## How the tests ran

- **Blind reviewers.** Each reviewer was a fresh agent given only `target/review.md`, "Ogiva (`ogiva`)", "read-test mode" and the render paths. The renders were copied to neutral folders (`target/readtest/<random id>/`) so the folder names `arch` and `gargoyle` did not give away the intended reading.
- **Reviewers measure against the keel brief.** `review.md` sends every reviewer to Ogiva's section of `docs/collections/tenebrae.md`, which is still the keel brief. Both gargoyle reviewers therefore judged it against "the pointed arch the name promises" and asked for the keel or an arch back. Their first impressions are still honest, but the gargoyle never had a neutral judge.
- **One caveat on keel test 2.** The revised section is cut through the crown, which is where the finial now stands. The "straight-sided rectangular fin" the reviewer read on `section.png` is the finial plate, not the keel's profile; `round1/section.png` shows the clean lancet. Test 2's verdict also rests on hero and face reading "gear", so the caveat does not change the result.
- **How each revision applied its reviewer's changes.** The keel took all three of its test-1 changes. The gargoyle took changes 2 and 3; change 1 was "rebuild it as the keel ring", which is another option.

## What was built, briefly

- **Arch.** Equilateral head: springers at y −3 and ±11.0 mm, arc radius 22, apex 6.75 mm over the bore. Piers drop to a flat sill 1.7 mm under the bore.
  - Keel: the outline at z 0, falling 0.8 mm inward over 1.05 mm (a 105° crease).
  - Face: 6.0 mm wide along the finger.
  - Orders: sunk 0.55 and 1.0 mm with 35° chamfers; the mouth is 1.5 mm deep. Imposts are 1.1 mm tall, project 0.6 mm and stand 0.35 mm proud.
  - Bore: a double cone drafted 3° from the parting line.
- **Keel.** The attempt-3 lancet section: 4.2 wide, head radius 1.5 × width, keel at r 14.38 (90° keel), 0.25 mm step moulding. The revision drops the step and adds:
  - 13 leaf crockets from θ 18° to 162°, 1.25 mm proud, with a 2.1 mm finial;
  - 24 lancet niches a flank, 1.1 mm wide, from r 10.0 to 12.4.
- **Gargoyle.** 015 Octagon sand master, table top at y ≈ 14.04. The figure is in four pieces (trunk, head, foreleg, tail), plus a wing membrane with four finger bones, haunch, foreleg, brow and an eye socket, half-widths 0.8–3.0 mm.
  - The revision rounds the trunk, haunch, foreleg and head in three nested layers (to ±3.5 mm), thrusts the head out to 2 mm past the table, and drops the tail.
  - It adds a 9.6 mm gabled pinnacle with a blind lancet, and 7 blind lancets (1.2 × 2.7 mm) on each head wall, cut 0.6 mm into the stock.

## Platform notes found on the way (no core changes made)

- `Operation::Loft` through polyline sections tessellates with open edges, and a kernel union of two lofts gives `NoClosedForm`. The arch went back to drafted extrusions.
- `extrude_tapered` refuses any loop where the inset consumes a short segment. This bites at a 90° corner next to short arc chords, and at a sharp apex. The workarounds were:
  - give pocket regions a straight leg below the springers;
  - round an apex by at least the inset;
  - walk spline outlines at an even 0.28 mm after corner cutting.

  One long gargoyle outline still failed, even though its own offset was clean, so the body is built in overlapping pieces.
- Drafted halves overlapped 0.03 mm across the parting line leave a waist that the ray release reads as undercut (0.03 mm deep on the 37° keel). Starting each half 0.008 mm under the line clears it.

## Recommendation for Logan (my call, if it were mine)

**Build the arch.** It is the only option that read, it read on the first test, and it read from the outline alone, before any crocket, tracery or stone. That is the lesson every failed Tenebrae and Bestiarium ring points to: put the subject in the silhouette the cameras see. It pours in Delft sand by construction, it weighs about 8 g, and its one real gate risk (the knife-edge crease) is a small fix.

To keep it clear of Porta, keep it to one great arch, not a doorway. The reviewer's next steps fit inside the concept:
- carry the orders down to the imposts;
- mould the imposts as capitals;
- add the crocket run up the extrados to a finial;
- consider a stone in the mouth as its one piece of glass.

The weak point to watch is the hero, which still reads "teardrop band".

I would drop the keel-as-section: two tests confirm the old finding that a revolved section cannot carry the read. I would also drop the gargoyle as a sand ring. Faces may be allowed, but along-the-pull carving stays a cut-out. If Logan wants a gargoyle, it belongs in lost wax, sculpted in the round, and probably as a ring of its own rather than as Ogiva.
