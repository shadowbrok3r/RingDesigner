# Ogiva (`ogiva`): final report

**Outcome: stopped at the block-out.** Three block-out attempts ran, and none passed the read test. Under TASK.md that means stop and report: the subject needs rethinking, not detailing. **Review rounds used: 0.** No full review ran and there is no score. The ring was not shipped and not cut by a full review. Its concept needs to go back to the lead or to Logan.

Branch `claude/tenebrae-ogiva`. The example is `crates/ringdesign-core/examples/tenebrae_ogiva.rs`, and the outputs are in `showcase/tenebrae/ogiva/`.

## Read tests (independent reviewer, fresh agent each time)

| Attempt | reads | What the eye saw | Changes asked |
|---|---|---|---|
| 1 | **false** | "A cog or sprocket": a plain round band with stud-like crockets and slot niches. The keel did not show, and end-on the ring was "a thin featureless lens or blade". | Real curled-leaf crockets, graded, climbing to a finial; a readable blind lancet arcade; a sharper keel with a step moulding. |
| 2 | **false** | "A crown, not a pointed arch": a round washer with a fence of hooks round its upper half and a cross on top ("bishop's crown"). The outline in the face view was a perfect circle. | Make the outer outline over the hand an ogive with the finial on its apex; mirrored graded crockets climbing its two slopes; taller lancets (2:1) with dark floors. |
| 3 | **false** | "A little church or chapel on top of a round crown": a small house-shaped gable (it read as straight roof slopes), three slots and a cross, with crockets "like shepherd's crooks … candles" climbing a staircase. Still "not a pointed arch". | Make the gable a true two-centred arch about 9–10 mm across (two-fifths of the width) and 1.5× taller, blending into the band at 45°/135°; two 3:1 lancets with a trefoil oculus and an edge moulding; crockets 5–6 per side as leafy knobs on the arch's moulding, with no stepped posts. |

The files are `read-test-1.json`, `read-test-2.json` and `read-test-3.json` in `showcase/tenebrae/ogiva/`.

### Why it needs rethinking
- A section revolved about the finger shows its shape only in silhouette, end-on or when cut. From the hero and face cameras, a pointed-arch section reads as a rounded band, whether the keel is 140° or 90°.
- To read at 300 px, the arch had to be moved into the outline the camera sees (attempts 2–3). That turns the ring into a gabled crown, a different subject from "the keel". The last read asks for an even larger arch (about 9–10 mm span, about 7 mm rise over the band). That is a pinnacle ring, not Ogiva.
- Suggested directions for the lead, in my view:
  1. Re-brief Ogiva as a *portal/arch ring*: an arch outline cut and extruded along the pull as the face (this overlaps Porta).
  2. Keep "the keel" but judge it on `section.png` and the end-on `side.png`, not the face.
  3. Drop it and give its sand slot to another subject.

## What was built (the attempt-3 block-out, the state committed)
- Base: CAD only, with no `Band` feature. The section (`section.png`) is a lancet 4.2 mm wide with 0.5 mm straight jambs off the bore. Its head arcs have a radius of 1.5× the width, centred 4.2 mm across the axis. The keel stands at r 14.38 at 90° (from 44.9° of draft each side). A 0.25 mm drafted step moulding sits 0.9 mm below the keel. There is a 0.12 mm comfort arc across the bore and 0.3 mm foot rounds. Bore r 9.30, size from an 18.6 mm bore. Silver 925, about 12.15 g (1173 mm³).
- Gable: a loft through 31 of the ring's own sections, stretched out to an ogive. The ogive is tangent to the keel circle at 30°/150°, with its apex 4.5 mm over the keel at 90° (right arc centre (−13.1, −7.6), radius 29.5).
- Crest moulding (0.8 mm proud, ±0.6 mm), 4 lobed leaf crockets per side (grown 1.0 to 1.6×, ±0.8 mm) and a 4.5 mm fleur finial. All are drawn on the parting plane and extruded along the pull with 3° draft, then mirrored across the band and across the crown's section plane.
- 15 blind lancet niches per side (1.4 × 2.8 mm, floor at z ±0.45), arrayed from 130° to 50°, clear of the gable. Three lancet lights (1.6 mm centre, 1.3 mm pair) are pierced through the gable, each half drafted 3° from the parting plane.

### The feature tree as sentences
1. Draw the pointed-arch section on the plane through the finger.
2. Revolve the arch round the finger.
3. Loft the gable over the hand through its pointed sections, rising to the ogive.
4. Lay the parting plane just under the keel.
5–8. Draw the crest moulding up the gable's right slope; raise its cope half along the pull; mirror it into the drag half; join the halves.
9–12. Draw the crockets climbing the right slope and half the finial; raise, mirror and join them the same way.
13. Set the crockets on the crest.
14. Mirror the climb across the crown onto the left slope.
15. Join both climbs and close the finial.
16–20. Lay a plane over the cope foot; draw one lancet niche; sink it along the pull; array the niches round the cope foot, clear of the gable; mirror them into the drag foot.
21–24. Lay a plane over the whole cope half; draw the three lancet lights; pierce them down to the parting plane; mirror them up from the drag side.
25–31. Cut the cope foot's niches, the first niche and the drag foot's niches; raise the gable on the band; join the crest, crockets and finial; pierce both halves of the lights.

