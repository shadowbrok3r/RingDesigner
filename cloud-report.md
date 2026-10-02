# Vepres Rubus: cloud report

Branch `claude/vepres-rubus`, from `master`. Example: `crates/ringdesign-core/examples/vepres_rubus.rs`. Outputs: `showcase/vepres/rubus/`.

## Outcome: cut at round 3, score 6.7

Rubus now reads as a bramble at 300 px. It never reached the 7.5 ship bar, and the round-3 reviewer cut it at 6.7.

| Item | Count |
|---|---|
| Block-out read tests | 4 (three in Delft sand failed; the fourth, in lost wax, read) |
| Review rounds | 3 of 3 |

The fourth read test and the three review rounds were run after Logan asked for a lost-wax rebuild that keeps the earlier work. Under `TASK.md` alone the ring would have stopped after three failed block-outs.

| Step | Verdict | Score | What the reviewer saw |
|---|---|---|---|
| Read test 1 (sand) | reads: false | n/a | "A punk spiked band or a crown of thorns with rivets." Pin prickles, berries like studs or rosettes, no leaves. |
| Read test 2 (sand) | reads: false | n/a | "Thorny vine ring", closer to barbed wire. 8 px berry discs, toothless leaves. |
| Read test 3 (sand) | reads: false | n/a | "Crown of thorns." Beaded-rim berries, and the crown leaf read as "an arrowhead or chevron". |
| Read test 4 (lost wax) | **reads: true** | n/a | "Blackberry … thorns plus blackberries name a bramble." Only just: the berries looked like finials and the crown leaves like arrows. |
| Round 1 | revise | **6.3** | The berries and hooked prickles carry the read. Leaves stair-stepped; calyx tabs looked flat; cups around the small prickles; crest bare. The wall and template record was incomplete (897 KB design). |
| Round 2 | revise | **6.6** | Gate record green and honest. Leaves still stair-stepped. Palm berries broke the bare-palm rule. Flat sepals, pin-like stalks, density below Caiman's. |
| Round 3 | **cut** | **6.7** | Palm bare and wall text fixed. But the leaf margins still render stepped or smeared, the bark reads as ruled corrugation, the small prickles are stubs, the sepals still look flat, and the berries still look like lollipops. The reviewer failed the lost-wax wall gate on the leaf parts' sampled walls (below). |

## What the last round leaves open

These are from the round-3 punch list, kept for anyone who revives the ring:

1. **Leaves (P0).** Rebuild each leaflet as a lofted or stored-mesh solid with a smooth serrate margin, at least 0.3 mm thick at the edge and blended into the face. The midrib should be folded into the leaf, not extruded as a separate thin sheet. Every leaf part must sample at least 0.15 mm on rays.
2. **Crest (P0).** Seat the small prickles with a 0.3 to 0.4 mm fillet. Remove the round recess and the almond on the centreline; that almond is the large prickle's foot seen from above.
3. **Bark (P1).** Use broken, wavering striae with node rings in place of ruled flutes. Clean the crest-edge combing.
4. **Berries (P1).** Cup and curl the sepals back on the berry. Hang each berry further down its side face.

I disagree with one point for the record, not to reopen the verdict: the stalks were bent 28° in round 3, and the reviewer still read them as straight radial pins.

## Process

The ring is **lost wax** (`probe::wax_setup`): 0.8 mm section, 0.15 mm detail, no draft. The brief allows this when sand costs the read. Sand failed three read tests for these reasons:

- **The side faces are too small for fruit.** The station-aware faces are only 2.16 to 2.75 mm, so any berry over about 2.4 mm is clipped square.
- **The crest takes only parting-monotone relief.** Separate leaflets off the parting line lock in the mould, and the filled outline the gate allows reads as an arrow.
- **The prickles show end-on.** Hooks lying in the parting plane look like studs from above.

What carried over from the sand work:
- the knuckled cane;
- the in-plane hooked prickles;
- the side-face runner;
- the trifoliate leaf outline;
- the side-gate probe row, which still shows 0.000 mm spill.

## Gates (final state, draft 768 × 320 and export 1536 × 448)

| Gate | Result |
|---|---|
| Finished mesh | Watertight, 0 degenerate faces, 0 self-crossings at both builds and at 384 × 192 |
| CAD parts | 21 made parts, each with 0 crossings. 22 features, all `Ok`. Parts and solids notes empty. |
| Finger hole | 0 vertices inside; nearest 9.2999995 mm against a 9.3 mm bore radius |
| Lost-wax field | **Castable**, thinnest band wall 2.877 mm at 220.8° |
| Part walls | See the next table |
| DFM | 0 findings |
| Stones | None: 0 reported, 0 previewed |
| Cold reload (`--verify`) | Identical at export |
| Export size | 1,372,962 triangles, under the 2 M budget |
| Casting pattern | Watertight, 0 degenerate faces, 0 crossings |
| Side-gate probe row (P5) | 0.000 mm spill over 360 stations |
| Template gate | **Pass** (see "Template gate" below) |

Part walls are sampled on rays with `cad::measure::thickness`:

| Part | Sampled minimum | Floor it is judged against |
|---|---|---|
| Berries | 1.686 mm | 0.8 mm section |
| Stalks | 0.811 mm | 0.8 mm section |
| Large prickle | 0.239 mm | 0.15 mm detail (pointed) |
| Small prickles | 0.191 mm | 0.15 mm detail (pointed) |
| Calyces | 0.243 mm | 0.15 mm detail (pointed) |
| Leaves | **0.065 / 0.076 mm** | 0.15 mm detail |
| Midribs | **0.063 to 0.125 mm** | 0.15 mm detail |

