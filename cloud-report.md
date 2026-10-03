# Tenebrae Arcus: cloud report

Branch `claude/tenebrae-arcus`, from `master` at `8e5a59a`. Example: `crates/ringdesign-core/examples/tenebrae_arcus.rs`.
Outputs: `showcase/tenebrae/arcus/`.

## Verdict

**Stopped at the block-out: the subject needs rethinking, not detailing.** Three block-out read tests ran, each with
a fresh reviewer, and none read as a flying buttress. TASK.md caps the block-out at three attempts, so no full-review
round ran.

- Rounds used: 0 of 3 full-review rounds; 3 of 3 block-out attempts.
- Review scores: none (no full review ran).
- Final state: by attempt 3, the pinnacles read as Gothic, and the reviewer called "Gothic cathedral" "a fair guess".
  "Flying buttress" never came: at 300 px the flyer reads as a level rail or a straight strut.

| Read test | Reads | What the reviewer saw first |
|---|---|---|
| 1 | false | "sapphire solitaire with spikes": plain square-post spikes on the shoulders; the flyers (the `shank.cathedral` wire plus a slab) read as ladder rails |
| 2 | false | "sapphire solitaire with two spire posts, Gothic-ish": the spires read as stacked beads, the flyer bowed up like a shoulder, the side-face lancets read as U-cups or gear teeth |
| 3 | false | "sapphire solitaire with two Gothic spires and a reeded band": the pinnacles now read as Gothic, but the flyer reads as a level rail in the hero and a straight strut over a box-shaped gap in the face; the arcade reads as reeding |

The full read-test JSON is in `showcase/tenebrae/arcus/read-test-{1,2,3}.json` and quoted below. The committed renders
are the last state (attempt 3, re-rendered at export). The renders for attempts 1 and 2 were overwritten in place.

## Why it did not read, and what a rethink should try

1. **The flyer is too small to carry the read at 300 px.** In the ring's frame, the span from pier to choir is about
   3.5 mm. On a 23 mm ring framed whole, that is about 45 px, and its arched soffit is a few pixels of curve. The
   reviewer saw a straight strut three times, even after the soffit had a 1.4 mm sag (zoomed in, it is clearly a half-arch:
   see `face.png` at the shoulders). A flyer reads when the span is long and the arch is deep. One way is a lower,
   wider head: a bezel or low basket with the piers further out (spread 60 to 70°), so each flyer spans 6 mm or more
   and the void under it is the dominant dark shape. The other is a hero camera that frames the shoulders, not the
   whole band.
2. **The band dominates the frame.** 3.2 × 2.3 mm Flat, with a plain crest, is most of every 300 px image. A Cathedral
   swell of 0.8 is invisible at this size. A rethink could raise the swell, or make the shoulders themselves the aisle
   roofs: sloped crests under the flyers, so the elevation reads nave, aisle, buttress.
3. **The arcade fights the band's edge.** Thirty narrow lancets on a 2.3 mm side face read as reeding in the
   three-quarter view. Fewer, larger bays (about 14 a side, 2 mm tall, with a string course) would read as arches.
   But they compete with the shoulders for attention.
4. **The builder flyer cannot be a flyer.** `shank.cathedral` always springs from the band's own surface (its
   `Bore` is the band's profile), so it cannot spring from high on a pier, which reviewers 1, 2 and 3 all asked for.
   I dropped it after read test 1 in favour of a hand-drawn rib. The core change below would let the builder be the
   flyer again.

## The ring as it stands (block-out attempt 3)

- **Base:** `ProfileStyle::Flat` 3.2 × 2.3 mm, `flatten_sides()`, `ShankKind::Cathedral` amount 0.8, bore 18.6 mm.
  Lost wax, Gold 18k, `CastProcess::LostWax` then `min_section_mm` 0.8.
- **Stone:** one emerald-cut sapphire, 6 × 8 mm, tint `[0.02, 0.06, 0.45]`, its length round the ring. A 90° turn
  put the claws off the 3.2 mm band. It stands 2.2 mm over the plain basket stand-off, so the choir is tall enough
  for the flyers to meet.
- **Choir:** `head.basket`, 4 claws, wire 0.9, 2 rails, `tip: Point` (P6). `seat.bur { through: true }` sets it
  à jour. `cutter.azure`: 6 teardrops under the stone, the crypt windows.
