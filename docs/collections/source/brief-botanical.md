# Vepres — design brief: the thorned botanical collection

Eight rings: four signets on the factory stock and four vines. They show the app's path tools: twisted sweeps, 3D sweeps, lofts, ring, section and About arrays, CAD parts joined to the band with seam beads, and leaves struck as stamps. The height field is used only where the doctrine allows it.

Paths are abbreviated: `core/` = `crates/ringdesign-core/src/`, `ex/` = `crates/ringdesign-core/examples/`, `graph/` = `crates/ringdesign-graph/src/`. "Lesson" citations quote the project CLAUDE.md. Every castability number here is a **prediction** until the probes in E11 measure it.

---

## 1. The collection

| | |
|---|---|
| Name | **Vepres** (Latin: thorn-bushes, brambles), in the Reptilia vein |
| House line | Kings of Alchemy |
| Sheet subtitle | **FOUR SIGNETS · FOUR VINES · FIFTY-TWO STONES** |
| Sheet footnote | "Poured in sand where the thorn lies in the parting plane; lost to wax where it does not." |
| Primary side of the app | **Paths.** `cad::twist` sweeps (thorns, shoots, sepals), 3D `Operation::Sweep` (canes, ivy), `Operation::Loft` (leaflets), `PatternKind::{Ring, About, Mirror}` arrays, `Component::blend_mm` seam beads, `setting::Stamp` leaves with true outlines, and made settings as buds (`head.claw`, `head.bezel`, `seat.bur`, `SolidKind`). |
| Finish | Studio 18k yellow gold with darkened recesses. Sand rings are shown finished with their stones and exported as the pattern: no cuts, and a raised drill-start dot at each seat (`SeatPadLayer::mark_mm`, `setting::pattern_seat`). |
| One theme per ring | One plant per ring, carried face to palm. No ornament is scattered around the ring for its own sake (the Palisade rule). |

### The doctrine this collection adds: the in-plane thorn

The collection's casting argument comes from existing lessons and is a prediction until the E11 probe measures it:

1. A feature pours in a two-part mould if it lies wholly in the parting plane and is symmetric about it. Each half then faces its own mould half. The lessons already say this for beads and stones: "A bead row rides the crest line only" and "a gem column must run along the parting plane".
2. A thorn is a tube or cone whose axis and hook both lie in the parting plane. Its section is round, so its top half is a single-valued terrain over its plan. That holds however the tip hooks, provided the hook stays in the plane. Hooked rose and bramble prickles can therefore be poured in sand when they ride the crest line with their hooks turned round the ring.
3. The part's local frame puts x along the finger and z out of the metal (`Placement::frame_on`, `core/cad.rs:456`). A hook drawn in the part's local y–z plane therefore lies in the plane `z = across_mm`. With `across_mm = 0` on a symmetric band, that plane is the parting plane. `spin_deg` must stay at 0 or 180, and `tilt_deg`, which leans the part along the ring, keeps the axis in the plane.
4. A straight spur is `Operation::Extrude` of a circle with positive `draft_deg`, which narrows the far end (`core/cad.rs:59-65`), leaned by `tilt_deg`. The same rule applies to it.
5. A thorn off the plane pours only if its axis lies within (half-angle − `min_draft_deg`) of the pull. On a side face the pull is the face normal, so thorns standing straight out of a side face pour. The rest of this family goes to lost wax.

### Stamps on a signet table follow a companion rule

A signet table has no draft. Anything proud of it and off the parting line undercuts (Saurian lesson). Caiman's rule is that "height may not rise walking away from the parting line". For a stamp that sits on the parting line with walls along the surface normal, that means:

- Every straight line along the finger must cut the outline in one interval that contains z = 0.
- Equivalently, both margins are single-valued functions of distance round the ring.

A holly margin whose spines lean toward the leaf tip satisfies this. Spines hooked back toward the stalk do not. Caiman's pointed scutes are a special case. E3 adds the check as `Stamp::parting_monotone`.

### How the eight differ

| # | Ring | Plant | Base | Process | Stones | Silhouette | Path construction on show |
|---|---|---|---|---|---|---|---|
| 1 | Rubus | bramble | Keyframed LowDome 7.0 × 3.4, squared flanks | Sand | 0 | Knuckled cane with 13 hooked prickles down its back | In-plane twisted-sweep prickles as ring arrays; side-face runner (`CurveLayer`) carrying leaf stamps |
| 2 | Sentis | briar | CAD only: four swept canes on a torus liner | Lost wax | 1 ruby, 4.0 round | Open thicket with three shoots at the crown | 3D polyline sweeps, arrayed thorns, stone seated on a part's face |
| 3 | Rosa mortua | dog rose | `ShankKind::Bypass`, HighDome 3.6 × 2.7 | Lost wax | ruby pear 7 × 5; garnet oval cabochon 6 × 4.5 | A stem that passes itself: bud on one tip, hip on the other | Thorns along the wandering arm crests, sepal claws, lofted leaflets, `About` array of sepal wisps |
| 4 | Hedera | ivy | Uniform DShape 5.5 × 2.0 host band, plus a CAD vine | Lost wax | 7 black spinel cabochons, 2.2 | Polished host band strangled from edge to edge | 3D stem sweep, twisted petioles, rootlet path array, conforming leaf stamps, collet umbel |
| 5 | Ilex | holly | Factory **014 Heater** | Sand | 8 garnet cabochons | Heraldic shield | Parting line as the path: a leaf laid across the face along the parting line (heraldic "fesswise"), berries, a garland down the shoulders; sprays on the cheeks |
| 6 | Viscum | mistletoe | Factory **007 Quatrefoil** | Sand | 21 moonstone cabochons | Quatrefoil seal | Bench-cut intaglio seal (engraved into the face), graded berry runs on the parting line, forked-twig stamps, oak-bark walls |
| 7 | Prunus | blackthorn | Factory **009 Drop** | Sand | onyx pear cabochon 10 × 7; 4 diamonds, 1.3 | A black sloe, and a parting line bristling with spurs | Extruded-cone spurs in the parting plane (ring array, then a section mirror), painted bark, blossom stamps, bench-cut Ogham |
| 8 | Datura | thorn-apple (a nightshade) | Factory **016 Star** | Lost wax | 8 black spinel, 1.5 | A spiked capsule split open on a star calyx | Revolved capsule, rows of spines arrayed with `About`, V-slot valve cut, bur-set seeds |

Stones: 0 + 1 + 2 + 7 + 8 + 21 + 5 + 8 = **52**. Processes: four sand, four lost wax.

Plants covered: briar, bramble, rose stem, blackthorn, holly, mistletoe, nightshade (as Datura), and ivy. Hawthorn is held in reserve as a ninth. Its long straight spines are Prunus's construction, so it would repeat a ring.

### Traps common to the whole collection

- **Axial web** ("the verdict is radial"). No ring carves both side faces opposite each other. All side-face relief is raised, and the carves are bench cuts on the table or crest. The wall figure can therefore be trusted on all eight.
- **Two stages.** Veins, bays and the Ogham strokes are fine lines that the sand cannot hold. They are bench stamps (`Stamp::bench`, `core/setting.rs:919`) or `bench_only` layers (`core/field.rs:832`), which the verdict sets aside.
- **Stamps stand on the band as it was swept, before parts are joined** (`core/setting.rs:1418`, `core/parts.rs:182`). A leaf cannot be struck onto a CAD stem. A leaf on a stem must be a CAD part (a loft or twist); a leaf on the band is a stamp.

