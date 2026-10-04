# Cataphracta: Phrynosoma, *the horned crown*: cloud report

**Outcome: cut at 7.2 after round 3 of 3.** The ship bar is 7.5. Every gate is green at draft and at export, and the template gate passes.

| | Result |
|---|---|
| Read tests | 5 (3 on 016 Star, 2 on 004 Shield); read on test 5 |
| Rounds used | 3 of 3 |
| Scores | 6.7 (revise), 7.1 (revise), **7.2 (cut)** |

**Where things are**

- Branch: `claude/cataphracta-phrynosoma`, with master `0c7c8c4` merged (it contains `803a93e`). I checked master at the start of rounds 2 and 3; nothing new had landed.
- Author file: `crates/ringdesign-core/examples/cataphracta_phrynosoma.rs`. I made no edits in `src/`.
- Outputs: `showcase/cataphracta/phrynosoma/`.

## Base and process

- **Process:** lost wax, per Logan's 2026-10-03 ruling. The ring is judged at a 0.8 mm minimum section with no pull rule. Any sand result is reported only, as a bonus.
- **Base:** the factory **004 Shield** at its own 18 × 18 face, bore 18.6 (US 8.6), unmirrored. Logan approved this rethink on 2026-10-03, after all three 016 Star block-outs failed.
- **Why 004:** its flat back edge with sharp corners, and its sides tapering to a point, are the horned lizard's skull in plan, so the table itself is the skull.
  - The six-horn comb sits on the straight back edge.
  - The eyes sit at the widest part.
  - The jaw fringe runs down the tapering sides.
  - The snout is at the point.
  - The runner-up was 014 Heater, which is smaller (15 mm) with softer corners. 001 Cushion is Chamaeleo's base.
- **Recorded in the doc:** the ruling, the rethink with this reason, and the final status are all in the ring's section of `docs/collections/cataphracta.md`. The table row is updated too.
- **Sand bonus:** none. The two-part undercut is 6.04% on the band (worst 52.2°) and 13.2% with the parts. It is reported, not gated.

## Read tests

Each read test was run by a fresh, independent reviewer agent, given only `target/review.md`, the name, the slug, the mode, the attempt number and the paths.

| # | Base | Reads | What the eye saw |
|---|---|---|---|
| 1 | 016 Star | no | "A spiky sunburst or a punk-maned beast head" |
| 2 | 016 Star | no | "A pine cone, a hedgehog or a thistle head" |
| 3 | 016 Star | no | "A six-lobed star signet … starfish or sea urchin, with a tortoise-shell patch" |
| 4 | 004 Shield | no (close) | "One symmetric, heart- or arrowhead-shaped shield of polygonal plates … a spiked crown along the wide edge." It asked for stout, laid-back horns, not a needle tiara. |
| 5 | 004 Shield | **yes** | "A flat, wide reptile head seen from above, and it now names itself": the horn comb either side of a midline notch, round eyes with brows, a horned lizard. |

The verdicts are in `read-test-1.json` to `read-test-5.json`.

## Review rounds

Each round started a fresh reviewer in full-review mode.

| Round | Verdict | Score | Main findings |
|---|---|---|---|
| 1 | revise | 6.7 | The head reads. The plate joints were serrated, the midline had notches, the shank was bare toward the palm, and the margin was weak. |
| 2 | revise | 7.1 | The shank is now covered face to palm and the template is within budget. Torn collar rings remained part-way up the horns, plus plate-joint serration and jaw thorns smearing into the plates. |
| 3 | **cut** | **7.2** | The horn collars are gone and each horn is one clean cone, raised to 40/38/38°. Still open: serrated joint rims and outer plate columns smearing into the jaw thorns, a boxy block on the midline behind the horns, the polished border's single pin-bead row reading as rivets on a heraldic shield, a bare fold under the snout, and saw-toothed granule bases on the shank. The reviewer ranked it level with Basiliscus (cut at 7.2). |

The full reviews are `review-round1.json`, `review-round2.json` and `review-round3.json`. In round 3 I moved the forced midline plate from z −5.55 to −5.15 before the build was reviewed. The reviewer still saw a block there. I have recorded its finding as written and did not dispute it.

## Gates (round 3; `report-draft.json` at 768 × 320, `report.json` at the 1536 × 448 export)

| Gate | Draft | Export |
|---|---|---|
| Finished mesh watertight, 0 degenerate faces, 0 self-crossings | pass | pass |
| Sculpted parts closed and uncrossed; notes empty; 3 parts joined | pass | pass |
| Nothing enters the finger hole (nearest vertex 9.2997 mm against a 9.30 mm radius) | pass | pass |
| Lost-wax field verdict at the 0.8 mm fill: **Castable** | pass | pass |
| Wall census (below) | pass | pass |
| DFM findings: 0 | pass | pass |
| Stones reported equal the preview (0 = 0) | pass | pass |
| Gates hold at 384 × 192 (311,882 triangles) | pass | pass |
| Casting pattern watertight, 0 degenerate faces, 0 crossings | pass | pass |
| Triangles within 2 M | 441,294 | 925,926 |
| Cold reload identical | pass | pass |

