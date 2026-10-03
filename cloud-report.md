# Vepres ring: Hedera (`hedera`) — cloud report

The work is on branch `claude/vepres-hedera`, merged with master `2e11632`. The ring is `crates/ringdesign-core/examples/vepres_hedera.rs` and its outputs are in `showcase/vepres/hedera/`.

## Verdict: stopped at the block-out (no review rounds)

All three block-out read tests failed. TASK.md says to stop after the third and report that **the subject needs rethinking, not detailing**, so no full-review rounds ran: **0 of 3 used**, and there was no ship/cut score. Every gate is green at draft and at export. The template gate fails on graph size alone (below).

| Read test | reads | What the reviewer saw at 300 px (summary) |
|---|---|---|
| 1 | **false** | A blackberry or mulberry on a vine. The leaves read as holly or thistle (thin, spiky lobes), and the packed berry dome as one compound fruit. That collides with Rubus. |
| 2 | **false** | Closer: one shoulder leaf reads as "ivy or maple". The crown leaves are puffy five-pointed stars (maple, sweetgum). The collets read as gold tubes or "cannon barrels". The stem does not register, and the stamp rootlets read as tyre tread. |
| 3 | **false** | Closer again: the berries now stand apart on stalks. But the leaves still read as five-pointed stars (maple, starfish), and the berries as black pips in thick gold cups (bezel gems, a molecule model). The rootlets read as a tick ladder, and the stem still cannot be traced. |

The full JSON is in `read-test-1.json`, `read-test-2.json` and `read-test-3.json`. Each test used a fresh reviewer, given only `target/review.md`, the name and slug, the mode, the attempt number and the paths.

### Why it does not read, and what a rethink should change

1. **The ivy leaf loses its identity on a 5.5 mm band.** At 300 px a palmate leaf seen on a curved crown reads as a generic star. The reviewers asked for three things the build rules work against:
   - a near-flat blade 0.35 to 0.4 mm high;
   - the 0.8 mm lost-wax section on every body part;
   - blunt tips of at least 0.3 mm on a 5 to 6 mm leaf.

   Together these give a plump star. A rethink should make the leaf the hero at about twice the size: one large adult heart or spade leaf, or a three-lobed juvenile leaf, across the whole crown, seen square-on. It should not be several mid-size leaves crowded round a cluster.
2. **Seven 2.2 mm cabochons in collets cannot look like berries** under the gates. The bezel builder always drives a collet down to the highest metal under it, which makes tubes. The fix was a receptacle face seat, so each collet stands on its own stalk. Even then, a 0.45 mm wall plus its foot leaves more gold than black at 300 px. The reviewers asked for 0.25 mm walls, which no lost-wax floor here accepts. A rethink should use fewer, larger stones (3 to 5 at 3 to 4 mm), or no stones at all, with cast metal berries.
3. **A stem on a 2 mm-thick band hides behind its own leaves and the umbel.** The vine needs a thicker host, or a split band whose second rail is the stem, before it can carry the read.

## What was built (final block-out, attempt 3)

- **Host:** procedural `ShankKind::Uniform`, D-shape 5.5 × 2.0, edge round 0.55 mm, comfort fit 0.15, bore 18.6 mm. Lost wax at the investment floors: 0.8 mm section, 0.15 mm detail, no draft.
- **Process:** lost wax, as the section planned, now also under Logan's 2026-10-03 rule; the decision is recorded in Hedera's section of `docs/collections/vepres.md`. It was not tried in sand: the undercut leaves and the berry spray cannot pull.

### CAD feature tree (49 features)

- **#1 Host band:** the procedural band.
- **#2 Ivy stem:** a stored tube, Ø 1.52, 38% sunk.
  - It runs 330° from its cut end at 250° to a growing tip that tapers to Ø 0.88 over its last fifth.
  - It swings ±2.1 mm across the crown on two waves a turn, crossing the crest under the umbel at 90°.
- **#3 to #14: six ivy leaves, each with a petiole.**
  - **The leaves** are stored solids laid on the band through a chart of its section.
  - **Outline:** a polar outline about the hub, with a long terminal lobe, two laterals at about 0.62 of its length, two small basal lobes and a cordate notch. The tips are rounded about 0.4 mm.
  - **Blade:** near-flat (0.40 to 0.50 mm over the band) with five raised palmate veins, 0.14 mm high.
  - **Thickness:** at least 0.85 mm, measured along the blade's own normal, with a 0.08 mm top-edge round.
  - **Over the edges:** lobes reaching past the crown run out on a drooping surface, capped at 12°, so they break the band's outline. The crown leaves curl up 0.3 mm at the tip.
  - **Petioles:** Ø 0.84 tubes. Each runs straight back from its leaf's notch to where that line meets the stem.
  - **Where they sit:** the crown leaves at 60°/120°, the shoulder leaves at −14°/194°, and the low leaves at 214°/326°.
- **#15 Peduncle:** from the stem's crossing up to the umbel's hub, 0.6 mm over the crown.
- **#16 to #29: seven pedicels (Ø 1.0) and seven receptacles.**
  - The berry directions were solved so that:
    - no two collets come within 0.5 mm of each other;
    - each clears the band and the leaves;
    - the spray stays within 3 mm of the crown round the ring.
  - Each receptacle is a `Revolve` (in plane): a disc the collet's foot sits on, with a 0.2 mm square band, flaring like a calyx into the stalk.
