# Cataphracta Gekko: cloud lane report

- **Branch:** `claude/cataphracta-gekko`, merged with master `803a93e` (round 3).
- **Example:** `crates/ringdesign-core/examples/cataphracta_gekko.rs`.
- **Outputs:** `showcase/cataphracta/gekko/`.
- **Core:** no `src/` file was touched.

## Outcome: cut at round 3, score 6.7

- **The read was solved on the fourth try.** The rethink moved the tokay onto a signet face, seen from above.
- **Three reviewed rounds were used, the TASK.md cap,** with no extra rounds.
- **The gates passed; the art did not.** Every lost-wax gate is green at draft, export and 384 × 192, the cold reload is identical, and the template gate passes. The art stayed under the 7.5 bar.

### Read tests

| Read test | Base and process | Reads | What the eye sees (reviewer, shortened) |
|---|---|---|---|
| 1 | Procedural Flat band, Delft sand | no | "a lizard or newt"; legs "like staples" |
| 2 | Procedural Flat band, lost wax | no | "a lizard crawling across a pebbled band"; the head "a small rounded knob" |
| 3 | Procedural Flat band, lost wax | no | "a lizard on a pebbled band"; the head an arrowhead; the back reads as Heloderma |
| 4 | **Factory 017 signet face, lost wax** (the rethink) | **yes** | "A gecko, on first look… all four limbs splayed… every foot ends in a fan of disc-tipped toes… the classic gecko emblem" |

### Reviews

| Round | Verdict | Score | Main points |
|---|---|---|---|
| 1 | revise | 6.0 | Reads as a gecko. Craft "clip-art": rivet spots, eyes bolted on, smeared tail stub. Template over the stock budget. Ray release not recorded. |
| 2 | revise | 6.6 | All gates green. Tail, ground cells, crescent lamellae and side rows fixed. Head a bulb, torn pupils, rivet spots, bare shoulders, streaky margin. |
| 3 | **cut** | **6.7** | All gates green. The margin is fixed. Head, spots, shoulders, side hide (read as "hex tiles"), lamellae (read as "gear teeth") and the 017 silhouette are still short. |

The full JSON is in `showcase/cataphracta/gekko/read-test-{1..4}.json` and `review-round{1,2,3}.json`.

## The base, and why

The ring uses **factory 017 Tonneau at 20 × 15 mm**, the master's 16 × 12 at 125%, with a Flat profile. The rethink is recorded in the Gekko section of the doc, approved by Logan on 2026-10-03.

- Its long axis runs round the ring, the way the gecko lies.
- Its 15 mm width takes the four splayed feet.
- It keeps the factory's hard angle between wall and face.
- No other Cataphracta ring uses it: Chamaeleo is on 001, Chelonia on 007 and Phrynosoma on 016.

On the procedural band of attempts 1–3, the figure was a thin strip along the crown, and its species cues were a few pixels at 300 px.

## Process

The ring is judged as **lost wax** under Logan's rule of 2026-10-03: a 0.8 mm minimum section and no pull rule.

The sand pass is a bonus, reported only, and the ring does not pull from sand. The two-part undercut is:
- 0.03% of the band;
- 5.13% with the part, 84.7 mm², almost all of it on the figure.

