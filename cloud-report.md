# Sphenodon, the parietal (`sphenodon`): revival report, lost wax

**Outcome: cut for good at 7.4 after round 6.** The ring was revived from its round-3 cut (7.1). Round 4 scored **7.3** (revise) and round 5 **7.4** (cut). The lead then granted one more round, round 6, which also scored **7.4** (cut). The ship bar is 7.5. Every gate is green at draft and at export, the cold reload is identical, and the template gate passes. The ring is cut on its read, not on manufacture. The round-6 reviewer called the bladed crest and the domed, browed eyes the most tuatara-like the ring has been. But in every three-quarter view the green stone still lands where a lizard's eye sits, so at 300 px it reads as "a lizard with a green eye".

- Branch: `claude/cataphracta-sphenodon-revival`, from `claude/cataphracta-sphenodon` with `origin/master` merged in (at `8e5a59a`). The only conflict was `cloud-report.md`; I kept this lane's version. `src/` was not touched.
- Master did not move during the revival. The merges at the start of rounds 5 and 6 were both no-ops, so the "crisper stamp and relief edges" platform work never reached this ring.
- Author file: `crates/ringdesign-core/examples/cataphracta_sphenodon.rs`. Outputs: `showcase/cataphracta/sphenodon/`.
- Reviews: `review-round4.json`, `review-round5.json` and `review-round6.json`. Each came from a fresh reviewer agent given only `target/review.md`, the ring's name and slug, "full-review mode" and the round number. Round 6's reviewer was also told it was the revival's last round.
- Process: lost wax (Logan, 2026-09-27), with `min_section_mm` 0.8 and `min_draft_deg` 0. Silver 925, investment recipe.

## Verdicts, all rounds (verbatim in the JSON files)

| Round | File | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|---|
| Block-out 1 | `read-test-1.json` | reads: true | — | "Lizard ring" at once, but not yet a tuatara. |
| 1 | `review-round1.json` | revise | 6.5 | Goggle eyes on a lozenge head, a square-paver tail, combing at the rim. |
| 2 | `review-round2.json` | revise | 6.9 | The tail was fixed. A slab head with screw-head eyes, knob feet, and square crest spines. |
| 3 | `review-round3.json` | cut | 7.1 | A crowned skull and toed feet. The eyes still read as screw heads, the snout was 6.5 mm (monitor-like), a bald oval surrounded the stone, and the dorsal rows read as tread. |
| **4** | `review-round4.json` | **revise** | **7.3** | Four of the five round-3 items passed: lidded dome eyes with no ring or cross, a 4 mm snout, skin up to a 0.4 mm rim, and the collection copy fixed. New faults: the 3 mm stone read as the lizard's eye from three-quarter views, there was no beak overhang in profile, the keeled scales formed straight osteoderm files, the head skin had crocodile tiles, and the tail was a corn cob. |
| 5 | `review-round5.json` | cut | 7.4 | Round 4's items were mostly done. The eyes are larger than the stone, the 2.5 mm stone sits back on the crown, the beak is squared and notched, the head is finely granular, and the tail is staggered. It still misses on three counts: in the profile view (`stones.png`) the stone still sits where an eye sits; the lid and slit read as a "coffee bean"; and the crest from the nape reads as square pyramids, with a stud grid at the tail's rim edge. |

| **6** | `review-round6.json` | **cut** | **7.4** | Two items passed. The eyes are smooth 2.0 mm domes, each with a brow over its top third and a short slit pupil, and nothing reads as a bean. The crest is a comb of thin, separate, tall spines, the best tuatara cue yet. Two failed. The flush 2.0 mm stone, now 5 mm behind the eyes on the midline, still lands on the head's visible flank in three-quarter view, where an eye sits; with the eyes forward, they read as nostrils. The tail's rim edge is still a stud grid, and from above the crest still shows square pyramids. |
Round 6's punch list is recorded in its JSON, in case the ring is ever revived again:
1. Move the stone about 3 mm further back onto the nape, or countersink it 0.3 mm with a granulated collar, and bring the eyes back about 1 mm.
2. Use jittered tail scales lowered 60% at the rim edge.
3. Make the crest blades 0.35 mm across, leaning 15° tailward.
4. Give the seat a 0.15 mm bearing shoulder, to shrink its knife-edge lip.

## Gates, final build (round 6; `report.json`: `draft` block 768 × 320, `export` block 1536 × 448 with `--verify`)

