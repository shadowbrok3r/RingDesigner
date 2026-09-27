# Sphenodon — the parietal (`sphenodon`): lane report, lost wax

**Outcome: cut at 7.1 after round 3.** The block-out read on the first test. Three full review rounds took the score from 6.5 to 6.9 to 7.1, against a ship bar of 7.5. **Review rounds used: 3 of 3.** Block-out attempts used: 1 of 3.

Every lost-wax gate is green at draft and at export, the cold reload is identical, and the template gate passed. The ring is cut on its read, not on manufacture. It reads at once as "a lizard ring with a green stone", but no reviewer would name it a tuatara.

- Branch: `claude/cataphracta-sphenodon`.
- Author file: `crates/ringdesign-core/examples/cataphracta_sphenodon.rs`.
- Outputs: `showcase/cataphracta/sphenodon/`.
- The earlier sand session's three failed read tests are kept as `sand-read-test-{1,2,3}.json`. Its report is superseded by this one.

## Process

Logan's decision of 2026-09-27 moved the ring to lost wax: `CastProcess::LostWax.apply(&mut d.draft)`, then `min_section_mm = 0.8` and `min_draft_deg = 0`. The recipe is an investment recipe (no sand), Silver 925. The sand gates are gone. In their place:

- the field verdict at the 0.8 mm fill;
- a `land_widths` block that names every section under 0.8 mm with its bench treatment.

The two-part undercut and ray release are reported as numbers only. I recorded the decision in `docs/collections/cataphracta.md`:

- the header list of Logan's decisions;
- both sheet subtitles ("FIVE IN SAND, THREE IN WAX");
- the sand/wax count line and the "primary side" paragraph;
- the Sphenodon Status and Process lines.

## Read test and reviews (verdicts verbatim in the JSON files)

| Step | File | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|---|
| Block-out 1 | `read-test-1.json` | **reads: true** | — | "A lizard wrapped round a plain polished band before you see anything else… a jeweller would say 'lizard ring' at once." Not yet a tuatara: the head was decorated rather than anatomical, the ridge was beaded, and the hind limbs and tail were broken up. |
| Round 1 | `review-round1.json` | revise | **6.5** | The strongest creature read in Cataphracta so far, with the peridot now parietal. Faults: goggle eyes on a lozenge head, a one-size square-paver tail, a blunt tail end, combing where relief crossed the rim, knob hind feet, and bench notes that contradicted their measurements. |
| Round 2 | `review-round2.json` | revise | **6.9** | "This round fixed the tail." The domed, staggered, graded scale rings work and the tail now ends in a point. Faults: the head was a flat slab with vertical walls and screw-head eyes, the hind feet were still knobs, the crest spines were square pyramids, and a radial comb ran from the neck into the seat. |
| Round 3 | `review-round3.json` | **cut** | **7.1** | "The best Sphenodon so far": a crowned skull, toed hind feet, pointed crest teeth, no comb at the seat, and every bench note stating its measured section. It still reads as "lizard ring" or "little crocodile". The eyes still read as screw heads, the snout is too long for a tuatara, a bald oval surrounds the stone, and the rows beside the crest read as tread. |

## Gates (`report.json`: `draft` block 768 × 320, `export` block 1536 × 448 with `--verify`)

| Gate | Draft | Export |
|---|---|---|
| Triangles | 490,554 | 1,371,272 (limit 2 M) |
| Watertight, degenerate faces | yes, 0 | yes, 0 |
| Self-crossings (ring; no made parts) | 0 | 0 |
| Solids and parts notes, stamps | empty, empty, 0 of 0 | empty, empty, 0 of 0 |
| Finger hole: vertices inside, nearest margin | 0, −0.00007 mm | 0, −0.00001 mm |
| Field under lost wax at the 0.8 mm fill | **Castable**, thinnest wall 2.13 mm | **Castable**, 2.13 mm |
| `land_widths`: all named | yes (7.59 mm² under 0.8 mm, thinnest 0.060) | yes (9.41 mm², thinnest 0.021) |
| DFM findings | 0 | 0 |
| Stones: report vs preview | 1 = 1 (174 preview faces) | 1 = 1 |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |
| Cold reload, empty library | — | identical vertices, faces and normals (685,634 / 1,371,272) |
| *Two-part numbers (not gated)* | undercut 47.4 of 1346 mm², worst −61.0°; release 519 / 745 obstructions at 0.100 / 0.075 | same areas; release 573 / 793 |

