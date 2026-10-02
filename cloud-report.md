# Vepres Rubus: cloud report

Branch `claude/vepres-rubus`, from `master`. Example: `crates/ringdesign-core/examples/vepres_rubus.rs`. Outputs: `showcase/vepres/rubus/`.

## Outcome: stopped at the block-out; the subject needs rethinking

None of the three block-outs read as a bramble at 300 px, so under the loop's rule I stopped and did not build the detail rounds.

- Read tests: 3 of 3 used, all `reads: false`.
- Review rounds: 0 of 3 used. No full review was run, so there is no score and no ship or cut verdict.
- Every draft gate the block-out can run is green (below). The failure is the read alone, as with the Bestiarium and Cataphracta cuts.

| Read test | reads | What the eye saw (reviewer's words, shortened) |
|---|---|---|
| 1 (`read-test-1.json`) | false | "A punk spiked band or a crown of thorns with rivets, not a plant." The prickles looked like needle-thin pins, the 2.3 mm berries like studs or flower rosettes, and there were no leaves. |
| 2 (`read-test-2.json`) | false | "Thorny vine ring", closer to barbed wire than to bramble. The berries were about 8 px flat granulated discs ("daisies or rivets"), and the leaves showed as 4 px almonds with no teeth. Most of the face view was bare polished metal. |
| 3 (`read-test-3.json`) | false | "Thorn ring" or "crown of thorns", perhaps "briar". The berries merged into a lumpy beaded rim ("encrustation or barnacles"). The trifoliate crown leaf read as "an arrowhead or chevron, the exact failure Logan named". |

### What each attempt changed

- **Attempt 1.** The section's block-out as written:
  - the knuckled cane with P5's keys;
  - 13 prickles, each a 1.1 mm round twist with a 70° hook;
  - 14 blackberry decals, 2.3 mm across, seven drupelets each.
- **Attempt 2.** Read test 1's changes:
  - The prickles got flattened elliptic feet, 1.8 × 1.1 mm (round the ring × along the finger), and a shorter 45° hook.
  - The node thickness key went from 1.15 to 1.25.
  - Added the 0.8 × 0.45 mm side-face runner, phased to crest at the nodes.
  - Added 14 serrated leaflet stamps, 4.2 × 1.9 mm, with a midrib ridge.
  - Domed berries with 19 hex-packed drupelets.
  - Parting fixed at z = 0. Before this, auto-parting had judged the field at z = −0.04, which made the prickles "Castable with care".
- **Attempt 3.** Read test 2's changes, as far as sand allows:
  - The upper nodes swell to a thickness key of 1.35; the palm nodes stay at 1.15.
  - The prickles grew again: the large ones to 2.0 × 1.2 mm with a 2.0 mm, 45° hook, the small ones to 1.5 × 0.9 mm.
  - The runner went to 1.1 × 0.55 mm.
  - Clusters of three berries at each upper node, with a stalk and a calyx star in the art.
  - A trifoliate leaf on the crown of each upper internode, where the face camera looks.

### Why it does not read: the diagnosis for the rethink

1. **The side faces are too small to carry the fruit.** On a 3.4 mm LowDome with flat sides, the station-aware gate gives 2.16 to 2.75 mm of face, even with the knuckle key at 1.35.
   - Berries larger than about 2.4 mm are clipped square by the side-face gate. In attempt 3 I tried 3.0 mm and the clusters rendered as blocks.
   - Read tests 1 to 3 each asked for 3.4 to 3.8 mm berries. That cannot be done on these faces in sand.
   - The hero and face cameras also see the side faces obliquely or edge-on. The lessons file says side-face work supports a read but cannot carry one, and all three tests confirmed it.
2. **The crest in sand only takes relief that is monotone through the parting line.** A trifoliate leaf has laterals off the parting line, and `parting_monotone` refuses them.
   - Laid as separate stamps, they failed on every outline point, and the ray release found 16 obstructions up to 1.4 mm deep.
   - The legal form is the filled outline, with the sinuses cut back at the bench (the Ilex fallback). That casts and pulls cleanly, but it reads as an arrowhead.
3. **The prickles read as needles at 300 px**, even with flattened 2.0 mm feet. Seen from above, a hook lying in the parting plane is a short oval: the "row of studs" every test describes.

### Recommendation

Rethink Rubus before detailing it. The candidate that answers all three diagnoses is lost wax, which the brief allows when sand costs the read:

- **Berries as true 3-D CAD parts.** Drupelet clusters, 3.5 to 4 mm across, with their calyces, seated on the crown shoulders where both cameras look.
- **Trifoliate leaves as real separate leaflets** draped over the shoulders.
- **Prickles that splay out of the parting plane**, so the face view shows hooks rather than ovals.

If the ring must stay in sand, the cane needs a far thicker section, about 5 mm, so the side faces reach 3.5 mm. Even then the side faces cannot carry the read on their own.

## Gates (block-out attempt 3, draft build 768 × 320)

| Gate | Result |
|---|---|
| Finished mesh | Watertight, 0 degenerate faces, 0 self-crossings, 542,972 triangles |
| CAD parts | All 5 made parts have 0 self-crossings. Features #1 to #6 are all `FeatureStatus::Ok`. |
| Notes | `built.solids.notes` and `built.parts.notes` are empty. 38 of 38 stamps resolved (20 of them are bench cuts). |
| Finger hole | 0 vertices inside. The nearest vertex is at 9.29999 mm against a 9.3 mm bore radius. |
| Field | `attributed_field_report` 256 × 128 plus `judge_parts`: **Castable**, 0.0011% undercut, worst −6.1°. Parting fixed at z = 0, the set-up's own. |
| Ray release at 0.100 and 0.075 mm | 0 obstructions and 0 unresolved at both pitches. The status reads "Review", not "Blocked". |
| Draft clamp | Not used. No relief is painted through a `Hide`, so there is no clamp bite. |
| `parting_monotone` | Every cast stamp passes; bench stamps are exempt. |
| DFM | `dfm::findings_in`: 0 findings |
| Stones | None, by design |
| Side-gate probe row (P5) | For keys node {w 1.05, t 1.35, c 1.10}, palm node {1.03, 1.15, 1.05} and internode {0.97, 0.93, 0.95}: across 360 stations, the gate spills 0.000 mm past each station's own faces. The narrowest gated face is 2.16 mm. Recorded in `report.json`. |

**Not run**, because the loop stopped before round 1 (the block-out only renders at draft):

- the export build at 1536 × 448 and its 2 M triangle budget;
- `--verify` (cold reload);
- the casting-pattern mesh;
- the 384 × 192 stamp re-check;
- the template gate.

Since no round was reached, there are no template-gate numbers to record.

## The ring as authored (attempt 3)

### Base

- `RingDesign::default()`, bore 18.6 mm.
- LowDome profile, 7.0 × 3.4 mm: crown 0.6, `flatten_sides`, edge round 0.3, comfort fit 0.1.
- Delft sand from `probe::sand_setup(0.1)`, with `draft.auto_parting = false` and parting at z = 0.
- `ShankKind::Keyframes` with amount 1.0 and 14 keys. Nodes sit at 90 + k·51.43°, internodes at node + 25.71°.

### CAD feature tree

1. **#1 Cane** is the procedural band (`Operation::Band`, role Shank).
2. **#2 Large prickle** is a twisted sweep:
   - its foot is a 48-point elliptic polyline, 2.0 mm round the ring by 1.2 mm along the finger;
   - its path rises 0.7 mm radially, then turns 45° round a 2.0 mm bend;
   - it tapers to 0.26 of the foot, so the tip is 0.31 mm across the parting plane.
   - It is seated at 347.14°, sunk 0.35 mm, joined with a 0.35 mm seam bead, cast stage, with no cant, spin or tilt.
3. **#3 Ring array** repeats #2 five times over 205.71°, so the large prickles stand on the nodes at 347°, 39°, 90°, 141° and 193°.
4. **#4 and #5 Small prickles** are twisted sweeps with a 1.5 × 0.9 mm foot, a 0.5 mm rise and a 1.4 mm, 42° bend, tapering to 0.36. They stand at 20.07° and 28.57°, in the upper half of each upper internode, clear of the crown leaf and of every hook.
5. **#6 Ring array** of #4 and #5 together, four times over 154.29°, gives the 8 small prickles. Multi-source, so format 6.

### Layers

- **Runner:** a `CurveLayer`, Round profile, 1.1 × 0.55 mm.
  - One arch per node cell, `mirror_v`, landed on the side face at 0.7 fill and gated `SideFaces(Both)`.
  - C-B1's `phase` has not landed, so the control points are shifted by `fract(90/51.43) + 0.25` of a cell. That puts the arch at the crown edge at each node.
- **Blackberries:** a `DecalLayer` reading the SVG alpha "Drupelet cluster", gated `SideFaces(Both)` with `Blend::Max`, 34 decals.
  - The art is hex-packed radial-gradient drupelets over a receptacle that stays above half ink, so the sand sees no gap finer than its floor.
  - It also carries a stalk and a calyx star.
  - At each upper node a 2.15 mm berry sits on the node, with a berry at 0.92 scale either side at ±9.5°. Each palm node has one berry at 0.92 scale.

### Stamps

- **Leaflets 1 to 14** sit one per internode per face:
  - outline: serrated, 4 teeth at 0.22 mm, 4.2 × 1.9 mm;
  - top: a 0.4 mm `Ridge` midrib over 0.3 mm eaves;
  - `along_pull`, 3° draft;
  - laid along the runner's tangent, alternately turned ±8°.
- **Crown leaves 1 to 4** sit one per upper internode, springing 1.6 mm past the lower node.
  - Each is one filled outline: the |y| envelope of a 3.6 × 2.2 mm terminal leaflet (6 teeth) and two 2.9 × 1.5 mm laterals (5 teeth) spread at ±46°.
  - The top is a 0.3 mm `Gable` whose ridge lies in the parting plane, over 0.35 mm eaves.
- **Crown leaf sinuses 1 to 8** are tier-1 bench cuts. They remove the plan between the terminal, or the stalk, and each lateral, sunk 0.45 mm from the leaf's top.
- **Crown midribs 1 to 12** are tier-1 bench cuts, each a 0.18 mm lanceolate groove 0.12 mm into its leaflet.

### Stones

None.

## What I could not do

- **Make Rubus read.** This is the whole verdict above.
- **Keep the brief's exact knuckle keys.** The node thickness key went from 1.15 to 1.35 on the upper nodes, at the reviewer's request and to enlarge the side faces. The side-gate row confirms zero spill with the new keys.
- **Keep the small prickles where the section put them.** They moved from 4.29° and 21.43° to 20.07° and 28.57°, so the crown leaf has room and no hook overhangs it. With hooks over the leaf the release was blocked, at up to 0.78 mm deep.
- **Lay the side-face veins.** The section's bench-cut veins and withered-leaflet bites belong to round 1, which was never reached.

## Core changes wanted

C-B1's `CurveLayer::phase`, so the runner's crest can be set without rewriting its points. In `crates/ringdesign-core/src/curve.rs`:

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
