# Officina ring: Sigil — cloud report

**Result: shipped at 7.5** (round 3), every gate green at draft and at export, and the template gate passed.
Branch `claude/officina-sigil`. Example `crates/ringdesign-core/examples/officina_sigil.rs`; outputs in `showcase/officina/sigil/`.

(This file replaces, on this branch only, the Tenebrae enablers report that master carries at this path.)

## Verdicts

| Step | File | Verdict | Score |
|---|---|---|---|
| Block-out read test, attempt 1 | `read-test-1.json` | **reads: true**. The face read at once as a quartered seal in a bordure; the hero read only weakly ("clock hands"). | — |
| Round 1 | `review-round1.json` | revise | 5.8 |
| Round 2 | `review-round2.json` | revise | 7.2 |
| Round 3 | `review-round3.json` | **ship** | **7.5** |

Rounds used: the block-out (one attempt) plus 3 review rounds.

What each round fixed:
- **Round 1 → 2.**
  - The round quartered disc read as the BMW roundel, so it became a heater shield.
  - Bench cuts are now of even depth on the crowned table: 0.36–0.47 mm, where they had run 0.22–0.95 mm.
  - The stair-stepped stock shoulders now shade clean.
  - The matting punch was dropped.
- **Round 2 → 3.**
  - The bordure no longer shows a milled-coin serration.
  - The lesson is back to Logan's 7 features.
  - The timeline uses one camera and shows tool bodies translucent.
  - The escutcheon outline is closed.
  - A README discloses the seam bead and the render-only shading.

Round 3's three optional polish items were left alone, so the shipped files are exactly what was reviewed:
- P2: a reflection smear on the table crest in `face.png`.
- P2: the slab in timeline frame 6 is too opaque.
- P3: the sunk quarters could be about 10% darker in the hero.

## Gates (final build; `report.json` → `gates`, with the draft run in `draft`)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, degenerate faces | yes, 0 | yes, 0 |
| `csg::self_crossings`: ring / made part "Seal sunk" | 0 / 0 | 0 / 0 |
| Solids notes, parts notes | empty | empty |
| CAD features Ok | 7 / 7 | 7 / 7 |
| Bore clearance (closest vertex vs bore radius 9.100) | 9.0999995 | 9.0999995 |
| Field verdict at 192 × 128 and 256 × 128 (Delft) | Castable, Castable (0.0000%, worst −0.34°) | Castable, Castable |
| Ray release at 0.100 mm (obstructions, unresolved) | 0, 0 | 0, 0 |
| Ray release at 0.075 mm (obstructions, unresolved) | 0, 0 | 0, 0 |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed | 0 / 0 | 0 / 0 |
| Triangles (limit 2 M) | 492,932 | 1,363,746 |
| `--verify` cold reload | — | identical |
| Casting pattern: watertight, degenerate, crossings | — | yes, 0, 0 (1,376,256 triangles) |

Other numbers:
- Thinnest wall 1.37 mm.
- `dfm::cut_lands` at 0.8 mm is empty (informational; this is a sand ring).
- 10.98 g in 18k.

The release tool's status reads "Review" because of its standard low-draft-area note (bore walls included), not because of any obstruction.

## Template gate (run after round 3, on the shipped files)

`collection_templates officina … --only sigil --verify-export`, class `procedural`:
- **1 `design.set` patch** (`/manufacturing`), within the limit of 4.
- **39,456 bytes** against the 300 KB class budget.
- 21 nodes.
- Cold design reload, cold graph reload and source: identical.
- Mesh parity: vertices, faces and normals identical at 1536 × 448 (1,363,746 triangles).
- `template_gate_passed: true`.
- The graph lists the 7 features as `cad.feature` nodes in timeline order, on `base.preset`.

The numbers are in `showcase/officina/sigil/template-gate.json` and `report.json` → `template_gate`.

## Feature tree (the lesson)

1. **Stock 013** (`Band`). The factory stock is the band, and this feature is the anchor. The reader learns that CAD parts stand on imported stock as they do on a procedural band.
2. **Table plane** (work plane, tangent at 90°). It seats on the stock's table to 0.000 mm and 0.003° off radial; this was the spike the section asked for, and it passed. The reader learns that a work plane reads the surface as built.
3. **Seal** (sketch on 2). It holds a round bordure (two circles, so a region with a hole) and a heater shield. The shield's outline is one loop; a second loop runs round the raised cross and the two bright quarters. The reader learns that loops nest even-odd, so the sunk field is one region with the cross as its hole.
4. **Seal cutter** (extrude of two regions picked by `RegionRef`, 2 mm down). The reader learns that several picked regions extrude at once, as a tool body that has not yet cut.
5. **Crown** (sketch on its own plane, square to the ring). It is the table's crown across the finger, read off the stock and let down 0.42 mm. The reader learns why a flat cut fails on a crowned table: the table falls 0.68 mm at 4 mm.
6. **Crown let down** (a sweep of 5 round the ring). It makes a slab whose underside follows the crown, as a mesh. The reader learns that a sweep carries a section along a path.
7. **Seal sunk** (intersect of 4 and 6; cut, `Stage::Bench`). It sinks the seal 0.36–0.47 mm everywhere, with crisp walls and oxidised satin floors. The reader learns that a planar cutter intersected with a surface-following slab gives an even-depth cut, and that the two-stage model keeps the cut out of the pour.

