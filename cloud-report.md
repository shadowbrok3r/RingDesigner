# Vepres ring: Viscum (`viscum`), cloud report

**Outcome: cut at 6.8 after round 3 of 3.** The rethink read at the first try (read test 4). The three reviewed rounds scored 6.2, 6.6 and 6.8, all under the 7.5 ship bar. Every gate was green at draft, 384 × 192 and export in every round. The template gate, run after the last round, is also green.

- **Branch:** `claude/vepres-viscum`. It started from `master` at `8e5a59a`. Master `2e11632` was merged in at `edb4fda`, and master `29babc4` (#260) at `8414dd4`.
- **Example:** `crates/ringdesign-core/examples/vepres_viscum.rs`.
- **Run:** `target/release/examples/vepres_viscum [OUT_DIR] [--draft] [--verify]`. The default output is `showcase/vepres/viscum/`.
- **Process:** lost wax, per Logan's rule of 2026-10-03: 0.8 mm minimum section, 0.15 mm detail, no draft, no pull rule, 18k yellow gold. No extra rounds were granted. The decision is recorded in Viscum's section of `docs/collections/vepres.md`, together with the rethink and the stock.
- **Stock:** factory **017 Tonneau** at its native 16 × 12, through its sand master with the envelope on (the reasons are below).
- **Stones:** 21 moonstone round cabochons, 1.07 to 1.95 mm. That is 9 on the face and 12 on the shoulders.
- **Rounds used:** 3 of 3, plus 4 read tests (3 on 003, then 1 on the rethink).

## Read tests and reviews

| Step | Stock | Verdict | Score | What it said |
|---|---|---|---|---|
| Read test 1 | 003 Clover | does not read | — | "A big gold flower with a pearl-cluster centre." The lobes read as petals. |
| Read test 2 | 003 Clover | does not read | — | "Gold blossom or bow-shaped cocktail ring with pearls." |
| Read test 3 | 003 Clover | does not read | — | "Gold flower ring with pearls": a double flower or pinwheel. Only the three-berry triangles worked. That stopped the first block-out, and I reported back. |
| *Rethink (a), chosen by Logan* | | | | |
| Read test 4 | 001 Cushion | **reads** | — | "Mistletoe, at first glance": dichotomous forking, opposite strap leaves, white berries in the crotches. It asked for fleshier 3:1 leaves, a fuller face with no crossed "propeller" node, and translucent berries. |
| Round 1 | 001 Cushion | revise | **6.2** | It asked for clean joints instead of pinched collars, and noted that the head-to-bore junction was torn all round (that tear is in factory 001's own mesh). It also asked for leaves with body, oak bark down the shank, and a fuller face. |
| Round 2 | 017 Tonneau | revise | **6.6** | The torn underside was gone. Still open: pinched nodes, the crossed four-leaf node, the upturned "Face berry 5", grey opaque berries, and smeared bark that stops at the head. |
| Round 3 | 017 Tonneau | **cut** | **6.8** | Fixed: the crossed node (it is now two opposite pairs on successive forks), and Face berry 5, which now sits upright. It still reads as mistletoe at 300 px. Still open: the nodes are collars with eyelets, the right-hand triplets float with no stalk, the right arm ends in a hooked cap, the berries render opaque grey, the bark is smeared and absent on the shank, the shoulder leaves are crumpled, and the stems are untapered. |

The round-3 reviewer marked the template gate "not recorded", because it was run after the review as TASK.md orders. It is recorded below and in `showcase/vepres/viscum/verification.json`.

## Gates (round 3, final design)

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Watertight / boundary / non-manifold | yes / 0 / 0 | yes / 0 / 0 | yes / 0 / 0 |
| Degenerate faces | 0 | 0 | 0 |
| Self-crossings, ring | 0 | 0 | 0 |
| Self-crossings, each of 65 made parts | 0 (all closed, manifold) | 0 | 0 |
| `solids.notes` / `parts.notes` | empty / empty | empty / empty | empty / empty |
| Bore margin | −5e-7 mm (zero) | −5e-7 mm | −5e-7 mm |
| Field verdict (lost wax) | Castable | Castable | Castable |
| Thinnest wall (floor 0.8) | 1.10 mm | 1.10 mm | 1.10 mm |
| DFM findings | 0 | 0 | 0 |
| Stones reported = previewed | 21 = 21 | 21 = 21 | 21 = 21 |
| Metal inside stones | 0 | 0 | 0 |
| Triangles | 533,080 | 228,338 | **1,307,036** (≤ 2M) |

The closest stone pair is B2.1 to B2.2, 0.34 mm at the girdle. The ring is 25.0 g of 18k.

- `--verify`: a cold reload with an empty library is **identical**.
- Casting pattern: watertight, 0 degenerate faces, 0 self-crossings, **pass**.

Rounds 1 and 2 also passed every gate at draft, 384 × 192 and export before their reviews (commits `f59fda5`, `c2199a9`).

## Template gate (after the last round)

`collection_templates vepres … --only viscum --verify-export`. The class is **stock**.

| Measure | Value |
|---|---|
| Gate | **passed** |
| Source method | lift |
| Template bytes | **961,517** (budget 1,000,000; no size review required) |
| Nodes / exposed controls | 246 / 0 |
| `design.set` patches | 1 (`/manufacturing`) |
| Source identical / cold graph reload / cold design reload | true / true / true |
| Export geometry | verified; 1,307,036 triangles, with vertices, faces and normals identical |
| Open times | first build 2.69 s, detail 0.49 s, rebuild 3 ms, verdict 39 ms |

At the end of round 2 the graph was 1.80 MB. Two changes brought it under budget:

- dropping the pillow ring counts, to 11 top and 4 bottom rings;
- trimming each part's ring segments: stems and joints 40 → 32, mounds 52 → 40, leaves 88 → 72.

`crisp_relief` is on, and #260 lets the lift carry it.

## Why the stock moved twice

1. **003 Clover → rethink.** In three read tests, 003's four swollen lobes read as petals, whatever was laid on them. Logan chose route (a): keep the forked twig and berry-bunch unit on a flat or cushion signet. That ruled out the lobed plans (003, 005, 007, 016).
2. **001 Cushion, 17 × 14.5.** I chose it for its quiet crowned table. The brief's 17 × 13 is refused, because 13 mm is below 70% of the 20 mm master. Round 1 found the head-to-bore junction torn all round, and the tear is in **factory 001's own mesh** at every size. 006's mesh is torn the same way.
3. **017 Tonneau, native 16 × 12.** This was the one candidate with a clean underside; upsized, it saw-tooths. Its face is still one quiet barrel table.

The stock always comes through **its sand master with the envelope on**. On every non-sand imported stock I tried (001 at two sizes, and 006), struck stamps collapse the build to about 20k triangles and break into the bore. The ring is still judged as lost wax.

## The construction, as sentences

The height field carries only the stock, the bark and the struck stamps. Everything else is a sculpted part made in the example: an `Operation::Stored` mesh joined with `Attach::Join` and `Placement::Free`, each sinking to its own unique depth so that no two part bottoms are coplanar. A `Band` feature named "Factory 017 Tonneau" heads the CAD document.

- **Depth maps.** The bare 017 surface is rasterised from its `Atlas` into:
  - a plane map looking down on the table;
  - a cylinder map round the crest.
- **Pillows.** Every part is a closed "pillow": rings shrunk from its margin to its spine, capped by ladders top and bottom.
  - **Stems** are half-round. Each runs 0.9 r past both of its ends, tapers 25%, and rides the highest stock within reach (Bridge ground).
  - **Joints** are ellipsoid swellings, 1.25 × the stem radius long.
  - **Berry mounds** sit on a quadric fitted under them (Free ground).
- **Face sprig.**
  - The main stalk comes in from the left end and forks at three nodes.
  - Two short side twigs at the first node carry the left-hand opposite leaf pair (3.3 × 1.5 mm).
  - At the main node a wide Y opens into two arms. Each arm ends in a node with an outward-splayed opposite pair (3.4 × 1.5 mm).
  - Every leaf is a struck `StampTop::Pillow` stamp (crown 0.7 mm, wall 0.35 mm) cut clear of the field, curved 12° away from its partner, and trimmed until it lies on the table.
- **Face berries.** There are three triangles of three:
  - 1.65, 1.5 and 1.45 mm in the main fork's crotch;
  - 1.18, 1.12 and 1.07 mm beyond each arm's tip.
  - The placer searches distance and a ±45° swing for the first spot that clears every leaf, stem and berry (gap 0.45 mm), and keeps each mound off the table's bevel. It tests the bevel as a 0.25 mm step within 0.8 mm, rather than the cushion's gentle fall.
- **Berries (all 21).** Each is a CAD moonstone placed by a Transform, tilted to its mound's normal. It sits in a thin flush bezel (wall 0.3, lip 0.1) with the builder's seat bur, on its own fitted mound part. The crown heights carry a few microns of jitter, so no two boolean seams meet edge on.
- **Shoulders.** On each side, the bough runs from the face's end down the crest, with two Y-fork units. Each unit has:
  - twigs at ±27°;
  - a joint at each tip carrying one draped leaf;
  - a tight triangle in the crotch: 1.95, 1.8 and 1.7 mm, then 1.8, 1.65 and 1.6 mm.
- **Oak bark.** `Procedural::Bark`, a hide-space `TilingLayer` subtracted 0.3 mm. It is masked to the head walls (θ 270°, span 140°) and the cheeks (θ 90°, span 76°), with 5° fades and windows that keep it out from under the shoulder units.
- **Stone count.** 9 (face) + 12 (shoulders) = 21.

## Enablers from master used

- **#248:** `render::write_png_framed` with `yaw_facing` for the close-ups (`stones.png`, `shoulder-close.png`), instead of a cropped mesh.
- **#248:** `StampTop::Pillow` for every face leaf.
- **#248 and #260:** `crisp_relief` on, now carried through the graph lift.

The C-B2 plans were not needed, because every stone is round. I reached none of the C-V or C-T enablers, nor #255, #258 or #259.

## What I could not do

- **Reach 7.5.** The open items are listed in round 3's punch list (`review-round3.json`):
  - the nodes still read as collars with eyelets, because separate stem and joint pillows meet at their caps;
  - the right-hand triplets have no stalk;
  - the bark smears and stops at the head;
  - the shoulder leaves are crumpled;
  - the stems are untapered tubes.
- **Translucent berries in the renders.** `stones.json` carries transmission 0.62, subsurface 0.5 and a warm tint. The renders still show opaque grey cabochons, so the material does not reach the render path as I write it.
- **Use non-sand imported stock with stamps.** See core change 1.
- **Seam beads.** `blend_mm` beads on the stored pillows fold or pinch, so every part's blend is 0. The lessons' 0.3–0.4 mm fillet is not met.

## Core changes wanted

I have not located the faulting code for any of these, so I cannot give exact code. Each item gives the repro instead.

1. **Struck stamps on non-sand imported stock.**
   - Repro: `VISCUM_SAND` unset with `STOCK_SAND_MASTER = false`, on 001 at 17 × 14.5, or on 006.
   - Result: the build collapses to about 20k triangles and intrudes into the bore. Through `sand_master` with the envelope on, the same stamps build correctly.
2. **Factory 001 Cushion and 006 Square meshes.** Their head-to-bore junction is torn all round at every size: hero, side and palm views, and `bare-vs-finished.png` on the bare stock. The factory meshes need re-baking.
3. **Moonstone material in renders.** `stones.json` transmission and subsurface should reach `render::write_png*`. If the render reads the material from somewhere else, the stock-ring docs should say where.
4. **Negative field relief on non-sand imported stock** (from the 003 attempts). A `Blend::Subtract` layer evaluates to non-zero heights but does not cut native 003.
   - Repro: `VISCUM_BARK=1 VISCUM_BARK_BENCH=1` on native 003.
5. **Seat pads that fold over a concave crease** (from the 003 attempts). A `SeatPadLayer` gypsy mound at the 003 centre (θ 87°, v 8.1) gives 114 self-crossings, and "its seat could not be cut".
   - Wanted: clamp the pad's normal offset by the local valley radius.