- **Piers (each side, built in one upright frame and seated once at θ = 140°, mirrored to 40°):** The frame is
  `Placement::Ring` with `tilt_deg = θ − 90`, so the pier's z is the world's up.
  - A battered plinth: a rounded plan drafted in to its set-off, with its foot cut along the shoulder's slope by a
    tilted box.
  - A 1.8 mm square shaft, 6.8 mm over the band at its axis, with a blind lancet 0.35 mm deep in its outward face.
  - A spire turned a quarter-square, so its arrises stand in the silhouette: 2.1 mm across its arrises at its foot,
    0.3 mm at its point, 4.0 mm tall.
  - Hooked leaf crockets at three heights, drawn on one arris and turned onto all four with an `About` pattern.
  - A collar and a pointed knop lofted through four sections.
- **Flyer (each side):** a 1.2 mm rib on the arches' plane. Its extrados rakes straight from 0.25 mm under the pier's top
  to the choir's upper rail. Its intrados is a segmental arch from 3.0 mm up the pier's inner face to the lower rail,
  its crown 1.4 mm over its chord. The rails' positions come from a probe build of the basket, so the rib lands on them.
- **Nave arcade:** 28 lancet stamps a side (0.85 × 1.75 mm, cut 0.45 mm, heads outward), on both side faces from 156° to
  384°, struck by `setting::stamp_row` on `RowPath::SideFace`. They are true-outline cut stamps, not a height field.

### Feature tree, as sentences

1. The band.
2. Stand the sapphire over the crown.
3. Raise the choir: a four-claw basket with two rails.
4. Bur the seat through to the bore.
5. Cut the six crypt windows under the stone.
6. Draw the plinth's plan under the shoulder.
7. Raise the plinth, battered in to its set-off.
8. Shape the bed under the shoulder.
9. Lay the bed along the shoulder's slope, sunk under it.
10. Cut the plinth's foot along the shoulder.
11. Draw the pier's shaft on the plinth.
12. Raise the pier's shaft.
13. Draw a blind lancet on the shaft's outward face.
14. Sink the lancet into the shaft.
15. Cut the blind lancet.
16. Draw the spire's foot on the shaft's top, turned to stand on its arrises.
17. Draw the spire's top.
18. Loft the spire.
19. Draw the crockets up one arris.
20. Raise the crockets.
21. Turn the crockets onto all four arrises.
22. Turn the finial's collar.
23. Seat the collar on the spire's top.
24. to 27. Draw the knop's sections 1 to 4.
28. Loft the pointed knop over the collar.
29. Draw the flyer: a raking coping over a segmental arch, from the pier to the choir.
30. Raise the flyer across the finger.
31. Mirror the pier and its flyer across the crown.

Plinth, shaft, spire, crockets, collar, knop and flyer are separate `Join` parts at one placement. The band's join
clusters them, because a kernel union of the lofted or drafted pieces either refused (`CutRefused`) or tessellated
open. Only the plinth meets the band, so only it carries a seam bead (0.15 mm).

## Gates (block-out state, not fixed: the loop stopped)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, 0 degenerate, 0 self-crossings | pass (491 144 tris) | pass (1 262 396 tris) |
| Every CAD part 0 crossings, every feature Ok | pass | pass |
| Solids and parts notes empty | **FAIL**: the plinth's seam bead folds at 1 of 273 stations, at its outer corners (26.6° / 153.4°) | **FAIL**: same |
| Nothing in the finger hole | pass (nearest vertex 9.29999 mm, r 9.3) | pass |
| `cad::measure::thickness` 0.8 mm, whole ring | **FAIL**: 88 of 384 rays under 0.8 on a 90 k-triangle coarse build | **FAIL**: same |
| Thickness 0.8 mm, each joined part | **FAIL** (see below) | **FAIL** |
| `dfm::cut_lands` 0.8 mm | pass (0) | pass (0) |
| `dfm::findings_in` | pass (0) | pass (0) |
| Stones reported = previewed | pass (1 = 1; crowding clean) | pass |
| Casting pattern closed, 0 degenerate, 0 crossings | pass | pass |
| Export within 2 M triangles | pass | pass (1.26 M) |
| `--verify` cold reload identical | not run | **pass** (identical vertices, faces, normals) |

Gold 18k: 10.14 g. Design file: 251 KB, 31 CAD features, 56 stamps.

Thickness failures are partly real and partly the probe. `thickness` casts from triangle centres along the inward
normal, so rays near edges and joins read short. The real thin features:
- the finial collar (0.3 mm tall);
- the crocket hooks (about 0.18 mm in-plane at the curl);
- the spire's 0.3 mm point;
- the basket's pointed claw tips (P6 `tip: Point`).