- **#30 to #37: rootlets, using C-V2.** There are two pairs of stored rootlet sources, one each side, and two `PatternKind::Along` arrays. Each array follows the stem's chart path (`AlongPath::Chart`) at a 1.0 mm pitch, the second set offset half a pitch and leaning the other way. These replaced the 117 stamp rootlets of attempts 1 and 2.
- **#100 to #113: seven stones and their collets.**
  - Seven black spinel round cabochons, 2.2 mm (tint 0.03, 0.03, 0.04), each a `stone_on_face` seated on its receptacle's planar face through a `FaceSeat`.
  - Each has a `head.bezel` with a 0.45 mm wall and a 0.1 lip.
  - Standing on the receptacle stops the collet at the receptacle, not at the band.
- **Layers:** none. **Stamps:** none in the final design.

### Enablers used from master

| Enabler | Used? |
|---|---|
| C-V2 `Along` (`AlongPath::Chart`) | Yes, for the rootlets. |
| #248 framed renders | Yes, for the crown and stone close-ups. |
| C-V3 (sweep scale laws) | No: the stem stayed a stored tube for its taper. |
| C-V1, C-V4, C-V5 | No. |
| C-B2 (true pear and other plans) | Not needed: round stones only. |

## Gates (draft 768 × 320 and export 1536 × 448)

| Gate | Draft | Export |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | yes, 0, 0 | yes, 0, 0 |
| Every CAD part closed, 0 crossings | pass | pass |
| Solids and parts notes empty, every feature Ok | pass | pass |
| Nothing in the finger hole | 0 vertices inside (nearest 9.300 mm vs bore 9.3) | 0 inside |
| Lost-wax verdict | Castable, thinnest wall 1.57 mm | Castable, 1.57 mm |
| Ray-sampled walls | body parts ≥ 0.815 mm; collets 0.449, receptacles 0.201, rootlets 0.212 (detail floor 0.15) | same |
| DFM findings | 0 | 0 |
| Stones reported = previewed; metal inside | 7 = 7; 0 in every stone; closest pair 1.47 mm at the girdle | same |
| 384 × 192 rebuild | pass | pass |
| Casting pattern closed, 0 degenerates, 0 crossings | pass (554,470 triangles) | pass (1,231,390 triangles) |
| Triangle budget (2 M) | — | 1,231,390 |
| Cold reload with an empty library | — | identical |

The design is 18k gold, 13.64 g, with 0.31 ct of stones, and is saved at format 6 (a `Revolve` is `in_plane`).

**The declared wall exception.** Body parts hold the 0.8 mm section. The settings (collets and receptacles) and the rootlets are held at the 0.15 mm detail floor, as Rubus's prickles and Manticora's aculeus were. This is stated in the gate's own wording in `report.json`. Without it, the read tests' demand for thin bezel rims could not be met at all.

## Template gate

Run as `collection_templates vepres … --only hedera --verify-export` with class `procedural`. The record is copied to `showcase/vepres/hedera/verification.json`.

| Check | Result |
|---|---|
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **1,340,180 B** against the 300 KB procedural budget: **fails**, needs review |
| Nodes | 69 |
| Cold source (lift) | identical |
| Mesh parity | vertices, faces and normals identical |
| Graph and design reload | pass |
| First build | 3.3 s |

The size comes from the stored meshes: the stem, six leaves and seven pedicels.

## What I could not do

- **Make it read as ivy within three block-outs.** See above.
- **Meet the template budget.** Conforming leaves and a tapering stem are stored meshes, and they are heavy in the graph.
- **Use builder collets as free-standing berry cups** without a receptacle part under each one.

## Core changes wanted (exact code)

1. **A bezel that keeps its own depth.** Let a collet stop at its own depth instead of reaching for the metal under it, so a berry on a stalk needs no receptacle part. In `crates/ringdesign-core/src/cad/builders.rs`, add the parameter to the `BEZEL` params:

   ```rust
   BEZEL => vec![
       number("wall_mm", "Wall", "mm", 0.25, 1.5, setting::collet_wall_mm(gem)),
       number("lip", "Lip", "of crown", 0.1, 0.8, setting::collet_lip(gem)),
       Param { key: "own_depth", label: "Own depth", unit: "", min: 0.0, max: 1.0, kind: Kind::Flag, default: json!(false) },
   ],
   ```

   Then, in `build_in`'s `BEZEL` arm:

   ```rust
   let metal = if v.b("own_depth") { seat.surface_z } else {
       floor.and_then(|f| under_wall(gem, v.f("wall_mm"), f)).unwrap_or(seat.surface_z).min(seat.surface_z)
   };
   ```

   Set `own_depth: true` from `geometry_extended`, so older readers are fenced.

2. **Graph weight of stored meshes.** A `Stored` recipe whose `kernel` names a registered ring-local generator could be lifted as its parameters rather than its mesh. Failing that, a `stored.leaf` graph node (polar outline, chart hub, axis, curl) and a `stored.tube` node (chart path, radii) would make Hedera-class rings fit the 300 KB procedural budget.

## Commits on `claude/vepres-hedera`

- The block-out and read test 1.
- The master `2e11632` merge, block-out attempt 2 and read test 2.
- Block-out attempt 3 and read test 3.
- This report, with the export gates, the template-gate record and the doc line.
