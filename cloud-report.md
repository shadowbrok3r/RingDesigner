# Vepres — Prunus (`prunus`), rethought for lost wax: cloud report

- **Branch:** `claude/vepres-prunus-rethink`, from `master` at `2e11632`. Master did not move during the session; every round's `git fetch origin master` and merge was a no-op.
- **Example:** `crates/ringdesign-core/examples/vepres_prunus.rs`.
- **Outputs:** `showcase/vepres/prunus/`, with the block-outs in `showcase/vepres/prunus/<option>/`.
- **No `src/` edits.**

## Verdict: **cut at 6.7** after 3 full-review rounds (6.0 → 6.6 → 6.7)

Every gate is green and the template gate passes.

**Process: lost wax.** Logan said "if we can still cast via lost wax, then just make it a lost-wax ring", and on 2026-10-03 chose "lost wax on a rounder head".
- Settings: `CastProcess::LostWax.apply`, then `min_section_mm` 0.8 and `min_draft_deg` 0.
- The Prunus section of `docs/collections/vepres.md` records this, the rethink, and the outcome. No extra rounds were granted, and the doc says so.
- **Sand bonus:** none. The field notes say the pattern "cannot move to sand as-is": the twig, spurs, sepals and blossoms all undercut a two-part pull.

## Step 0: block-outs and read tests

The three options in TASK.md are built in one example and chosen with `--option`.
- Option 1 (`wax-twig`) uses the native 012, as TASK.md specifies.
- Options 2 and 3 use the native round 013, following Logan's "rounder head" answer.

Each read test went to a fresh reviewer, given only `review.md`, the ring's name and slug, "read-test mode", and that option's two 300 px renders.

| # | Option | Reads | What the eye saw (abridged) |
|---|---|---|---|
| 1 | wax-twig | **no** | A signet with a cabochon in a bezel, and a twig of "barbed wire or nails" across it; the blossoms tiny |
| 2 | wax-calyx | **no** | A black cabochon in a "gear / crown bezel"; stubby pins; specks for flowers |
| 3 | wax-twig-calyx | **no** | A "flowering branch with a black pearl" in claws; the rods read as pins, not thorns |
| 4 | wax-twig, revised | **no** | A "solitaire signet with a blossom garland"; the stone reads as a set gem |
| 5 | wax-calyx, revised | **no** | A "gothic black-pearl solitaire" in sawtooth claws; one lone blossom |
| 6 | wax-twig-calyx, revised | **yes** | "Bare thorny branch with five-petal flowers and a dusky dark fruit says blackthorn or sloe … within a second or two" |

The one revision each option was allowed changed three things:
- the spurs became sharp, graded and acutely leaning;
- the blossoms went on the wood in pairs;
- the collet was replaced by a calyx and stalk, or by claws on 012.

**Taken:** `wax-twig-calyx` on 013, the only option that read. Options 1 and 2 are recorded in their folders and in `read-test-{1,2,4,5}.json`.

## Full reviews

| Round | Verdict | Score | Main asks |
|---|---|---|---|
| 1 | revise | 6.0 | The calyx read as a mechanical crown; flat collars at the roots; ornament on about 25% of the ring; template gate not recorded |
| 2 | revise | 6.6 | The calyx still read as claws; stepped roots and a ragged twig seam; density; the blossoms' "opposite-wall" section readings |
| 3 | **cut** | **6.7** | The calyx regressed (sepals detached from the stone); stepped roots and seam remain; cheeks and table bare |

Every review passed the plant read and the silhouette. Workmanship failed every round, in three places:
- the calyx;
- the fairing where roots meet the twig;
- the blossom hearts.

Density stayed below Caiman's. The reviews are in `showcase/vepres/prunus/review-round{1,2,3}.json`.

## Gates (final design; each build has its own block in `report.json`)