The whole-ring probe was also wrong at first: it returns "not assessed" over 250 000 faces, and my first harness counted
that as a pass. It now measures a coarse build and requires rays > 0.

## Template gate

Run as the brief specifies (`template_class: procedural`, `exposed_controls: []`, `--verify-export`). Recorded in
`showcase/tenebrae/arcus/template-verification.json`:

- **Passed** (`template_gate_passed: true`).
- `design.set` patches: **1** (at most 4): `/manufacturing`.
- Graph size: **288 435 bytes**, against the procedural budget of 300 000 (96 %). No size review is required.
- Graph: 118 nodes, 0 exposed controls.
- Cold source parity: **identical**. Cold design and graph reloads pass.
- Mesh parity at export: **identical** vertices, faces and normals, 1 262 396 triangles.
- Open time: the first build takes 2.04 s; evaluate 7 ms, verdict 17 ms.
- 0 detail findings.

The graph is close to its budget, because of the 56 arcade stamps' outlines and the sketches. A rethink with fewer,
larger bays would also free room.

## Platform and enablers

- **Master did not move** during the session: `git fetch origin master` showed nothing past `8e5a59a`. None of
  tonight's enablers reached master, so none was used: crisper stamp and relief edges, C-B2, C-V1 to C-V5, C-T5 to
  C-T7.
- **P6** used: `tip: "Point"` on the basket.
- **P7 placement pins:** not wired. Spread is now the pier's `Placement::Ring` `theta_deg` (90 + spread), with the
  pier's lean `tilt_deg = spread`, and the flyer is part of that placement. So the one-number Spread is two numbers on
  one placement, which a placement pin can carry. The template was not lifted with exposed controls (the brief's
  gate runs with `exposed_controls: []`). The rib's outline is solved from a probe of the basket's rails at author
  time, so a Spread or stone-size change would need the example to re-author the rib.
- **C-T3 artwork:** not used.
  - `gothic/crocket-leaf` and `gothic/gargoyle-silhouette` are drawn at 5.7 and 9.1 mm. At pinnacle scale (about
    1 mm), their lobes fall far under the 0.8 mm section and 0.15 mm detail floors.
  - `gothic/nave-arcade` is a height-field tile, and the lessons say height-field outlines stair-step. The arcade
    is cut stamps instead.
  - No gargoyle was built.

## Core changes wanted (exact code; not made, `src/` untouched)

1. **Let `shank.cathedral` spring from a height over the band**, so the builder's arch can be a flyer leaving a
   pier's face. In `crates/ringdesign-core/src/cad/builders.rs`, `schema`, under `CATHEDRAL =>`:

```rust
CATHEDRAL => vec![
    number("spread_deg", "Spread", "°", 12.0, 75.0, cutters::SPREAD_DEG),
    number("rise", "Rise", "", 0.15, 1.2, cutters::RISE),
    number("wire_mm", "Wire", "mm", 0.4, 2.0, cutters::arch_wire_mm(gem)),
    number("spring_mm", "Springing", "mm", 0.0, 8.0, 0.0),
],
```

   and in `crates/ringdesign-core/src/cad/builders/cutters.rs`, `cathedral`, start the arch `spring_mm` over the band
   (inside the pier that stands there) instead of sunk into it, still reaching the rail at `height`:

```rust
let spring = v.f("spring_mm").min(height - r_a);
// ... inside the `fine` map, replacing `scale(radial, (height + sink) * u.powf(lift) - sink)`:
Ok(add(s, scale(radial, spring - sink + (height - spring + sink) * u.powf(lift))))
```

   plus `claw_geometry_extended`-style fencing (`spring_mm` non-zero → format fence), since an older reader would
   drop the springing.

2. **`cad::measure::thickness` should accept large meshes** by sampling instead of refusing. In
   `crates/ringdesign-core/src/cad/measure.rs`, replace the early return:

```rust
if !mesh.validate().watertight {
    r.note = "Thickness not assessed: invalid mesh";
    return r;
}
// Over the work limit, measure against a BVH instead of every triangle.
let bvh = crate::interaction::bvh::Bvh::build(mesh);
```

   and use `bvh` for the nearest hit, so a lost-wax ring's export mesh (1.2 M faces) is measured itself. Today every
   lost-wax ring must re-build coarse to be measured at all.

3. **Seam bead at a sloped plinth's rounded corner:** the bead folds where a battered plinth's rounded corner meets a
   steep shoulder (`blend.rs`). A note naming the fold is right. But a `blend_mm` the corner cannot take could clamp
   to the largest radius that lays, rather than failing the whole bead.

## Read tests in full

### Read test 1: reads false

