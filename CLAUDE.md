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
signet shank every swept build reports 0.000%, while refined builds report
0.03-0.08% and up to -2.9°. Under the 1% that reads as "will not release", but
enough to move the verdict. **Judge castability from a swept build.**

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

### A signet is a shank, not a bump

The thing that makes a signet read as a signet is the **band's own width**: a
narrow shank swelling into a broad head, whose silhouette *is* the head outline.
`ShankKind::Signet` does that — `width_mm` is the head, and the taper makes the
rest. The width follows a superellipse in plan, `1 - x^a` over `head_span_deg`,
where `a` is the same fullness exponent the table outlines use: 2 oval, 4
cushion, 8 rectangle. Match `head_shape_a` to the table's outline and the two
read as one shape.

Measured: **the taper itself is 0.000% undercut on every profile**, down to a
1.9 mm shank on a 12 mm head. A band that widens toward the top is single-valued
in Z over `(r, theta)`, so it is a terrain and releases by construction. What
undercuts is the table, not the swell.

`crown_scale` on `ShankMod` lets the narrowing section round off toward a wire
while the head keeps a flat crest, which is the classic combination — flat table,
round shank. The crown clamp caps it, so a large value only ever means "more
domed here".

### The signet table

The table is a **plane**, solved per point (`plane / cos` of the angle off
centre), because a constant offset along a curved band's normal stays curved.
It is a vertical wall with respect to a ±Z pull, which is fine — it is blank and
hand-engraved. Design goes on the sides.

`SignetLayer::room_across` measures the surface a table can stand on: the run
around the crest whose base draft stays under `TABLE_MAX_DRAFT_DEG`. Past that
the base has fallen so far below the table plane that the shoulder has to claw
the difference back over its own width, which is a wall rather than a fairing.
That is the whole reason **a flat crest is the right base for a signet and a
half-round the wrong one** — measured at 0.000% versus 1.04%.

Table and shoulder together have to fit the room. `fitted_to` takes the shoulder
out first so it can never produce a table its own `overhangs` then complains
about, and `fill_head` grows the table to `SIGNET_TABLE_FRAC` of the room —
measured clean at 0.70, bowing at 0.82, walled up at −36° by 0.92.

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
