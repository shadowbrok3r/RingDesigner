# Lanterna (Tenebrae): final report

**Outcome: cut at 6.6 after three review rounds** (5.8, 6.2, 6.6; ship bar 7.5). Every gate was green at draft and at export in every reviewed round, and the template gate passed. The ring failed on art, not on a gate.

Branch `claude/tenebrae-lanterna`. Example `crates/ringdesign-core/examples/tenebrae_lanterna.rs`. Outputs are in `showcase/tenebrae/lanterna/`.

## Read tests (block-out, 3 attempts used of 3)

| Attempt | Reads | What the reviewer saw |
|---|---|---|
| 1 | false | "A plain octagonal signet with a spiky crown, or a chess rook / castle turret" stuck on top. |
| 2 | false | "Gothic tower" or "cathedral turret ring", but the window heads read rounded and the face read as a snowflake. |
| 3 | **true** | "An octagonal Gothic tower" with a ring of pinnacles, lamps and a square amethyst on the roof. |

## Reviews (3 rounds used of 3)

| Round | Verdict | Score | Main punch items |
|---|---|---|---|
| 1 | revise | 5.8 | Spires should taper with crockets and a knop, one per corner. Windows should be pointed two-light windows with a cusped oculus and a hood. Piers should be sculpted. Ornament should run down the shank. |
| 2 | revise | 6.2 | Spirelets should come to a point. The windows still read as keyholes. The buttress is a saw-tooth and the shank is bare. Stones too small. Template gate not yet run. |
| 3 | **cut** | **6.6** | Spires still blunt, with no leaf crockets. The stained glass shrank (the 8 lamps were removed). The shank arcade reads as square tiles and the buttress as stair-steps. The roof rim has no profile. |

In round 3 the reviewer credited:
- the lantern reading as a Gothic building at 300 px;
- crisp plate-tracery windows, with the keyholes gone;
- the star vault on the face;
- the most distinct silhouette in the collection;
- clean engineering throughout.

## Gates (round 3, as reviewed)

| Gate | Draft (768 × 320) | Export (1536 × 448) |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | pass | pass (77,136 triangles) |
| Every made part 0 crossings, every feature Ok | pass | pass (37 parts, 68 of 68 features Ok) |
| Solids and parts notes empty | pass | pass |
| Nothing in the finger hole | pass | pass (nearest vertex 9.3000001 from the axis, bore radius 9.3) |
| Thickness at 0.8 mm (`cad::measure::thickness`) | pass (384 rays, 0 below, min 0.829) | pass (same) |
| `dfm::cut_lands` at 0.8 mm | pass (0 findings) | pass |
| `dfm::findings_in` | 0 | 0 |
| Stone count equals the preview | 1 = 1 | 1 = 1 |
| `--verify` cold reload | n/a | identical |
| Casting pattern watertight, 0 degenerate, 0 crossings | pass | pass |
| Export ≤ 2 M triangles | 77,136 | 77,136 |

On top of the gates:
- **Cut lands measured.** Running `dfm::cut_lands` again with a 50 mm floor makes every cut report its narrowest land. They are 0.83 (window lights), 1.20 (vault lights), 2.47 and 3.52 mm, recorded in `report.json → cut_lands_measured`.
- **Dense thickness screen.** My own per-part screen at 4,000 samples, not the gate's 384, found nothing under 0.8 mm outside the bare stock.
- **The bare stock has its own thin spots.** The bare 015 stock has 21 of 12,500 triangles under 0.8 mm on its own edge rounds, the least 0.185 mm. They are recorded in `report.json → bare_stock_census` and kept apart from the ring's parts.

## Template gate

`collection_templates tenebrae target/tpl-src --output-dir target/tpl --only lanterna --verify-export` (procedural, no exposed controls). The full output is in `showcase/tenebrae/lanterna/verification.json` and summarised in `report.json → template_gate`.

| Check | Result |
|---|---|
| Template gate | passed |
| `design.set` patches | 1 (`/manufacturing`), limit 4 |
| Graph size | 282,443 bytes, budget 300,000 (procedural class), 79 nodes |
| Cold source parity | source identical; cold design and graph reload true |
| Mesh parity | vertices, faces and normals identical (77,136 triangles) |
| First build when opened | 1,412 ms |

