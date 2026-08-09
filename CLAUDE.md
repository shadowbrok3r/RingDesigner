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
band surface.** Tiled alphas, borders, milgrain, and raised gem-seat pads are
all layers in that field. There is no CSG anywhere.

- `u` — arc distance around the ring at the crest radius. **Wraps** at the
  circumference.
- `v` — arc distance across the cross-section, from one bore edge, over the
  outer surface, to the other bore edge.

Because it is one function, tiling, the unrolled layout editor, draft analysis,
and cross-sections are all just different ways of evaluating it. A tile drawn in
the layout editor is exactly where the metal lands.

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

Measured on a size-7 D-shape with a tiled alpha and milgrain, worst facet
deviation via `refine::grid_error_mm`:

| build | triangles | ms | worst error |
| --- | --- | --- | --- |
| swept 384x144 | 110,592 | 13 | 0.107 mm |
| swept 512x192 | 196,608 | 26 | 0.075 mm |
| swept 1536x448 | 1,376,256 | 277 | 0.045 mm |
| refined, 0.08 mm / 20° | 136,668 | 421 | 0.080 mm |
| refined, 0.04 mm / 14° | 212,892 | 328 | 0.040 mm |
| refined, 0.02 mm / 9° | 406,848 | 633 | 0.020 mm |

The win is in how the cost *scales*, not at any one point. Halving a swept
grid's error means halving the step in both directions, so it pays 4x the
triangles every time — which is why 1.4M of them still only reach 0.045 mm.
Refinement pays about 1.6-1.9x per halving, so it goes places the grid cannot
afford at all. At loose tolerances the sweep is faster, being a trivial loop.
**Sweep for the interactive preview, refine for export.**

One caveat with teeth: `castability::analyze` reads face normals, and an
irregular mesh reports small spurious undercuts along the crest line, where the
true surface is tangent to the pull and any facet noise crosses zero. On a
signet every swept build reports 0.000%, while refined builds report 0.10-0.18%
and up to -15°. Under the 1% that reads as "will not release", but enough to
move the verdict — and on a signet it does not fall with the tolerance, because
the table is a *plane* at zero draft rather than a crest line, so a whole band
of the surface has nothing to decide its sign but its own slope error.
**Judge castability from a swept build.**

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
  the ring by 17.64 across: **1.12**. That alone was 30% of the missing area.
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
  heart's notch fairs from 0.276 of the half-width to 0.065 while its lobes, its
  point and the head's plan size stay exactly put.

`BODY_FAIR_R` is 0.55 half-lengths because the residual notch and the bluntness
of the head's end pull against each other:

| ball radius | 0.35 | 0.55 | 0.85 | 1.30 |
| --- | --- | --- | --- | --- |
| notch left of 0.276 | 0.080 | 0.065 | 0.051 | 0.039 |
| body's width at the head's end | 0.83 | 0.93 | 1.29 | 1.52 |

Past about 0.85 the head stops ending and starts being a slab.

**The body must contain the face**, or the flank leans back over the mould half
it sits in — the same undercut as any other, said where it can be proved instead
of sampled off a mesh. Closing is extensive, so it can only add, and it stays so
with the ball truncated at the head's ends because the erosion's own station is
always one of the samples it minimises over. `the_body_contains_the_face_it_carries`
asserts it to within 1e-5, which is Catmull-Rom reconstruction noise where the
two curves touch; `head_at` clamps the crest into the bore regardless.

#### The table is read a little inside the face's end

At the end itself the outline is a *point*, so a table read there runs to nothing
and wedges the section to a fin. `HEAD_TAKEOFF` holds the crest span at the
station 5% inside, which costs the table the last twentieth of its length — spent
on an edge break it wanted anyway. The **body** is read right out to the point;
it is the swell under it that keeps the band wide there, not the outline.

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
  build reports a phantom bounded only by its own slope error. Judge a signet
  from a swept build.

A short shoulder makes that phantom worse, which is worth knowing before
shortening one. The shoulder morphs the section faster than anything else in the
model — dead-flat crest to a rounded wire — and if it morphs faster than the
sweep samples, a vertex's `z` shifts between slices and the skewed facet at the
crest crosses zero. Measured on a bare signet band, undercut faces reported:

| shoulder | Draft 192x96 | Preview 384x144 | Fine 512x192 |
| --- | --- | --- | --- |
| 20° | 5 (0.020%) | 3 (0.004%) | 4 (0.003%) |
| 26° | 1 (0.003%) | 1 (0.001%) | 2 (0.001%) |
| 34° | 0 | 0 | 0 |
| 42° | 0 | 0 | 0 |

`HEAD_SHOULDER_DEG` was picked from that table before the reference was measured
properly; at 43° it is comfortably past the knee and now comes from the object
rather than from the mesh. `mesh::tests::scratch_signet_head_undercuts` is the
table.

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

The phantom is worse on an upright outline than a symmetric one, and for a
reason: the section it sweeps is no longer symmetric about its own crest, so the
facets straddling the crest no longer cancel. A shield goes 0.011% at Draft to
0.0013% at Export — converging, but not to zero at any resolution worth paying
for. `mesh::tests::scratch_signet_head_undercuts` therefore asserts that what is
reported stays tiny **and stays on the crest line**, which is what tells a
phantom from a real undercut. That check is what caught the -19.4°.

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