## Gates (round 3, as shipped to the reviewer)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | pass (215,436 triangles) | pass (215,436) |
| Tokay and Eyes uncrossed as placed | pass | pass |
| Notes empty, every stamp resolved (75), both parts joined | pass | pass |
| Nothing in the finger hole | pass | pass |
| Field Castable at the 0.8 mm fill, band and with the part; thinnest band wall 1.48 mm | pass | pass |
| Wall census (`cad::measure::thickness(&built.mesh, 0.8)`, lead's interim rule) | pass | pass |
| 0 DFM findings | pass | pass |
| Stones equal the preview (0 = 0) | pass | pass |
| Under 2 M triangles; casting pattern closed | pass | pass |

- **384 × 192 build:** passes.
- **Cold reload with an empty library:** identical vertices, faces and normals.
- **Triangle count:** the stored stock surface does not change with build steps, so every build has the same count. `report.json` records why.

**Wall census, as read:**
- 217,676 samples, 0 unresolved.
- Two wall zones, both specks listed in `report.json`:
  - 0.0094 mm² at 0.60 mm;
  - 0.0014 mm² at 0.57 mm.
- Edges: 977 samples over 4.55 mm².

**Walls the census named in round 2, all fixed:**
- the chevron's arms, 0.4 mm thick under 0.48 mm of relief;
- hind pads hanging over the face edge;
- toe roots 0.7 mm across;
- neighbouring pads pinching;
- shoulder tubercles 0.75 mm across.

The hand-made land census was removed when #261 landed.

## Template gate (after the last round)

- **Command:** `collection_templates cataphracta target/tpl-src --output-dir target/tpl --only gekko --verify-export`.
- **Result:** in `showcase/cataphracta/gekko/template-verification.json`.

| Measure | Value |
|---|---|
| `design.set` patches | **0** (limit 4) |
| Graph size | **2,498,642 bytes**, under the 3,000,000 budget of class `painted` |
| Cold design and graph reload | identical |
| Source | identical (`lift`) |
| Mesh parity at 1536 × 448 | vertices, faces and normals identical (215,436 triangles) |
| `template_gate_passed` | true |

**Why the class is declared `painted`:** the default class for an imported base is `stock`, whose 1 MB budget covers the factory surface alone. This ring also carries the tokay and the pebbled face as one stored sculpt (183k faces) plus its eyes (27k faces). At about 10 bytes a face, that weighs what a painted atlas weighs, and it cannot fit in 1 MB without losing the figure. The reason is in `report.json` and the doc.

## What is built, and why

- **"Tokay"** is a stored sculpt mesh, joined. It is a distance field from `sculpt` primitives in a frame laid on the measured face, meshed with `tetra_mesh` at 0.05 mm, relaxed, decimated to 183k faces and settled. It holds:
  - **The pebbled ground.** It covers the face and is cut from a chamfer-distance grid of how far each point stands inside the flat face:
    - a polished border 0.85 mm wide, where the plate lies under the factory face;
    - one crisp step up;
    - Worley granules 0.13 mm high at a 0.8 mm pitch;
    - a halo at half height round the figure.

    It is part of the sculpt because any field layer on this stock dips the bore (see the core bug below).
  - **The body**, seen from above:
    - a wedge head broad at the eyes, with brows, nostrils and a mouth line;
    - neck, trunk, shoulder and hip swells, and a faint spine ridge;
    - 11 irregular flat-topped cushion spots;
    - an S tail tapering to a tip on the face.
  - **Four limbs** bent at elbow and knee. Each foot carries five toes fanned 46° apart, with round pads. The toes end 0.75 mm inside the face's edge and are at least 0.86 mm across, so every toe stands over the floor.
- **"Eyes"** is a second stored part, joined. Two balls look up and out, each with a lid rim and a capsule-shaped slit pupil. They are meshed at 0.02 mm and decimated to 26,822 faces, with no crossings at the 1e-5 mm storage grid.
- **Stamps (75)**, on the shank:
  - **Split lamella:** a chevron at 270° (`StampTop::Pillow`) with arms 0.7 mm wide.
  - **Lamellae (14):** crescents out to ±68°, pillow-topped pads graded in pitch from 1.15 to 1.6 mm.
  - **Shoulder hide (60):** three staggered rows of hexagonal granules and tubercles from 138° down to the lamellae on each side, with six-point outlines to keep the graph small.
- **Stones:** none.

## What could not be done

The reviewer's round-3 list: open items, and the blocker for each where there is one.
- **The head and eyes.** They still read as a bulb with eyes stuck on, and the pupils and nostrils still tear at this mesh density. A clean 0.25 mm groove decimated to self-crossings at every target tried, which fell back to a 180k-face raw mesh.
- **The rivet look of the spots.** Softer cushions removed the ring, but the reviewer still reads them as rivets.
- **The shoulders.** The crown between the face's walls and 138° can't take stamps: they run off the edge or into the bore. I also tried side-face rows (`RowPath::SideFace`) with build-checked pruning: only 14 of them stood wholly on the face, some reached the bore, and one read as a 0.66 mm wall, so I dropped them. A field layer would fill the shoulders, but it trips the core bug below.
- **The silhouette.** Toe fans lapping over the face edge become thin lips over the factory wall, which the wall census rejects.
- **Wall-census whack-a-mole in round 3.** Specks under 0.02 mm² moved from run to run, between the granules and the limbs or toes lying just over them. The lead's 0.02 mm² clarification ended the chase, and the shipped build is the last one that looked right.

## Core changes wanted

1. **A field layer on stock 017 dips the bore.**
   - **Symptom:** any field layer on this stock, even one of height 0, pulls two bore vertices to r = 9.2889 mm. That is 0.011 mm inside the 9.3 mm bore, at θ 62.74° and 117.26°, z −0.007.
   - **Effect:** the ring has no field layers; its ground is in the sculpt and its shank hide is stamps. Stamps do not trigger it.
   - **Regression test** (for `crates/ringdesign-core/tests/`), built from `band()` in the example:

```rust
#[test]
fn a_field_layer_on_stock_017_keeps_the_bore() {
    use ringdesign_core::{AlphaLibrary, ProfileStyle, RingDesign, field::{Layer, LayerEntry, Window}, imported_base::{ImportedBase, PRESETS, SurfaceChart}, mesh, reptile::svg::{self as rsvg, Params}, svg::SvgAlpha, tiling::TilingLayer};
    let mut d = RingDesign::default();
    let source = PRESETS.iter().find(|p| p.id == "017").unwrap().load().unwrap();
    ImportedBase::attach(&mut d, source).unwrap();
    d.profile.apply_style(ProfileStyle::Flat);
    d.profile.width_mm = 15.0;
    d.shank.head.length_mm = 20.0;
    d.size = ringdesign_core::resize::size_from_bore(18.6).unwrap();
    d.imported_base.as_mut().unwrap().chart = Some(SurfaceChart { profile: d.profile.clone(), bore_radius_mm: d.inner_radius_mm() });
    let ctx = d.field_context();
    d.svgs.push(SvgAlpha { name: "Test".into(), svg: rsvg::tubercle_rows(&Params::new(1.3, 3.0, 0.42, 0.9)), invert: false });
    let mut t = TilingLayer::default_for("Test", &ctx);
    t.height_mm = 0.0;
    let mut e = LayerEntry::new("Test", Layer::Tiling(t));
    e.window = Window::around(270.0, 60.0);
    d.layers.layers.push(e);
    let mut lib = AlphaLibrary::default();
    d.bake_all(&mut lib);
    let built = mesh::try_build(&d, &lib, d.build).unwrap();
    let bore = d.inner_radius_mm();
    let inside = built.mesh.vertices.iter().filter(|q| (q.0 as f64).hypot(q.1 as f64) < bore - 0.01).count();
    assert_eq!(inside, 0, "a height-0 field layer moved bore vertices inside the finger hole");
}
```

2. **`clean_decimate` should say when it gave up.** Today it returns the raw mesh, compacted, when every try crosses. In round 2 that silently turned a 27k-face eye part into 180k faces, and a 185k-face sculpt into 1.8 M, and only the graph size showed it. Proposed addition to `crates/ringdesign-core/src/sculpt.rs`:

```rust
/// [`clean_decimate`], and whether a decimated try was kept rather than the raw mesh.
pub fn clean_decimate_reporting(raw: &Solid, target: usize) -> (Solid, bool) {
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, 18.0, 35.0);
        if csg::self_crossings(&nets) == 0 {
            return (nets, true);
        }
    }
    (decimate(raw, raw.f.len(), 0.0, 0.0, 0.0, 180.0), false)
}
```

3. **A feature-preserving decimation constraint.** Fine grooves (pupils, nostrils) tear because the quadric collapse rounds their edges. A predicate hook would let a sculpt keep them, rejecting a collapse when any resulting face fails the predicate:

```rust
pub fn decimate_with(mesh: &Solid, target: usize, max_cost: f64, keep: &dyn Fn(P3, P3) -> bool) -> Solid
```

   `keep` takes a face's normal and centroid. It is not written. The same hook would serve a sand-pulled sculpt by keeping its drafts.

4. **`stock` template class with a stored part.** The class picker in `collection_templates.rs` (around line 619) could choose `painted` when the design carries a stored CAD mesh, as it does for embedded atlases, instead of each ring declaring it.

## Master enablers used

- `render::write_png_framed`, `render::yaw_facing` and `render::Framing` (#248), for the head close-up (`stones.png`) and the `GEKKO_SHOW` close-up checks.
- `StampTop::Pillow` (#248), for the chevron, the lamellae and the shoulder hide.
- `RowPath::ChartV` with `stamp_row`, for the shoulder rows.
- `cad::measure::thickness` and the census from #261, as the wall gate.
- `castability::judge_parts` (judging the joined parts).
- The `sculpt` toolkit (`tetra_mesh`, `relax`, `decimate`, `clean_decimate`, `settle`, `packed`), stored CAD parts, and `ImportedBase` with `SurfaceChart`.

Not used:
- `crisp_relief`, since the graph lift cannot carry it yet.
- Textura and marks.
- Patterns along a path (#258), since the stamps went in row by row.
- True stone plans, since the ring carries no stone.

## Commits (since the rethink)

- `4d35340`: round 1. Then `056176a`: its review (revise, 6.0).
- `d306448`: round 2, with master #261 merged and the census wall gate. Then `6f7c951`: its review (revise, 6.6).
- `482d33e`: round 3, with master `803a93e` merged. Then `564a840`: its review (cut, 6.7).
- This report, with the doc's status line, is the last commit.