| Gate | Draft | Export |
|---|---|---|
| Triangles | 491,438 | 1,374,112 (limit 2 M) |
| Watertight, degenerate faces | yes, 0 | yes, 0 |
| Self-crossings (ring; no made parts) | 0 | 0 |
| Solids and parts notes, stamps | empty, empty, 0 of 0 | empty, empty, 0 of 0 |
| Finger hole: vertices inside, nearest margin | 0, −0.000044 mm | 0, −0.000015 mm |
| Field under lost wax at the 0.8 mm fill | **Castable**, thinnest wall 2.13 mm | **Castable**, 2.13 mm |
| `land_widths`: all named | yes (20.89 mm² under 0.8 mm) | yes (23.24 mm²) |
| DFM findings | 0 | 0 |
| Stones: report vs preview | 1 = 1 | 1 = 1 |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |
| Cold reload, empty library | — | identical vertices, faces and normals (687,054 / 1,374,112) |
| *Two-part numbers (not gated under lost wax)* | undercut 59.6 mm² (4.46%), worst −67.4°; release 405 / 521 obstructions at 0.100 / 0.075, 0 unresolved | same areas; release 384 / 518, 0 unresolved |
| Draft clamp | none (lost wax, no clamp layer) | none |

The ring carries no stamps, so no 384 × 192 run was needed. Every gate was also green at draft and at export in rounds 4 and 5; see those commits' `report.json`.

Export `land_widths` by feature, one ray along each face's inward normal. Each feature carries its bench treatment in the JSON.

| Feature | Area under 0.8 mm | Thinnest |
|---|---|---|
| The crest's spines (thin blades, 1.5× as many as round 5) | 12.71 mm² | 0.111 mm |
| Tail scale rings and saw | 3.54 mm² | 0.207 mm |
| The parietal seat (flush lip, burnished over the girdle) | 2.92 mm² | 0.0002 mm |
| The tail's tip | 2.19 mm² | 0.095 mm |
| Legs and toes | 1.64 mm² | 0.031 mm |
| Body granules and tubercles | 0.14 mm² | 0.546 mm |
| Head: beak, eyes, lids, nostrils | 0.09 mm² | 0.126 mm |

The land-width area grew from 7.24 mm² in round 5 to 23.24 mm². The thin crest blades the punch list asked for account for most of the rise, and sinking the stone flush left a knife-edge lip at the seat. All of it is named, so the gate is green, but the reviewer flagged the seat lip as a burnishing cost.

## Template gate (run after the last round; `collection_templates … --only sphenodon --verify-export`, class `painted`)

| Measure | Value |
|---|---|
| Source | lift, 26 nodes, 0 exposed controls |
| `design.set` patches | **1** (`/manufacturing`), limit 4 |
| Graph size | **994,836 bytes**, against the painted budget of 3 MB (no size review) |
| Cold source parity | identical (cold design and graph reload true) |
| Mesh parity | 1,374,112 triangles; vertices, faces and normals identical |
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

**Round 6** (the round-5 punch list):
1. **Stone.** The peridot went from 2.5 to **2.0 mm** and moved back to `STONE_T` 12.0, 5 mm behind the eyes. The head's back knots (jaw joint, neck and crest start) moved 1 mm aft to match. The seat is 2.8 mm across and 1.75 mm high, with the girdle 0.45 mm down (`set_depth_mm`), so the table sits flush with the skull's skin in `side.png`.
2. **Eyes.** Each is a full 2.0 mm dome, 0.6 mm proud, moved up the side slope (`EYE_X` 1.75). The brow crescent covers only the top third and stands 0.4 mm proud. The pupil runs 60% of the eye's height and is 0.12 mm deep.
3. **Crest.** The back has 1.5× as many spines (0.95 mm apart). Each is a steep-flanked blade 0.56 mm across at its foot and about 0.75 mm long, with a gap before the next and a near-zero web. Heights vary about 15% from spine to spine. The tail keeps its old, sparser saw.
4. **Tail.** Scales are 40% lower at the rim edge, with softer, rounder overlaps and a lighter keel.

I zoomed every render to 2x before each review. The pupil and lid stair-steps, the head-grain combing, the snout's boxy front wall, the tail scales' hard back edges and, in round 6, the crest's square-pyramid spines were found that way and fixed.

## What I could not do

- **Take the stone out of the eye's place.** Three rounds attacked it: 3.0 mm, then 2.5 mm, then a flush 2.0 mm stone, moved from 2.8 to 4 to 5 mm behind the eyes. From above it now reads as a parietal jewel. In three-quarter view the skull's flank still presents it where an eye sits. Moving the eyes forward and up to clear it made them read as nostrils. The concept itself (a stone on a lizard's head, seen from the hero camera) fights the read. The next move would be the round-6 punch list: put the stone on the nape, behind the skull's widest point, or countersink it under a granulated collar.
- **Make the tail's rim edge irregular enough.** The tail scales sit on a ring and column lattice. Lowering and rounding them helped from above (`palm.png`), but over the rim fillet (`reverse.png`) the lattice still reads as studs. Jittered cells, like the dorsal scales use, would be the fix.
- **Use the platform's crisper relief edges.** Master did not move during the revival.
- **Keep the land-width area low with thin crest blades and a flush seat.** Both were punch-list asks, and both trade section for read. Everything thin is named, but the area grew to 23 mm².

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