## Enablers and master merges

I merged master at the start of every round:
- round 2 included #248, the crisp render fix, and #250;
- round 3 included #257 (C-B2) and #258 (C-V2/C-V5 arrays along paths).

| Enabler | How I used it |
|---|---|
| `Sketch::tracery` (C-T1) | Built the vault in round 1. It was later replaced by half-round rolls, which unite cleanly. |
| `dfm::cut_lands` (C-T4) | Run as a gate, and again at a 50 mm floor to measure every cut. |
| C-T7/C-V3 sweep and twist (`Operation::twist`) | The spirelets. |
| #248 framed close-ups | `stones.png` is rendered with `render::write_png_framed(..., render::Framing::new([0, 17, 0], 8.5), ...)`, never a cropped mesh. |

Not used:
- **`crisp_relief` and `StampTop::Pillow`.** The ring has no height-field relief and no domed stamps. The design still saves at `format_version` 6, which is master's current format, not because of an opt-in.
- **C-V2/C-V5 arrays along a path.** They landed after round 3's design was set. The shank bays are laid one by one on tangent work planes instead.

## The feature tree, in sentences

1. Take the factory 015 octagon (16 × 16, lost wax, Gold 18k, section raised to 0.8 mm) as the lantern's plinth.
2. Raise the lantern on the table: an octagon of apothem 6.4, 7.2 high.
3. Hollow the lantern inside 1.0 mm walls and roof, open down to the finger.
4. Raise a base course round its foot, weathered at 35°.
5. Raise a rim round the roof that overhangs the walls by 0.15 as a cornice.
6. Raise the boss at the vault's crown, the amethyst's collet on it with a bore that follows the Asscher's cut-corner square, and a roll moulding round the boss's foot.
7. Run a half-round rib from the boss to the first corner and two tiercerons out to the star's point, then pattern them round all eight bays into an eight-point star vault.
8. Pierce a light in each bay inside the ribs.
9. On each wall, raise a pointed-arch tracery plate 0.35 proud, then pierce two two-centred lancets and a cusped quatrefoil through the plate and wall together. Pattern this onto all eight walls.
10. Stand a pinnacle on every corner, as an 8-fold pattern of:
    - a chamfered lower stage, weathered back at 45°;
    - a chamfered upper stage, weathered at 45°;
    - a chamfered shaft;
    - a chamfered spirelet, twisted 45° as it tapers 1.25 → 0.82;
    - a knop, lofted on 16-sided latitudes.
11. Lay a plane under the band's mid-plane and raise the clasping buttress on the right shoulder in three level stages stepping down the shoulder, then mirror it.
12. Lay tangent planes over the band at 159°, 172.6°, 186.2°, 199.8° and 213.4°. On each, raise a flat-topped arcade bay round a lancet open to the band, sunk 0.9 mm into the crowned shank. Mirror the five bays down the other shank.
13. On a plane leaning 12° with the head's long wall, raise a five-lancet arcade frame, and pattern it onto the other long wall.
14. Set the amethyst in the boss's collet and cut its seat through the collet and boss, open to the lantern below.

## Parts and stones

- **Lantern, vault, rim and boss.** These are the subject: a Gothic crossing lantern seen from above (the face) and from three-quarters (the hero).
- **Tracery plates.** They give each window a pointed outline in the hero. The pierce runs through plate and wall together, so no raised edge sits beside a hole lip.
- **Pinnacles.** They make the crown silhouette.
- **Buttresses and the two arcades.** They carry the building onto the shoulders and shank, leaving the palm plain.
- **Stone: one Asscher amethyst, 3.0 mm (0.134 ct)**, tinted violet, in a made collet, set à jour through the boss.

## What I could not do

