# Cataphracta — Moloch, the thorn idol: cloud lane report (lost wax)

**Outcome: cut at 7.1 in round 4.**
- The ring passed its lost-wax block-out read test on the first attempt, then went through the three standard full-review rounds (6.3, 6.7, then 7.0 and cut).
- Logan read the round-3 cut and asked for one more round aimed only at the round-3 punch list. Round 4 scored **7.1**.
- 7.1 is above Caiman (7.0) but below Basiliscus (7.2, cut) and the 7.5 ship bar, so the ring is cut. The reviewer's verdict stands.

Counts:
- Block-out read tests: **1 used, and it passed** (`reads: true`). The count restarted at 1 for lost wax. The first (sand) session's report is in git history at `a25c95d`, and its three failed read tests are kept as `read-test-sand-{1,2,3}.json`.
- Review rounds used: **4**: the standard three, plus the one Logan granted.
- Every lost-wax gate is green at draft (768 × 320), at export (1536 × 448, with `--verify`) and at 384 × 192, in every round.
- The template gate **passed** after round 3 and again after round 4.

Branch: `claude/cataphracta-moloch`. Author file: `crates/ringdesign-core/examples/cataphracta_moloch.rs`. Artwork: `crates/ringdesign-core/examples/cataphracta/art/moloch/sand-ripples.svg`. Outputs: `showcase/cataphracta/moloch/`.

## Read test and reviews (independent reviewers, a new one each time)

Every reviewer was told the ring is lost wax and which gates apply.