---

## 2. The eight rings

### 1. Rubus — the bramble cane

| | |
|---|---|
| Concept | A single bramble cane closed into a ring. It is knuckled at seven leaf nodes, and every hooked prickle along its back points the same way round the finger: the ring slides on with the grain and resists coming off. |
| Base | Procedural band, bore 18.6. `ProfileStyle::LowDome`, 7.0 × 3.4, `crown_mm` 0.6, then `BandProfile::flatten_sides()` (`core/profile.rs:548`). `edge_round_mm` 0.3, `comfort_fit_mm` 0.1. |
| Process | **Sand, two-part (Delft).** Every proud element is either on a side face or in the parting plane. |
| Stones | None. This is the collection's all-metal piece, as Lorica is for Reptilia. |
| Difficulty / risk | 2/5, low–medium. The only new claim is the in-plane prickle. |

**Face to palm**

- **Crest over the top 206°:** 13 hooked prickles in the parting plane. Large ones stand on the five upper nodes and small ones in pairs between them.
- **Both side faces, all the way round:** a fruiting runner arches between the nodes, carrying one bramble leaflet per internode and one drupelet blackberry per node. The band is symmetric (`SideFaces::is_even()`), so the pattern is mirrored.
- **Palm crest:** bare and polished, the worn part of the cane. The flank runner continues round it.
- **Bore:** comfort fit.

**Construction**

1. **Knuckles.** `ShankKind::Keyframes` (`core/profile.rs:1223`) with 14 `ShankKey`s (`core/profile.rs:1747`; the cap is 16), `amount` 1.0.
   - Nodes at θ = 90 + k·51.43°: `{width 1.05, thickness 1.15, crown 1.1}`.
   - Internodes at node + 25.71°: `{0.97, 0.93, 0.95}`.
   - Width changes stay within ±5%, so the flanks remain side faces under `SIDE_FACE_MIN_DRAFT_DEG` 80° (`core/field.rs:192`).
2. **Runner.** `Layer::Curve(CurveLayer)` (`core/curve.rs:62`): `WireProfile::Round` (a cosine dome, `core/curve.rs:44`), 0.8 wide × 0.45 high, `repeats_around` 7, `mirror_v` true.
   - Placed with `land_on_side_face(ctx, 0.7)` (`core/curve.rs:149`) and gated with `VGate::SideFaces(Both)` (`core/field.rs:420`).
   - The arches must crest at the nodes. Offset the control points' x by 0.75 of a cell, which works because x wraps at the cell edge, or add `CurveLayer::phase` (E1a).
3. **Leaflets.** 14 `setting::Stamp`s (`core/setting.rs:897`), one per internode per face.
   - Outline: E3 `Leaf::Bramble`, a serrate ovate leaflet 2.8 × 1.4 mm with teeth at least 0.36 mm.
   - `height_mm` 0.42, `sink_mm` 0.25, `draft_deg` 3, `along_pull` true.
   - Each sits where the runner has swung to the far edge, with `rot_deg` taken from the runner's tangent (`CurveLayer::sample_path`, `core/curve.rs:172`).
   - Every third leaflet is withered: a bite taken out of the outline, plus one cast hole. The hole is a cast `cut` stamp; the side face is the sanctioned place for carves.
4. **Blackberries.** `Layer::Decals` (`core/field.rs:1730`) of one SVG alpha, "Drupelet cluster": seven radial-gradient drupelets 0.75 mm across in a 2.3 mm berry. `size_mm` 2.3, `height_mm` 0.55, 7 decals per face at the nodes, gated to the side faces. Gradient SVG is the texture family that survives the detail floor (Serpentarium).
5. **Large prickle.** Feature #2: `Operation::Twist` (`core/cad.rs:76`).
   - Section: `Sketch::circle(0.55)` on the default local XY plane.
   - Path: a sketch on `Workplane { x: [0,1,0], y: [0,0,1] }`, the local tangent–radial plane. A 0.5 mm `Geometry::Line` runs radially out, then a 70° `Geometry::Arc` of radius 1.6 turns back round the ring.
   - `degrees` 0, `end_scale` 0.28, giving a 0.31 mm tip against Delft's 0.30 floor.
   - Component: `Placement::Ring { theta 347.14, across 0, height −0.35, spin 0 }`, `Attach::Join`, `Stage::Cast`, `blend_mm` 0.35. The seam bead is the flared prickle base (`core/blend.rs`).
6. **Large-prickle array.** #3: `Operation::Pattern { kind: Ring { count 5, span_deg 205.71 } }` (`core/cad/pattern.rs:37`). Copies land on the five upper nodes, and each is dropped onto the band at its own radius (`core/cad/pattern.rs:252`), which the knuckles require.
7. **Small prickles.** #4–#7: two small prickles (circle 0.40, arc 60° at radius 1.1, `end_scale` 0.38) at node + 17.14° and node + 34.29°. Each gets `Ring { count 4, span 154.29 }`.
8. **Veins.** 14 comb-shaped bench cut stamps, one per leaflet: `cut: true`, `bench: true`, `sink_mm` 0.12. Veins at 0.15 mm are below the sand's detail floor.

**What it showcases:** CAD parts poured in sand; ring arrays over a span; seam beads; a keyframed body; a side-face path carrying stamps along it; decals.

**Traps and how they are avoided**

- *Prickles off the crest line lock* ("A bead row rides the crest line only"; off-crest rails measured 5.8% at 29°). Avoided with `across 0`, crest bias 0, and spin fixed at 0 or 180.
- *Faces straddling the plane.* The verdict forgives a facet spanning the plane only as a chord (`chord_lean`, `core/castability/judge.rs:22`). An in-plane round section satisfies this, and the probe must confirm it.
- *Ridges along the ring on the crest lock* (melon lobes measured 8.2% at −22°). A bramble cane's angled ridges are not modelled on the crest. The only ridge running round the ring is the runner, and it sits on the side faces.
- *Two flanges make a valley.* Neither edge is raised.
- *Tip and teeth below the floor.* Tip end_scale 0.28 on a 1.1 mm section. Leaflet serrations are at least 0.36, the Zenith trail lesson. `dfm::stamp_finest_mm` (`core/dfm.rs:200`) reads the gaps as well as the ink.
- *Drupelets merge in the pour* ("casts softer than drawn", the Waves/Chevron precedent). They are sized so the berry still reads when softened. Check with the As-cast preview.
- *Wear:* no prickles on the palm crest, and every tip is at least 0.3 mm.

**Needs:** E11 prickle probe (blocking for the Castable claim), E3 bramble leaflet, E1a `CurveLayer::phase` (optional), G1 stamp nodes.

**Template:** band, shank-keyframe, `layer.curve` and `layer.decals` nodes lift as they stand. The six CAD features lift as a `cad.feature` chain (`graph/nodes/cad.rs:254`). The stamps currently lift as one `/stamps` patch (`graph/lift.rs:437-450`); G1 replaces it. Exposed controls: width, thickness, keyframe `amount`, and prickle count. The count is a script node building the Pattern JSON into `cad.feature`'s `operation` pin (`graph/nodes/cad.rs:32-39`). The graph is small, around 0.1 MB with one SVG.

---

### 2. Sentis — the briar thicket

