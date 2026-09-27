# Heloderma — the beaded one: cloud lane report (lost wax)

**Outcome: stopped at step 0 again. After Logan's switch to lost wax (2026-09-27), the block-out failed all three read tests. TASK.md says that when the third block-out still does not read, the lane stops and reports. The subject needs rethinking, not detailing.**

- Rounds used: **0 of 3.** There were no full reviews, so there is no score and no ship, revise or cut verdict.
- Block-out attempts: **3 of 3 in lost wax.** The count restarted at 1, as TASK.md says. The earlier sand lane's three read tests also failed; they are archived in `showcase/cataphracta/heloderma/sand/`.
- Branch: `claude/cataphracta-heloderma`.
- Files:
  - Author file: `crates/ringdesign-core/examples/cataphracta_heloderma.rs`.
  - Artwork: `crates/ringdesign-core/examples/cataphracta/art/heloderma/gila-beadwork-{i..viii}.svg`.
  - Renders, `report.json` and the read tests: `showcase/cataphracta/heloderma/`. The renders and `report.json` are from block-out 3, at draft resolution.

## Read tests

Each test used an independent reviewer in read-test mode, given only `review.md`, the ring's name and slug, the attempt number, the paths, and the process (lost wax, with the lost-wax gates).

| # | Reads | What the eye sees (the reviewer's words, shortened) | Changes the reviewer asked for |
|---|---|---|---|
| 1 | **false** | "A plain polished half-round band … patches of granulation … blocky vertical clusters with bald, mirror-smooth metal between them … granulation work, a sea-urchin or blackberry texture … 'granulated band with a citrine'." | 1. Bead the whole hide, with no bald metal. 2. Carry the banding by height: high beads 0.36 mm, low cushions 0.24 mm, in organic forking bands 3–5 beads wide. 3. A dorsal row along the crest. Grade the beads from shingles at the crest to domes at the edges. |
| 2 | **false** | "The whole band is now beaded … still a sea-urchin shell or a caviar/granulation band … separate round pearls on visible land … fields of big pearls against fields of fine pearls, in rectilinear blocks … rows of larger beads fan out diagonally from the stone in an X … the ambulacral star of an urchin test." | 1. One uniform lattice, with the banding carried by height and shadow. Irregular forking bands, and nothing radiating from the stone. 2. Close the lands. Near the crest, touching polygonal shingles; full domes only in the outer third. 3. A dorsal spine at least 1.5 × its neighbours through the stone, and a fatter swell. |
| 3 | **false** | "Covered all over in round gold pearls … still reads as a caviar or granulation band, or a sea-urchin shell. The beads form rings around the bezel, so the stone reads as the urchin's boss … the 0.12 mm step between high and low beads makes no shadow at this size, and in plain gold nothing else can carry the Gila's black and salmon bands … 'granulated dome band with a citrine'." | 1. Show the banding by a relief step of at least 0.35 mm, with the high bands on a plateau or the low fields sunk. 2. Replace the bead rings round the bezel with a dorsal spine, and bead right up to it. 3. Push the swell to 1.35 / 1.42. Tight polygonal shingles near the crest. |

The full JSON is in `showcase/cataphracta/heloderma/read-test-{1,2,3}.json`.

## What was tried, and what I learned

**Lost wax solved the sand problem and exposed the real one.** Beads now stand full and round anywhere on the dome, and every lost-wax gate is green at draft. Six reviews, three in sand and three in wax, have now given the same answer: in plain studio gold, a pattern-only Gila reads as a sea urchin, caviar, or granulation. The hide's identity is its colour, black against salmon. Every way of carrying that colour in one metal was tried, and each lost either the bands or the animal:

| Carrier of the banding | Block-out | What it did at 300 px |
|---|---|---|
| Tall beads against smooth ground | 1 (and an unsubmitted variant) | **The bands read clearly**, as bold transverse, forking bands. The ring read as "granulation clusters on a plain band", not as hide. |
| Height alone, one lattice (0.40 against 0.20 mm, then 0.44 against 0.16) | tried before 2 | Collapsed to one even "golf ball" texture. The reviewer confirmed this in test 3: a 0.12 mm step casts no shadow at 300 px. |
| Round domes against flat, fused shingles | tried before 2 and in 3 | A pebbled hide that reads as skin, but the bands vanish. |
| Scale: big domes against fine granulation (half pitch) | 2 | The bands show, but as "blocks" of big and fine pearls: urchin, caviar. |
| Separate shadowed domes against a fused, seamed salmon skin, a dorsal spine 1.55 × its neighbours, swell 1.30 / 1.38 | 3 | The spine shows in the face view, but the bands are still too faint. The circular clearing round the mound reads as the urchin's boss. |

The reviewers' asks pull against each other:
- Test 1 wanted bald metal gone, but bald metal is the one thing that made the bands legible.
- Test 3 now asks for a 0.35–0.45 mm plateau step. That is roughly block-out 1's contrast, carried by a step instead of by bead presence. It is the most promising next move. It was not tried, because it arrived with the last test.

### Findings worth keeping

