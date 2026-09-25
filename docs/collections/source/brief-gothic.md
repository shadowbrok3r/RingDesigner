# Tenebrae — the Gothic cathedral collection (design brief)

**Eight rings, one building each.** Tracery, rose windows, lancet and ogee arches, flying buttresses, pinnacles, a star vault, a reliquary and a portal, all modelled as CAD feature histories on factory stock or a band. The stones are stained glass: calibrated coloured stones in bezels and baskets, most set *à jour* so light passes through them the way it passes through a window.

| | |
|---|---|
| Collection | **Tenebrae**: the office of shadows, in which the candles go out one by one. Alternative name: *Opus Francigenum*, the medieval name for Gothic. |
| Primary side of the app | **CAD modelling.** Sketches with regions on work planes and faces, extrude and negative-extrude cuts, press-pull, revolve, loft, the twisted sweep, ring, About and mirror patterns, the builders (`head.basket`, `head.bezel`, `seat.bur`, `cutter.pierce`, `cutter.azure`, `shank.cathedral`), Separate parts, bench stages, seam beads, and one OpenCascade fillet. |
| Process mix | 5 lost wax (Rosa, Arcus, Lanterna, Capsa, Porta). 3 two-part sand (Oculus, Ogiva, Sigillum), two of them with honest bench work. |
| Sheet subtitle | `FOUR SIGNETS · TWO BANDS · A SOLITAIRE · A RELIQUARY · TWENTY-SIX STONES OF GLASS` |
| Nominal bore | 18.6 mm (r = 9.3), the same as Reptilia. Renders in studio gold; sand rings are shown finished and exported as the pattern. |

## The eight at a glance

| # | Ring | One line | Base | Process | Stones | CAD lesson | Diff. / risk |
|---|---|---|---|---|---|---|---|
| 1 | **Rosa** | The west rose: oculus ruby, eight sapphire lights, pierced tracery | Factory **001** Cushion, face 18 × 18 | Lost wax | 9 | A net of circles becomes cells, then lights; one cut pierces them all; the stones are arrayed about the ruby | 4 / medium |
| 2 | **Oculus** | The side face is a wheel window and the finger is its eye | Procedural Flat 7.0 × 4.0 | Sand (Delft) | 0 | A sketch on a side-face plane centred on the finger axis; cuts along the pull, drafted from the parting plane | 2 / low |
| 3 | **Ogiva** | A ring whose cross-section is a pointed arch, with crockets on its keel | CAD-only: one revolved section | Sand (Delft) | 0 | A constrained sketch revolved into a whole ring; fins drawn in the parting plane | 3 / medium |
| 4 | **Arcus** | A sapphire in a basket, held by flying buttresses that spring from pinnacled piers | Procedural Flat 3.2 × 2.3, Cathedral shank | Lost wax | 1 | Builders and hand-built parts meet at one number (`spread_deg`); loft, sketch on a face, a gargoyle from SVG | 4 / med-high |
| 5 | **Sigillum** | A chapter seal: the intaglio is cut at the bench and the arcade is cast | Factory **004** Shield | Sand (Delft) | 0 | Bench-stage cuts with draft, a mirrored seal sketch, finished ring versus pattern | 3 / medium |
| 6 | **Lanterna** | Ely's octagon: a star vault, eight lamp-lit pinnacles and an amethyst boss | Factory **015** Octagon | Lost wax | 9 | Twisted sweep, revolve, press-pull, a stone on a part's face, and an About array of a whole assembly | 5 / high |
| 7 | **Capsa** | A reliquary chasse (coffin-ring convention) with a hinged lid and a skull inside | Procedural Flat 4.6 × 2.3, Cathedral shank | Lost wax | 6 cabochons | Native shell, press-pull, kernel cuts kept B-rep, a Separate lid, a joint, OpenCascade as the last step | 4 / med-high |
| 8 | **Porta** | A splayed portal: stepped archivolts, a ruby in the tympanum, an ogee hood | Factory **002** Kite | Lost wax | 1 | Offsets as architecture: a depth ladder, a sweep along an arch, Bezier ogees, a pattern along a curve | 3 / medium |

Stained-glass palette (`Gem::preview_tint`, gem.rs:234): Chartres blue (sapphire), ruby red, amethyst violet, citrine gold, garnet. Stone total: 9 + 1 + 9 + 6 + 1 = **26**.

---

## House rules for all eight

1. **The CAD form of the side-face guarantee.** Draw on the parting plane or on a side face, extrude along the pull, and draft each half from z = 0; the result casts by construction. A sketch extruded along a surface normal off the crown locks and belongs to the bench. The evidence:
   - side-face piercing measured 0.000 mm² and staged Cast; crown piercing locks 8.63 mm² at −88° (PLAN.md:720);
   - `cutters::cut_stage` stages Cast within `ALONG_PULL_DEG` = 10° of the pull (cutters.rs:24, :1067);
   - CLAUDE.md, *Side faces are where ornament goes*.

   The rule is what makes three sand rings possible in a CAD-first collection.
2. **Model at the origin and seat once.** Every part is built at the origin in the part frame (x along the finger, y round the ring, z out of the metal; cad.rs:452-483) and seated with `Placement::Ring`, which follows a resize (cad.rs:286). World-coordinate parts do not follow a resize (docs/WORKSHOP-CAD.md:30). Sketches lie only on work planes that read the built band: `PlaneBase::{Tangent, Section, Parting, Face}` (pattern.rs:70-83, :521-572).
3. **Keep a part a kernel body while you still need its faces.** Sketch-on-face, `FaceSeat` stones, press-pull and native fillet refuse meshes (cad.rs:1586-1597, pattern.rs:587-590). Builders, patterns, the twisted sweep, csg booleans and stored meshes all produce meshes. Build faces first and do mesh operations last, or leave the union to the band join, where Join parts are clustered (parts.rs:17).
4. **Stained glass is set *à jour*.** Every faceted stone gets `seat.bur {through: true}`, a pilot to open air past the bore (builders.rs:163), so light passes through the stone. The tint needs enabler E8.
5. **Bars are measured by the author, not the verdict.** Tracery bars are at least 0.8 mm, the investment recipe's section (stock_masterworks.rs:482) and Delft's section (castability.rs:180). The verdict is radial and cannot see a tangential bar or an axial web (CLAUDE.md, *Two things the model does not yet know*). Each author script asserts bar width from the sketch offsets until E11 exists.
6. **Split before regions.** Curves that cross are refused (region.rs:133-137). Draw overlapping, run `split_at_intersections` (sketch/edit.rs:192), and let T-junctions close the cells (sketch/graph.rs:1-3).
7. **No coincident faces.** A floor or exit face that lies in a metal face is a degenerate boolean (CLAUDE.md, *Booleans are exact in topology*; the crescent-cutter lesson). Through-cuts overshoot the far face by 0.3 mm. Drafted halves overlap 0.05 mm across z = 0. `CUT_CLEAR_MM` handles the entry side (cad.rs:2025).
8. **OpenCascade is used once.** It appears only as the last feature of Capsa's lid. `kernel-occt` is off by default and a no-go for the Windows release (PLAN.md:730). A stored mesh goes stale on upstream edits and forces format 6 (stored.rs:30; `library::format_version_for`). Everything else is native.
9. **The feature tree is the lesson.** Feature names are sentences ("Pierce the eight spandrel lights to the bore"), and the CAD workspace's rollback (`Document::through`, cad.rs:1290) steps through them.