| | |
|---|---|
| Concept | Four briar canes wound into an open thicket round the finger. They cross over and under in a three-fold rhythm with thorns all along them, and three cut shoots bristle up at the crown. Deep in the tangle a single ruby rose-hip is gripped by four thorn-claws. This is the Sleeping Beauty hedge at ring scale. |
| Base | **CAD only**: no `Operation::Band`, so `Document::replaces_band()` holds (`core/cad.rs:1315`) and overlapping parts are united by `parts::assembled` (`core/parts.rs:471`). Bore 18.6. |
| Process | **Lost wax.** No parting plane clears a weave ("a true helix locks"), and the thorns point every way. |
| Stones | 1 ruby, 4.0 round, in a four-claw head. |
| Difficulty / risk | 3/5, medium. The risk is csg robustness where many sweeps cross. |

**Face to palm:** the thicket runs all the way round, densest and tallest at the crown, where the shoots and the hip stand. It thins toward the palm and has no shoots there. The bore is a smooth torus liner hidden inside the tangle.

**Construction**

1. **Heartwood liner.** #1: `Operation::Torus { major 9.85, minor 0.55 }`, so the tangle never touches the finger.
2. **Canes.** #2–#9: `Operation::Sweep { sketch: Sketch::circle(0.72), path }` (`core/cad.rs:72`). There are four canes, each built as two 200° halves that overlap by 20° at both joins. Two halves are needed because Sweep passes `closed: false` (`core/cad.rs:1868`), and each half has 128 stations, the cap (`core/cad.rs:1850`).
   - Path of cane i: r(θ) = 10.85 + 0.55·cos(3θ + iπ/2), z(θ) = 2.1·sin(3θ + iπ/2).
   - Opposite canes cross 1.1 mm apart radially, so each crossing is a definite 0.34 mm overlap, never a tangency.
   - Inner envelope 9.58, liner to 10.4.
3. **Thorns.** 12 hooked `Operation::Twist` prickles on one 120° period: base 0.40, 1.3 mm long, `end_scale` 0.4.
   - Each takes `Placement::Ring { theta, across: z_cane, height: r_cane + 0.72 − 0.25 − (bore + thickness) }`. With no band surface, `frame_on` falls back to the analytic `frame()` (`core/cad.rs:424`).
   - Each is then arrayed with `Pattern::Ring { count 3 }`, giving 36 thorns. The thorns are separate small solids, so no copy overlaps another inside one Pattern part.
4. **Shoots.** 3 × `Operation::Twist`.
   - Section: a five-point star sketch 1.2 mm across; briar canes are angled, and the twist only shows on a non-round section.
   - Path: a 2.6 mm planar arc in the section plane.
   - `degrees` 150, `end_scale` 0.5, placed at θ 72, 90 and 108.
5. **Hip cup.** `Operation::Revolve` (`core/cad.rs:66`) of an urn profile, Ø 4.6 × 3.0 with a flat top, placed `Ring { 90, 0, h }` among the canes.
6. **Ruby.** `builders::stone_feature` (`core/cad/builders.rs:916`) carrying a `FaceSeat` on the cup's planar top (`core/cad.rs:504`, `:576`). Then `head.claw` with `prongs` 4 (E6 thorn tips) and `seat.bur`, both `Stage::Cast`.

**What it showcases:** a whole ring built from CAD features; 3D sweeps; csg union of overlapping parts; a stone on a part's face; claws.

**Traps**

- Tangent contacts become `Snag::Degenerate` and are nudged up to 8 times ("Booleans are exact in topology"). Every crossing is designed as a real overlap.
- Seam beads are not laid in a CAD-only assembly: `assembled` never reads `blend_mm` (`core/parts.rs:471-510`). Without E7 the thorn roots meet the canes crisp.
- 256-feature cap (`core/cad.rs:1294`): the plan uses about 33 features.
- Wax floors (0.5 mm section, 0.15 mm detail): canes are Ø 1.44, thorn bases Ø 0.8 and tips 0.32.
- The claw feet must reach metal. They stand on the cup, which is a kernel part with a planar face, rather than on a cane.

**Needs:** E7 beads in CAD-only assembly. E2 closed and tapered star-section canes, an upgrade from constant circles. E1 `Along { Feature }` would replace 24 thorn features with two. E6 thorn-tip claws. E8 path nodes, to expose strands, crossings and amplitude.

**Template:** a `cad.source` → `cad.feature` chain of about 33 nodes, which lifts byte for byte (`graph/lift.rs:407`). Today each cane path is a 128-point JSON array. A `script` node can generate it and feed the `operation` pin. With E8 it becomes a `path.wreath` node exposing "Strands", "Crossings", "Wander" and "Cane". The graph is tens of KB.

---

### 3. Rosa mortua — the dead rose

| | |
|---|---|
| Concept | A single rose stem closes round the finger and passes itself. One arm ends in a ruby bud that never opened, held in five sepals. The other ends in a garnet hip still crowned by its dried sepals. Hooked prickles run down both arms, and a withered leaf hangs off one. |
| Base | Procedural `ShankKind::Bypass` (`core/profile.rs:1220`; `BYPASS_OFFSET` 0.45 and `BYPASS_TIP_DEG` 35 at `core/profile.rs:2321-2323`). `ProfileStyle::HighDome`, 3.6 × 2.7, a round stem. Bore 18.6. |
| Process | **Lost wax.** The sepal claws and wisps overhang. The prickles ride arm crests that slide along the finger ("the crest wanders in v there", 3% at 50° in the configurator lesson). |
| Stones | Ruby pear 7 × 5 (bud) and garnet oval cabochon 6 × 4.5 (hip). |
| Difficulty / risk | 3/5, medium. |

**Face to palm:** the bud and hip sit at the two arm tips flanking the top. Seven prickles on each arm grade smaller toward the palm. A three-leaflet leaf springs from arm A below the bud, and a withered leaf hangs from arm B. The palm is the plain stem, and the bore is smooth.

**Construction**

1. **Bud.** `stone_feature(Pear 7 × 5)` placed `Ring { theta: tip_A, across: +arm offset, height: builders::stand_off_mm, tilt 20 }`, so the bud nods. Then `head.claw` with `prongs` 5 (E6 **Sepal** style) and `seat.bur`, all Cast.
2. **Hip.** `stone_feature(Gem::cabochon(Oval, 4.5), l 6)` at tip B with `head.bezel` (`wall_mm` 0.45, `lip` 0.35).
3. **Dried sepal crown.** A `Twist` wisp: lens section 0.7 × 0.5 (above the 0.5 mm fill floor), a planar arc curling out 100°, `degrees` 60, `end_scale` 0.6. Arrayed with `PatternKind::About { part: hip stone, count 5 }` (`core/cad/pattern.rs:45`), round the stone's own axis.
4. **Prickles.** 14 `Twist` prickles, placed individually at `(θ_k, across_k)` read off the arm crest. The crest comes from `profile::bypass_span` (`core/profile.rs:2351`) through `RingDesign::modulation_at`. `spin` alternates ±25° so the hooks lean down the stem. `blend_mm` 0.25.
5. **Leaf.** Three leaflets, each an `Operation::Loft` (`core/cad.rs:82`, 2–32 sections) of five lens sketches along a curling spine: widths 0.6 / 1.9 / 2.2 / 1.4 / 0.5, each section turned progressively up to 35° so the leaflet cups. A twisted petiole of circle 0.36 joins them.
6. **Withered leaf.** A lens `Twist` with `degrees` 70, minus a `Cylinder` bite through `Operation::Boolean { Subtract }`. The twist is a mesh, so the boolean runs through csg (`core/cad.rs:1916`), not the kernel.

**What it showcases:** a procedural shank carrying CAD parts; arrays round a stone; lofts; csg booleans inside the feature tree; claws and bezels; seam beads.

