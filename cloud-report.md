# Sphenodon, the parietal (`sphenodon`): revival report, lost wax

**Outcome: cut at 7.4 after round 5.** The ring was revived from its round-3 cut (7.1) for two more reviewed rounds. Round 4 scored **7.3** (revise) and round 5 **7.4** (cut), against a ship bar of 7.5. Every gate is green at draft and at export, the cold reload is identical, and the template gate passes. The ring is cut on its read, not on manufacture. Both new reviewers call it the best Sphenodon so far. At 300 px it still reads as "a lizard ring with a green stone", or an iguana, rather than a tuatara.

- Branch: `claude/cataphracta-sphenodon-revival`, from `claude/cataphracta-sphenodon` with `origin/master` merged in (at `8e5a59a`). The only conflict was `cloud-report.md`; I kept this lane's version. `src/` was not touched.
- Master did not move between rounds. The round-5 `git fetch origin master && git merge origin/master` was a no-op, so the "crisper stamp and relief edges" platform work was not available to this ring.
- Author file: `crates/ringdesign-core/examples/cataphracta_sphenodon.rs`. Outputs: `showcase/cataphracta/sphenodon/`.
- Reviews: `review-round4.json` and `review-round5.json`. Each came from a fresh reviewer agent given only `target/review.md`, the ring's name and slug, "full-review mode" and the round number.
- Process: lost wax (Logan, 2026-09-27), with `min_section_mm` 0.8 and `min_draft_deg` 0. Silver 925, investment recipe.

## Verdicts, all rounds (verbatim in the JSON files)

| Round | File | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|---|
| Block-out 1 | `read-test-1.json` | reads: true | — | "Lizard ring" at once, but not yet a tuatara. |
| 1 | `review-round1.json` | revise | 6.5 | Goggle eyes on a lozenge head, a square-paver tail, combing at the rim. |
| 2 | `review-round2.json` | revise | 6.9 | The tail was fixed. A slab head with screw-head eyes, knob feet, and square crest spines. |
| 3 | `review-round3.json` | cut | 7.1 | A crowned skull and toed feet. The eyes still read as screw heads, the snout was 6.5 mm (monitor-like), a bald oval surrounded the stone, and the dorsal rows read as tread. |
| **4** | `review-round4.json` | **revise** | **7.3** | Four of the five round-3 items passed: lidded dome eyes with no ring or cross, a 4 mm snout, skin up to a 0.4 mm rim, and the collection copy fixed. New faults: the 3 mm stone read as the lizard's eye from three-quarter views, there was no beak overhang in profile, the keeled scales formed straight osteoderm files, the head skin had crocodile tiles, and the tail was a corn cob. |
| **5** | `review-round5.json` | **cut** | **7.4** | Round 4's items were mostly done. The eyes are larger than the stone, the 2.5 mm stone sits back on the crown, the beak is squared and notched, the head is finely granular, and the tail is staggered. It still misses on three counts: in the profile view (`stones.png`) the stone still sits where an eye sits; the lid and slit read as a "coffee bean"; and the crest from the nape reads as square pyramids, with a stud grid at the tail's rim edge. |

Round 5's punch list is recorded in its JSON for a further revival: a flush table or a 2.0 mm stone at stone_t 12; eyes raised 0.3 mm, with the lid on the top third only and a shorter pupil; a crest of about 1.5× more thin blades; and a staggered, lower tail edge at the rim.

## Gates (`report.json`: `draft` block 768 × 320, `export` block 1536 × 448 with `--verify`)

| Gate | Draft | Export |
|---|---|---|
| Triangles | 491,182 | 1,373,452 (limit 2 M) |
| Watertight, degenerate faces | yes, 0 | yes, 0 |
| Self-crossings (ring; no made parts) | 0 | 0 |
| Solids and parts notes, stamps | empty, empty, 0 of 0 | empty, empty, 0 of 0 |
| Finger hole: vertices inside, nearest margin | 0, −0.000025 mm | 0, −0.000009 mm |
| Field under lost wax at the 0.8 mm fill | **Castable**, thinnest wall 2.13 mm | **Castable**, 2.13 mm |
| `land_widths`: all named | yes (6.07 mm² under 0.8 mm, thinnest 0.034) | yes (7.24 mm², thinnest 0.0004) |
| DFM findings | 0 | 0 |
| Stones: report vs preview | 1 = 1 (174 preview faces) | 1 = 1 |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |
| Cold reload, empty library | — | identical vertices, faces and normals (686,724 / 1,373,452) |
| *Two-part numbers (not gated under lost wax)* | undercut 55.6 of 1344.5 mm², worst −59.4°; release 398 / 558 obstructions at 0.100 / 0.075, 0 unresolved | same areas; release 443 / 608, 0 unresolved |
| Draft clamp | none (lost wax, no clamp layer) | none |