Export `land_widths` by feature. Each is read by one ray along a face's inward normal, the way `dfm::part_sections` reads a part. Each feature carries its bench treatment in the JSON.

| Feature | Area under 0.8 mm | Thinnest |
|---|---|---|
| The tail's tip | 3.38 mm² | 0.097 mm |
| The crest's spines | 2.63 mm² | **0.324 mm** (round 1 asked for ≥ 0.30) |
| Legs and toes | 1.44 mm² | 0.187 mm |
| Tail scale rings and saw | 1.08 mm² | 0.122 mm |
| Head: beak, eyes, lids, nostrils | 0.84 mm² | 0.173 mm |
| Body granules and tubercles | 0.02 mm² | 0.510 mm |
| The parietal seat (lip and bore vent) | 0.02 mm² | 0.021 mm |

The ring carries no stamps, so no 384 × 192 run was needed.

## Template gate (`collection_templates … --only sphenodon --verify-export`, class `painted`)

| Measure | Value |
|---|---|
| Source method | lift, 26 nodes, 0 exposed controls |
| `design.set` patches | **1** (`/manufacturing`), limit 4 |
| Graph size | **1,016,465 bytes** against the painted budget of 3 MB (no size review) |
| Cold source parity | identical |
| Mesh parity | 1,371,272 triangles; vertices, faces and normals identical |
| Result | `template_gate_passed: true` (`template-verification.json`) |

The editable design is 2.0 MB. On open, the first build takes 587 ms and the detail pass takes 5.9 s cold and 38 ms warm.

## What each part is, and why

- **Base.** `ProfileStyle::Flat`, 7.5 × 2.5 mm, crown 0.8 mm, comfort 0.15 mm, bore 18.6 mm.
  - Thickness-only keys: 1.0 at 0°, 1.05 at 90°, 1.0 at 180° and 0.9 at 270°.
  - The sides are **not** flattened. The Flat style's own 0.875 mm edge round gives the legs a rounded shoulder to wrap. A tight fillet combed every relief wall that crossed it.
  - The band is thinner than the brief's 3.4 mm because the animal adds up to 2.9 mm of relief.
- **"Tuatara".** One painted layer: a 2048 × 640 atlas over the bare band, 3.4 mm full scale, `skin::hide_layer`, blend Max. It is modelled in hide millimetres (`skin::Hide`: along the parting line from the stone, and across the section). Every part is a smooth analytic form, with no cliffs or near-vertical walls running across mesh columns.
  - **Head** (t = 0–14 mm from the snout).
    - Plan: a wedge 1.4 mm wide at the beak, 2.4 at 3 mm, 3.3 at the jaw joint (10.5 mm).
    - Section: a crowned skull (1 − 0.55u² − 0.45u⁴) with sloping cheeks, peaking at 1.6 mm. The beak's leading edge is rounded.
    - Features: a mouth line, nostrils, and domed eyes (1.6 mm, 0.55 mm proud) with a lens pupil. A heavy lid over each eye's upper half runs back toward the stone.
  - **Crest.** One spine line from the nape (t = 12.9) to the tail tip, tallest (1.3 mm) over the neck and shoulders, with a low saddle at the hips.
    - Spines are about 1.45 mm apart on the back, closing to 0.8 mm at the tip, on a web at 40%.
    - The section is a 1 mm blade (`dome^0.62`) and every tip is rounded. This is what holds the spines at 0.32 mm thinnest.
  - **Body.** Neck, barrel and pelvis, domed, on the crown. A row of keeled scales runs either side of the crest's foot.
  - **Legs.** Tapering tubes with a muscle swell. The fore feet grip the upper side faces. The hind feet lie on the crown beside the tail's root, toes pointing back. Five toes each.
  - **Tail.** Rounder in section, so its edge fairs into the band. It sways in a slow S and ends in a point that curls over the rim onto the +Z side face, 9 mm short of the snout. That gap keeps it clear of Ouroborus's composition.
  - **Skin.**
    - Body: granules on a 0.46 mm Voronoi grain, plus sparse mixed-size tubercles on the flanks.
    - Head: granules on a 0.28 mm grain. The two grains are blended, because a grain whose size varied along the ring sheared into streaks at the neck.
    - Tail: rings of domed squarish scales, staggered and bowed, graded from 0.9 to 0.35 mm with its width.
    - The eyes stay smooth. The granules stop 0.3 mm outside the seat's skirt.
