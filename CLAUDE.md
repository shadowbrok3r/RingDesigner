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

`HEAD_FACE_DRAFT` uses it to draft the head's flanks by 9%, so the table is a
slightly smaller copy of the outline that carries it — which is what the
reference does (16.0 mm body, 14.7 mm table) and what a two-part mould wants of
the one surface it has to slide off. Proportional, not a distance: insetting by
a distance drafts a narrow station to nothing and leaves a fin standing off the
end of the head, which a heart does first.

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
every pattern from functions periodic in both axes, and `Alpha::make_seamless`
cross-fades imported images.

### Stability under shank modulation

The shank taper changes the cross-section per angle, so `ProfileLoop::v_mm`
differs per angle too. Layers are evaluated against the **reference** profile:
`v_norm = p.v_mm / loop_i.surface_len_mm` then `v = v_norm * ctx.band_v_len_mm`.
The pattern therefore follows the band as it tapers instead of sliding across
it. `mesh::build` and `castability::section_at` must do this identically — if
they diverge, the section view lies about the solid.

## Manufacturing analysis speaks in the sand's numbers

`DraftSettings` carries the sand itself: `min_draft_deg`, `min_section_mm`
(the fill floor the field verdict checks), and `min_detail_mm` (the feature
floor), with `SandProcess::{DelftClay, Petrobond}` presets writing all three.
On top of that:

- **Per-layer DFM** (`dfm.rs`): layers are analytic, so their finest feature
  is a parameter, not a measurement — `feature_footprints` against
  `min_detail_mm`, surfaced as warning badges on the layer rows, in the
  report notes, and on MCP as `castability.dfm`.
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
- **The size-run CLI** (`src/bin/ringdesign.rs`): `ringdesign export
  ring.json --sizes 5:9:0.5 --formats stl,3mf --shrink sterling` builds each
  size, field-checks it, writes the files and a manifest CSV (verdict,
  thinnest wall, volume per size) — one flask, one command; the manifest is
  also the export-regression diff. `ringdesign check` prints the field
  verdict and the stones findings for one design.

### Pavé is a generator, split is a modulation, the sheet is HTML

- **Auto-pavé** (`pave.rs`): packs an arc × v-band (or a side-face run) with
  gypsy seats — hex-staggered rows, full-ring rows wrap-exact with integer
  counts, capped at 240 seats *with the refusal said out loud*. The output is
  an ordinary Group of `SeatPadLayer`s: every seat stays editable, and the
  stones report rolls a uniform seat group up to one line instead of two
  hundred rows. Gypsy mounds because that is the measured-safe row on curved
  ground.
- **`ShankKind::Split`**: the castable read of a split shank. A real split —
  two crests — is a valley no single parting plane clears; instead the band
  flares 55% over a 110° arc while a channel is carved into *each side face*
  (`ShankMod::side_groove_mm`, capped at 0.35 of the half-width). The
  groove's floor faces along the pull and its walls stand radial, so the
  whole ring fields 0.000% — side-face doctrine, applied to the shank
  itself. Seen side-on, the ring reads as two diverging rails.
- **The casting sheet** (`spec.rs`): one self-contained printable HTML page —
  dimensions, weight in every alloy with its pattern scale, the field
  verdict with notes and DFM findings, the stones table with bench warnings,
  provenance. Desktop File menu and the Android share sheet both emit it.
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
meander), so `repeats_for_square_cells` lands each *period* at a fraction
of the cell — sub-detail-floor mush on a 2 mm side face. Templates carry
hand-tuned counts with the per-tile period in a comment, and fine-lined
alphas (Greek Key at 0.15 mm strokes) are simply not usable on narrow
faces in sand.

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
  `MIN_EDGE_MM`, metal available for the pavilion along the seat's normal
  (to the bore wall on the crown, across the band on a side face) vs
  `gem.pavilion_mm()` + `MIN_WALL_MM`, run bridges vs `MIN_EDGE_MM`, and
  carat totals. Analytic — it reads the layers and the modulated bare
  profile, never the mesh, so it costs nothing and cannot disagree with the
  design.
- `gems.rs` (GUI) — render-only faceted previews: one superellipse-plan
  brilliant per stone-bearing station, positioned by evaluating the
  *displaced* section under the seat, girdle settled into the pad so the
  pavilion vanishes into metal and the crown stands proud. Flat facet
  normals under the viewport key light do the sparkle; drawn as a second
  buffer in the same GL program, toggled by the toolbar's Stones checkbox.
  **Never in the `Mesh`, never exported** — `RD_GEM_SHEET=/dir` on the
  `stones_land_on_their_seats` test writes a software-rasterized sheet for
  eyeballing placement.

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

## The `parallel` feature and the wasm door

`ringdesign-core` puts rayon behind a default-on **`parallel`** feature;
off, every fan-out runs serial through the same call sites (per-site `cfg`
splits, no trait shims). `BuildClock` guards the one other wasm landmine —
`Instant::now` panics in a browser. Both core and the configurator pass
`cargo check --no-default-features --target wasm32-unknown-unknown`; what a
live web build still needs is the configurator's `WebRunner` entry + trunk
packaging and its `std::thread` workers made synchronous or moved to a web
worker (a serial preview build is ~40 ms, so synchronous is viable).

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
