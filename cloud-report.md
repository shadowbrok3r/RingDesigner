# Cataphracta — Moloch, the thorn idol: cloud lane report (lost wax)

**Outcome: cut at 7.0 in round 3.** The ring passed its lost-wax block-out read test on the first attempt. It then went through all three full-review rounds and scored 6.3, 6.7 and 7.0. That is the level of Caiman (7.0) and Kraken (cut at 7.0), below the 7.5 ship bar. The reviewer's verdict stands.

- Block-out read tests: **1 used, and it passed** (`reads: true`). The count restarted at 1 for lost wax. The first (sand) session's report is in git history at `a25c95d`, and its three failed read tests are kept as `read-test-sand-{1,2,3}.json`.
- Review rounds used: **3 of 3.**
- Every lost-wax gate is green at draft (768 × 320), at export (1536 × 448, with `--verify`) and at 384 × 192, in every round.
- The template gate **passed**.

Branch: `claude/cataphracta-moloch`. Author file: `crates/ringdesign-core/examples/cataphracta_moloch.rs`. Artwork: `crates/ringdesign-core/examples/cataphracta/art/moloch/sand-ripples.svg`. Outputs: `showcase/cataphracta/moloch/`.

## Read test and reviews (independent reviewers, a new one each time)

Every reviewer was told the ring is lost wax and which gates apply.

| Step | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|
| Read test 1 | **reads: true** | — | "The first Moloch that is an animal and not a spiked band." Face-300 is "a spiky lizard with four legs, a head, a false-head hump and a tail; most would say thorny devil or horned toad." Three notes: the head was a lump with hollows; the hero read as a hedgehog bristle, with the legs hidden; the tail was a fishbone. |
| Round 1 | revise | **6.3** | It reads as an animal, and every gate was independently re-verified. The limbs were even tubes reaching 2–3 mm past the cheeks. The toes were at 0.11 mm. Ball-under-cone "eyes" sat at the nape. The body was bare between the cones. The ripple band was smeared and had tile seams. |
| Round 2 | revise | **6.7** | The limbs are within 0.75 mm of the cheek, the toes are at 0.77 mm, and the band is seamless. New faults: the head was a smooth pointed wedge ("shark snout"), and the granules were loose beads that clumped like frog spawn and read as eyes. The ripples read as wood grain. |
| Round 3 | **cut** | **7.0** | A close pebbled tubercle hide now reads as a thorny devil at 300 px, and the hide checklist item passes. What still fails: the ripples read as grain or brushed streaks, run 40–60° off the circumference, show stair-step banding, and leave a 3–4 mm polished halo round the lizard. The feet show no 5 distinct toes. The tail still runs outboard in reverse view. A few cones stand on smooth domes. A socket by the brow horn reads as an eye. |

Full verdicts: `showcase/cataphracta/moloch/read-test-1.json` and `review-round{1,2,3}.json`.

## Gates (round 3, the committed design)

| Gate | Draft 768 × 320 | Export 1536 × 448 | 384 × 192 |
|---|---|---|---|
| Watertight, degenerate faces, self-crossings | yes, 0, 0 | yes, 0, 0 | yes, 0, 0 |
| Made part "Thorny devil": closed, crossings as made and as placed | 0 open edges, 0 / 0 | 0 open edges, 0 / 0 | 0 |
| Solids and parts notes; part joined | [], joined 1 | [], joined 1 | [], joined 1 |
| Bore: nearest vertex against the 9.300 mm radius | 9.300, 0 inside | 9.300, 0 inside | — |
| Field (`attributed_field_report` 256 × 128 plus `judge_parts`, lost wax) | **Castable**, thinnest wall 1.66 mm | **Castable**, 1.66 mm | — |
| `land_widths`: every section under 0.8 mm named | pass (0 unnamed) | pass (0 unnamed) | — |
| DFM findings | 0 | 0 | — |
| Stones reported / previewed | 0 / 0 | 0 / 0 | — |
| Casting pattern (`try_build_pattern`) | closed, 0 / 0 | closed, 0 / 0 | — |
| Triangles (budget 2 M) | 601,684 | 1,412,122 | 285,576 |
| Cold reload with an empty library | — | **identical** | — |
| Past the cheek (the round-1 punch limit is 0.8 mm) | 0.752 mm | 0.752 mm | — |

Two-part undercut, reported as a number only:
- band 6.67% (worst 31°, from the ripples);
- with the part, 11.93%;
- the part itself undercuts 114.1 mm² of its 439.3 mm².

Land widths:
- `dfm::part_sections` on the part: thinnest 0.000 mm, 63.7 mm² under the floor, of 612.4 mm².
- By kind, thinnest section and area under the floor:

| Kind | Thinnest | Under the floor |
|---|---|---|
| body | 0.802 | 0 |
| hump | 0.802 | 0 |
| head | 0.803 | 0 |
| limbs | 0.800 | 0 |
| tail | 0.801 | 0 |
| toes | 0.719 | 0.29 mm² |
| hump spines | 0.280 | 3.38 mm² |
| brow horns | 0.205 | 2.34 mm² |
| major thorns | 0.043 | 17.76 mm² |
| minor thorns | 0.002 | 9.70 mm² |
| tail thorns | 0.027 | 9.91 mm² |
| hide tubercles | 0.000 | 20.36 mm² |

Every entry under the floor is named in `report.json` with its bench treatment and its measured section:
- **Thorn points:** fed through the root and invested point up. A short-filled point is built back with a laser tack and filed.
- **Minor thorns and hide tubercles:** relief over the 0.15 mm detail floor.
- **Toes:** fused into the cheek.

