# Cataphracta — Moloch, the thorn idol: revival report (lost wax)

**Outcome: cut at 7.3 in round 6.**
- The revival granted two more reviewed rounds. Round 5 scored **7.2** and round 6 scored **7.3**, both under the 7.5 ship bar. Under TASK.md a score under 7.5 after round 6 is a cut, and the reviewer's verdict stands.
- Every gate is green at draft (768 × 320), at export (1536 × 448, with `--verify`) and at 384 × 192, in both rounds.
- The template gate passes on the final build: 0 `design.set` patches, 2,362,958 B against the 3 MB painted budget, and cold source and mesh parity identical.

Both reviewers wrote "cut" by the three-round cap in `review.md`, since the repository records no extension. TASK.md grants round 6 whenever round 5 does not ship, so round 6 went ahead. Round 5's 7.2 is a "did not ship" for that purpose.

Branch: `claude/cataphracta-moloch-revival` (from `claude/cataphracta-moloch`, with `origin/master` merged at the start, again mid-round 5 for the crisp-render fix, PR #248, and at the start of round 6). Author file: `crates/ringdesign-core/examples/cataphracta_moloch.rs`. Artwork: `crates/ringdesign-core/examples/cataphracta/art/moloch/sand-ripples.svg`. Outputs: `showcase/cataphracta/moloch/`.

## Verdicts, every round (an independent reviewer each time)

| Step | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|
| Read test 1 | reads: true | — | "A spiky lizard with four legs, a head, a false-head hump and a tail." |
| Round 1 | revise | 6.3 | Even-tube limbs, toes at 0.11 mm, eye-like balls at the nape, a bare body, smeared ripples. |
| Round 2 | revise | 6.7 | Better limbs and a seamless band. The head was a shark snout, the granules read as frog spawn and the ripples as wood grain. |
| Round 3 | cut | 7.0 | The tubercle hide reads as a thorny devil. Ripples ran off the circumference, stair-steps, a polished halo, no five toes. |
| Round 4 (Logan's extra round) | cut | 7.1 | The ripples run round the ring. The feet read as cartoon gloves, with pegs off the cheek, toes at 0.175 mm and wrist knobs. |
| **Round 5 (revival)** | **cut (by the cap; did not ship)** | **7.2** | Slender clawed toes at last, no peg (toes ≥ 0.631 mm), tubercles into the cone roots. Still: the band read as wood grain or water with crinkled palm highlights, outward spikes and a "forked tip" at the rear, a blobby head with a snout cone, an elbow knob. The template-gate file was stale (round 4's). |
| **Round 6 (revival)** | **cut** | **7.3** | The tail is fixed (a beaded taper, no outward rail, no fork), the ripples are long, continuous and clean on the face side, and every gate is green including a fresh template gate. Still: the ripples now read as stylised water waves, too even and symmetric, with pooled highlights on the palm side. The head is a bubbly mass with seamed plates, a polished snout dome and a low forward horn. The upright flank thorns stand past the band edge in reverse view. An elbow knob remains, and the front limb bridges over the band in profile. |

Full verdicts: `read-test-1.json` and `review-round{1..6}.json` in `showcase/cataphracta/moloch/`.

## What the revival changed

### Round 5 (round 4's punch list, the feet first)
1. **Feet (punch 1, the species-defining shape).**
   - Each toe is a new `Ridge` shape, sized against the floor before anything else. It is a crest swept in plan whose section is an uneven capsule: a round top of 0.34 → 0.30 → 0.24 mm radius, with flanks flaring 0.5 mm per mm down into a 0.75 mm keel buried in the band. Every ray leaving a crest or flank face crosses the keel, so the section holds about 2.7× the crest radius. The toes measure 0.653 mm at their thinnest, with 0.027 mm² under the floor.
   - Each toe ends in a separate pointed claw: a 0.42 mm-wide cone reaching about 0.75 mm past the crest's end. Claws are their own land kind, under the floor by design like a thorn point.
   - Five toes per foot, the middle three the longest. Each toe is searched for length and a few degrees of heading so that it stays on the crown with its keel 0.9 mm inside the cheek; stays clear of the hump, head, tail, trunk underside and its own forearm; and keeps its claw 0.45 mm from its siblings'.
   - A small buried sole under each wrist knits the keels to the trunk's sunk underside, so no buried crevice opens.
   - The toe that curled over the edge is gone. Nothing stands past the cheek (−0.26 mm).
2. **Limbs (punch 3).** The hand egg is gone. Tubercles on the limbs are finer and lower (0.45 mm pitch, 0.07 mm), and fade out by the wrist. There are fewer, smaller limb cones, and the elbows are pulled in off the crown edge.
3. **Ripples (punch 2).** The SVG was redrawn. Each crest is one stretch of a single line `s`, so it stays seamless round eight tiles. The profile is built from nested bands of ink (a long windward slope and a short steep lee) instead of stacked strokes, which removes the terracing. Pitch, waver and height vary smoothly with `s`, and some crests sink away and pick up again.
4. **Tail (punch 4).** The thorns lie 22–28° off the tail's own line.
5. **Cones and head (punch 5).** Tubercles fade by height above the skin, so they run up into every cone's fillet and stop on the flank. The head is lower and wedge-shaped, with a flat mosaic of angular plates instead of round tubercles.
6. **Platform (from master).**
   - The close-up uses `render::write_png_framed` on the whole ring, replacing the cropped mesh.
   - **Shading note:** the crease normals that framing keeps drew the sculpt's sub-0.05 mm tetrahedral folds as crumpled foil. The renders therefore shade faces standing more than 0.3 mm proud of the bare crown with the mesh's smooth vertex normals. Every face at or near the band keeps its crease normals. Geometry is untouched (`shading_mesh` in the example).

### Round 6 (round 5's punch list)
1. **Template gate.** Re-run on the exact build (punch 1, now closed).
2. **Ripples.** Long coherent crests at 1.5–1.9 mm, swaying nearly in step (the waver is close to whole cycles per crest, so neighbours do not drift across each other like grain). No micro-ripples, and four breaks.
   - The SVG was cut from 1.0 MB to 279 KB (0.1 mm polyline steps, 14 levels), which brings the graph back under budget.
3. **Tail.** Thorns 0.3–0.55 mm long and 22° off the tail's line, none on the last fifth, and the tail ends in a bare taper 0.9 mm across.
4. **Flank, hip and back thorns.** Splay cut from 0.4–0.7 to 0.1–0.2, the lean back from 38° to 26°, and the rear cones shortened.
5. **Head.** Horns aimed off the ring's radial up (not the head's steep flank), so neither juts sideways. Larger, flatter plates (1.15 mm pitch). A fuller snout fills the hollow behind it.
6. **Limbs.** The forearm arches, with two limb cones.
7. **Claws.** The claw treatment uses lost-wax wording.
8. **`crisp_relief` is off in the final design.** With it on, the template graph fails at node #29 ("the design failed upstream"), so the template gate cannot pass (see core changes). The ripples' lee faces are gentle (0.22 mm over about 0.24 mm), and a 2× zoom of the export palm shows no stepping without it. The design therefore stays at its earlier format, with no format-6 opt-in.

## Gates (final, round 6: the committed design)

| Gate | Draft 768 × 320 | Export 1536 × 448 | 384 × 192 |
|---|---|---|---|
| Watertight, degenerate faces, self-crossings | yes, 0, 0 | yes, 0, 0 | yes, 0, 0 |
| Made part "Thorny devil": open edges, crossings as made / as placed | 0, 0 / 0 | 0, 0 / 0 | 0 |
| Solids and parts notes, part joined | [], 1 | [], 1 | [], 1 |
| Bore: nearest vertex against the 9.300 mm radius, vertices inside | 9.300, 0 | 9.29999952, 0 | — |
| Field (lost wax, `attributed_field_report` 256 × 128 plus `judge_parts`) | Castable, thinnest wall 1.66 mm | Castable, 1.66 mm | — |
| Land widths: every section under 0.8 mm named with its treatment | pass | pass | — |
| DFM findings | 0 | 0 | — |
| Stones reported / previewed | 0 / 0 | 0 / 0 | — |
| Casting pattern (`try_build_pattern`): watertight, degenerate faces, crossings | yes, 0, 0 | yes, 0, 0 | — |
| Triangles (budget 2 M) | 591,120 | 1,402,980 | 274,554 |
| Cold reload with an empty library | — | identical | — |
| Past the cheek | −0.263 mm | −0.263 mm | — |

- The sand gates (ray release at 0.100 and 0.075 mm, draft-clamp bites) do not apply: the ring is lost wax (Logan, 2026-09-27). Both reviewers confirmed the waiver.
- The two-part undercut is reported only: 10.46% of the band, 14.75% with the part.
- Round 5's final build had the same gate results: 1,402,610 export triangles, all 12 gates passing.

Land widths (export), thinnest section and area under the 0.8 mm floor:

| Kind | Thinnest (mm) | Under (mm²) |
|---|---|---|
| body | 0.801 | 0 |
| nuchal hump | 0.809 | 0 |
| head and neck | 0.800 | 0 |
| limbs | 0.800 | 0 |
| **toes** | **0.653** | **0.03** |
| claws (points, by design) | 0.009 | 1.84 |
| tail | 0.800 | 0 |
| hump spines | 0.035 | 3.43 |
| brow horns | 0.003 | 2.59 |
| major thorns | 0.001 | 18.20 |
| minor thorns | 0.000 | 16.99 |
| tail thorns | 0.003 | 0.71 |
| hide tubercles (relief) | 0.000 | 20.54 |

`dfm::part_sections` on the part alone reads 0.0001 mm thinnest, with 64.3 mm² under the floor out of 622.9 mm². Every shortfall is a point or a relief flank, and each is named in `report.json`.

## Template gate (after the last round)

`collection_templates cataphracta target/tpl-src --output-dir target/tpl --only moloch --verify-export`, with `template_class: painted` and no exposed controls:

| Measure | Value |
|---|---|
| Gate | **passed** |
| `design.set` patches | **0** (limit 4) |
| Graph size | **2,362,958 B** against the 3,000,000 B painted budget |
| Nodes | 27 |
| Cold source parity (`source_identical`) | true |
| Mesh parity (`vertices_faces_normals_identical`) | true, 1,402,980 triangles |
| Cold design, cold graph and editable graph reloads | true |

The file is `showcase/cataphracta/moloch/template-verification.json`. It matches this build.

## What each layer and part is, and why

| Layer or part | What it is | Why |
|---|---|---|
| **Band** | A low dome squared at the cheeks, 6.6 mm wide, keyed wider under the lizard (up to 1.5×), 18.6 mm bore, lost wax at a 0.8 mm fill floor. | The figure needs crown under its sprawled legs. The cheeks stay polished with hard edges. |
| **Sand ripples** (`TilingLayer`, Max blend, 8 tiles, 0.22 mm) | The desert the devil lies on, winding round the crown under the figure. | It gives the band a ground without a polished halo. |
| **Thorny devil** (one stored sculpt, joined, about 204k faces) | A distance field meshed by `sculpt` (see below). | — |

The sculpt's parts:
- **Trunk:** a broad flat egg sunk into the crown.
- **False head:** the nuchal hump, with its two great spines.
- **Head:** a low wedge with a flat-plate mosaic and two swept horns.
- **Limbs:** four tapering legs.
- **Feet:** five keeled, clawed toes each, plus a buried sole.
- **Tail:** a flattened chain of eggs tapering along the crest toward the palm.
- **Cones:** 4 brow-horn parts, 2 hump spines, 22 major thorns in paired rows graded shoulder to hip, 62 minor thorns over the flanks and limbs, and 17 tail thorns lying along the tail.
- **Hide:** a close field of pebbled tubercles (Worley cells, 0.8 mm pitch, 0.2 mm) running into every cone's fillet.

**Stones:** none, as specified.

## What I could not do

- **Reach 7.5.** Round 6's punch list is still open:
  1. **Ripples:** asymmetric 3:1 crests, pitch varied ±25%, 2–3 Y-junctions per 90°, and no pooled highlights on the palm side.
  2. **Head:** a cleanly plated wedge with no seams, no polished snout dome and no forward horn. Tubercles over the smooth domes at the brow and mid-body cone roots.
  3. **Flank thorns:** leaned back at least 45°, with every tip inside the cheek plane.
  4. **Front limbs:** no elbow knob, and laid onto the band so they don't bridge it in profile.
  5. **Treatment text:** the thorn-point treatments state a 0.28 mm tip sphere, while the measured tips read 0.001–0.035 mm.
- **Keep `crisp_relief`.** The template graph cannot carry it yet (below).
- **Make the crease-normal render work for a sculpt.** The framed renderer's crease normals are right for stamps and booleans, but they show a tetrahedral sculpt's micro-folds as foil. I shaded the free sculpt surface smooth in the example, and said so above.

## Core changes wanted (exact code)

1. **Smooth shading for a sculpt's free surface.** This is the example's `shading_mesh`, lifted into `render.rs` so a ring can opt in without copying it:

```rust
// crates/ringdesign-core/src/render.rs
/// `mesh` with its corner normals dropped on every face whose three corners all satisfy `free`: a tetrahedral sculpt's
/// sub-0.05 mm folds then shade with the mesh's smooth vertex normals instead of as crumpled foil, while faces at a
/// join or a boolean keep their creases. Geometry is untouched.
pub fn smooth_where(mesh: &crate::mesh::Mesh, free: &dyn Fn(crate::mesh::Vec3) -> bool) -> crate::mesh::Mesh {
    let mut out = mesh.clone();
    out.corner_normals.retain(|(fi, _)| !mesh.faces[*fi as usize].iter().all(|&i| free(mesh.vertices[i as usize])));
    out
}
```

2. **`crisp_relief` through the template graph.**
   - With `d.crisp_relief = true`, `collection_templates … --verify-export` fails with `#29: the design failed upstream` (`ringdesign-graph/src/eval.rs:601`). The same design without the flag passes.
   - I did not trace which node drops the flag, so I have no exact patch for this one. The design node's import from a saved `RingDesign` needs to carry `crisp_relief` into the graph's design value (format 2), and the design-assembly node needs to accept it.
   - Repro: set `d.crisp_relief = true` in `band()` of `cataphracta_moloch.rs`, export, and run the template gate.

Earlier requests, still wanted:


3. **A relief-aware section census**, so pebbled hide over a thick body is not reported as a 0.000 mm section. This is the logic in `land_census` in the example, lifted into `dfm.rs`:

```rust
// crates/ringdesign-core/src/dfm.rs
/// [`part_sections`], with a face whose ray leaves the metal within `relief_mm` of `skin` (the part's field without
/// its relief) counted as relief rather than as a section: a tubercle's own flank is not a wall the metal must fill.
/// Returns (thinnest section, area under `floor_mm`, relief area under `floor_mm`).
pub fn part_sections_relief(solid: &crate::csg::Solid, floor_mm: f64, relief_mm: f64, skin: &dyn Fn(crate::csg::P3) -> f64) -> (f64, f64, f64) {
    use crate::interaction::bvh::Bvh;
    let mesh = crate::mesh::Mesh {
        vertices: solid.v.iter().map(|p| crate::mesh::Vec3(p[0] as f32, p[1] as f32, p[2] as f32)).collect(),
        faces: solid.f.clone(),
        ..Default::default()
    };
    let bvh = Bvh::build(&mesh);
    const IN: f64 = 1e-4;
    let (mut min, mut under, mut relief) = (f64::MAX, 0.0, 0.0);
    for f in &solid.f {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let e1: [f64; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let e2: [f64; 3] = std::array::from_fn(|k| c[k] - a[k]);
        let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
        let twice = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if !(twice > 1e-14) {
            continue;
        }
        let inward = n.map(|x| -x / twice);
        let o: [f64; 3] = std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0 + IN * inward[k]);
        let Some((_, t)) = bvh.ray(&mesh, o, inward) else { continue };
        let section = t + IN;
        if section >= floor_mm {
            min = min.min(section);
            continue;
        }
        let exit: [f64; 3] = std::array::from_fn(|k| o[k] + t * inward[k]);
        if skin(exit) > -relief_mm {
            relief += 0.5 * twice;
        } else {
            min = min.min(section);
            under += 0.5 * twice;
        }
    }
    (min, under, relief)
}
```

4. **A decimation that reports where it crossed**, so a caller can repair the field rather than fall back to the raw mesh:

```rust
// crates/ringdesign-core/src/sculpt.rs
/// [`clean_decimate`], or the sites where its first try crossed itself when no try is clean.
pub fn clean_decimate_or_sites(raw: &Solid, target: usize) -> std::result::Result<Solid, Vec<P3>> {
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, 18.0, 35.0);
        if csg::self_crossings(&nets) == 0 {
            return Ok(nets);
        }
    }
    Err(crossing_sites(&decimate(raw, target, 2e-3, 2.0, 18.0, 35.0)))
}
```

5. **A relax that repairs its own folds.** This is the round-4 fix in the example, lifted into `sculpt.rs`:

```rust
// crates/ringdesign-core/src/sculpt.rs
/// [`relax`], then the unrelaxed positions put back round each crossing it made, the patch widening until none is
/// left. Returns the vertices put back.
pub fn relax_clean(mesh: &mut Solid, field: Field, rounds: usize) -> usize {
    let before = mesh.v.clone();
    relax(mesh, field, rounds);
    let mut restored = 0;
    for reach in [0.3, 0.6, 1.2, f64::INFINITY] {
        let sites = crossing_sites(mesh);
        if sites.is_empty() {
            break;
        }
        for (v, orig) in mesh.v.iter_mut().zip(&before) {
            if sites.iter().any(|s| len(sub(*v, *s)) < reach) && *v != *orig {
                *v = *orig;
                restored += 1;
            }
        }
    }
    restored
}
```

6. **A band-blended join for stored sculpts.** This is a proposal only; I have not written its code.
   - A stored part flagged `fillet_into_band` should be unioned in the field domain: `sculpt::smin(part, stock.at(p), blend)` over the part's box, where `stock = sculpt::Stock::of(&atlas.samples, ..)`, clipped to the part's footprint dilated by the blend.
   - That would give the tail and the feet the 0.4 mm fillets the reviewers asked for.