---

## 1. Rosa — the west rose

**Concept.** The rose window of a west front on a cushion signet. A ruby oculus sits in a revolved roll moulding, surrounded by eight sapphire pears radiating like lancet lights, with pierced spandrels between them. Pierced mouchettes fill the four corners of the square.

| Zone | What is there |
|---|---|
| Face (table) | The rose, Ø 15.2 inside a rim moulding. Oculus: Round 5.0 ruby in a bezel. Ring of 8 Pear 4.5 × 2.8 sapphires in bezels, points outward. 8 pierced trefoil spandrel lights between the pears. 4 corners with pierced mouchette pairs (8 lights). |
| Walls (cushion's 4) | "Gallery of kings": a blind three-lancet arcade struck into each cheek, recessed 0.35. |
| Shoulders | 4 oculi each side, graded 1.6 → 0.9 mm, dimming toward the palm: the rose's light fading. |
| Palm | Plain, polished. |
| Bore | The à jour pilots and through-lights open here; all edges are broken at the bench (0.2 mm). |

**Base.** `ImportedBase::attach` with PRESETS 001 (imported_base.rs:472, :1095), face resized to 18 × 18 and `sand_envelope = false`, as Nocturne and Vesper are (stock_masterworks.rs:415-440). **Process:** lost wax. The through-lights, proud bezels and crown piercing all lock in sand (PLAN.md:720). **Alloy:** Gold 18k.

**Feature timeline.** About 44 features with E2, about 71 without.
1. *Procedural shank*: `Operation::Band` (cad.rs:34). The 001 stock is the band, and parts resolve onto it through csg (mesh.rs:387-396).
2. *Table*: `Plane { base: Tangent { theta_deg: 90, across_mm: 0 } }` (cad.rs:125; pattern.rs:539-548). Sketch x runs round the ring and y up the finger.
3. *Rose net*: `Sketch` on the Table (`Workplane.on_face = FaceAnchor { feature: 2 }`; sketch.rs:34-49; cad.rs:1596 work-plane branch).
   - Draw circles at r 3.0 and r 6.8, one radial mullion line, one pear cell (two arcs), and one spandrel light: a trefoil head of three arcs over two lines.
   - Run `Sketch::pattern(ids, &Pattern::Polar { centre: [0,0], count: 8, sweep_deg: 360 })` (edit.rs:45, :755).
   - Draw one corner mouchette pair and run Polar ×4.
   - `split_at_intersections()` (edit.rs:192) gives cells (region.rs:138).
   - `offset(cell, −0.4)` on each light cell (edit.rs:433) gives 0.8 mm bars.
   - Mark the net and pear cells construction.
4. *Pierce the lights to the bore*: `Extrude { sketch: Profile::Feature { feature: 3 }, height_mm: −7.0, draft_deg: 0 }` with `Attach::Cut` (cad.rs:59, :1809-1830). The table stands about 1.75 mm over the bore at the centre and about 5.5 mm at the corners.
5. *Oculus moulding*: an inline sketch in the part's x–z plane (a 0.35 half-round at 3.25 from the axis plus a 0.2 cove). Revolve it with `Revolve { pivot: [0;3], axis: [0,0,1], degrees: 360 }` (cad.rs:66, :1831). Seat with `Placement::ring(90, 0)`, Join, `blend_mm` 0.15 (parts.rs:1-6).
6. *Rim moulding*: the same revolve at r 6.8–7.6, Join, blend 0.2.
7. *Oculus stone*: `builders::stone_feature(id, Round 5.0 ruby, Placement::ring(90, stand_off_mm("bezel", gem)))` (builders.rs:916, :487).
8. *Bezel*: `feature_on(.., "head.bezel", 7, {wall_mm: 0.45, lip: 0.3})` (builders.rs:19, :928, schema :147-195).
9. *Seat bur*: `{through: true}`.
10. *First light*: stone Pear 4.5 × 2.8 sapphire at `Placement::Ring { theta_deg: 90 − 28.6 (5.2 mm round the table), spin_deg: 90 }`, point outward. Add its bezel (wall 0.4, lip 0.3) and bur (through).
11. *Eight lights*: `Pattern { sources: [10, bezel, bur] (E2), kind: About { part: 7, count: 8, span_deg: 360 } }` (pattern.rs:37-44). These are world turns about the ruby's axis (pattern.rs:245, :274), and they stay on the table because the table is flat.
12. *Oculi of the nave*: `cutter.pierce` at θ 90 − (55, 67, 79, 91), sized by `cutters::pierce_at` (cutters.rs:1099) and overridden to 1.6/1.4/1.2/0.9. Mirror with `Pattern { Mirror { Section { theta_deg: 90 } } }` (pattern.rs:61-66, :263-270). The shape is Round now and Quatrefoil with E7.

**Stamps (the walls are stock mesh, not kernel faces).** Two `setting::Stamp` on the ±z cheeks (setting.rs:897): a three-lancet-on-sill outline of at most 512 points (setting.rs:928), `cut: true`, `sink_mm: 0.35`, `along_pull: true`, flipped for the −z cheek. Two more go on the end walls.

**Shows off.** Work plane over stock; net → cells → offsets → one multi-loop cut; revolve at the origin, then seated; the stone/bezel/bur trio; an About array; stamps on stock; the pierce builder mirrored through a section.

**Traps and how they are avoided**
- **Crossing circles are refused** (region.rs:133-137). Split first. Name each light by an entity on its outer loop so edits keep it (region.rs:52-60).
- **The table rim rolls 0.6–0.9 mm under the plane** (CLAUDE.md, *The head has one edge*). The rim moulding ends 1.4 mm inside the 9.0 half-face, and the mouchettes stay 1.0 mm inside the rim.
- **The pilot and the nearest light share metal.** The oculus bur's pilot and the first spandrel light must keep 0.8 mm between them. Cut tools that overlap are clustered into one tool (parts.rs:17), so they would merge silently rather than fail. The author asserts this distance.
- **Pattern copies of stones lose their gem** (`gem: None`, pattern.rs:518). The stones report, gem preview and render would count one stone. E2 is blocking; the fallback is 8 hand-placed stones with bezels and burs.
- **World-coordinate revolves drift on resize.** Both mouldings are modelled at the origin and seated (rule 2).
- **Stamps on the cheeks.** Use `along_pull`, and no wall may straddle z = 0 (setting.rs:920-957). This is harmless in lost wax, but it keeps the file honest if someone switches the process.

**Needs.** E2 (blocking for honest stones), E8 (ruby and sapphire tints), E3 (saves ~9 offset edits and makes bar width one number), E7 (quatrefoil oculi), E11.

**Template.** `lift::from_design` gives stock nodes (face, US size) plus a `/stamps` patch (lift.rs:439-452; E13) plus the `cad.feature` chain (nodes/cad.rs:254).
- Controls: **Lights** is one number node feeding both the rose-tracery cluster (E4) and the About pattern's `count` through `json.set` into `cad.feature.operation` (nodes/cad.rs:32-39). **Bar**, **Oculus stone** and **Pear size** are also exposed.
- Pattern counts lift cleanly. The offsets are baked into the sketch until E3/E4.

**Difficulty 4/5; risk medium.** About 20 csg cuts go through a 5.5 mm-deep head. At export density a Subtract costs 10–331 ms (PLAN.md:823), so a build takes seconds; that is why E1 matters.

---

## 2. Oculus — the wheel

**Concept.** Seen along the finger, a ring's side face is an annulus, which is the plan of a wheel window. Its bore is the oculus. Twenty-four lancet lights pierce the band from side face to side face along the pull, so the finger itself is the eye of the rose. The crown carries the wheel's voussoirs.

| Zone | What is there |
|---|---|
| Side faces (both) | The wheel: 24 lancet lights pierced through along the pull, each half drafted 2° from the parting plane; cusped blind spandrels over the lancet heads. |
| Crown | 24 voussoir reeds across the crown, aligned on the mullions, with a crest milgrain row. |
| Shoulders / palm | The same wheel all the way round; the building is circular. |
| Bore | The oculus. Plain, comfort 0.1. |

**Base.** Procedural band: `ProfileStyle::Flat`, 7.0 × 4.0, `flatten_sides()`, edge round 0.25, comfort 0.1, `ShankKind::Uniform`. **Process:** Delft two-part sand; everything here follows the pull. **Alloy:** Silver 925, about 19 g. **Stones:** none.

**Feature timeline** (9 CAD features, 2 field layers):
1. `Band`.
2. *High side face*: `Plane { base: Parting, offset_mm: 3.5 }` (pattern.rs:81, :541). The sketch origin is the finger axis, so the wheel is centred by construction.
3. *Wheel*: circles at r 10.1 (0.8 over the bore) and r 12.4, which is 0.3 inside the side-face run that ends near 9.3 + 4.0 × 0.85. Add one radial mullion and one pointed head (two arcs meeting 0.35 under the outer circle), then Polar ×24, split, `offset(light, −0.45)`, and mark the net construction.
4. *Cut the lights to the parting plane*: `Extrude { Feature{3}, height_mm: −3.55, draft_deg: 2.0 }`, Cut. The draft narrows each half toward z = 0.
5. *The drag half*: `Pattern { source: 4, kind: Mirror { plane: Band } }` (pattern.rs:61), Cut.
6. *Cusps*: a second sketch on plane 2 with the 24 spandrels above the heads as trefoil-cusped pockets. `Extrude −0.30, draft 5°`, Cut; then 7. Mirror across the Band.

**Field layers.** `FlutesLayer { count: 24, profile: Round, width_mm: 1.3, height_mm: 0.2, lean: 0, along: false }` (field.rs:1846) as voussoirs, and `MilgrainLayer { v_mm: band_v_len/2, bead_diameter_mm: 0.5, beads_around: 96, height_mm: 0.2, mirror: false }` (field.rs:2207).

**Shows off.** A parting-plane work plane with an offset; polar patterns about the finger axis; cuts drafted from z = 0; a mirror across the band; CAD and height field in one ring; the sand verdict on CAD cuts.

**Traps and how they are avoided**
- **Perfect draft exists only on the side-face run.** That is the run `FieldContext::side_faces(80°)` finds (field.rs:192, :264), which is `thickness − crown` wide. Flat spends 15% of the thickness on the crown, and the lights stay 0.3 inside the run.
- **Sand slots.** The minimum is 0.6 (manufacturing/mod.rs:58, release.rs:460). A 1.3 mm mouth narrows to about 1.18 mm at the waist under 2° over 3.5 mm. The slot check is a screening check, not a strength model (package.rs:43). Each half-window is 3.5 mm of sand hanging from its own mould half (aspect ~3), which is a named physical-trial item.
- **Exit faces and the parting plane.** Each half stops 0.05 past z = 0 and the halves overlap (rule 7), so no floor lies in the far face.
- **The reeds must stay straight.** Lean 0 measures 0.000%; lean 0.2 already gives 0.37% (field.rs:1850-1866). The shank must stay Uniform, because relief whose walls face round the ring leans wherever the section changes width (CLAUDE.md, *Two masterworks*).
- **Beads.** The milgrain rides the crest line only (CLAUDE.md, *The showcase is the measured tour*).
- **Axial web.** Blind cusps from both sides leave 7.0 − 0.6 = 6.4 mm of web. The verdict cannot see an axial web (CLAUDE.md), so the README states it.
- **Resize.** The wheel's radii are absolute. A larger size pushes the bore into the inner circle, so E4's cluster must read `band.size`.

**Needs.** E4 (a bore-driven wheel cluster), E3 (optional), and optionally a 1-line `FlutesLayer::phase_deg` to lock reeds to mullions without rotating the sketch.

**Template.** The lift gives profile and shank nodes, 2 layer nodes and 7 `cad.feature` nodes. Controls: **Lights**, **Bar**, **Width**, **Thickness**. Lights also drives the flute count, so reeds and mullions stay aligned.

**Difficulty 2/5; risk low.** The one open item is the sand-core trial.

---

## 3. Ogiva — the keel

**Concept.** The ring *is* a pointed arch: its cross-section is a blunt lancet, 5.6 wide and 3.6 thick, standing on the bore. Thirteen crockets climb its keel over the top of the hand, and thirty-six blind lancet niches are cut into each foot. Every feature is an extrusion along the pull, and the ring pours in two-part sand.

| Zone | What is there |
|---|---|
| Crest | The keel (a 151° crease on the parting plane), with 13 crockets over the top 144°. |
| Flanks / feet | 36 blind lancet niches each side, cut along the pull into the near-vertical feet. |
| Palm | Keel only; no crockets to snag. |
| Bore | A shallow comfort arc (sagitta 0.12). |

**Base.** A CAD-only ring with no `Band` feature, so the document is the whole ring (mesh.rs:397-404). **Process:** Delft two-part sand. **Alloy:** Silver 925, about 11 g. **Stones:** none.

**Feature timeline** (about 16):
1. *Section plane*: `Plane { base: Section { theta_deg: 0 } }`, with x radial and y along the finger (pattern.rs:535-537). Equivalently, use `Workplane::section()` (sketch.rs:77).
2. *Pointed-arch section* (the constraint lesson).
   - Points: springers A (9.3, −2.8) and B (9.3, 2.8); apex C (12.9, 0); arc centres L (9.3, +0.914) and R (9.3, −0.914). The radius 3.714 follows from h² = s² + 2sc.
   - Constraints (sketch.rs:143): `Symmetry(A, B, O)` and `Distance` L–A = L–C = R–B = R–C = 3.714.
   - Corners: `fillet_corner(A, 0.3)` and `fillet_corner(B, 0.3)` (edit.rs:678), each above `MIN_EDGE_MM` = 0.2 (profile.rs:18).
3. *Revolve the ring*: `Revolve { Feature{2}, pivot 0, axis [0,0,1], degrees 360 }`, the same move as the workshop Lantern's analytic shank (workshop_collection.rs:149-173).
4. *Parting plane*: `Plane { Parting, offset_mm: −0.03 }`.
5. *Crocket*: a curled leaf drawn in XY at θ 18° (the first of a 144° array), sunk 0.25 into the keel, hooking round the ring (`Geometry::Bezier`, sketch.rs:121).
6. *Cope half*: `Extrude { 5, +0.33, draft 3° }`, which runs from −0.03 to +0.30. Then 7. `Mirror { Band }` makes the drag half, and 8. `Boolean Union` joins them (csg route for mesh operands, cad.rs:1912-1924).
9. *Thirteen crockets*: `Pattern { 8, Ring { count: 13, span_deg: 144 } }` (world turns about the finger axis, pattern.rs:241-243).
10. *Foot plane*: `Plane { Parting, offset_mm: 2.85 }`. 11. *Niche*: a 0.9 × 1.2 pointed niche at r 10.4, Polar ×36 about the axis.
12. *Cut the niches*: `Extrude { 11, −0.95, draft 3° }`, which puts the floor at z = 1.9. 13. Mirror across the Band.
14. Union the ring with the crockets. 15. Subtract 12. 16. Subtract 13. All csg.

**Shows off.** A constrained sketch; revolving a whole ring; features drawn in the parting plane and extruded along the pull; ring arrays; kernel-versus-mesh booleans; a sand verdict on a parts-only ring.

**Traps and how they are avoided**
- **No chart means no field tools.** A parts-only ring has no field verdict, stamps or cutters ("this ring is its parts alone", cutters.rs:281, :1100; `Bore::of` has no band, builders.rs:581-582). It is judged by `manufacturing::inspect`'s ray release (manufacturing/mod.rs:568), in the pattern of workshop_collection.rs:293-333 for sand. The README states this route.
- **The keel sits on z = 0 by constraint.** Parting is fixed with `auto_parting = false, parting_mm = 0` (stock_masterworks.rs:483-484).
- **The section is a single-crest monotone drop.** Its feet are side faces, because the arc's tangent is radial at the springer. This is the superellipse guarantee by another road (CLAUDE.md, *The castability guarantee lives in profile.rs*).
- **Crocket hooks.** A hook curling back over the keel would lock if it were extruded along the normal. Drawn in the parting plane and extruded along the pull, it cannot (rule 1).
- **Drafted halves meeting on z = 0 share a face**, which is `Snag::Degenerate`. Each half starts at −0.03.
- **Crest phantom.** An irregular mesh reports spurious undercut along a crest (CLAUDE.md, refined-build caveat). This keel is a crease with 14° of real draft each side, so facet noise has draft to be measured against. The release study still runs at 0.100 and 0.075 mm, as Reptilia's did.
- **Resize.** A revolved section does not follow ring size. E4's pointed-arch cluster takes the bore from `band.size`.

**Needs.** E4 (a `pointed-arch-section` cluster: width, thickness, keel sharpness c, bore) and E5. An `EqualRadius` constraint would make the arch a one-number edit (small, sketch.rs:143).

**Template.** `nodes::cad::from_document`/`chain_document` gives `cad.source` plus 16 `cad.feature` nodes (nodes/cad.rs:202-252). Controls: **Width**, **Thickness**, **Keel**, **Crockets**, **Niches**.

**Difficulty 3/5; risk medium.** This is the first parts-only sand ring in the library.

---

## 4. Arcus — the flying buttress

**Concept.** A cathedral solitaire that means it. The sapphire sits in an openwork basket, the choir. The app's own `shank.cathedral` arches are the flyers, and each one springs from a pinnacled pier with a gargoyle spout, standing on the band exactly where the builder's arch lands. Six teardrop azures under the stone are the crypt windows. The shank's side faces carry the nave's blind arcade to the palm.

| Zone | What is there |
|---|---|
| Head | Emerald-cut 8 × 6 sapphire in a four-claw basket with 2 rails; 6 teardrop azures under it. |
| Shoulders | Two flyers (`shank.cathedral`, spread 34°), each springing from a pier with a lofted spire, 4 crockets, a revolved knop finial, and a gargoyle spout. |
| Side faces | The nave: a blind lancet arcade (SVG tiling) from the piers to the palm. |
| Palm | The apse, plain. |

**Base.** Procedural `Flat` 3.2 × 2.3, `flatten_sides()`, `ShankKind::Cathedral` amount 0.8 (a +55% width swell on the shoulders, profile.rs:2662-2669). **Process:** lost wax; the basket, azures and overhanging flyers are investment work. **Alloy:** Gold 18k. **Stones:** 1 sapphire.

**Feature timeline** (about 20):
1. `Band`.
2. *Stone*: Emerald w 6 l 8, sapphire tint, at `Placement::ring(90, stand_off_mm("basket", gem))`.
3. `head.basket {prongs: 4, wire_mm: 0.9, rails: 2}` (builders.rs:21, :158).
4. `seat.bur {through: true}`.
5. `cutter.azure {shape: "Teardrop", count: 6, head: 3}` (cutters.rs:712, :1167).
6. *Flyers*: `shank.cathedral {spread_deg: 34, rise: 0.75, wire_mm: 1.0, head: 3}` (cutters.rs:977, :1176).
7. *Pier*, at the origin: `Box { size: [1.6, 1.6, 2.4] }`.
8. `Plane { Face { feature: 7, face: top } }` (pattern.rs:82). Sketches 9 and 10 give a square 1.6 on it and a square 0.25 on the same face offset +2.2.
11. *Spire*: `Loft { sections: [9, 10] }` (cad.rs:82, :1893; matching curve counts). 12. `Boolean Union 7 + 11`, a kernel boolean under 500 faces (cad.rs:2020).
13. *Crocket*: a leaf sketched on one sloped planar face of 12 (`FaceAnchor`), `Extrude +0.35`. 14. `Pattern { About { part: 12, count: 4 } }` about the pier's own z (pattern.rs:245).
15. *Knop*: `Revolve` of a ball-and-collar profile about local z at the spire tip.
16. *Gargoyle*: `sketch::exchange::import_svg(gargoyle.svg)` (exchange.rs:146) on an inline workplane in the local y–z plane, so the spout projects outward and is seen side-on. `Extrude` ±0.45 (plane offset −0.45, height 0.9).
17. *Pinnacle*: a Union chain of 12, 14, 15 and 16 (csg, since 14 is a mesh). It is seated at `Placement::Ring { theta_deg: 124, height_mm: −0.3 }` with `blend_mm` 0.3. **124 = 90 + spread_deg**: the pier stands where the builder's arch lands.
18. `Pattern { 17, Mirror { Section { theta_deg: 90 } } }`, dropped onto the band at 2·90 − θ (pattern.rs:263-270).

**Field layer.** `TilingLayer` with a "Nave arcade" SVG alpha (3 bays a tile, 36 repeats, relief 0.25), windowed from 124° to 416°, `VGate::SideFaces(Both)` (field.rs:399-436).

**Shows off.** Builders and your own parts meeting at one number; loft; sketch on a face; About arrays; an imported SVG outline; a seam bead; a mirror through a section.

**Traps and how they are avoided**
- **The flyer builder has preconditions.** `shank.cathedral` needs a head and a stone standing out of the crown (cutters.rs:981, :988). It refuses an arch that runs back into the band with "give it more rise, or less spread" (cutters.rs:1038). Rise is 0.75 for that reason.
- **The pier must track the spread.** The template feeds one **Spread** number to feature 6's params and feature 17's placement, which needs E5 for the placement pin.
- **Faces before meshes.** The crocket's sketch-on-face must precede the About pattern; after 14, the spire is a mesh (rule 3).
- **The gargoyle SVG has strict rules.** It needs units on width and height, a uniform viewBox, no transforms, and no S/T shorthand (exchange.rs:158-186, :363).
- **Seam bead v1 blends no three-edge corners** (blend.rs:15-16). The pier's corners pinch toward 0.02, so the pier foot gets a 0.3 chamfer (`Chamfer`, cad.rs:95) before the bead.
- **Wall over the finger.** Claws are floored by the bore (builders.rs:40), and azures refuse a stone too small by name (PLAN.md:718). An 8 × 6 clears both.

**Needs.** E8 (sapphire), E15 (gargoyle and crocket SVGs), E5 (one Spread number on a placement). E14 (pointed Gothic claw tips) is optional.

**Template.** The lift gives profile and shank nodes, a tiling layer and SVG alpha node, and 18 `cad.feature` nodes. Controls: **Stone size**, **Spread**, **Rise**, **Pier height** (via `json.set` into the Box size).

**Difficulty 4/5; risk medium-high.** csg union chains of small parts, and a builder's arch meeting a hand-built pier.

---

## 5. Sigillum — the chapter seal

**Concept.** A cathedral chapter's seal ring on a heater-shield stock. The seal is **cut at the bench**, which is what a seal is: mirrored intaglio with drafted walls, so wax releases the impression. What can be cast is cast: the chapter-house arcade recessed into the cheeks along the pull, and quatrefoil roundels down the shoulders. This is the collection's honest sand-plus-bench ring.

| Zone | What is there |
|---|---|
| Face (table) | Bench intaglio, mirrored: a cusped quatrefoil field (−0.30), a fleur-de-lis (−0.70), a Textura legend ✠ SIGILLVM CAPITVLI round the rim (−0.40). |
| Cheeks | Cast recessed three-bay lancet arcade (stamps, `along_pull`, cut). |
| Shoulders | 3 graded quatrefoil roundels each side (stamps, `along_pull`, cut 0.25). |
| Palm / bore | Plain. |

**Base.** Factory **004** Shield (flat, notched top), face 16 × 17, `sand_envelope = true`. **Process:** Delft two-part sand; nothing is proud on the table. **Alloy:** Silver 925. **Stones:** none.

**Timeline.** It has 10 stamps and 6 CAD features.
- *Stamps* (setting.rs:897): 2 cheek arcades (`cut`, `along_pull`, `sink_mm` 0.35, the −z one flipped) and 6 roundels.
- CAD:
  1. `Band`.
  2. *Table*: `Plane { Tangent { 90, 0 } }`.
  3. *Seal*: a sketch holding the quatrefoil (4 arcs plus cusps), a fleur-de-lis (harvested outline, E7/E15, through `import_svg`) and the legend (E10 text on an arc, r 7.2). Then `Sketch::mirror(all, vertical axis)` (edit.rs:738) so the impression reads true.
  4. *Field*: `Extrude { Region{3, field}, −0.30, draft_deg: 15 }`, `Attach::Cut`, **`Stage::Bench`** (cad.rs:310-316).
  5. *Fleur*: `Extrude { Region{3, fleur}, −0.70, draft 12° }`, Bench Cut.
  6. *Legend*: `Extrude { Region(s) letters (E3's Profile::Regions), −0.40, draft 12° }`, Bench Cut.

**Shows off.** Bench-stage cut features; `draft_deg` on a cut (cad.rs:62-64); mirrored sketches; finished versus pattern (`try_build_pattern`, mesh.rs:409-413; `leave_bench_parts_out`, cad.rs:1340); stamps on stock with the sand envelope.

**Traps and how they are avoided**
- **Nothing is cast on the table.** On a signet's face, anything proud off the parting line is an undercut (CLAUDE.md, *One theme per ring*). The seal is all bench.
- **Bench marks can lock.** Under sand, bench parts leave a raised mark in the pattern (mesh.rs:409). A dot off the parting line on the zero-draft table leans 14° (CLAUDE.md, *sketches* lesson), and a mark off the parting line is never moved onto it (PLAN.md:694). Each bench cut feature's footprint is therefore centred on z = 0: one feature per depth, all centred. E12 is the real fix.
- **Stamps.** Use `along_pull` (the 0.35 mm tuck lesson, setting.rs:920-923). No wall may straddle z = 0 (setting.rs:944-957). Strokes must be at least Delft's 0.30 detail (castability.rs:180), which is judged by granulometry (`dfm::findings`).
- **The sand envelope must be on**, or single-sample obstructions appear on bare stock (CLAUDE.md, *One theme per ring*, the Caiman notes).
- **The chart reads true from −Z**, so the −z cheek's stamp is flipped (CLAUDE.md, *The showcase*).
- **Depth over the bore.** The deepest cut, 0.70, leaves about 1.05 of the 1.75 table over the bore, above `MIN_WALL_MM` 0.5 (mesh.rs:20).

**Needs.** E10 (Textura font plus text-to-sketch on an arc), E12 (bench marks), E7/E15 (fleur-de-lis outline), E13 (stamp nodes), E3b (`Profile::Regions`, or one sketch per legend word).

**Template.** Stock nodes for 004, a stamps patch and 6 `cad.feature` nodes. Controls: **Legend** (text node, E10), **Seal depth**, **Face size**.

**Difficulty 3/5; risk medium.** The bench-mark rule and the missing text-to-sketch are the risks.

---

## 6. Lanterna — the octagon lantern

**Concept.** Ely's octagon, seen from beneath. A star vault of ribs is raised on the octagonal table and gathers into a revolved boss carrying an Asscher amethyst. Eight lancet windows pierce the table between the ribs. At the eight corners stand pinnacles with twisted spirelets, each carrying a citrine lamp on its outward face. Stepped clasping buttresses hold the head to the shank.

| Zone | What is there |
|---|---|
| Face (table) | Star vault: 8 ribs and 8 tiercerons, 0.6 wide, raised 0.45. The boss carries a 6.0 Asscher amethyst in a bezel, à jour. 8 pierced lancet lights. |
| Corners | 8 pinnacles: a shaft, a twisted spirelet (45°, taper 0.08), 4 crockets and a knop finial. A Round 1.5 citrine lamp sits on each outward face. |
| Walls (octagon's 8) | Paired blind lancets (stamps, `along_pull`, cut 0.3). |
| Shoulders | Clasping buttresses with two set-offs, mirrored. |
| Palm | Plain. |

**Base.** Factory **015** Octagon, 16 × 16. **Process:** lost wax. **Alloy:** Gold 18k. **Stones:** 1 amethyst + 8 citrines.

**Feature timeline.** About 34 features with E2, about 75 without.
1. `Band`. 2. *Table* (Tangent 90).
3. *Star vault*: one closed star outline. Draw the ribs from the boss circle (r 3.0) to the corners and the tiercerons to the mid-sides, drawn once, Polar ×8, split and trimmed into a single loop with no crossings and no holes. 4. `Extrude +0.45` Join, `blend_mm` 0.12.
5. *Lantern windows*: one lancet 1.1 × 2.4 at r 5.2, Polar ×8. 6. `Extrude −6.0` Cut, through.
7. *Boss*: `Cylinder { radius_mm: 3.0, height_mm: 1.2 }`, seated at the table centre. 8. *Raise the boss top*: `PressPull { face: top, distance_mm: +0.3 }` (cad.rs:131; pattern.rs:601). This is the direct-edit lesson.
9. *Boss moulding*: a revolved roll at r 3.0–3.5, seated with the boss.
10. *Amethyst*: `cad::stone_on_face(id, Asscher 6.0, on: 8, &FaceSeat::on(..))` (cad.rs:576, :656). 11. `head.bezel`. 12. `seat.bur {through: true}`.
13. *Pinnacle shaft*, at the origin: `Box [1.3, 1.3, 1.8]`.
14. *Twisted spirelet*: `Twist { sketch: rectangle 1.3 on the shaft top, path: straight 2.4, degrees: 45, end_scale: 0.08 }` (cad.rs:76, :1875; twist.rs:361).
15. *Crocket plus About ×4*. 16. *Knop*: a Revolve.
17. *Lamp*: stone Round 1.5 citrine on 13's outward face (`FaceSeat`), seated **before** the union makes a mesh. 18. `head.bezel {wall_mm: 0.3}`. 19. `seat.bur` blind.
20. *Pinnacle*: union 13 + 14 + 15p + 16, seated at the first corner (θ and across from the table plan, 1.0 inside the outline, height −0.2), blend 0.2.
21. *Eight pinnacles*: `Pattern { sources: [20, 17, 18, 19] (E2), About { part: 10, count: 8 } }`.
22. *Buttress*: a `Parting` plane; an elevation sketch at θ 130 with two weathered set-offs; `Extrude` ±0.9; Join, blend 0.3. 23. `PressPull` on the upper set-off, −0.25. 24. `Mirror { Section { 90 } }`.

**Stamps.** 8 wall stamps.

**Shows off.** Twisted sweep with a taper; revolve; press-pull; a stone on a part's face; an About array of a whole assembly; a parting-plane elevation.

**Traps and how they are avoided**
- **Seat the lamp before the union.** The lamp needs a kernel planar face, so it is seated on the Box before the union turns the pinnacle into a mesh (cad.rs:593-637; rule 3).
- **The twist is a mesh.** Fillet, press-pull and sketch-on-face refuse it by name (CLAUDE.md, *CAD parts stand on the band*). Its section must stand square to the path at the start (twist.rs, `SQUARE_MM`).
- **The rim rolls.** The table's rim is filleted 0.6–0.9 below the plane, so pinnacle feet stand 1.0 inside the outline and sink 0.2.
- **About arrays stay on the table only because it is flat.** They are rigid world turns about the stone's axis (pattern.rs:274).
- **Face budget.** `MAX_PATTERN_FACES` is 2,000,000 (pattern.rs:22). Eight twisted spirelets at the 0.015 mm export chord (cad.rs:2010) must be counted by the author.
- **Windows versus ribs.** The windows run 5+ mm deep near the corners, which investment casting accepts, but at least 0.8 of metal must remain between window and rib (rule 5).

**Needs.** E2 (blocking: 8 lamps would otherwise be 32 hand-placed features), E8 (amethyst, citrine), E11.

**Template.** Stock nodes for 015, a stamps patch and about 24 `cad.feature` nodes. Controls: **Pinnacle height** (twist path length), **Twist**, **Lamp size**, **Boss stone**.

**Difficulty 5/5; risk high.** This is the most complex CAD tree in the library.

---

## 7. Capsa — the reliquary

**Concept.** A Gothic chasse lies along the finger, following the coffin-ring convention of memento-mori rings. It stands on a plinth, its long walls arcaded and set with cabochons like Limoges enamels. Its gabled roof is a **separate cast lid**, hinged at the bench, crested with fleurs-de-lis and finialled. Open it: a skull lies on the relic floor.

| Zone | What is there |
|---|---|
| Head | Chest 13 (along the finger) × 9.4 × 3.4, shelled 0.8, on a plinth. Three-bay blind arcades on both long walls. 4 oval cabochons in the centre bays and 2 on the gables. |
| Lid (Separate) | A gable roof prism, fleur-de-lis cresting on the ridge, 2 revolved finials, eaves filleted 0.2 by OpenCascade. |
| Inside | A memento-mori skull in relief (+0.4) on the relic floor. |
| Shoulders | 3 graded quatrefoil piercings each side (Round until E7). |
| Palm / bore | Plain. |

**Base.** Procedural `Flat` 4.6 × 2.3, `ShankKind::Cathedral` amount 0.5. **Process:** lost wax, **two castings** (ring plus lid). **Alloy:** Gold 18k. **Stones:** 6 cabochons: 4 garnet, 2 sapphire.

**Feature timeline** (about 31):
1. `Band`.
2. *Chest*: `Box [13.0, 9.4, 3.4]` at the origin (Free).
3. `Shell { source: 2, open_faces: [top], thickness_mm: 0.8 }`, natively (analytic box; cad.rs:1967-1980).
4. *Relic floor*: `PressPull` on the inner floor, +0.4.
5–7. *Arcade*: a sketch on the +y wall's outer face (`FaceAnchor { 3, face }`) with 3 lancet bays; `Extrude −0.30`; `Boolean Subtract`, a kernel boolean that keeps the chest B-rep. Features 8–10 do the same for the −y wall.
11–13. *Skull*: `import_svg(skull.svg)` sketched on the floor face; `Extrude +0.4`; kernel Union. Feature 13 carries `Placement::Ring { 90, height_mm: … }`.
14. *Plinth*: `Box [9.0, 4.4, 2.0]` seated at 90 with height −0.4, Join, blend 0.35. The parts cluster unites it with the chest (parts.rs:17).
15–26. *Cabochons*: `stone_on_face(.., Gem::cabochon(Oval, 3.0) l 4.0, on: 13, FaceSeat { niche floor })`, each with `head.bezel {wall_mm: 0.35, lip: 0.3}` and a blind bur for the bed.
27. *Lid*: a pentagon sketch (eaves 9.8, ridge 2.6) on a workplane at the chest's end, `Extrude 13.2`, **`Attach::Separate`**, `Stage::Cast`, seated like the chest plus its height.
28. *Cresting*: an `import_svg` fleur-de-lis strip on the ridge's mid-plane, `Extrude` ±0.3, Union.
29. *Finials*: a Revolve plus `Mirror { Plane { feature } }` across the chest's mid-plane.
30. *Eaves fillet*: OpenCascade, `ringdesign_occt::parts::fillet(c, eaves, 0.2, EXPORT)` giving `stored_feature` (occt parts.rs:35, :70). It is the lid's last feature.
31. `Joint { a: 13, b: 27, clearance_mm: 0.05, method: "Three-knuckle hinge, 0.8 mm pin, soldered at the bench" }` (cad.rs:1277).
32–33. `cutter.pierce` ×3 via `pierce_at` at θ 90 − (48, 60, 72), mirrored through Section 90.

**Shows off.** Native shell; press-pull; sketch on a part's face; B-rep booleans kept B-rep; stones on part faces; a Separate part as its own casting (`manufacturing::Casting::Part`, manufacturing/mod.rs:301-307); a joint; OpenCascade as a last step; SVG outlines.

**Traps and how they are avoided**
- **Native shell is only for analytic boxes, cylinders and spheres** (cad.rs:1979). The chest is shelled before any cut. The pentagon lid stays solid.
- **Seat the stones before the chest becomes a mesh.** Stones need planar kernel faces (rule 3).
- **The chest floats at its ends.** Round the ring its 9.4 mm width drops 0.97 mm at ±4.7 on r_o ≈ 11.6. The plinth fills the gap, and a seam bead closes the join.
- **Wall behind a niche.** A 0.30 niche in a 0.8 wall leaves 0.5, under the 0.8 investment section. Walls are 1.0 where niches are cut, or niches are 0.25.
- **The lid is a separate casting**, poured only when chosen (CLAUDE.md, *The ring is the casting*). Both castings are exported.
- **OpenCascade costs.** It is a no-go on Windows (PLAN.md:730), and it is a stored mesh that stands as made until it runs again (stored.rs). It forces format 6 for the design and the template. It stays the lid's last feature; the README documents "Run again", and a native-only variant without it is available.
- **Shoulder piercings on a swelling section.** `pierce_at` sizes to the local band and refuses near the edge (cutters.rs:1099-1150).

**Needs.** E8, E15 (skull and fleur-de-lis cresting SVGs), E7 (quatrefoil). E6 (a line pattern for the bays and cabochons) is optional. E10 is optional for a MEMENTO MORI under the lid.

**Template.** Band nodes plus about 33 `cad.feature` nodes, including one `Stored` (format 6; the graph carries it by digest reference). Controls: **Chest size** (`json.set` into the Box), **Wall**, **Cabochon size**.

**Difficulty 4/5; risk medium-high.** The stored mesh in a bundled template and the two-casting manufacturing path are the risks.

---

## 8. Porta — the portal

**Concept.** A cathedral's west portal cut into a mandorla-shaped kite signet. Four archivolts step down into the head like the splay of a real portal, each carrying a swept roll moulding. A ruby sits in the tympanum, and the twin doors are split by a trumeau. An ogee hood-mould with crockets rises to a fleur-de-lis at the kite's upper point. A pierced trefoil crypt window fills the lower point.

| Zone | What is there |
|---|---|
| Face (table) | A portal 9.0 × 13.0. 4 archivolts with a depth ladder of 0.25/0.50/0.75/1.00 and a roll moulding each. Tympanum at −1.0 with a Round 3.0 ruby, à jour. Door leaves at −1.0 and the trumeau at −0.75. An ogee hood (+0.4), 6 crockets and a fleur-de-lis finial. A crypt trefoil pierced through. |
| Walls | Blind arcades (stamps, `along_pull`, cut). |
| Shoulders | 4 pierced lancets each side, diminishing (E7; Marquise until then). |
| Palm | Plain. |

**Base.** Factory **002** Kite, whose face is 14 × 25 (imported_base.rs:1095), resized to 14 × 21. **Confirm the long axis runs up the finger**, so the portal stands upright on the hand (CLAUDE.md, *An upright face*). If it runs round the ring, fall back to 009 Drop. **Process:** lost wax. **Alloy:** Gold 18k. **Stones:** 1 ruby.

**Feature timeline** (about 30):
1. `Band`. 2. *Table*.
3. *Portal*: a pointed arch (two arcs plus a threshold). `offset(−0.7)` four times gives 4 nested loops. Add a lintel line at 60% and a trumeau pair as T-junctions. The regions are 4 rings, the tympanum, 2 leaves and the trumeau.
4–7. *Archivolt i*: `Extrude { Region{3, ring i}, −0.25·i }`, Cut.
8–10. Tympanum and leaves at −1.0, Cut. The trumeau is not cut deeper than −0.75.
11–14. *Roll moulding i*: `Sweep { Sketch::circle(0.22), path: step-i arch edge sampled every 0.3 mm on its floor }` (at most 128 stations, cad.rs:1849-1852), Join.
15. *Ruby*: seated at the tympanum centre, height −1.0 plus stand-off. 16. `head.bezel`. 17. `seat.bur {through}`.
18. *Hood*: an ogee drip strip, 0.5 wide, drawn with two `Bezier` S-curves. 19. `Extrude +0.4`, Join, blend 0.1.
20. *Crocket*: a leaf, `Extrude +0.3`. 21. A pattern along the ogee (E6 `Along`; the interim is 6 hand-placed copies).
22. *Finial*: the fleur-de-lis outline, `Extrude +0.5`.
23. *Crypt trefoil*: `Extrude −6` Cut, through.
24–25. `cutter.pierce` ×4 plus Mirror through Section 90.

**Shows off.** Offsets as architecture; depth ladders by region; a sweep along an arch; Bezier ogees; region picks that survive edits; a pattern along a curve.

**Traps and how they are avoided**
- **Wall over the bore.** The table stands about 1.75 over the bore at the centre (CLAUDE.md, *Three bought signets*), so the ladder stops at 1.0. The ruby's pilot is the only through-cut there. The field verdict's `thinnest_wall` catches a violation radially.
- **`offset` refuses folds** (edit.rs:430-432). The pointed apex offsets inward cleanly; the hood's concave ends are trimmed.
- **Sweep paths are world points.** A resize translates the imported face rigidly (docs/SIGNET-BASES.md, *Deformation*), but not the path. `SweepPath::Sketch` (E16) ties the path to the Table.
- **Crockets are proud of a zero-draft table.** That is lost-wax-only work, and it is why Porta is not sand.
- **Kite orientation** (see Base).

**Needs.** E6 (`Along`), E7 (Lancet), E15 (fleur-de-lis, crocket leaf), E8, E16.

**Template.** Stock nodes for 002, stamps and about 25 `cad.feature` nodes. Controls: **Step depth** (one number times i through a script node into features 4–7), **Steps**, **Stone size**.

**Difficulty 3/5; risk medium.**

---

## Enablers, ranked

[S] marks enablers shared with the other three collections.

| Rank | Enabler | Needed by | API sketch |
|---|---|---|---|
| **E1** [S] | **Async template open with staged progress.** This is Logan's note: "asynchronous loading of the templates so that we dont stall the app while it builds the template ... with a loading progress too." | All 8 (40–75 CAD features and 10–40 csg resolves each), and every heavy template | `load_catalog_template` (gui export.rs:543) runs `instantiate`, `unpack_embedded` and `bake_all` on the UI thread; Android does the same (app.rs:1540, files.rs:158). Replace it with `start_template`, modelled on `start_import` (gui app.rs:612): a `PendingTemplate { answer, cancel, progress: Arc<Progress> }` slot polled each frame, with a status-strip bar and Cancel. Stages: Decompress; Graph (the `Evaluator` reports node i/N); Bake (alpha j/M on a cloned library, merged on adopt); CAD (`BuildCtx` gains `progress: Option<&AtomicU32>` beside `cancel`, cad.rs:2067, ticked per feature in `evaluate_memo`, :2639); Parts (cluster c/C in `parts::resolve_with`); First preview. The old design stays live until adopt. |
| **E2** [S] | **Pattern a feature set; patterned stones stay stones** | Rosa, Lanterna (blocking), Capsa/Porta (nice) | `Operation::Pattern { sources: Vec<Id>, kind }` (serde accepts `source`). The copies of a stone keep a per-copy `gem` (today `gem: None`, pattern.rs:518), inherit `reference`, and are counted by `stones::report`, `gems::preview_vertices` and `render`. A head and bur move with their stone by one motion set. |
| **E3** | **Tracery from a net** | Rosa, Oculus, Lanterna, Porta | `Sketch::tracery(&mut self, net: &[Id], bar_mm: f64) -> Result<Vec<Vec<Id>>>` in sketch/edit.rs: split (:192), then cells (region.rs:138), then `offset(cell, −bar/2)` (:433) per cell, naming cells that would fold, then mark the net construction. **E3b:** `Profile::Regions { feature, regions: Vec<RegionRef> }` so a branched sketch extrudes a chosen set (cad.rs:1648 `regions_of`). |
| **E4** [S] | **Parametric Gothic clusters** (script → operation JSON) | Oculus, Ogiva (resize), Rosa, Porta | `graphs/clusters/{rose-tracery, lancet-arcade, pointed-arch-section}.cluster.json`: rhai script nodes emitting an `Extrude`/`Revolve` op as `Json` (script node.rs:56). They read the bore from `band.size`, so a resize moves the architecture, and wire into `cad.feature.operation` (nodes/cad.rs:32-39). |
| **E5** [S] | **Pins on `cad.feature`** for the op's numbers and placement | Arcus (Spread), all templates' controls | Auto input pins per numeric JSON pointer of the operation, plus `theta_deg`, `across_mm`, `height_mm` and `spin_deg` of `Component.placement`. Today only whole-op JSON is accepted (nodes/cad.rs:24-46), which forces `json.set` chains. |
| **E8** [S] | **Stone tint on CAD stones; multi-colour renders** | Every stone ring | A `tint` param in the `stone` schema (builders.rs:147-155), written by `stone_params` (:269) and read by `gem_of` (:248) into `Gem::preview_tint` (gem.rs:234). A `tints` cycle for `halo`. `render::Part::stone` reads the gem's tint instead of `GEM_TINT` (render.rs:66). Blender reads a stones manifest instead of the hard-coded amethyst (tools/render_reptilia.py:49, :93). |
| **E6** [S] | **Line and along-curve patterns** | Porta (crockets), Capsa (bays, cabochons) | `PatternKind::Line { along: [f64;3], count, pitch_mm }` and `PatternKind::Along { sketch: Id, entity: Id, count, align: bool }` (pattern.rs:37). |
| **E7** | **Gothic cutter shapes and a harvested outline library** | Rosa, Capsa, Porta, Sigillum | `cutters::Shape::{Lancet, Ogee, Trefoil, Quatrefoil, Mouchette}` (cutters.rs:101) plus `PIERCE_SHAPES` (:16). The bright cut must inset concave cusps instead of pushing along normals (`outline(.., g)`, :147). Harvest `assets/User/Profiles/`: Under Gallery Cuts 001 (ogee), 002 (quatrefoil) and 003 (cusped lozenge); Jalis 000, 002, 010 and 016 (tileable tracery nets, as centre-lines for E3); Ornaments 027 (quatrefoil ring) and 028 (fleur-de-lis, a B-rep). Harvest these through `tools/harvest` into `bundled/sketches/gothic/*.svg` in `import_svg`-clean form, and list them with `library::list_sketches()`, overlaid like outlines. All of these were viewed and are usable. |
| **E11** [S] | **DFM for CAD cuts: land width** | Rosa, Oculus, Lanterna | For each Cut part, measure the minimum distance between cut regions and to part and band edges against `min_section_mm`, as a finding on the feature. It closes the tangential-bar half of CLAUDE.md's *two things the model does not yet know*. |
| **E12** | **Bench cut marks** | Sigillum | Bench `Cut` parts under sand get no raised mark, or their mark is moved onto the parting line (PLAN.md:694 open item). |
| **E10** [S] | **Textura font plus text as sketch** | Sigillum, Capsa (optional) | `TextFont::Textura` (OFL UnifrakturMaguntia) beside Serif and Script (text.rs:25). `sketch::text(font, text, size_mm, arc_r: Option<f64>) -> Sketch`: glyph outlines as Bezier loops with counters as holes. It also serves the collection sheet's title face. |
| **E13** [S] | **Stamps as graph nodes** | Rosa, Sigillum, Lanterna, Porta | A `stamp` node. The lift carries `/stamps` whole as one `design.set` patch (lift.rs:439-452). |
| **E15** | **Gothic SVG artwork set** | Arcus, Capsa, Porta, Sigillum | Gargoyle profile, crocket leaf, fleur-de-lis, memento-mori skull, fleur cresting strip, cusped quatrefoil. Each must meet `import_svg`'s rules: units, uniform viewBox, no transforms, no S/T (exchange.rs:146-371). |
| **E16** | **Sweep along a sketch curve** | Porta | `SweepPath::Sketch { feature, entity, lift_mm }`, so a moulding follows the Table on resize (today the path is world points, cad.rs:72-75). |
| **E14** | **Pointed and fleur claw tips** | Arcus (optional) | `head.claw`/`head.basket` param `tip: Dome | Point | Fleur` in `setting::claw_head`. |
| — | *Not needed:* a kernel shank as the band anchor | — | Ogiva works as a parts-only ring because every feature follows the pull. This would only matter for a bench-staged cut on a CAD shank. |

---

## Deliverables (Reptilia format)

- `crates/ringdesign-core/examples/tenebrae.rs`: the author, with `--draft`, `--verify` and a SLUG filter. It asserts bars, walls, sand slots and zero obstructions (sand rings at 0.100 and 0.075 mm). It writes `showcase/tenebrae/<slug>/`:
  - `design.ring.json`, `editable-graph.ring.json`;
  - `sketches/*.svg`, every sketch exported with `sketch::exchange::svg` (exchange.rs:8), which is the artwork;
  - `finished-metal.stl`, and `casting-pattern.stl` (sand) or `investment-pattern.stl`, plus `lid-pattern.stl` for Capsa;
  - `reference-<n>-<colour>.stl`;
  - `report.json`, `mesh.json`, `release-fine.json`, `verification.json`;
  - `hero/face/palm.png`, plus `section.png` for Ogiva and Oculus, and `studio.blend`.
- `crates/ringdesign-graph/examples/tenebrae_templates.rs`: lift, then E4 cluster wiring, then byte-identical design and mesh from a cold library, as in reptilia_templates.rs:12-35. It writes `graphs/templates/<slug>-tenebrae.graph.json`.
- Registration: `ringdesign-graph/src/templates.rs` `bundled!`; `ringdesign-workbench/src/templates.rs` `group("Tenebrae collection", …)` (:31-60); previews in `preview_bytes` (:62), which `every_authored_graph_and_collection_has_a_real_preview` requires.
- `tools/render_tenebrae.py` (studio gold, tinted stones by manifest), `tools/catalog_tenebrae.py` (sheet, Textura title), and `showcase/tenebrae/README.md`.

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example tenebrae -- NEW_DIRECTORY
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example tenebrae -- NEW_DIRECTORY --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example tenebrae_templates -- NEW_DIRECTORY
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_tenebrae.py -- NEW_DIRECTORY
python3 tools/catalog_tenebrae.py NEW_DIRECTORY
```

**Build order.** E1, E2 and E8 first, because they are shared and cheap to verify. Then Oculus and Ogiva, which need no enablers, prove the along-the-pull rule in sand, and make a fast first sheet row. Then Rosa (E3), Sigillum (E10/E12), Porta (E6/E16), Arcus, Capsa and Lanterna.

## Open questions for Logan

1. The name: **Tenebrae** or *Opus Francigenum*?
2. Capsa's OpenCascade eaves fillet: ship it in the template (format 6, stale on edit, no Windows OpenCascade), or keep the lid native-only?
3. Capsa's lid: a real bench hinge, or cast shut with the skull visible through pierced gables?
4. Porta on 002 Kite (21 mm up the finger after resize) or on 009 Drop?
5. Silver for the three sand rings and gold for the five lost-wax rings, or gold throughout?