The leaf and midrib rows are below the floor. I judged the leaves by their outlines' finest strokes instead (0.467 mm for the leaf, 0.341 mm for the midrib, 0.8 and 0.32 mm thick), and the example's gate passes on that. **The reviewer rejected that substitution, so count this gate as failed.** The ray measure is also noisy on thin relief: the same leaf sampled anywhere from 0.065 to 0.165 mm between builds.

The ring weighs about 32.5 g in 18k gold, at size US 8.6.

## Template gate

Command: `collection_templates vepres … --only rubus --verify-export`, with `template_class` set to `procedural`.

| Measure | Result |
|---|---|
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **298,939 B** against the 300 KB procedural budget |
| Nodes | 57 |
| Cold source | Identical |
| Graph and design reload | Pass |
| Vertex, face and normal parity | Pass |
| Export geometry | Verified |
| First build | 1.7 s |

To get under budget:
- the decal raster went;
- the 84 leaf stamps became 8 sketch-extrude parts;
- the berry meshes were coarsened;
- the calyx, stalk and prickle polygons were trimmed.

## The ring as authored

### Base

- LowDome 7.0 × 3.4 mm with flat sides, bore 18.6 mm.
- Keyframed at seven nodes (90° + k·51.43°):
  - upper nodes {w 1.05, t 1.35, c 1.10};
  - palm nodes {1.03, 1.15, 1.05};
  - internodes {0.97, 0.93, 0.95}.

### CAD feature tree

1. **#1 Cane:** the procedural band.
2. **#2 Large prickle:** a twisted sweep at 347.14°.
   - Its foot is a 24-sided ellipse, 2.8 × 1.5 mm, rising 1.3 mm then turning 55° round a 2.7 mm bend.
   - It tapers to 0.26 of the foot.
   - Joined, sunk 0.35 mm, with no seam bead.
3. **#3:** five copies of #2 over 205.71°, so a large prickle stands on each upper node.
4. **#4 and #5 Small prickles:** twisted sweeps with a 2.2 × 1.2 mm foot and a 1.8 mm, 50° hook, tapering to 0.46.
   - They sit at the internode on the high and the low shoulder (across ±2.3 mm), canted ±50° out over the side faces.
5. **#6:** four copies of #4 and #5 over 154.29°.
6. **#7 to #12 Berry parts on the first upper node.** On each shoulder, staggered ±9.37° either side of the node, at across ±3.0 mm and cant ±55°:
   - a stalk, 0.84 mm round and 1.4 mm long, bent a further 28° down the side face;
   - a blackberry, a 3.8 × 3.5 mm ellipsoid raised into 36 drupelets;
   - a five-sepal calyx, tapering from 0.65 to 0.25 mm thick and curling 0.5 mm up the berry.
   - All three are stored meshes.
7. **#13:** five copies of #7 to #12 over 205.71°, giving ten berries on the upper nodes.
8. **#14 to #21 Side leaves.** On each side face of the first internode:
   - the trifoliate leaf's true outline extruded 0.8 mm square out of the face, sunk 0.25 mm. The outline is the marching-squares union of a 4.4 × 1.6 mm terminal leaflet and two 3.0 × 1.0 mm laterals at ±20°, all serrate;
   - three raised midribs.
9. **#22:** seven copies of #14 to #21 round the ring.

### Layers

- **Runner:** a `CurveLayer`, round wire 1.1 × 0.55 mm, on both side faces. It crests at the nodes through a control-point shift, because C-B1's `phase` has not landed.
- **Cane bark:** flutes running round the ring, 22 ribs, 0.045 mm high, over the upper 200° of the crown and kept off the palm.

### Stamps and stones

None of either. The palm crest is bare.

## What I could not do

- **Reach 7.5.** The verdict stands.
- **Keep the ring in sand.** See "Process" above.
- **Lay organic bark.** Diagonal flutes (`lean` 0.12) left degenerate faces where they met the parts.
- **Put seam beads on the prickles.** Beads folded or pinched over the bark, so there are none, which is what the reviewer read as cut walls.
- **Build procedural berries.** Revolves whose profile touches the axis would not tessellate. Lofts of whole circles were refused. Polyline lofts of overlapping drupelets self-crossed inside their patterns. Stored meshes were the only route that passed.
- **Make the leaf walls honest on rays.** See "Part walls" above.

## Core changes wanted

C-B1's `CurveLayer::phase`, in `crates/ringdesign-core/src/curve.rs`:

```rust
// in `pub struct CurveLayer`, after `mirror_v`:
    /// Shift of every instance along the ring, in cells (0..1), so arches can crest on chosen stations.
    #[serde(default)]
    pub phase: f64,

// in `impl Default for CurveLayer` (and every struct literal of it):
            phase: 0.0,

// in `CurveLayer::height`, replacing the `x_frac` line:
        let x_frac = ((uv.u / circ).rem_euclid(1.0) * repeats - self.phase).rem_euclid(repeats);
```

A deterministic `cad::measure::thickness`: sample every face of a component rather than a stride, so a ring's wall record does not move between builds. In `crates/ringdesign-core/src/cad/measure.rs`:

```rust
pub fn thickness_all(mesh: &Mesh, limit_mm: f64) -> Thickness {
    // as `thickness`, with `let stride = 1;` and the 250 000-face guard raised to the part's own face count
}
```