**Wall census (gate per PR #261 and the lead's 2026-10-04 notes):** `cad::measure::thickness(&built.mesh, 0.8)` runs on the finished mesh, and the hand-made exception censuses are gone.

- Real walls (0.05–0.8 mm, at least 0.02 mm² and at least 0.15 mm across): **0**.
- Specks listed and passed: 30.
- Suspected census artifacts, under 0.05 mm: 34.
- Edge zones: 64, with the edges listed in `report.json`.
- A wider-reach census (2.0 mm) is recorded beside the main one as `wall_census_edge_reach_2mm`. It is for information only.

## Template gate (after the last round; class `painted`)

| Measure | Result |
|---|---|
| `design.set` patches | 1 (at most 4): pass |
| Size | **2,908,155 of 3,000,000 bytes: pass** |
| Source and mesh | Source identical, and vertices, faces and normals identical |
| Triangles | 925,926, the same as the build |
| Cold reloads | Design and graph cold reloads identical |

Overall `template_gate_passed: true`. The figures are in `template-verification.json`, and `round.sh` merges them into `report.json`'s gates.

## What the ring is made of, and why

**Sculpted parts.** These are stored sculpts, joined. I used sculpt because the head needs true 3D: horns, sockets and V-jointed plate tiers that a height field cannot undercut.

- **Horned head** (204,474 faces after decimating from 1.78 M):
  - **Skull outline:** the wedge skull is the table edge intersected with a wedge, plus a snout half-ellipse held 0.8 mm inside the factory point, so the shield keeps its crisp edge.
  - **Plates:** 65 mirrored Voronoi plates in three tiers, with smoothstep V-joints and domed tops.
  - **Horn comb:** six horns, with occipitals 2.7 mm and temporals 1.7 and 1.5 mm. Each is a round cone with a root of 0.28 + 0.36 × length and a 0.28 mm tip.
  - **Face:** eyes in sockets with brows, and nostrils.
  - **Jaw-fringe thorns:** rooted 0.38 mm outside the skull outline.
  - **Margin:** 33 margin granules out to a 0.3 mm lip.
- **Two seam granule parts** (about 14 k faces each), at θ 0° and 180°. The source stock's relief folds there, so the layers are masked off and these parts carry the hide across the gap.

**Live layers, not sculpt.** I used live layers to keep the template within its painted budget, and so the hide stays editable.

- **Granule layers:** five chart-space `TilingLayer`s, one bead SVG each, windowed per region with rows sized to that section (head 23 mm, two shoulders at 13 mm, shank 7.6 mm). Rows set for the full band had produced squashed-cell DFM findings.
- **Mask:** a painted PNG16 mask, "Off the table". It excludes:
  - the table's top 0.9 mm;
  - 1.2 mm round the bore;
  - ±6° round the source seams;
  - hard edges turning more than 22°;
  - discs under the stamps.

**Stamps:** struck where each one is needed, placed from atlas samples.

- **Crest tubercles:** Dome top, 11 per side, 1.6 mm, taper 0.44, along the parting line.
- **Fringe scales:** `rounded_triangle` with a Cone top, 16 per rim per side, set at 0.88 of the rim radius on the outer face. Placed on the rims, they made thin walls.

## Enablers used (all on master)

- **#248:** `render::write_png_framed` with `Framing` for the `head.png` and `stones.png` close-ups, and for the `PHRYNO_LOOK` probe.
- **#261 and #262:** the `cad::measure` wall census, which is the wall gate.
- **P4 stamps** (`StampTop::Dome` and `StampTop::Cone`, plus the `outline::rounded_triangle` shape) for the crest and fringe.
- **Tiling layers with painted masks** for the hide. I considered C-R4's Hide-space tilings, but they did not clear the DFM finding (below).

**Not used:**

- `crisp_relief` and `StampTop::Pillow`: there is no height-field relief to crisp, and the graph lift could not carry `crisp_relief` anyway.
- #255 (Textura), #258 (patterns along a path), #259 (CAD fallbacks) and #257 (stone plans): there is no stone and no path pattern.

## What I could not do

- **Reach 7.5.** The round-3 punch list in `review-round3.json` is unaddressed:
  - joint rims and outer plate columns;
  - the midline block;
  - a border that reads as rivets;
  - the bare fold under the snout;
  - the granule bases.
- **Clear the census edges.** 64 edge zones and 34 sub-0.05 mm artifact zones remain on the shank walls and the border. They are listed, not gated. The reviewer called them finish faults.

## Core changes wanted

1. **Imported-stock relief folds near the source mesh's seams.** Relief on an imported factory stock self-crosses near θ 0° and 180° and close to the bore, which is why the mask exclusions and the two seam sculpt parts exist. The core should carry relief across the source seam cleanly.
2. **DFM does not honour Hide-space tilings.** With Hide-space tilings the DFM still judged the cells in chart space and reported squashed cells. I had to fall back to chart-space layers windowed per region.