- **"Parietal peridot".** A 3.0 mm round peridot, tint (0.50, 0.78, 0.12), on a `SeatPadLayer` at (90°, crest).
  - `GypsyMound`, `fit_stone`, then diameter 4.2 mm, crown 0.55, height 2.3 mm, blend 0.35 mm.
  - `Flush`, `through`, with a 0.6 mm raised drill dot on the casting pattern.
  - It sits on the crown of the skull 2.8 mm behind the eye line: parietal, never an eye.
- **Renders.** The brief's full set, plus `head.png` and `head-hero.png`: close-ups of the head's arc from the face and hero cameras.

## What I could not do

- **Name the animal.** Three rounds moved the read from "lizard ring" toward the tuatara but never reached it. The round-3 reviewer names four things still holding it back:
  - The eyes: the smooth zone I keep clear of granules round each eye, plus the ball's edge, still render as a ring with a slot.
  - The snout: 6.5 mm ahead of the eyes, where a tuatara's is short and blunt.
  - The bald oval round the stone. Ending the granules outside the mound's skirt, as round 2 asked, is what made it.
  - The keeled rows beside the crest, which read as tread.

  These are concrete, and a fourth round would likely clear 7.5. Under the loop the verdict is final.
- **Keep toes and every relief flank at 0.3 mm or more in the single-ray census.** The spines reach 0.32 mm. The toes, tail tip and head reach 0.10–0.19 mm, and the seat's lip and bore vent 0.021 mm. Rays near the foot of low relief cross the flank close to the metal beneath, so the census reads small numbers where nothing free-standing is thin. The notes state the measured figure rather than a nominal size.
- **Close a few sub-mm² slivers at the hind feet** (−Z side face, about 184°). They survive smoother toe sections. They are named under "legs and toes".

## Core changes wanted (not made; lanes may not edit `src/`)

1. **`dfm::face_sections`.** `land_widths` had to re-implement `part_sections` in the example to attribute thin faces to features. Expose the per-face reads, and fold `part_sections` over them. In `crates/ringdesign-core/src/dfm.rs`:
   ```rust
   /// Each face's single-ray section, in face order: `None` for a degenerate face, one turned toward `up`, or one no ray leaves.
   pub fn face_sections(solid: &crate::csg::Solid, up: Option<crate::csg::P3>) -> Vec<Option<f64>> {
       use crate::interaction::bvh::Bvh;
       let mesh = crate::mesh::Mesh {
           vertices: solid.v.iter().map(|p| crate::mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
           faces: solid.f.clone(),
           ..Default::default()
       };
       let bvh = Bvh::build(&mesh);
       let up = up.and_then(|u| {
           let l = (u[0] * u[0] + u[1] * u[1] + u[2] * u[2]).sqrt();
           (l > 1e-12).then(|| u.map(|x| x / l))
       });
       const IN: f64 = 1e-4;
       solid.f.iter().map(|f| {
           let [a, b, c] = f.map(|i| solid.v[i as usize]);
           let (e1, e2) = (std::array::from_fn::<f64, 3, _>(|k| b[k] - a[k]), std::array::from_fn::<f64, 3, _>(|k| c[k] - a[k]));
           let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
           let twice = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
           if !(twice > 1e-14) { return None; }
           let inward = n.map(|x| -x / twice);
           if up.is_some_and(|u| (inward[0] * u[0] + inward[1] * u[1] + inward[2] * u[2]).abs() > std::f64::consts::FRAC_1_SQRT_2) { return None; }
           let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0 + IN * inward[k]);
           bvh.ray(&mesh, o, inward).map(|(_, t)| t + IN)
       }).collect()
   }
   ```
   `part_sections` then becomes a fold over `face_sections(solid, up)` zipped with the face areas: the minimum, and the area of faces under `floor_mm`.
2. **A relief-aware section measure.** For painted relief, the census should skip rays that leave within a small angle of the local base surface's tangent plane, or read the section in the plane normal to the relief's own ridge line. Without that, low relief reports near-zero sections that the bench does not treat. I have not written this as code: it needs a design decision on the base surface reference.
