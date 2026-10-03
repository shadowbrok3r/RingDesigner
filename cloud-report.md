# Tenebrae — Rosa: final report

**Outcome: stopped at the block-out.** All three read tests came back `reads: false`, so under TASK.md the loop stopped before round 1. **Rounds used: 0 of 3.** No full review ran, so there is no review verdict or score. The subject needs rethinking, not detailing (see "What the reviewers kept saying").

- Branch: `claude/tenebrae-rosa`. Master `2e11632` is merged in.
- Example: `crates/ringdesign-core/examples/tenebrae_rosa.rs`.
- Outputs: `showcase/tenebrae/rosa/`.
- Collection doc: one line added to the Rosa section of `docs/collections/tenebrae.md`. It records the process decision, the base, the pears and the stop. No extra rounds were granted.

## Read tests (independent reviewer, read-test mode)

| # | Build | reads | What the eye saw |
|---|---|---|---|
| 1 | Sunk-cell rose. Eight 1.8 × 3.0 oval sapphires in drawn collets round a 3.5 ruby. Sunk curved-triangle spandrels between the petal heads. | **false** | "a sapphire and ruby flower cluster on a square signet… nothing reads as Gothic cathedral" |
| 2 | Raised tracery wheel: full mullions, an outer order and pointed heads. Lights are true pears pointed at the rim. Graded shoulder oculi. | **false** | "closer… still 'sapphire and ruby flower cluster'; the tracery is too thin to win; no arch on the walls" |
| 3 | Tracery 0.9 mm proud, over the collets. Mullions 1.0 mm. Three-lancet gallery arcade cut into each cheek. Larger shoulder oculi. | **false** | "reads as a rose window on a second look, but the almond collets read as petals first; nothing outside the face says Gothic" |

The full JSON is in `showcase/tenebrae/rosa/read-test-{1,2,3}.json`.

### What the reviewers kept saying

- **The stones beat the tracery.** Each reviewer read eight bright stones round a red centre as a flower before they saw a window.
- **The 0.8 mm floor makes it worse.** Every collet wall must be at least 0.8 mm. That makes each 1.8 × 3.0 light a 3.6 × 4.8 mm gold almond, which fills the lancet cell, so the petal outline wins over the mullions.
- **Requests from attempt 3 not yet done:**
  - collets trimmed to low bezels inside their cells;
  - a cusped inner rim (16 cusps);
  - pierced trefoils in the four cushion corners;
  - an arcade deep enough to cast a shadow, on the end walls as well.
- **What would fix it:** fewer, smaller or flush-set stones (or stones only in the oculus and the spandrels), so that the gold spokes, the cusped rim and pierced foils carry the read. The plan's stones and the 0.8 mm floor cannot both fit in a 16–19 mm face together with 0.8–1.0 mm tracery bars.

## Base and process

- **Base: 001 Cushion at 19 × 19, bore 18.6.**
  - 013 Round tops out at 13 mm without baking (the 70–130% rule).
  - The baked 130% source travels inline. The design alone measured 1,250,150 bytes, over the stock template budget of 1,000,000.
  - So the brief's fallback, 001, was used. It went to 19 rather than 18 to make room for the 0.8 mm collet walls.
  - The palm is raised its full half millimetre (1.5 → 2.0): the bare stock's shank edges measured 0.50 under `thickness(0.8)`.
- **Process: lost wax, Gold 18k.** `CastProcess::LostWax.apply`, then `min_section_mm` 0.8 and `min_detail_mm` 0.15. This matches Logan's 2026-10-03 rule, so nothing changed. The sand field also reads Castable, but no two-part ray release was run, so I claim no sand bonus.
- **Lights: Pear 1.8 × 3.0.**
  - Master's C-B2 (#257) gives the pear a true girdle through `GemCut::has_true_girdle`, although `plan_pow` still returns 2.0.
  - I took that as the enabler landing and switched the lights from ovals to pears, points aimed at the rim (`spin_deg` −90).
- **Enablers used:**
  - C-T1: `Sketch::tracery` on the rose net, and `Profile::Regions` for the spandrels.
  - C-T3: the `cutter.pierce` builder and `cutters::outline(Shape::Lancet)` for the arcade.
  - C-T4: `dfm::cut_lands` at 0.8.
  - C-B2: pear plans.
  - #248: framed close-ups with `write_png_framed`.

## Gates (committed block-out 3)

| Gate | Draft 768×320 | Export 1536×448 |
|---|---|---|
| Triangles (≤ 2 M) | 65,012 | 65,012 (the stock and CAD parts build at their own resolution) |
| Watertight / degenerate faces | yes / 0 | yes / 0 |
| `self_crossings`: ring / made parts | 0 / **20 in "Their eight collets"** | 0 / **20** |
| Solids notes / parts notes | empty / empty | empty / empty |
| CAD features `Ok` | all | all |
| Finger hole | 0 vertices inside, min r 9.2995 vs bore 9.300 | same |
| Field verdict (lost wax) | Castable, thinnest wall 1.88 | same |
| `cad::measure::thickness(0.8)` at 384×160 | **382 rays, 44 below, min 0.025** | same |
| `dfm::cut_lands(0.8)` | 0 findings | 0 |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed | 9 / 9 | 9 / 9 |
| `--verify` cold reload, empty library | — | identical |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |

