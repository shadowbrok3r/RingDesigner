# Vepres ring: Viscum (`viscum`), cloud report

**Outcome: stopped at the block-out after three failed read tests.** No review round was run (0 of 3 used). Under TASK.md, a third block-out that does not read means the subject needs rethinking, not detailing. Every draft gate was green on the last attempt. The ring fails on its read, not on manufacture.

- Branch: `claude/vepres-viscum` from `master` at `8e5a59a`, with master `2e11632` merged in (`edb4fda`).
- Commits: `1563d45` (attempts 1 and 2), `5fe3c4e` (attempt 3, and the process line in the collection doc), `133e677` (read test 3), then this report.
- Example: `crates/ringdesign-core/examples/vepres_viscum.rs`.
- Run: `target/release/examples/vepres_viscum [OUT_DIR] [--draft] [--verify] [--blockout]`.
- Outputs: `showcase/vepres/viscum/`. The folder holds the renders, `hero-300.png`, `face-300.png`, `contact-300.png`, `report.json`, `design.ring.json` and `read-test-{1,2,3}.json`.
- Process: lost wax on native factory 003 Clover, 18 × 18, unmirrored, no sand envelope; 0.8 mm section, 0.15 mm detail, no draft; 18k yellow gold. Per Logan's rule of 2026-10-03, I recorded this in Viscum's section of `docs/collections/vepres.md`, with no extra rounds granted.

## Read tests

| Attempt | Reads | What the eye saw | What it asked for |
|---|---|---|---|
| 1 | **false** | "A big gold flower with a pearl-cluster centre." The four swollen clover lobes read as petals, and the seven collared stones read as a pearl cluster or grapes. | Leaves cut along each lobe; three separate berries in the fork, not collars; repeated Y-fork / leaf pair / berry units. |
| 2 | **false** | "Gold blossom or bow-shaped cocktail ring with pearls." The lobes are still the loudest form. The shoulder singles read as pearl studs. | A leaf outline filling each lobe with a dark recess, and the lobes 40% flatter; tight 2–3 berry bunches in Y-forks on the shoulders; visible stalks to the node; translucent berries. |
| 3 | **false** | "Gold flower ring with pearls", now read as a double flower or pinwheel of eight or more petals. The three-berry shoulder triangles and the right-hand face triangle "work" and are "the most mistletoe-like detail". | Exactly four narrow 3:1 leaves cut out of the lobes; all berries on the central node or in axils, none on blades; Y-forks with leaf pairs on the cheeks and shoulders. |

**Why it failed: the stock is the problem.** Factory 003 is four swollen, heart-shaped lobes meeting at a sharp central crossing. At 300 px the lobes are the face's loudest form, and every reviewer named them petals. Here is what each attempt tried:

- **Raised leaves on the lobes** (attempts 1 and 2) read as veins on petals.
- **Large free blades running past the lobe edges** (attempt 3, with engraved trenches and midribs) changed the outline. The reviewer then counted the blades together with the lobe edges, and saw an eight-petal flower.
- **Taking the lobes' shine off.** I tried an oak-bark tiling and a hammered stipple, both in hide space (C-R4), as Subtract and as bench-only layers. Neither cut the imported stock. As Add, the bark folded the field into self-crossings and read as combing, so I dropped it.
- **Flattening the crowns.** Every reviewer asked to flatten them by about 40%. That would mean rebuilding the factory stock, which the brief keeps "with its lobes".

What did work is the berry bunches: tight triangles of three moonstones in the crotch of a forked twig.

### Suggested rethink

- **(a)** Keep the forked twig and berry-bunch unit, which the reviewers praised. Put it on a stock whose face is not four petals, such as a flat or cushion signet.
- **(b)** If 003 must stay, fill the lobes' crowns and creases with one sculpted ground plate so the face becomes a flat seal. The clover would survive only as the plan, and the sprig would sit on top. That needs Logan's consent, because it hides the factory lobes.

## Gates (block-out attempt 3, draft 768 × 320)

