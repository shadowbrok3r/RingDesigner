# Cataphracta — Chelonia, the carapace seal: cloud report

Branch `claude/cataphracta-chelonia`, from `master` at `8e5a59a`. Master was merged twice on the way: at round 2 (`6f7bce0`, PR #248 crisp edges) and at round 3 (`779a3d6`, C-B1, C-V1..C-V4, C-T2). One example file: `crates/ringdesign-core/examples/cataphracta_chelonia.rs`. Outputs: `showcase/cataphracta/chelonia/`.

## Outcome

**Cut at 6.6 after three full rounds** (round 1: 6.4 revise; round 2: 6.6 revise; round 3: 6.6 cut). The ship bar is 7.5 and Caiman scores 7.

| Stage | Verdict | Score | Review file |
|---|---|---|---|
| Block-out, read test 1 | **reads: true** | — | `read-test-1.json` |
| Round 1 | revise | 6.4 | `review-round1.json` |
| Round 2 | revise | 6.6 | `review-round2.json` |
| Round 3 | **cut** | 6.6 | `review-round3.json` |

Rounds used: one block-out attempt plus 3 review rounds, the cap.

Every reviewer named it a turtle at 300 px without a caption. It was cut on four things:
- **The read.** It is a turtle charm laid on a quatrefoil, not the quatrefoil made into the turtle. The polished 007 lobe bulbs and the bare lobe walls stay visible round scaled flippers ("pine cones on pillows").
- **The scutes.** They are rounded rectangles, and the reviewer read their annuli as letters.
- **Outlines at close range.** The close-ups show sawtooth along the seams.
- **Gates.** It failed two gates the rubric applies to every ring, on its own terms.

### Gates review.md failed, and why

- **Ray release.** The rubric's gate is 0 obstructions. This ring is lost wax: Logan's decision, in the doc header (2026-09-24) and in TASK.md. A raised turtle with a shell bridging the cusps cannot part on a plane: 86 obstructions at 0.100 mm and 121 at 0.075 mm, 0 unresolved at both.
  - `report.json` cites the doctrine verbatim (CLAUDE.md: under lost wax the pull statistics are "measured and reported … but never gate").
  - The reviewers held the rubric's literal gate.
  - **Recommendation:** review.md should judge the release gate by the ring's own process, as it already says it does for the field verdict.
- **Land widths on made parts ≥ 0.40 mm.** The one-ray census reads every relief flank's top corner as a near-zero section.
  - I split the census:
    - A sub-floor read is *supported* when the ray re-enters metal, or finds metal straight beneath, within 0.3 mm. That is a flank on the metal it fills from.
    - Otherwise it is a fin or lip.
  - Unsupported area fell from 8.0 mm² to **0.38 mm²**, and every read is named with a bench treatment.
  - The per-feature `thinnest_mm` still records the flank reads (0.0001–0.018 mm), and the reviewer gated on those.

## Gates (draft 768 × 320 and export 1536 × 448)

The stock is an imported mesh, so both builds give the same mesh: 206,640 triangles.

| Gate | Draft | Export |
|---|---|---|
| Watertight, 0 degenerate | pass | pass |
| Self-crossings: ring / 8 made parts | 0 / 0 | 0 / 0 |
| Solids and parts notes | empty, 8 of 8 parts joined | same |
| Inside the finger hole | 0 vertices (nearest −7.6e-8 mm) | same |
| Field verdict (LostWax, 0.8 mm fill) | **Castable**, thinnest wall 0.997 mm | same |
| Ray release 0.100 mm | 86 obstructions, 0 unresolved (NotApplicable) | same |
| Ray release 0.075 mm | 121 obstructions, 0 unresolved (NotApplicable) | same |
| Draft-clamp bite | 0 (lost wax, no painted layer) | same |
| DFM findings | 0 | 0 |
| Stones report = preview | 0 = 0 | same |
| Land widths named | all named; 34.9 mm² flank reads, 0.38 mm² unsupported | same |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |
| Within 2 M triangles | 206,640 | 206,640 |
| `--verify` cold reload | — | identical vertices, faces and normals (103,320 / 206,640) |

There are no stamps, so the 384 × 192 stamp re-run does not apply. The two-part numbers are reported, not gated: 243 mm² undercut, 15.83% at 61.4°.

## Template gate

`collection_templates … --only chelonia --verify-export` passed. Output: `template-verification.json`.
- Class `painted`, per brief.md: "stock for bare factory stock, painted otherwise". This is stock with eight made parts.
- **1 `design.set` patch** (`/manufacturing`).
- **Graph 2,990,532 bytes**, against a 3,000,000 budget. The editable design is 3,006,511 bytes.
- Source identical; vertices, faces and normals identical; cold design and graph reloads true; 19 nodes; detail findings 0.
- Open: evaluate 78 ms, first build about 11 s.
- Size history: 12.5 MB (round 1, failed the budget), then 2.41 MB (round 2), then 2.99 MB (round 3, at 2–3× the raw density).

## What the ring is

Base: the real factory **007 Quatrefoil**, unmirrored, face 16 × 17 mm, bore 18.6 mm. Lost wax at a 0.8 mm fill, Silver 925. No stone: the carapace is the jewel.

The four lobes sit on the diagonals. The along-ring cusps take the head (toward +X, the hero camera) and the tail.

Every turtle form is a **made part**: a closed `Operation::Stored` mesh, joined. It is sculpted in plan millimetres, fitted to the stock's own surface by rays toward the finger axis, then decimated by quadric error with a crossing back-off. Every outline is true geometry; nothing is painted.

| Part | Construction | Why |
|---|---|---|
| Carapace | A lens free of the stock: absolute crown, floor sweeping into the metal, bridging both across-ring cusps. About 13.6 × 11 mm. Scutes are a power diagram (5 vertebrals, 4 costals a side) with soft-min rounded corners; an odd ring of marginals puts the nuchal on the midline and the supracaudal seam at the rear; 3 annuli per scute as shares of its own inradius; shingled tilt (hawksbill imbricate). | The shell is the signature, so it carries the read. The lens lets it overhang the cusps as a real shell overhangs its bridge. |
| Head and neck | A star-shaped distance-field blob (cranium, snout, jaw, neck capsule, skin collar) under three soft head scales, crown held at the marginal tier, read along rays from its centre. Left undecimated. | An undercut head on a neck is legal in wax. Round 1's eyes read as a snake, so they were removed. |
| Fore and hind flippers | Constant-curvature arc spines, which cannot fold below the bend radius. Each side's width is read off the lobe's own rim and capped. Plates in staggered courses with a larger leading-edge row and a claw. The edge is footed (S-profile) instead of a kernel fillet. | One lobe, one flipper. A kernel fillet runs round a joined cluster's whole seam at its largest radius, which folded at the tail and head. |
| Tail | A short strip whose root stands up into the shell's rear rim. | No crevice under the supracaudals. |
| Hawksbill plates and plastron | One strip from behind the head round the palm to behind the tail. Shoulders: overlapping keeled plates, graded 2.1 → 1.3 mm, staggered, free edges palmward. Palm: three broad plastron pairs, straight transverse seams, a shallow midline seam for the bench. | One theme, face to palm. |

## Enablers used

- PR #248 (crisp edges): `render::write_png_framed` / `yaw_facing` / `Framing` for every close-up.
- `d.crisp_relief` was not set: there is no height-field relief.
- The design is written at format 6 because it carries stored meshes.
- C-R1, C-R4, C-R7 and C-R8 were on master but are not used: the turtle is made parts, not hide-space tilings.
- C-B1, C-V1..C-V4 and C-T2 landed at round 3. They are not used: no curve layers, seat runs or tracery.
- `sculpt::decimate` (core) is used for the parts.
- C-B2 (true stone plans) and C-T5..C-T7 had not reached master.

## What I could not do

- **The lobes as flippers.** The 007 lobes are fat round bulbs. A flipper that fills a lobe is a paddle (the "pine cone" read), and a flipper-shaped blade leaves polished bulb showing. A real fix needs the lobe crowns cut or re-sculpted, which changes the factory stock that Logan wants kept.
- **"Fore 25% longer and swept back 30°"** conflicts with "fill each lobe to within 0.5 mm": the lobes are mirror images.
- **Seam crispness at close-up.** A height-field strip on a regular grid aliases any narrow groove. C¹ profiles, shorter-diagonal quads and 2–3× density shrank the saw, but did not remove it. A true fix is grooves as swept parts, or an adaptive mesher.
- **The lobe walls and inter-lobe valleys stay bare.** Radial rays cannot reach a side wall, so the cheek texture needs a second projection.
- **The release gate at 0 obstructions.** It cannot be met under the process Logan chose, as recorded above.

## Core changes wanted

1. **The land census should know a flank from a fin** (`dfm.rs`). Today `part_sections` reads every relief flank's corner as a near-zero section:

```rust
/// [`part_sections`], counting only reads with air behind them: a ray that, once out of the metal, re-enters it or
/// finds it straight beneath (toward the finger axis) within `gap_mm` is a relief flank that fills from that metal.
pub fn part_fins(solid: &crate::csg::Solid, floor_mm: f64, gap_mm: f64) -> (f64, f64) {
    let mesh = crate::mesh::Mesh::from_solid(solid);
    let bvh = crate::interaction::bvh::Bvh::build(&mesh);
    let (mut thinnest, mut area) = (f64::MAX, 0.0);
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let n = crate::csg::cross(crate::csg::sub(b, a), crate::csg::sub(c, a));
        let twice = crate::csg::len(n);
        if !(twice > 1e-14) { continue; }
        let inward = n.map(|x| -x / twice);
        let cen: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0);
        let o: [f64; 3] = std::array::from_fn(|k| cen[k] + 1e-4 * inward[k]);
        let Some((_, t)) = bvh.ray(&mesh, o, inward) else { continue };
        if t + 1e-4 >= floor_mm { continue; }
        let exit: [f64; 3] = std::array::from_fn(|k| o[k] + (t + 1e-4) * inward[k]);
        let r = exit[0].hypot(exit[1]).max(1e-9);
        let down = [-exit[0] / r, -exit[1] / r, 0.0];
        let flank = bvh.ray(&mesh, exit, inward).is_some_and(|(_, g)| g < gap_mm) || bvh.ray(&mesh, exit, down).is_some_and(|(_, g)| g < gap_mm);
        if !flank {
            thinnest = thinnest.min(t + 1e-4);
            area += 0.5 * twice;
        }
    }
    (thinnest, area)
}
```

   `Mesh::from_solid` and the `csg` vector helpers are the obvious ones if not already public. My example carries the same logic inline as `land_widths`.

2. **A stored part that keeps its recipe, not only its mesh.** Chelonia's 2.99 MB is all packed meshes. It wants an `Operation::Strip { spine, widths, height_script, sink_mm }` evaluated against the band as built: the example's `Strip` with the height as a `script` expression. Templates would then carry kilobytes, and the parts would follow a resize. The example's `Strip::build` and `Blob::build` are the reference implementations.

3. **The reviewer rubric** (review.md, not code): gate the ray release by the ring's own process, as the doctrine says, or record Logan's waiver for the lost-wax rings once in the collection file.

## Notes

- TASK.md named `claude/cataphracta-chelonia` as the results branch, and every push went there; nothing went to master. The session's harness had also named `claude/task-md-wv4rpg`, which was not pushed.
- The checkout lives at `/home/user/RingDesigner`; TASK.md's `/home/user/repo` did not exist.