| Gate | Draft 768×320 | Export 1536×448 |
|---|---|---|
| Triangles (≤ 2 M) | 346,600 | 1,061,614 |
| Watertight; degenerate faces | yes; 0 | yes; 0 |
| `self_crossings`: the ring / the worst of 50 made parts | 0 / 0 | 0 / 0 |
| Solids notes; parts notes; every feature `Ok` | none; none; yes | none; none; yes |
| Stamps resolved (Straif) | 1/1 | 1/1 |
| Closest vertex to the axis vs the bore radius | 8.568 / 8.578 (within 0.01) | same |
| Field verdict (lost wax) | **Castable** | **Castable** |
| The band's thinnest wall (field) | 1.40 mm | 1.40 mm |
| Sections by construction all ≥ 0.8 | yes | yes |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed; metal in the stone | 1 / 1; 0 | 1 / 1; 0 |
| Crowding census | clean | clean |

**Cold reload and casting pattern**
- Cold reload (`--verify`, export, empty library): identical vertices, faces and normals.
- Casting pattern: watertight, 0 degenerate faces, 0 crossings, 1,061,614 triangles.

**Sections by construction**

| Part | Least section, mm |
|---|---|
| Calyx cup | 2.52 |
| Sepals | 0.86 |
| Stalk | 0.96 |
| Main twig, net of fissures | 0.94 |
| Shoots, net of fissures | 0.90–0.94 |
| Spur roots | 1.0–1.2 |
| Blossom rims (rolled) | 0.84 |

The spur points and the sepals' rounded tips run under 0.8 mm across. They end at a 0.1 mm point radius, after Manticora's 0.2 mm sting tip. Each spur's point run is listed in `authored.spurs`.

**Section diagnostic**
- **Method:** inward rays from the made parts' vertices, skipping vertices on a crease and counting only rays that exit the metal.
- **Result:** 697 of 45,291 samples under 0.8 mm, the least 0.12 mm. `section_sampled_diagnostic.breakdown` sorts them by part and by kind.
- **Cause:** rays that leave a blossom's rolled rim sideways and cross the V where two petals meet, and rays that cross bark fissures or spur roots at a slant.
- **Evidence:** a lone blossom with no band gives the same readings. The round 3 reviewer accepted this as a stated cause and noted that no cross-section confirms it.

**`cad::measure::thickness`**
- In round 1, under its 250k-face limit, it read 56 of 384 samples under 0.8 mm. They were all at the factory stock's rounded bore and side-face edges, which the tool also reads as thin on the bare 013.
- In round 3 both builds are over the face limit, and the report says so.

**The factory 013 mesh folds over itself** by about 15 µm at θ ≈ −5°, z ≈ 1.97.
- On the bare stock this gives 8 self-crossings at every resolution.
- The example finds the crossings on the bare build and unites a 0.1 mm lens over the fold ("Seam pad 1"), which clears them.

**Twig seam.** A 0.3 mm `blend_mm` bead on the twig and shoots "folds" where they cross, so both are joined with no bead. The ragged seam the reviewers saw remains.

## Template gate

`collection_templates vepres … --only prunus --verify-export`, class `stock`.

| Check | Result |
|---|---|
| `design.set` patches | **1** of 4 (`/manufacturing`) |
| Graph | 70 nodes, **997,600 bytes**, within the 1 MB stock budget with 2.4 KB of headroom; no size review |
| Cold source | identical (lift) |
| Cold design and graph reloads | pass |
| Mesh parity at 1536 × 448 | 1,061,614 triangles; vertices, faces and normals identical |
| First build | 1.43 s |
| `template_gate_passed` | **true** |

- The numbers are in `showcase/vepres/prunus/template-verification.json` and in `report.json → template_gate`.
- `crisp_relief` is left off: the ring has no steep height-field relief.

## Enablers from master

None were used. C-B2, C-V1 to C-V5 and C-T5 to C-T7 were already on master at `2e11632` when the session resumed. Every form here is a sculpted stored mesh made in the example, so no path pattern, sweep scale law or seam bead was needed. The ring placement's new `level` field is set to `false`.

## The CAD feature tree, as sentences