| Gate | Result |
|---|---|
| Watertight, degenerate faces | yes, 0 |
| Self-crossings: ring, and each of the made parts | 0, and 0 on every part (all closed and manifold) |
| `solids.notes` / `parts.notes` | empty / empty; 91 parts joined, 12 graver cuts |
| Bore margin | −0.0009 mm (the stock's own bore) |
| Field verdict (lost wax) | **Castable**; thinnest wall 1.37 mm (floor 0.8) |
| DFM findings | 0 |
| Stones reported / previewed | 21 / 21 (CAD moonstones counted by `built_vertices`); metal inside stones 0 |
| Closest pair | Shoulder B2.1 to B2.2, 0.22 mm at the girdle |
| Metal | 29.8 g of 18k |

These gates were not run, because no round was reached:

- the export build at 1536 × 448 and `--verify`;
- the casting pattern;
- the template gate.

The example runs all of them: an export run gates, verifies, writes the STLs and `stones.json`.

## The construction, as sentences

Nothing goes through the height field except the stock. Every element is a sculpted CAD part (`Operation::Stored`, `Attach::Join`) generated in the example.

- **Depth maps.** The bare 003 surface is rasterised from its `Atlas` into three kinds of depth map:
  - a plane map looking down on the table;
  - one plane map for each cheek;
  - a cylinder map round the crest.
- **Pillows.** Each part is a pillow: rings shrunk from its margin to its spine, closed by ladders top and bottom.
  - Leaves have an obovate strap plan, a 0.2 mm wall, a 0.1 mm top round, and a cushion crown of 0.95 mm.
  - Stems are half-round, 0.12 mm proud, with round ends.
  - "Bridge" stems ride the highest stock within reach, so the bough crosses the crease instead of lying in it.
  - "Free" blades lie on a quadric fitted to the stock, and run on past a lobe's edge on their own underside.
- **Unique sinks.** Every part sinks to its own depth, so no two part bottoms are coplanar.
- **Face.**
  - One node.
  - Four strap leaves along the lobe axes: 9.4 and 9.0 mm on one pair, 7.9 and 7.5 mm on the other, about 3.2 mm wide.
  - Stalks running into the node.
  - Twelve graver cuts (`Attach::Cut` tubes): a 0.24 mm half-round trench round each leaf, and a tapering midrib.
  - A bunch of three berries (2.4, 2.1 and 2.1 mm) beside the node, and a pair of 1.9 mm berries astride the bough.
- **Berries.**
  - The face and shoulder berries are CAD moonstones placed by a Transform. Each sits on its own fitted mound part, in a thin flush bezel (wall 0.3, lip 0.1), with the builder's seat bur.
  - Field gypsy mounds folded over the clover's central crease and could not cut their seats there. A seat straddling the chart's 0°/360° seam failed the same way.
  - The four cheek berries (1.5 mm) are flush gypsy-mound field seats on the lobe walls.
- **Shoulders.** The bough runs from the face notch down each crest. Two Y-fork units sit on each side:
  - twigs at ±27°, with a joint knob at each tip carrying one leaf of the pair;
  - a tight bunch of three in the crotch: 1.95, 1.8 and 1.7 mm, then 1.8, 1.65 and 1.6 mm.
- **Cheeks.** Each lobe wall carries a twig hanging from the face's edge to a joint, with a downward leaf pair and one berry in its V.
- **Stone count.** 5 (face) + 12 (shoulders) + 4 (cheeks) = 21 moonstones.

## Enablers from master

The new close-up renders use #248's `render::write_png_framed` with `yaw_facing` (`stones.png`, `shoulder-close.png`), in place of a cropped mesh. No other enabler was used:

- `crisp_relief` and `StampTop::Pillow` do not apply, since nothing on the ring is height-field relief or a stamp;
- the C-B2 plans are not needed, since every stone is round;
- the C-V and C-T enablers, and #255, #258 and #259, were not reached before the stop.

## What I could not do

- **Make the 003 lobes stop reading as petals.** None of the three approaches above worked. See the rethink.
- **Cut texture into imported stock.** A `Subtract` tiling, with or without `bench_only`, showed no relief on native 003. Its heights evaluate to between 0.02 and 0.18 mm, but the mesh does not change.
- **Lay seam beads on these parts.** `blend_mm` beads on the stored pillows folded or pinched ("the bead folds at N stations"). I set every part's blend to 0, so the lessons' 0.3–0.4 mm fillet is not met.

## Core changes wanted

1. **Negative field relief on imported stock.** A `Blend::Subtract` layer should cut the stock, as it does on a procedural band, so that matte grounds and bark can be laid on factory signets. I have not located the clamp in `imported_base`'s field evaluation, so I cannot give exact code. The repro is `VISCUM_BARK=1 VISCUM_BARK_BENCH=1 VISCUM_HAMMER=1 target/release/examples/vepres_viscum OUT --draft`: the layer evaluates to non-zero heights, but the finished mesh is unchanged.
2. **Seat pads that do not fold over a concave crease.** Clamp a pad's normal offset by the local valley radius, or blend the pad's normals over its footprint before displacing. The repro is a `SeatPadLayer` gypsy mound at the 003 centre, `(θ 87°, v 8.1)`: it gives 114 self-crossings and "its seat could not be cut".
