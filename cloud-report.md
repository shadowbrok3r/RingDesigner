# Heloderma — the beaded one: cloud lane report (lost wax, the figure carried to three rounds)

**Outcome: cut at round 3 with a score of 6.3.** The scores were 5.8 in round 1, 6.4 in round 2 and 6.3 in round 3. The ship bar is 7.5. Every gate was green before each review from round 2 on, including at round 3.

- **Rounds used:** 3 of 3.
- **Why the lane ran rounds at all:** on 2026-09-28 Logan overrode the block-out read-test stop. The figure block-out had failed its three read tests on 2026-09-27. He asked for the best figure block-out to be carried through full rounds 1 to 3, with the last read test's changes as round-1 priorities.
- **Starting point:** figure block-out 3, whose hero read as "a reptile crawling on a ring". It is the one that had absorbed every read test's changes.
- **Branch:** `claude/cataphracta-heloderma`.
- **Files:**
  - Author file: `crates/ringdesign-core/examples/cataphracta_heloderma.rs`.
  - Sector SVGs: `crates/ringdesign-core/examples/cataphracta/art/heloderma/`.
  - Renders at export resolution, `report.json`, `template-verification.json`, the three reviews and the three read tests: `showcase/cataphracta/heloderma/`.
  - Earlier lanes are archived in `sand/` and `lost-wax-pattern/`.

## Verdicts

| Stage | Verdict | Score | The reviewer's core finding |
|---|---|---|---|
| Read test 1 (27 Sep) | reads: false | — | "A lizard, perhaps a baby croc or gecko, but not Gila monster" |
| Read test 2 (27 Sep) | reads: false | — | "A lizard or crocodile on a textured band"; the tail reads as segments, the head is lost |
| Read test 3 (27 Sep) | reads: false | — | "Some small reptile"; in the face view "clustered rosettes" |
| *Logan's override (28 Sep)* | | | Carry the figure to finished rounds |
| Round 1 | revise | 5.8 | Ray release and the draft clamp were unrecorded, and the template gate was stale and over budget. A bald face, straight hide cut-offs, a bulb head with pin eyes, the tail off the crest, stair-stepped band edges and tile seams |
| Round 2 | revise | 6.4 | Every gate green. It still reads as a lizard, not a Gila: a bulb head from above, a croc snout in profile, no countable bands. The face ground cut into blocks with a bald halo, and seams and ledges on the shank |
| Round 3 | **cut** | **6.3** | Every gate green. Of round 2's seven punch items, one is closed (the collet's beaded lip), one partly (the eyes) and five open. The head is still a bulb or croc snout, the trunk reads as "cracked-mud polygons" with no countable bands, the face ground is stepped blocks with bald areas, the shank shows sector walls and rim ledges, and the tail tip hangs off the crest |

The full JSON is in `showcase/cataphracta/heloderma/review-round{1,2,3}.json` and `read-test-{1,2,3}.json`. The verdict stands.

## What the final ring is

| Layer or part | What it is | Why |
|---|---|---|
| Base | HalfRound 8.0 × 3.2, edge round 0.3, comfort fit 0.2, bore 18.6. Keys (width / thickness): 1.42 / 1.00 at 90°, 1.20 / 1.00 at 35°, 1.32 / 1.00 at 145°, 1.26 / 0.98 at 200°, 0.92 / 0.90 at 270°, 1.05 / 0.95 at 330° | An 11.4 mm crown, room for the animal |
| Gila (stored part) | One sculpted part (`ringdesign_core::sculpt`), stored as `Operation::Stored` with `Attach::Join`, 210,000 triangles. Its parts are listed below the table | The figure that names the animal |
| Gila beadwork I–VIII | Eight `DecalLayer` sectors, each overlapping its neighbours by 0.6 mm, `SmoothMax` with a 0.05 mm soft. Their contents are listed below the table | The ground, and the hide carried round the shank as the tail's rings |
| Spessartite | 3.0 mm round, preview tint (0.95, 0.38, 0.06), at θ 42.5°, about 0.5 mm off the snout. `GypsyMound` 0.45 mm, `SolidKind::Bezel` collet sunk 0.35 mm, through, 0.5 mm drill dot | The egg the Gila noses, in a made setting |

The Gila part holds:
- **The head:** a flat wedge with a squared nose, jowls, a mouth groove, nostrils, and eyes sunk in lidded sockets under brows.
- **The body:** a creased neck, a fat trunk, and a tail curled toward the face.
- **The legs:** four splayed legs, each with five toes and rounded claws.
- **The hide:** 3D Voronoi beads. The salmon bands carry 1.0 mm beads that crown one envelope; the black bands sink 0.40 mm into it on 0.4 mm beads.
- **The collet's lip:** a ring of 24 beads.

The beadwork sectors hold:
- a fine ground of 0.10 mm beads at a 0.46 mm pitch everywhere, except within a 0.7 mm moat of the animal's contact outline (rasterized from the stored mesh) and the collet;
- on the shank, salmon plateaus 0.26 mm high with 0.12 mm domes, as transverse rings. They are drawn as marching-squares vector paths, feathered over 4.5 mm off the face and graded to nothing at the rims. The ring period divides the circumference.

## Gates, final design (round 3), from `report.json`

The process is lost wax, with `min_section_mm` 0.8 and `min_draft_deg` 0.