The ring carries no stamps, so no 384 × 192 run was needed.

Export `land_widths` by feature. Each is read by one ray along a face's inward normal; each feature carries its bench treatment in the JSON.

| Feature | Area under 0.8 mm | Thinnest |
|---|---|---|
| The tail's tip | 2.46 mm² | 0.107 mm |
| The crest's spines | 2.46 mm² | 0.314 mm |
| Legs and toes | 1.45 mm² | 0.039 mm |
| Tail scale rings and saw | 0.62 mm² | 0.190 mm |
| Head: beak, eyes, lids, nostrils | 0.18 mm² | 0.174 mm |
| Body granules and tubercles | 0.07 mm² | 0.534 mm |
| The parietal seat (lip and bore vent) | 0.006 mm² | 0.0004 mm, a grazing ray on the vent wall. It did not change when I widened the seat from 3.3 to 3.4 mm. |

## Template gate (run after the last round; `collection_templates … --only sphenodon --verify-export`, class `painted`)

| Measure | Value |
|---|---|
| Source | lift, 26 nodes, 0 exposed controls |
| `design.set` patches | **1** (`/manufacturing`), limit 4 |
| Graph size | **995,177 bytes**, against the painted budget of 3 MB (no size review) |
| Cold source parity | identical (cold design and graph reload true) |
| Mesh parity | 1,373,452 triangles; vertices, faces and normals identical |
| Result | `template_gate_passed: true` (`template-verification.json`) |

## What changed in the revival, and why

**Round 4** (the round-3 punch list):
1. **Short, blunt skull.** The beak's tip is now at t = 3.0 (`SNOUT_T`), so the snout runs 4 mm ahead of the eyes. `GAP_MM` went from 9 to 6, so the polished gap between the tail tip and the snout is still 9 mm. The skull section is `(1−u²)^1.5·(1+1.2u²)`: a crowned top, tangent to the band at the outline, so there is no ledge. A notched beak lip stands 0.35 mm proud.
2. **Eyes.** Each is a smooth dome, tangent at its edge, with no ring groove. A crescent lid on the medial half fades at both corners, and a single lens-shaped pupil is 0.15 mm deep. The mouth groove now stays clear of the eye, which removed the "cross". Granules are cleared only on the eyeball, not in an annulus.
3. **Seat.** The seat shrank to a 0.4 mm rim round the stone, and the head's granules run up to it, so the bald oval is gone.
4. **Dorsal rows.** The fixed-pitch notched bars became domed, keeled scales on jittered Voronoi cells, graded into the granules. The crest's spines became separate teeth with a lower web, where before they formed a castellated blade.
5. **Body outline.** The outline fairs into the band over its last quarter, which stopped it stair-stepping with the grid.
6. **Docs.** `docs/collections/cataphracta.md` now has the Sphenodon section's Concept, Theme and table row as built.

**Round 5** (the round-4 punch list):
1. **Eyes outrank the stone.** The eyes are 1.9 mm across and 0.6 mm proud, on the side slopes (`EYE_X` 1.95), with 0.45 mm lids and a lateral socket shadow. The peridot went from 3.0 to **2.5 mm**, in a 3.3 mm seat, and moved back to `STONE_T` 11.0 at the skull's height peak, 4 mm behind the eyes. The head's knots moved back to match: the jaw joint is at 11, the neck at 12.6–14.4, and the crest starts at 13.5.
2. **Beak.** The snout has a squarer tip and a wedge-flattened top. The lip hangs over the mouth line, which now runs to the tip, and the notch is 0.25 mm.
3. **Dorsal scales.** They are now 0.5 mm, lower, with weaker keels, and graded over about 1.5 mm.
4. **Head skin.** It is round 0.3 mm beads, with 0.42 mm plates between the eyes.
5. **Tail.** It is overlapping keeled scales, each rising from the front and dropping at a rounded back edge, staggered ring to ring.

I zoomed every render to 2x before each review. The pupil and lid stair-steps, the head-grain combing, the snout's boxy front wall and the tail scales' hard back edges were found that way and softened.

## What I could not do

- **Name the animal at 300 px.** Two rounds moved the score from 7.1 to 7.4. The two remaining reads are both about the concept: a green stone on a lizard's head reads as an eye from the side, and the crest reads as osteoderms rather than a tuatara's soft comb. A flush or 2.0 mm stone set further back, and a crest of thin blades, are the next moves.
- **Use the platform's crisper relief edges.** Master did not move tonight, so the round-5 merge brought nothing.
- **Keep every single-ray section at 0.3 mm or more.** Rays near the foot of low relief, and the vent wall at the bore, read near-zero sections where nothing free-standing is thin (see core change 2 below).

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