1. **Band** is the native factory 013 Round, unmodified.
2. **Seam pad 1** is a 2.3 × 0.26 × 0.1 mm lens over the stock's own fold on the shank's side face. It is united with the band so the fold's crossing faces fall inside it.
3. **Round cabochon 5.6 mm** is the onyx sloe, a `stone` builder.
   - **Placement:** on the table at (x −0.3, z −2.2), its girdle 0.5 mm over the table.
   - **Render:** a blue-black bloom, matte (roughness 1.0, not rendered as a gem).
4. **Calyx cup** is a turned collar under the girdle that carries the stone.
5. **Calyx sepals 1–5** are elliptical sections swept up the stone's meridian, 0.03 mm off its dome.
   - Each is 0.86 mm thick and 2.5 mm wide at the girdle.
   - Each narrows to a rounded, slightly reflexed point at about 0.45 of the crown.
6. **Sloe stalk** is a cubic arch, 1.2 to 0.96 mm across and 3.4 mm long. It leaves the top of the twig and runs under the cup, so the sloe hangs from the branch.
7. **Twig over the head** is a 64.7 mm tube swept over the ground's normals.
   - **Path:** it starts 18° short of the palm (270°) on the left, climbs the left shank, and crosses the table diagonally (slope 0.55), bowing away round the fruit. It runs down the right shank and stops 18° short of the palm on the right, diving into the band at both ends.
   - **Radius:** 1.15 mm at the head, tapering to 0.6 mm at the palm, swelling 10% at 26 irregular nodes.
   - **Bark:**
     - five long fissures, 0.16 mm deep, wavering, with one run in five broken;
     - low plates between them;
     - lenticel dashes.
8. **Shoot, right wall** and **Shoot, left wall** are thinner shoots, 22 mm long, radius 0.74 tapering to 0.6 mm.
   - Each branches from the main twig at the shoulder and crosses to the other edge of its wall.
   - Each dives into the band 19° from the palm's centre, so the two frame Straif.
9. **Spurs (28)** are cones grown from inside the wood at a node.
   - **Lean:** an acute 48–60°, toward the nearer twig tip, turned out to each side in turn.
   - **Root:** flared, never wider than the wood it grows from, with five faint ridges.
   - **Body:** slightly bent, ending in a 0.1 mm point.
   - **Lengths, long and short in turn:** 3.9–4.7 mm and 2.0–2.5 mm over the table; 1.0–1.6 mm on the shoots.
10. **Blossoms (10)** are five-petal flowers 2.4–3.9 mm across, in pairs over the table and single down the shank.
    - Shallow-cupped petals parted by recessed notches.
    - A domed heart ringed by ten stamen beads.
    - A 0.84 mm rolled rim on every petal, over a narrow foot seated in the wood.

**Stamp: Straif**, the Ogham letter for blackthorn, is cut 0.3 mm deep in the palm crest. It is one comb outline: a 6 mm stem with five 1.6 × 0.35 mm strokes slanting through it at 45°. Six overlapping cuts crossed at export, so the comb replaced them.

**Layer: "Fine sampling (0 mm)"** is a zero-height border round the whole band. Its only job is to make the build sample the factory surface on the build's own grid. Part seams then fall on fine facets; on the stock's coarse mesh the joins showed stair-steps.

**Renders**
- Studio gold with the stone set, recesses darkened.
- The wood (twig, shoots, stalk, spurs) is oxidised dark, with the fissures darker still.
- The blossoms, sepals and bore stay polished.
- Close-ups use `render::write_png_framed`.

## What I could not do

- **Ship.** The final review found these faults:
  - The calyx regressed: the swept sepals read as detached grubs, with table showing between them and the onyx.
  - There are stepped facets at the blossom and spur roots on the twig over the head.
  - The seam where the twig meets the band is ragged.
  - The blossom hearts are still creased.
  - The cheeks, the table outside the stone and the shank's outer crest are bare polish.
- **A seam bead or fillet on the twig.** A 0.3 mm `blend_mm` folds where the main twig and the shoots meet the band close together.
- **Wall bark between the shoots.** A painted hide would break the template budget, which the graph already fills to 997.6 KB of 1 MB.
- **Crisp Straif edges.** The reviewer still sees doubled edges on the cut.
- **A cross-section proof for the blossom section readings.** There is only the lone-blossom reproduction and the construction.

