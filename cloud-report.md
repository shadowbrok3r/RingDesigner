# Heloderma — the beaded one: cloud lane report (lost wax, a figure)

**Outcome: stopped at step 0 for the third time. The figure block-out failed all three read tests. TASK.md says that when the third block-out still does not read, the lane stops and reports: the subject needs rethinking, not detailing.**

- **Rounds used:** 0 of 3. There were no full reviews, so there is no score and no ship, revise or cut verdict.
- **Block-out attempts:** 3 of 3, counted from 1 again for the figure, as TASK.md says. The two earlier lanes (sand, then pattern-only lost wax) are archived in `showcase/cataphracta/heloderma/sand/` and `showcase/cataphracta/heloderma/lost-wax-pattern/`.
- **Branch:** `claude/cataphracta-heloderma`.
- **Files:**
  - Author file: `crates/ringdesign-core/examples/cataphracta_heloderma.rs`.
  - Renders, `report.json`, `template-verification.json` and the read tests: `showcase/cataphracta/heloderma/`. The renders and `report.json` are from block-out 3, at draft resolution.
  - The figure has no SVG artwork. It is a distance field sculpted in the example itself. The ground's eight sector SVGs are in `crates/ringdesign-core/examples/cataphracta/art/heloderma/`.

## What was built

Logan's third start: the Gila herself, not texture alone. **One sculpted part** (`ringdesign_core::sculpt`), stored in the design as `Operation::Stored` (`Attach::Join`, `Stage::Cast`, `Placement::Free`). It holds:
- the blunt head with its eyes, brows, mouth line and nostrils;
- a thick neck with a crease behind the head;
- a fat trunk;
- four short splayed legs, each with five toes and claws;
- a fat, round-tipped tail, the whole animal lying along the crown.

The spessartite sits in front of the snout.

- **The field:**
  - The head is rigid, in a frame at its own crest point.
  - The body, legs and tail lie in a frame bent round the band: `u` is arc mm at the crown's radius, `h` is height over the bare band. The band is read from its own outward faces, rasterized per (θ, z).
  - The black bands are high domed beads; the salmon bands are low tiles sunk into the same envelope. Both are 3D Voronoi bead fields.
- **The mesh:** `tetra_mesh` at a 0.057 mm step, then `relax`, `decimate`, `settle`.
  - About 303,000 triangles and about 550 mm³.
  - Closed, with 0 self-crossings, also after the 10 nm quantization of the stored mesh.
- **The ground:** eight `DecalLayer` sectors of fine domes, 0.07 mm high at a 0.6 mm pitch, kept 0.6 mm clear of the animal's outline (a polished moat) and of the stone's mound.

| Layer or part | What it is | Why |
|---|---|---|
| Base | HalfRound 8.0 × 3.2, edge round 0.3, comfort fit 0.2, bore 18.6. Keys (width / thickness / crown): 1.42 / 1.00 / 1.05 at 90°, 1.20 / 1.00 at 35°, 1.32 / 1.00 at 145°, 1.26 / 0.98 at 200°, 0.92 / 0.90 at 270°, 1.05 / 0.95 at 330° | A crown 11.4 mm wide, room for the animal and its splayed feet |
| Gila beadwork I–VIII | The ground: one `DecalLayer` per 45° sector, one decal each, fine low domes (land 0.08 × pitch) | A quiet beaded ground, so the animal is not on bare stock (read test 1) and stands clear of it (read tests 2 and 3) |
| Spessartite | 3.0 mm round, preview tint (0.95, 0.38, 0.06), at θ 49°. `GypsyMound` 0.45 mm, sunk 0.35 mm, `SolidKind::Bezel` collet, through, 0.5 mm drill dot | The egg the Gila noses: a stone in a made setting in front of the snout |
| Gila (stored part) | The sculpted animal described above | The figure that names the animal |

## Read tests

Each test used a fresh, independent reviewer in read-test mode. It was given only `target/review.md`, the ring's name and slug, the attempt number, the paths, and the process (lost wax). The full JSON is in `showcase/cataphracta/heloderma/read-test-{1,2,3}.json`.