Hide faces are separated from real sections as follows. A body face whose ray leaves the metal still within the tubercles' height of the smooth skin has crossed a tubercle's own flank. That is relief, not a section.

Timing:
- Authoring takes about 108 s, most of it the sculpt: 750 k raw faces, decimated to 205 k.
- The build takes 3.7 s at draft and 8.1 s at export.
- The design file is 2.27 MB.

## Template gate

`collection_templates cataphracta … --only moloch --verify-export`, class `painted`:

| Nodes | `design.set` patches | Graph bytes | Budget | Cold source | Mesh parity | First build | Gate |
|---|---|---|---|---|---|---|---|
| 27 | **0** | 2,163,801 | 3,000,000 | identical (source lifted natively) | 1,412,122 triangles, identical | 2.8 s | **passed** |

`showcase/cataphracta/moloch/template-verification.json` holds the record.

## What each part is, and why

**Base.**
- `ProfileStyle::LowDome` 6.6 × 2.0, crown 0.55, `flatten_sides()`, comfort 0.15, bore 18.6.
- Keys:
  - Width is 1.45 under the lizard (65–110°), tapering to 1.0 by 210°. The legs can then splay over the crown and still grip the cheeks within 0.8 mm.
  - Thickness is 1.15 under the lizard and 1.0 at the palm.
- Process: `CastProcess::LostWax.apply`, then `min_section_mm = 0.8` and `min_draft_deg = 0`, as Logan decided.

**"Thorny devil"**, one stored part (`Operation::Stored`, recipe kernel `sculpt`), joined to the band:
- **Method.**
  - A distance field in a bent frame that follows the band's crest round the ring, with arc `x`, height over the crest `h` and across `w`.
  - Meshed by `sculpt::tetra_mesh` at a 0.055 mm step, relaxed, decimated to 230 k faces and settled.
  - If the decimation crosses itself, the cone at the crossing is dropped and the field meshed again. None were dropped in the final design.
- **Body.** A broad, flat ellipsoid, 14 × 7.9 × 4 mm, sunk into the crown.
- **False head.** A nuchal dome at the face (about 97°) with two stout spines, 1.75 mm tall.
- **Head.** Small and blunt: an ellipsoid plus a dipped snout. It has curved two-segment brow horns (1.9 mm) and three small cones along each brow.
- **Legs.** Each leg is a tapering two-segment limb (radius 0.62 to 0.42) with four small cones, and five toes fused along the cheek.
- **Tail.** Round and tapering from 1.4 to 0.5 mm radius over 15.5 mm down the crest, with whorls of cones graded to the tip.
- **Thorns: 88 cones.**
  - 22 major cones in paired rows: dorsal, dorsolateral and lateral, with the great shoulder and hip spines. Their roots are 55% of their length.
  - 34 minor cones and knobs, and 26 tail-whorl cones.
  - Every cone's root sphere is buried to 72% of its radius and cut along the skin normal, with no ball under the cone.
  - Every cone leans 38° toward the tail.
- **Hide.** A Worley (F2 − F1) tubercle field: 0.8 mm cells, 0.2 mm high, with 0.2 mm grooves. It is faded off the toes and out of the limb–body crease.

**"Sand ripples"**, the band's only layer:
- A `Tiling` in one seamless tile round the crown. The cheeks stay polished.
- The SVG draws near-circumferential wind ripples at 25° to the circumference and 1.4 mm pitch. Some crests end and some fork, each with a half-weight windward slope and a sharp crest, 0.18 mm tall.
- An SVG mask fades the ripples round the lizard's footprint.

**Stones:** none, as specified.

## What I could not do

- **Reach 7.5.** The last reviewer's remaining punch items:
  - Crisp, genuinely sand-like ripples within 20–35° of the circumference, with no stair-step and no halo.
  - Five distinct toes per foot.
  - The tail fully on the crown, filleted.
  - No smooth domes under the major cones.
  - No eye-like socket beside the brow horn.
- **The ripples.**
  - My angle constant is 25°. Because the chart's `v` is compressed against `u` on the crown, the crests render at 40–60°.
  - The stair-step comes from the tile's raster: one 71 × 6.5 mm tile.
  - Both are fixable next time by pre-compensating the angle for the chart's aspect, and by splitting the crown into several tiles or raising the alpha resolution.
- **Fillets between the part and the band.**
  - A sculpt that joins the band meets it at a crease, so the tail and feet cannot be filleted into the crown.
  - A skirt blended under the tail made the join fail: "two cuts cross inside a face".
  - `sculpt::Stock` could carry the band into the field, but at this atlas size it costs minutes per build.
- **Toes that read as five.** Toes 0.8 mm thick (the fill floor), five to a foot on a 2.3 mm cheek, merge into a paddle at render scale.

## Core changes wanted (exact code)

1. **A relief-aware section census**, so pebbled hide over a thick body is not reported as a 0.000 mm section. This is the logic in `land_census` in the example, lifted into `dfm.rs`:

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

2. **A decimation that reports where it crossed**, so a caller can repair the field rather than fall back to the raw mesh:

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

3. **A band-blended join for stored sculpts.** This is a proposal only; I have not written its code.
   - `Component::blend_mm` today beads the seam loop.
   - A stored part flagged `fillet_into_band` should instead be unioned in the field domain: `sculpt::smin(part, stock.at(p), blend)` over the part's box, where `stock = sculpt::Stock::of(&atlas.samples, ..)`, clipped to the part's footprint dilated by the blend.
   - That would give Moloch's tail and feet the 0.4 mm fillets the reviewers asked for.
