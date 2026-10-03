# Core: CAD operations that stop failing on Gothic geometry

Branch `claude/core-cad-robust`, off master `8e5a59a`, with master merged in up to `60b3881` (crisp edges, Gothic clusters, frame timing, relief sculpt, true stone plans). One code commit, three merges and this report. The last merge's conflicts were in `twist.rs`, where master's closed and scaled sweep keeps its form and the cap's `fill` takes the label that `cad::draft` and `cad::turn` share, and in CLAUDE.md, where master's twisted-sweep text comes first and the new doctrine bullet follows it.

**The rule behind every fix:** each one is a fallback that runs only where the kernel failed. If the kernel already built a body, the core still uses that body, so no existing result moves. The core suite and golden test pass unchanged, and the graph and template tests are below. No new option, no serde field and no format change were needed, so nothing is fenced. `cadkernel` is not forked; every fix is in `cad.rs`, the new `cad/draft.rs` and `cad/turn.rs`, `cad/twist.rs` (two helpers made `pub(super)`) and `sketch/region.rs`.

## Per request

| # | Request | Status | What changed |
|---|---|---|---|
| 3 | Revolved arcs do not tessellate | **Fixed** | There were two separate defects. (a) "N nonmanifold edges": the kernel covers a revolved arc's torus seam with a zero-width strip, every triangle laid twice, once each way. `tessellate_traced` now removes opposite twin triangles wherever some edge has more than two faces (`cancel_twins`). The volume is unchanged and the body stays the kernel's. (b) "Kernel could not tessellate 1 faces": the kernel drops a torus face (on Ogiva's arch it is the comfort arc), and whether it does depends on the chord (on one arch it failed at 0.04 mm, passed at 0.015 and failed again at 0.012). Retrying cannot be relied on, so a revolve that has arcs and fails to tessellate becomes our own mesh (`cad::turn`). It is sampled at a quarter of the chord, with full and part turns, holes on full turns, and points on the axis shared. |
| 4 | Drafted extrusion refuses Béziers and inset-dropping outlines | **Fixed** | When the kernel's tapered extrude refuses a region with no holes, `cad::draft` builds it. The outline is walked to the chord, and the far end is a mitred inset that removes each edge as the wavefront collapses it. Each side face is planar. A draft that would carry a notch's root across the outline (a split event) is refused by name: "the draft closes the outline across a neck or notch". |
| 7 | Brep − Brep gives `NoClosedForm` (`CutRefused` here) | **Fixed** | When `brep::combine` fails, the Boolean goes through `csg` on the operands tessellated at the export chord, the same path a mesh operand already took. The 500-face refusal before the kernel stays as it was. |
| 2 | Loft through non-parallel sections has open seams | **Fixed** | The kernel splits a ruled face's straight edge where its planar neighbour leaves it whole, so the seam has T-junctions. `split_t_junctions` fans each open edge's triangle through the open corners lying on it, within 1e-6 mm. No vertex moves. |
| 6 | Loft only runs along the section normal | **Fixed, differently** | Measured: what decides success is the sections' winding relative to the direction the loft runs, not their order. On the probe's fanned planes, the order the report found working has (c1 − c0) · n0 **> 0**, so the literal rule ("reverse when > 0") would reverse the order that works. Instead, when the kernel refuses a polygon loft, `wound_sections` winds every section about the first-to-last centre line and lines each one up with the section before it. Either order now gives the same solid. |
| 1 | Loft winding read off the first corner | **Fixed** | The same `wound_sections` pass also starts every section together where the first and last sections are convex at their second corner. The kernel's `polygon_normal` reads that corner through the first fan triangle, so it is the one that matters, not the first corner itself. |
| 5 | Mirrored outlines in one sketch fail the drafted extrude | **Fixed** | Measured cause: the halves overlap or touch at the centre line, and the sketch refuses "loops may nest but not touch". The extrude itself does not fail. Now a **Sketch feature** whose loops meet extrudes each loop alone (straight, kernel-drafted or `cad::draft`) and joins the loops by `csg`. Inline profiles still refuse several loops, as their tests pin. Each loop is drafted on its own, so halves that only touch leave a draft groove along the line where they meet. Overlap them by at least the draft's inset (height × tan(draft)), as Ogiva's `FINIAL_OVERLAP_MM` did. |

Every fallback part is a mesh value, as `cad::twist` is. Fillet, press-pull and sketch-on-face refuse it by name: "a drafted extrusion's mesh", "a revolution's mesh", "a mesh of loops extruded and joined".

## Tests

New tests in `cad::robust_tests` and `cad::draft::tests`, each built from the report's kind of geometry:

- `a_revolved_sketch_arc_closes_and_holds_its_volume` (3a): a domed band section, closed at preview and export, volume within 0.4% / 0.15% of Pappus, and still the kernel's body.
- `a_pointed_arch_revolves_whole_and_in_part` (3b): Ogiva's arch (comfort arc, jambs, two head arcs). It asserts the kernel's own tessellation still fails, then checks the full turn against a fine-walked Pappus within 0.5% / 0.2%, and the half turn either way within 0.5% of half.
- `a_brep_cut_the_kernel_refuses_is_resolved_by_csg` (7): a revolved ring less a lancet niche. It asserts `brep::combine` still refuses, then checks the volume against the analytic ring less the niche's foot.
- `a_loft_through_fanned_sections_closes_listed_either_way` (2, 6): five fanned sections, both orders, preview and export, all closed with identical volumes.
- `a_loft_section_may_start_on_a_concave_corner` (1): asserts `brep::loft` still refuses, then checks the volume is exactly 10.
- `mirrored_loops_that_meet_extrude_together` (5): straight volume exactly the union's 9.0, drafted below it, closed.
- `a_bezier_outline_drafts`, `an_inset_that_drops_a_piece_drafts`, `a_draft_the_kernel_takes_is_still_its_body` (4): each asserts `brep::extrude_tapered` still refuses. Checked: the first-order draft volume, the mirrored run, the collapsed tip (5 far corners), the filled notch on the grown rectangle, the named refusal of a split, and that a draft the kernel accepts stays its body.

Reproduced on unmodified master first, with a scratch probe that was not committed: domed revolve "0 open and 16 nonmanifold edges"; arch "Kernel could not tessellate 1 faces"; ring − niche "CutRefused"; Bézier and notch drafts "unsupported or degenerate geometry"; fanned loft "68 open edges" in one order and "unsupported" in the other; overlapping halves "loops may nest but not touch". Tests that cannot fail on master by construction instead assert the kernel's own refusal in place.

Results:
- `cargo test -p ringdesign-core`: 848 passed, 0 failed, 16 ignored; golden 1 passed. After merging master up to `60b3881`: 912 passed, 0 failed, 17 ignored; golden and the other integration test both pass.
- `cargo test -p ringdesign-graph` (templates byte for byte, showcase, bestiarium, imported bases, cad edits): all passed before the merges (140) and after them (152, including the new `gothic_clusters`), 0 failed.

**Merge note.** Master (from `779a3d6`) brought its own seam repair, `zip_chord_seams`, which splits open edges at the other side's samples within the chord. The merged `tessellate_traced` runs master's `stitch_chord_gaps` and then `zip_chord_seams` exactly as master does. Only what those two leave open goes on to `cancel_twins` and `split_t_junctions`, followed by one more stitch. So whatever master's pass already closes is closed byte for byte as master closes it, and my passes see only what it cannot close: the doubled seams, and T-junctions it reverts.

## For a ring author

Draw what you mean and stop working around the kernel:

- **Revolves:** use real sketch arcs, not chord-walked polylines.
- **Drafted extrusions:** Béziers and sharp crocket tips are fine. A draft that would close a neck is refused by name; draft less or widen the neck.
- **Mirrored outlines:** they may share one Sketch feature. Overlap the halves by at least height × tan(draft), or a groove stays where they meet.
- **Lofts:** list fanned sections in either order, and start a section on any corner.
- **Booleans:** a Brep minus a Brep the kernel cannot close now resolves through csg.

Each of these comes back as a mesh rather than a kernel body only where the kernel failed. So fillet before the step that falls back, not after it, and check `Value::mesh_words` in any refusal you see.

What remains: a part turn of a section with holes that the kernel cannot tessellate is still refused. A revolve with arcs now pays one extra tessellation in `body_for`, about the cost of one tessellation, so up to about 0.6 s at export on a large arch.
