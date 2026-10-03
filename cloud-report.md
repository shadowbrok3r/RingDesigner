# Ogiva: the arch, rounds 1–3 — report

**Verdict: cut after round 3, at 6.2.** It reads at once as a Gothic pointed arch and every gate the brief sets is green at draft and at export. The art direction still failed it: three fresh reviewers called it stacked 2.5D extrusions with no seam beads, faceted crockets and a sparse head. The concept is sound and worth a fresh brief. The construction this platform gives a parts-only sand ring (planar sketches extruded along the pull, no beads) is what held it below 7.5.

- Example: `crates/ringdesign-core/examples/tenebrae_ogiva.rs`. Run `target/release/examples/tenebrae_ogiva [OUT_DIR] [--draft] [--verify]`.
- Outputs: `showcase/tenebrae/ogiva/`, holding the renders, `report.json`, `template-verification.json` and `review-round{1,2,3}.json`.
- The spike that chose the arch is `showcase/tenebrae/ogiva-spike/`, with `tenebrae_ogiva_spike.rs`.
- Doc: a dated note sits at the top of Ogiva's section in `docs/collections/tenebrae.md`. It says the ring is now the 2026-10-02 spike's `arch` option, pending Logan.
- Branch: `claude/tenebrae-ogiva-spike`, with master merged at 779a3d6 (PR #248 and later). The ring was rebuilt after the merge and every gate re-passed.

## Rounds

| Round | Score | Verdict | What changed | The reviewer's main objections |
|---|---|---|---|---|
| 1 | 5.8 | revise | Full detail on the spike's arch: keel rounded to a 0.82 mm straight land, then 37° flanks; two chamfered orders; sunk mouth; crockets and finial; capitals; pier lancets; pierced trefoil; drafted bore. | Flat round-lobe crockets; no seam beads; square palm slab; empty spandrel; worn view a band with teeth; face 32.3 mm tall; release "Review". |
| 2 | 6.2 | revise | Hooked leaf crockets with bosses; arch lowered 1.5 mm; larger trefoil; bigger lands; template compacted. | Same list, less acute: blend 0 on all 85 features; crocket and finial chords visible; box-on-box imposts; hairline pier lancets. |
| 3 | 6.2 | **cut** | Right half mirrored (crockets, capitals, pier niches) to fund detail inside the template budget; fleuron finial; round bosses; finer crocket pitch; cavetto bell under a chamfered abacus. | Changed "almost nothing a render can see". Blend still 0 on 101 features; crocket tiers stepped plates; small form nearly absent; the 0.075 study did not reach its pitch. |

Every round's reviewer passed the read: at 300 px, with no caption, the ring is "a crocketed Gothic pointed arch on two piers with a finial". The silhouette is unique in the collection. It stays distinct from Porta: one free-standing arch seen along the finger, with no doorway, jambs, tympanum or factory stock, where Porta is a portal on factory 009.

## Gates (round 3, draft 768×320 and export 1536×448; identical at both)

| Gate | Result |
|---|---|
| Watertight, degenerate, self-crossings | 0 boundary, 0 non-manifold, 0 degenerate, 0 crossings, one shell, 17,968 triangles |
| Parts and features | 101 features, all Ok; made part closed; solids and parts notes empty |
| Finger hole | 0 vertices inside; nearest 9.297 mm against r 9.3 |
| Ray release 0.100 mm | 0 obstructions, 0 unresolved (pitch 0.0998 × 0.0998) |
| Ray release 0.075 mm | 0 obstructions, 0 unresolved. **Pitch reached 0.075 × 0.084**: the grid is capped at 384 cells per axis and the face is 31.5 mm tall (see core changes) |
| Local wall (`cad::measure::thickness`, 0.8) | 383 rays, 0 below, min 0.818 mm. The 0.3 mm keel round is what clears it; the spike had 93 samples below |
| Sketch lands | all ≥ 0.8 mm section, ≥ 0.6 mm sand web |
| DFM / stones / pattern | 0 findings; 0 stones = 0 previewed; pattern watertight |
| Cold reload | identical (vertices, faces, normals) |
| Field verdict | **Does not apply.** A parts-only ring has no band chart, so it reads "Castable with care" with "CAD solids require mesh-space manufacturing inspection". It is judged by ray release at both pitches and the local wall instead, and `report.json` records that under `field.why` |
| Draft clamp | n/a: no painted relief |
| Template gate | 107 nodes, 0 controls, **1** `design.set` patch (`/manufacturing`), 291,657 of 300,000 bytes, source-identical, 17,968 identical triangles |

Both release studies return status **Review**, never Blocked. The two warnings behind it are outside the brief's gate, and `report.json` explains them under `release_status_note`:
1. **Low-draft area (735 mm²).** It comes from the 0.82 mm, 0° straight belts across the parting line, plus 3.0° walls sitting exactly on the threshold. Belts are what keep the halves from leaving a hair lip (0.016 mm in `thickness`) or a waist the rays read as undercut.
2. **Sand slots under 0.6 mm (28 at 0.100, 31 at 0.075, the narrowest 0.07–0.10).** The scan walks axis-aligned lines, so every re-entrant corner on the curved extrados reads as a narrowing slot near its tip: crocket stalk to keel, trefoil cusps, the order's chamfer roots. None is a closed slot of sand.

**Format:** the design writes at **format 5**. Ogiva uses neither PR #248 opt-in. It has no height-field relief, so `crisp_relief` is not set. Its leaves are CAD parts, not stamps, so `StampTop::Pillow` is not used. Close-ups go through `render::render_parts_framed`, the same framing `write_png_framed` uses, so the reviewers saw true crease normals, not a cropped mesh.

## The ring, as built

- **The arch.** An equilateral pointed arch seen along the finger: half-span 12.4 mm, radius 24.8, springers at y −5, apex at y 16.5, sill at y −11. Face 28.1 × 31.5 mm, 7.0 mm along the finger, 12.7 g of silver 925, US 8.6 (18.6 mm bore). Centre of mass 0.54 mm off the bore axis.
- **The keel.** The outline sits on the parting plane as a 0.82 mm straight land, inset 0.15 mm. From it, 37° flanks fall 0.6 mm in, giving a 106° crease rounded by the land. That rounding cleared the thin-wall samples.
- **The face.** The outline is inset 0.6 mm and raised in drafted halves, starting inside the land.
- **The orders and mouth.**
  - The order is sunk 1.45 mm with a 35° chamfer and runs down onto the capitals.
  - The mouth is a crescent inside the head, kept 0.9 mm off the bore and sunk 1.6 mm, with squared ends where it narrows to 0.7 mm.
- **Crockets and finial.** Three hooked leaves a side up the extrados, each with a round boss, cored and belted across the parting line. They are built on the right and mirrored about the crown plane. A symmetric fleuron finial sits at the apex.
- **Capitals.** A cavetto bell under a chamfered abacus at each springer, standing 0.5 and 0.3 mm proud, mirrored.
- **Pier niches.** Sunk 0.7 mm, one per pier face, mirrored.
- **Trefoil and bore.**
  - A trefoil is pierced through the mouth.
  - The bore is a belted double cone, drafted 3° from the parting line, with its radius corrected so the belt never enters the finger hole.
- **The pour.** Everything is a sketch on a parting-parallel plane, extruded along the pull at 3°, with the drag half made by `Mirror{Band}`. That is why release is 0/0 by construction.

## What could not be done, and why

- **Seam beads (open on every round, house rule 10).** `blend_mm` acts only where a part joins the Band. `parts.rs:343` gates the bead pass on `cuts.iter().any(|c| c.blend_mm > 0.0)` against the band chain. A parts-only ring has no band, so no union in its tree can carry a bead.
- **Smooth leaf curves.** Drafted true arcs, and lofts through polylines, tessellate with open seams at some resolutions. A kernel union of two B-rep lofts returns `NoClosedForm`. The leaves therefore stay polylines, with chords visible at 1600 px.
- **Tracery, arcading and a moulded keel.** Each sketch point costs about 150 bytes of pretty-printed template JSON. At 291,657 of 300,000 bytes, after snapping, coarser pitches and mirroring, the head tracery, pier arcading and keel moulding the reviewers asked for do not fit.
- **The 0.075 pitch.** The release grid stops at 384 cells, so on a 31.5 mm face the y pitch is 0.084.
- **Rounded drafted corners.** `extrude_tapered` refuses any loop where the inset swallows a short segment, such as a sharp convex corner next to short chords, or a cusp. Pocket outlines needed legs below the springers and squared crescent ends.

## Core changes wanted (not made; exact code)

1. **Let a parts-only ring carry seam beads.** In `parts.rs`, when the document has no `Band` feature, bead every seam that a Boolean Union traces between two parts, using the larger `blend_mm` of the two. That is the same rule the joined-cluster seams already use:
   ```rust
   // parts.rs, after the union chain of a band-less document is built
   if !doc.has_band() {
       for (a, b, traced) in &union_traces {
           let r = a.blend_mm.max(b.blend_mm);
           if r > 0.0 {
               for seam in blend::seams(traced, &a.solid, &b.solid, true) {
                   match blend::bead_seam(traced, &seam, r, ctx.cancel) {
                       Some(Ok(bead)) => chain.combine(&bead.solid, Op::Union)?,
                       Some(Err(e)) => out.notes.push(format!("{} / {}: seam bead failed ({e})", a.name, b.name)),
                       None => {}
                   }
               }
           }
       }
   }
   ```
2. **Release grid: reach the asked pitch.** `manufacturing/release.rs:309-314` clamps each axis to 384 cells. Raise the cap to what the pitch asks, bounded by a cell budget rather than a per-axis cap:
   ```rust
   const MAX_RELEASE_CELLS: f64 = 1024.0 * 1024.0;
   let want = |span: f64| (span / setup.sample_pitch_mm).ceil().max(4.0);
   let (mut fx, mut fy) = (want(hi[0] - lo[0]), want(hi[1] - lo[1]));
   let k = (MAX_RELEASE_CELLS / (fx * fy)).sqrt().min(1.0);
   fx = (fx * k).floor().max(4.0);
   fy = (fy * k).floor().max(4.0);
   let (nx, ny) = (fx as usize, fy as usize);
   ```
   The note at line 590 then reads `"Grid capped at {MAX_RELEASE_CELLS} cells; …"`.
3. **Sand slots: measure across the gap, not along an axis.** At `release.rs:500-528`, a slot found on an axis scanline is the chord of a re-entrant corner. Confirm it against the slot's width perpendicular to its own walls, and drop it when the gap opens past `min_sand_web_mm` within its own depth:
   ```rust
   let true_width = width * (1.0 - (other_axis_gradient(col, z)).powi(2)).sqrt(); // walls' own normal
   let opens = gap_opens_within(&out.columns, idx, a, step, setup.recipe.min_sand_web_mm, depth_at(col, z));
   if true_width < setup.recipe.min_sand_web_mm && !opens { /* push SandFinding */ }
   ```
   Do the same for a 0° belt that is the parting line's own land: exclude it from `low_draft_area_mm2` when it straddles z = 0 within `2 * belt`.
4. **Drafted arcs and polyline lofts that close.** `cadkernel::brep::extrude_tapered` on `Geometry::Arc` loops, and `Operation::Loft` through polylines, should share edge tessellation between adjacent faces, so the tessellated shell is closed. That would let leaves be true curves.
5. **`extrude_tapered` that drops a consumed segment instead of refusing.** When the inset makes a segment's offset reverse, remove that vertex and re-intersect its neighbours (a standard polygon-offset cleanup), rather than returning an error.
6. **Compact template JSON for sketch points.** Write `Sketch` points as a flat `[x, y, …]` array in graph files, rather than pretty-printed objects, behind the graph format ladder. That would roughly triple the detail a procedural template can hold.

## Recommendation

**Cut Ogiva as it stands. Keep the arch as the concept.** It is the only Ogiva that ever read, and it read every round. What failed is surface quality, which this construction cannot reach on a parts-only sand ring without core changes 1 and 4. With those, the round-2/3 punch list is straightforward:
- 0.2–0.3 mm beads on every join;
- curved leaf crockets and a fleuron;
- cusped head tracery;
- blind arcading on the piers;
- a moulded keel.

The other route is to rebrief it on a procedural band, where `blend_mm` already works, carrying the arch in the outline the band's own silhouette makes.
