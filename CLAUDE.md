# RingDesigner

Procedural generator for ring designs that must be cast in **sand** (Delft clay
/ petrobond), not lost wax. Everything in the geometry model exists to keep the
result pullable from a two-part sand mould.

## The casting constraint

The ring lies flat in the sand. The mould parts on a plane perpendicular to Z
(the finger axis) and pulls in **both** directions — cope up in +Z, drag down
in −Z.

- The two annular **side faces** (facing ±Z) have perfect draft. Relief embossed
  there pulls straight out. Design placed there is always castable.
- The **outer surface** is castable only where its cross-section drops
  monotonically from a single crest — i.e. where it is domed. Relief on a flat
  outer wall, or anything that leans back under itself, locks in the sand.
- The **bore** is a straight through-hole. Zero draft, but it cores or gets
  reamed at the bench, so it is reported as a vertical wall, never an undercut.
  A comfort-fit bore widens toward both edges and actually gains draft.

This is why the profile library is a family of domes and why the section view
exists.

### Side faces are where ornament goes

On a face whose normal is parallel to the pull, displacement along that normal
moves metal *along* the pull and the walls it raises are *parallel* to it. Such
relief cannot undercut at any height. This is a guarantee of the same kind as
the superellipse drop, not a tuning result — and it is the whole answer to "put
the design on the sides so the detail comes through".

Measured on a size-7 band with a real ornament alpha, undercut area:

| surface | relief 0.15 | 0.30 | 0.50 | 0.80 | 1.60 |
| --- | --- | --- | --- | --- | --- |
| squared side face | 0.000% | 0.000% | 0.000% | 0.000% | 0.000% |
| edge-flange flat (90°) | 0.000% | 0.000% | 0.000% | 0.000% | 0.000% |
| crest of a half-round | 0.75% | 1.77% | — | — | — |

`FieldContext::side_faces(min_deg)` finds them by walking the base draft inward
from each bore edge. `SIDE_FACE_MIN_DRAFT_DEG` is **80°** — calibrated, not
guessed: at 55° a half-round's fillet grazes the threshold on its way past and
reports a face that then undercuts at 0.15 mm.

The usable width of a side face is **`thickness - crown`**. A half-round spends
90% of its thickness on the crown and honestly has no side; `ProfileStyle::Flat`
spends 15% and leaves most of it. `BandProfile::flatten_sides()` drops the side
draft to zero and shrinks the edge fillet, which is what turns a nearly-square
face into a square one.

The two edges are independent — an edge flange gives a wide face on one edge and
none on the other — so `SideFaces::low` and `.high` are each `Option`. Only
mirror a tiling onto both when `is_even()`; mirroring a one-sided flange lands
the copy on bare dome, which is the worst case there is.

Two flanges, one per edge, is **not** castable: the dome between two proud rims
is a valley, and no single parting plane clears both.

## Architecture

Two crates. `ringdesign-core` has no UI dependency and is where the geometry
lives; `ringdesign-gui` is eframe/egui.

### The one idea

**The entire design is a scalar height field `h(u, v)` in mm over the swept
band surface.** Tiled alphas, borders, milgrain, raised gem-seat pads, swept
curve wires (`curve.rs`), parametric flutes/reeding, free-placed decal stamps,
and nested groups are all layers in that field. There is no CSG anywhere.
Inscriptions (`text.rs`, two bundled OFL fonts), hand-drawn strokes and
imported SVG art (`svg.rs`, resvg; ink coverage reads as height, `invert`
flips, `<text>` elements deliberately unrendered) all travel in the design
as source data and rasterize into the alpha library on load — `bake_drawn`
/ `bake_texts` / `bake_svgs` are called as a trio at every load site. A
per-layer `Remap` (curve or terrace) reshapes relief profiles, and
`Alpha::draft_limited` bakes a cone opening so no wall in an imported texture
exceeds a chosen angle at the layer's cell size.

A tiling's `edge_mm` rebuilds height from the alpha's **signed distance
field** (exact EDT, x-wrapped, derived by `bake_all` and never persisted):
the bevel holds its width in mm at any tile size, to within half a texel.
`Layer::Openwork` carves the mask's ink toward a floor over the bore —
depth·nr of radial metal is what a normal carve spends, so the cap opens up
exactly on side faces, the sanctioned home for deep carves; the crown
version is caught by the field verdict, and the test pins both. A tiling's
`warp` bends its rows along a guide point-list around the ring, purely in
sampling space.

Shank kinds beyond the originals: Pinched, Bombé, Saddle, FlatTop, **Wave**
(edges slide along the finger while the crest stays on the parting plane —
swing capped at 0.6 of the half-width where the measured undercut converges to
phantom scale) and **Twist** (Wave's slide plus a phase-locked flank-exponent
skew, `ShankMod::flank_bias` — the light-line spirals while both flanks stay
monotone drops; a true helix locks). **Keyframes** hands the band
over outright: authored width/thickness/crown stations blended by periodic
Catmull-Rom, exact at the knots and C1 through the joint, `amount` as the
master strength. `BandProfile::morph` blends the crown
toward a second style around the top (`ShankMod::drop_blend`, filled by
`RingDesign::modulation_at`, which every modulated-section consumer goes
through), and `SignetHead::table_dome_mm` puts a cabochon cap on a signet
table — which also retires the zero-draft plane behind the refined-build
phantom on such heads.

- `u` — arc distance around the ring at the crest radius. **Wraps** at the
  circumference.
- `v` — arc distance across the cross-section, from one bore edge, over the
  outer surface, to the other bore edge.

Because it is one function, tiling, the unrolled layout editor, draft analysis,
and cross-sections are all just different ways of evaluating it. A tile drawn in
the layout editor is exactly where the metal lands.

Per-layer gating composes the same way: the angular `Window` carries an
optional cross-band `VGate` (a `v` strip, or the side-face runs resolved at
evaluation time), plus a painted alpha mask multiplied into the strength.
`Blend::SmoothMax`/`SmoothMin` fillet crossings with the tie-exact `smax`
crossfade the signet union proved out. A `Layer::Group` composites a nested
stack first and is then blended, windowed and masked as one unit, so `Replace`
inside a group cannot leak past it.

A curve wire's headline placement is a **side face**: measured 0.000% undercut
at 0.5 mm relief there, against 1.1% at −31° for a rail waving just 0.2 mm
across the crown — a ridge undercuts on its crest-side flank wherever the
dome's draft is shallower than the wire's own slope. The add-menu presets land
on a side face automatically when the profile has one, and `WireProfile::Round`
is a cosine dome rather than a circle because a circular section carries a
vertical wall at its own edge.

### Pipeline

```
BandProfile ──sample──> ProfileLoop  (closed loop in (r,z), CCW, outward normals)
                             │
ShankStyle ──modulation──────┤       (per-angle width/thickness/Euro chord)
                             │
LayerStack.height(u,v) ──────┤       (mm of displacement along the normal)
                             ↓
                        mesh::build  (sweep + displace + triangulate)
                             ↓
                     Mesh ──> castability::analyze ──> CastReport
                          └─> stl::write_stl / write_obj
```

### Why the mesh is always watertight

The cross-section is a **closed** loop and the sweep closes at 360°, so both
grid directions wrap. The result has torus topology: no caps, no seams, no
special cases. `Mesh::validate()` confirms it (zero boundary and non-manifold
edges) but it is watertight by construction, not by repair.

Face winding: for a CCW loop with tangent `(dr, dz)` the outward 2D normal is
`(dz, −dr)`; sweeping in +θ makes `e_θ × e_profile` point outward, so triangles
`(i,j)→(i+1,j)→(i+1,j+1)` are already wound correctly.

Three fidelity mechanisms sit on top of the sweep, all measured:

- **Vertex normals come from grid central differences** (`mesh::grid_normals`),
  not facet averaging — the grid is the surface's own parameterization, so the
  cross of the two tangents is the surface normal. Worst error on a plain band
  at 96x64: 0.015° against 0.095° for area-weighted accumulation.
- **A sample row lands on every profile feature.** `sample_spaced` records the
  crest, fillet tangencies and flange corners as `ProfileLoop::feature_v` and
  snaps the nearest sample onto each, so no facet chords across a slope
  discontinuity — the reported crest radius is exact rather than the chord's.
  A sweep must take everything about the row layout from the **reference**
  loop, and the same one for every slice — the snap fractions *and* the
  bore/surface row split. Slice-own features drift with the modulation, and
  rows snapping to drifting targets tear the grid along theta (a 0.013%
  phantom on a bare signet head); a per-slice rounded split steps by one
  wherever the surface's share crosses a half-row, every surface row
  renumbers, and the grid tears a vertical zipper (60–82° folds down a
  signet's shoulder at exactly the stepping slices). `sample_spaced` takes
  `Option<&ProfileLoop>` for this reason.
- **Refinement is seeded by the layers.** `Layer::feature_footprints` names
  each layer's finest scale and where it lives; `refine::build` pre-splits
  those regions to half that scale before the error loop, so a bead or small
  pad cannot slip between a cell's nine probes. `RefineStats::seeded_cells`
  reports it, and `saturated_leaves` flags a depth-limited tree that would
  otherwise report a worst error of 0.0.

Design files carry a `format_version` (migration ladder in `library.rs`) and
embed every referenced non-regenerable alpha as 16-bit PNG on save, so a
`.ring.json` survives moving machines. Export speaks STL, OBJ and 3MF —
`threemf.rs` writes the package with a hand-rolled store-only zip (zeroed
timestamps, deterministic bytes) so `unit="millimeter"` and the design's
name and size travel with the mesh; no zip dependency was bought for a
container three files big.

### Refined builds: a tolerance instead of a step count

`BuildParams::refine` swaps the swept grid for a quadtree over the `(u, s)`
torus (`refine.rs`), where `s` runs around the closed cross-section. A cell
subdivides while the surface at its edge midpoints and centre sits further than
`tolerance_mm` from the flat facet it would become. **The triangle count is an
output, not an input.**

It is still watertight by construction, from the same source as before: every
corner, midpoint and centre is a point of one integer lattice, and vertices are
keyed by lattice coordinates, so cells sharing an edge share its endpoints. The
tree is balanced 2:1, leaving at most one hanging node per edge, and any cell
carrying one is fanned from its centre through it.

Two things that had to be right, both caught by tests:

- **Balance is probed from the fine side.** A coarse cell's edge may border two
  finer cells and one probe at its midpoint sees only whichever owns that point;
  a fine cell's edge always has exactly one neighbour. Reading it the other way
  leaves a finer neighbour partway along a long edge undetected, and that crack
  shows up as boundary edges.
- **Refinement runs until nothing is marked**, not once per level. Balancing
  subdivides too, so a pass creates cells the pass that made them never
  examined; a fixed level count left `tol 0.008` stuck at 0.022 mm while
  spending 36% more triangles than `tol 0.02`.

Two tolerances, not one. `tolerance_mm` bounds how far the mesh sits from the
surface; `normal_tolerance_deg` bounds the facet's *slope*, which position does
not — a 0.08 mm sag across a 0.2 mm cell is a 20° slope error. They pull against
each other, so the presets loosen the angle at the coarse end and tighten it
toward export.

The tree is **anisotropic**: every leaf carries a level per axis, sag is
attributed per axis, and only the offending direction splits — a milgrain
row pays across the bead without paying along it. The slope criterion stays
the plane-vs-plane measure on purpose: a half-edge turn measure reads a
crease's own angle at every scale, and the first draft of the split
criterion used one — cells straddling every bead rim split to full depth
without converging (239k leaves, 0.23 mm residual). The lattice runs one
level finer than the deepest split so midpoints and centres stay lattice
points even for one-axis slabs, which is what keeps the watertight
guarantee. Balance is per axis: horizontal edges constrain the `u` levels,
vertical edges the `s` levels, still probed from the fine side.

Measured on a size-7 D-shape with a tiled alpha and milgrain, worst facet
deviation via `refine::grid_error_mm` (isotropic numbers in parentheses):

| build | triangles | ms | worst error |
| --- | --- | --- | --- |
| swept 384x144 | 110,592 | 13 | 0.107 mm |
| swept 512x192 | 196,608 | 26 | 0.075 mm |
| swept 1536x448 | 1,376,256 | 277 | 0.045 mm |
| refined, 0.08 mm / 20° | 87,264 (was 136,668) | 229 | 0.080 mm |
| refined, 0.04 mm / 14° | 148,728 (was 212,892) | 449 | 0.040 mm |
| refined, 0.02 mm / 9° | 276,768 (was 406,848) | 581 | 0.020 mm |

The win is in how the cost *scales*, not at any one point. Halving a swept
grid's error means halving the step in both directions, so it pays 4x the
triangles every time — which is why 1.4M of them still only reach 0.045 mm.
Refinement pays about 1.7-2.0x per halving, so it goes places the grid cannot
afford at all — and the per-axis splits buy a further ~30-36% off every row
of the table at the same error. At loose tolerances the sweep is faster, being a trivial loop.
**Sweep for the interactive preview, refine for export.**

One caveat with teeth: `castability::analyze` reads face normals, and an
irregular mesh reports small spurious undercuts along the crest line, where the
true surface is tangent to the pull and any facet noise crosses zero. On a
signet every swept build reports 0.000%, while refined builds report 0.10-0.18%
and up to -15°. Under the 1% that reads as "will not release", but enough to
move the verdict — and on a signet it does not fall with the tolerance, because
the table is a *plane* at zero draft rather than a crest line, so a whole band
of the surface has nothing to decide its sign but its own slope error.

**The verdict therefore comes from `castability::analyze_field`**, which
samples the true surface on a `(theta, s)` grid — the same reference-snapped
sections the mesh is built from — with central-difference normals. A smooth
normal at the crest is exactly radial and on the table exactly radial, so the
phantom cannot exist in it: measured on a bare heart signet, 0.006% at preview
sampling falling with resolution against the refined mesh's non-converging
0.10-0.18%, and exactly zero on every symmetric outline. It also sweeps the
thinnest outer-to-bore wall over the finger hole into the verdict against
`DraftSettings::min_section_mm` (thin sections fail to fill, they do not
lock). The GUI banner, the MCP `castability` tool's `field` block and the
worker all read it; `analyze` stays for painting faces in the viewport. The
retired rule was "judge castability from a swept build" — nothing needs
judging from any build now.

`adaptive.rs` was the earlier attempt at the same goal by redistributing the
same number of sample *lines*. It is kept, default off, and its module doc
records why it loses on anything carrying relief: the densities are separable,
so detail localized in `u` and `s` at once cannot be expressed.

### The castability guarantee lives in `profile.rs`

The outer surface is a superellipse drop from a single crest:

```
d(x) = 1 − (1 − x^a)^(1/b)
```

`x` is normalized distance from the crest toward the nearer edge. `d` is
monotonically non-decreasing for any `a, b > 0`, so **the base profile can never
undercut a ±Z pull**. Style presets are just `(a, b, crown, edge_round)` tuples.
Only the height field can introduce an undercut, which is what
`castability::analyze` looks for.

Two constants encode real metallurgy: `MIN_EDGE_MM` (0.2) because feather edges
will not fill, and `mesh::MIN_WALL_MM` (0.5) so displacement never eats into the
finger hole.

### Windowing a layer to an arc

`LayerEntry::window` gates any layer to part of the ring, or (inverted) to
everything but that part. It is what puts ornament on the shoulders flanking a
signet without running it across the table. A gated-out layer is skipped
entirely rather than scaled to zero, so a `Replace` layer outside its window
cannot wipe the layers under it.

A window is positional, not periodic, so it needs no integer count — `wrap_delta`
on 360° makes it continuous across the joint. Leave some `fade_deg`: a hard end
raises a wall the mould has to clear.

### A signet is the band, not something on it

The head **is** the ring over its arc. `ShankKind::Signet` and `SignetHead` in
`profile.rs` build the whole thing out of the sweep:

- the **body** is the band's plan silhouette: the union of
  `SignetOutline::body_extent` read at the position this angle projects to on the
  table plane, and a **swell** that carries the head's own span back to the shank
  over an arc two and a half times as long. The outline gives the head its shape,
  the swell gives it its tail, and neither alone is a signet. That arc is
  `HEAD_SWELL_RATIO` × the head's own half-angle, and a **ratio** rather than an
  angle because the swell is the head's influence: a fixed 75° came from one
  reference and ran the swell out 20° early on a bigger head;
- the **table** is the band's crest, with `crown_scale` taken to zero across the
  section and the radius solved as `plane / cos` of the angle off centre. Flat
  in both directions is a plane;
- the **shoulder** is the arc past the face over which the crest falls back to
  the shank, which is the band coming up to meet the underside of the head. It
  is much shorter than the swell, so the width goes on widening under a crest
  that has already come down — the broad thin shoulder of a real signet.

There is no CSG and no pad. Two fields on `ShankMod` make it possible.
`outer_r`: a head's depth is set by where its table plane sits, not by a
fraction of the band's own thickness, so the modulation names a crest radius
outright and `sample_spaced` takes the thickness from it. `z_center_frac`: a
swept band is centred on its own mid-plane, and an upright face is not.

The band at each angle is the **union** of the outline and the swell, closed with
`smax` and `smin` so the corner where they cross is filleted rather than creased,
with the shank strip under both as a floor. A union, not a blend: easing the
outline into the swell fattens it, and the whole point of the head is that its
silhouette is the face wherever the face is the wider of the two.

**Measured 0.000% undercut on every symmetric outline**, at every taper down to
a 1.9 mm shank on a 12 mm head, every rise to 2.2 mm, every face length to 20 mm,
every shoulder from 10° to 40°. A band that widens *and* rises toward the top is
single-valued in Z over `(r, theta)`, so it is a terrain and releases by
construction — the same guarantee as the superellipse drop, not a tuning result.

#### The hollow under the head

`SignetHead::hollow_mm` (0–4, default 0; GUI "Hollow", MCP
`head_hollow_mm`, graph `hollow_mm`) scoops the head's belly out from the
finger hole: `ShankStyle::modulation` carries it as
`ShankMod::bore_lift_mm`, the dominant head's own `on_head` presence
smoothed over the shoulder, and `sample_spaced` adds it to the bore
closure — the whole bore chord lifts and the side walls rise from the
lifted corner, capped so an edge of `MIN_EDGE_MM` always survives above
the comfort dome. Nothing outside moves: the crest is absolute on a head
and the outer surface is built from `inner_r`, so the scoop only removes
metal. It is castable for the reason the bore is reported as a vertical
wall and never an undercut: the pull is along Z and a bore surface's
normal is radial at any radius, so the verdict reads the scoop's roof
and its fading ends as vertical wall and only the **wall it leaves**
changes, which `thinnest_wall_mm` measures per slice. Measured on a
12 mm lofted signet with a 0.6 mm hollow: bore lifted 0.600 under the
face, palm bore nominal, the lift falling monotonically to zero across
the shoulder, field clean, and the ring lighter. A toi et moi's second
head follows the primary's setting.

#### Three bought signets, measured

`/home/shadowbroker/jewelry-scan/RING/Signets/` holds a heart, a hexagon and an
oval as **STEP and STL** — parametric CAD, not scans, so the numbers are exact.
`examples/scan_signet.rs` reads them (via `examples/common/scan.rs`: split into
components, fit the bore, align the head by its own symmetry) and prints the
same measurements for one of them and for ours, so the columns compare.

The heart's STEP settles the construction outright. Per ring: **one plane** (the
table), three cylinders, six tori, six B-splines, two spheres. The bores are
8.65 / 9.10 / 9.50 / 9.90 / 10.30 — size 7 is exactly our `BORE_R`. The table
sits **1.75 mm over the bore on every size**. The shank is a single torus, major
8.7679 and minor **2.9321**, which is 4.63 mm wide by 1.40 thick with a flattened
crown — not a half-round. Edge breaks are 0.2 mm, plan corners 0.25.