| Step | Verdict | Score | What the reviewer saw (condensed) |
|---|---|---|---|
| Read test 1 | **reads: true** | — | "The first Moloch that is an animal and not a spiked band." Face-300 is "a spiky lizard with four legs, a head, a false-head hump and a tail; most would say thorny devil or horned toad." Three notes: the head was a lump with hollows; the hero read as a hedgehog bristle, with the legs hidden; the tail was a fishbone. |
| Round 1 | revise | **6.3** | It reads as an animal, and every gate was independently re-verified. The limbs were even tubes reaching 2–3 mm past the cheeks. The toes were at 0.11 mm. Ball-under-cone "eyes" sat at the nape. The body was bare between the cones. The ripple band was smeared and had tile seams. |
| Round 2 | revise | **6.7** | The limbs are within 0.75 mm of the cheek, the toes are at 0.77 mm, and the band is seamless. New faults: the head was a smooth pointed wedge ("shark snout"), and the granules were loose beads that clumped like frog spawn and read as eyes. The ripples read as wood grain. |
| Round 3 | cut | **7.0** | A close pebbled tubercle hide reads as a thorny devil at 300 px. What still failed: the ripples ran 40–60° off the circumference, read as grain, showed stair-step banding and left a 3–4 mm polished halo. The feet showed no 5 distinct toes. The tail ran outboard in reverse view. There were cones on smooth domes and an eye-like socket by the brow horn. |
| Round 4 (Logan's extra round) | **cut** | **7.1** | Fixed: the ripples now run round the ring, cover the whole crown, fork and end, and no longer read as grain. Each foot has five splayed toes. It still reads as a thorny devil at 300 px. New or remaining faults (below). |

The round-4 faults, in the reviewer's words (condensed):
- **Feet:** the feet read as flat cartoon hands or a gecko's feet, with rounded fingertips. The toe curled over each edge stands off the cheek like a peg or sprue stub. Toe sections fell to 0.175 mm (named).
- **Wrists:** lumpy knob clusters remain at the wrists.
- **Ripples:** they are regular even-pitch zigzags that read as tyre tread or stylised water. They show crinkled highlights on the palm side and still leave a 1.5–2.5 mm polished gap by the hands.
- **Tail:** it still reads as a spiked rail from the reverse.
- **Cones and head:** polished domes remain under the mid-body cones. There is a round dome on the head, and in close-up the head is a cluster of bubbles.

The reviewer also noted that the extra round was not recorded in the repo, only in the launching agent's message. It said this does not change the verdict.

Full verdicts: `showcase/cataphracta/moloch/read-test-1.json` and `review-round{1,2,3,4}.json`.

## What round 4 changed (aimed only at the round-3 punch list, the three named faults first)

1. **Sand ripples.**
   - The ripples are now eight periodic tiles round the crown, each about 8.9 × 6.5 mm. The alpha rasterises its 1024 px edge over one tile instead of the whole 71 mm circumference, so about 0.01 mm per pixel instead of 0.07. That removed the stair-step banding.
   - Each crest climbs exactly one crest spacing across a tile, so crests run on from tile to tile and wind round the ring at a shallow angle.
   - Profile: a stack of light windward strokes, then a sharp lee crest, 0.22 mm tall, at 1.35 mm pitch.
   - Some crests break and pick up again. Their windward strokes taper away at a break, where earlier they stacked into a tick.
   - The fade mask round the lizard is gone: the ripples run under the joined figure, so it sits in the ground.
2. **Feet.**
   - Each leg now arches from the flank: the elbow rises over the crown edge, and the hand rests on the crown.
   - Each hand has five toes, 0.8 mm at the root tapering to 0.6 mm. Four splay on the crown from −40° to 45°, and the outermost curls over the edge with a knuckle and grips the cheek.
   - A crown-height lookup from the atlas lays each toe on the crown.
   - The fan was re-laid several times to keep tips off neighbouring toes, the tail and the forearm.
3. **The rest of the punch list:**
   - **Tail:** a flattened oval (72% as tall as wide), sunk to 0.1 × its radius over the crest, built as a chain of blended eggs.
   - **Cone roots:** the tubercle hide now runs up into every cone's fillet, and major fillets are 0.28 mm.
   - **Brows:** blended swellings under the horns fill the socket.
   - **Sculpt pipeline:** where a relax folds the mesh, the relax is undone only within 0.3 mm of the fold.

## Gates (round 4, the committed design)

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
| Triangles (budget 2 M) | 598,240 | 1,403,596 | 283,718 |
| Cold reload with an empty library | — | **identical** | — |
| Past the cheek (the round-1 punch limit is 0.8 mm) | 0.479 mm | 0.479 mm | — |

Two-part undercut, reported as a number only:
- band 12.42%, up from 6.67% because the ripples now run round the ring;
- with the part, 19.44%;
- the part itself undercuts 166.6 mm² of its 427.4 mm².

Land widths:
- `dfm::part_sections` on the part: thinnest 0.000 mm, 103.5 mm² under the floor, of 622.7 mm².
- By kind, thinnest section and area under the floor:

| Kind | Thinnest | Under the floor |
|---|---|---|
| body | 0.801 | 0 |
| hump | 0.800 | 0 |
| head | 0.802 | 0 |
| limbs | 0.800 | 0 |
| tail | 0.800 | 0 |
| toes | 0.175 | 31.98 mm² |
| hump spines | 0.223 | 3.45 mm² |
| brow horns | 0.013 | 2.82 mm² |
| major thorns | 0.002 | 19.04 mm² |
| minor thorns | 0.001 | 11.13 mm² |
| tail thorns | 0.002 | 7.77 mm² |
| hide tubercles | 0.000 | 27.30 mm² |

- Every entry under the floor is named in `report.json` with its bench treatment and measured section.
- The toe entry is a regression against round 3's 0.719 mm, and it fails round 3's own acceptance number (0.6 mm). The toes are 0.6–0.8 mm fingers fused to the crown. On the part alone, their tips and crotches read under the floor.

Timing:
- Authoring takes about 116 s: 755 k raw faces, decimated to 206 k.
- The build takes 2.6 s at draft and 5.7 s at export.
- The design file is 2.80 MB.

## Template gate (re-run after round 4)

`collection_templates cataphracta … --only moloch --verify-export`, class `painted`:

| Round | Nodes | `design.set` patches | Graph bytes | Budget | Cold source | Mesh parity | First build | Gate |
|---|---|---|---|---|---|---|---|---|
| 3 | 27 | 0 | 2,163,801 | 3,000,000 | identical (lifted natively) | 1,412,122 triangles, identical | 2.8 s | passed |
| **4** | 27 | **0** | **2,227,034** | 3,000,000 | identical (lifted natively) | **1,403,596 triangles, identical** | 2.3 s | **passed** |

`showcase/cataphracta/moloch/template-verification.json` now holds the round-4 record.

## What each part is, and why

**Base.**
- `ProfileStyle::LowDome` 6.6 × 2.0, crown 0.55, `flatten_sides()`, comfort 0.15, bore 18.6.
- Keys:
  - Width is 1.45 under the lizard (65–110°), tapering to 1.0 by 210°. The legs can then reach over the crown and still stay within 0.8 mm of the cheeks.
  - Thickness is 1.15 under the lizard and 1.0 at the palm.
- Process: `CastProcess::LostWax.apply`, then `min_section_mm = 0.8` and `min_draft_deg = 0`, as Logan decided.

**"Thorny devil"**, one stored part (`Operation::Stored`, recipe kernel `sculpt`), joined to the band:
- **Method.**
  - A distance field in a bent frame that follows the band's crest round the ring.
  - Meshed by `sculpt::tetra_mesh` at a 0.055 mm step, relaxed (undone locally where it folds), decimated to 230 k faces and settled.
- **Body.** A broad, flat ellipsoid, 14 × 7.9 × 4 mm.
- **False head.** A nuchal dome at the face with two stout 1.75 mm spines.
- **Head.** Small and blunt, with a dipped snout. It has blended brow swellings, curved two-segment brow horns (1.9 mm) and three small cones along each brow.
- **Legs.** Each leg is a tapering, arched limb with four cones and a hand of five toes on the crown, the outer toe curled over the cheek.
- **Tail.** A flattened oval chain, 1.4 to 0.5 mm radius over 15.5 mm, with whorled cones.
- **Thorns: 75 cones.** 22 major (paired rows and the shoulder and hip spines), 30 minor, 17 on the tail, 4 horn segments and 2 hump spines.
- **Hide.** A Worley tubercle field, 0.8 mm cells, 0.2 mm high. It runs into the cone fillets and is faded off the toes.

**"Sand ripples"**, the band's only layer: eight periodic tiles of wind ripples round the crown, running under the figure. The cheeks stay polished.

**Stones:** none, as specified.

## What I could not do

- **Reach 7.5.** Round 4's punch list:
  1. **Feet:** Moloch feet with slender clawed toes, not glove-like hands, and no toe leaving the crown. Toes at 0.6 mm or more, with 1.0 mm² or less under the floor.
  2. **Ripples:** varied wavelength (1.0–1.8 mm) and amplitude, a micro-ripple in the troughs, clean crest highlights on the palm side, and ripples reaching within 0.8 mm of the figure everywhere.
  3. **Limbs:** no knob clusters at the wrists.
  4. **Tail:** tail thorns pointing up and back along the tail, not outward.
  5. **Cones and head:** no smooth domes under the cones, and a head of flat angular scales rather than bubbles.
- **Toe sections.**
  - 0.8 mm is the fill floor. A toe any finer reads under it on the part alone, and every fan that gives five distinct toes on a 1–1.5 mm strip of crown leaves some tip or crotch reading thin.
  - A census on the as-cast metal (the part joined to the band) would show the fused toes' real sections. The example measures the part alone, as `dfm::part_sections` does.
- **Fillets between the part and the band.** This is unchanged since round 3: a band-blended join would seat the tail and the toes properly.

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

3. **A relax that repairs its own folds.** This is the round-4 fix in the example, lifted into `sculpt.rs`:

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

4. **A band-blended join for stored sculpts.** This is a proposal only; I have not written its code.
   - A stored part flagged `fillet_into_band` should be unioned in the field domain: `sculpt::smin(part, stock.at(p), blend)` over the part's box, where `stock = sculpt::Stock::of(&atlas.samples, ..)`, clipped to the part's footprint dilated by the blend.
   - That would give the tail and the feet the 0.4 mm fillets the reviewers asked for.