**Traps**

- *A bypass crossing needs room under a stone* (collection3: a 0.67 mm wedge on a 4.5 mm band, clean at 5.0). Both stones sit at least 25° from the crossing.
- *Folds in the claws* ("a swept tube folds through itself wherever its rings tilt more than they stand apart"). The sepal claws keep the straight–arc–straight path at a bend radius of at least 0.8 wire.
- *Wax floors:* wisps and petioles at least 0.6 thick.
- *Prickles on a sliding crest:* place each one from the modulated section, never from a fixed `across`.

**Needs:** E6 sepal claws (the bud is the point of the ring; a plain five-prong claw is the fallback). E1 `Along { Crest }` would replace the 14 placements with two arrays. E2 scale law for the withered leaf's taper. E5b relative placement, so the wisps follow the hip if the stone moves ("a head moves by its stone").

**Template:** band and shank nodes plus a chain of about 30 `cad.feature` nodes. Exposed: stem width and thickness, and bud and hip sizes, since the stone builders' `params` are JSON pins. Prickle placement stays in the feature tree until E1 lands.

---

### 4. Hedera — the strangling ivy

| | |
|---|---|
| Concept | A plain polished court band, the host, is strangled by an ivy stem climbing it edge to edge. The stem grips with rows of aerial rootlets. Lobed juvenile leaves follow the creeping stem, heart-shaped adult leaves crowd the crown, and a black berry umbel crowns it. The botany is correct: ivy fruits only on its adult, unlobed growth. |
| Base | Procedural `ShankKind::Uniform`, `ProfileStyle::DShape`, 5.5 × 2.0, `comfort_fit_mm` 0.15, bore 18.6, polished. |
| Process | **Lost wax.** The stem crosses the crown diagonally and wraps both edges (off-crest rails measured 1.1% at −31°), and the rootlets and petioles overhang. |
| Stones | 7 black spinel round cabochons, 2.2, in collets. |
| Difficulty / risk | 3/5, medium. |

**Face to palm**

- **Crown:** the umbel and three adult heart leaves.
- **Shoulders:** the stem crosses edge to edge twice, with lobed juvenile leaves on petioles.
- **Palm:** the stem's cut end, rootlets, one juvenile leaf.
- **Side faces:** the stem rolls over each edge onto them for a few millimetres.
- **Bore:** bare host metal.

**Construction**

1. **Stem.** Two `Operation::Sweep` features of circle 0.62, 128 stations each, overlapping 12°.
   - Path: θ from 250° over 450°, with z(θ) = 2.0·sin(1.5(θ − 250°)), riding the outer surface at r_surface(θ, z) + 0.25 so 40% of the stem is sunk.
   - Near the edges the path steps out over the edge fillet.
   - `Attach::Join`, `blend_mm` 0.3; a seam bead is laid because this is a procedural band.
2. **Rootlets.** A tiny cone (`Extrude` of circle 0.22, height 0.6, `draft_deg` 6) under the stem, arrayed along the stem path (E1 `Along { Feature: stem, pitch 0.6, alternate }`), about 80 copies, under `MAX_PATTERN_COUNT` 120 (`core/cad/pattern.rs:20`).
3. **Leaves.** 9 leaf stamps: 6 juvenile (E3 `Leaf::IvyJuvenile`, five-lobed palmate, 3.5–4.5 mm) and 3 adult (`Leaf::IvyAdult`, cordate, 5.0 mm).
   - `height_mm` 0.45, `sink_mm` 0.2, raised midrib stamp at 0.6.
   - Cast cut vein stamps, allowed under lost wax at a 0.15 floor.
   - Every leaf lies within the outer surface's footprint; `Stamp::stand` refuses one that "runs off the edge of the surface it stands on".
4. **Edge leaves.** Two leaves curl over the band edge as CAD parts: E2 lens twist with a leaf scale law, or a `Loft` of lens sections today.
5. **Petioles.** 9 `Twist` features, circle 0.36 along a 60–90° planar arc from the stem to each leaf base.
6. **Umbel.** 7 × `stone_feature(Gem::cabochon(Round, 2.2))` placed in a dome over the crown, with `Ring { 90, across_k, height_k }` offsets. Each gets `head.bezel` (wall 0.4, lip 0.3). Stalks are `Sweep`s of circle 0.35 between two points: from the umbel point to each collet base, the same construction as `cad::examples` "gallery" (`core/cad/examples.rs:159-203`).

**What it showcases:** a 3D sweep on a procedural band; many CAD parts meeting one band through csg with seam beads; conforming stamps; a cluster of collets.

**Traps**

- Leaves cannot sit on the stem, because stamps stand on the band as swept.
- Section floor: stalks and petioles are at least Ø 0.7 mm against the investment floor of 0.5.
- A collet on a stalk: `BEZEL_SINK_MM` 0.25 (`core/cad/builders.rs:42`) expects metal under the collet's base, so each stalk must end inside it.
- csg load: about 30 joined parts in one cluster. Oriel's 37 made stones took 4.3 s at export. Budget the export build accordingly.