**What the eye sees:** A plain smooth gold band carrying an emerald-cut blue stone in a tall four-claw basket, with a bare square-post-and-pyramid spike standing on each shoulder. It reads as a solitaire with two punk spikes, or a 'tower' novelty ring. In the face view the flyers are two thin, nearly straight parallel rails running from the basket down to the spikes, like a ladder or a guard rail, not an arch. Nothing curves upward, there is no open arch to see light through, no crockets or finial on the spikes, no gargoyle, and the band's side faces look blank, so there is no arcade. A jeweller would say 'sapphire solitaire with spikes'. They would not say 'flying buttress', and probably not 'Gothic cathedral' either: two plain pyramids are not enough to suggest that.

**Changes asked:**
1. Make the flyers read as flying-buttress arches (Shoulders, both sides: the 'Flyers' shank.cathedral feature, theta about 90 +/- 34 deg): Raise rise from 0.75 to about 1.3-1.5 and wire_mm from 1.0 to 1.3, so each flyer is a clearly curved half-arch (concave underside) with an open, pointed void under it, at least 2 mm tall, that shows black background in the face view. Merge the two thin rails into one solid arch rib, or space them so the eye sees a single arched member, not a ladder. The arch must spring from high on the pier (just below the spire), not from its foot. Acceptance: in face-300 the flyer outline is visibly curved and the void beneath it reads as an arch.
2. Turn the spikes into Gothic pinnacles (The 'Pinnacle' parts at theta 56 and 124 deg (pier, spire, crockets, knop, gargoyle)): The crockets and knop do not show at 300 px. Raise crocket projection from 0.35 to about 0.6 mm and use two tiers (8 per spire). Make the knop finial at least 0.8 mm in diameter. Raise the pier from 2.4 to about 3.0 mm and cut a lancet niche or a gabled blind arch about 0.4 mm deep into its outward face. If the gargoyle spout cannot read at 300 px, enlarge it to about 1.5 mm of projection or drop it. Acceptance: in hero-300 each pinnacle shows bumpy crocketed edges and a ball tip, and no longer reads as a smooth pyramid spike.
3. Make the nave arcade visible on the side faces (Field layer 'Nave arcade', side faces from the piers through the palm (window theta 270, span 280)): Raise height_mm from 0.25 to about 0.5 and cut repeats_around from 36 to about 18-20, so each pointed lancet bay is about 2 mm tall and wide with deep, dark recesses. Acceptance: in hero-300 the visible side face of the band shows a row of pointed arches instead of plain polished metal.


### Read test 2: reads false

**What the eye sees:** Better than attempt 1, but still not there. Hero: a blue emerald-cut stone in a tall four-claw basket, flanked by two slim towers topped with stacked beads, a bit like a Christmas tree or a pagoda. A jeweller would now say 'tower' or 'spire', and 'Gothic' is possible. The flyer between basket and tower is a flat rail at mid-height, so it looks like a bridge or a handrail. The band's side face has a row of small square notches, like gear teeth or tyre tread, not arches. Face view: the flyer drops from the basket rail and bends down onto the pier. Its curve bows up, like a shoulder, so the dark gap under it is a rounded-off rectangle, not the concave, pointed opening of a half-arch. The side-face marks read as little U-shaped cups (round, not pointed). The crockets show as round balls stacked on a stick, not leaves on a sloped spire, so the pinnacles look beaded more than crocketed. A jeweller would say 'sapphire solitaire with two spire posts, Gothic-ish'. They would not say 'flying buttress', and 'Gothic cathedral' comes only as a guess, from the spires.