There are no stones, no stamps and no field layers.

## Gates: draft (768 × 320) and export (1536 × 448 `--verify`); both builds give the same numbers
| Gate | Result |
|---|---|
| Watertight, 0 degenerate, 0 self-crossings | **FAIL**: watertight, 0 crossings, but **3 degenerate faces** |
| Every CAD part uncrossed, every feature `Ok` | pass (31/31 Ok) |
| Solids/parts notes empty | pass |
| Nothing inside the finger hole | pass (nearest vertex 9.3003 mm, 0 inside) |
| Sand field verdict Castable | **FAIL**: "Castable with care". For a parts-only ring the field does not apply: "CAD solids require mesh-space manufacturing inspection". |
| Ray release 0.100 mm | **FAIL**: 24 obstructions, 0 unresolved |
| Ray release 0.075 mm | **FAIL**: 16 obstructions, 0 unresolved |
| Local wall ≥ 0.8 mm (`cad::measure::thickness`) | **FAIL**: 42 of 383 samples below; minimum 0.02 mm at (7.16, 17.00, −0.03), a sliver where the crest's drafted halves meet the gable near the parting plane |
| Sketch lands ≥ floors | **FAIL**: the niche-to-bore rail is 0.78 mm (floor 0.8). Other lands: niche bar 2.87, axial web 0.90, niche floor width 1.19, crocket plate 1.52. |
| 0 DFM findings | pass |
| Stones reported = preview | pass (0 = 0) |
| Casting pattern closed, 0 degenerate, 0 crossings | **FAIL**: 3 degenerate faces |
| ≤ 2 M triangles | pass (81,046) |
| Cold reload identical | pass (vertices, faces and normals identical) |

The export build gives the same triangle count as the draft: a CAD-only ring is tessellated at the export chord from 512 θ-steps up. The sand gates were not worked on, because the block-out never read.

## Template gate (run once, after stopping)
- `collection_templates tenebrae … --only ogiva --verify-export`, class `procedural`.
- **1** `design.set` patch (`/manufacturing`; at most 4 allowed): pass.
- Graph **1,137,494 bytes** against the **300 KB** procedural budget: **fails** ("review required"). The bulk is the 31 inline loft sections and the chord-walked polylines.
- Cold source identical: **true**. Mesh parity (vertices, faces and normals): **true**. 81,046 triangles. `template_gate_passed: false`.
- 37 nodes; first build 2.1 s.
- The record is in `showcase/tenebrae/ogiva/template-verification.json`.

## What I could not do, and core changes wanted
1. **Revolved circular pieces do not tessellate.** Revolving any sketch arc gives "Kernel could not tessellate 1 faces" or "N nonmanifold edges": adjacent torus faces sample their shared circle differently. Every arc in the section is walked in 0.1 mm chords (cones), which loses the exact geometry and the constrained-sketch lesson. Wanted: shared-edge tessellation in `cad::tessellate_traced`, so that both faces take the edge's own samples:
   ```rust
   // cad.rs, tessellate_traced: sample each edge once and hand the samples to both faces
   let tol = TessellationTolerance::new(angle, 1e-7).with_chordal_deflection(chord_mm).with_shared_edges(true);
   ```
   (`with_shared_edges` would be a new cadkernel option; the alternative is stitching the seams in `stitch_chord_gaps` with a T-junction split, not only closing chord gaps.)
2. **A drafted extrusion refuses Béziers** (`extrude_tapered` returns `None` for anything but lines and arcs). It also refuses outlines whose inset drops a segment: sharp points, and acute notches narrower than the draft offset. Crockets and the finial are walked in chords of at least 0.12 mm, with blunted tips. Wanted: tolerate dropped segments in the inset (`sweep.rs`: rebuild the loft from the offset's own segment count) rather than return `None`.
3. **Mirrored outlines in one sketch fail the drafted extrude.** The left climb is therefore a CAD `Mirror { Section { theta_deg: 90 } }` of the right.
4. **`Loft` only runs along each section plane's normal.** Fanned section planes must be ordered from 150° down to 30°. Wanted: in `cad.rs` `Operation::Loft`, reverse the sections when `(centre[1] − centre[0]) · normal[0] > 0`.
5. **Ring (Brep) minus niche (Brep) gives `NoClosedForm`.** The booleans are reordered so the first operand is a mesh and csg runs everything.
6. The 1.1 MB template would need a `cad.loft` node that samples its sections from the parameters rather than storing 31 sketches.

## Renders (studio gold, draft and export)
- The views are `hero`, `face`, `side`, `top`, `palm`, `shoulder`, `reverse`, `keel`, `stones` (the crown close-up; the ring has no stones), `section`, `bare-vs-finished`, `hero-300`, `face-300` and `contact-300`.
- Cameras keep the crown up. For this ring the **face** is its façade seen along the finger, and the **side** is the keel end-on.