## Core changes wanted, as exact code

**1. `cad::measure::thickness` should skip creases and count only exits.** As written, it reads a factory band's rounded edges as walls under 0.1 mm, on the bare stock and on the finished ring alike. In `src/cad/measure.rs`, inside the per-sample loop, before casting:

```rust
// A sample on a crease has no single inward direction; skip it.
let incident_ok = mesh.faces.iter().filter(|f| f.contains(&tri_index_vertex)).all(|f| {
    mesh.triangle(f).is_some_and(|(a, b, c)| dot(unit(cross(sub(b, a), sub(c, a))), normal) > 0.82)
});
if !incident_ok { continue; }
```

Then, where a hit is accepted:

```rust
// Only an exit counts: the far wall faces along the ray.
if t > 1e-4 && t < best && dot(face_normal(hit_face), inward) > 0.0 { best = t; }
```

**2. A crossing report in `csg` that says where.** With it, a ring's or a stock's crossings can be found and patched. This is the example's `crossing_sites`, moved to core:

```rust
/// The face pairs that cross (sharing no vertex), the first `limit` of them.
pub fn self_crossing_pairs(s: &Solid, limit: usize) -> Vec<(usize, usize)> {
    let all: Vec<u32> = (0..s.f.len() as u32).collect();
    let mut grid = Grid::over(s, &all);
    let boxes: Vec<(P3, P3)> = s.f.iter().map(|f| tri_bounds(f.map(|i| s.v[i as usize]))).collect();
    let mut stamp = vec![u32::MAX; s.f.len()];
    let (mut near, mut out) = (Vec::new(), Vec::new());
    for i in 0..s.f.len() {
        near.clear();
        grid.each_cell(boxes[i].0, boxes[i].1, |cells, c| for &j in &cells[c] {
            if j as usize > i && stamp[j as usize] != i as u32 { stamp[j as usize] = i as u32; near.push(j as usize); }
        });
        let a = s.f[i].map(|k| s.v[k as usize]);
        for &j in &near {
            if !overlap(&boxes[i], &boxes[j]) || s.f[i].iter().any(|v| s.f[j].contains(v)) { continue; }
            let b = s.f[j].map(|k| s.v[k as usize]);
            let hit = (0..3).any(|k| matches!(pierce(a[k], a[(k + 1) % 3], b[0], b[1], b[2]), Ok(Some(_))))
                || (0..3).any(|k| matches!(pierce(b[k], b[(k + 1) % 3], a[0], a[1], a[2]), Ok(Some(_))));
            if hit { out.push((i, j)); if out.len() >= limit { return out; } }
        }
    }
    out
}
```

**3. Fix the factory 013 fold in the master.**
- **Where:** the stock folds about 15 µm at θ ≈ −5°, z ≈ 1.97, on the bore/side-face edge round at the seam.
- **Effect:** 8 crossings on the bare stock.
- **Fix:** re-export the 013 master, or weld that seam.
- **Until then:** rings on the native 013 need the example's seam pad.

## Files

- `crates/ringdesign-core/examples/vepres_prunus.rs` replaces the earlier sand example.
- `showcase/vepres/prunus/` holds:
  - `design.ring.json`, `report.json`, `stones.json` and `template-verification.json`;
  - `read-test-{1..6}.json` and `review-round{1,2,3}.json`;
  - the renders: `hero face palm side shoulder reverse stones bare-vs-finished hero-300 face-300 contact-300` (`.png`).
- `showcase/vepres/prunus/{wax-twig,wax-calyx}/` hold those options' revised block-outs, the renders read tests 4 and 5 judged.
- `showcase/vepres/prunus/wax-twig-calyx/` was re-rendered while round 1's gates were being fixed. The renders read test 6 judged are in commit `0b2ce4c`.
- `docs/collections/vepres.md`: the Prunus section records the lost-wax decision, the rethink, that no extra rounds were granted, and the outcome.
- STL files are written locally and are git-ignored.
