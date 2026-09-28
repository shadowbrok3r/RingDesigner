# Tenebrae — Oculus: final report

**Outcome: stopped at the block-out.** All three read tests came back `reads: false`, so under TASK.md the loop stopped before round 1. **Rounds used: 0 of 3.** No full review was run, so there is no review verdict or score. The subject needs rethinking before it gets any more detail (see "What the reviewers kept saying").

Branch `claude/tenebrae-oculus`. Example `crates/ringdesign-core/examples/tenebrae_oculus.rs`. Outputs in `showcase/tenebrae/oculus/`.

## Read tests (independent reviewer, read-test mode)

| # | Build | Process | reads | What the eye saw |
|---|---|---|---|---|
| 1 | 24 pointed lights through a 7.0 × 4.6 Flat band (the spec) | Delft sand | **false** | "a flat gold washer with 24 small identical bullet-shaped notches… a bearing cage, gear blank, sun-ray border" |
| 2 | 12 lancets + 12 quatrefoils in a sunk tracery field, crown arcade of pointed arches | Lost wax | **false** | "a decorative border, a pierced gallery wire… 'pierced band with quatrefoils', possibly 'Gothic-ish'" |
| 3 | 16 lancets, flat sills, with 16 quatrefoils between the heads; V-joint voussoirs on the crown; antique-darkened piercing walls | Lost wax | **false** | "'pierced band with Gothic-style quatrefoils' or 'tracery gallery'… the concept is being defeated by scale" |

The full JSON is in `showcase/tenebrae/oculus/read-test-{1,2,3}.json`.

### What the reviewers kept saying

All three reviewers named the same limit. Seen along the finger, the side face is an annulus of about 3.4–4.3 mm around an 18.6 mm hole. At 300 px it reads as a thin decorated frame around a big void, not as a window filled with tracery. Each reviewer asked for three concentric orders: hub moulding, lancets with spokes, and an outer ring of foils. That plus 0.8 mm lands and rails needs about 6 mm of radial run, so a band about 6 mm thick or a raised side flange. The hero camera makes it worse: at any three-quarter angle a 7 mm-deep piercing shows its walls, not light, so the band reads as a tube or roller cage. Detailing will not fix this. The concept has to change, for example:

- give the band a raised side-face flange;
- move the wheel onto a head (which overlaps Rosa);
- or make the window's orders from stained-glass stones.

## Sand → lost wax (the brief allows this; recorded here)

Read test 1 asked for cusped lancets. In Delft sand each light is two drafted pins (≥3° is needed for the field to read Castable). At 3.5° over each 3.5 mm half, every opening shrinks by 0.21 mm per side at the parting plane. Measured on the sketch, a 0.38 mm cusp in a 1.45 mm lancet goes from a 0.46 mm gap at the face to **−0.63 mm** at the waist, so the cusps cross. Even 0.15 mm cusps leave only 0.33 mm. The crown can hold no Gothic relief in sand either: any pocket or boss on a wall parallel to the pull locks one half. Sand could not hold what makes the ring read, so from attempt 2 on it is lost wax.

For the record, the sand route did pass its gates before the switch: a 12-lancet, 12-quatrefoil version with lofted, drafted lights. It was Castable, with ray release at 0.100 and 0.075 mm giving 0 obstructions and 0 unresolved, and 0 DFM findings. It lost the cusps, and it was not committed because it was superseded.

## Gates (the committed ring, block-out 3, lost wax, Silver 925)

| Gate | Draft 768×320 | Export 1536×448 |
|---|---|---|
| Watertight / degenerate faces | yes / 0 | yes / 0 |
| `csg::self_crossings`, ring / every made part (4) | 0 / 0 | 0 / 0 |
| `built.solids.notes` / parts notes / unresolved stamps | empty / empty / 0 | empty / empty / 0 |
| CAD features `Ok` | 9 of 9 | 9 of 9 |
| Finger hole: min vertex radius vs bore 9.300 | 9.30001, 0 inside | 9.30000, 0 inside |
| Field verdict (lost wax, informational) | Castable | Castable |
| `cad::measure::thickness(0.8)`, on a 384×160 build (the measure refuses more than 250k faces) | 384 rays, min 0.817, 0 below, 0 unresolved | same |
| Asserted lands (≥ 0.8) | lancet to quatrefoil 0.834; bore rail 1.03; outer rail 0.85; quatrefoil cusp tips 0.84 wide; field floor web 5.8 | same sketch |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed | 0 / 0 | 0 / 0 |
| `--verify` cold reload, empty library | — | identical |
| Triangles (≤ 2 M) | 402,736 | 1,090,868 |
| Casting pattern: watertight, degenerate, crossings | yes, 0, 0 | yes, 0, 0 |