**Needs:** E1 (rootlets). E2 (edge leaves and the stem's tapered growing tip). E3 ivy outlines. G1.

**Template:** band nodes; the stamps as a patch until G1; a chain of about 35 `cad.feature` nodes. Exposed: host width and thickness, and berry size, which feeds the 7 stone params through one script. The stem path is a script node → `operation` (E8 `path.climb` later).

---

### 5. Ilex — the Holly King

| | |
|---|---|
| Concept | The Holly King's shield. On the face, a holly leaf lies fesswise (heraldic: across the shield) between two garnet berries on the parting line. Holly sprays with berries fill both cheeks, and a garland of small holly leaves rides the parting line down both shoulders. The palm is bare, like holly's smooth bark. |
| Base | **Factory 014 Heater**, 15 × 18 (`core/imported_base.rs:1108`). Loaded with `ImportedBase::attach` (`core/imported_base.rs:472`), passed through `sand_stock` (`ex/common/sand_stock.rs:53`) with `sand_envelope` true. The `SurfaceChart` is taken from its own profile, as in `ex/stock_masterworks.rs:415-512`. Bore 18.6. |
| Process | **Sand (Delft).** Face relief is on the parting line and passes the monotone-margin rule; cheek relief faces the pull. |
| Stones | 8 garnet round cabochons: 2 × 2.0 on the face, 6 × 1.8 on the cheeks. |
| Difficulty / risk | 2/5, low–medium. |

**Face to palm**

- **Face:** the fesswise leaf and two berries on the line; the field polished.
- **Cheeks:** holly sprays of three leaves and three berries each.
- **Shoulders:** a garland of four leaves per side on the parting line, graded smaller toward the palm.
- **Palm:** bare and polished.
- **Bore:** the stock's own.

**Construction**

1. **Stock.** Base as `ex/stock_masterworks.rs:415` with slug `ilex`. The `Atlas` (`:40`) and `Hide` (`:1573`) are built from this stock. These move into core as E4.
2. **Face leaf.** `Stamp` with outline `Leaf::Holly { len 8.6, wid 5.0, spines 6, lean 20° }` (E3); its bays are at least 0.45 wide.
   - Placed at `hide.crest_at(0.0)` (`:1645`) with `rot_deg` 0 (along the ring).
   - `height_mm` 0.55, `sink_mm` 0.3, `draft_deg` 4, `along_pull` false. These are Caiman's horn numbers.
   - `Stamp::solid` splits the outline where it crosses the plane (`core/setting.rs:1006`).
3. **Midrib.** A lens stamp 8.0 × 0.55 on the line at `height_mm` 0.8.
4. **Lateral veins.** A bench cut comb stamp: `cut`, `bench`, `height_mm` 1.1, `sink_mm` −0.35, so the floor stands 0.2 mm into the leaf (the crescent recipe, `ex/stock_masterworks.rs:1469`).
5. **Face berries.** 2 × `SeatPadLayer` (`core/field.rs:1109`) at along ±5.8 on the line, from `flush_seat` (`ex/stock_masterworks.rs:1382`).
   - `GypsyMound`, mound Ø gem + 0.8 = 2.8, so it ends 0.3 mm inside the face's end.
   - `metal_true`, `solid: Flush`, `mark_mm` 0.6.
6. **Cheek sprays.** Per cheek: 3 holly leaf stamps (5.0 × 2.6, `along_pull` true, `height_mm` 0.5, draft 4). Plus 3 garnet cabochons in gypsy mounds at the spray's heart; "side face = castable by construction" (`core/stones.rs`).
7. **Shoulder garland.** 4 leaves per shoulder at along 8.5, 12.5, 16.5 and 20.5 mm on the line, graded 4.2 × 2.4 down to 3.2 × 1.8. `rot_deg` alternates 0 and 180; spines lean toward each leaf's own tip. No leaf comes within 1 mm of the fold where the parting line turns over the head's end wall.

**What it showcases:** the parting line used as a path; figurative stamps poured in sand on a table with no draft; flush cabochons; the stock workflow.

**Traps**

- *Anything proud off the parting line undercuts* (Saurian). All face relief is on the line, and `Stamp::parting_monotone` (E3) gates it.
- *A stamp on the crest needs the crest exactly* (0.023 mm, Caiman). Placement uses `crest_at`'s interpolated `v`, never the nearest atlas row.
- *Under a stamp the surface may vary only across the band* (phantoms of 0.03–0.07 mm). The table is flat. The garland stays short and off the fold; any leaf that still reports phantoms becomes a bench cut.
- *Cheeks lean* (0.35 mm tuck): cheek stamps use `along_pull`.
- *A cast dot on the table must ride the parting line* (a dot 2 mm off it leaned 14°). The face berries sit on the line. The mound skirts are at least 0.45, since a skirt is a seat's finest DFM feature (0.4 measured 0.34 against a 0.35 floor).
- *Crescent lesson.* The bays between spines are concave. On the face they are safe only because of the monotone-margin rule, which the probe must confirm. The fallback is E3 `hull_and_bays`: cast the spine-tip hull and bench-cut the bays, as `crescent_cutter` does (`core/setting.rs:1091`).

**Needs:** E3 holly outline plus `parting_monotone` and `hull_and_bays`; E4 `Hide` in core; E11 parting-stamp probe; G1.

**Template:** imported-base lift (the "Stock body" and "Stock face dimensions" nodes, `graph/lift.rs:336-382`), seats as `layer.seat` nodes, stamps as a patch until G1. The stock JSON is about 0.4 MB. No painted alphas are needed, so it is far lighter than Caiman's 12.7 MB.

---

### 6. Viscum — the golden bough

| | |
|---|---|
| Concept | Aeneas's golden bough, his passport into the underworld, which Frazer identified as mistletoe. The quatrefoil face is a seal: the mistletoe's two crossed leaf pairs are cut in intaglio (engraved into the face), one leaf per lobe, with three moonstone berries in the fork on the parting line. Forked twigs with paired leaves fill the cheeks. Moonstone berries run down both shoulders, graded along the parting line. The shank is the host oak, with bark on its walls. |
| Base | **Factory 007 Quatrefoil**, 16 × 18.5 (`core/imported_base.rs:1101`), sand stock, bore 18.6. |
| Process | **Sand.** The face's lobed figure is cut at the bench, which is the only way it can sit off the line. The berries ride the line, and cheek and wall relief faces the pull. |
| Stones | 21 moonstone round cabochons: 3 × 2.5 on the face, 4 × 1.6 on the cheeks, 14 graded 2.0 → 1.3 on the shoulders. |
| Difficulty / risk | 3/5, medium. There are many seats and stamps on the stock, so the build time is heavy. |

**Face to palm**

- **Face:** bench-cut intaglio leaves and fork, with three proud berries on the line.
- **Cheeks:** Y-forked twigs, three opposite leaf pairs, and two berries in the fork.
- **Shoulders:** graded berry runs on the line.
- **Shank and palm walls:** oak bark.
- **Palm crest:** bare.

**Construction**

1. **Intaglio leaves.** 4 bench cut stamps, outline `Leaf::Mistletoe` (6.0 × 2.2 strap leaves), `rot_deg` ±45° and ±135° into the lobes. `cut: true`, `bench: true`, `height_mm` 1.0, `sink_mm` 0.25. Plus a Y-fork cut (`Outline::fork`, E3) joining them at the centre.
2. **Face berries.** 3 moonstone cabochons of 2.5 in gypsy mounds 3.4 across on the line, at along −3.8, 0 and +3.8. The 0.4 mm gaps keep them from merging ("gypsy-skirted seats at column pitch merge into one ridge").
3. **Cheeks.** A Y-fork twig stamp and three spatulate leaf pairs (3.6 × 1.4; entire margins, so convex) with `along_pull`. Two moonstone cabochons of 1.6 flush in the fork.
4. **Shoulder runs.** 7 cabochons per shoulder, graded 2.0 → 1.3 (taper 0.35).
   - Stations at an equal metal bridge of 0.45 mm along `Hide::along`. This follows the graded-run principle that holds the bridge constant, not the angle.
   - Each is a `SeatPadLayer` at `crest_at(along)`, mound Ø gem + 0.9, `solid: Flush`, `mark_mm`.
   - `SeatRunLayer` cannot do this: its `v` is fixed and the stock's crest wanders (`core/field.rs:1917`). This needs E1 or E4.
5. **Oak bark.** An `Atlas` + `Hide` layer painted on the walls only: furrows running round the ring, relief 0.35. Joined with `Blend::Max` through `draft_clamp` (`ex/stock_masterworks.rs:1303`, `hide_layer` `:1716`). This is Caiman's idiom.

**What it showcases:** a bench seal as a first-class construction; graded stones along a painted-free path; host and parasite as one theme.

**Traps**

- *Face off-line relief locks.* The seal is cut after the pour, and bench stamps never enter a pattern (`Stamp::bench`).
- *A bead row rides the crest line only.* Every berry is on the line.
- *Ridges along the ring on a crest lock* (melon lobes). The bark stays on the walls; the palm crest is bare.
- *Cheek forks are concave crotches.* They sit on the cheeks, which face the pull. Keep them at least 1 mm below the rim, away from the parting line (crescent lesson).
- *Build time.* 21 seats + 14 stamps on a stock at 1536 × 448. Oriel's 37 stones took 4.3 s, so expect several seconds at export. This is where E9 progress matters.

**Needs:** E3 (mistletoe leaf, fork); E4 `Hide`; E1 `Along { Parting }` for seats (or `SeatRunLayer::follow_parting`); G1.

**Template:** imported-base lift; 24 `layer.seat` nodes (or a single seat-run node after E1); the painted bark alpha embedded, about 3 MB; stamps as a patch until G1.

---

### 7. Prunus — Straif, the blackthorn

| | |
|---|---|
| Concept | The witches' tree. A black sloe sits flush in the drop-shaped face. The parting line bristles with long straight blackthorn spurs down both shoulders. The walls are black-bark furrows starred with white blossom, since blackthorn flowers on bare black wood. Straif, the Ogham letter for blackthorn, is cut on the palm. |
| Base | **Factory 009 Drop**, 17 × 22.3 (`core/imported_base.rs:1103`), sand stock, bore 18.6. |
| Process | **Sand.** The spurs lie in the parting plane, the sloe sits in a flush seat on the line, and the bark and blossoms are on the walls. |
| Stones | Black onyx pear cabochon 10 × 7 (the sloe) and 4 white diamonds of 1.3 (blossom centres). |
| Difficulty / risk | 3/5, medium. Spurs on stock are an untested pour. |

**Face to palm**

- **Face:** the sloe, pointing along the finger to echo the drop, with a bench-engraved twig silhouette round it.
- **Shoulders:** 7 spurs per side on the parting line, leaning alternately forward and back.
- **Cheeks and shank walls:** black bark, with two blossoms per cheek.
- **Palm crest:** Straif, bench-cut.

**Construction**

1. **Sloe.** A `SeatPadLayer` Boss at `skin.on_face(0, 0)` (`ex/stock_masterworks.rs:1287`).
   - `gem` is `Gem::cabochon(Pear, 7.0)` with `l_mm` 10, `rot_deg` 90.
   - `height_mm` 0.9, `metal_true`, `solid: Flush`, `through`, `mark_mm` 1.0.
   - This is Caiman's emerald recipe (`ex/stock_masterworks.rs:2008-2026`).
2. **Long spur.** `Operation::Extrude { sketch: circle(0.5), height_mm: 2.6, draft_deg: 7 }`, giving a tip of Ø 0.36.
   - Component: `Placement::Ring { theta: first station, across 0, height −0.4, tilt +38 }`, Join, Cast, `blend_mm` 0.3.
   - `Pattern::Ring { count 4, span 36 }` along shoulder A.
3. **Short spur.** `Extrude(circle 0.5, 1.8, draft 8)` at `tilt −30`, pointing back, with `Pattern::Ring { count 3, span 30 }` interleaved.
4. **Other shoulder.** Both patterns mirrored with `Pattern { kind: Mirror { plane: Section { theta 90 } } }` (`core/cad/pattern.rs:52-62`).
5. **Bark.** Black-bark furrows round the ring with lenticel dashes, painted by `Atlas` + `Hide` on the walls only, at 0.35 relief through `draft_clamp`.
6. **Blossoms.** Per cheek, 2 × five-petal blossom stamps (E3 `Outline::blossom(5, 3.4)`) with `along_pull` and `height_mm` 0.4. Each has a 1.3 diamond centre: `SeatPadLayer` GypsyMound, `solid: Bead`, `mark_mm` 0.6.
7. **Straif.** One bench cut stamp: a comb-shaped outline with a 6 mm stem line on the palm's parting line and five diagonal strokes 1.6 × 0.35 at 45°.
8. **Face engraving.** Bench cut stamps of a blackthorn twig encircling the sloe.

**What it showcases:** straight parts poured in sand; the section mirror; painted skin as a supporting texture, not the subject; a single dramatic stone.

**Traps**

- *A spur whose axis tips off the plane undercuts.* A stock crest's normal tips a few degrees off horizontal. Stamps already correct for this (the straddle fix, `core/setting.rs:948-957`, 0.05 mm on 017). **Parts do not**, because `frame_on` uses the raw normal (`core/cad.rs:456-495`). This needs E5 `level`.
- *The fold* (Caiman: 0.06 mm): no spur within 1 mm of where the parting line turns over the end wall.
- *Raised Ogham strokes lock.* A diagonal stroke's ends fail the monotone-margin rule, so Straif is a bench cut.
- *Bark on the crest* (melon lobes) is kept to the walls.
- *Spur tips* are at least 0.3 mm (draft chosen for this).
- *Pattern arrays cannot grade sizes.* Motions are rigid (`core/cad/pattern.rs:161-193`), so graded spur lengths need two sources, or E1's scale.

**Needs:** E5 `level` (blocking for a clean sand verdict); E4; E3 blossom; E11 prickle probe extended to stock; G1.

**Template:** imported-base lift; `layer.seat` × 5; the bark alpha embedded; a chain of 5 `cad.feature` nodes (two spurs, two rings, one mirror); stamps as a patch until G1.

---

### 8. Datura — the thorn-apple

| | |
|---|---|
| Concept | The devil's thorn-apple, a thorned nightshade. A spined seed capsule stands on the star-shaped calyx, which is the factory star face. It is split into four gaping valves with black seeds spilling in the splits. Sinuate datura leaves climb the shoulders, and the palm is the smooth stem. It is Logan's heaviest, most sculptural register, in plant form. |
| Base | **Factory 016 Star**, 19 × 20 (`core/imported_base.rs:1110`), taken as the investment stock with no sand envelope. The recipe is set as `base()` sets it for non-sand stock: `CastProcess::LostWax`, `min_draft` 0, `min_detail` 0.15, `min_section` 0.8 (`ex/stock_masterworks.rs:477-483`). |
| Process | **Lost wax.** Spines radiate from a dome. Under two-part sand only spines within (half-angle − draft) of the pull would release, about a quarter of them. |
| Stones | 8 black spinel rounds of 1.5 (the seeds). |
| Difficulty / risk | 4/5, medium–high. Spine placement must survive a resize. |

**Face to palm**

- **Head:** the capsule on the star.
- **Shoulders and cheeks:** 2 leaves per side with cast veins.
- **Palm:** polished stem.
- **Bore:** the stock's own.

**Construction**

1. **Capsule.** `Operation::Revolve` of a half-egg `Bezier` sketch (base radius 4.2, height 5.6) through 360°, placed `Ring { 90, 0, −0.6 }`, Join, Cast, `blend_mm` 0.5. The bead flares the capsule into the calyx.
2. **Valves.** One `Operation::Extrude` of a cross-shaped slot sketch (two 0.9 mm slots, 9 mm long).
   - Placed over the capsule top and extruded −3.4 with `draft_deg` 6, so the slot narrows downward and the valves gape.
   - `Attach::Cut`. Cuts resolve after joins (`core/parts.rs:175`), so the cut goes through the joined capsule and stops 2.4 mm above the table.
3. **Spines.** 4 rows at latitudes 18°, 38°, 58° and 76°.
   - Each spine is `Extrude(circle 0.42, h 1.5 → 1.1, draft 9°)`, tip Ø 0.36.
   - Each row is `Pattern::About { part: capsule, count 12, 12, 10, 6 }` at a 15° phase, so no spine lands on a slot: 40 spines.
   - The row source must be placed **in the capsule's frame**, which needs E5b. Today it takes a world `Transform` computed from the capsule's seated frame, and that breaks on resize.
4. **Seeds.** 8 × `stone_feature(Round 1.5)` in the slot walls, each with `seat.bur` (Cut, Cast).
5. **Leaves.** Per cheek, 2 × `Leaf::Datura` stamps (ovate, coarse sinuate teeth, 7.0 × 4.2), `along_pull`, `height_mm` 0.5. Each has a raised midrib stamp and cast cut veins (a 0.15 floor under lost wax).

**What it showcases:** `Revolve`; `About` arrays; a cut part through a joined part; seeds set by the bur; CAD sculpture on factory stock.

**Traps**

- *Kernel booleans on freeform bodies* ("Faceted operands past 500 faces are refused", `core/cad.rs:2020`). The valves are a `Cut` part resolved by csg, never an `Operation::Boolean`.
- *Weight.* The capsule is about 207 mm³, roughly 3.2 g in 18k. It stays solid; `Shell` supports only boxes, cylinders and spheres (`core/cad.rs:1979`).
- *Wear.* The spines are the point of the design. Tips are at least 0.3 mm, and there are none on the palm.
- *Resize.* Without E5b the spines stay where they were while the capsule re-seats.

**Needs:** E5b `Placement::Relative` (blocking for a robust template); E3 datura leaf; G1.

**Template:** imported-base lift plus a chain of about 30 `cad.feature` nodes. Exposed: capsule height and diameter, spine length, and seed size. No painted alpha, so the graph is under 1 MB.

---

## 3. Enablers, ranked

"Shared" marks capabilities that the other three collections will also need.

| # | Enabler | Rings | Shared | Blocking? |
|---|---|---|---|---|
| E11 | Probes: in-plane prickle in sand; parting-line stamp on stock | Rubus, Ilex, Prunus | no | **Yes** for any Castable claim |
| E5 | `Placement` gains `level`, `Relative`, `Side` | Prunus, Datura, Rosa | **shared** | Prunus (level), Datura (relative) |
| E1 | `PatternKind::Along` path arrays, with scale | Sentis, Rosa, Hedera, Viscum, Prunus, Rubus | **shared** | Hedera (rootlets); elsewhere it replaces many features |
| E3 | Botanical outline family, `parting_monotone`, `hull_and_bays` | all but Sentis and Rosa | outline infra **shared** | Ilex, Viscum, Prunus, Datura, Hedera |
| G1 | Stamp graph nodes; lift emits them | 6 rings | **shared** | No (the patch fallback works) |
| E4 | `Hide` and `Atlas` promoted into core | Ilex, Viscum, Prunus, Datura | **shared** | No (example code works) |
| E2 | Twisted sweep along 3D paths with a scale law; closed `Sweep` | Sentis, Hedera, Rosa | **shared** | No (planar and constant fallbacks) |
| E6 | Claw styles: Thorn, Sepal | Sentis, Rosa | **shared** (talons) | No (Wire fallback) |
| E7 | Seam beads in CAD-only assembly | Sentis | **shared** | No |
| E8 | Graph path nodes; `cad.feature` component pin | Sentis, Hedera, Rubus | **shared** | No |
| E9 | Build progress for async template open | all | **shared** | No; serves the async-loading request |
| E10 | Generic collection tooling and a stone material table | all | **shared** | No |

**E11: the probes (do these first).**
- `ex/prickle_probe.rs` sweeps a crest prickle on a LowDome and on the 009 and 017 stock, reporting `judged_field_report` (`core/castability/judge.rs:118`) plus `mf::release::analyze` (`core/manufacturing/release.rs:211`) obstructions against:
  - tilt of the hook plane off the parting plane: 0, 1, 2, 5, 10°;
  - section facet count;
  - `blend_mm`: 0, 0.2, 0.35;
  - spin: 0, 90, 180.
  - Expected: 0.000% at 0° tilt, growing with tilt. That curve sets the `level` tolerance.
- `ex/parting_stamp_probe.rs` puts a holly leaf on a stock table and records obstructions against spine lean (+20° down to −20°) and bay depth. It pins the monotone rule and the crescent fallback.

**E5: placement.** Sketch against `core/cad.rs:385`:

```rust
pub enum Placement {
    Free,
    Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg,
           #[serde(default)] level: bool },          // square z to the pull's plane, as Stamp::frame does
    Relative { part: Id, at: [f64; 3], rotation_deg: [f64; 3] },  // in `part`'s seated frame; part joins sources()
    Side { theta_deg, radius_mm, face: SideFacePick, height_mm, spin_deg, tilt_deg }, // z = ±Z on a side face
}
```

- `level` zeroes `normal[2]` when the frame straddles or rides the plane. It reuses `setting.rs:946-957`.
- `Relative` fixes Datura's spines and Rosa's wisps on resize, and suits heads on shanks and horns on skulls.
- `Side` lets a part (a spur along ±Z, a boss) stand on a side face, which a radial `surface_hit` (`core/cad.rs:847`) cannot reach.

**E1: path arrays.** Sketch against `core/cad/pattern.rs:37`:

```rust
PatternKind::Along {
    path: AlongPath,   // Crest { from_deg, to_deg } | Parting | Feature { id } | Chart { points: Vec<[f64; 2]> } | Curve { layer: usize }
    count: u32, pitch_mm: Option<f64>, phase: f64,
    alternate_deg: f64,   // spin added on every other copy (alternate leaves and thorns)
    roll_deg: f64,        // spin per copy (137.5 for spiral phyllotaxis)
    scale: [f64; 2],      // start and end: graded spurs, prickles, rootlets
    level: bool,
}
```

- Frames come from the path tangent and the surface normal. `Crest` reads `RingDesign::modulation_at`, so it follows Bypass, Wave and Twist shanks.
- `Parting` reads E4 `Hide`. `Feature` reads a Sweep or Twist centreline.
- Scale needs non-rigid copies, so the motions must carry a scale (`Motion` is rigid today, `pattern.rs:161`).
- The same path source should drive seat pads (`SeatRunLayer` with `path: AlongPath`) and stamps (G1 `stamp.along`).
- E1a: `CurveLayer::phase` and `TilingLayer::phase` in cells. This is a two-line change that aligns runner arches with keyframe nodes.

**E3: botanical outlines.** A new `core/outline/botanical.rs`, in the spirit of `core/reptile.rs`:

```rust
pub enum LeafKind { Holly { spines, lean_deg }, Bramble, RoseLeaflet, IvyJuvenile { lobes }, IvyAdult,
                    Mistletoe, Blackthorn, Datura, Hawthorn { lobes } }
pub fn leaf(kind: LeafKind, len_mm: f64, wid_mm: f64, o: &LeafOpts) -> Vec<[f64; 2]>  // CCW, point every 0.1 mm
pub struct LeafOpts { pub withered: f64, pub bite: Option<([f64; 2], f64)>, pub min_tooth_mm: f64 }
pub fn fork(arm_deg: f64, stem_w: f64, arm_w: f64, len: f64) -> Vec<[f64; 2]>
pub fn blossom(petals: u32, dia_mm: f64) -> Vec<[f64; 2]>
pub fn vein_comb(outline: &[[f64; 2]], veins: u32, groove_mm: f64) -> Vec<[f64; 2]>  // one bench cutter
pub fn hull_and_bays(outline: &[[f64; 2]], margin: f64) -> (Vec<[f64; 2]>, Vec<Vec<[f64; 2]>>)  // generalises crescent_cutter
impl Stamp { pub fn parting_monotone(&self, design: &RingDesign) -> Result<(), Vec<usize>> }        // cheap, analytic
```

- Teeth are sized against `DraftSettings::min_detail_mm`, and `dfm::stamp_finest_mm` checks them.
- `hull_and_bays` uses `field::hull_defect`'s hull (`core/field.rs:3018`).
- For hand-drawn leaves, add `Stamp::outline_from_svg`: exact curves through `sketch::exchange::import_svg` (`core/sketch/exchange.rs:146`), flattened at 0.1 mm. `contour::trace` is a radial sweep (`core/contour.rs:42`) and loses the bays of a deep serration.

**G1: stamp graph nodes.** Today a design's stamps lift as one `design.set /stamps` array (`graph/lift.rs:437-450`), so no stamp is editable in the graph. Add:
- `stamp`: all `setting::Stamp` fields as pins;
- `stamp.outline.leaf`, `.moon`, `.blossom`, `.polygon`;
- `stamp.along`, which distributes a stamp over a chart path, a curve layer or the parting line, with tangent rotation and alternation;
- `design.stamps`, which collects them.

The lift should emit these nodes. This is shared by all four collections, since stamps are the figurative vocabulary.

**E4: `Hide` and `Atlas` in core.** Move `ex/stock_masterworks.rs:40-125` and `:1573-1663` to `core/imported_base/hide.rs`:

```rust
pub struct Hide { along, across, rim, wall, crest }
impl Hide {
    fn new(&FieldSurface) -> Self;
    fn crest_at(&self, along_mm: f64) -> (f64, f64);
    fn at(&self, theta, v) -> HideSample;
    fn folds(&self, min_turn_deg_per_mm: f64) -> Vec<f64>;
}
```

`folds` is Caiman's fold detector. Every signet collection places things on the parting line.

**E2: 3D twisted sweep with a scale law.** Against `core/cad.rs:76` and `core/cad/twist.rs:361`:

```rust
Operation::Twist { sketch: Profile, path: TwistPath, degrees: f64, end_scale: f64,
                   #[serde(default)] scale: Vec<[f64; 2]>,   // (share, scale), monotone share; empty means linear to end_scale
                   #[serde(default)] closed: bool }
pub enum TwistPath { Sketch(Sketch), Points { points: Vec<[f64; 3]>, smooth: bool } }  // Catmull-Rom, <= 256 points
```

- Use rotation-minimising frames, as `setting::tube` transports its frame (`core/setting.rs:359-420`). The planar path keeps its in-plane normal.
- Leaf law: 0.1 → 1.0 at 0.35 share → 0.05. Thorn law: 1.4 → 1.0 → 0.28.
- Closed paths close the station ring onto itself. Today `chain()` refuses them (`core/cad/twist.rs:112-140`).
- Also pass `closed` and `SweepOptions { scale, twist }` through `Operation::Sweep`. cadkernel already supports them (`cadkernel brep/sweep_path.rs:21-44`); only `core/cad.rs:1868-1870` hard-codes them off.

**E6: claw styles.** `head.claw` and `head.basket` gain `style: "Wire" | "Thorn" | "Sepal"` in `builders::schema` (`core/cad/builders.rs:147`).
- *Thorn:* the wire tapers to a hooked point past the crown facet, keeping a notch-fit tip of at least 0.3.
- *Sepal:* a flat lens claw swept up the pavilion and curled over the girdle. It is notched by `setting::envelope` like every claw, and keeps the straight–arc–straight path rule.

Talons, feathers holding a stone, and raven's feet in the sibling collections are the same parameter.

**E7: seam beads in CAD-only assembly.** In `parts::assembled` (`core/parts.rs:471`), after each successful union, call `blend::seams`/`bead_seam` for parts with `blend_mm` > 0. This is the same code path `resolve_with` uses for band joins.

**E8: graph path nodes and a component pin.**
- Nodes `path.arc`, `path.helix`, `path.wreath(strands, crossings, wander)`, `path.climb(turns, amplitude, surface)` and `path.crest`, returning point lists.
- `cad.feature` gains an optional `component` JSON pin beside `operation` (`graph/nodes/cad.rs:63`). This makes placement drivable, so a template exposes "Prickle count" or "Crossings" as numbers.
- Also `cad.features` (plural), which appends one feature per list item. Today an implicit list on `operation` makes N designs.

**E9: build progress for the async template open.** This directly serves the async-loading request.
- `Template::instantiate` (`crates/ringdesign-workbench/src/templates.rs:18`) runs `TemplateGraph::instantiate` synchronously (`graph/templates.rs:40`).
- These eight parse small, because the CAD chains are KB. The stock JSON is up to 0.9 MB, and Viscum and Prunus embed about 3 MB of painted bark.
- Their cost is in the first build's csg resolve.
- Proposal: `BuildCtx::with_progress(&dyn Fn(Progress))` where `Progress { stage: Sweep | Field | Parts(done, total) | Stamps(done, total) | Seats(done, total) | Refine }`. The totals come from `Document::features.len()`, `design.stamps.len()` and `setstone::set_stones(design).len()`, so the loader's bar has a real denominator. The app then opens a template off the UI thread, parse and evaluate included, and paints the bar from those callbacks.

**E10: collection tooling.**
- Generalise `tools/render_reptilia.py` and `tools/catalog_reptilia.py` into `render_collection.py` and `catalog_collection.py --collection vepres`.
- Generalise `graph/../examples/reptilia_templates.rs`, which hard-codes five slugs, into `collection_templates.rs <collection>`.
- Add a stone material table keyed by `Component::stone_id`: ruby, garnet, black spinel, moonstone, onyx, diamond.

---

## 4. Templates and the showcase

- **Menu.** Add a `VEPRES` group beside `REPTILIA` (`graph/templates.rs:95`) and a "Vepres collection" group in `crates/ringdesign-workbench/src/templates.rs:31-60`. Slugs are `rubus-vepres`, `sentis-vepres`, `rosa-vepres`, `hedera-vepres`, `ilex-vepres`, `viscum-vepres`, `prunus-vepres` and `datura-vepres`.
- **Authoring.** `ex/vepres.rs NEW_DIR [--draft] [SLUG]` writes each ring, following `ex/reptilia.rs:309-352`.
- **Packaging.** `vepres_templates` lifts each design with `lift::from_design`. It verifies a cold, empty-library round trip, byte-identical design and identical vertices, faces and normals, as `reptilia_templates.rs:13-28` does. It then writes `graphs/templates/*-vepres.graph.json` and `showcase/vepres/<slug>/editable-graph.ring.json`.
- **Per-ring folder,** as `showcase/reptilia/README.md` has it: `design.ring.json`, `editable-graph.ring.json`, `artwork/` (outline SVGs and any painted alpha), `finished-metal.stl`, `casting-pattern.stl`, `reference-<stone>.stl`, `report.json`, `mesh.json`, `verification.json`, `hero.png`, `face.png`, `palm.png` and `studio*.png`.
  - Sand rings add `release-fine.json`, which must show zero obstructions and zero unresolved rays (`ex/reptilia.rs:350`, `:369`). Their pattern is shrink-compensated.
  - Lost-wax rings pour the design itself; seats and heads are cast in place.
- **README.** Reproduce commands under the memory guard, using the Reptilia README's `systemd-run … cargo run … --example vepres` lines.
- **What lifts cleanly:** procedural layers, shank keyframes, seats, the imported stock, and CAD chains (as `cad.feature`, byte for byte).
- **What does not yet lift cleanly:**
  - stamps, which are one opaque patch until G1;
  - generated paths, which are JSON arrays until E8;
  - features placed per stamp or per thorn, until E1 folds them into arrays.

---

## 5. Build order

1. E11 probes, then E5 `level` and E3 outlines with the monotone check.
2. **Ilex**, **Prunus** and **Viscum** on the stock (mostly existing API plus E4), then **Rubus**, the first CAD ring poured in sand.
3. E5b, then **Datura**.
4. E1, E2 and E6, then **Rosa**, **Hedera** and **Sentis**. Sentis can be built today at reduced fidelity: constant canes, per-period thorns, wire claws.
5. G1 and E8 before packaging, so all eight graphs can be learned from. E9 alongside, since it serves the async-open request for every template.