What that measurement changed, all of it visible:

| | was | reference | now |
| --- | --- | --- | --- |
| table area, matched to the same band | 119 mm² | 242 | 210 |
| table over the bore | 2.65 mm | 1.77 | 1.69 |
| width half gone by | 37° | 53° | 48° |
| heart's upper boundary at a fifth out | 0.60 | 0.90 | 0.94 |

- **`heart_radius` was wrong.** It returned **zero at the dimple** — a cusp
  running to the centre — so the outline had a spike for a point. Replaced with
  the classic `(x² + y² − 1)³ = x²y³`, solved per ray by bisection.
- **`head_aspect(Heart)` was 0.95**, and the reference's plate is 18.53 mm round
  the ring by 17.64 across: **1.05** (an interim note here said 1.12, which its
  own numbers contradict — 18.53/17.64 = 1.05, and the classic curve's natural
  box is 1.02).
- **`HEAD_FACE_DRAFT` was 0.09.** The reference's head is a **prism** — its
  section at the head is a clean rectangle from bore to table, so the table's
  outline *is* the body's. Now 0.02, a token draft rather than a look.
- **`HEAD_RISE_MM` was 0.8**, against the reference's 0.38 over its own shank.
  Now 0.3. A signet's bulk comes from the plane standing off over a curve — the
  reference's table centre is 1.77 mm over the bore where its corner is 5.29 —
  not from the rise.

#### The face is a facet, not an extrusion

**The body and the face are different shapes.** The face is a facet cut across
the crown of a wider body — measured on the reference, 16.0 mm of body under a
14.7 mm table — and if the band simply carries the face's outline from the table
down to the finger, the result is a prism with a signet's face on it. On a heart
that is unmistakable: the dimple runs the whole depth of the ring and each lobe
leaves a crease down the flank.

So `SignetOutline::body_extent` is the face's own reach with its hollows faired
out, and it is what the band follows. `SignetHead::body_fair` blends between the
two; 0 is the extrusion.

It is a **morphological closing** — dilate then erode — and the shape of the
structuring element is the whole thing:

- Not a blur. A blur pulls the peaks in with the hollows, and the head comes out
  a blob with the face lost in it.
- Not a flat window either. A flat one fills a hollow to the level of its rim and
  holds it *exactly* there: closing a heart with one took the whole dimple side
  of the head to a straight parallel edge over 122 of 200 stations, and left a
  cliff where that met the shoulder. Both of those are visible.
- A **rolling ball** — a paraboloid of radius `BODY_FAIR_R` — bridges a hollow
  with an arc of its own radius and leaves everything it cannot reach alone. A
  heart's notch fairs from 0.212 of the half-width to 0.071 while its lobes, its
  point and the head's plan size stay exactly put.

`BODY_FAIR_R` is 0.75 half-lengths because the residual notch and the bluntness
of the head's end pull against each other. Measured on the heart by
`examples/fair_probe.rs`; the floor is not zero — with the ball taken to nothing
the corner rounding alone leaves 0.12 of notch, and the outline's own rounded
end leaves 0.74 of width, because width falls off a smooth end like a square
root:

| ball radius | 0.35 | 0.55 | 0.75 | 1.00 | 1.30 |
| --- | --- | --- | --- | --- | --- |
| notch left of 0.212 | 0.098 | 0.083 | 0.071 | 0.060 | 0.051 |
| body's width at the head's end | 1.11 | 1.24 | 1.33 | 1.42 | 1.54 |

0.75 is the least radius that holds the notch near the 0.07 the original
calibration picked; past it the ball keeps paying width for notch at the same
rate until the head stops ending and starts being a slab. (An earlier table
here read 0.55 off the pre-reference heart — the classic curve and the 1.12
aspect moved every number, and the radius was re-tuned to keep the residual.)

**The body must contain the face**, or the flank leans back over the mould half
it sits in — the same undercut as any other, said where it can be proved instead
of sampled off a mesh. Closing is extensive, so it can only add, and it stays so
with the ball truncated at the head's ends because the erosion's own station is
always one of the samples it minimises over. `the_body_contains_the_face_it_carries`
asserts it to within 1e-5, which is Catmull-Rom reconstruction noise where the
two curves touch; `head_at` clamps the crest into the bore regardless.

#### The head has one edge, and it is the face outline

A dihedral census over the reference heart's mesh (`examples/edge_probe.rs`)
settles what a signet's surface is: **0.0 mm of ≥15° creases anywhere except
its bore edge break**. Even the plate's rim is filleted — the rounding reaches
0.91 mm below the plane — and the only near-sharp features are the heart's
point and its cleft, which are corners of the *outline in plan*, not creases
in the surface. Everything else, walls included, is smooth and convex. The
same census on our pre-rebuild mesh found 740 mm of hard edges at up to 123°,
which is what all of the following removed (final state: the point, the cleft,
and the rim itself, plus ~90° sampling folds at the two face-end closure
stations that a refined build resolves).

- **The wall is one convex C² curve** (`sample_spaced`'s `wall`): vertical
  into the rim fillet so the plate holds the outline's shape, vertical again
  into the bore corner, the whole bore-to-crest offset carried in the belly —
  `w = t + (smootherstep(t) − t)·head`. `ShankMod::head` (smoothed `on_head`)
  crossfades it with the band's straight drafted chord, so ordinary bands are
  bit-identical, and both blend weights are monotone in `z`, so the wall can
  never fold into a ceiling at any mix. A heart's cleft rides down it as a
  smooth cove; the old take-off/bulge composite (and its cap's C⁰ kink in
  theta) is gone.
- **The rim is the head's own fillet**, `SignetHead::rim_round_mm` (default
  0.6, the reference's ~0.6–0.9), blended in over `head` in place of the
  band's edge fillet — how hard the one edge reads no longer depends on how
  the shank's edges are broken. The parting-plane straddle `keep` grows by the
  rim on-head, because the fillet takes its radius out of the span's ends: a
  0.6 rim on a 0.28 straddle rounded the crest away from the plane and the
  ceiling came back through the fillet at −54° over 0.18% of a Draft heart.
- **Every clamp that engages mid-sweep is smooth, with a radius sized to the
  plunge.** A hard `min` is a slope step in theta that sweeps a crease down
  the wall at the locus where it bites. The straddle floor's radius follows
  the station (`ShankMod::straddle_soft`): a heart's lobe boundary crosses the
  floor at 0.86 mm/deg near the face's ends, so a small value-space radius
  transits in under a degree (measured 100° grid folds at exactly the two
  slices where it crossed) — while over the plate's middle a wide radius drags
  boundaries that legitimately sit near the floor (it pulled the cleft half
  shut). Biased up by the crossfade's worst undershoot of a true max, 0.087
  of the radius. Where the floor holds the span past the outline's reach, the
  forced run **rolls as one fillet** (0.85 of the run) instead of flat plus a
  migrating corner.
- **The crest line's own corner at the plate's theta-end is filleted too**
  (`head_at`: tie-exact `smin(climb, dive, rim)` of the plane solve against
  the shoulder fall): the unrounded peak was an 80° fold between the two
  slices straddling it. The crest *span* now simply follows `h00` — the
  quarter-shoulder hold this replaced propped a shelf beside the plate that
  the reference does not have, and the crease it patched is gone at the
  source.
- **`HEAD_TAKEOFF` is 0**: the outline is read to its true end, which is what
  closes a heart's lobes instead of chopping them; the straddle floor and the
  roll do what the hold used to. The silhouette table inherits its neighbour
  at stations the boundary scan misses (the tangent sliver at x = ±1) — a
  zeroed end cell collapsed the span to a bogus centred point, a 4 mm yank
  inside one table cell, 128° folds.
- **`head_aspect(Heart)` is 1.05**: the reference plate is 18.53 mm round the
  ring by 17.64 across, and 18.53/17.64 = 1.05 (an earlier note said 1.12 —
  its own numbers disagree). The classic curve's natural box is 1.02. The
  plate our build produces now matches the classic curve's per-station
  extent table to ±0.02 of half-width over the whole face, and the classic
  curve matches the reference plate to ~0.08.

One casting tax remains, and it is honest: the reference's plate edge dips
wholly below its ring's equator at the face's ends — that shape cannot part
on a plane, so there our crest holds a `keep`-wide strip at z = 0 and the
plate's last interval widens onto it, rolled. It is the two-part-sand price,
kept as small as the fillet allows.

#### A crease is a step in curvature, not in slope

A surface can be C¹ and still catch the light in a line, which is why the
silhouette is judged on its second derivative. Worst curvature of the bore's
reach, per degree squared:

| | heart | hexagon | rectangle | oval |
| --- | --- | --- | --- | --- |
| face extruded | 10.68 | 2.92 | 0.0062 | 0.0042 |
| faired body | 0.0073 | 0.0232 | 0.0062 | 0.0037 |

The shapes with a concavity or a corner in plan are the ones that had a crease.
What is left on a heart is its own curvature. The hexagon keeps 0.023 because a
hexagonal head is *meant* to show the corner where its flat side meets its
slanted one; the body rounds it to a tight fillet rather than passing it through.

Getting there needed the blends to be C² as well as C¹. `smoothstep` leaves a
step in curvature at each end of its window, and both ends of the edge break had
one — `smootherstep` is what the head's blends use.

#### The section is built between two spans

`sample_spaced` no longer sweeps one symmetric half-width. It builds between a
**bore span** and a **crest span**, both absolute in the section's own frame:
the bore carries the band's body, the crest carries whatever is faceted onto it.
`ShankMod::crest_span` names the second; `None` hands it the bore's own, which
is the ordinary band, one shape drafted by one angle.

Spans and not widths, because a face that stands upright is off-centre by a
different amount at its crest than at its bore, and a symmetric inset puts its
crest in the wrong place — measured at 1.44% undercut on a shield before this,
0.011% after. It also subsumes the old crest-levelling: the crest sits at the
parting plane by construction rather than by a correction term.

`HEAD_FACE_DRAFT` uses it to draft the head's flanks by **2%**, so the table is
a slightly smaller copy of the outline that carries it — a token draft rather
than a look, which is what a two-part mould wants of the one surface it has to
slide off. Proportional, not a distance: insetting by a distance drafts a
narrow station to nothing and leaves a fin standing off the end of the head,
which a heart does first.

The reference's own 16.0 mm body under a 14.7 mm table is an 8% inset, and that
is **not** this constant: the body is wider than the face because
`SignetOutline::body_extent` fairs the face's hollows out with the rolling
ball, not because the flanks are drafted. Conflating the two is what put 0.09
here originally — see the measurement table above, which records the correction
to 0.02.

#### The shank is flat, and the swell is what makes it read

`/home/shadowbroker/jewelry-scan/RING/BlankSignet.obj` is a real 14.7 mm round
signet — 20 mm bore, 7 mm shank, 1.75 mm thick, table 0.265 mm proud, body 16.0
mm across — and it settles the whole shape of a head. Measured off the mesh by
bucketing its vertices round a circle-fitted bore:

| off the head | 0° | 15° | 25° | 35° | 45° | 60° | 75° |
| --- | --- | --- | --- | --- | --- | --- | --- |
| body width | 100% | 90% | 74% | 52% | 31% | 11% | 2% — the shank |
| its face there | 100% | 90% | 65% | — | — | — | — |
| crest above the shank | 13% | 37% | 77% | 85% | 39% | 9% | 0% |

Three separate things, and the head only reads right when all three are:

- Its **shank** varies by 1% over the 215° behind the head, so tapering the
  strip was a wrong turn and is pinned flat by `the_shank_is_flat`.
- Its **width** comes down over 75°, more than twice the face's own 31°. A plain
  `smoothstep` over that arc tracks the reference to within 4% of the drop
  everywhere. That arc is **not a constant**: see `HEAD_SWELL_RATIO` below.
- Its **crest** follows the table plane out to the face's edge, peaks at the
  table's corner, and is back on the shank by 75° — 43° of fall, and it leaves
  the rim *already diving*, because a rim is an edge and the flank under it is a
  fillet running out flat. `HEAD_SHOULDER_DEG` and `HEAD_SHOULDER_POW`.

So the body is the **union of the face outline and the swell**, and that is what
gives a head both a shape and a tail. The outline stands clear where it is
fuller — a heart's lobes, a shield's shoulders, a rectangle's sides — and the
swell carries everything else. Before this the band's silhouette simply *was*
the face outline, so the swell was over the moment the face was: half gone by
22° and finished by 28°. `the_swell_matches_a_real_signet` is both columns side
by side; worst error went from 0.63 of the drop to 0.04.

A consequence worth knowing: for an outline **leaner** than the swell at some
station the swell wins outright, so a marquise and an oval carry the same body
20° off the top. That is right — a real marquise signet is a pointed table on a
rounded body — and the plate's shape lives in the crest span, which is read from
the outline alone.

**The outline hands over to the swell across the head's end**, over the last
`HEAD_EDGE_BREAK` of the face. A straight-ended outline reaches full width at its
last station and nothing past it, so one that simply stops steps the band onto
the swell in a single sample: measured on a shield, **1.76 of full width per
degree**, which is a wall. Faired onto the swell it is 0.002. What it costs is a
chamfer on the head's last arc in plan, which is the edge break a corner wanted
anyway.

The fade has to **close at the face's end**, not straddle it. The silhouette's
slope is discontinuous there for any shape with a straight end — the station
stops advancing while the outline is still plunging — so a fade still half open
at that point carries the kink straight through: a rectangle steps 0.19 of full
width per degree against 0.002 once the fade has already closed.

It is 0.45 and not the 0.18 it started at because the hand-over is the last
curvature left in the silhouette, and widening the window spreads it: 0.022 per
degree squared at 0.18, 0.011 at 0.30, 0.007 at 0.45, which is the head's own.
Widening it costs nothing in the face — the bore stays outside the table's own
span the whole way, so the safety clamp in `head_at` never bites.

**The union has to be exact on a tie.** The usual smooth maximum,
`max + r·h²/4`, rounds a crossing *outward*, and the outline and the swell are
equal by construction at the head and again past the outline's end — a tie, not
a crossing. Rounding there pushed the head past full width and fattened the whole
shank by 4%. `smax` therefore crossfades between the two over the band instead of
adding to their maximum: exact when either dominates, exact on a tie, C¹ at both
ends.

#### An upright face moves the band off its mid-plane

A crest reads up the finger — flat top toward one band edge, point toward the
other — so `SignetOutline::upright()` turns the heart and the shield a quarter
round. That makes the face reach further one way across the band than the other,
which a symmetric width cannot express: hence `extent` returning an interval
rather than a half-width, and `z_center_frac` moving the section along the
finger to carry it.

**An offset section has to keep its crest level.** The mould parts at one height
for the whole ring, so if the crest rides down with the section, the flank
between the two leans back over the mould half it sits in. Measured on a shield
head: **-19.4° over 0.67% of the surface** — a real undercut, not facet noise.
`sample_spaced` biases the crest back by the offset, which takes it to 0.008%,
and what is left is the crest-line phantom.

**And the crest span has to reach the parting plane at all.** An upright outline
at its last station does not straddle it — a heart's lobe is a patch of table on
one side of the band and nothing on the other — so a span taken as drawn puts
the crest *below* the plane and turns everything between the two into a ceiling.
Measured on a heart: 0.24% of the surface at **-77°**, and unlike the phantom it
did not fall with the sweep (0.30% at 192x96, 0.24% at 2048x512), which is what
tells the two apart. `sample_spaced` widens the crest span to contain the
parting plane by `keep` either side; the heart is 0.059% at Draft and 0.0035% at
Export after, converging. It costs the head's ends a flat run of `keep`.

Two consequences worth keeping in mind:

- The furthest point on the ring is the **corner of the table**, not its middle.
  A plane over a curve stands off at its ends; that stand-off is the chunk a
  signet reads as, and `HEAD_MAX_HALF_DEG` bounds it before `1/cos` runs away.
- The table is a **dead-flat, zero-draft wall**. That is fine — it is blank and
  hand-engraved, and design goes on the head's flanks, which face the pull. But
  it means facet noise there has no draft to be measured against, so a refined
  build's *mesh* reports a phantom bounded only by its own slope error. The
  field-sampled verdict reads the plane's own normal and has no such term —
  judge a signet from `analyze_field`, like everything else.

A short shoulder used to make that phantom worse — dead-flat crest morphing
to a rounded wire faster than the sweep samples put 5 undercut faces on a 20°
shoulder at Draft. After the one-edge rebuild the bare head reports **0 faces
at every preset and every shoulder arc down to 20°**: the rim fillet means the
crest region is never flat-tangent over a band, so facet noise has real draft
to be measured against. `HEAD_SHOULDER_DEG` stays 43° because it comes from
the reference's own crest fall, not from the mesh.
`mesh::tests::scratch_signet_head_undercuts` is the table.

#### The cut dome is the other construction

`SignetHead::dome` (0..1) swaps the reference prism for the buff look: per
section the surface is **min(dome, plane)** — the band keeps a full crown
over the head (floored at `HEAD_DOME_CROWN` = 0.5 of the head's thickness,
drop law blended to a circular quadrant) and the facet is a radial cap at
the table plane, raised per angle just enough that the cut opens to the
outline's width. The plan ignores the outline entirely (the swell alone
shapes a lens — `cut_dome_heads_field_clean_and_never_pinch` asserts no
waist), the wall is the band's own chord (`head` weight fades with `dome`),
and every section is a single-plateau monotone drop, so the whole family
fields 0.000% by construction. Hard-won specifics:

- The facet inscribes at `HEAD_DOME_INSET` = 0.90 of the dome's span (the
  reference's 14.7 mm table on a 16.0 mm body): allowed to reach the band
  edge it exits through the corner at −7.7°.
- The arris is a **hard min**, deliberately: the smooth-min crossfade's
  documented 8.7%-of-radius overshoot raises a lip that measured a −3° ring
  around the facet. min() of a monotone fall and a constant is monotone.
- The `e_facet` raise moves the outer curve, so `edge_t` (the corner the
  wall and edge fillet build to) moves with it.
- The cab cap rides the facet cap (`HeadAt::cap_r = plane + cab`), or a
  buff-top's dome gets sliced off at the bare plane (−80°).
- **The facet is exactly the level curve — it cannot be masked smaller.**
  A cap restricted to the outline's own interval leaves a recessed flat
  beside proud dome, and that pocket locks: measured −89° at a heart's
  cleft. Concave-in-section features (a heart's cleft) are therefore
  impossible in this construction — physically, not as a code limit — so
  hearts and shields keep the prism; convex-per-section outlines (diamond,
  round, cushion, hexagon, cross — the plan may still be non-convex along
  the ring) are where the dome shines.

#### The lofted head is the factory construction

`SignetHead::loft` (0..1, default 0) builds the head the way CrossGems'
`Signet_Ring` cluster does, decoded from its wiring
(`tools/harvest/cluster_recipe.py`; the GH chunk parser's type table had
been guessed — points are 50–52, intervals 60–61, line/bbox/plane 70–72 —
and a full parse of the `.doc` to its last byte is what settled it). Their
head is **two surfaces joined at the ring's equator**: below it, the side
and bottom sections swept round the bottom half; above it, one *loose*
cubic B-spline loft — the curves are the surface's control rows, not
sections it passes through — through five closed plan curves at fixed
heights over the ring's centre: the table outline at 0.98, the outline,
the outline grown by the frontal/lateral distances 3 mm under the table,
and the ring's equator silhouette at +3 and at 0. That one sheet is the
"rest of the ring is smooth" read. The flat table fills the 0.98 row; the
band between 0.98 and the outline is the rim's roll, and it is a *ledge* a
tenth deep and millimetres wide, not a fillet.

**Every factory preset carries its finished ring as a cached mesh** (the
`Rings`/`Metal`/`Settings`/`Cutters` blocks after the parameters —
`cgpreset.py dump-all` unpacks all 448 into `assets/decoded/presets/`,
869 meshes with a gallery `index.html`; `obj_render --cg` draws them in
our frame). The recipe re-executed numerically (`tools/signet_tent.py`)
against preset 001's cached ring: crest 12.10/12.10 mm at the centre,
15.58/15.59 at the table's corner, widths within a tenth everywhere. The
same meshes fixed two things the recipe alone did not say: the shank is
cast **T + 0.25** thick, and its section is the top of a tall ellipse —
the unit profile curve stretched 4:1.33 — **3.0 × 1.49 mm on a 6 × 1.75
band**, crown 0.745·(T + 0.5). Not flat-topped; forcing it flat put the
shank 0.35 mm off.

`Tent` in `profile.rs` holds the five rows — resampled by arc length from
the −y seam, which is the correspondence the whole shoulder hangs on, and
blended by a clamped cubic on **uniform** knots — and reads, per ring angle:
the crest (the plane under the 0.98 row, else the first row down the loft
the ray enters), the bore span, the face as a chord 12% of the bore-to-
ridge drop below the ridge (never deeper than the ridge sits below the
plane, so the crown grows from nothing at the table's end), the ridge's
height over that chord as a **parabolic** crown — the loft's top is
parabolic, so the crown meets the wall tangent-continuously — and the
wall as a table of finger offsets (`WallShape`, `WALL_TABLE` 40) at equal
**arc length** between the bore edge and the chord's corner, radii stored.
The equator row is the band's own section silhouette at the shank's width
and the side's radius, so the loft's last rows *are* what the shank
sweeps and the two meet at the equator by a switch, not a fade. The
modulation API takes `&BandProfile` for this (`modulation`, `head_at`,
`signet_span`…): the loft needs the band in millimetres.

The flank **curls under** the table toward the finger — the crest span is
wider than the bore span, which the prism forbids. It is not an undercut:
every section is a single-valued width over its radius, so each wall
faces its own mould half, and `analyze_field` says 0.0000% on five
rebuilt presets. Measured against the cached meshes, exact point-to-
surface (`tools/harvest/deviation.py`): head 0.04–0.05 mm mean, shoulder
0.07–0.11, shank 0.07–0.11 — the bore is 8.65 against their 8.6, which is
half of it. A dihedral census leaves the shoulder at ≤ 12° between
facets; the only ≥ 45° edges left are the table rim's own corner and the
bore edges, which the reference mesh has too.

Five defects on the way, each seen in a render and pinned by a number:

- **A parameter blend between two descriptions of the same curve is
  lossy.** Fading the loft's chord-and-wall form into the band's crown-
  and-chord form over 82–89° cut a 0.15 mm groove at 86°. Make the
  forms coincide (the equator row *is* the band's section) and switch.
- **A wall table pinned to the chord's value at the crest's radius** put
  the value 0.1 mm too far out and the end-correction smeared 0.4 mm
  down the wall: 82° folds at the table's end. The table ends at the
  corner the crown stands on.
- **Evenly spaced radii chord across the rim's ledge** — 0.18 mm deep,
  3 mm wide at a cushion's end — and the chord swings between slices.
  Sample by arc length along the wall.
- **The 0.98 row is the table, not the outline.** Reading the flat from
  the outline row left a 0.18 mm fin of plane standing at its tip.
- Hand-drawn factory curves are not symmetric: the 001 cushion is 0.1
  fuller on +y at its ends (its +x tip sits 0.008 of the half-length
  further out on one side, and a near-vertical end turns that into 0.107
  in y), and the crest held on the parting plane then puts a curvature
  step at the ridge's apex. A builtin outline reads clean; it is the
  curve, not the construction. `CustomOutline::symmetrize` folds a table
  symmetric — opt-in, a heart is asymmetric by design — and the importer
  now densifies by arc length and smooths circularly like the exporter
  always did. The raycast itself had a gap: a ray through a vertex can
  round a hair outside both adjacent segments and find nothing, leaving a
  1e-6 spike that smoothing spread into a dip; it takes slack now, and a
  missed ray inherits its neighbours.

**The loft, measured against the loft itself.** The Rhino side baked the
head loft of presets 001, 005 and 016 as `.3dm` (rows and surface, in
`batch6-8/from-rhino/brief01-loft/`), and a loose `Brep.CreateFromLoft` is
exactly `S(u, v) = Σ Nᵢ(u)·rowᵢ(v)`: a clamped cubic across the rows on a
**uniform** knot vector (one interior knot at the middle of the domain on
every preset, however the rows are spaced), the rows paired by their own
parameter from their own seam. A Python replica on that rule reproduces
the 001 surface to 0.0000 mm; our chord-length-averaged knot (0.35) put
the sheet 0.053 mm off on average and 0.32 at worst, uniform knots take
that to 0.034 on 001 and to 0.017 / 0.003 on 005 / 016. Against the
cached meshes the capped 005 head went 0.088 → 0.045 mm mean (p90 0.178 →
0.064), 016 0.081 → 0.074, 001 unchanged. `the_loft_knots_are_uniform`
pins the knots and the analytic (P₁ + 2P₂ + P₃)/4 read at the middle. What
is left on 001 is the factory's own seam: its hand-drawn cushion starts
2.4 mm off the −y axis and the equator rows are re-seamed to match, so
the flank is twisted by a hundredth of the perimeter against our
symmetric pairing — the 0.1 asymmetry `symmetrize` exists for, seen from
the other side. The −y seam stays. Two more facts off the same files: the
bore through the head is a four-row loft that dips **0.245 mm** inward at
its middle (their comfort fit), and the row set of a Smooth-Table preset
in that capture was still the flat five, so the apex loft remains pinned
by the cached meshes alone.

**The smooth table is the same loft from an apex.** `table_dome_mm` on a
lofted head (0–3 mm, the "Cab dome" slider and MCP `head_table_dome_mm`) is
the apex height: the flat table's rim row is dropped and the loft runs
through six rows — an apex point `cap` above the table's centre, the
outline scaled to 0.6 about its centroid at that same height, the outline
at the plane, the body, and the two hull rows. Because the apex and the
0.6 row share a height the cap is a rounded **plateau**, not a point, and
the lobes of a clover or a star read as relief in the dome. Every angle
takes the ridge path: the crest is where the ray first enters the loft
(`(plane + cap) / cos` at the centre), the chord is read `LOFT_RIDGE_STEP`
of the bore-to-crest run below it, capped at the crest's own stand-off
from the plane so the law is continuous at the rim and a flat table is
untouched.

**The section's crown law is the loft's own**, not a parabola.
`TentAt::ridge` samples the loft's half-extent at evenly spaced depths
between the apex and the chord and inverts that onto `RIDGE_TABLE` shares
of the chord; `ShankMod::ridge_table` carries it and `sample_spaced` blends
toward it by `ridge_drop`. The parabola came first and rebuilt the head to
0.08 mm mean while drawing a pointed ridge along the dome — a highlight
line the factory mesh does not have, visible in a render and absent from
every section dump, because a plateau against a parabola is a difference
in curvature, not in slope. Measured against the cached meshes
(`tools/harvest/deviation.py`): the 2.7 mm cap rebuilds at 0.127 mm mean
on the head and 0.152 on the shoulder, p90 0.18 / 0.38 (the parabola's
shoulder p90 was 0.73); the 1.5 mm cap at 0.086 / 0.060. Both carry a
uniform +0.055 mm — our size-7 bore is 8.65 mm to the reference's 8.6 —
and the across-band top profiles match to a few hundredths once that is
taken out. With the cap off, the flat preset is unchanged at 0.047.

Every section is still a single-plateau monotone drop and a domed cap has
real draft everywhere, so the family fields 0.0000% by the same
construction as the prism. What it cannot do is a concave feature *within*
a section: a lobe reads in the chord's width and the crest line, never as
a hollow across the band. `a_smooth_table_is_an_apex_loft_and_still_pulls`
pins the apex height, the crest falling monotonically off the centre, the
chord opening past the flat table's, the verdict and the wall.
`examples/cg_signet.rs` rebuilds any decoded preset from its `params.json`
and `curves.json` — the smooth-table flags map onto `table_dome_mm` — and
prints the crest/width table in the cached mesh's frame.

**New signets are lofted, and the cut dome wins.** `apply_signet` sets
`loft = 1.0`, and every "new signet" path funnels through it — the GUI
kind switch, "Make this the band", MCP `set_shank` on switching to Signet,
the templates, the configurator bases, `SignetHead::lofted()` for extra
heads — while `Default` and the serde default stay 0, so a design file
without `loft` keeps the prism it was saved with. The two strengths
compose through `SignetHead::mix()`: `dome` takes precedence and the loft
runs at `loft · (1 − dome)`. That order was decided by a render. The CG
Clover on a 9 × 2.6 band went lofted and came out corrugated exactly like
the prism, because the loft's body row sits 3 mm under the table — below
the bore of a thin head — so the whole visible flank is inside the blend
from the lobed table row. Fairing that row with the rolling ball did not
help the thin case and cost the factory presets (the 2.7 mm cap's head
went 0.127 → 0.223 mm: the recipe really does carry the lobes down the
flank), so the loft stays faithful and `suggest_dome` keeps its authority —
a deeply lobed plan goes onto the cut dome whether or not the head is
lofted. Examples whose subject is the prism pin `loft = 0.0`; the cut-dome
ones need no pin. `a_new_signet_is_lofted_and_fields_clean` pins the
default, the serde fallback, the verdict, and the precedence.

#### Imported plans are outlines too

`SignetOutline::Custom(u8)` indexes `ShankStyle::custom_outlines` — a
`CustomOutline` is a 720-entry polar boundary table (~3 KB), carried **in
the design file**, so a custom head renders identically on any machine
with no asset in the loop. The table is the source of truth; the derived
silhouette rebuilds on first use like the tiling SDFs. The head
construction resolves through `ShankStyle::outline_extent` /
`outline_body_extent` / `outline_aspect` (the registry owners); the bare
enum methods fall back to Oval so a design missing its registry entry
degrades instead of panicking, and `Custom` stays out of
`SignetOutline::ALL` so every builtin picker and sweep is untouched.

Any closed polyline comes in through `CustomOutline::from_points`, which
runs the same recentre-fit-and-raycast the builtin polar tables use and
then the same rolling-ball fairing — so the containment guarantee is
inherited, not re-proven per shape.

**The fairing ball is per-import** (`CustomOutline::fair_r`, default the
calibrated `BODY_FAIR_R` 0.75), because 0.75 is tuned on a heart's two
gentle lobes and a four-lobe clover corrugates a flank it only smooths.
`CustomOutline::from_points` sizes it from the plan's convex-hull area
defect (`hull_defect` → `fair_r_for`: the default for a convex plan,
rising to 2.5 at a 15% defect — the exporter's own rule, so the GUI, MCP
and graph imports agree with it); closing is extensive at any radius, so
containment is not a function of the choice.
And the table's *quality* matters as much as the machinery: 256
uniform-parameter curve samples left chord kinks that swept ripple bands
down every wall (clover max second difference 0.0121; 0.0044 after
arc-length densify + a 0.75° circular Gaussian — the residual is the
notches' own curvature).

**Deeply lobed plans default onto the cut dome** via
`ShankStyle::suggest_dome`: ≥4 prominent radius maxima plus real hull
defect (`fair_r > 0.9`) is the lobed signature — a square's four corners
have the maxima but no defect, a shield or heart has the defect but only
three maxima, so both keep the prism and its doctrine-approved single
cove. On a prism a clover's notches ride the whole flank as corrugation;
on the dome the body is one smooth lens and the lobes read in the arris,
which is the two-smooth-surfaces read a signet wants. The GUI picker and
MCP apply the suggestion on outline change; the dome slider overrides it. `a_drawn_outline_makes_a_head_that
_pulls` pins it with a deliberately hostile plan (an asymmetric clipped
star): containment to 1e-5, field-clean, and a JSON round-trip that must
not carry the derived table. The library half lives in
`library::outline_dir()` (`<name>.outline.json`); applying one **copies**
it into the design. 19 factory signet plans decoded from the CrossGems
presets (`tools/harvest/outline_export.py`) ship there as user assets —
clover, rosette, star, butterfly, escutcheon and the rest — every one
fielding 0.000% on a bare head.

#### Outlines have to survive being turned into a silhouette

`SignetOutline::half_extent` is a cached table per outline, built by scanning
inward from the extent — inward, because a heart's lobes leave a gap at their
own height and a band's width is the outermost reach, not the first crossing.
Three things it must get right, each of which was wrong once:

- **Read it signed.** `head_at` returns a signed `x`, because a shield's flat
  top and its point are opposite ends of the head. Folding the head about its
  centre gives a shield two flat tops, which is a square.
- **Fit polar outlines to their bounding box, not to their radius.** Scaling
  two axes by different factors moves every boundary point round the circle, so
  its new radius belongs at a new angle; and scaling about the origin only works
  if the shape is centred there. A heart reaches four times as far to its point
  as to its lobes, so dividing by the larger extent squashed the lobes to a
  sixth and the outline came out a lens.
- **Stand the asymmetric ones up.** A shield lying on its side is not a shield.

What phantom is left lives only on the upright outlines, and for a reason: the
section they sweep is not symmetric about its own crest, so the facets
straddling it do not cancel. After the one-edge rebuild every symmetric
outline reports **0.0000% at every preset**; a shield goes 0.0149% at Draft to
0.0024% at Export and a heart 0.0049% to similar — converging, but not to zero
at any resolution worth paying for. `mesh::tests::scratch_signet_head_undercuts`
therefore asserts that what is reported stays tiny **and stays on the crest
line**, which is what tells a phantom from a real undercut. That check is what
caught the -19.4°.

`SignetLayer` still exists and is still a pad standing on the band. That is the
right thing for a flat facet on an ordinary band and the wrong thing for a
signet, where it leaves a disc glued to a ring. The layer editor offers
**Make this the band**, which moves its outline, length and stand-off onto the
head and deletes the layer.

### Seamlessness

Anything positioned by an **integer count around the ring** closes on itself
automatically, because `u` wraps at the circumference. That is why
`TilingLayer::repeats_around`, `MilgrainLayer::beads_around`, and
`BorderLayer::rope_twists` are all `u32`. Do not make them floats.

Alphas must also tile seamlessly in themselves — `Procedural::generate` builds
every pattern from functions periodic in both axes. An **imported** image is
used as drawn, seam and all: `Alpha::make_seamless` exists and is tested and
nothing calls it, which is a gap rather than a mechanism — it becomes an
`alpha.transform` op in M16.

### Stability under shank modulation

The shank taper changes the cross-section per angle, so `ProfileLoop::v_mm`
differs per angle too. Layers are evaluated against the **reference** profile:
`v_norm = p.v_mm / loop_i.surface_len_mm` then `v = v_norm * ctx.band_v_len_mm`.
The pattern therefore follows the band as it tapers instead of sliding across
it. `mesh::build` and `castability::section_at` must do this identically — if
they diverge, the section view lies about the solid.

## Two things the model does not yet know

Recorded here because they are doctrine-level, not tickets: a reader of this file will otherwise
assume both are handled, since everything around them is. The 2026-08-30 audit
(`docs/AUDIT-2026-08-30.md`, roadmap M10-M24) established them, and both are still open.

**The verdict is radial and cannot see the axial web.** `thinnest_wall` walks bore points and
interpolates the surface at the same `z`; `bore_span_wall` bins by `z` and subtracts bore radius
from surface radius. Both measure outward from the finger hole. Nothing measures metal *across* the
band — which is exactly where this file sends every deep carve. `OpenworkLayer::height` says so
itself: the carve eats `depth * nr` of radial metal, so on a side face where `nr` is ~0 the cap
opens up and `depth_mm` is the only limit left. A 1.8 mm band carved 0.8 mm from each face leaves a
0.2 mm web and reports a healthy wall. Four independent audit dimensions found this, and it is a
**fill** rule, not a draft one: a thin web does not lock the mould, it comes out of the flask as two
halves. Until `min_local_thickness_mm` exists on `FieldReport` (M11.1), do not trust a wall figure
on a design carrying opposing side-face relief.

**The model has one stage where the trade has two.** Every entry in the `LayerStack` is cast
geometry, judged by `analyze_field` against the sand's 0.30-0.40 mm detail floor. That is correct
for cast relief and it is why the showcase records that the guilloche generators carry 7-24 periods
per tile and fall under the floor at any tile size a band holds. But in the trade the fine work is
not cast: bright-cut, wriggle, ramshorn scroll, Florentine line, true rose-engine guilloche, inside
lettering and a signet's seal are cut into the cast blank afterwards, with a graver or a machine.
There is no way to say "this line is cut at the bench", so such a feature is either refused as
NotCastable or measured as mush. `LayerEntry::stage` (M14) is the fix, and it is the prerequisite
for intaglio, monograms, crests, hallmarks and everything on the bore.

## Manufacturing analysis speaks in the sand's numbers

`DraftSettings` carries the sand itself: `min_draft_deg`, `min_section_mm`
(the fill floor the field verdict checks), and `min_detail_mm` (the feature
floor), with `SandProcess::{DelftClay, Petrobond}` presets writing all three.
`DraftSettings::process` picks the **casting process the verdict judges
against**: `CastProcess::SandTwoPart` (default, the app's home ground) or
`CastProcess::LostWax`. The geometry model is identical either way — under
lost wax the pull statistics are still measured and reported (with an
explicit "cannot move to sand as-is" note when undercut exists) but never
gate; only fill and detail do, at investment floors (0.5 mm section,
0.15 mm detail). Generators read the process and switch construction:
`pave::halo` builds the sand plate-and-markers form or the classic proud
melee ring. `lost_wax_frees_the_halo_and_the_verdict_says_which_is_which`
pins both directions. On top of that:

- **Per-layer DFM** (`dfm.rs`): layers are analytic, so their finest feature
  is a parameter, not a measurement — `feature_footprints` against
  `min_detail_mm`, surfaced as warning badges on the layer rows, in the
  report notes, and on MCP as `castability.dfm`. A texture is the
  exception: a tiling's cell pitch says nothing about the strokes inside
  it, so `findings_in(design, lib)` — what every app surface calls — also
  reads each tiling's and openwork's mask by **granulometry**
  (`Alpha::min_feature_px`: the opening diameter at which a tenth of the
  ink, and of the gaps, disappears, bisected over the radius on the
  distance field, 3×3-tiled so a seamless mask reads seamless, cached by
  content) at the layer's own mm-per-texel, after the layer's
  contrast/bias/invert shaping. Greek Key on 2.7 mm cells passes the
  pitch check and measures 0.06 mm strokes — that is the finding. A
  **stamp** is measured the same way at its own mm per texel, with the
  alpha stretched by the section's own arc ratio at the stamp's station
  first — the chart's `v` is that arc normalized, so on a lobe three
  times the reference thickness a stamp stands that much taller than it
  is wide — and the measurement replaces the footprint's 15%-of-size
  guess, which called a 2.25 mm hook with a 0.45 mm stroke mush. Run on
  the shipped templates (`dfm::measured_tests::the_templates_measured`,
  `--nocapture`) it names three: Waves at 0.04 mm strokes on the waved
  hexagon signet's 11.8 × 0.8 mm cells, Chevron at 0.03 mm gaps on the
  shouldered cushion's 7.6 × 0.6 mm shoulders, Braid at 0.04 mm gaps on
  the braided band — all castable by the field, all casting softer than
  drawn, which is what the chip now says instead of nothing.

  **A tiling is measured at the tightest station its window covers**, not
  against the reference section. The chart's `v` is the section's own arc
  normalized, so a cell is the reference height only where the band is the
  reference thickness; a shoulder that narrows to a third of it carries
  cells a third as tall, and the mask's strokes with them. Reading the
  reference context alone under-reported both signets by about 2.5x — the
  two figures above moved from 0.10 and 0.07 when `worst_arc_ratio` was
  added, and the finding now names the angle (185° on both). A *decal* had
  always done this per station; a tiling covers an arc, so what matters is
  the worst one in it.

  A **flute** is measured *around* the ring, not across the band. It was
  filed as a `FeatureFootprint::across` — whose own doc named a flute —
  which sets `feature_u_mm` to infinity, and `metal_feature_mm` scales only
  the `u` side by the arc ratio. So reeding was the one ornament that
  evaded the arc-scale correction entirely, reported 15-20% coarser than
  the metal it becomes. `FeatureFootprint::along` is the right shape, and
  the **land** between two flutes counts as well as the cut: a dense
  reeding fails on its land first, and only the cut was ever measured.
- **Undercut attribution** (`castability::attribute_undercuts`): undercut
  arcs are clustered in theta off the field samples, then each enabled layer
  is muted in turn at the *same parting plane* — the culprit is the layer
  whose muting clears ≥80% of the arc. "Undercut 74–106° on the lower
  shoulder: 2.1 mm² leaning to 18° — caused by "Flat boss"; muting it clears
  it." Runs only when there is undercut to explain;
  `attributed_field_report` is what the GUI worker and MCP call.
- **As-cast preview** (`BuildParams::soften_mm`, toolbar "As-cast"): the
  height field evaluated through a 9-tap Gaussian at the sand's detail
  radius, so beads merge on screen the way they will in the pour. Preview
  builds only — exports and the section view stay at true geometry.
- **Patternmaker's shrink**: `Metal::shrink_pct` (silver 1.9%, golds ~1.3%),
  `metal::pattern_scale` = `1/(1−s)`; the export menu's "Shrink for" combo
  and the CLI's `--shrink` scale the mesh and *rename* the file as a pattern
  so an oversize file can never be mistaken for nominal.
- **The size-run CLI** (`crates/ringdesign-cli`): `ringdesign export
  ring.json --sizes 5:9:0.5 --formats stl,3mf --shrink sterling` builds each
  size, field-checks it, writes the files and a manifest CSV (verdict,
  thinnest wall, volume per size) — one flask, one command; the manifest is
  also the export-regression diff. `ringdesign check` prints the field
  verdict and the stones findings for one design. `ringdesign graph
  eval|check|describe <graph.json>` evaluates a graph file the way the
  app does (same registry, script engine attached): `--set Name=value`
  sets an exposed parameter, `--preset` a saved preset's values,
  `--out` writes the evaluated design with its graph embedded, and
  `--run-sinks` runs the side-effect sinks afterwards. The binary lives in
  its own crate because the core cannot depend on the graph.

### The profile library is user-extensible

`library::profile_dir()` (designs sibling `profiles/`) holds saved
cross-sections, one `<name>.profile.json` each. `save_profile` /
`list_profiles` (+ `_in` variants for tests) round-trip the full
`BandProfile`; `BandProfile::apply_shape` applies one while keeping the
band's own width and thickness — **a profile is a section, never a size**.
The GUI's profile panel draws every entry — preset and saved — as its own
little section (`profile_row`), and a name-plus-Save row files the current
shape. 

`tools/harvest/cluster_curves.py` decodes their master preset library
(the Profile_Curves cluster: 23 unit-box sections whose true index → name
mapping is the C# script input's Source order — the containers' own
nicknames disagree with the published selector list in places) into the
manifest `examples/import_profiles.rs` consumes. 16 of the 23 import
under their true names (`CG Round` … `CG Tapered Smooth`, stepped Triple/
Second Floor silhouettes included); the 7 skips are honest — flat crowns
our squared presets already are, and multi-crest valleys a single-crest
band cannot be. Imported sections, like every profile asset, live only in
the user dir. `examples/profile_gallery.rs` renders the whole saved
library on one band as a contact sheet — the Saved picker at a glance and
the roundtrip check in one image.

### Generators are live until baked

The lesson, in this model's idiom: a generated group carries the
`GenRecipe` that made it (`GroupLayer::recipe` — Pavé, Halo or Channel
spec, serde'd into the design file). While the recipe is present the group
is **live**: `pave::regenerate_live` re-runs every live generator and
replaces its stack in place, and the GUI calls it from `mark_dirty`, so
editing the recipe *or the band under it* re-solves the layout — change the
band width and a pavé re-packs. The entry's own window/blend/mask stay the
user's. A recipe that no longer fits keeps its old stack and says so.
**Bake** clears the recipe and the layers become hand-owned. Builds and
analysis never regenerate — a design file renders exactly as saved, and the
recipe is editable provenance, not a build input.
`live_groups_regenerate_with_the_band_and_bake_detaches` pins the
lifecycle. The eternity `SeatRun` needs none of this: it is already a
parametric layer.

**Pins are the third state, and they existed as a bug first.** The layer
panel offered a seat editor and a delete button on the children of a live
group, and `g.stack = fresh.stack` destroyed every such edit on the same
frame — silently, because the panel then redrew the regenerated seat.
`PaveSpec::pinned` is the fix: a `PinnedSeat` is data *in the recipe*, so it
survives by construction.

A pin and a deletion are the same claim said twice — *the packer may not
place here* — so they are one mechanism, and a pin merely also emits a seat
(`seat: None` is a hole). Stated as a **region**, never as the identity of a
station: generated stations are recomputed from scratch every time the band
moves, so matching a stored one by position would quietly cull a neighbour
instead the moment the pitch changed. `blocked()` tests ellipse against
ellipse in the chart with the spec's own `bridge_mm` between them, so an
elongated pin claims the ground its plan actually covers.

What must not be taken from CrossGems here is their **solver**. Their
`CgPhysicalSystem` relaxes free positions (`step = MoveSum / (WeightSum +
Mass)`, velocity damped 0.99); `fill()` is a closed-form lattice, and its
`n = floor(circumference / pitch)` with `step = 360/n` *is* the seamlessness
guarantee. Take the lock semantics, leave the relaxation — and
`pinned_seats_survive_regeneration_and_the_packer_yields_to_them` asserts
every unpinned seat still lands on the integer lattice, which is the
assertion that would catch anyone reaching for it.

One knock-on: `stones.rs` rolled a fill up to one line by demanding a single
prototype, so one pin carrying a different stone would have collapsed the
rollup into two hundred rows. `seats_by_shape` groups by shape instead and
emits one line per distinct seat.

### Pavé is a generator, split is a modulation, the sheet is HTML

- **Auto-pavé** (`pave.rs`): packs an arc × v-band (or a side-face run) with
  gypsy seats — hex-staggered rows, full-ring rows wrap-exact with integer
  counts, capped at 240 seats *with the refusal said out loud*. The output is
  an ordinary Group of `SeatPadLayer`s: every seat stays editable, and the
  stones report rolls a uniform seat group up to one line instead of two
  hundred rows. Gypsy mounds because that is the measured-safe row on curved
  ground.
- **Graduated runs**: `SeatRunLayer::taper` (0..0.85) shrinks the stones
  toward the far side of the ring — cosine in angular distance from
  `taper_theta_deg`, so a full-ring run stays seamless and C1 at both
  poles. Seats scale whole (footprint, stand-off, skirt) so a graded row
  is still a row of self-similar mounds, and the report sums the graded
  carats via `SeatCheck::carats_override` instead of count x largest.

  **The stations move too.** They used to sit at a uniform `k·360/n` while
  the seats shrank under them, so the metal between neighbours grew with
  every step: measured 0.42 mm at the large pole against 3.05 mm at the
  small one on a taper-0.85 row — a **7.29x** spread down what is meant to
  read as one continuous line (`examples/graded_probe.rs`). Holding the
  *bridge* constant instead makes `R dΔ = span·scale(Δ) + bridge`, and
  `scale_at` is exactly a raised cosine in Δ, so this is
  `R dΔ = A + B cos Δ` — which integrates in closed form by the
  eccentric-anomaly substitution. `eccentric_warp(x, c) = 2 atan(c tan(x/2))`
  in its branch-safe `atan2` form, `c = sqrt((span(1−t)+bridge)/(span+bridge))`,
  stations at uniform warped angle. No solver, no iteration.

  Three properties fall out, and all three are load-bearing: at `taper = 0`
  the warp is **the identity**, so every ungraded row is bit-identical and
  no design migrates; φ is a monotone reparameterization of the circle onto
  itself, so an integer count still closes exactly at `u`'s wrap; and the
  count the law wants is the circumference over the **geometric mean** of
  the two pole pitches, `∫dΔ/(A + B cos Δ) = 2π/√(A²−B²)` and `A²−B²` is
  their product. Measured after: 1.18x at taper 0.85. `bridge_at` inverts
  the same identity — the positive root of `b² + b·span(2−t) + span²(1−t) =
  (C/n)²` — so what the report says is what the row holds, within 3%.
  `theta_of_station` / `station_of_theta` are the one lattice; the field,
  the stones report and the gem preview all go through them, because three
  copies of a station formula is exactly the divergence this file warns
  about elsewhere.
- **Shared prongs**: `SeatRunLayer::shared_prong_mm` stands one post pair
  at each boundary between neighbouring stones — the Prongs_Row
  rule (pair each gem with its shift-by-one neighbour, prong the boundary,
  cull only an open row's wrap pair) read into the height field, where a
  full-ring run keeps every boundary and the window handles open arcs.
  Posts grade with their stones. **Lost-wax stock**: proud posts flank the
  column off the parting plane and lean under a two-part pull — measured
  2.8–3.0% at −62° on a low dome, converging (`examples/prong_probe.rs`) —
  so in sand the report says "cast flush and bead-set or judge for lost
  wax", the field verdict enforces it, and the sheet prints the claw stock
  (pairs, post Ø, proud mm) for the setter either way.
- **Tilted runs**: `SeatRunLayer::tilt_deg` turns every stone of a row in
  plan on top of the seat's own bearing — 45 sets a square stone on the
  diagonal, which is what the factory eternity's boolean "Tilt Gems" means,
  and it composes with the graduation. One helper, `turned`, is the only
  way the row reads its seat's plan — span, height, footprints, prong
  offset, the stone record and the report's check all go through it — so
  the row re-packs to the reach it actually has and `bridge_at` still says
  what the solver asked for. A princess plan is a rounded square
  (`plan_pow` ≈ 4), so its diagonal support grows 1.19×, not the box's
  1.41×. A turned convex plan is still one monotone mound: measured clean
  on the crest line and on a 7×5 side face. On the default 2 mm band the
  same 2.5 mm row spills over both edges of a 1.7 mm face at 3.0% — that
  is the stone being too big for the face, not the tilt.
- **`ShankKind::Split`**: the castable read of a split shank. A real split —
  two crests — is a valley no single parting plane clears; instead the band
  flares 55% over a 110° arc while a channel is carved into *each side face*
  (`ShankMod::side_groove_mm`, capped at 0.35 of the half-width). The
  groove's floor faces along the pull and its walls stand radial, so the
  whole ring fields 0.000% — side-face doctrine, applied to the shank
  itself. Seen side-on, the ring reads as two diverging rails.
- **`ShankKind::Bypass`**: the castable read of a bypass, and the first
  draft of it was wrong in a way only a render showed. A bypass is two
  arms passing each other over the top; what says "bypass" is the **plan
  outline** — each arm's rounded tip ending past the top while the other
  continues beneath it. The first draft slid the section ±z through a
  symmetric widening and rendered as a broad wave. The construction now
  is two explicit arms (`bypass_arm`), each the band's own width, sliding
  to ±`BYPASS_OFFSET` (0.45 of the half-width, under Wave's 0.6 cap) over
  the 100°→30° before the top and ending in a rounded tip
  (`BYPASS_TIP_DEG` 35°, rounded over 30°); the section at any angle is
  the **union** of the arms present (`bypass_span`), so the tips show as
  steps in the outline, with the re-entrant corner a real bypass has
  where a tip meets the other arm. The crest rides the parting plane by
  Wave's mechanism, and the seam between the arms is Split's side-face
  channel (1.0 mm at the crossing) — a seam along the crest would be the
  valley no parting plane clears. Measured 0.0085% at −0.8° on a low
  dome, 0.0000% on a flat band; `examples/bypass_probe.rs` prints the
  table and renders hero, top and side views.
- **The casting sheet** (`spec.rs`): one self-contained printable HTML page —
  dimensions, weight in every alloy with its pattern scale, the field
  verdict with notes and DFM findings, the stones table with bench warnings,
  provenance. Desktop File menu and the Android share sheet both emit it.
- **The stone map** (`stonemap.rs`): the setter's sheet — every stone the
  design sets drawn to scale at 2:1, in plan (each girdle projected onto
  the ring's plane from the census's own `StoneFrame`) and on the band
  unrolled (each plan at its chart bearing), labelled with its size, with
  the census's tight pairs drawn in red between the stones and the gap
  that decides written on the line. File > Stone map…, MCP
  `export_stone_map`, CLI `--formats stonemap`, the phone's Share menu.
  Refuses a design that sets no stones.
- **The parting line** (`castability::parting_line`): the widest surface
  point per slice — where the sand parts — exported as a printable SVG
  (plan view plus the line's height unrolled, File > Parting line…).
  `ShadeMode::Halves` paints cope blue and drag sand with the parting band
  bright, from the object-space normal's axial share, on both apps.
- **The comparison ghost**: the toolbar's Ghost checkbox pins the current
  *design*; the viewport draws its mesh translucent (third VBO, `u_alpha`,
  no depth writes) and the section view cuts the pinned design at the live
  angle and dashes its outline — resolution-independent on both counts.
- **Ctrl+K** opens the command palette (`panels::Command`, one enum arm per
  action); Ctrl+S/O/N and Delete-layer ride the same dispatch.
- **The viewport probe**: click the 3D view to ray-cast the built mesh
  (Möller–Trumbore over every face — a millisecond on a click, no BVH):
  readout of θ/v/relief/wall/class, and the topmost contributing layer
  becomes the selection. Shift-click drops pins; two pins measure mm.
- **Channel set** (`pave::channel_set`): two rails flanking a recessed
  channel, one Group gated to the wider side face — the only place a
  channel's walls stand parallel to the pull. It is honestly a *thick-band*
  feature: stone + two rails need ~2.9 mm of face for a 1.5 mm stone, so
  the generator returns `None` on a thin or domed band and the menu item
  says what to change. Stones set at the bench, as always.
- **Halo** (`pave::halo`): a centre stone on a domed plate ringed by melee.
  A ring of *proud* accent mounds does not cast — each sits off the crest
  and makes a two-flange valley with the centre (measured 1.4% at −33°) —
  so the halo casts as a **clean gypsy plate** (one gentle dome, 0.000%)
  with the centre seat on the crest, and the accent ring rides the plate as
  **zero-height markers**: the report counts them and the gem preview stands
  each stone on the plate, but they raise no proud geometry. The setter
  drills and beads the melee into the cast dome, which is how a cast halo is
  actually made. `HaloSpec` sizes the plate from centre + gap + melee ring.

### Exports beyond the mould

STL/OBJ/3MF cut patterns; three more formats exist for everything around
the pour, all hand-rolled in core with tests:

- **GLB** (`gltf.rs`): two-chunk binary glTF, PBR metal tinted to the
  chosen finish, coordinates ×0.001 because glTF's unit is the metre — the
  one convention every viewer agrees on. For renders and web viewers, never
  for casting.
- **PLY** (`stl.rs::write_ply`): binary little-endian with normals, for
  scan and measurement tools.
- **Renders** (`render.rs`): the examples' software z-buffer rasterizer,
  promoted to core so CLI, GUI, tests and any future configurator can draw
  a ring without a GPU. `write_png` is one supersampled hero frame;
  `write_turntable_gif` is a looping 36-frame spin. The GUI File menu has
  both, tinted to the finish; `examples/template_shots.rs` renders the
  whole template gallery for an eyeball pass.

  A frame is a list of **`Part`s**, framed on the first, because a finished
  piece is metal *and* stones and the stones are never in the `Mesh`.
  `gems::preview_mesh` is the same stones the viewport draws, as a plain
  mesh — a soup of loose triangles, emphatically not something to export.
  Metal shades from the mesh's own vertex normals (`Part::smooth`); flat
  facet shading put a visible contour on every ring of triangles, worst
  where the surface is nearly flat and the normals alternate, which is the
  skirt around a seat. A stone keeps flat facets, because there the facets
  are the point.

The CLI speaks all of them: `--formats stl,obj,3mf,glb,ply`.
`Report.quality` (`Mesh::quality`) carries worst-triangle statistics — min
corner angle, aspect, degenerate count — on the report panel and the sheet.

### Templates are code, and the field verdict edits them

`templates.rs` holds the File-menu gallery (and MCP `apply_template`): nine
starters built from the same API the panels drive, so they cannot go stale
against the format. Only builtin alphas, so they open identically on an
empty machine. The test holds every one to `analyze_field` — and that test
did real work the day it was written: showcase 5's rails-and-milgrain crest
composition, blessed for months by the mesh analyzer, fields **5.8% at
29°** — off-crest rails on a near-flat crown lean back on their crest-side
flank exactly as the wire-layer table said. Beads *at* the crest line
survive; rails beside it do not. The templates now carry the composition
that passes.

The other lesson with teeth: **builtin procedural tiles carry several
pattern periods per tile** (Scales is 7×7 scallops, Greek Key a 3×3
meander, `Voronoi` 3×3 sites, `Trellis` 4 wires each way), so
`repeats_for_square_cells` lands each *period* at a fraction of the cell —
sub-detail-floor mush on a 2 mm side face. Templates carry hand-tuned
counts with the per-tile period in a comment, and fine-lined alphas (Greek
Key at 0.15 mm strokes) are simply not usable on narrow faces in sand.
`every_pattern_tiles_seamlessly` pins the whole family's seam step against
its own interior step; `Voronoi` (cellular relief, from Auto_Voronoi) and
`Trellis` (a round-wire lattice, from Wire_Pattern) are the castable reads
of the decoration clusters, both side-face features by
construction.

### The showcase is the measured tour

`examples/showcase.rs` builds thirteen finished designs — signets and bands,
one or two feature families each — field-checks every one (the run refuses a
NotCastable ring), prints DFM findings and stone warnings, renders hero/face
shots plus turntable GIFs, and saves `designs/showcase/*.ring.json` so each
opens in the app. Lessons it measured, kept in its comments:

- A bead row rides **the crest line only**: straddling the parting plane it
  splits between cope and drag; the same row 1.9 mm off-crest locks at −37°
  over 4.8% — the off-crest-rails lesson, in bead form.
- Pit textures lock near the crest by their own slope (a full-crown hammer
  peen fields 2.7% at −9°); peen the *flanks* and keep a polished crest
  ribbon. Melon lobes along the ring are the two-flange valley: 8.2%/−22°.
- The guilloche generators carry 7–24 periods per tile — under the sand's
  floor at any tile size a band face holds. A one-circle SVG mask (one
  porthole per cell) is the pierced look that survives.
- **The (u, v) chart reads true from −Z.** A text stamp on the high side
  face casts mirrored unless `Decal::flip` is set; the configurator's
  `compose()` and the showcase both derive it from the face the stamp landed
  on, because `wider()`'s tie on a symmetric band breaks on float noise.

`examples/collection3.rs` is the same tour over one day's additions — a
bypass solitaire, a diagonal princess band, a hollowed clover signet, a
cabochon cigar band, a graded prong eternity judged for lost wax — each
rendered with its stones, given its sheet and setter's map, and saved to
`designs/collection3/`. What it measured, all of it from the verdicts
rather than the renders: **a stone is sized to its face, not to its
band** — a gypsy seat carries 1.8 mm of stock round its stone and a
turned square reaches 1.19× further, so 2.5 mm princesses overhung a
5 mm band's face by a millimetre (bosses carry 1.2 mm; 1.8 mm stones on
a 5.5 mm band leave a quarter millimetre either side), and a 5 mm cab's
bezel overhung a 4.5 mm band's face by 1.5 mm and leaned to −68° where a
7.5 mm cigar band gives a 4 mm cab a 6.4 mm face; **a bypass crossing
needs room under a stone** — on a 4.5 mm band the mound's skirt left a
wedge against the crossing's edge that the wall sweep read as 0.67 mm,
clean at 5.0; and **a seat's skirt is its finest DFM feature**, read at
the chart's 0.85 metal scale, so a 0.4 mm skirt measures 0.34 against a
0.35 floor. The hollow takes 11% out of a 12 mm clover signet.

`examples/sketches.rs` is the same discipline on pencil sketches, built
here and through CrossGems' own components for the side-by-side
(`SKETCHES=A,N` runs only the named designs; `NOCT_*`, `BOLT_*`, `CLOUD_*`
knobs probe one). What it measured: a **cast dot on a signet's table must
ride the parting line** — a 0.2 mm dot 2 mm off it on the zero-draft
table leaned 14° on its near flank, on the crest line it is clean; a
plate wide across the band curls the loft's flank under it and the wall
over the hole thinned to 0.62 mm where along the band it holds 1.30; a
**collar across the whole outer surface** (a flat curve wire from bore
edge to bore edge, eight round the ring on a beveled band) fields 0.000%
— its flanks face round the ring, parallel to the pull; and a **cloud is
a keyframed band** — five lobes lifting the crest and widening the plan
with dips between, every section still one dome — whose curls go on the
lobes' side faces as stamps, each drawn squashed by its lobe's own
stretch (2.9 / 2.5 / 1.7) so it comes out round, centred and sized from
the face the modulated section actually has (3.6 / 3.0 / 1.9 mm tall):
raised onto the crown they leaned 12–19°, fitted to the face 0.007%.

`examples/commissions.rs` is the same discipline applied to client sketches
(half-wrap patterned crescents, a gem-set cross band, the Diamond and Cross
`SignetOutline`s added for them). Its own measured lessons: a gem column
must run **along the parting plane** — a table mound straddling it splits
between cope and drag, while the same mound 2.3 mm off it locks at −9°;
a bur dimple locks even at the crest (a pit's walls are a dome's inverse);
and gypsy-skirted seats at column pitch merge into one ridge, so spot
mounds carry `gem + 0.9` diameters instead of full seat stock.

### The workbench conveniences, and where they live

Exports run **off the UI thread** — `export.rs::spawn_export` snapshots
(design, lib, params, shrink) into an `ExportJob`, one at a time, reaped by
`poll_export` into the status line. Layer rows drag-reorder (grip glyph,
`dnd_drag_source`, drop inserts) and **alt-click on the enable box solos**.
The unrolled canvas pans (right-drag) and zooms (Ctrl+scroll, pointer
anchored, u wraps — the closures carry the view, so grips, paint and the
field texture all follow); a selected layer's angular window shows draggable
arc grips (centre, edges, fades). `ProcRecipe` gives the builtin generators
knobs that cannot break seamlessness — integer repeats, quarter turns,
value gamma — stored in the design and baked by `bake_all`. Seats carry an
optional **bur dimple** (`dimple_mm`); tilings a **helix shear** and
**k-fold kaleidoscope** in `u`. The worker's settled pass runs
`castability::modulus_scan` (Chvorinov area/perimeter per slice) and the
report names where the ring freezes last; `prices.json` beside the designs
folder prices the metal table; File > Cost JSON writes the volume/weights
interchange for the sibling calculator; the section view draws each seat's
stone to scale, girdle on the pad and pavilion into the metal.

## Stones are stock, not geometry to cast

Stones are set at the bench; the ring casts the *stock* for them — bosses,
bezel collars, gypsy mounds, prong bumps. Three pieces keep that honest:

- `gem.rs` — cuts, carat estimators (anchored at 6.5 mm round = 1 ct),
  calibrated stock sizes, girdle/pavilion proportions.
- `stones.rs` — the analytic bench-check report, surfaced in the report
  panel's Stones section and on the MCP `castability` tool. Per seat: what
  the base surface under it is (side face = castable by construction; a
  crown reports its draft — and the warning keys on a *flat top's rim* on
  low draft, because that is the measured 8.6% hazard, while a fully-domed
  mound on the same base measures 0.000%), foot-to-band-edge clearance vs
  `MIN_EDGE_MM` — measured along the section **as modulated at that
  station**, so a keyframed or signet top carries the seat its reference
  band could not (read off the reference width it refused a 4 mm princess
  on a 3 mm band widened to 7) — metal available for the pavilion along the seat's normal
  (to the bore wall on the crown, across the band on a side face) vs
  `gem.pavilion_mm()` + `MIN_WALL_MM`, run bridges vs `MIN_EDGE_MM`, and
  carat totals — plus the pairwise crowding census below. Analytic — it
  reads the layers and the modulated bare profile, never the mesh, so it
  costs nothing and cannot disagree with the design.
- `gems.rs` (GUI) — render-only faceted previews: one superellipse-plan
  brilliant per stone-bearing station, positioned by evaluating the
  *displaced* section under the seat, girdle settled into the pad so the
  pavilion vanishes into metal and the crown stands proud. Flat facet
  normals under the viewport key light do the sparkle; drawn as a second
  buffer in the same GL program, toggled by the toolbar's Stones checkbox.
  **Never in the `Mesh`, never exported** — `RD_GEM_SHEET=/dir` on the
  `stones_land_on_their_seats` test writes a software-rasterized sheet for
  eyeballing placement.

### One record per stone

`setstone.rs` is where a stone *is*. `set_stones(design)` walks the stack
once — pads carrying a gem, every kept station of a run at the size the
grade gives it and on a seat scaled with it, pavé seats and halo markers
inside groups, windows honoured, disabled entries skipped — and returns a
`SetStone` per stone: label path, source, `theta`/`v`, the gem, and the
seat as that stone meets it. The report's crowding census, the gem preview
(`gems::preview_vertices`) and the section view all read that list and
nothing else, so they cannot disagree about where a stone is. Before it,
the report and the preview each walked the layers themselves and the
section view knew only pads and runs — a pavé group's stones and a halo's
melee never appeared in a section. `stones_near` is the section view's
filter: every stone whose seat reaches the slice. The per-layer
`SeatCheck`s (pavilion room, edge clearance, bridges, prongs) still come
from the report's own walk, because they are properties of a seat *layer*
rather than of a stone; `every_consumer_counts_the_same_stones` pins the
report's count and carats to the record's.

### A seat is the stone's plan, not a circle round it

`SeatPadLayer` carries `elong` (long over short, `diameter_mm` staying the
short axis), `rot_deg` (bearing in the chart) and `plan_pow` (the
superellipse exponent), and `fit_stone` fills all three from the gem. A
marquise used to get a round boss sized off its *width*, so the stone
overhung its own stock by 0.6 mm at each end.

- The rim is `field::superellipse_radius_mm` along the sample's own ray, and
  every drop law then reads `d / r` exactly as it did. The skirt stays a
  **millimetre width** in every direction, because it is measured from the
  rim outward rather than as a fraction of the radius — normalizing instead
  would thin an authored 0.5 mm blend to 0.25 mm at a marquise's point,
  straight through `MIN_EDGE_MM`.
- Prong bumps stand on the plan outline, on its axes for a round plan and
  at its **corners** once `plan_pow` passes 2.5 — a princess is claw-set at
  its corners, and the claws land where the girdle is.
- `plan_pow` is floored at 1, which keeps the plan **convex**. Convex is
  star-shaped about the centre, so a mound on it is still a monotone drop
  from a single crest and releases wherever a round one does — measured
  0.000% on an emerald-cut eternity. A re-entrant plan (a heart's cleft)
  would put two skirts face to face, which is the two-flange valley the
  showcase measured at 8.2%/−22°; that is why the family stops at convex.
- `plan_half_extents_mm` is exact at any bearing: a superellipse is the unit
  ball of a weighted `p`-norm, so its support function is the dual `q`-norm
  with `1/p + 1/q = 1`. `p = 2` gives the ellipse formula; `p → ∞` gives the
  rotated rectangle's, which is what a step cut wants. The band edge, the
  run pitch, the pavé packer, the refiner's footprints, the section view and
  the unrolled outline all read it, so nothing measures a diameter any more.
- One plan table, `GemCut::plan_pow()`, is read by both the stock and the
  viewport preview — the exponents used to live privately in `gems.rs`, so
  the drawn stone and the metal cut for it were two different shapes.
- The halo follows suit: its ring is the centre's own outline grown by the
  gap, with accents placed at **equal arc length** round it, so an oval
  centre gets an oval halo instead of a circle drawn round its length.

### Cabochons are flat-backed, and refusing them was a bug

`GemForm::{Faceted, Cabochon}` (their gem-info `ObjectType`, `0 = Gem,
1 = Cabochon`). A cabochon has no pavilion: `pavilion_mm` returns
`BED_CLEARANCE_MM` (0.1) rather than 0.65 of depth. Reading the faceted
figure refused a 6 mm cab on a 1.6 mm band — it demanded 2.42 mm of metal
under a stone that needs none, which is the single easiest stone a cast band
can carry. Its plan is fatter too (`CABOCHON_MAX_ASPECT` 1.25 as a ceiling
reproduces their whole cabochon table, where the faceted marquise and pear
run 1.7 and 1.6), its dome is a medium 0.45 of width, its weight is half an
ellipsoid at 0.0176 ct/mm³, and it sits *on* its bed in the preview instead
of burying a pavilion it does not have.

### The bezel stands on its stone

A bezel's collar has no height of its own. `fit_stone` derives it — the
recess the girdle sits down, plus `bezel_lip` (default 0.3) of the stone's
crown height (`Gem::crown_mm`: the third of the depth a faceted stone
keeps above its girdle, the whole dome for a cabochon) — so a 4 mm round
at a 0.5 mm recess gets a 0.76 mm collar and a 6 mm cab a taller one,
which is what a burnished bezel is: metal pushed a little way up the
crown. The pocket floor is a bearing ledge `bezel_bearing_mm` (0.3) wide
inside the wall, and inside the ledge the floor dishes toward the
pavilion as a paraboloid — steepening toward its ledge, never below 0.1
mm of pad — only when the seat carries a stone, so a stone-less bezel is
the flat cup it was. The plan already followed the girdle: the pocket is
the stone's own superellipse inset by the wall. Measured clean on a 7×5
side face with its stone; the crown warning is unchanged. The section
view now draws the girdle at `stand_off_mm`, where the preview and the
report have always put it — it drew it at the pad's top, which the one
stone record made visible.

### How deep the stone sits is one number

`SeatPadLayer::set_depth_mm` (`Option`, `None` = the style's own) is how far
the girdle sits below the pad's top, and `girdle_drop_mm` / `stand_off_mm`
are what everything reads. It replaces two constants that disagreed:
`gems.rs` sank the drawn stone by 0.22 of its depth (0.35 for a bezel) while
`stones.rs` credited the *whole* pad height as metal under it — so the report
claimed 0.41 mm more room than the preview showed, and a bezel's two figures
were unrelated numbers. The derived defaults are the physical ones: a
bezel's girdle lands on its pocket floor, a drilled pad takes the stone a
whisker in, a cabochon rests flush on its bed.

### `u` is arc at the crest, and metal is not

`u` is arc distance **at the crest radius**, so it is the true metal only on
the crest. Everything else sits inside that radius: measured by
`examples/metric_probe.rs`, the wider side face runs at

| profile | HalfRound 6x3 | LowDome 6x3 | Flat 6x3 | Flat 7x5 | Beveled 6x2 |
| --- | --- | --- | --- | --- | --- |
| `k = r(v)/r_crest` | 0.813 | 0.833 | 0.849 | 0.796 | 0.859 |

so a bridge the chart called 0.55 mm was 0.44 mm of metal — 17–20%
optimistic, in the unsafe direction, on exactly the surfaces the doctrine
sends all ornament to. `FieldContext::arc_scale(v)` is that scalar and
`arc_scale_min(lo, hi)` the conservative read over a span.

What it corrects is **reported numbers and the integer counts generators
solve**, never `h(u, v)`: the chart stays one clean reparameterization of
theta, no saved design changes shape, and every 0.000% guarantee is
bit-identical. Concretely — `SeatRunLayer::bridge_at` (the pitch and the
seat's span both scale by `k`, so the chart figure times `k` is the real one
*exactly*, one multiply); `solve_spacing`, which keeps the invariant that
what it solves is what `bridge_at` then reports; `pave::fill`, where each
row reads its own radius, so different rows legitimately get different
integer counts — a hex pavé on an annulus physically does; and
`pave::halo`, whose accents were placed at `off_u / r_crest` and so landed
at only `k` of the radius they were asked for, squashing the ring along the
ring by a fifth under a comment already promising an even one.

`FeatureFootprint` splits into `feature_u_mm` and `feature_v_mm` for the
same reason, because they are not the same measure: `v` is arc length on the
**reference** section — true as it stands there, and off by the station's
stretch wherever the shank modulates. `min_feature_mm()` is the chart figure
refinement seeds on; `metal_feature_mm(ctx)` is what `dfm.rs` judges against
the sand's detail floor.

The cross-band direction has the same lie at modulated stations, and its own
scalar: the chart's `v` is the section's arc **normalized**, so one chart mm
is `k = surface_len(θ) / band_v_len` of metal — ~1.7 mid-wall on a lofted
signet head, 1.43 on a keyframed lobe at width ×1.5, under 1 in a waist.
`FieldContext::station_stretch(θ)` is that ratio: one table per band
shape, built by `field_context` through a tent-style cache keyed on the
serialized profile and shank (a field added to either can never serve a
stale table), `None` — exactly 1 — on an unmodulated band. It is the number
the decal measurement, the seat report and the serpentarium probes each
derived on their own before it existed. What reads it: `stones.rs` measures
a seat's foot-to-edge clearance in the station's own mm (the section arcs it
compares against always were metal, so the old figure mixed frames — a boss
mid-wall on a lofted head reported a chart `-0.65` for a metal overshoot
`k` times it), and a pad's footprint carries `v_stretch`, so a skirt on a
waist measures finer than the chart says — the direction the chart was
unsafe. Geometry is gated exactly as for `u`: `h(u, v)` never moves on its
own, and every saved design is bit-identical. The one deliberate exception
is **`SeatPadLayer::metal_true`** (serde default off; GUI "True size", MCP
and graph `metal_true`): a flagged pad reads its offsets in metal mm at its
own station and casts as drawn — measured on that keyframed lobe
(k = 1.425), a 3.0 mm drawn reach cast 4.28 mm chart-drawn and 3.0 mm
flagged, with the clearance figure agreeing to 0.02 mm either way. A run
clears the flag at `turned`, its one seat gate: rows station and solve in
the chart, and a flagged prototype must not split the solver from the
metal. `a_pad_on_a_stretched_lobe_is_judged_in_metal_mm` pins all three
reads.

### Stone against stone: the pairwise census

A run's `bridge_mm` only knows its own neighbours, so a pad beside a run,
two runs at different `v`, or a halo's melee against its centre went
unchecked. `stones::report` now walks every station in the design — pads,
runs (graded, at the size each station actually carries) and pavé groups —
and measures each pair in 3D from the girdle frames the modulated bare
profile gives, girdle plan against girdle plan via the same support
function. Analytic, like the rest of the module.

Two numbers per pair, and the second is the finding. **The ring's own
curvature closes the arc under the girdle**: pitch `p` at crest radius `r`
is only `p (r − t) / r` at depth `t`, and a straight-walled pavilion keeps
its full width the whole way down. `examples/crowd_probe.rs` measures it,
and `loss = p · pavilion / r` to within a hundredth:

| cut (2.5 mm on a size-7 low dome) | girdle | culet | loss |
| --- | --- | --- | --- |
| Round brilliant | 2.507 | 2.058 | 0.448 |
| Princess | 2.507 | 1.943 | 0.564 |
| Emerald | 2.589 | 2.040 | 0.549 |
| Trillion | 2.507 | 2.217 | 0.289 |

A 16-stone row of 2.5 mm emeralds clears at 0.640 mm and is at **0.259 mm**
at the culet — under the 0.3 mm sand floor, and nearly twice `MIN_EDGE_MM`
of metal the girdle bridge never sees. This is a **fill** rule, not a draft
one: a thin bridge does not lock the mould, it comes out of the flask as two
stones sharing a hole, so it is said in the `min_section_mm` voice. The
culet column holds each girdle's full width all the way down — the truth for
a step cut, pessimistic for a brilliant, and step cuts are the population
that gets set tight. `StonesReport::closest` always carries the tightest
pair whether or not it is a finding; `crowding` lists at most 12, because a
240-seat pavé is not a list.

## The configurator is a second frontend, and it is core-only

`crates/ringdesign-configurator` (binary **`build-a-ring`**) is the
customer-facing kiosk: a five-step guided flow (style, size and metal,
stones, detail, review) that writes a finished order folder — design file,
`choices.json`, casting sheet, GLB, hero PNG, turntable GIF — under
`designs/../orders/<customer>/`. Two decisions carry the weight:

- **The whole order is `compose::Config`**, a small serde struct, and
  `compose()` is a pure function from it to a `RingDesign`. A web frontend
  or an order queue can carry the same struct verbatim; nothing about a
  customer's ring lives in UI state.
- **It depends on core only.** The preview is `render.rs` drawn to an egui
  texture (half-res while dragging, supersampled at rest), so the crate has
  no GL plumbing at all. The style step renders its base cards the same
  way, once, on a one-shot thread.

A `prices.json` beside the designs folder (`{"Silver 925": 1.2}` per gram)
puts a metal estimate on the metal step and the review; absent, weights
show alone. Solitaires offer five cuts; the seat stock is the same gypsy
mound either way.

Curation is castability: every base is castable bare, and `reconcile()`
strips what a base cannot carry — stones off signets, patterns and
lettering off domed bands, milgrain off Wave and Twist (**the crest wanders
in `v` there**: the edges slide along the finger, so a fixed-v bead row
lands on the dome flank and leans — measured 3% at 50°), and engraving
displaces the pattern rather than stacking on it. Engraving is a single
`Decal` stamp of a `TextAlpha`, sized to the side face, not a 1-repeat
tiling — one tile spans the whole circumference, so a windowed tiling shows
a stretched fragment. The test runs every base through both dresses
(pattern-heavy and engraved) and holds each to the field verdict.

## The graph is provenance one level above `GroupLayer::recipe`

`crates/ringdesign-graph` is a core-only dataflow runtime: nodes are the
core's own calls, wires carry their values, and evaluating a graph *is*
building the design through the same API every panel and MCP tool uses.
It is what `compose::Config` already is for the configurator — a pure
function to a `RingDesign` — made editable. The rules, fixed before the
first node and recorded in the crate's lib doc:

- **Implicit lists.** A pin fed `N` items runs its node `N` times with
  longest-list matching (shorter lists repeat their last item); an empty
  list in is an empty list out; a failed item is a `Null` with an
  attributed error and its siblings continue; a nested list passes whole.
- **A closed `Value` enum with `Arc` domain handles**, never a JSON tree
  and never reflection. Coercions are one table, pinned by a test.
- **Stable identity, serde truth.** `NodeId` is a `u64` handed out once —
  never a list position. The serde `Graph` is the truth; every editor is a
  view rebuilt from it, and `pos` is the only view data persisted.
- **Native evaluation** with a recipe-signature cache so one edit re-runs
  one chain; scripts run only at expression pins and script nodes.
- **Mode is a property of the graph.** `SandRing` evaluates to the design
  *with* its field verdict, and file-writing sinks refuse `NotCastable`;
  `Free` adds the solid kernel and the mesh verifier.
- **Generators emit live groups and the evaluator never regenerates** —
  the graph is provenance the way a `GenRecipe` is, one level up. A design
  carries its graph (`RingDesign::graph`, live until baked); standalone
  graph, cluster and preset files carry their own version ladder.
- **Caps on everything a literal can size**: `MAX_LIST_ITEMS`, `MAX_NODES`,
  `MAX_CLUSTER_DEPTH` — the 67 GB lesson, applied before the first list.

The crate forwards `parallel` to the core and passes the wasm check; the
editor (`ringdesign-graph-ui`), scripting (`ringdesign-script`), the Python
module and the solid kernel land as their milestones do (`docs/ROADMAP.md`).

What the runtime settled while being built, each pinned by a test:

- **A struct is a node as a patch over its base** (`nodes/structs.rs`,
  `StructNode`): unset pins leave the base's fields alone, so a modifier
  after a node of the same kind is safe; enum pins carry serde names;
  `coverage()` holds every node to its struct's serialized keys, which is
  how a field added to the core cannot go unnoticed (it caught `theta_deg`
  declared on the wrong struct the first day). `prepare` hooks run before
  the patch (`apply_style` cannot clobber an explicit crown), `finish`
  hooks after (`fit_length_to`).
- **Generators emit live groups**: `gen.pave/halo/channel` call `fill`,
  `halo` and `channel_set`, which stamp the recipe; the evaluator never
  regenerates. A drawn plan through `outline.custom` gets the lobed-plan
  dome suggestion because `CustomOutline::from_points` now sizes `fair_r`
  from the hull defect on every import path, not only in the exporter.
- **SandRing file sinks are judged first** (`nodes/sink.rs`): `sink.export`
  and `sink.save_design` take the field report (wired, or computed from the
  wired design); an unjudged ring is refused as unjudged, a `NotCastable`
  ring with the notes, and nothing is written. Side-effect nodes run only
  under `Targets::Everything` and are never cached. `sink.mesh_verdict`
  (face normals) is Free-only — the field is the verdict in SandRing.
- **A cluster carries its graph in its params** (`nodes/cluster.rs`), so a
  design embedding its graph stays whole off-machine; pins come from the
  cluster's exposed inputs (typed from the inner spec) and outputs; the
  node's values are *injected* onto the inner pins — handles included, not
  just literals — one depth down under the outer mode, so a Free-only node
  is refused through a cluster in a SandRing graph. Any inner failure fails
  the item: a `Null` must not become a default downstream.
- **`RingDesign::graph`** is the design's provenance (no ladder bump — an
  absent key reads `None`, an older build ignores it). Graph, cluster and
  preset files have their own ladder in `file.rs`, one step per version.
- **The nine templates are committed graphs** (`graphs/templates/*.graph.json`,
  bundled by `include_str!`) generated from builders in `templates.rs`;
  the golden test holds each file to its builder and each evaluation to the
  code template **byte for byte**. Regenerate with
  `RD_WRITE_TEMPLATE_GRAPHS=1 cargo test -p ringdesign-graph write_template_graphs`.
- **Scripts compute and nothing else** (`crates/ringdesign-script`): one
  sandboxed rhai engine per process — operation, call-depth, expression-
  depth, array, map and string caps, `eval` disabled, no module resolver —
  and `loop {}` halts with "Too many operations" (pinned). The graph crate
  defines the `ExprEvaluator` hook and `Literal::Expr` (`{"expr": …}` in a
  file, the one tagged literal shape); the script crate implements it.
  An expression pin runs per item with the node's other inputs in scope
  plus `i`/`n`; a `script` node declares its pins in header comments
  (`// in: a: Number = 1.0, b: List<Number>` / `// out: h: Number`), reads
  inputs as variables and leaves outputs as variables, and header errors
  name their line. A literal left on a pin the header no longer names
  stays a pin, so the graph still validates and the node reports the
  header itself. `ringdesign_script::registry()` is the builtin library
  plus the script node; the GUI and its worker use it.
- **MCP edits the same graph the GUI shows** (`ringdesign-mcp/src/graph_tools.rs`):
  the `graph_*` tools read `design.graph` off the shared engine, edit it
  and store it back through `set_design`, whose generation bump is what a
  GUI sharing the engine polls — there is no second graph host.
  `graph_evaluate` runs one evaluator with the script engine attached and
  makes the engine's design what the graph produced (graph kept);
  `ring://graph` and `ring://graph/nodes` are the resources. The two tool
  routers combine with `+` in the handler; the Court band built by tools
  alone evaluates to the code template byte for byte (pinned).
- **The first hosted cluster is the signet** (`graphs/clusters/signet.cluster.json`,
  built by `templates::build_signet_cluster` from our own nodes — one
  exposed Width fans out to the section, the face fit and the shank;
  Rise, Shoulder, Rim, Loft, Cap, Taper, Outline, Name, Size are the
  panel; design, head, shank and profile come out). Bundled presets
  (`graphs/presets/*.preset.json`) set it, and the "Heart signet" preset
  evaluates to the code template **byte for byte** (pinned). The file
  layer lists user-dir clusters and presets first and the bundled ones
  behind them, so a user file of the same name shadows a bundled one.
- **The lift is exact by construction** (`lift.rs`, `Graph::from_design`):
  it wires the nodes a person would, evaluates them, diffs the result
  against the design field by field, and carries whatever the nodes cannot
  express (a flange, a tiling warp, the draft settings) as `design.set`
  patches — so "Convert to graph" never loses a field, and the test that
  every template lifts back byte-for-byte also caps the patches at four.
- **The list idioms follow Grasshopper where it was measured**
  (`batch6-8/from-rhino/brief02-lists/`, Rhino 8.34): longest-list
  matching repeats the last item; a negative Series count generates
  nothing and warns rather than failing; Partition's size list cycles, a
  zero size is an empty chunk, and the last chunk keeps what is left.
  Polar Array only pinned that item 0 is the original and the step is
  `angle / count` for a sweep past a full turn; the partial-arc law, and
  Weave, Cull, List Item, Shift, Reverse and Sort, came back with panels
  reading the inputs, so those keep their documented semantics (the
  re-run list is in `SharedVM/answers/followup-brief02.txt`).

## Free mode: `crates/ringdesign-solid` and the Manifold kernel

Mesh CSG for everything the height field cannot be — settings, vines,
anything off the band — on Manifold (`manifold3d` 0.4), **behind a
feature**: `ringdesign-solid/manifold` (and the graph crate's
`kernel-manifold`, which enables it) pulls the C++ through cmake on first
build (~30 s here, a git clone of Manifold's source); off, the crate is
`kernel_available() == false` and the default workspace build has no C++.
`kernel.rs` is the sibling `mandrel` crate's construction set ported —
cylinder/cone/sphere/cube, the marquise lens, rails, `union_all`,
`Frame` (`on_ring` puts one on the band at an angle, `z` out of the
metal), `segment`/`tube`, `Parts { add, cut }` with the marquise and
round settings and the leaf — plus `to_mesh` (a Manifold as the core's
`Mesh` with area-weighted normals) and `from_mesh` (a watertight core
mesh as a Manifold, refused if it is not one). Measured and pinned: two
cylinders union watertight and round-trip; a tube ring reads as
**vertical walls, zero undercut** under `castability::analyze`, which is
the Free-mode verdict — faces, not the field, because a solid has no
chart. Everything here is lost-wax territory unless the field says
otherwise; the SandRing verdict never reads a solid.

The nodes (`ringdesign-graph/src/nodes/solid.rs`, behind
`kernel-manifold`, all Free-mode only so a SandRing graph refuses them
at validation): `solid.cylinder/sphere/box/extrude/revolve`,
`solid.from_design` (the watertight sweep taken into the kernel),
`solid.union/difference/intersect/union_all`, `translate/rotate/scale`,
`frame.on_ring` (a frame on the band at an angle, `z` out of the metal,
tilt and roll) with `solid.place`, `solid.tube`, `solid.setting`
(marquise or round), `solid.leaf`, `solid.import` (the solid crate's OBJ
and STL readers, welded; refused unless watertight) and `solid.mesh`,
which is how a solid reaches `sink.mesh_verdict`, `sink.export` and
`sink.render`. Pinned: a semi-mount — the Court band as a solid, a round
setting placed by `frame.on_ring`, unioned — exports as STL and imports
back within 0.1% of its volume; a revolved section matches π·(R²−r²)·h.
The editor's palette lists the graph's own mode. The GUI and the CLI
carry a `kernel` feature forwarding to it
(`cargo run -p ringdesign-gui --features kernel`).

**The mandrel merge.** The sibling `../mandrel` crate's vine semi-mount —
a round band wire, a bypass vine over the top, a marquise centre with
flanking marquises on stems, round buds on tendrils, four leaves,
thirteen stones — is `kernel::vine_ring(&VineOptions)` (its `catmull_rom`
alongside), ported with its own frame construction kept verbatim so the
geometry is identical. `VineOptions` keeps the two knobs the construction
reads (`inner_dia_mm` 16.5, `vine_radius_mm` 0.9); mandrel's amplitude
and lobe fields were never read by its build and were not carried. The
node is `solid.vine_semimount` (solid, stone count, carats, stone list),
and the bundled **Vine semi-mount** cluster
(`graphs/clusters/vine-semi-mount.cluster.json`, listed only under the
kernel feature so a build without it never offers a cluster it cannot
run) wires it through `solid.mesh` and `sink.mesh_verdict` with the
inner diameter and vine radius exposed. Lost wax, not sand: the verdict
is the mesh verifier's, reported as what it is. Carats were reconciled
the other way — core `gem.rs` now carries mandrel's pear (0.00527) and
marquise (0.00565) factors, which were calibrated against an external
stone report, where the old figures were the textbook's; the rest
already agreed. Mandrel's own MCP (`generate`, `get_options`,
`set_options`, `export_stl`) is retired: the same work is
`graph_add_node` on the cluster, `graph_set_input` on its exposed pins,
`graph_evaluate`, and a `sink.export` in Free mode.

## Python: `crates/ringdesign-py`

The core and the graph runtime as a Python module (`import ringdesign`),
built by maturin into `tools/venv` (`VIRTUAL_ENV=$PWD/tools/venv
tools/venv/bin/maturin develop --release -m crates/ringdesign-py/Cargo.toml`),
abi3 for Python 3.12+ so the workspace build needs no Python headers.
Numpy-free on purpose: geometry crosses as tuples, reports as dicts
(serde → Python). `Design` (templates, files, JSON pointers, builds with
the GIL released, the field verdict, sections, stones, the modulus scan,
renders), `Build` (geometry, report, weights, pattern shrink, the five
exports), `Graph` (load/template/lift, exposed parameters by name, edits,
evaluate with the script engine attached), `Library`, `node_specs()`.
`crates/ringdesign-py/tests/test_smoke.py` holds every template's lift to
the design through Python and a from-scratch Court band to the template;
`tools/harvest/py_build.py` feeds the mesh probes from the module.

## GUI

`app.rs` owns all state. Geometry-affecting edits call `app.mark_dirty()`; a
90 ms debounce then dispatches a build to a worker thread which drops stale
jobs. Panels never build synchronously — only export does, at
`export_params` resolution, which is separate from the interactive
`preview_params`.

Panels are `pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui)` and are
wired in `panels/mod.rs`.

The left column is **two panes**, not one scroll area: design on top, layers in a
nested bottom panel. They shared a scroll once and the design column grew long
enough to push `Add layer` off the bottom of the window, which reads as the
feature not existing. Anything the user has to reach for belongs in its own pane
or above the fold. For the same reason only Ring and Profile default open.

File dialogs start in `library::default_design_dir()` and its `exports` sibling,
created on demand, so everything the app writes lands in one predictable tree.
`Workspace.recent` keeps the last ten opened/saved design paths (File >
Recent, missing files disabled rather than hidden); File > New from template
lists `templates::all()` with blurbs as hover text.

### The graph pane, the node tool, and the bridge

A design with `graph` set is **driven**: `app.sync_graph` rebuilds the
`ringdesign_graph_ui::Editor` whenever `design.graph` differs from what the
editor last saw (history, MCP, a file, Convert, Bake), and
`app.graph_changed` writes the editor's graph back and marks dirty. The
build worker evaluates the graph before it builds (a persistent
`Evaluator`; the library's `Arc` identity is its epoch), builds the
evaluated design, reuses the evaluation's field report, and returns
per-node values and notes; `tick` splices the evaluated design in *under
whatever the graph has become since*, so an edit made during a build is
never clobbered. `mark_dirty` skips `regenerate_live` on a driven design —
its generators are nodes on the worker.

`PaneKind::Graph` is the editor (arrange, bake, the empty state offering
Convert / the simple graph / the template graphs); `ToolKind::Node` is
the inspector (pins with widgets or the wire feeding them, expose and
withdraw, output values, params, diagnostics). While driven, the Design,
Layers and Tiles tools show a banner and their bodies are disabled —
the graph is where edits go — and the Layers banner lists the stack's
entries as **Edit in graph** buttons: `Graph::entry_nodes` (the `entry`
nodes in evaluation order, the lift's stack order) finds the producing
node, the editor centres on it through snarl's `current_transform`, and
the Node tool opens. Bake drops the graph and the design stays exactly as
last evaluated. The Delete key acts on the active pane: a selected node
in a Graph pane, otherwise the selected layer. **Collapse** folds the
view's selection (or a node with its upstream, from the node menu) into
one cluster node: `Graph::collapse` keeps the folded nodes' ids inside,
turns every boundary wire into an exposed input or output and rewires
through the new node, and carries the parent's own exposures on folded
nodes onto it — the graph evaluates the same before and after (pinned).
Fit and the minimap read the view transform snarl hands to
`current_transform` each frame. History names a graph
change as a document — converted, edited, baked — which is why
`first_difference` treats a key present on one side only as a change.

### The editor looks like the comfyui-android graph

`ringdesign_graph_ui::style` is that app's graph view, number for number,
and both the desktop pane and the phone tab draw through it: an AMOLED
canvas (`CANVAS`) lit by three pools of colour — violet, aqua, pink —
anchored to the visible rect so the light does not slide as you pan, a
dot grid in graph units (spacing 28, coarsened by powers of two when
zoomed out, dim teal), glass nodes (`NODE_FILL` at alpha 190, corner 8)
under a white hairline rim that turns hot pink when chosen and the error
colour when the node carries diagnostics, axis-aligned wires with 8 px
corners at 2.6 px, 15 px pins standing 3 px outside the body, inputs
stacked above outputs (`NodeLayout::sandwich`). Pins keep their kind's
hue at the palette's muted saturation and value; labels are plain ink.
`apply_visuals` installs that app's egui visuals — black page, glass
panes, aqua hover, pink press and selection — inside the editor's `scope`
only, so the app around it keeps its own theme. `NODE_FIELD_W` caps a
field row at 260 graph units because a width taken from
`available_width` feeds back into the node size it is derived from and
ratchets the node wider every frame. The frosted-glass blur behind each
node is the one thing not carried: it is the phone's `backdrop-blur`
grab pass, which the desktop's glow window has no equivalent for; the
translucent fill over the lit canvas is what remains of it. Header-only
dragging is that app's too: `Editor::drag_gate` classifies a press by
where it lands on the drawn node frames (`classify_point` — any header
wins over any body, so a title-bar grab always moves a node where nodes
overlap), a body drag pans the view through `current_transform` while
the node's own move is undone from a position snapshot after the frame,
and a locked editor (`editable = false`) pans on any node drag and vetoes
every move — so a finger that misses a pin scrolls the view instead of
dragging the node around.
Arrange lays nodes out by depth from their **measured** sizes
(`final_node_rect`, keyed by graph id so a rebuilt snarl keeps them) with
that app's gaps, and re-runs while the measures it used disagree with the
current ones, three passes at most — snarl's first frame measures a node
before its widgets settle (the Court band's profile node read 125 wide on
frame one and 253 on frame two), and a layout from those numbers overlaps.
`RD_GRAPH_SHOT=/dir cargo test -p ringdesign-graph-ui --features shot shot_the_editor`
renders the editor through wgpu offscreen to `graph.png` for an eyeball
pass; the `shot` feature keeps wgpu out of every other build, because it
unifies `egui/bytemuck` into the whole UI stack and costs a full rebuild.

### Paint-on-band: pressure is millimetres, the ceiling is the draft

`paint.rs` (core) is the brush both apps share: `bite()` resolves a press
into millimetres of metal with the ceiling read from the *local base draft*
— a squared side face takes the full measured 1.6 mm, a half-round's crest
is honest only to 0.05 mm, smoothstepped between so there is no cliff for
the pen to fall off. `ensure_band_layer` is the one file convention: strokes
live in `design.drawn` under the name "band" (2048x320, seam-wrapped) and
show through an ordinary one-cell `TilingLayer` at the 1.6 mm maximum — so a
band painted on the phone opens on the desktop as the same layers, and vice
versa. The desktop's unrolled pane has a Paint mode (floating brush bar:
size, depth with a live mm readout, soft, erase, last-stroke undo; the
cursor shows the local ceiling and warns when the ask exceeds it); the
Android app is the pen-first version with real pressure and palm rejection.
Bakes happen on stroke end, not per sample — `Arc::make_mut` deep-copies the
library — and the unrolled field cache hashes the stroke tally, because a
re-baked drawing keeps its name and size.

### The unrolled editor grips every layer

Tiling keeps its lattice drag, scroll-for-repeats and band-edge handles; every
other placeable layer now has a handle of its own — border, milgrain and seat
runs as dashed v-lines, seat pads, decal stamps and signet-layer plates as
centre crosses, a pad's rim as a draggable radius ring. Grabbing a handle
selects its layer, v-drags snap to the side-face boundaries, and the shade
modes include a wall-thickness heatmap (red under `min_section_mm`, baked as a
second vertex colour so mode switches never re-upload).

### Tool panels are `egui_tiles` trees

`dock.rs` holds one `egui_tiles::Tree<ToolKind>` per side, shown inside an egui
side panel so the centre stays free for the viewport panes. Tools split, stack
and tab within a side by drag; moving one across sides is a button, because a
pane cannot be dragged between two separate trees.

The behaviour needs `&mut RingDesignerApp` to draw a tool while the tree lives
in the app, so `dock_side` swaps the tree out with `Tree::empty` for the
duration of `tree.ui` and puts it back. Layout persists via eframe storage under
`DOCK_STORAGE_KEY`.

### The viewport needs a depth buffer asked for explicitly

`eframe::NativeOptions::depth_buffer` defaults to **0**, and eframe passes that
straight to glutin's `with_depth_size`. The window then has no depth attachment,
`glEnable(GL_DEPTH_TEST)` and `glClear(GL_DEPTH_BUFFER_BIT)` both silently do
nothing, and the ring renders see-through — the far wall painting over the near
one. `main.rs` sets `depth_buffer: 24`, and `GpuMeshRenderer` queries
`FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE` once and logs a warning at 0 bits, so this
can never fail quietly again.

## The phone has the graph too

`crates/ringdesigner-android` (moved in from the EguiMobile workspace on
2026-09-04, history included) carries a Graph tab on the same three crates
the desktop uses — `ringdesign-graph`, `ringdesign-graph-ui`,
`ringdesign-script` — through this root's `egui-snarl` patch (still
byte-identical to EguiMobile's copy, which the wirelab plugin uses). Its
`graph.rs` is the whole
bridge: `GraphState` keeps an `Editor` in step with `design.graph` (the
same sync rule as `sync_graph` — whichever side moved, a pulled design
included), writes the editor back, converts, opens templates, bakes and
locks; `GraphRunner` is the worker's evaluator, built once, and the build
worker evaluates a design's graph before every build exactly as the
desktop worker does, handing back values, notes and the field verdict the
evaluation already paid for. The editing tabs show a driven banner with
Bake while a graph is in charge, for the same reason the desktop panels
do. There is no gesture layer: snarl pans and zooms through egui's
`register_pan_and_zoom` (one-finger drag, pinch), and egui reports a long
touch as `secondary_clicked`, which is what opens snarl's node and
background menus — so the touch work the plan budgeted for came down to a
Lock toggle (`Editor::editable`). Verify with `cargo test -p
ringdesigner_android` on the host and `cargo ndk -t arm64-v8a check -p
ringdesigner_android` from the crate dir with `ANDROID_NDK_HOME` set.

The crate builds on `egui-mobile` from <https://github.com/shadowbrok3r/ios-egui>
(checked out at `~/Documents/Rust/Mobile/EguiMobile`) as a **git dependency**,
pinned in `Cargo.lock`: `cargo update -p egui-mobile` moves the pin, and the
root `[patch.crates-io]` carries that repo's `android-activity` fork, which a
git dependency does not inherit. `crates/ringdesigner-android/java` is a copy of
the framework's Java bridge that `cargo egui-mobile build`/`run` re-syncs from
the resolved egui-android — edit it in EguiMobile, never here. The wrapper is
installed from that checkout (`cargo install --path crates/cargo-egui-mobile`;
its `ANDROID_SETUP.md` covers the SDK/NDK/JDK). Build from the crate dir:
`cargo egui-mobile build -a --release` lands
`target/release/apk/ringdesigner_android.apk` at this root. To ship: bump
`version` in the crate's `Cargo.toml` first — the phone only installs a strictly
greater versionCode, `(1<<24)|(major<<16)|(minor<<8)|patch` — add the
`CHANGELOG.md` entry, then `AS_URL=… AS_KEY=…
crates/ringdesigner-android/scripts/publish-appstore.sh --no-changelog
ringdesigner-android crates/ringdesigner-android "what changed"`. The signing
key is `~/.android/debug.keystore`, shared across machines; a different key
makes every update fail `INSTALL_FAILED_UPDATE_INCOMPATIBLE`. The `local-npu`
feature compiles, but the manifest declares no `runtime_libs`, so an APK built
with it ships without the QNN `.so` files the feature dlopens.

The phone's exports (`export.rs`) run one thread per job — STL, 3MF, GLB,
sheet, render, turntable — and open the share sheet from `poll_exports`
when the file lands, so an export is never coalesced away behind a
preview build, the guarantee the desktop's `export.rs` also makes. "Save a
copy to Downloads" is the durable mirror: `HostExt::save_to_gallery`
inserts a non-media file into MediaStore Downloads, which survives an
uninstall where app storage does not. Opening a `.ring.json` from another
app is still not possible — the framework has no incoming-intent plumbing,
and an intent filter the app then ignores would be worse than none.

## The `parallel` feature and the wasm door

`ringdesign-core` puts rayon behind a default-on **`parallel`** feature;
off, every fan-out runs serial through the same call sites (per-site `cfg`
splits, no trait shims). `BuildClock` guards the one other wasm landmine —
`Instant::now` panics in a browser. Both core and the configurator pass
`cargo check --no-default-features --target wasm32-unknown-unknown`.

**The configurator runs in the browser.** `trunk serve` in
`crates/ringdesign-configurator` (its `index.html` carries
`data-cargo-no-default-features`, so core runs serial; `Trunk.toml` builds
release on port 8787) starts `build-a-ring` through an eframe `WebRunner`
on the `#build_a_ring` canvas, glow on WebGL2. Three things had to change,
none of them in `compose`:

- **There is no thread in a browser**, so the job body is a `Worker` whose
  `run` is the same function on both targets: natively `Engine::new` wraps
  it in the `compose-build` thread, on wasm `Engine::pump` runs the pending
  jobs on the UI thread from `poll`, with the same coalescing (the newest
  job wins, a design change is never dropped for a camera frame). A
  preview build is ~40 ms serial, which the frame absorbs. The style
  cards (`Thumbs`) render one base per frame and ask for another frame
  while any remain, instead of a one-shot thread.
- **An order is bytes before it is files.** `order_files` builds the six
  documents in memory — `library::design_json`, `gltf::to_glb`,
  `render::png_bytes`, `render::turntable_gif_bytes` — and
  `deliver_order` writes the folder natively or hands the browser one
  `order-<slug>.zip` through `threemf::zip_store` (the 3MF writer's
  store-only zip, now public with `threemf::Entry`) and a Blob download
  link (`web.rs`). The file writers in core are the byte writers plus a
  `std::fs::write`, pinned equal by a test.
- The workspace egui's `rayon` feature stays on: rayon-core 1.13 falls
  back to the calling thread where it cannot spawn, so the browser build
  needs no feature surgery. `prices.json` is simply absent on the web, and
  the metal step says so.

`.claude/launch.json` (ignored) carries a `build-a-ring-web` entry for the
in-app browser preview.

## Running the tests

Always run under a memory guard:

```
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --offline
```

This machine has no swap, so an unbounded allocation does not thrash — the
kernel OOM-killer takes down whatever it likes, including the user's other
applications. `TilingLayer::cells()` once allocated 67 GB from a `u32` cell
count and killed the desktop.

Use the cgroup guard above, not `ulimit -v`. `ulimit -v` caps *virtual* address
space, and the threaded test harness reserves far more virtual than resident —
a 4 GB `-v` cap fails ~10 unrelated tests that pass individually, which reads as
a regression that is not there. `MemoryMax` caps actual RSS, which is what
matters.

Anything sized by a `u32`/`usize` struct field rather than a constant needs an
explicit cap. `MAX_CELLS` is the pattern: clamp the loop bounds *and* break on
the accumulated length, and keep `cell_size` on the unclamped counts so
footprints stay aligned with `height`.

## Conventions

- Ring angle: **90° is the top of the ring** (`profile::TOP_DEG`), matching the
  sibling `mandrel` crate.
- Units are mm and f64 throughout. Guard every division.
- Icons: `use egui_phosphor::regular as icon;`. The font is loaded in
  `theme.rs`. Do not use raw unicode arrows or geometric shapes — they render as
  tofu.
- The network works. `--offline` is fine for a quick rebuild, but do not treat it
  as a constraint: `cargo add` and `cargo search` reach crates.io. Assuming
  otherwise once cost a real feature — `egui_tiles` was declared impossible
  when it was one `cargo add` away.

## Related projects

- `../mandrel` — CSG semi-mount builder (manifold3d). Different approach for a
  different process (lost wax): booleans of settings and vines. Shares the frame
  and carat conventions.
- `../jewelry_cost_calculator` — the egui_glow viewport in `src/ui/gpu_mesh.rs`
  is the reference this app's renderer was ported from. Also has ring sizing and
  STL/OBJ loading.

- `tools/harvest/` (gitignored) — the CrossGems decode tools, their reports and the
  mesh-comparison probes (`deviation.py`, `dihedral.py`, `measure_cg.py`); venv at
  `tools/venv` (`requirements.txt` beside the scripts). The archive itself — decompiled
  sources, the 122 decoded clusters, resources — stays in `../PostLoad/PostLoad`.