All gates are green. The ring was stopped on legibility alone.

**Template gate:** `procedural`. 21 nodes. **1 `design.set` patch** (`/manufacturing`, at most 4 allowed). Graph **41,284 bytes** against the 300 KB budget. Cold source identical, cold graph reload true, mesh parity (vertices, faces, normals) identical at 1536×448, `template_gate_passed: true`. The file is in `showcase/tenebrae/oculus/verification.json`.

**Weights (open question 4):**

- **4.6 mm band as built:** 16.6 g Silver 925, or 25.0 g in Gold 18k.
- **4.0 mm band, same 16 lights, apex pulled in to 12.15:** 14.4 g silver, or 21.6 g 18k. At that size the lancet-to-quatrefoil land falls to 0.72 mm, under the 0.8 minimum. So the lighter ring needs smaller foils, and it read no better, because the read failed at 4.6 already.

## Feature tree, as sentences

1. Procedural shank: a Flat band, 7.0 × 4.6, squared sides, a shallow 1.7-exponent crown, comfort 0.1, Uniform, bore 18.6.
2. High side face: a parting-plane work plane at +3.5.
3. The tracery field between the hub and the rim: an annulus from r 10.2 to r 12.95.
4. Sink the tracery field, leaving the hub and the rim standing: 0.6 mm deep, 20° bevel.
5. Sink the same field in the low side face: a mirror across the band.
6. High side face, lifted clear of the metal: at +3.8.
7. One bay of the wheel: a pointed lancet (1.55 wide, flat sill at r 10.3, equilateral head to r 12.75) and the quatrefoil beside its head (r 11.95, cusp tips rounded to 0.84 mm).
8. Pierce the bay through the band from side face to side face.
9. Wheel the bay round the finger: sixteen lancets and sixteen quatrefoils (a Ring pattern).

Field layer "Voussoir joints": 16 V joints, 0.6 wide and 0.3 deep, cut over the spokes and gated to the crown. The renders darken the piercing walls as an antique finish, which is also in the bench notes. There are no stones or stamps.

## What I could not do

- **Cusped (trefoil) lancet heads.** In sand the draft closes them. In wax a sharp cusp tip fails `thickness(0.8)`: a 0.04 mm sample hit one. A 0.8 mm round tip does not fit inside a lancet 1.5 mm wide. So the cusps live only in the quatrefoils, with blunted tips.
- **Crest milgrain.** Beads of 0.55 and 0.9 mm both measure under 0.8 in `thickness`, so the layer is off (`WITH_BEADS`).
- **A through-light or back-lit render.** It is not available in `render`.

## Core changes wanted

1. **Kernel loft reads a section's winding off its first corner.** `cadkernel::brep::loft::polygon_normal` takes the first non-degenerate fan triangle. On a concave start corner that gives the wrong winding and the loft fails. The workaround in the example starts every section on a convex corner. Fix it with the signed area (shoelace):
```rust
fn polygon_normal(points: &[[f64; 3]]) -> Option<Vec3> {
    let mut n = Vec3::new(0.0, 0.0, 0.0);
    for (i, p) in points.iter().enumerate() {
        let q = points[(i + 1) % points.len()];
        n = n + Vec3::from(*p).cross(Vec3::from(q));
    }
    n.normalize()
}
```
2. **Lofts through non-parallel polygon sections tessellate with open seams.** The NURBS quads are refined more finely than their planar neighbours. The workaround offsets each section parallel to itself, so every wall is a planar trapezoid.
3. **A `measure::thickness` that ignores surface bosses**, so beads and milgrain on a massive body are not counted as thin sections. For example, skip a ray that exits into air within `limit` and re-enters metal within 2 × `limit`.
4. **`render` antique finish:** a `Part` class for recess walls, so authors do not have to split meshes by `origin` as this example does (`antiqued`).
