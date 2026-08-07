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

### The signet table

The table is a **plane**, solved per point (`plane / cos` of the angle off
centre), because a constant offset along a curved band's normal stays curved.
It is a vertical wall with respect to a ±Z pull, which is fine — it is blank and
hand-engraved. Design goes on the sides.

`SignetLayer::room_across` measures the surface a table can stand on, from the
crest out, **excluding the side faces**. A shoulder that rolls off onto the
fillet between crest and side face walls up instead of fairing: sizing a head to
the full band width of a squared-sided profile costs 0.47% undercut at −16°,
where a head fitted to the room costs 0.000%.

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
- `cargo` must be run with `--offline`; the network is sandboxed.

## Related projects

- `../mandrel` — CSG semi-mount builder (manifold3d). Different approach for a
  different process (lost wax): booleans of settings and vines. Shares the frame
  and carat conventions.
- `../jewelry_cost_calculator` — the egui_glow viewport in `src/ui/gpu_mesh.rs`
  is the reference this app's renderer was ported from. Also has ring sizing and
  STL/OBJ loading.