**Changes asked:**
1. Redraw the flyer as a true flying half-arch: a concave underside and a sloping top that rises toward the choir (Sketch 'Flyer' (feature 'Draw the flyer: an arched rib from the pier to the choir'), both shoulders; the mirror follows): Make the intrados (the lower edge) a quarter-arc or pointed-arc segment that springs from the pier face about 1 mm below the spire foot and lands on the basket's lower rail. Make its centre lie below and outboard, so the curve is concave seen from beneath, with a sag of at least 1.2 mm. Make the extrados (the top edge) a nearly straight raking line, rising 1.5-2 mm from the pier top to the basket's upper rail, so the rib leans up and in like a strut and does not bow over like a shoulder. Keep the rib about 0.9-1.0 mm deep at the crown of the arc, and thicken it to about 1.6 mm at each end. Acceptance: in face-300 each flyer shows as a dark, roughly quarter-pointed opening with its curved edge below and a straight raking top, and the eye sees a buttress, not a rail.
2. Make the pinnacles read as crocketed Gothic spires, not bead stacks ('Loft the spire', 'Draw the crockets up one arris' / 'Raise the crockets', finial collar and ball; both piers): Widen the spire foot to the full shaft width and taper it to a sharp point, so the spire is a clearly visible tapered cone or pyramid at least 1.2 mm wide at the base. The current bead stack hides the taper. Turn the crockets into hooked, leaf-shaped lobes that sit on the spire's arrises and curl outward and upward, at about 0.5 mm projection, 3 per arris, and stop them short of the tip. Make the finial a cross-shaped finial or a pointed knop about 0.8-1.0 mm across, not a ball on a stack. Acceptance: in hero-300 each pinnacle's outline is a tapering point with notched edges, and nothing in it reads as a stack of round beads.
3. Make the nave arcade read as pointed lancets (Field layers 'Nave arcade, 1-3', side faces from the piers through the palm): Raise height_mm from 0.1 to about 0.35-0.45, and change the alpha tile so that each bay is a tall pointed lancet, about 2.5:1 height to width and about 1.8 mm tall, with a pointed head and a thin colonnette between bays. The current cells read as square notches or U-cups. Run the arcade over the full side face, not as a fringe along its edge. Acceptance: in hero-300 and face-300 the side face shows a row of pointed-arch openings with dark recesses, and nothing that reads as gear teeth or scallops.


### Read test 3: reads false

**What the eye sees:** The pinnacles are now the strongest part. Each is a slim square shaft with a tapering, notched spire and a pointed finial, and a jeweller would call them Gothic pinnacles. The hero now says 'Gothic', and 'Gothic cathedral' comes as a fair guess. The subject still does not read. In the hero, the member between the basket and each pinnacle is a flat, straight rail at the top of the pier, so the ring looks like a sapphire solitaire with two church towers joined to the head by a bridge or handrail. In the face view the flyer is a short straight strut that slopes down from the basket's top corner onto the pier. The dark gap under it is a tall rectangle framed by the pier, the basket post and the band, with no curved underside, so nothing reads as an arch, let alone a flying half-arch. The side faces carry a dense ring of short radial ribs. In the hero they read as coin-edge reeding or gear teeth. In the face view they read as a fringe of small flames or teeth, not as a row of pointed lancet bays. A jeweller would say 'sapphire solitaire with two Gothic spires and a reeded band'. They would not say 'flying buttress'.

**Changes asked:**
1. Give each flyer a visible curved underside, so the opening between pier and basket is an arch rather than a rectangle. This is the third attempt asking for it, and the render still shows a straight strut over a box-shaped gap. (Both shoulders, the flyer rib between the pier (theta about 56 and 124 deg) and the basket; the mirror follows): Fill the lower part of the gap with the arch itself. Make the rib's lower edge a quarter-arc or pointed arc with a radius of about 2-2.5 mm. It springs from the pier's inner face about 1.5 mm below the spire foot and meets the basket's lower rail, so the void under it is a concave, quarter-pointed opening at least 1.2 mm deep at mid-span. Keep the top edge a straight rake that rises toward the choir. Make the rib at least 1.0 mm deep at the crown of the arc and about 1.6 mm deep at each end, so it is the most prominent shape on the shoulder at 300 px. Acceptance: in face-300 each shoulder shows a dark opening whose upper edge is clearly curved (concave from below) with a straight raking top above it, and no rectangular gap remains. In hero-300 the flyer reads as an arched strut, not a level rail.
2. Replace the radial ribs on the side faces with a legible blind arcade of pointed lancets (Field layers 'Nave arcade', both side faces from the piers through the palm): Cut the bay count by about half. Each bay should be about 2 mm tall and about 0.8 mm wide, with a clearly pointed head, a flat colonnette about 0.3 mm wide between bays, and a recess at least 0.35 mm deep. Run the bays over the full height of the side face, with a plain fillet above and below, rather than as short ribs at the edge. Acceptance: in hero-300 the visible side face shows a row of separate pointed-arch niches, and nothing reads as reeding, gear teeth or flames.
3. Make the pier shafts read as buttress piers, not plain posts (Both piers below the spire, outward and inner faces): Add one set-off (a sloped weathering step about 0.3 mm deep) about halfway up each pier. Cut a gabled lancet niche about 0.4 mm deep and 1.2 mm tall into the outward face. Optionally widen the pier foot to 1.8 mm, so the flyer visibly lands on a mass. Acceptance: in hero-300 each pier shows a stepped profile and a pointed niche below the spire, and it reads as a buttress pier rather than a square stick.

