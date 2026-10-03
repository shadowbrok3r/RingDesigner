# Officina ring: Aile (`aile`): cloud report

**Result: CUT at round 3, score 6.5.** One block-out attempt and all three full rounds were used. Aile reads at 300 px from the first read test onward ("a winged stone in a second"). It was cut on workmanship: the tips, the knobs beside the collet, the palm view, a collet foot that can't be seen, and the sculpted wing still being a baked mesh. The round-3 reviewer also failed the 0.8 mm thickness gate on the wing parts (see Gates).

Branch `claude/officina-aile`, from `master` at `8e5a59a`. Master was merged at the start of round 2 (`3566cc6`: C-V1 to C-V4, C-B1, crisp edges #248) and again at the start of round 3 (`60b3881`: C-B2, C-V3/C-T7, core relief sculpt #250). Code: `crates/ringdesign-core/examples/officina_aile.rs`. Outputs: `showcase/officina/aile/`.

| Commit | What |
|---|---|
| `7c2f8f5` | Block-out, read test 1 |
| `2ed73e7` / `a840c39` | Round 1 build / review (revise, 6.0) |
| `b901b87` / `156098e` | Round 2 build / review (revise, 6.5) |
| `907333c` / `dfc5d46` | Round 3 build / review (cut, 6.5) |

## Read tests and reviews

| Step | Verdict | Score | The reviewer's headline |
|---|---|---|---|
| Read test 1 (block-out) | **reads: true** | – | "A blue oval in a plain collet with a pair of wings spread flat to either side, like a pilot's badge or a winged insignia. You name 'winged stone' in a second." It asked for wings that clasp the collet, fanned and pointed feathers, and ridged tops. |
| Round 1 | revise | 6.0 | It reads, and every gate is green. Blunt faceted tips, straight tangent wings, a plain collet, knobbed clasp ends. The lesson was gone (stored meshes, no Plane, sketch or extrude). |
| Round 2 | revise | 6.5 | The lesson was half restored (Plane, the Bézier "Wing" sketch and three tiers came back), and the wings now follow the band. The sculpt was still not driven by the sketch, the clasp knobs and chisel tips remained, a palm-view star appeared, and the foot could not be seen. |
| Round 3 | **cut** | 6.5 | The sculpt is now tied to the tiers and its tips land on the sketch's tips. Still open: knobs with spurs beside the collet, a stepped "collar and capsule" at each tip, the palm star, and a foot frame that adds nothing visible. The thickness gate regressed on the wing parts. |

Rounds used: 3 of 3, plus 1 block-out attempt.

## Gates (round 3, the final state)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings, 1 shell | yes | yes |
| Self-crossings on each made part (oval, collet, collet foot, east wing, west wing) | all 0 | all 0 |
| Solids and parts notes empty | yes | yes |
| Every CAD feature Ok (11) | yes | yes |
| Nothing in the finger hole: vertices inside / nearest vertex (bore 9.1) | 0 / 9.0999996 | 0 / 9.0999995 |
| Lost wax: `judged_field_report` verdict / thinnest wall | Castable / 1.18 mm | Castable / 1.18 mm |
| Lost wax: `dfm::cut_lands` at 0.8 | clean | clean |
| Thickness, whole ring at 320 × 112 (384 samples, same sampler as `cad::measure::thickness`) | 7 below 0.8: the collet lip (lowest **0.389 mm**) and feather tips (judged at the section's 0.5 mm) | same |
| Thickness, each wing part measured alone | **10 samples below 0.8 off the lip or tips**, down to 0.010 mm | **7**, same zone |
| `dfm::findings_in` | 0 | 0 |
| Stones reported = previewed, warnings, crowding; culet over the crest | 1 = 1, none, none; 0.22 mm | same |
| Casting pattern (`try_build_pattern`) watertight / degenerate / crossings | yes / 0 / 0 | yes / 0 / 0 |
| Triangles (budget 2 M) | 497,842 | 1,290,118 |
| `--verify` cold reload with an empty library | – | identical |

**The thickness gate, stated plainly.** My report marked it passed because I gated only the whole ring. A joined stored part's fillet collar runs into the band, so I judged the ring as cast and reported the parts for information. The round-3 reviewer did not accept that, on two counts:
- The wing parts alone show unexcepted samples down to 0.01 mm at θ 44 to 53°, on the root skirt where the underside dips into the band; round 2 had 0 there.
- The collet lip samples 0.389 mm, under the 0.5 mm lost-wax minimum section.

The reviewer's gate verdict is fail, and it stands.

## Template gate (after the last round)

| | Value |
|---|---|
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **297,286 bytes**, procedural budget 300,000 (0.9 % margin) |
| Cold design reload / cold graph reload / source parity / vertices, faces and normals identical | true / true / true / true |
| Nodes | 20, of which 11 are `cad.feature` |
| `template_gate_passed` | true |

**Ordering finding:** the graph lists its `cad.feature` nodes in id order: Band, Oval, Collet, Wing plane, Wing, Tier I to III, Sculpt, West wing, **Collet foot**. The timeline shows Collet foot fourth, because I appended it with id 11. The rule "the graph must list the features in the timeline's order" is therefore not met. The fix on the ring side is to give the foot id 4 and shift the others, which I did not do after the verdict.

## Master moves and enablers

- **Merged at each round:** round 2 at `3566cc6`, round 3 at `60b3881`.
- **Used:**
  - Crisp edges (#248): close-ups through `render::write_png_framed` / `render_parts_framed`, which also frame every timeline step on one fixed point.
  - C-V1 only as a compile fix. `Placement::Ring` gained `level`, so the seat is now built from `Placement::ring` with its spin set.
  - `Component::fillet_into_band` from the core relief-sculpt merge (#250, not one of the named enablers): a 0.35 mm fillet grown out of the band on the sculpted wing, its mirror and the collet foot. Both rounds had asked for root fillets, and the builder fillet (`blend_mm`) folds at every radius here.
- **Not used:**
  - C-B2: Aile's oval was already a true plan.
  - C-V2, C-V4: C-V4's beads apply only to rings made of parts alone.
  - C-V3 / C-T7: a twisted sweep through points under `LEAF_LAW` would make a native cambered feather, but it cannot carry the barbs that make it read as a feather.
  - C-V5, C-T5, C-T6: not seen on master by the round-3 merge.

These opt-ins write the design at format 6.

## The feature tree (11 features, in timeline order)

1. **Band**: a DShape 2.4 × 1.8 with a reverse taper of 0.4 and an 18.2 mm bore. The reader learns that the lesson starts from a plain procedural shank that narrows to the top.
2. **Oval 7 × 5**: the stone, north-south, standing 0.9 mm over the bezel's own height. The reader learns to place the stone first and build round it.
3. **Collet** (`head.bezel`, wall 0.8): the made setting, with a lip burnished over the crown.
4. **Collet foot**: a sculpted cup that carries the collet's wall down into the crown. It tapers in 0.3 mm, rounds under, and grows out of the band with a 0.35 mm fillet. It shows that a flat-based collet on a domed band needs a foot. The frame barely shows it, which was a review fault.
5. **Wing plane**: a work plane square to the band 5 mm round the crest, sunk 1 mm. It is drawn as wire in the timeline.
6. **Wing**: one sketch on that plane. An outer contour of Béziers runs from the root at the collet round three tips, and two inner Béziers from the notches to the root part it into three regions. The reader learns regions from T-junctions. It is drawn as wire.
7. to 9. **Tier I, II, III**: each region extruded off the plane (1.8 / 1.6 / 1.4 mm). These are the flat block-out tiers, and the reader sees the wing's plan stand up.
10. **Sculpt the east wing**: a stored mesh that reads and replaces tiers 6 to 8, standing in tier I's frame. It carries the digest of the tiers it was sculpted over, so editing the sketch marks it stale. It holds the clasp and three primaries, united, and grows into the band with a 0.35 mm fillet. Each feather has:
    - a cambered superellipse vane, 3.6 mm wide at the root;
    - a raised rachis and a leading vane narrower than the trailing one;
    - V-cut barbs at 0.46 mm pitch that lean toward the tip at 1.6 (leading vane) or 1.25 (trailing vane) mm per mm out from the rachis and fade over the first 2 mm;
    - a roll of 16°, so each inner feather laps over the next;
    - a spine that rides the band for half its length, then lifts up to 1.5 mm, and runs to an ogive point with a 0.3 mm round.

    The reader learns to block out with sketch regions, then sculpt over them.
11. **West wing (mirror)**: the sculpt mirrored across the section at 90°. The reader learns to mirror round the ring.

**Parts and stones.**
- **Stone:** one blue sapphire, oval 7 × 5, 0.65 ct, north-south, in a `head.bezel` collet. A bezel was chosen because it is the section's setting and it protects a long oval on a slim shank.
- **Seat:** no `seat.bur` is cut. On this 1.6 mm crown, a full bur leaves 0.54 mm under the culet, and the culet relief it cuts for a raised stone skims the dome to a 0.01 to 0.2 mm skin. The stone is raised 0.9 mm instead, its culet clears the crest by 0.22 mm, and the collet's bearing ledge seats it.
- **Weight:** 6.7 g in 18k.

## What I could not do

- **Fillets with the builder.** `blend_mm` folds at every radius from 0.12 to 0.5 mm, whether laid on the wing, on the collet, or on one united head. The fold is where the 8.6 mm collet straddles the 2.4 mm band's edge round and its flat base grazes the domed crown. `fillet_into_band` (round 3) solved it for stored parts only.
- **A wing driven by the sketch.** The sculpt is a stored mesh, so the sketch cannot move the finished feathers; it only marks the sculpt stale. A native sweep or loft could not carry the barbs within the 300 KB template budget. The stored barbs are cut on chevron-bent mesh rows to keep about 7 k triangles per feather.
- **Clean tips and clasp.** The reviewer still saw a stepped "collar and capsule" at the tips and knobs with spurs beside the collet. The tips are capped at 22 rings by the size budget.
- **A clean palm view.** The draft-resolution band facets still make a sawtooth against the foot.
- **Thickness on the wing parts alone and on the collet lip.** Both are open, as stated above.

## Core changes wanted (exact code where I could find the site)

1. **Rollback shows features a later feature consumes.**
    - The problem: `Document::through` builds with the outputs the *whole* history left. When rolled back to tier I, the tier does not show, because the sculpt consumes it.
    - What I did: worked around it in the timeline by re-appending a prefix document.
    - Proposed, in `crates/ringdesign-core/src/cad.rs`:
      ```rust
      impl Document {
          /// The outputs the history up to and including `through` leaves, as the rollback marker builds it.
          pub fn outputs_through(&self) -> Vec<Id> {
              let mut prefix = Document::default();
              for f in &self.features {
                  let _ = prefix.append(f.clone());
                  if Some(f.id) == self.through {
                      break;
                  }
              }
              prefix.outputs
          }
      }
      ```
      Read `doc.outputs_through()` in place of `doc.outputs` wherever the build or `attachments()` runs with `through` set.
2. **A bezel that sinks into the band.**
    - The problem: `head.bezel` fixes its sink at `BEZEL_SINK_MM`, so a long collet on a domed crown grazes it, and no fillet lays.
    - Proposed, in `crates/ringdesign-core/src/cad/builders.rs`:
      ```rust
      // params, beside "wall_mm" and "lip" (line ~200):
      number("sink_mm", "Sink", "mm", 0.0, 1.5, BEZEL_SINK_MM),
      // where the collet is made (lines ~834-835):
      Some(w) => walled_collet(gem, v.f("wall_mm"), v.f("lip"), metal - v.f("sink_mm"), w).map_err(|e| refused(who, e))?,
      None => setting::collet_named(gem, v.f("wall_mm"), v.f("lip"), metal - v.f("sink_mm")),
      ```
3. **No culet relief when the culet clears the metal.**
    - The problem: `seat.bur` on a raised stone cuts a relief that skims a domed crown to a skin.
    - Proposed, in `builders.rs` `BUR` (line ~841), before `relief_named`:
      ```rust
      if fit.surface_z < -(setting::girdle_half_mm(gem) + gem.pavilion_mm() + 0.1) {
          bail!("{who}: the culet stands clear of the metal; no seat cut is needed");
      }
      ```
4. **Extrusion draft on Bézier regions.** `Extrude { draft_deg: 10 }` on a Bézier-bounded region fails with "Extrusion: unsupported or degenerate geometry", so Aile's tiers use 0°. This is a kernel change; I don't have exact code.
5. **Bézier region extrusions tessellate open at fine chords.**
    - The problem: at 512 × 192 and finer, tiers I and II carry "Solid tessellation has 12 open … edges". Aile's timeline therefore builds at 320 × 112, and the finished ring never builds the tiers.
    - To reproduce: the `Wing` sketch in `design.ring.json`, at a 768 × 320 build with the history up to feature 7.
6. **The template lift keeps document order.** `collection_templates` writes `cad.feature` nodes in id order, not in `doc.features` order (see Ordering finding above).
7. **Stored-mesh quantization can make crossings.**
    - The problem: `stored::Packed::encode` snaps to 1e-5 mm, and two unions clean before packing came back with 1 self-crossing (section counts 34 at round 2 and round 3). I picked counts that pack clean.
    - Proposed: `encode` could re-check `csg::self_crossings` on the snapped positions and refuse with the site.