- **Pointed spires.** The reviewers asked for a tip ≤ 0.25 mm, which conflicts with the 0.8 mm lost-wax section: any taper under 0.8 shows up as thin samples. The spirelets stop at 0.82 mm under a knop.
- **Leaf crockets.** Turned-sphere crockets cost about 8,000 faces each and pushed the mesh past the thickness screen's 250k-face work limit. Lofted buds read as "pins", so they were removed. Hooked leaves were not built.
- **Citrine lamps in the walls.** A collet holding 0.8 mm of wall round a 1.5 mm stone is 3.5–4 mm across, which does not fit the 2.8 mm of wall between piers. The proud collets read as washers and keyholes, and setting the stones flush in the 1.0 mm wall left a 0.6 mm cone wall. I removed them in round 3. The reviewer counted that against the ring.
- **A 5 mm amethyst.** The boss (r 3.45) sits inside the vault's ridge at 4.6. A bigger stone needs a bigger boss and smaller vault lights.
- **A continuous shank arcade.** Frames laid on tangent planes 13.6° apart cannot share an edge. Overlapping them left 0.02 mm slivers, so the bays are separate tiles with 0.19 mm joints, and they read as tiles.
  - Cuts into the crowned shank feathered at their lips, which the thickness screen catches.
  - A swept arcade along the band (C-V2/C-V5) was the right tool, but it landed too late.
- **A moulded rim, a string course, and sloped weatherings on the buttress.** Not reached.
- **Stale bench notes.** The design's bench notes still mention citrine lamps from rounds 1–2. I left them as reviewed rather than change the design after the verdict.

## Core changes wanted

1. **Let the thickness screen measure big meshes and sample densely.** Today `thickness` refuses meshes over 250,000 faces and samples 384 triangles. Wanted: the same screen with the sample count and face cap as parameters, keeping the old signature.
   ```rust
   // crates/ringdesign-core/src/cad/measure.rs
   pub fn thickness(mesh: &Mesh, limit_mm: f64) -> Thickness {
       thickness_sampled(mesh, limit_mm, 384, 250_000)
   }

   pub fn thickness_sampled(mesh: &Mesh, limit_mm: f64, samples: usize, max_faces: usize) -> Thickness {
       let mut r = Thickness { sampled_min_mm: None, point: None, rays: 0, unresolved: 0, below_limit: 0, limit_mm,
           note: "Surface-normal samples on the display mesh; small unsampled features and oblique walls require section inspection" };
       if !mesh.validate().watertight || mesh.faces.len() > max_faces {
           r.note = "Thickness not assessed: invalid mesh or sampling work limit exceeded";
           return r;
       }
       let stride = mesh.faces.len().div_ceil(samples.max(1)).max(1);
       // ... the existing loop unchanged, stepping by `stride` ...
       r
   }
   ```
2. **Report lip samples separately.** A cut into a convex surface leaves a lip where the inward ray grazes out through the surface. A pierced hole beside a raised band does the same. The screen counts these as thin metal. Wanted: tag a sample as a lip when the ray leaves through a face nearly parallel to it, close to where it started.
   ```rust
   // in the sampling loop, once `nearest` is known (keep the index of the face hit, `j`):
   let exit_normal = unit(face_normal(triangles[j]));
   let grazing = dot(exit_normal, inward).abs() < 0.17; // within ~10° of parallel to the ray
   if value < limit_mm && grazing && value < 0.25 * limit_mm {
       r.lip_samples += 1; // new field: reported, not counted in below_limit
       continue;
   }
   ```
3. **A cheap ball primitive, or a chord option on `Revolve`.** A revolved r 0.45 sphere tessellates to about 8,000 faces. Forty of them tripled this ring's mesh.
   ```rust
   // crates/ringdesign-core/src/cad.rs, Operation::Revolve
   /// Chord tolerance for this revolve's tessellation, mm; the build's default when None.
   #[serde(default, skip_serializing_if = "Option::is_none")]
   chord_mm: Option<f64>,
   ```
4. **Pattern union notes should name the colliding pair.** "Could not be united with each other (two points coincide in a face)" listed every component. Finding the two (a spirelet foot coplanar with the rim's top, and a base course apothem) took bisection.
   ```rust
   // where the n-way union falls back to one-by-one joining, record the first failing pair:
   notes.push(format!("{} and {} meet degenerately ({snag}); joined one by one", names[a], names[b]));
   ```
