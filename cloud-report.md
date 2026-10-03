# Officina ring: Torsade (`torsade`): cloud report

Branch `claude/officina-torsade`, cut from `master` at `8e5a59a`. Master was merged in three times: at
`6f7bce0` (crisp renders, PR #248), at the C-V1–C-V4 / C-B1 / C-T2 batch, and at `60b3881` (C-B2 and
C-V3/C-T7). The example is `crates/ringdesign-core/examples/officina_torsade.rs`; outputs are in
`showcase/officina/torsade/`. No `src/` edits.

## Verdict: **cut at 6.9** after the block-out and all 3 review rounds

| Step | Verdict | Score | What the reviewer said, in brief |
|---|---|---|---|
| Read test 1 (block-out) | **reads** | n/a | "Cab solitaire on a band with rope-twist edges", at once. Twist too tight, collet heavy. |
| Round 1 | revise | 6.1 | The rope reads. The collet is three stacked cylinders, there are no seam beads, the crest is flat and the bore sharp. |
| Round 2 | revise | 6.6 | The collet is now one revolve with a rounded lip, but it still sits like a coin on edge. Still no beads, and the stone report said "no setting holds this stone". |
| Round 3 | **cut** | 6.9 | Every gate is green, the template gate is recorded and the ring reads at once. The collet still reads as a turned washer over a bare cone, about 2 mm proud of each side face, with no seam bead. |

Rounds used: the block-out (one read test) plus all three review rounds. The files are `read-test-1.json`
and `review-round1.json` to `review-round3.json`. Every gate passed in every round. The ring was cut on
craft: the collet never grew out of the band, and no join carries a seam bead (see "What I could not do").

## Measured first: the twist and its csg join along the rope (`report.json` → `rope`)

Final rope: 67.1 mm closed loop, 8 turns, three strands 1.6 mm across, 247,680 triangles. Times at the
export build (1536 × 448):

| Build through | Total | Step |
|---|---|---|
| Band alone (features 1–5) | 0.51 s | |
| The twisted sweep and its csg join (feature 6) | 2.93 s | +2.42 s |
| The mirrored rope's join (feature 7) | 4.79 s | +1.86 s |
| The collet and the stone (features 8–9) | 8.48 s | +3.69 s |

At draft (768 × 320) the rope adds about 1.4 s and the mirror about 1.0 s. The open 63–65 mm ropes of rounds 1
and 2 added 2.5–2.9 s and 1.3–2.0 s at export. A rope costs a few seconds, so the doc's "risk: medium" was
too cautious.

**The section's 7200° does not build.** `Operation::Twist` refuses anything over ten turns (`cad.rs`, "Twist
exceeds ten turns"). The block-out built at the documented fallback of 3600°. From round 1 on I used 2880°
(lay angle about 31°), because 3600° read as knurled wire at 300 px.

## Gates (final outputs, round 3)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Triangles, build time | 736,006, 4.3 s | 1,390,544, 8.3 s (within 2 M) |
| Watertight; boundary, non-manifold, degenerate | yes; 0, 0, 0 | yes; 0, 0, 0 |
| `csg::self_crossings`: ring, Rope, Mirror, Collet, stone | all 0 | all 0 |
| `built.solids.notes`, `built.parts.notes` | both empty | both empty |
| CAD features `Ok` | 9 of 9 | 9 of 9 |
| Finger hole: nearest vertex to the axis (bore r 9.100), vertices inside | 9.100 mm, 0 | 9.100 mm, 0 |
| Field verdict (lost wax) | Castable | Castable |
| `cad::measure::thickness(.., 0.8)` | min 0.980 mm; 0 below, 0 unresolved, 384 rays | same screen (see note) |
| `dfm::cut_lands` at 0.8 | clean | clean |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed; crowding; warnings | 1 / 1; 0 tight; none | 1 / 1; 0 tight; none |
| Stone clear of its seat | 0.020 mm | 0.020 mm |
| `--verify` cold reload with an empty library | n/a | identical vertices, faces and normals |
| Casting pattern (`mesh::try_build_pattern`) | watertight, 0 degenerate, 0 crossings | same |

**Thickness note.** `measure::thickness` refuses meshes over 250k triangles, and one rope is 248k at draft.
The screen therefore reads the same design built at 128 × 48 at the preview chord (241k triangles). If that
build also went over the limit, the example would screen the band and each part on their own.

**Field-note quirk (platform text, not mine).** The lost-wax field report's first note says "no undercut
anywhere", while the part notes that follow list undercuts. Two reviewers flagged it.