## Parts and stones

- **No stones.** `stones.json` is empty, and `stones.png` is a close-up of the seal.
- **Made parts:** only "Seal sunk", a bench cut. "Seal cutter" and "Crown let down" are tool bodies that the intersect consumes.
- **Layer "Parting seam dressed".** A 0.5 × 0.004 mm round border on the crest line. Bare, the 013 sand master fields "Castable with care" (0.07% at −0.8° at 192 × 128). The cause is a mirror seam about 1 µm deep between θ 0° and 55°, and the bead fills it. It is disclosed in `report.json` notes and in `showcase/officina/sigil/README.md`.

## Process and decisions

- **Delft is not what `templates::stock` gives for 013.** `templates::stock(013)` opens 013 as native lost wax, because the core records its sand master as Marginal. TASK.md and the section ask for Delft. The example builds the Delft sand master itself, with the same steps as `stock_as(.., true)`, at an 18.2 mm bore, and dresses the seam as above.
- **The table is crowned across the finger** and flat round the ring. A planar extrude therefore cuts 0.22–0.95 mm deep, which round 1 failed. Hence features 5–7.
- **Renders:** the sunk seal is left oxidised satin, as Logan's own signets keep their recesses.
- **Render-only shading:** the stock's source mesh, and so `finished-metal.stl`, carries 0.55 mm facets that stand within microns of the true surface. On a polished shank they read as stepped highlights, so the renders diffuse the shank's shading normals over 0.6 mm, never across an edge sharper than 20°. On the table the build's own normals are kept, and the faces round the cuts use their own normals. The geometry and the gates are untouched; the README says so.
- **Enablers:** master did not move during the session; I fetched before every round and after the restart. So no enabler (C-B2, C-V1–C-V5, C-T5–C-T7) or render-edge change reached me, and none was used.
- **Session limit:** the session stopped at the account's limit after the round-2 review, and resumed with the checkout intact. The round-2 review was pushed first, and from then on every review was pushed as it landed.

## What I could not do

- **No edge bead on the seal.** A rolling-ball bead (`blend_mm` 0.03–0.06) folds or pinches at the shield's acute corners (its point and the cross's feet), which fails the empty-notes gate. The walls are crisp intaglio walls; reviewers accepted this.
- **No drafted walls.** A drafted prism of these regions does not tessellate closed (48–156 open edges). The kernel's analytic intersection with the polyline slab is refused (`NoClosedForm`, then `CutRefused`), so the slab is made as a mesh (a `Twist` with 0°) and the boolean runs through `csg`.
- **mesh.json's minimum angle is still 0.002°.** These are stock-mesh and csg slivers, which the gates allow; a remesh needs core. It is disclosed.
- **Not the section's own sketch topology.** The section's T-junction quarters, with every junction an endpoint, did not survive. Nested loops are refused as cells inside a hole, and sunk regions that share an edge give a non-manifold prism. The lesson teaches even-odd nesting and region picking instead.

## Core changes wanted (exact code; no `src/` file was edited)

1. **Make 013's sand master Delft-ready.** This is the workaround as a core default. In `core/templates.rs`:

```rust
pub fn stock_sand_ready(preset: &crate::imported_base::Preset) -> bool {
    matches!(preset.id, "002" | "006" | "013" | "015" | "017")
}
```

and in `stock_as`, after `SandProcess::DelftClay.apply(&mut d.draft);`:

```rust
if sand && preset.id == "013" {
    // The mirrored master leaves a ~1 µm seam on the crest line (θ 0–55°), which fields Marginal.
    let ctx = d.field_context();
    let seam = crate::field::BorderLayer { v_mm: ctx.crest_v_mm, width_mm: 0.5, height_mm: 0.004, mirror: false, ..Default::default() };
    d.layers.layers.push(crate::LayerEntry::new("Parting seam dressed", crate::field::Layer::Border(seam)));
}
```

Also drop `"013"` from `stock_process_note`'s first arm. The root cause sits in `imported_base/sand_master.rs`: the crest strip |z| < 0.125 mm lies 1.1 µm below its edges. It should be fixed there, by holding each section's outer radius non-increasing in |z| after the draft pass.

2. **Shading for imported stock in `render.rs`.** Promote the example's `diffused` as `pub fn diffused_normals(m: &Mesh, deg: f64, reach_mm: f64, step_mm: f64) -> Vec<Vec3>`. The body is in `officina_sigil.rs`: edge-gated Laplacian passes, `(reach/step)²` passes capped at 400. Apply it to an imported base's band in `render::finished` and in the viewports, so every stock ring stops banding its highlights.

3. **Normals at csg cut edges.** `parts::resolve` leaves the vertex normals on a cut's top edge averaged with the wall, about 45° off. Faces carrying those vertices without a `corner_normals` entry shade as a sawtooth. Give every band face touching a cut vertex its own corner normals:

```rust
// parts.rs, after the cut resolves, for each band face f with a vertex whose origin is a cut's:
m.corner_normals.push((f as u32, [n_face; 3]));   // then sort corner_normals by face
```

4. **`Sketch::profile_regions` should return the cells of a branched loop nested at odd depth**, or say why it drops them. Today a quartered shield drawn inside an outline groove quietly loses its quarters.