- **Whole beads, placed one by one, with crisp band outlines.** The hide is drawn as one bead list (row by row, graded, jittered, metal-true through `arc_scale`, `station_stretch` and `crest_scale`). It is rasterized as 8 sector SVGs shown by 8 one-decal `DecalLayer`s, so every bead is whole: a mask never cuts one. Each 1024 px sector raster holds a bead to 0.01 mm. The DFM decal check measures them (0 findings). This technique carries over to any bead-pixel hide.
- **Fusing a field into a skin.** Drawing the low beads at full ink inside one SVG `<g opacity=h>` gives a flat plateau at `h` with a seam at each bead's edge. SVG compositing never stacks their overlaps above `h`.
- **A circular clearing round the stone reads as an urchin boss.** Any next attempt should run the pattern, or a spine, up into the setting.
- **A 3 mm stone's standard collet** (`setting::collet_wall_mm` = 0.52 mm) is under the 0.8 mm fill floor. `dfm::part_sections` reads 0.18 mm at its lip, with 26.2 mm² of its surface under the floor. `report.json` names it (lip burnished, wall sunk 0.35 mm into the mound). A round-1 build would need a custom 0.8 mm collet, as Manticora's is.

## Layers, parts and stone (block-out 3)

| Layer or part | What it is | Why |
|---|---|---|
| Base | HalfRound 8.0 × 3.2, edge round 0.3, comfort fit 0.2, bore 18.6. Keyframes 1.30 / 1.38 / 1.05 at 90°, 1.08 / 1.12 at 35° and 145°, 0.96 at 210° and 330°, 0.90 / 0.90 at 270°. The sand-era ogive crown was dropped | The fat tail; lost wax needs no ogive |
| **Gila beadwork I–VIII** | One `DecalLayer` per 45° sector, each a single decal of its own SVG, 0.50 mm full ink. Contents: the black bands' domes (0.38 mm, land 0.24 × pitch, half-domes near the crest, full domes in the outer third); the salmon bands' cushions, fused into one 0.22 mm seamed skin; and the dorsal row (1.55 × its neighbours, 0.46 mm) on the crest. Pitch 1.25 mm at the face, graded Cosine 0.26 to the palm. Nine bands of uneven width that lean, bow, fork (every third) and are strapped (every fourth gap) | The Gila's black and salmon beadwork |
| **Spessartite** | 3.0 mm round, preview tint (0.95, 0.38, 0.06), on a `GypsyMound` 0.45 mm tall, sunk 0.35 mm, in a `SolidKind::Bezel` collet, `through`, with a 0.5 mm drill dot | The salmon bead on the spine, in a made setting |

There are no stamps and no CAD parts besides the collet.

## Gates at draft (768 × 320), block-out 3, from `report.json`

The process is lost wax, with `min_section_mm` 0.8 and `min_draft_deg` 0.

| Gate | Result |
|---|---|
| Watertight, 0 degenerate faces | pass (493,248 triangles, 0 boundary and 0 non-manifold edges) |
| 0 self-crossings | pass (0) |
| Solids and parts notes empty; every stamp resolved | pass (no stamps) |
| Nothing in the finger hole | pass (closest vertex 9.29997 mm against a 9.30 mm bore, 0 inside) |
| Lost-wax field verdict Castable with the 0.8 mm fill | pass (Castable, thinnest wall 2.10 mm) |
| `land_widths` | recorded. Finest full bead 0.62 mm against the 0.15 mm detail floor. The collet is under the 0.8 mm floor (0.18 mm at the lip, 26.2 mm²) and is named with its bench treatment; see above |
| 0 DFM findings | pass (0) |
| Stones reported = previewed | pass (1 = 1) |
| Casting pattern closed | pass (watertight, 0 degenerate, 0 crossings) |
| Two-part undercut (a number, not a gate) | 6.93 %, worst −44.9° ("Will not release" if it were poured in sand) |
| Export build (1536 × 448), `--verify` | not run: step 0 renders at draft, and detailing never started |
| Template gate | not run: TASK.md runs it after the last round, and the lane stopped before round 1. `design.ring.json` is 142.9 KB with 8 embedded sector SVGs (painted class, 3 MB budget) |

## What I could not do

- Make a pattern-only Gila read at 300 px in plain gold, in three lost-wax attempts; three sand attempts failed before them.
- Satisfy "the whole hide beaded" and "legible black and salmon bands" at once without a relief step. Test 3's plateau step was not tried, because the loop allows no fourth block-out.

## Recommendation for the rethink

1. **Carry the bands on a relief step.** Take read test 3's point 1: the black bands' beads on a plateau 0.35–0.45 mm proud of the salmon fields, whose beads lie low and fused. Block-out 1 showed that bands read at 300 px when the two states differ that much. The step keeps the hide fully beaded.
2. **Give it a figurative anchor.** Six reviews have named an urchin. A Gila head at the face (faces are allowed), lying along the crest with the swell behind it, would name the animal. The beadwork would then say which lizard. The Bestiarium shipped only on iconic silhouettes (Arachne, Manticora).
3. **Or give up plain gold for the colour.** Oxidized (blackened) silver in the black bands with polished salmon beads is the Gila, literally. It is a bench finish, not a mould feature, and the studio-gold renders cannot show it today.

## Core changes wanted

None. The block-out ran on master as it stands: `DecalLayer`, `SvgAlpha`, `TileGrade`'s `phi` and `x_of_phi`, and the `FieldContext` scale tables. The one change I would want for recommendation 3, a second metal finish in `render::finished`, is too open a design question to write as exact code here.