## Template gate (after the last round)

Command: `collection_templates officina target/tpl-src --output-dir target/tpl --only torsade --verify-export`,
class `procedural`. Results:

- `design.set` patches: **1** (`/manufacturing`), within the limit of 4.
- Graph: **27,390 bytes** against the 300 KB procedural budget; 16 nodes, 0 exposed controls.
- Cold design reload, cold graph reload and export geometry: **all pass** (1,334,228 identical triangles at the gate's own check).
- The 9 features appear as `cad.feature` nodes in timeline order.
- Recorded in `report.json` → `template_gate`.

## The feature tree, one sentence a feature

1. **Band** is a Flat 4.4 × 1.8 band with flattened sides, a 0.35 mm barrel crown, 0.22 mm arrises and a 0.3 mm comfort bore. The reader learns that the stock is the ground everything else stands on.
2. **Plane over the arris** is the parting plane lifted 2.1 mm to the band's upper arris. The reader learns that a path needs a plane to be drawn on.
3. **Path** is a closed loop of r 10.68 mm, drawn as two half-arcs so that it starts at the top, under the collet. The reader learns where the rope runs, and that a path's start matters.
4. **Plane across the path** is a section plane at θ 90, square to the path where it starts. The reader learns that a sweep's section must stand square to its path.
5. **Section** is three round strands drawn as one trefoil 1.6 mm across, each groove rounded by a 0.1 mm fillet. The reader learns that the rope's whole look lives in this small drawing.
6. **Rope** sweeps the section round the path as a closed twisted sweep of 2880° (eight whole turns), joined to the band. The reader learns the twist parameters, and that a closed twist needs whole turns and has no ends.
7. **Mirror across the band** reflects the rope across the band's mid-plane, which gives the opposite lay on the other arris. The reader learns that a mirror halves the work and reverses the lay.
8. **Collet** is a revolved half-section drawn in the stone's own frame and seated where the stone is placed: a cone flaring up from inside the band, an upright wall, a lip of two 0.4 mm rounds and a solid seat. Because it stands where the stone stands, the stone record counts it as the stone's hand-made head. The reader learns to turn a setting, and what makes the platform count it as one.
9. **Garnet cabochon 7.0** goes last, its girdle 0.65 mm over the crest, on the seat made for it. The reader learns that the stone comes last and sits in its own setting.

## Parts and stone

- **Rope and its mirror** are the subject. They are two continuous three-strand ropes with opposite lays, one on each arris. Each stands 0.6 mm proud of the crest and about 0.7 mm past the side face. They pass through the collet's solid foot, so no rope end shows.
- **Collet** is a hand-made head (role Head, Join, at the stone's placement), turned in one piece with a closed seat. I replaced the `head.bezel` builder, used in the block-out and round 1, because its wall stops at the crest. Over a 4.4 mm band that leaves the setting open underneath, so the stone's back showed from below; its flat lip also cannot be rounded.
- **Stone** is a 7.0 mm round garnet cabochon (`Gem::cabochon` with a preview tint): one focal point, in a colour that sits well against yellow gold. The pictures draw it subdivided and laid on its dome. The reference STL is the stone as built.

## Enablers used

- **C-V3** (twisted sweeps round closed loops): the rope uses `closed: true`. That removes both end caps, so they no longer have to be buried in the collet. A closed twist writes the design at **format 6** (`report.json` → `design_format` is 6).
- **PR #248 crisp renders:** `stones.png` is drawn with `render::write_png_framed` and `render::yaw_facing`, never from a cropped mesh. The ring has no height-field relief, so `crisp_relief` stays off.
- **Not used:** C-B2, C-V1, C-V2, C-V4, C-V5, C-T5–C-T7 and C-B1. Torsade has a round stone and is not built from parts alone; C-V4's beads apply only to rings built from parts alone.

## What I could not do

1. **Seam beads on the rope.** Every blend from 0.04 to 0.3 mm failed. The bead folds where the strand grooves cross the band's arris. On the open ropes it also pinched at the end caps, a three-edge corner that `blend.rs` does not blend. Rounding the grooves, closing the loop and raising the band's edge round to 0.5 mm moved the folds but never removed them. Each failure is a `parts.notes` entry, which fails gate 1. So the ropes join crisp, like soldered twisted wire, and all three reviewers marked it down.
2. **A bead or cove where the collet meets the band.** The collet is wider than the band, so its seam crosses both arrises, and the bead pinches to 0.02 mm, which is again a note. A cove drawn into the revolve's profile would put a torus next to the cone, which item 3 rules out.
3. **Rounds next to a cone in a revolve.** A torus round next to a cone face tessellates with 16 to 64 non-manifold edges ("Solid tessellation has 0 open and N nonmanifold edges"). This happened at the cone's top, and at its base once the cone was steeper. So the collet's cone meets its upright wall at a sharp, obtuse crease, and its base edge stays sharp, hidden inside the band. That is why the collet kept reading as a turned washer over a cone, and it is what cut the ring.
4. **A collet within 1 mm of the side faces.** A 7 mm stone in a 0.95 mm wall makes the collet 9 mm across, on a 4.4 mm band. The wall cannot go much under 0.9 mm without failing the 0.8 mm section screen (a 0.6 mm wall measured 0.68).
5. **Docs and packaging.** In `docs/collections/starters-and-officina.md` I rewrote only Torsade's own section, to match the built tree, and its row in the Officina table. The shared `examples/officina/main.rs` and the `cad::examples::NAMES` registration are left for packaging, as the brief says.

## Core changes wanted

**1. Let a seam bead give way where it folds, instead of failing the whole seam.** In
`crates/ringdesign-core/src/blend.rs`, in the bead's fold loop, replace the branch that returns
`Err("the bead folds at ...")` with this, so a twisted rope keeps its bead everywhere else:

```rust
if round == FOLD_ROUNDS || crossing.iter().all(|&i| radii[i] <= RADIUS_MIN_MM + 1e-12) {
    // Stations still folding at the floor radius are left bare: the bead runs out into the seam there.
    for &i in &crossing {
        radii[i] = 0.0;
        folded[i] = true;
    }
    solid = sweep_runs(&st, m_arc, &radii); // new: splits the loop at zero-radius stations and caps each run
    break;
}
```

`parts.rs` would then report the folded stations as a count in the build report rather than as a
`parts.notes` entry, for instance while they stay under 5% of the stations.

**2. Bound a twist by its stations, not by ten turns.** In `crates/ringdesign-core/src/cad.rs`, in the
evaluator's `Operation::Twist` arm, replace the ten-turn check with:

```rust
ensure!(
    degrees.is_finite() && (degrees.abs() / twist::TWIST_STEP_DEG).ceil() as usize <= twist::MAX_STATIONS,
    "Twist exceeds {:.0} turns at {} degrees a station",
    twist::MAX_STATIONS as f64 * twist::TWIST_STEP_DEG / 360.0,
    twist::TWIST_STEP_DEG
);
```

`twist::sweep` already caps its triangles at `MAX_TRIANGLES`. With this change, the section's 7200° builds.

**3. A thickness screen past 250k triangles.** In `crates/ringdesign-core/src/cad/measure.rs`, in
`thickness`, drop the `mesh.faces.len() > 250_000` refusal and replace the brute-force inner loop with the
BVH that `dfm` already uses:

```rust
let bvh = crate::interaction::bvh::Bvh::build(mesh);
// For each sampled face, a ray started just inside the surface:
let start = std::array::from_fn(|i| center[i] + inward[i] * 1e-5);
let nearest = bvh.ray(mesh, start, inward).map(|(_, t)| t + 1e-5);
```

**4. A torus beside a cone in `Revolve`.** This is a tessellation bug in the kernel and belongs upstream.
Repro: revolve `[(9.95, 0) → (9.95, 3.55) → (11.7, 4.62) → (11.7, 0)]` about its x axis with a 0.35 mm round
at `(11.7, 4.62)`. The result has 64 non-manifold edges; without the round it is clean. Until it is fixed,
a closed-back option on the bezel builder would give lessons like this one a made collet that does not show
its underside:

```rust
// builders::defaults(BEZEL, ..): a closed back for a stone wider than the band under it.
Param { key: "closed", label: "Closed back", unit: "", min: 0.0, max: 1.0, kind: Kind::Flag, default: json!(false) },
```

When `closed` is set, `setting::collet_named` would run the bearing ledge to the axis at `-depth` instead of
leaving the section open, and round the lip's top corners by `0.4 · wall`.

## Reproduce

```sh
CARGO_INCREMENTAL=0 cargo build --release -p ringdesign-core --example officina_torsade
target/release/examples/officina_torsade --draft      # draft gates and renders
target/release/examples/officina_torsade --verify     # draft and export gates, cold reload, STLs, renders
```
