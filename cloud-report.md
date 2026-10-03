# Cataphracta: Chamaeleo, cloud report

Branch `claude/cataphracta-chamaeleo`. The first block-out is in `f337575` and `4f9b152`, with master `2e11632` merged in `a55dc47`. The rethink Logan approved is in `a1869da` (read test 4), `b95592b` (read test 5) and the commit that carries this report (read test 6). Nothing was pushed to master and nothing was tagged.

## Outcome

**Stopped at the block-out a second time.** Logan approved the rethink, and it improved every read, but none of read tests 4 to 6 named a chameleon. Following his instruction ("if none of the three reads, stop and report"), I ran **0 reviewed rounds**, so there is no review score. The template gate and the export `--verify` build were not run, because both belong to the rounds.

| Test | Design | `reads` | First read |
|---|---|---|---|
| 1 | Factory 001 in Delft. The head in profile on the parting line, with a ringed turret eye holding the stone. | false | "a target"; an abstract stepped signet |
| 2 | Factory 001 in Delft. A casque on the line behind a cushion boss, plus crests, coils and granules. | false | a tribal or industrial signet |
| 3 | Factory 001 in lost wax. A broader casque, open crest steps and bench grain. | false | a Maya glyph or robot mask |
| 4 | **Rethink**: a keyed body with a 9 mm plinth, and the head sculpted as one part (`sculpt.rs`) with a blade casque and cone eyes. The tail is coiled 2.4 turns on each 7.8 mm flank. | false | "a seal, a sleeping bird, or a fish with a dorsal fin". The spiral "finally reads as a curl". |
| 5 | The same, with ringed turret eyes, a wedge casque, cast granular skin and a mouth groove. | false | "a toad"; "closest yet". The casque read as a CAD slab and the eye as a spike. |
| 6 | The same, with a snout 3 to 4 mm past the eyes, beaded dome turrets with pupil bosses, a helmet blended into the skull, and the grin. | false | "a toad", "pickle", "sea cucumber". The helmet was smoothed away, the warts read as toad hide, and the eyes and grin were lost at 300 px. |

The reviews are `showcase/cataphracta/chamaeleo/read-test-{1..6}.json`. The renders in that folder are attempt 6's draft set.

## Why it still does not read

- **The tail is solved; the head is not.** From attempt 4 on, every reviewer read the spiral as a coiled tail. The head ran through four wrong animals. A blade read as a fin (seal or fish). A slab read as CAD. A merged helmet left a loaf (toad), and uniform warts read as toad skin. The fixes for each attempt undid the gain of the one before.
- **Two of the three cues are too small to read from the hero and face cameras.** The turret eye and the grin read clearly in the side render (`side.png`), which neither read-test camera shows. At 300 px from above, the eye becomes a knob and the grin disappears.
- **What I would do next.** I would not attempt a seventh test without a sculptor's reference sheet for the head, such as Logan's ZBrush sheets. They are not available in this cloud environment.
  - The casque should be a polished, tubercle-edged helmet standing 2.5 to 3 mm proud, with a shadowed back drop.
  - The skin should be fine graded granulation, not uniform warts.
  - The eyes should be larger cone-domes aimed at the hero camera.
  - The body should run from the head down into the coil, so the animal reads as one piece.

  These are read test 6's punch items, and they are concrete. The risk is the pattern above: each fix overshot.

## Gates (attempt 6, lost wax, draft and 384 × 192)

| Gate | Draft 768 × 320 | 384 × 192 |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | yes, 0, 0 | yes, 0, 0 |
| Solids and parts notes empty | yes | yes |
| Sculpted head part | 342,372 faces, 0 open edges, 0 self-crossings, 478 mm³ | same |
| Nothing inside the finger hole (≥ −0.01 mm) | −0.00006 mm | −0.00026 mm |
| Field verdict (lost wax), thinnest wall | Castable, 2.27 mm | Castable, 2.27 mm |
| Design-for-manufacture findings | 0 | 0 |
| Stones reported = preview; metal inside the stone | 1 = 1; 0 | 1 = 1; 0 |
| Triangles | 820,346 | 486,322 |
| Bonus: two-part Delft pull | blocked: 249 obstructions at 0.100 mm, deepest 3.6 mm; 3.7% undercut. It does not pour from sand. | same |

The export build and the cold reload were not run for attempt 6. The first block-out's export was clean: watertight, 0 crossings, Castable, pattern closed, cold reload identical.

## What the rethink is

- **Body:** a procedural Flat band, 7 × 3 mm. Keys run from 0.8× thickness at the palm (2.4 mm) to 3.0× (9.0 mm) and 1.3× width from 70° to 112°. The flank's side face is flat over 7.8 mm, from r 9.56 to 17.37 mm.
- **Head:** one stored sculpt (`sculpt.rs`) from a signed distance field, drawn at unit size and scaled 1.55×. It is joined to the plinth with 0.8 mm of sink.
  - The skull, a squared snout and the jaw are blended ellipsoids.
  - The helmet casque is a tilted ellipsoid narrowed toward the brow.
  - Each turret is a sphere with four rings of beads, a pupil boss and a 0.1 mm pit, aimed 25° forward.
  - The downturned grin is a groove each side.
  - Cast granules cover the skull, from a hashed lattice of domes; the casque ridge and the eyes are left smooth.
  - The head stands 7.3 mm over the crown. The snout faces rising theta, so the hero camera sees its face.
- **Tail:** an along-pull stamp on each flank, with a Pillow top 0.9 mm high. Its stroke runs from 1.05 mm down to 0.42 mm, from the body's rear into a 2.4-turn coil 7.55 mm across.
- **Alexandrite:** a 5 × 4 cushion on a boss 0.9 mm high on the back at 61°, behind the casque.
- **Hero render:** yaw 0.5 and pitch 0.55, lower than the collection's 1.0, so that the hero shows the head's profile.

## Enablers used

- **#248:** the `StampTop::Pillow` top on the tail. `crisp_relief` is off; nothing in this design needs it.
- From the first block-out: C-R1, C-R4 and C-R7. The rethink drops them, because a procedural lost-wax body needs none of them.
- **Not used:** C-B2, C-V1 to C-V5, C-T5 to C-T7, #255, #258, #259.

## Core changes wanted

These carry over from the first report.

1. **`granule_voronoi` packs sparsely.** Choose the largest radius that fits each throw, and draw coordinates from SplitMix64.
2. **The sand envelope runs whatever the process is.** Skip it when `draft.process == LostWax`. Separately, the 001 sand master self-crosses without it.
3. **DFM squashes hide-space cells.** Leave `ChartSpace::Hide` cells out of the `station_stretch` scaling.
4. **`frame_on` should project a stamp frame's `x` onto the parting plane**, so `parting_monotone` passes a ridge struck off a symmetric station.
5. **New: keyed `thickness_scale` is clamped at 3.0** (`profile.rs:2624`). A plinth taller than three profile thicknesses needs a thicker reference profile, which makes the palm heavier unless the palm is keyed down. Documenting the clamp, or raising it for bands whose palm is keyed below 1.0, would help.

## Process note

The Chamaeleo section of `docs/collections/cataphracta.md` records:
- Logan's lost-wax decision;
- the approved rethink (2026-10-03);
- the stop.

No extra rounds were granted or used.
