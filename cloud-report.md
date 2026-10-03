# Vepres Rubus revival: cloud report

Branch `claude/vepres-rubus-revival`, from `claude/vepres-rubus`. `master` was merged twice: `2e11632` before round 4, and `29babc4` (#260, crisp relief through the lift) before round 5.

- Example: `crates/ringdesign-core/examples/vepres_rubus.rs`.
- Outputs: `showcase/vepres/rubus/`.

## Outcome: cut at round 5, score 6.8

Logan's 2026-10-03 extension gave Rubus two more reviewed rounds. Round 4 rose to 6.9 and round 5 fell back to 6.8. Round 5 was the last round, so the reviewer cut the ring. Every gate is green at both builds. The verdict stands.

| Step | Verdict | Score | What the reviewer saw |
|---|---|---|---|
| Read test 4 (lost wax) | reads: true | n/a | "Blackberry … thorns plus blackberries name a bramble." |
| Round 1 | revise | 6.3 | Stair-stepped leaves, flat calyx tabs, cups round the small prickles, a bare crest. |
| Round 2 | revise | 6.6 | Gate record green. Still: stepped leaves, berries on the palm, flat sepals, pin stalks. |
| Round 3 | cut | 6.7 | Stepped leaves, ruled bark, stub prickles, flat sepals, lollipop berries. The leaf walls sampled under the floor on rays. |
| **Round 4 (revival)** | **revise** | **6.9** | "The best Rubus so far." Organic bark, leaves no longer stepped at overview scale, an honest gate record. Still open: see below. |
| **Round 5 (revival)** | **cut** | **6.8** | Reads as a bramble at 300 px, with the most distinct silhouette in Vepres. Workmanship items remain: see below. |

What round 4 left open:
- the leaves are too small and ragged at 1600 px;
- the sepals are flat cross plates;
- the stalks look like pins with flat-cut ends;
- the prickle roots have cut walls;
- almond bosses sit on the crest;
- tabs show at the crest edges;
- the side faces are bare.

What round 5 found:
- the enlarged leaves render stair-stepped and castellated, with no visible midrib;
- the sepals are still flat plates;
- the almond still sits on the crest centreline;
- the crest-edge tabs are larger;
- one stalk passes through a large prickle's base and shows below it as a cut rod;
- the node rings read as hairline seams.

## What the revival changed

### Round 4

- **Merged master and moved the example to the new API:**
  - the twist's `scale` and `closed`;
  - the placement's `level`;
  - the curve's `widths`, `heights` and `beads`.
- **Framed close-ups** with `render::write_png_framed`, never a cropped mesh: `crown-close`, `stones` and `cane-close`.
- **Made the leaves stamps again.** Each is a serrate leaflet outline with a petiole, struck off the side face with `StampTop::Pillow`, at every internode on both faces. These replace the 8 extruded leaf plates and their midrib sheets.
- **Rebuilt the bark.** It is the procedural `Bark` recipe, quarter-turned so its broken, wavering furrows run along the cane, and tiled continuously. It replaces the 22 ruled flutes.
- **Grew the prickles from flared feet** (a scale law on the twist). Seam beads folded on the bark, so the prickles carry none.
- **Found and fixed a cant sign error.** `cad::ring_seat` leans a positive cant toward −z.
  - Rounds 1 to 3 used +55° on the high shoulder, so the berries leaned in over the crown.
  - The stalk's bend then stood them back up into the "radial pins" every reviewer named.
  - The small prickles had the same error.
- **Made each blackberry one body.** Stalk, berry and calyx are unioned once in the example.
  - The calyx is a shell laid on the fruit: five sepals, with their angles spaced by arc length.
  - The low shoulder's fruit is a mirror of the high one, which keeps the template in budget.
- **Recorded Logan's extension and lost-wax ruling** in the ring's section of `docs/collections/vepres.md`.
- **Left `crisp_relief` off.** The lift could not yet carry it: its `design.set` at `/crisp_relief` failed.

### Round 5

- **Merged master `29babc4` and turned `crisp_relief` on**, now that #260 carries it through the lift.
- **Enlarged the leaves** to 4.2 × 1.8 mm, with seven 0.27 mm teeth, a 0.26 mm margin and a `StampTop::Ridge` along the midrib.
- **Replaced the five almond leaf scars** with node rings at all seven knuckles.
- **Filleted the small prickles** 0.3 mm into the bark, and eased their cant to 30° so the bead neither folds nor pinches.
- **Carried the bark down both side faces** as a second tiling layer.
- **Reworked the berries:**
  - the seat's cant is eased to 20°, because at 55° the root stood up out of the crest;
  - the stalk bends 50°;
  - the drupelets are summed rounded bumps;
  - the calyx is thicker and lies on the fruit.

Two things I tried in round 5 and dropped:
- **A sunk bed under each leaf, to remove the crest tabs.** The platform refuses a joined stamp standing on a cut.
- **A keyhole groove round each leaf plus a bench-cut vein comb.** Both passed every gate, but the template graph came to 396 to 623 KB against the 300 KB budget. The graph is pretty-printed, so each stamp outline point costs about 40 to 60 B.

The round-5 reviewer named the cost of dropping them: no visible midrib, stepped teeth at 1600 px, and the edge tabs.

## Gates (final state)

Draft is 768 × 320 and export is 1536 × 448. The ring is judged as lost wax, per Logan on 2026-10-03.

| Gate | Draft | Export |
|---|---|---|
| Finished mesh watertight, 0 degenerate faces, 0 crossings | pass (613,382 triangles) | pass (1,426,428 triangles) |
| Every CAD part and stamp solid closed, 0 crossings | pass | pass |
| Solids and parts notes empty; 21/21 stamps; features #1 to #9 Ok | pass | pass |
| Nothing in the finger hole (nearest 9.2999995 mm against a 9.3 mm bore) | pass | pass |
| Part walls on rays (see below) | pass | pass |
| Leaf finest stroke 0.408 mm; margin 0.26 mm over the face | pass | pass |
| Lost-wax verdict **Castable**; thinnest band wall 2.872 mm at 220.8° | pass | pass |
| 0 DFM findings | pass | pass |
| Stones reported = previewed (none) | pass | pass |
| Gates hold at 384 × 192 (0 crossings, 0 notes) | pass | pass |
| Casting pattern watertight, 0 degenerate faces, 0 crossings | pass | pass |
| Within 2 M triangles | pass | pass |
| Cold reload (`--verify`, empty library) | n/a | **identical** |
| Side-gate probe row (P5) | 0.000 mm spill | 0.000 mm spill |

Part walls are sampled with `cad::measure::thickness`:

| Part | Sampled minimum | Floor it is judged against |
|---|---|---|
| Large prickle | 0.203 mm | 0.15 mm (pointed detail) |
| Small prickles | 0.179 mm | 0.15 mm (pointed detail) |
| Blackberry with its calyx and stalk, as one body | 0.268 mm | 0.15 mm (sepals) |
| Its stalk alone | 0.907 mm | 0.8 mm section |
| Its berry alone | 2.177 mm | 0.8 mm section |

The leaves and node rings are stamps: relief, not walls. `made_solids` keeps a ray sample of each stamp's prism, labelled `ray_diagnostic_non_gating_mm`. It reads hundredths of a millimetre even across a stamp 0.36 mm deep, so it does not measure a stamp's section.

The ring weighs about 32.0 g in 18k gold, at US 8.6.

**Sand bonus:** not established. I did not run the sand field or ray release on this lost-wax build. The hanging fruit, the canted prickles and the side-face leaves would each lock a two-part mould, so I do not expect it to pull.

## Template gate (after the last round)

Command: `collection_templates vepres target/tpl-src --output-dir target/tpl --only rubus --verify-export`, with `template_class` set to `procedural`.

| Measure | Result |
|---|---|
| Passed | **yes** |
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **285,897 B**, against the 300,000 B budget |
| Nodes | 79 |
| Cold source | identical |
| Cold graph reload | pass |
| Vertex, face and normal parity | pass |
| Export geometry | verified |

`crisp_relief` is on and travels through the lift (#260), so the parity mesh is built with it.

## The ring as authored

### Base

- A LowDome profile, 7.0 × 3.4 mm, with flat sides and an 18.6 mm bore.
- Keyframed at seven nodes (90° + k·51.43°):
  - upper nodes {w 1.05, t 1.35, c 1.10};
  - palm nodes {1.03, 1.15, 1.05};
  - internodes {0.97, 0.93, 0.95}.
- Lost wax: 0.8 mm section, 0.15 mm detail. `crisp_relief` is on.

### CAD feature tree

1. **#1 Cane** is the procedural band.
2. **#2 Large prickle** is a twisted sweep at 347.14°.
   - Its foot is a 24-sided ellipse, 2.8 × 1.5 mm, rising 1.3 mm and then hooking 55° round a 2.7 mm bend.
   - A scale law flares the foot to 1.25× where it leaves the bark and tapers it to a 0.21 mm point.
   - It is joined to the cane and sunk 0.35 mm.
3. **#3** sets five copies of #2 over 205.71°, one on each upper node.
4. **#4 and #5 Small prickles** are twisted sweeps at each internode, one on each shoulder.
   - The foot is 2.0 × 1.1 mm, rising 0.85 mm and hooking 62° round 2.0 mm.
   - It flares to 1.2× and tapers to 0.14 of its width.
   - They sit 2.3 mm either side of the centreline, canted 30° out over the side faces, each filleted 0.3 mm into the bark.
5. **#6** sets four copies of #4 and #5 over 154.29°.
6. **#7 Blackberry with its calyx and stalk, high shoulder** is one stored solid, the union of three pieces made in the example:
   - a stalk 1.0 mm thick tapering to 0.92 mm, rooted 1.2 mm into the cane and bent 50° over a 1.3 mm arc;
   - a 3.8 × 3.5 mm berry of 46 summed, rounded drupelets;
   - a calyx shell of five sepals laid on the fruit up to 55° from its foot, 0.4 mm thick at the root and 0.2 mm at the tip, its inner skin buried 0.5 mm.

   It sits 9.37° behind the node and 3.1 mm across, canted 20° out.
7. **#8** mirrors #7 across the band's mid-plane for the low shoulder.
8. **#9** sets five copies of #7 and #8 over 205.71°: ten berries on the upper nodes. The palm stays bare.

### Layers

- **Runner:** a round wire 1.1 × 0.55 mm on both side faces. It fills 70% of each face and crests at the nodes, thinning to 30% under each leaf.
- **Cane bark:** the procedural `Bark` recipe, quarter-turned, 13 tiles, 0.08 mm high, over the upper 200° of the crown, gated to a 4.6 mm band.
- **Cane bark, side faces:** the same bark, 0.07 mm high, on both side faces all round.

### Stamps (21)

- **14 bramble leaves**, one at each internode on each face.
  - Each is a 4.2 × 1.8 mm serrate leaflet with seven teeth and a petiole, its outline smoothed with a 0.045 mm gaussian.
  - The margin stands 0.26 mm proud and the leaf is sunk 0.25 mm, with a 0.20 mm ridge along the midrib.
  - Neighbouring leaves point alternately back and forward round the ring, each turned 6°.
- **7 node rings**, one across the crown at every knuckle: 6.4 × 0.5 mm, 0.06 mm proud plus a 0.06 mm pillow.

### Stones

None. Rubus is the collection's all-metal piece.

## What I could not do

- **Reach 7.5.** The reviewers scored 6.9 and 6.8, and the verdict stands.
- **Give each leaf a visible midrib and laterals, and lose the crest-edge tabs, within the 300 KB template budget.**
  - A keyhole groove and a vein comb per leaf passed every gate, but took the graph to 396 to 623 KB.
  - A sunk bed is refused, because a joined stamp may not stand on a cut.
- **Keep the stalks clear of the large prickles.** The round-5 reviewer found one stalk passing through a large prickle's base (`stones.png`). The berries hang 9.37° behind each node, and the prickle's flared foot now reaches them. I did not catch this before the review.
- **Make the calyx read as cupped sepals, not a cross plate.** To stay above the 0.15 mm floor on rays, the shell must stay about 0.2 mm thick or more at its rim.
- **Smooth the drupelets fully.** The berry mesh is held to 36 × 72 to fit the budget, so its close-ups still show facets.

## Core changes wanted

1. **Compact numeric arrays in graph files.** `file::graph_to_string` pretty-prints each `[x, y]` across four indented lines, so a stamp costs about three times its compact size. Write arrays of numbers, and arrays of number pairs, on one line. In `crates/ringdesign-graph/src/file.rs`:

```rust
// in graph_to_string, in place of serde_json::to_string_pretty(&value):
let mut out = Vec::new();
let mut ser = serde_json::Serializer::with_formatter(&mut out, CompactArrays::new(serde_json::ser::PrettyFormatter::with_indent(b"  ")));
value.serialize(&mut ser)?;
// CompactArrays: a serde_json::ser::Formatter that forwards to the PrettyFormatter, except that
// inside an array whose first element is a number (or an array of numbers) it writes no newlines
// or indentation.
```

2. **Let a joined stamp stand on a cut's floor**, for sunk relief. In `crates/ringdesign-core/src/setting.rs`, `overhang` should count a point that lies inside a lower cut, more than 1e-3 mm from that cut's edge, as carried rather than as spill:

```rust
// in overhang(), replacing the test that marks every point inside a lower cut as bad:
let on_floor = |q: [f64; 2]| cuts.iter().any(|c| clear(c, q));
if !(joined.iter().any(|p| clear(p, *q)) || on_floor(*q)) {
    bad.insert(*i);
}
```