**Two gates are red: thickness and the made-part crossings.** Under TASK.md that does not block a block-out read test, but it would block a review round.

- **Thickness:** the thin samples sit where the drilled pilots, the bur relief and the collet bearings meet near the girdle and the bore.
- **Crossings:** the pattern copies of the drawn collets meet the raised tracery degenerately, and the part is "joined one by one".

What I learned getting part of the way:
- Cut tools apply after join tools, so a pocket cut through a collet's footprint shaves the collet to a skin.
- A seat bur on a stone standing over a joined collet takes the whole-bur route, and its clearance and girdle wall shave the collet's inner wall.
- `head.bezel`'s leaning lip is 0.16 mm thick vertically. No lip share reaches 0.8, so I drew straight-walled collets instead.
- Vertical through-cuts leave knife edges where they exit the bore at an angle, so the pilots are drilled toward the finger's axis.

**Template gate** (`collection_templates --verify-export`, class `stock` because the base is factory 001):

| | |
|---|---|
| Nodes | 62 |
| `design.set` patches | 1 (`/manufacturing`), at most 4 allowed |
| Graph size | 528,319 bytes, inside the 1 MB stock budget |
| Cold source / cold graph reload / mesh parity | identical / yes / identical vertices, faces and normals |
| `template_gate_passed` | true |
| Open time | first build 1.31 s, evaluate 37 ms |

`crisp_relief` is off, because the lift cannot carry it yet. The file is in `showcase/tenebrae/rosa/verification.json`.

**Weight:** 36.4 g in Gold 18k. That is heavy, and comes from the 19 mm cushion and the 2.0 mm palm.

## Feature tree, as sentences

1. Cushion signet, factory 001 at a 19 mm face.
2. Table, lifted clear of the metal (+1.2).
3. The rose net traced into lights a bar apart (C-T1):
   - hub circle r 3.0, outer circle r 8.5;
   - 8 mullions from hub to rim, between the lights;
   - 8 pointed heads, each two arcs struck at 1.0 of the span, apex on the rim;
   - bar 1.0 mm.
4. Tracery top (+0.9).
5. The bars between the lights, inside the outer order (r 9.0).
6. Raise the tracery: the oculus order, eight mullions, the heads and the outer order.
7. Sink the sixteen spandrels deep (3.0 mm, `Profile::Regions`).
8. The ruby stone, Round 3.5, girdle 0.4 over the table.
9. Its seat bur.
10. The oculus collet, drawn:
    - lip height plane, lip ring and "raise the lip to the bearing";
    - bearing height plane, bearing ring and "stand the bearing on the metal".
11. The first light, Pear 3 × 1.8, 5.65 mm out, girdle 0.25: its seat bur and its drawn collet, as above.
12. Eight sapphire lights round the oculus, eight seats and eight collets. These are `About` patterns round the ruby: P2 keeps a gem on every copy.
13. Drill the oculus pilot, and each light's pilot, toward the finger's axis (nine cuts, each leaned to its own angle).
14. Oculi of the nave 1–4: round piercings 2.0, 1.8, 1.5 and 1.2 at θ 90 − (44, 53, 62, 71).
15. Mirror the oculi through the crown.
16. Outside the near cheek: a parting-parallel plane at z 9.9.
17. Gallery arcade: three lancets, 1.6 × 2.3, centred 2.5 apart.
18. Cut the gallery arcade into the cheek.
19. The same arcade in the far cheek (mirrored across the band).

The four head-wall arcade stamps (`setting::Stamp`) are written but switched off (`WITH_ARCADES`). On the stock's cheeks at draft they struck torn, stair-stepped cuts, so the arcade became the CAD cut above.

## What I could not do

- **Make it read.** See the read tests. The plan's stone count and sizes, together with the 0.8 mm walls, turn the rose into a flower.
- **013 at 16 mm** within the template budget, without the `pre_scale` the doc asks P7 for.
- **A seam bead on the tracery or the collets.** The clustered join's fillet folds in every acute light corner, so `blend_mm` is 0 on the tracery and the collets. That breaks house rule 10.
- **A drafted tracery extrude (12°).** The kernel refused it: "unsupported or degenerate geometry".
- **Pierced-through spandrels.** They exit the shoulders at about 45° and fail thickness, so they are sunk 3.0 mm.
- **Clean thickness and collet crossings** (above).

## Core changes wanted

1. **A `head.bezel` "cast straight" option**, a wall standing square above the girdle with no lean, so a cast collet holds the investment floor:
```rust
// cad/builders.rs, BEZEL params
number("lean", "Lip lean", "", 0.0, 1.0, 1.0),
// setting::collet_named: scale the lean
let lean = lean_share * 0.8 * (1.0 - crown_scale(gem, top - g)) * plan.b;
```
2. **Seat bur against the band only.** `seat.surface_z` should be read from the band, not from the band plus joined collets, so the bur takes the relief route and never shaves a collet it sits in. Equivalently, exclude `ComponentRole::Setting` joins from the surface read in the BUR builder.
3. **Per-part fillets in a join cluster**, laid only on each part's own seam with the band, with every acute corner skipped rather than failing the whole cluster.
4. **`base.preset { pre_scale }`** (as the doc asks P7), so 013 can reach 16 mm without carrying the baked source inline.
5. **`cad::measure::thickness` with a list of thin samples**, not just the minimum, so authors can find every thin spot.