| Gate | Draft (768 × 320) | Export (1536 × 448) |
|---|---|---|
| Watertight, 0 degenerate faces | pass (665,574 triangles) | pass (1,512,536 triangles) |
| 0 self-crossings on the ring, the figure and the pattern | pass | pass |
| The figure joined; solids and parts notes empty | pass | pass |
| Nothing in the finger hole | pass (0 inside) | pass (0 inside) |
| Lost-wax verdict Castable with the 0.8 mm fill | pass (thinnest wall 2.11 mm) | pass (2.11 mm) |
| Land widths | pass, measured per feature (see below) | pass |
| Ray release at 0.100 and 0.075 mm | recorded; NotApplicable under lost wax | 1,143 and 1,852 two-part obstructions, 0 and 1 unresolved: reported, not gated |
| Draft clamp | none applied, bite 0 mm | same |
| 0 DFM findings | pass | pass |
| Stones reported = previewed | pass (1 = 1) | pass |
| Casting pattern closed | pass | pass |
| Within 2 million triangles | — | pass |
| `--verify` cold reload with an empty library | — | pass: identical vertices, faces and normals |
| Two-part undercut (reported, not a gate) | 2.64 %, worst −37.4° | same |

**Land widths,** per feature. Each value is twice the inscribed radius the sculpt's own field reads along the feature's axis:
- limbs 1.57 mm;
- toes 0.82 mm;
- neck 1.31 mm;
- tail's end 1.21 mm;
- claw tips 0.50 mm, which are rounded ends named against a 0.5 mm floor.

Two things are named under the floor rather than removed:
- **The figure's whole-part ray read:** 112 mm² under the floor. These are chords through the bead relief and seams.
- **The standard collet:** 0.18 mm at the lip, 26.2 mm² under the floor, burnished at the bench.

## Template gate (run on the final design, after round 3)

```
collection_templates cataphracta target/tpl-src --output-dir target/tpl --only heloderma --verify-export   (template_class painted)
```

- `design.set` patches: **1** (`/manufacturing`). The limit is 4.
- Graph: **2,848,705 bytes**, within the painted class's **3,000,000** budget. `template_gate_passed: true`.
- Cold source parity: **passed** (`source_identical`, cold design and graph reloads).
- Mesh parity: **passed**. Vertices, faces and normals are identical at export (1,512,536 triangles).
- It sits under budget because the stored sculpt was decimated from about 300,000 to 210,000 triangles in round 2. It passed there at 2,666,540 bytes.

## What each round changed

- **Round 1**, from the read tests' priorities:
  - The figure's hide became one envelope, with the salmon beads crowning it and the black bands sunk.
  - The jaw groove got deeper and the tail fatter.
  - The shank carried the tail's rings as beaded plateaus. The face was left as a polished field.
- **Round 2**, from punch list 1:
  - Ray release was recorded at both pitches and the draft clamp recorded as not applied.
  - The template gate was re-run and brought under budget by decimating the sculpt.
  - The whole animal was moved onto the face, with a flatter, broader head, lidded eyes and 0.5 mm rounded claw tips.
  - A per-feature section check was added.
  - The fine ground ran up to a 0.7 mm moat.
  - The plateaus became smooth vector paths, feathered, with sector overlap.
- **Round 3**, from punch list 2:
  - A square-nosed head wedge with coarse beads.
  - Black bands sunk 0.40 mm.
  - The tail tip swung sideways.
  - The stone moved to the snout, with a beaded collet lip.
  - The moat is measured from the mesh's contact outline.
  - The ring period divides the circumference, the decals blend with `SmoothMax`, and the plateaus grade to nothing at the rims.

## What I could not do

- **Make it read as a Gila, not a generic lizard or croc.**
  - The Voronoi bead field on the figure read as "cracked-mud polygons" to the last reviewer.
  - At 300 px the sunk black bands never became countable stripes.
  - From above the head stayed a rounded bulb, and in profile it looked crocodilian.
- **Make a continuous face ground.** The bead lattice clipped by a moat shows stepped block edges at bead scale, and the feet's spread-toe outlines leave bald patches.
- **Remove the shank's sector walls and rim ledges.** The reviewer still saw them after the overlap, the `SmoothMax` blend, the exact period and the rim grading. I did not find their cause within the round.
- **Keep the tail tip on the crest in profile.** Swinging it sideways was not enough.

## Recommendation, if Heloderma is restarted

1. **Model the hide as explicit bead rows, not Voronoi noise.**
   - Place domes along the spine's own (u, angle) grid, at 1.0 mm in the salmon bands and 0.4 mm in the sunk black bands.
   - Stop the bands at 3 on the trunk and 3–4 on the tail, each 1.5–2.0 mm wide, so they can be counted at 300 px.
2. **Build the head from a drawn plan and profile.** Use a sweep between a square-nosed top outline and a flat profile outline, not blended ellipsoids.
3. **Carry the face ground as one continuous painted height map,** with the moat as a distance falloff instead of per-bead culling. Put the shank hide on the same single map, so there are no sector joins.
4. **Or reconsider plain gold.** In gold, the Gila's black and salmon has resisted every relief carrier across four lanes. Blackened silver in the sunk bands is the literal Gila.

## Core changes wanted

None were needed for this lane. The two proposals from the earlier report still stand:
- **A second metal finish in `render::finished`,** so an oxidised-band version can be reviewed.
- **A relief-aware `dfm::part_sections`.** This lane worked around it with the per-feature field measurement described above.