| # | Block-out | Reads | What the eye sees (the reviewer's words, shortened) | Changes asked for |
|---|---|---|---|---|
| 1 | A long lizard along the crown and over the shoulder. Voronoi beads with 0.2 mm relief bands. Plain band | **false** | "A lizard crawling over a plain polished half-round band … uniformly pebbled, like a toad or a generic lizard … the left third is a lumpy, ridged mass … the head is small and hard to separate from the forelegs … 'lizard', perhaps 'baby croc' or 'gecko', but not Gila monster." | 1. Banding: high against low bead bands, 4 on the trunk and 4–5 on the tail. 2. A short, fat, blunt tail lying on the face. 3. A broad, blunt head clear of the legs, stubby legs, and low beads on the bare face with the stone in the bead field |
| 2 | A shorter animal centred on the face. Banding carried by bead type (round domes against flat tiles). A fine ground, 0.07 mm | **false** | "A lumpy lizard or small crocodile … the tail's bands show up as raised transverse ridges … a segmented crocodile or armadillo tail … the trunk shows no banding … the head cannot be picked out … the ground beads are about the same scale and brightness as the lizard's beads … 'lizard' or 'crocodile on a textured band'." | 1. Stripes of bead height inside one smooth envelope, with no step over 0.10 mm. 2. One unmistakable head: square snout, 0.3 mm crease. 3. A finer ground or a polished moat |
| 3 | A square-snouted head wedge with a crease and big eyes. One envelope, with 0.40 mm domes at a 0.95 mm pitch against sunk low tiles. A 0.6 mm polished moat | **false** | "In hero-300 a knobbly lizard-like lump with splayed legs … 'some small reptile crawling on a ring' … in face-300 it stops reading as an animal: a raised strip of clustered rosettes, like cauliflower, coral or a pine-cone chain … no head can be pointed to … lizard and ground are the same gold, similar bead scale and similar brightness." | 1. One smooth silhouette with the detail inside it: no knob over 0.10 mm proud, a 0.3 mm undercut shadow line on the flank. 2. Banding by depth: the dark bands sunk 0.25–0.30 mm and filled with fine beads, against 1 mm polished domes. 3. A quieter ground and a 0.6–0.8 mm moat |

## What I learned

**The figure fixed the category but not the species.** Every reviewer now names a reptile, where the pattern-only lanes named an urchin or caviar six times out of six. What never arrived was "Gila":
- Smooth, the animal reads at once as a lizard (an unbeaded draft before block-out 1), but as a generic one: gecko, salamander or croc.
- Beaded, the beads are what should make it a Gila, but at 300 px they destroy the outline.
- The tension is the same one the earlier lanes met, now on the figure: in one gold, the Gila's identity is colour (black against salmon), and relief strong enough to carry the colour breaks the silhouette.

| Carrier of the banding | Attempt | Effect at 300 px |
|---|---|---|
| Relief bands as envelope steps (0.2 mm, then 0.3 mm) | 1 and a variant | The bands read as segments: a caterpillar, an armadillo, a croc's tail |
| Bead type (round domes against flat tiles) at a 0.56–0.72 mm pitch | 2 | No stripes; the body reads as one knobbly lump |
| Big domes (0.40 mm, 0.95 mm pitch) against sunk tiles in one envelope | 3 | "Rosettes", "cauliflower", "pine cone" |

Findings worth keeping:
- **The ground competes with the figure.** A dense bead ground at a similar scale camouflages the animal, the way a real Gila hides on gravel. The polished moat made the outline traceable at full resolution, but not at 300 px while the animal itself was knobbly.
- **The head at 300 px needs a smooth mass and two bright eyes.** Beads on the head hide it. The smoothest head (block-out 3) came closest to being picked out, and the reviewer still could not.
- **Sculpting in lost wax works.** The chain is deterministic and every lost-wax gate is green. The notes below are the pitfalls met on the way.

### Technique notes for the next attempt

- **Decimation can cross itself at one or two sites.** The sites move with the meshing step, so the example tries four steps a hair apart, each at three budgets, before `clean_decimate`.
- **The stored mesh's 10 nm grid can make two settled faces cross.** A local Laplacian smooth of the vertices within 0.06 mm of the site fixes it, re-checked on the quantized mesh (`unfold_stored`).
- **A figure that grazes the band will not join** ("labels disagree across an edge"). A fused fillet meeting a plain, moated band at a shallow angle failed at every micro-nudge. Sinking every contact (toes, pads, throat, belly) at least 0.1 mm into the band, with no fillet, joins cleanly at both builds. The example still retries a join with micro-nudges and checks it at draft and at export.
- **The sculpt takes about 3 to 5 minutes.** The example caches it under `target/`, keyed by the hash of the band's keys and the figure's own source section.

## Gates, block-out 3

The process is lost wax, with `min_section_mm` 0.8 and `min_draft_deg` 0. The draft column is from `showcase/cataphracta/heloderma/report.json`. The export column is from a `--verify` run at 1536 × 448, whose report is kept as `report-export.json`; the showcase keeps the draft renders the reviewers judged.

| Gate | Draft (768 × 320) | Export (1536 × 448) |
|---|---|---|
| Watertight, 0 degenerate faces | pass (718,644 triangles, 0 boundary and 0 non-manifold edges) | pass (1,555,580 triangles, 0 boundary and 0 non-manifold edges) |
| 0 self-crossings on the ring and on the figure | pass (0 and 0) | pass (0 and 0) |
| The figure joined; solids and parts notes empty; every stamp resolved | pass (1 joined, no notes, no stamps) | pass (1 joined, no notes) |
| Nothing in the finger hole | pass (closest vertex 9.2999 mm against a 9.30 mm bore, 0 inside) | pass (9.2999 mm, 0 inside) |
| Lost-wax field verdict Castable with the 0.8 mm fill | pass (Castable, thinnest wall 2.08 mm) | pass (Castable, 2.08 mm) |
| `land_widths` | See the list below the table | same |
| 0 DFM findings | pass (0) | pass (0) |
| Stones reported = previewed | pass (1 = 1) | pass (1 = 1) |
| Casting pattern closed | pass (watertight, 0 degenerate, 0 crossings) | pass (1,555,580 triangles, watertight, 0 degenerate, 0 crossings) |
| Within 2 million triangles | pass | pass (1,555,580) |
| `--verify` cold reload with an empty library | — | pass: identical vertices, faces and normals |
| Two-part undercut (a number, not a gate) | 1.59 %, worst −9.9°. The stored figure alone would undercut a two-part pull over about 60–76 mm² | 1.59 %, worst −9.9° |

**`land_widths`,** recorded and named. Nothing under the floor is removed.
- **Ground beads:** the finest full bead is 0.38 mm, against the 0.15 mm detail floor.
- **Collet:** 0.18 mm at the lip, 26.2 mm² under the floor. It is the standard 3 mm collet, with its bench treatment named.
- **Figure:** `dfm::part_sections` reads 132 mm² of its 686 mm² under the 0.8 mm floor.
  - This is the relief: rays that cut short chords through the 0.4 mm bead domes and across their seams, plus the claw points and eye moats.
  - The load-bearing sections are over the floor, by construction: toes 0.82 mm or more, limbs 1.2 mm or more, trunk and tail 2.6 mm or more.
  - This is named in `report.json` but not proven per feature, because `part_sections` cannot tell a chord from a section (see "Core changes wanted").

## Template gate

Run after the last step, although no round ran, so its numbers are on record for the rethink. It uses the showcase design, `template_class` painted.

- `design.set` patches: **1** (`/manufacturing`). The limit is 4.
- Graph: **3,468,573 bytes**, over the painted class's **3,000,000-byte** budget. **The gate fails on size** (`template_gate_passed: false`, "review required").
  - The cause is the stored sculpt: its 303,000 triangles are most of the 3.46 MB design.
- Cold source parity: **passed** (`source_identical`, cold design and graph reloads).
- Mesh parity: **passed**. Export geometry verified, with vertices, faces and normals identical at 1536 × 448 (1,555,580 triangles).
- The numbers are in `showcase/cataphracta/heloderma/template-verification.json`.

## What I could not do

- Make a figurative Gila read as a *Gila*, not a generic lizard, at 300 px in plain gold, in three attempts.
- Carry the black-and-salmon banding in relief without it reading as body segments (steps) or as clutter (beads).
- Keep the stored sculpt inside the painted template budget at its current density.
- Prove the figure's sections per feature.

## Recommendation for the rethink

1. **Keep the figure, and make it smooth.** Take read test 3's first point literally: one smooth, undercut silhouette (lost wax allows a 0.3 mm shadow line under the flank), with the detail inside the outline and nothing more than 0.10 mm proud. The one unbeaded draft that read as "a lizard" at once was smooth.
2. **Carry the banding by depth, not by bumps.** Sink the dark bands 0.25–0.30 mm into the envelope as troughs filled with fine low beads, and keep the light bands as smooth polished plateaus. In plain gold, a sunk band holds shadow (or oxide at the bench) the way the Gila's black does. It is the one carrier not yet tried on the figure.
3. **Quiet the ground to satin, or leave it plain polished,** with the animal the only relief on the face.
4. **Or give up plain gold for the colour.** Blackened silver in the sunk bands against polished salmon plateaus is the Gila, literally. It needs a second finish in `render::finished` to be reviewed.
5. **Budget:** decimate the stored figure to about 200,000 triangles, or store a lighter smooth silhouette and carry the beads as a procedural layer, to fit the 3 MB painted class.

## Core changes wanted

None were needed: the block-out ran on master as it stands (`sculpt`, `cad::stored`, `DecalLayer`). Two changes would help the next attempt.

**1. A second metal finish in `render::finished`,** for recommendation 4. It is too open a design question to write as exact code here.

**2. `dfm::part_sections` should not count relief chords as sections.** Today a ray from a bead's flank that crosses the bead's cap and leaves through the next seam reads as a thin section. The proposed change is in `crates/ringdesign-core/src/dfm.rs`, in `part_sections`. It is untested. It skips a hit whose exit face turns back toward the entry face within the floor, which is a chord through relief and not a section through the part:

```rust
// replaces: let Some((_, t)) = bvh.ray(&mesh, o, inward) else { continue };
let Some((hit, t)) = bvh.ray(&mesh, o, inward) else { continue };
let section = t + IN;
if section < floor_mm {
    let [p, q, r] = solid.f[hit].map(|i| solid.v[i as usize]);
    let m = [
        (q[1] - p[1]) * (r[2] - p[2]) - (q[2] - p[2]) * (r[1] - p[1]),
        (q[2] - p[2]) * (r[0] - p[0]) - (q[0] - p[0]) * (r[2] - p[2]),
        (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]),
    ];
    let ml = (m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt().max(1e-14);
    // A true section leaves through a face turned away from the entry (exit normal along the ray);
    // a chord through a dome or across a seam leaves through one turned half back toward it.
    if (m[0] * inward[0] + m[1] * inward[1] + m[2] * inward[2]) / ml < 0.5 {
        continue;
    }
}
// (the existing `let section = t + IN;` line is then removed)
```
