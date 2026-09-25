# RingDesigner CAD: from a feature list to a viewport you can model in

## Context

Logan asked for the next moves on the CAD section ("very limited"), pointed at OpenCADStudio as
the Rust CAD reference, asked me to drive the real GUI to find the limits, and added mid-session
that there is "just about no way to drive the cad from the 3d viewport, like blender or fusion
360, or rhino" — tools should be mouse / right-click / snap driven.

Decisions Logan made this session:

| Question | Answer |
| --- | --- |
| Kernel | Pure-Rust default; OpenCascade as a desktop child-process worker found at run time, carried inside `package.sh --occt` builds |
| Interaction | Ring-aware hybrid: gumball + osnaps, feature history + sketch-on-face + in-canvas dimensions, G/R/S + axis lock + typed values, right-click menu, Ctrl+K |
| Order | Unblock first, then the viewport tool framework, then builders |
| Phone | Shared core/workbench, desktop UI first, touch a milestone or two behind |

Added 2026-09-22 (Logan):

- **Plain sketches**: a sketch is a feature of its own that extrude/revolve/sweep/loft consume, and
  a sketch can be drawn **on a face**. Schema landed in M1 (`Operation::Sketch`, `Profile`,
  `Workplane::on_face`); the face-pick and in-context canvas are M8.
- **Fusion-style dimension entry**: while creating or editing anything with dimensions (a sketch, an
  extrude, a cylinder…) you can type a number for the live dimension at any time, press **Tab** to
  focus the next dimension field and keep typing, and **Enter** to finalise. egui claims Tab for
  global focus traversal, so the viewport's dimension fields must own it (M4 below).

### What driving the GUI showed (egui inspection port, 2026-09-21)

I could not finish one fused ring.

| # | Finding | Cause |
| --- | --- | --- |
| H1 | "CAD workspace" opened with no CAD pane | `app.rs:758` restores a stored layout without checking it holds `desktop.pane()` |
| H2 | Primitives land at the origin inside the finger hole, Z only; on-ring placement is a checkbox and two numbers three menus deep | `panels/cad.rs:1354`, `cad.rs:652` |
| H3 | One Escape discarded the whole uncommitted candidate, not undoable; Enter applies globally and also closes a polyline | `panels/cad.rs:115,401,1723`; pinned by `interaction_tests.rs:55` |
| H4 | Band ∪ cylinder: 8+ min at 100% CPU, never finished; Cancel cannot stop the worker; pane blocked; app killed | `cad.rs:305` feeds the band to the kernel as `faceted_solid`; the GUI inserts `Operation::Band` itself (`panels/cad.rs:222`) |
| H5 | Torus shank ∪ cylinder refused: `NoClosedForm`. No head fuses to any shank; the five shipped examples never union | cadkernel `classify.rs:94`, `intersect.rs:83` |
| H6 | Right-click does nothing in either viewport; no tool keys; only one-edge pick, body pick, a few drag grips | `viewport.rs:1176`, `panels/cad.rs:703-775` |
| H7 | Sketch editor is a detached 2D canvas: no ring underlay, no pan, no delete; the starter rectangle cannot be removed, so a drawn loop fails with "unsupported or degenerate geometry" | `panels/cad.rs:1562` |
| H8 | Fillet/shell/chamfer take opaque positional indices with no highlight; numeric fields carry no accessible labels | `cad.rs:292`, `panels/cad.rs:1185` |
| H9 | Full linear re-evaluation per change, one global error string, no feature delete/reorder | `cad.rs:597` |

Structural causes (from the planning session's explorer, research and critic reports, claims
spot-verified against the code):

- `design.cad.is_some()` is an exclusive mode (`mesh.rs:349`; 26 gates in 19 files). A second
  lockout survives it: every CAD Apply calls `open_graph` (`panels/cad.rs:417`) and all surface
  tools refuse on `d.graph.is_some()`.
- Three solid stacks that do not talk: cadkernel B-rep, Manifold (ships dark), and `csg.rs` — our
  exact mesh boolean that already fuses 37 settings into a 1.38M-face band in 4.3 s and has one
  consumer (`setting.rs:932 apply`).
- cadkernel: no torus/NURBS booleans, fillet only on convex planar solids, shell only
  box/cylinder/sphere, no pattern/mirror/offset, no persistent naming; upstream is chasing
  AutoCAD/ACIS parity. No pure-Rust B-rep kernel in Sept 2026 fuses and fillets reliably (truck,
  monstertruck, vcad, brepkit checked). OCCT via `cadrum` (MIT wrapper, prebuilt static OCCT) is
  the credible optional kernel.

## Architecture decisions

1. **The band never enters a B-rep kernel.** cadkernel builds small analytic parts; parts are
   tessellated watertight and joined/cut into the height-field band by `csg.rs` in the same
   resolve stage seats and stamps use (`mesh.rs:371 resolve_solids`). Kills H4, H5 and the
   `mesh.rs:349` fork together. Feature values become `enum Value { Brep(Body), Mesh(csg::Solid) }`;
   Boolean routes to `brep::combine` only for small analytic pairs.
2. **CAD stops being a mode and stops forcing graph-driven.** `design.cad` is the design's parts.
   One typed `CadEdit` enum with two appliers: `Document::apply` (plain designs) and
   `nodes::cad::apply_edit(&mut Graph, ..)` (graph-driven designs). Paint/Stamp/Path keep working
   beside parts.
3. **No candidate model.** Each commit is evaluated on a cancellable worker, then lands as one
   `History` entry. Escape backs out one level (field → step → command → selection) and never
   discards committed work. Enter confirms a step only.
4. **Placement, pattern and mirror live outside the kernel.** A part is a cached local
   tessellation times a `csg::Frame` from `Placement::{Free, Ring{theta, v, height, spin, tilt,
   roll}}`; gizmo drags cost a matrix, resolve on release. Pattern/mirror are placement instances.
5. **One viewport stack in `ringdesign-workbench`** (desktop + phone): CPU BVH picking
   (vertex > edge > face > part, occlusion-tested, identical headless), selection with
   pre-highlight through the existing focus channel, `ViewCommand` trait + one `StepInput` funnel
   (GUI, MCP, tests), hand-rolled ring-frame gizmo (`transform-gizmo-egui` is Cartesian-only),
   tiered snap engine, `response.context_menu` per selection type. OpenCADStudio contributes
   patterns only (GPL): command trait with private step enum, tokens not mutation, two-phase
   commit, snap tiers, locked-DOF numeric fields, reflected command metadata for automation.
6. **Geometric references** `FaceRef/EdgeRef {source, at, dir, ordinal, signature}` in the part's
   local frame replace positional indices; a miss badges the feature instead of retargeting.
7. **Rounds come from three places**: 2D corner fillets in sketches (revolve/extrude/rail-sweep
   accept line+arc), kernel fillets where they work, and a mesh-domain rolling-ball seam bead at
   part/band junctions joined through `csg.rs`.
8. **Mouse-first discoverability is tested**: every command has an icon, a tool-rail slot, a
   palette entry and a menu path (a test in the spirit of `every_node_has_its_own_mark`).
9. **One v5 migration**, landed once in M1, walking `cad.feature` params inside `design.graph`
   too; a paired Android release ships with it so the phone keeps opening desktop files.
10. **OCCT is late, optional, desktop-only, child-process** (the `occt-worker` built with
    `kernel-occt`, cadrum; since batch 14 the app finds it at run time and a `package.sh --occt`
    build carries it): fillet/shell/general sweep/STEP import; results cached as meshes in the
    design file so default, phone and wasm builds still render and judge them.

## Milestones (merged order from three plans + sequencing critic)

| # | Milestone | Size | What Logan can do after |
| --- | --- | --- | --- |
| M0 | **Safety, reachability, first right-click** | 1 wk | Not lose work to a key; stop a bad boolean; right-click "Add primitive here" on the ring |
| M1 | **v5 schema + traced tessellation + risk spikes** | 1 wk | (foundation) |
| M2 | **Join/Cut that finish; CAD stops being a mode** | 2-3 wk | Fuse a bezel to the real band in seconds, keep painting it |
| M3 | **Pick, hover, multi-select, context menu, entity browser** | 2 wk | Faces/edges light under the mouse; right-click does the obvious thing |
| M4 | **One edit funnel, feature strip, per-feature status + cache** | 2-3 wk | Delete/reorder/suppress on a timeline; errors sit on the feature |
| M5 | **Command session**: G/R/S in ring frame, axis locks, typed values with **Tab-cycled dimension fields**, Place on ring, Join/Cut, grid/vertex/midpoint snaps, tool rail | 3 wk | Model by mouse + hotkeys with exact numbers |
| M6 | **Ring-frame gizmo**, ring dial, click-drag primitives, shared grips | 2 wk | Slide a head round the shank, spin, tilt |
| M7 | **Ring-aware snaps** (theta, side face, parting plane, stone stations, castability tint), work planes, bench pin, measure | 2 wk | Snap to what matters on a ring |
| M8 | **Sketch in 3D context**: pick a face or plane to sketch on (writes `Workplane::on_face`), ring underlay, plain `Sketch` features in the tree with "Extrude/Revolve/Sweep this sketch" on right-click, trim/offset/corner fillet, multi-loop regions, in-canvas dimensions | 2-3 wk | Draw a profile where it lives |
| M9 | **Press-pull, geometric refs everywhere, mirror, ring array** | 2-3 wk | Push faces; six prongs from one |
| M10 | **Gem-driven builders** as one `Operation::Builder{key, params}` + graph nodes: heads, bezels, baskets, halos; stone right-click → Setting | 3 wk | A solitaire in three gestures |
| M11 | **Verdict for parts** (per-face draft tint vs parting plane, stage rule: fused head under sand = bench + locating dot) and the **seam bead** | 2-3 wk | Honest casting read on CAD rings; filleted junctions |
| M12 | **Phone parity + shared renderer**, crisp B-rep edge pass (GLES-safe quads) | 2-3 wk | Same tools by touch |
| M13 | Rolling: shank builders, cutters/azures, stones on part surfaces, mixed STEP, 3MF per part, size-run with parts, **OCCT spike then feature** | rolling | |

Honest calendar: roughly 30 weeks for M0-M12 at one developer. H1/H3/H4-hang gone in week 1;
H4/H5 dead by ~week 4; hover + multi-select ~week 6; G/R/S + place + snaps ~week 12.

### M0 status — 2026-09-21

Landed (tests: core 475, workbench 33, GUI 46 green; H1/H3/H4/H7 replayed in the live app):

- `cad::evaluate_with(.., &BuildCtx { cancel })` polled between features and tessellations;
  `MAX_ANALYTIC_BOOLEAN_FACES` (2000) refuses a faceted operand in milliseconds instead of
  hanging (`a_faceted_operand_is_refused_before_the_kernel_is_asked`). The CAD worker carries the
  flag; a new edit or Cancel raises it; at most three abandoned kernel threads.
- Escape ladder: pending picks → history rollback; never the candidate. Enter previews only (not in
  the sketch tab); **Ctrl+Enter** applies. Cancel sets the candidate aside with a
  **Restore discarded candidate** button. Apply no longer clears `document_path` or bounces the
  workspace (`set_graph`). `interaction_tests.rs` pins all of it.
- `restore_desktop_view` puts a workspace's own view back into a visible pane on switch and at
  startup (`a_workspace_always_shows_its_own_view` sweeps every desktop).
- Right-click menus: Ring viewport (Add CAD part here → seats the part at the clicked ring angle and
  radial height, seeding a Procedural shank when none exists; Fit; Open CAD workspace) and CAD pane
  (Add here, Fillet/Chamfer this edge, Select feature, Isolate, Show all, Fit, Wireframe, Grid).
- Sketch: `Sketch::profile_curves` orders lines drawn in any order into one loop and names the
  failure ("open at point #n", "2 separate loops"); `remove_entity`/`remove_point`; canvas pan
  (drag empty / Shift / middle), zoom about the cursor, click a curve to select it, Delete key and
  button, **Empty** starter, rubber band while drawing, polyline closes on its first point or `C`,
  Enter leaves it open.
- Ctrl+K: CAD workspace / add box / cylinder / sphere / extrusion / preview / apply / discard.
- `controls::named` labels every numeric field for the AccessKit tree; `tools/egui_drive.py`
  drives the inspection port.

Still open from M0's list: Report dock on the CAD desktop; a kittest for "Enter during a polyline
does not Apply" (covered by the Enter-never-applies pin instead).

### M0 detail (first week)

- `app.rs switch_desktop`: repair any stored `DesktopLayout` lacking `desktop.pane()`; CAD desktop
  docks Report. General guard + kittest for every desktop.
- `panels/cad.rs`: Apply → Ctrl+Enter; Escape ladder with a "Restore discarded candidate" slot;
  polyline closes on `C` or first-point click; Apply keeps `document_path` (`app.rs:1193`);
  rewrite `interaction_tests.rs:55-74`.
- `cad.rs`: `BuildCtx { cancel: &AtomicBool, cache }`, `evaluate_with` polled between features;
  Boolean pre-flight refuses faceted operands with a message (placeholder until M2); orphan cap.
- Right-click in both 3D views via `response.context_menu` (`viewport.rs:932,1184` already
  `click_and_drag`): Add primitive here (ray hit → existing `ring_anchor_deg`/`anchor_height_mm`),
  Look at, Fit, Isolate.
- Cheap half of H7: entity/point delete, deletable starter rectangle, error naming the loops,
  canvas pan. Labels on every numeric field (`widget_info`), CAD entries in Ctrl+K.

### M1 status — 2026-09-21

- `cad::tessellate_traced(body, chord) -> (Mesh, PartTrace)` keeps the kernel's triangle→face
  ordinal through the weld and the stitch (`u32::MAX` for a stitched gap), the f64 positions, the
  surface kind per face and the body's vertices; `EvaluatedComponent.trace` carries it. Pinned by
  `a_traced_tessellation_names_the_face_and_kind_behind_every_triangle`.
- R2 retired (below); the boolean gate is 500 faces.
- v5 schema (2026-09-22): `Component.placement: Placement::{Free, Ring{theta_deg, across_mm,
  height_mm, spin_deg, tilt_deg, cant_deg}}` replaces the anchor pair (legacy fields fold in on any
  read; `migrate_v4_to_v5` rewrites the document and every `cad.feature` node inside
  `design.graph`); `EdgeRef`/`FaceRef { ordinal, signature }` replace positional indices (a bare
  number still reads; a signature is checked every evaluation, a moved edge is found again and
  noted on the feature, a missing one is refused by name); `Operation::Sketch`, `Profile::{Inline,
  Feature}` on extrude/revolve/sweep/twist/loft, and `Workplane::on_face` (a `FaceAnchor` whose
  plane is the face's own, outward normal, origin and x projected onto it).
- Open: `Component` attach/stage/blend (additive, with M2), `csg::Solid` from a trace, spikes R1/R4.
- Known red, pre-existing (fails on the tree before this work), three graph integration tests
  with one cause: `imported_bases::stock_masterworks_match_their_sources_with_native_maps_and_casting_modes`
  ("nocturne lost portable source data") and `showcase_templates::{showcase_graphs_reproduce_source_and_geometry_without_a_user_library,
  showcase_face_size_and_layer_edits_survive_evaluation_and_reload}` — the bundled
  `graphs/templates/nocturne-imported.graph.json` no longer matches
  `showcase/stock-masterworks/nocturne/design.ring.json`; regenerate one from the other
  (`showcase_templates --write`) when the Reptilia batch is reviewed. `cargo test` stops at the
  first failing test binary, so run the graph suite with `--no-fail-fast` to see all three.

### Dimension entry (M4/M5 design note)

The tool's dimension fields are real `egui::TextEdit`s in an `Area` by the cursor, one per free
dimension (a cylinder: radius, height; an extrude: height, taper; a sketch line: length, angle).
Typing a digit while the tool is live focuses the first field and starts its text. **Tab moves to
the next field and Shift+Tab back, inside the tool only**: egui moves focus on Tab (and surrenders
it on Escape) in `Memory::begin_pass`, before any UI runs, so a field cannot consume the key late.
egui **0.36.2** (emilk/egui#8530, 2026-09-08) adds `TextEdit::event_filter(EventFilter { tab: true,
escape: true, .. })` for exactly this: the field keeps focus and receives the Tab/Escape events, and
the tool reads `Event::Key { key: Tab, .. }` from `input_mut` and calls `request_focus` on the next
of its own ids. Logan is bumping the vendored `patches/egui` (0.36.0 + the numeric-input policy) to
0.36.2 for it; on 0.36.0 the same thing is `set_focus_lock_filter` by hand. A field with text is a locked degree of freedom (the mouse stops driving it);
**Enter** commits the step, Escape empties the field first, then leaves the step. The same fields
answer to AccessKit so kittest and `egui_drive.py` can type into them.

### M1 spikes (retire before writing refs into saved files)

| Risk | Spike (1-2 days each) |
| --- | --- |
| R1 graph-hosted CAD + snapshot cost | Prototype parts on a plain design beside paint; measure `History` cost with `Arc<Value>` graph |
| R2 uncancellable kernel calls | **Retired 2026-09-21** (`examples/kernel_probe.rs`): an intersecting faceted union is super-linear and never succeeds — 256 faces 0.3 s, 576 faces 3.8 s, 1024 faces 14 s, all `CutRefused`; 2304+ faces never return; `slice_by_plane` never returns on a 1024-face faceted body; analytic torus/sphere/crossing-cylinder pairs refuse in 0.1 ms. Gate set to **500 faces**. Band booleans go through `csg.rs` (M2), never the kernel. Child-process evaluation is not needed for cancel once faceted operands are gated. |
| R3 refs retarget silently | **Retired 2026-09-21** (`examples/ref_probe.rs`): ordinals are stable under every parameter sweep of Box/Cylinder/Torus/extruded circle, and keep their face kinds (drifting only in direction) for Extrude with draft, Loft and Sweep; they retarget for real when topology changes — Revolve 360°→180° grows caps (6→12 edges), and a boolean whose cut moves. **v5 stores `EdgeRef { ordinal, kinds: [SurfaceKind; 2], curve, dir, mid }` in the part's local frame**: accept the ordinal when kinds+curve match and `dir·dir' > 0.9`, else search for the best signature within a midpoint tolerance, else badge the feature. Also measured: `box − cylinder` through-holes are refused by position (x=0, 1, 4.5 succeed; x=2, 3 `CutRefused`) — drilled holes and pockets belong to `csg.rs`. A whole `Curve::Circle` is refused by `extrude`; `profile_curves` now hands the kernel two arcs, so circle sketches extrude. |
| R4 seam bead folds | Cylinder-on-torus and 0.8 mm wire-on-dome through `csg` + variable-section sweep; `self_crossings == 0` over 200 placements |
| R5 anchors ignore relief | **Retired 2026-09-21** (`examples/anchor_probe.rs`): the reference-crest anchor (`cad.rs:652`) is exact on plain bands (Court, Wishbone, Split: 0.00 mm) but buries a part's foot by **+2.06 / +2.42 / +2.26 mm on the three signets' shoulders** (45°/135°), +0.30 at their tables, +1.34 on the cathedral stock's top, +0.21 under braided relief, +0.92 on the toi et moi. **v5 `Placement::Ring` resolves by ray-drop onto the built mesh** (radial ray in the finger's plane at θ, then along the hit normal), with `height_mm` a stand-off from the true surface; the M0 right-click already uses the clicked hit's radius, so it is exact where the click was. |

### M2 status — 2026-09-22, phase 1 (branch `m2-integration`, three worktree agents)

- `csg.rs` is the road every part takes onto the band: `combine_with(.., cancel)` with
  `Snag::Cancelled` polled inside the pair loop (a flag set mid-way is seen in ~1 ms; a pre-set flag
  returns before a 786k-face band is touched), `Solid::check(crossings)` (grid-culled, no longer
  O(n²)) and `strip_zero_area`, inputs refused as `Snag::Unclosed` before any work,
  `combine_traced` → `Traced { solid, parent, seam }` with `seam_loops` (a bezel on the Court band:
  one closed loop, 118 vertices at preview, 284 at export), `cluster(parts, pad)` for eight collars
  round a ring. Contact matrix pinned: a coplanar foot resolves after the first nudge, sunk 0.05 mm
  resolves first time with the exact volume, a tangent sphere is lifted clear and stays unfused.
  Cost: every `combine` now validates both inputs (~12 ms per 786k faces; ~+0.8 s on Oriel's 4.3 s
  export build) — a `combine_unchecked` for chained outputs is the fix if it matters.
- `Component.attach: Attach::{Separate, Join, Cut}`, `stage: Stage::{Cast, Bench}`, `blend_mm`
  (serde defaults, no format bump), `Component::attaches()`, and `cad_tools::attachment` on both apps.
- A CAD Apply on a plain design stays plain: `apply_plain` copies the evaluated document into
  `design.cad` and re-lifts it; the graph path is unchanged; Undo restores `design.cad`
  (`a_plain_design_stays_plain_after_a_cad_apply`, `a_graph_driven_design_keeps_its_graph_after_a_cad_apply`).

### M2 status — 2026-09-22, phase 2 (branch `m2-integration`, three worktree agents + integration)

- **CAD stops being a mode.** `mesh::try_build` always sweeps the band when it is procedural
  (`RingDesign::band_is_procedural` = no enabled `Band` feature *and* a real feature means the CAD
  replaces the band); seats and stamps resolve as before, then `parts::resolve` evaluates the CAD
  with the built mesh as its surface and joins, cuts or appends every output part through
  `csg::combine_traced` (joins clustered by padded box, one tool per cluster; a cluster that will not
  unite falls back one by one). `Mesh.origin` names each part (`Resolved::feature_of`), the
  evaluation rides along in `Resolved.evaluated` so the CAD pane never evaluates twice, and
  `try_build_with(.., cancel)` stops the CAD stage within ~150 ms of a raised flag.
- **The Band feature is an anchor, not a body** (`Evaluated.band`); a Boolean against it is read as
  the other operand's attachment (Union → Join, Subtract → Cut), so every existing document keeps
  its meaning without a migration. `Placement::frame_on` drops a Ring placement onto the built
  surface (Heart signet shoulder: 2.06 mm off → < 0.02 mm).
- Measured (dev profile): bezel r 3 × 2.5 joined + pilot cut — Court band 256×128: band 10.9 ms,
  band + parts 41.8 ms; at 1024×384: 122 ms → 500 ms; every case open 0 / non-manifold 0; a Join
  adds 69.7–70.7 mm³ of π·9·2.5 = 70.69.
- Every surface-tool gate (Paint, Stamp, Path, Move ornament, unrolled editor, section, report,
  node focus, manufacturing inspection, imported-base attach) now reads
  `band_is_procedural()` through one body, `interaction::surface::replaces_band`; a ring of parts
  only says `PARTS_ONLY`. New parts beside a procedural shank default to **Join**; the CAD pane
  previews the whole ring (`Parts only` toggle for the old view) and Live cuts off skips the joins.
- Bench-stage parts are shown finished and left out of a sand pattern (`pattern_parts`); the field
  verdict judges a procedural band with parts as it judges made settings, with a note.
- **Spike R4 retired** (`blend.rs`, `examples/bead_probe.rs`): a rolling-ball bead swept along the
  traced seam and joined through csg fillets a post on a plane to **0.0046 mm** of the analytic
  torus (96-gon, r 0.3), rounds a drilled rim as a cut, survives a plus-shaped post's re-entrant
  corners by pinching (fold guard), and beads 20 random bezels round the Court band clean (0 of
  3831 stations clamped, ≤ 196 ms debug). Known limit: a wire lying on a low dome closes its wedge
  and clamps to the 0.02 mm floor; sink it 0.4 mm or stand it up.
- **Verified live** (`851c0de`): File ▸ New, CAD workspace, right-click the band ▸ Add here ▸ Cylinder —
  the procedural shank comes with the first part, the cylinder seats where the click landed, the
  CAD pane previews the whole ring, and after Apply the Model workspace shows one fused mesh
  (668.7 mm³, Castable, "1 CAD part stands on the band … (1 joined, 0 cut, 0 separate)") with the
  Design panel editable and Paint 3D offered. H2 and the exclusive mode are gone.
- Open (phase 3): wire `Component.blend_mm` into `parts::resolve` (the bead must keep the part's
  origin provenance — `fillet_junction` compacts its result today); `launch()` still seeds a Band
  when a design has no CAD document; `assembly::inspect` no longer sees a band component;
  `manufacturing::prepare`'s `uses_band` still counts the Band id in `outputs`; the csg input
  census costs ~12 ms per 786k faces per combine.

### Phase 3 status — 2026-09-22 (on master: `b97fe3d`, `8ce5558`)

- **`Component.blend_mm` reaches the build**: after a Join or Cut, `parts::resolve` reads every
  seam loop off the traced boolean, beads each (`blend::bead_seam`, the non-compacting variant, so
  every bead vertex names the part) and lays it in through the same chain; a bead that fails is a
  note, never a failed build; `Resolved` counts beads and clamped stations. `csg::combine_unchecked`
  skips the census on a chained output: Oriel's `setting::apply` 428 → 207 ms preview,
  2723 → 892 ms export. Measured: a 1.5 mm bezel sunk 0.5 beads clean (0/192 clamped); a 3 mm
  bezel overhanging a 4 mm band pinches at four corners (135/303) — a stone is sized to its face.
- **M3's pick scene** (`interaction::{bvh, pick}`): one BVH over the built ring; faces, edges and
  vertices of parts answer from their placed traces (face ownership by origin + centroid on the
  part's own tessellation), stones by path, the band by (θ, v); ranking vertex < edge < face <
  part < stone < band, occlusion-tested. Measured: scene build 7–9 ms at preview, 81–84 ms at
  export (780k faces); a pick 2–3 µs; box select 0.3 / 3 ms. Open: CAD-only rings have no origin
  provenance (`parts::assembled`), seat solids and stamps read as Band.
- **The Ring viewport selects** (`fb713d8`, `workbench::viewport::{selection, menu}`): hover
  pre-lights through a second focus channel (attribute 5) and names the entity ("Cylinder face 1
  (plane) · Tab 1/2"), click selects, Shift adds, Ctrl removes, Tab / Alt-click walk the depth
  stack, Escape clears; right-click lists what the selection can do (band: Add CAD part here, Fit,
  Open CAD; part: Edit feature, Attach and Stage with the current one ticked, Isolate; edge:
  Fillet / Chamfer this edge; face: a "Face n of <part>" heading). Hover costs 1.2 µs mean, 11 µs
  worst on 780k faces; the scene rebuilds in `tick` at 7-9 ms per preview build. **Verified live
  2026-09-22**: right-click the band → Add CAD part here → Cylinder, set 1.5 × 2.5, Apply; back in
  the Model workspace the report reads "1 joined", hovering the post lights its end face and the
  viewport's AccessKit label says "hovering Cylinder face 1 (plane)", click + Shift-click the band
  gives "2 selected", Escape clears, Attach → Cut turns the post into a pocket (568.27 → 559.88
  mm³) and Undo restores the join. The probe's Shift-click pins were removed with it (the Measure
  tool measures). Open: box select is in the scene but not on a drag; CAD-only rings still read
  as band (fixed by `m4-feature-status`); until its first evaluation lands, the CAD pane says
  "Parameters changed — preview to evaluate this candidate" when nothing has changed.

### Batch 4 status — 2026-09-22 (on master: `3d48634`, `a8ec71c`, `acda446`)

The three agents hit the model's usage limit before committing; they were resumed in their
worktrees, then verified, measured and committed by the integrator.

- **M4's edit funnel** (`603cbe0`, `core::cad::edit`, graph `nodes::cad`): one `CadEdit`
  (Add, Remove, Move, Enable, Rename, Operation, Component, Placement, Attach, Stage, Blend,
  Outputs, Through). `Document::apply` validates before it mutates and names the features it
  refuses on; the graph's `apply_edit` reads the chain back into a document, applies the edit
  there and re-encodes it, transactionally; `edit_design` dispatches plain vs driven. Every edit
  kind gives the same document byte for byte through both, on every example (`tests/cad_edits.rs`).
  Cost on the 7-feature gallery: 0.03-0.24 µs on the document, ~57 µs on a graph, ~105 µs on a
  driven design's stored graph.
- **M4's per-feature status and cache** (`d04c0bd`): `FeatureStatus::{Ok, Suppressed,
  Failed(msg), Skipped(reason)}` on every `FeatureReport`; evaluation carries on past a failure
  and skips only what reads it. `cad::Cache` + `Memo { cache, surface_epoch }` through
  `evaluate_memo` / `parts::resolve_with`: a warm edit of the gallery's last feature 35.1 → 2.3 ms
  at preview, 89.5 → 4.6 ms at export. The boolean resolve that joins parts into the band is not
  cached and now dominates a part edit (34 ms preview, 377 ms export on the Court band). CAD-only
  rings carry `Mesh.origin` per part and their evaluation, so they pick like the rest.
- **M5's command core** (`f56d8b8`, `workbench::command`): `Session` + `ViewCommand` over one
  token vocabulary (`StepInput`), `Outcome::Commit(Vec<Effect>)` as data for the funnel, the
  escape ladder, `catalog()`; Move/Rotate/Scale/Place/AddPrimitive/Attach in the ring frame with
  axis locks and typed values; the `DimensionBar` on `TextEdit::event_filter` with Tab and
  Shift+Tab kept inside the bar, pinned by kittests with decoy buttons; the tiered `Snapper`.
- **Integration** (`b7507f6`, `2a460e7`, `ede8c95`): `gui::cad_edit::apply` is the app's one road
  for a committed CAD edit (the viewport's Attach and Stage use it, now on driven designs too);
  `mesh::try_build_memo` lets a worker keep a `cad::Cache`. Two defects found driving the app:
  "Convert design to graph" carried a CAD document as one `design.set` at `/cad`, which the funnel
  cannot read, so every edit of a converted design was refused — the lift now chains the document
  as `cad.feature` nodes (`nodes::cad::chain_document`); and Undo of a funnel edit on a driven
  design took back only the build's splice, because the funnel's entry was committed before the
  evaluated document arrived — the funnel now carries the document the edited graph reads as.
  Both pinned (lift test over every CAD example; a kittest with the Graph workspace open). The
  Android crate still builds and its 132 host tests pass.
- Next (batch 5): the GUI takes them — the feature timeline on the Ring viewport and in the CAD
  pane through the funnel, G/R/S/Place/Attach/add in the viewport with the dimension bar and a
  moving ghost, box select, the worker holding the cache; and M8's sketch core in parallel.

### Batch 5 status — 2026-09-22 (on master: `eee2f15`, `bfc7e30`, `bff0c43`, fixes `3d27332`-`725b594`)

- **M4's feature timeline** (`workbench::timeline`, `panels::timeline`): one chip per feature with
  its status — Ok, Suppressed, Failed and Skipped with the evaluation's words, and Pending whenever
  the document moved since the evaluation, so a stale red or green is never shown. Click selects
  the part, double-click edits it, a drag reorders (a drop the funnel would refuse shows the
  funnel's own reason and applies nothing), the rollback marker drags, and the menu has Rename,
  Suppress, Delete (refused with the dependents named), Delete with dependents, Roll back, Isolate
  and Move earlier/later. A strip under the Ring viewport, a list in the CAD pane (which edits a
  pending candidate's draft instead of discarding it); Delete over the strip removes the chip.
  60 features: `chips()` 18-20 µs, `show()` 58-85 µs a frame.
- **M5 in the Ring viewport** (`gui::command`, `workbench::command::ring`): G, R, S, P (place under
  the pointer), J (cycle Join/Cut/Separate), Shift+A (add Box/Cylinder/Sphere at the click), X/Y/Z
  locks read as the ring's axes, typed values in the dimension bar, a ghost drawn under a model
  matrix (no upload per frame), the Escape ladder, right-click cancels, B for box select (left to
  right a window, right to left a crossing, Shift adds, Ctrl removes), a tool rail, palette
  entries, and `every_command_has_an_icon_a_rail_slot_a_palette_entry_and_a_key`. A pointer sample
  costs 5.7 µs mean and 28 µs worst on a 646k-face build; the band surface the ring frame is read
  on is built at the first key press after the band changes (34 ms preview, 220 ms export). The
  worker keeps a `cad::Cache`, but a warm part move is 68.5 ms against 72.3 cold: the csg joins
  dominate.
- **M8's sketch core** (`sketch::{edit, region, solid}`): split, trim, offset (lines stay lines,
  arcs stay arcs), corner fillet and chamfer, mirror, rectangular and polar patterns; regions with
  holes by even-odd nesting (a washer extrudes at −0.040%, two squares are two lumps); a sketch on a
  part's face takes the face's own frame (area centroid, longest straight edge, outward normal),
  is re-found by signature and moves with the face — a post on a box rises 0.5 mm when the box
  grows 1 about its centre. The kernel's planegcs port was measured and not adopted: LGPL, its
  constraints are `Rc` (not `Send`), and it brings a second nalgebra; it would lift the 128-point
  solver cap (about 30 fillets in one sketch).
- **Integration fixes**: sketching on a box's face dropped the box, because output bookkeeping
  read `sources()`; it reads `Operation::consumes()` now. Undo during a live command ends the
  command and takes back nothing else (the command used to commit over what Undo restored); the
  Escape router skips while a command or box holds the viewport; a graph edit that only moves
  nodes no longer rebuilds the ring; the lift lists nodes in id order, which the editor's first
  frame used to re-sort into a spurious edit after every Convert.
- Verified: core 548, workbench 91, gui 74, graph 81 + 6 (the three Nocturne tests red as
  before), graph-ui 26, mcp 43, cli 5, configurator 5, script 5, android 132 host tests, wasm clean,
  zero warnings across the workspace (the machine's `-Awarnings` is gone, 65fbfdd).
- **Verified live** on a restored graph-driven band with one cylinder: select it, G, type 12,
  Enter → it moves 12° round the ring as one History entry; Undo takes it back; Ctrl+Z during a
  live move ends the move and takes back nothing else; J turns the pocket into a post
  (559.89 → 585.91 mm³); the strip's menu suppresses and Undo restores; B and a window drag take
  the whole post without orbiting. Three defects found and fixed: Shift+A on a driven design was
  refused ("Feature identity #5 names another node", the command asks for `Document::fresh_id`,
  which knows nothing of the graph's other nodes) — the funnel now asks the graph for a fresh id
  where one of its nodes carries the asked-for one; after an add on a driven design the editor's
  auto-arrange was recorded as its own History entry, so the first Undo only moved nodes —
  History treats a change that only moves graph nodes as no edit; and History named a CAD edit by
  its first changed field ("Across 0 mm -> 0.50 mm") — funnel commits carry the edit's own label.

- Open: the band surface for the ring frame is built on the UI thread; a Sketch feature has no
  cache slot; which regions a `Profile::Feature` extrudes is a schema decision
  (`regions: Option<Vec<usize>>`); a sketch strip on a driven design lags its graph by one build.

### M11 status — 2026-09-22 (core half on master: `75cc5ec`, merged in the batch-6 window)

- **The verdict judges the parts a ring pours with** (`castability::judge_parts`,
  `judged_field_report`): every enabled, non-reference, Cast-stage Join or Cut part's faces in the
  built mesh — owned by the pick scene's one rule (`interaction::pick::part_owners`), beads
  included — read at the field's parting plane with the face analyzer's classes; their areas join
  the report's and can only worsen the verdict. `FieldReport.parts` carries one `PartVerdict` per
  part (undercut, silhouette, marginal, vertical, total, worst draft, where), and the notes say
  what to do: "stage it Bench to solder it on after the pour … or move it onto the parting line",
  "drill it at the bench". Lost wax measures and never gates; Separate parts are noted, not judged.
  Measured: a post on the parting line adds 0.000 mm²; 1.5 mm off it locks 5.23 mm² against 5.21
  from the geometry; a radial pilot hole locks both walls, 1.51 against 1.51. Cost 1.9 ms on a
  preview build, 10.5 ms on an export build.
- **The stage rule**: under sand a Join or Cut part staged Bench leaves the pattern and a raised
  locating or drill mark takes its place in the height field (0.4 × the foot, 0.6–1.0 mm, 0.2 mm
  high), at the foot when the field says it pulls there, else on the parting line at the part's
  angle with the offset said in the note. The finished ring never shows it.
- The draft colours paint at the verdict's own plane (`analyze_at`): the mesh's own plane sat
  0.02–0.14 mm away and painted 4–12 part faces differently. The GUI worker judges parts on every
  build with live cuts on; exports, the phone, MCP, the CLI (check and the size-run manifest), the
  graph's SandRing export and Python judge on the build they have. `casting_pattern` and
  `pattern_parts` now take the library (the marks read the field).
- **A part at the crest leaned with the build's resolution**, found while merging: grid vertex
  normals took the plain chord between neighbours, and the row snapped onto the crest sits nearer
  one of them, so the crest normal tilted 0.84° at preview and 0.38° at export — and a post seated
  there tilted with it, turning its flat top into 0.7 mm² of "undercut". The tangent is now the
  second-order difference for uneven spacing wherever the grid resolves the curve (neighbouring
  chords turning under 6°): 0.002° and 0.001°. Across a fillet the grid does not resolve (a 0.1 mm
  edge round over 0.13 and 0.21 mm rows) it keeps the plain chord, which reads it better (0.1°
  against 3.0°).
- Open: the phone lists no parts (its notes carry them); placing a bench part's mark costs a
  design-only verdict 94 ms; the silhouette rule reads a fan-triangulated flat face touching the
  plane as silhouette.

### Batch 6 status — 2026-09-22 (on master: `m6-gizmo`, `m10-gem-builders`, `m8-sketch-ui` merged, integration `6e88026` and after)

- **M6, the gizmo** (`workbench::{gizmo, grips}`, `gui::command`): with one part chosen, arrows
  round the ring, across the band and off the surface and rings for spin, tilt and cant, in the
  ring's own frame (a free part gets world axes); the **ring dial** at the part's crest radius
  slides it round the shank on a 5° grid (Ctrl frees it); Shift+A primitives also take
  press-drag-release then a lift for the height; the CAD pane's parameter grips moved into
  `workbench::grips` and show on the chosen part in the Ring viewport too. Every drag is a command:
  ghost, caption and dimension bar as the hotkeys have them, one History entry on release, Escape
  or the right button cancels, and a press on a handle never orbits or reselects. A gizmo frame
  costs 7-9 µs to seat, 6 to lay out, 0.8 to hit-test, 12 to paint; a drag frame 7 µs mean and
  21 µs worst on a 650k-face build. Verified live: the dial slid a post round the shank as one
  "Place Cylinder" entry.
- **M8, sketching where the profile lives** (`gui::sketch_mode`, `workbench::sketch_tools`,
  `sketch::{draw, dimension, query, fill, anchor}`): "Sketch on this face" anchors a Sketch
  feature to a planar face of a part; "Sketch on a plane here" takes the plane square to the band
  at the click (a toggle turns it to the section through the finger's axis). The camera looks
  along the plane, the ring stays as the underlay with the face outline lit, and the toolbar draws
  lines, rectangles, circles and three-point arcs, trims, offsets, fillets and chamfers corners,
  mirrors, deletes and dimensions (a typed value is a constraint and the solver runs), with snaps
  to ends, middles, centres, crossings, the face's edges and corners, the plane's axes and the
  grid, point drags that hold the constraints, in-sketch undo, and an Escape ladder that asks
  before it drops strokes. Finish is one History entry; a region's right-click extrudes or revolves
  it, and a part extruded on a face moves with the face. The CAD pane's canvas uses the same
  tools. 200 entities cost 1.7 ms a frame; a dimension solves in 0.02 ms on one rectangle.
- **M10, gem-driven builders** (`cad::builders`, `Operation::Builder { key, on, params }`, a
  mesh-valued `cad::Value`): a stone, claw heads (4 and 6), a bezel, a basket, a seat bur and a
  halo, each seated on its stone's frame with parameters defaulted from the gem and a schema the
  inspector reads (`panels::builder`), faces named by patch ("Claw 3 of Four-claw head"). The
  solitaire in three gestures: right-click the band ▸ Add stone here (8 presets), right-click the
  stone ▸ Setting (Four claws, Six claws, Bezel, Basket, Halo), each one funnel commit; a
  height-field stone takes a setting the same way. Measured on the Court band: the head is 45.19
  mm³ and joining it adds 44.80 (its feet share 0.39 with the band), the bur takes 5.78, the 6.5 mm
  stone is 1.04 ct, the ring stays one piece; a halo seats 12 melee of 1.3 mm on equal 2.355 mm
  chords; a solitaire builds in 80 ms at preview and 549 ms at export.
- **Integration**: every evaluated part records the frame the build seated it by
  (`EvaluatedComponent::frame`), and sketch anchors and fillet-edge references are signed in it —
  sketch mode no longer sweeps a band of its own, and edges on a part seated on a leaning flank no
  longer retarget. Stones set as parts are drawn with the gem previews and picked as parts. With
  M11 judging parts, the Setting gesture stages a head and its seat Bench under sand (a cast claw
  head locks the mould by name, pinned) and the pattern carries a locating mark and a drill mark.
  STEP skips builder parts; the toolbar's Undo and Redo work inside a live sketch and a History jump
  waits for it; double-clicking a Sketch chip reopens it in the Ring viewport.
- Open: the sketch solver refuses past 128 constrained points (about 32 held rectangles); extruding
  one region of several needs a schema field; builder rails count as creases at the 30° threshold,
  so a hover lands on edges before faces; the CAD pane's first grip drag lands off the pointer (its
  scale changes when a candidate becomes a draft); the bare band the ring frame is read on still
  builds on the UI thread, now when a part is first chosen; the phone has none of the M3–M10
  viewport tools yet (M12).

### Batch 7 status — 2026-09-22 (on master: `m7-ring-snaps`, `m9-patterns`, `batch6-sweep` merged, and their integration)

- **M7, snapping to the ring** (`workbench::command::snap`, `castability::ghost`,
  `visual::measure`, `viewport::pins`): a command lands a part on ten tiers of targets, ring
  features before the grid — pins, stone stations, side-face boundaries and centre lines, named
  angles (the top, the sides, the palm, a signet's head, every other part's theta), the parting
  line, part vertices, midpoints and edges, the grid — snapping where the part lands rather than
  where the pointer is, each labelled, Ctrl to free; the gizmo's dial and arrows use them. While a
  part is carried its ghost is painted by the draft class each face would have at the field's
  parting plane and the caption says what it would do ("Ghost would lock 2.1 mm² at -35°"): 0.22
  ms a frame on a 7,692-face claw head. Measure reads between any two picks — vertices, edges,
  faces, band points, stones — with a dimension line, Shift chaining a third. Pins (right-click
  the band ▸ Pin here) are references the snaps and Measure read; they live in the workspace, by
  design file, so they never dirty the design. **The ring frame comes from the build**:
  `BuildResult.band` is the band as swept before seats, stamps and parts, the surface parts are
  seated on, so choosing a part no longer sweeps a band on the UI thread (14–16 ms sweep + 19 ms
  tree at preview, 94–121 + 131 ms at export); the worker builds the tree per band change on CAD
  designs. A part over a stamp or a seat's solid now sits on the band, not on top of them.
- **M9, patterns, work planes, press-pull** (`cad::pattern`, `workbench::command::pattern`):
  `Operation::Pattern { source, kind }` — round the ring (an integer count closes on itself),
  round a stone or part (its frame's axis: "six prongs from one"), or a mirror (across the band's
  mid-plane, through the finger axis at a theta, or across a work plane) — is one mesh component
  of placed copies of the source's tessellation, a seated source re-dropped onto the band at each
  copy's own angle, the source staying its own output. `Operation::Plane` is a work plane (a
  section, the plane square to the band, the parting plane, a planar face, each offset); sketches
  lie on one and mirrors use one. Press-pull on a planar face edits a primitive's size where the
  face maps to one and otherwise pushes the face through the kernel. Every bare face or edge
  reference is signed in its part's seat frame when it is committed (`cad::sign_refs`). Measured:
  six prongs at 60° steps to 1e-6, one watertight piece; three heads each stand off the built
  surface within 0.02 mm of the source; a mirror keeps its volume to 1e-9 and both copies are
  judged; a press-pulled box top adds exactly its area × the distance; a fillet survives the
  box's resize within 3% of the analytic round; the two appliers agree over 143 edits. Twelve
  posts cost 109 ms at preview and 1.38 s at export; three claw heads 225 ms and 2.12 s.
- **The batch-6 sweep**: the sketch solver solves independent systems apart, each by damped
  Gauss-Newton on one sparse LDLᵀ per step — thirty held rectangles 4.39 → 0.13 ms, two hundred
  (800 points) 0.68 ms, caps now 512 points to a system, 1024 items, 2048 constraints.
  `Profile::Region { feature, region: RegionRef { entity, at } }` extrudes or revolves one region
  of several, found again by its entity, else by its point, else failed by name; the sketch
  mode's region menu and the CAD pane's picker offer it. A builder's tube facets no longer count
  as creases (a claw head's 2714 edges past 30° keep 1004; its notch, a bezel's lip and the stone's
  table stay), so a hover lands on a claw's face. The CAD pane's canvas holds still when a
  candidate becomes a draft (its banners now overlay it), and the claw solitaire is in the gallery.
- **Integration**: an edge's menu still leads with Fillet and Chamfer, the patterns after them;
  press-pull reads the pointer on the view plane through the face; bare references are also
  signed when the CAD pane adds a feature (against the evaluation from before it consumes its
  source); a sketch on a work plane opens in the Ring viewport; `SET_STONES` is gone.
- Open: an array's ghost uses world motions, so on a modulated band the committed copies can
  differ from what it showed; work planes are not drawn or pickable in the viewport; pins do not
  travel with the file; a Cut part's ghost over-reports faces outside the band; array and
  press-pull are in the right-click menu only (not the catalog or the rail); Sweep, Twist and
  Loft ignore a Region profile; **the four-claw builder on a 5 mm round at 60° on the default
  band reaches into the finger hole** (faces at r 8.61 mm inside the 8.65 mm bore) — the claws'
  reach to the band needs the bore as its floor; the floating tool inspector covers the viewport's
  middle in small windows. Seen live: the dimension bar anchored at the pointer spills past the
  viewport's right edge; a pattern part (placed Free, following its source) shows a world-axes
  gizmo whose drag would wrap it in a Transform.
- **Verified live**: right-click a post ▸ Pattern ▸ Array round the ring…, the six ghosts round the
  ring with the caption and the Instances field, type 4, Enter → "Ring array of Cylinder" on the
  timeline, three copies at 90° steps, one History entry "Add Ring array of Cylinder".

### Batch 15 status — 2026-09-24 (on master: `b15-leftovers`, `b15-stamps`, `b15-skin`, `b15-cad-stones`, `b15-open-speed` merged, and their integration; not pushed)

The platform half of the director's plan (`scratch/b15/plan.md`, batch 0: P1–P4) and batch 14's leftovers.
No ring was authored; the Bestiarium starts on this.

- **Templates and files open cancellably, with expressions, and nothing is done twice (P1).** The
  evaluator stops between nodes and the bake between sources, so Cancel, a second choice, or a file from
  Recent stops an open within a step; both apps' opens run the script engine (a template with an
  expression pin now opens from the menu); the bar never falls; the old design stays live until the new
  one lands; a library that moved is baked onto again. Every raster a design's artwork makes is shared by
  content digest and every distance field by its alpha, so a rebuild of an unchanged driven design bakes
  nothing (Nocturne's per-build bake 397 ms to 0.2; undo and redo 442 ms to 1.5). File > Open and Save
  run on threads; the phone autosaves on a writer thread. `template_open_probe`, release, ms:

  | template | MB | unpack | read | evaluate | bake | first build | verdict | rebuild | detail cold / again |
  | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
  | nocturne | 0.1 | 0 | 1 | 3 | 109 | 19 | 56 | 0.6 | 1355 / 35 |
  | thalassa | 0.1 | 0 | 1 | 4 | 85 | 24 | 135 | 0.7 | 1957 / 66 |
  | oriel | 0.1 | 0 | 1 | 5 | 10 | 316 | 67 | 1.0 | 383 / 56 |
  | nocturne-imported | 3.8 | 6 | 8 | 16 | 8 | 266 | 58 | 2.3 | 2952 / 33 |
  | vesper-imported | 3.8 | 5 | 8 | 19 | 7 | 261 | 67 | 2.4 | 4406 / 38 |
  | solstice-imported | 13.5 | 11 | 28 | 58 | 6 | 16 | 24 | 4.3 | 1059 / 16 |
  | zenith-imported | 9.1 | 6 | 22 | 48 | 2 | 260 | 8 | 3.1 | 71 / 12 |
  | caiman-imported | 12.7 | 13 | 26 | 57 | 7 | 87 | 19 | 4.9 | 2803 / 20 |
  | varanus-reptilia | 12.7 | 15 | 25 | 45 | 12 | 16 | 12 | 6.3 | 692 / 5 |

  Every starter opens in under 1 ms of reading and 0–83 ms of evaluation; the shared bakes hold
  41.8 Mtexels after the whole catalogue. Detail is no longer on the open's path: the worker measures
  it after the first build, once a panel reads it.

- **The UI thread stopped measuring detail.** The Report and Layers panels called `dfm::findings_in`
  every frame (35–64 ms warm, and the first call on a new design measured every mask cold: a 29 s landing
  frame on Caiman). They read `detail_findings()` now, which the build worker measures after it sends
  each build, only once something has asked, and gives up when the next job is dispatched. Release,
  `ui_thread_costs_on_heavy_designs`, ms (before = the lane at `e7ed75f`, panels still measuring):

  | ms | Nocturne before | after | Caiman before | after |
  | --- | --- | --- | --- | --- |
  | idle frame (median) | 72.0 | 2.1–6.0 | 39.7 | 2.7–3.7 |
  | slowest frame while it opened, landed and built | 147.7 | 53–57 | 103.4 | 42–46 |
  | history commit of a settled edit | 1.3 | 1.3 | 31.1 | 31.0 |
  | undo / redo | 1.1 / 1.1 | 1.1 / 1.0 | 34.1 / 33.9 | 32.3 / 32.4 |
  | session save | 15.1 | 15.5 | 41.1 | 43.0 |
  | open from Recent | 19.8 | 6.9 | 49.4 | 47.5 |

  "Before" is e7ed75f's own release binary run today at the same load (the reviewer measured 73.4 and
  40.0 idle). While the worker's cold measure runs — 1.4 to 4.4 s after a landing, on every core — the
  UI thread's work is about half as fast again: Caiman's commit, undo and redo 47–58 ms, session save
  62, Recent 77–88.

- **CAD stones are stones everywhere (P2).** A stone part, a halo's melee, every copy a pattern makes
  of a stone or a head, a stone a Transform moves and whatever a Boolean keeps enter the one record
  (`StoneSource::Cad`), seated on the bare band's own section (0.0001 mm from a 1024-slice build on a
  signet's shoulder, where the reference crest misses by 2.6 mm) or read off a build
  (`set_stones_built`); the report, census, sheet, stone map (which refused a claw solitaire),
  section view, reel and both viewports count them. `Operation::Pattern` takes several sources (fenced
  at design 6 / graph 2). `render::finished` is the metal and every stone by colour; the stone builder
  carries a tint; the CAD thumbnails and template shots are studio gold with stones set.
- **The skin is core (P3).** `skin.rs` (`Atlas::of`, `Hide`, `Joints::eccentric`, `draft_clamp` →
  `ClampReport`, `hide_layer`) and `imported_base::sand_master`, moved from the example so Saurian,
  Zenith and Caiman rebuild byte for byte; the atlas lands on the swept mesh within 0.0006 mm on a
  keyframed band and 0.049 mm on a bypass. The spikes: the eleven sand-safe plans bare through the master: 002 Kite, 006 Square, 015 Octagon
  and 017 Tonneau Castable, 001 and 012 Cushion and 013 Round Marginal (at most 0.21%), and 003 Clover
  13.2%, 005 Rosette 6.6%, 007 Quatrefoil 14.7% and 016 Star 2.9% NotCastable — Viscum, Sigillum,
  Chelonia and Phrynosoma must answer their plans' own lobes first; 013 reaches 16 mm in two resize
  steps (sand 0.045% at −1.1°); a prickle hooked in the parting plane pulls clean (tilted 2° it blocks
  at 0.075 mm, spun 90° it blocks); side-face cells spill 0.51 mm onto the crown fillet at 1.36× width
  and 1.00× thickness (NotCastable 3.43%), so Phoenix keys thickness at or above width until P5; a holly
  leaf's spines may lean 20° on 006's parting line, and 25° blocks.
- **Stamps, second version (P4).** Tiers (a stamp struck on the ring as struck so far), shaped tops
  (Gable, Ridge, Cone, Dome, Taper; the floor of a cut), sixteen outline families, `parting_monotone`,
  `hull_and_bays`/`cast_as_hull`, `stamp_row` along the parting line, a chart `v` or a side face, with
  taper, fold clearance and mirrored shoulders, and tier-aware DFM. A design pouring only plain stamps
  strikes them exactly as master did (hashes equal for Zenith, Caiman and the Heart signet, finished
  and pattern); a tier, a shaped top or an outline past 512 points writes format 6.
- **Leftovers.** A cut carves a ring of parts alone (desktop and phone sketches), and its graph, cluster
  or preset is fenced at graph 2 when it may evaluate to one (a band counts only when it certainly
  feeds the output); a pocket's walls pick as the cut; the CAD footer's Fit frames all the metal and
  the canvas menu's Fit view the history pick; a primitive clicked on the spot keeps its default size;
  relative worker paths read from the executable's folder; an install prunes the unpacked OpenCascade
  worker; a node's migration runs only on files older than the shapes this build writes; the phone's
  stamp window stays in a landscape view with the keypad up.
- The integration also fixed: a Transform round a head counting its stone twice, both held (one, where
  it was, unheld); a pattern's mesh drawn at every stone once a later Transform consumed the stone (six
  bodies for three, 14.98 mm off); a pattern naming one `source` and one listed left at graph 1; the
  desktop worker evaluating a driven graph against the library before its own artwork (a tiling reading
  the design's lettering by name fell back on every build); two GUI pins reading process-wide counters
  (failed in parallel); the phone re-arranging an opened file's graph; a mirrored row striking keels
  0.26 mm apart when its span crossed the head unevenly; a wall-clock DFM pin that failed 3 runs in 4
  under the guard (it counts points made now: 24 on first sight, none again); a bench stamp moving the
  cast stamps off the frame the pattern strikes them on; a band dangling off a graph's chain keeping its
  cut at graph 1; every stale format-6 doc line; and, found live, **an open's progress waking egui from
  the rayon pool** (a four-core phone deadlocked opening Caiman: the UI thread held the Context lock
  waiting on the pool for its tessellation while every worker waited on the lock in
  `request_repaint`) and **the first Undo of a graph edit doing nothing** (the evaluation's splice sat
  uncommitted until Undo committed and took back only it; `History::absorb` joins it to its edit).
- Verified: core 736 (golden 1), workbench 218 (232 with glow), gui 195, graph 106, graph-ui 26, mcp 45,
  cli 11, configurator 5, script 5, occt 9, solid 1, assets 4, phone 201, every suite under the 4 GB guard (the
  GUI on one thread); NDK arm64, wasm and the locked workspace clean with zero warnings. Live: desktop (`shots/b15-*.png`): Caiman chosen from the menu and cancelled while reading
  ("Stopped opening Caiman — armoured hide: the design is unchanged", Lorica still live); opened cold
  it lands 0.22 s after the click and shows built at 1.18 s (0.34 s to landing warm); an edit, undo and
  redo on Caiman each round-trip under 0.2 s, one History entry per graph edit once the integration's
  undo fix landed (before it, "Graph edited" then "Profile width 14 mm -> 15 mm", and the first Undo
  did nothing); the claw solitaire from CAD › Example projects shows its stone in the viewport, in the
  Report's Stones section (1 stone, 1.04 ct, Round 6.5 mm, edge clearance 0.53 mm) and on the exported
  stone map. Phone, x86_64 release on rdsmoke: opening Caiman deadlocked the app ("isn't responding";
  fixed, see above); after the fix it builds within about 4 s with Nocturne on screen until then.
- Closed: batch 14's open list (the per-build re-bake, the landed template evaluated twice, the
  sketch's Cut on a ring of parts alone, the CAD pane's Fit view, the plate over the CAD timeline, the
  phone's stamp window with the keypad up, the pocket-wall naming, the carved clone's STEP bead, the
  run-time relative worker paths).
- Open:
  - The phone's first build after a landing evaluates the template's graph again; it ignores the seed
    (`ringdesigner-android/src/ring.rs:518`).
  - The worker's cold detail measure runs on every core for 1.4–4.4 s after a landing and halves the
    UI thread's speed meanwhile (`ringdesign-gui/src/app.rs:1982`); `history::describe` serializes the
    whole design twice per commit, 31 ms on Caiman (`ringdesign-core/src/history.rs:274`).
  - Renders and turntables on the desktop, phone and configurator are metal only
    (`ringdesign-gui/src/export.rs:282`, `:302`; `ringdesigner-android/src/export.rs:274`, `:278`;
    `ringdesign-configurator/src/main.rs:322`); only the thumbnails, template shots and CLI go through
    `render::finished`.
  - CAD stones: the section view records every stone every frame (`panels/section.rs:330`; 92 ms a
    frame dragging a band under 24 heads); the analytic `Frames::built` is always true
    (`setstone.rs:721`); the stone map labels a CAD family "Round 6.5 mm 6.5" (`stonemap.rs:91`).
  - Stamps: a graph's `/stamps` patch and a standalone graph file carrying a tier or a shaped top are
    not fenced (`ringdesign-graph/src/file.rs:60`); the first shaped stamp moves the plain ones up to
    0.043 mm, by design.
  - Graph fence: two design chains behind a `flow.if`, an in-plane revolution through a wired operation
    pin, and a root-pointer `design.set` are read as one document (`ringdesign-core/src/parts.rs:677`).
  - Parts alone: a pocket's walls name the cut, so `Sel::Part` on the block misses them
    (`parts.rs:716`); `carved_apart` swallows a failed boolean and STEP writes the part uncarved
    (`parts.rs:819`); a partly buried seam still gets its whole bead (`parts.rs:224`); the primitive
    rest zone snaps back to 2 mm when a drag returns under 0.2 mm (`command/commands.rs:941`).
  - Skin: station-aware side-face gates (P5; `field.rs:443`); the draft rule is first order
    (`skin.rs:408`); `skin::hash` is a generic public name (`skin.rs:332`); `stock_spike`'s crease
    census reads sliver faces (`examples/stock_spike.rs:76`); the parting-stamp probe's −40° leaf
    fails to join ("two cuts cross inside a face").
  - Desktop saves are detached threads sharing one `.tmp` name, unjoined on exit
    (`ringdesign-gui/src/export.rs:472`); the shared bakes hold up to 48 M texels on the phone too
    (`alpha.rs:686`); the status line is rewritten every frame of an open (`app.rs:709`); the template
    plate stands over the ring view's info card.
  - Doctrine-level, unchanged: the axial web, and `LayerEntry::stage`.

### Batch 14 status — 2026-09-24 (on master: `cad-fixes`, `desktop-fit`, `occt-embedded`, `phone-eguimobile` merged, and their integration; not pushed)

- **A revolution's line is read in its sketch's plane** (`Operation::Revolve { in_plane }`), and a design
  carrying one is written at format 6 (graph, cluster and preset files at 2) so an older build refuses
  it by name instead of turning the region about a world line; every other file is byte for byte.
- **A cut carves what it reaches**: a Cut part is taken from the band and from every Separate part its
  box meets (a 2 × 1.5 pocket 0.8 mm into a 4 × 3 × 2 block set apart takes 2.4 mm³); a part it
  swallows is taken away with a note, and STEP writes a carved part as the build carved it (a bored
  post, joined or apart, 3.061 mm³). Scale, grips and press-pull keep a cut's sign (−1 scaled 2× is
  −2; a grip dragged through stops at −0.001; pushing past the floor is refused by name), and a
  sketch-made cut grips its depth — live, a 0.5 mm cut into the Court band dragged to 0.74 mm took
  the ring from 387.23 to 387.11 mm³.
- **Every STEP writer sizes the band** (`step::ring_sized`) and says it in one line (`Sized::summary`,
  solids counted by record): MCP's Court band with a post 1.5 MB (5,052 facets from 16,794), the CLI's
  claw solitaire 2.8 MB, the assembly package 2.8 MB reading back at −0.30%, the phone's Court band
  with a post 1.8 MB (5,922 facets from 655,360, 1.7–2.1 s).
- **Fit view frames what is chosen on the desktop** (a post at zoom 8.98, half-extent 1.99 mm, the orbit
  turning about it), named views and the cube go back to the ring's middle, the construction guide's
  rebuilds keep the reader's view, and dimension fields grow to their hint (the Height field read
  "1.0…" at 40 pt).
- **OpenCascade ships inside the desktop app**: `packaging/package.sh --occt` embeds the stripped
  worker (27.8 MB, deflated to 11.6 MB; the Linux app 93.2 → 104.9 MB stripped, Windows 95.6 → 159.2
  MB), unpacked under the data folder on first use (67 ms; a later start checks the digest in 12 ms).
  No build feature is needed; STEP import reads in the core first and hands OpenCascade only what it
  leaves; Tools > Licences carries the licences, the LGPL notices and the build's commit. Live: a
  fillet through the embedded worker, unpacked to `occt/d23e82e8…` on the spot.
- **The phone runs on EguiMobile 274baae**: number fields get the number keypad, shares queue one per
  frame and say where the copy landed, a dark splash, a seat pressed near a ring feature snaps as the
  desktop's click does, Fit view, plane names clear of the navigator, and the stamp window stands
  under the navigator, directly over the Tools rail and under any palette opened after it.
- **Templates open off the UI thread** (asked for while the batch ran): opening one ran the graph, a
  56 ms, 326 MB copy of the alpha library, a bake `instantiate` threw away, an unread field verdict
  and the same bake again in the app — 0.4–1.0 s here, seconds on the phone. `Template::open` runs it
  on a thread with a plate (reading, each node, baking, building the ring, Cancel) and lands the
  design with the library already baked; `AlphaLibrary` holds its entries behind `Arc`, so every
  clone and `Arc::make_mut` of it now copies pointers. Live, Zenith opened and built within a second
  on the desktop, and Nocturne in about 1.5 s on rdsmoke with the old ring on screen until it landed.
- The integration also fixed: the phone's STEP line counting solids by substring; a design opened
  after a guide change keeping the old view; the stamp window burying palettes opened after it (a
  sublayer of the rail now); a removal leaving `sdf_index` pointing at shifted entries; a refused
  cut also said to have found no seam; the dead `Worker::locate`; the licences window eating "Tools >
  Licences"'s ">"; and the OpenCascade plate ignoring the edge its own CAD canvas picked.
- Verified: core 681 (golden 1), workbench 211 (225 with glow), gui 174, graph 100, graph-ui 26, mcp
  45, cli 11, configurator 5, script 5, occt 7, solid 1, assets 4, phone 197; NDK arm64, wasm and the
  locked workspace clean with zero warnings; the desktop live-checked with the worker embedded; the
  phone's template plate on rdsmoke from the release APK.
- Closed: batch 13's list, and from older lists the phone's numeric keyboard and OpenCascade's
  shipping (bar the LGPL written offer and CI).
- Open: the worker re-bakes a driven design's artwork on every build (`eval::evaluate_design`: 413 ms
  a rebuild on Nocturne, 355 on Thalassa), and a landed template is evaluated a second time there;
  the sketch's Cut refuses a ring of parts alone although a cut now carves Separate parts
  (`sketch_mode.rs` `ALL_PARTS`); the CAD pane's Fit view reads the Ring viewport's selection, not
  its own history pick; a named view after a close fit keeps the zoom (deliberate, as on the phone)
  and so shows the bore close up; the template plate stands over the CAD timeline while it shows;
  OpenCascade on Windows and macOS unverified, the LGPL §6 offer and a CI build carrying the worker
  undecided; the phone's stamp window can fall below a landscape view with the keypad up, a DragValue
  may re-parse its rounded text on losing focus, and EguiMobile's Done leaves a field focused and
  fast keypad typing reorders; the reviewers' nits (a part's pocket walls name the part, a carved
  joined clone's STEP bead, run-time relative worker paths, the packed-refs commit watch).

### Batch 13 status — 2026-09-24 (on master: `m8-sketch-regions`, `desktop-files`, `m12-phone-view` merged and `a883233`; integrated locally, pushed with batch 14)

- **A sketch closes wherever its curves meet** (`sketch/graph.rs`): every entity is cut where another's
  end lies on it and the pieces left hanging off a loop are pruned, so a rectangle with one overhang
  trimmed extrudes to its area times the height; "open at point" is said only of a real free end.
  Where curves branch, `profile_regions` gives the faces they divide the plane into (a line across a
  rectangle makes two), `RegionRef::among` names one by a side only it runs along, and 200 held
  rectangles read their regions in 1.46 ms.
- **An extrusion runs either way off its plane**: `Operation::Extrude` takes a negative height with
  no schema change; a cut starts `CUT_CLEAR_MM` above its face, since a tool face lying in it left a
  skin over the mouth. The phone's Cut and the desktop's sketch solid step (Join, Cut, Separate; J
  cycles) cut from where the sketch was drawn: a 2 × 1.5 rectangle 1 mm into a block takes 3 mm³ on
  both apps, and a half-ring revolve cut leaves no skin (six faces before).
- **Big part files read off the UI thread in every build** (over 1 MB, at 13 ms a MB or less in
  release in every format; the 216 MB export-grid STEP held the UI 5.4 s before).
- **Export STEP collapses the band to 0.01 mm** (`step::ring_sized`, a checked half-edge collapse on a
  quarter-octave bucket queue — 6.23 M of 6.57 M heap pops were stale, 4.0 s to 1.1 s on a 655k-face
  band): at 1024 × 320 the claw solitaire 217.0 MB to 2.7 MB in 1.62 s against 3.12 s as built,
  Zenith 213.2 to 3.0 MB, the Braided band 215.5 to 7.4 MB.
- **A chosen stamp or seat station is kept by identity** across Undo, Redo, a history jump, MCP, a
  graph evaluation and an open (`viewport::made::follow`); one stone of a run hovers, lights and
  chooses alone; the stamp inspector opens apart from the tool inspector.
- **The phone's view follows the work**: a pinch keeps the metal under the fingers
  (`OrbitCamera::keep_under`; a 2× pinch carried the Block ~90 pt off them before) and the pivot moves
  onto it (2.3 ms over 649k triangles on rdsmoke); Fit view frames the chosen parts; box select
  filters parts, faces, edges or vertices; the band's long press makes a plane square to the band or
  on the parting plane; Box, Cylinder and Sphere are dragged out from the band as one History entry.
- Closed from batch 12: Trim's T-junction read as open, the cut sketch on a lowered plane, the
  blocking big import, the desktop's 220 MB STEP, the phone's zoom-pivot drift and whole-ring Fit
  view, the stale chosen-stamp index, the stamp and tool inspectors on one spot, the seat run
  lighting whole.
- Left for batch 14: a cut on a Separate part carved only the band (pinned, ignored); the desktop's
  Fit view framed the whole ring; MCP, the CLI and the phone still wrote STEP at the export grid; a
  revolution's line was read in world coordinates with no fence for older builds; Scale, grips and
  press-pull dropped a cut's sign; OpenCascade only under `kernel-occt`; the phone's numeric keyboard.

### Batch 12 status — 2026-09-24 (on master: `desktop-picks`, `m13-sweeps`, `m12-phone-tools` merged, integration `dc71b18`)

- **Every piece of metal answers the pointer as itself**: a made seat's solid and a struck stamp pick
  as `Entity::Seat` (by the stone's layer path) and `Entity::Stamp` (by index), claimed off the
  mesh's origin ranges (7,787 faces in 1.5 ms at preview, 8,424 in 5.1 ms at export; a design with
  neither skips it). A seat offers its layer, Live cuts and Show cutters; a stamp offers Edit
  stamp… (name, angle, across, turn, height, sink), Attach, Stage and Delete — each one History
  entry, the Delete key too — on both apps; placement commands land on the bare band behind them.
  Live on Zenith: the crescent reads "hovering stamp "Beside the hunter: crescent, cut at the bench""
  and offers its rows.
- **OpenCascade's STEP import runs off the UI thread**: a pending slot polled each frame (three
  frames ran through a 1.5 s read), the status line counting, and Cancel killing the worker
  (`Worker::run_cancellable`, `Failure::Cancelled`; a sleeping 30 s worker stops at once). A stored
  CAD desktop with the old empty docks gains the Report once (`Dock::catch_up`); a rolled-back
  document pours the ring it evaluates to (387.16 mm³: the bare band's 387.14 and a 0.027 mark,
  where the spacer and the bench post were poured with it).
- **The twisted sweep is our own** (`cad::twist::sweep`): closed at every twist from −720° to 720°,
  area × length to 0.5% (−0.046% for the 720° starter, −0.18% over ten turns), crossing-checked,
  its faces named, corners mitred; both Create menus offer it again, and a twisted post joins the
  Court band (+9.20 mm³, 29.7 ms at preview, 352 at export).
- **Our own exact STEP comes back without OpenCascade** (`step::read_meshes`, `solid_meshes`:
  plane, cylinder, cone, sphere and torus faces, units read): fourteen exact example parts back to
  +0.0000%, the claw solitaire's 7.9 MB file whole in 113 ms, a bought signet's B-spline solids
  named; the CLI's `cad step` prints volumes and `cad import` adds a part; MCP, the desktop and the
  phone import through it.
- **The phone's CAD tools reach the desktop's**: Trim, Offset, Chamfer and Mirror on the sketch bar;
  Finish offers Join, Cut (an offset frame 1 mm deep took 512.0 to 507.0 mm³, one Undo) and
  Separate; several parts isolated at once and taken out one by one; the bar folds above the
  keyboard; refused array copies drawn red; STEP in the Share menu on the preview grid (37.7 MB —
  the share bridge copies through one Java array and threw OutOfMemoryError at 217 MB, so a file
  over 128 MB is kept and said); Import part from the app's folders and Downloads up to 32 MB.
- Verified: core 661, workbench 191 (205 with glow), gui 145, graph 97, graph-ui 26, mcp 45, cli
  11, configurator 5, script 5, occt 3, solid 1, assets 4, phone 175; NDK arm64, wasm, `kernel-occt`
  and the locked workspace clean with zero warnings; the desktop live-checked on Zenith; the phone's
  tools on rdsmoke from the branch build.
- Closed: batch 11's seats and stamps picking as the band, the disabled twisted sweep, the blocking
  OpenCascade import, exact solids left out of the default import, the phone's missing sketch
  tools, Finish only joining, isolation of one part, the phone's silent refused copies, rolled-back
  outputs and the CAD desktop's stale docks.
- Open: Trim leaves a T-junction the whole-sketch profile reads as open (it chains endpoints; both
  apps); a cut sketch extrudes up from a lowered plane because Extrude refuses a negative height;
  the default build's big STEP import still blocks (3.6 s on 137 MB); the desktop's STEP at its
  export grid runs about 340 bytes a face (~220 MB on 655k faces); the phone has no system file
  picker, its revolve cut's pivot stays in world coordinates, a cut on a Separate part carves only
  the band, Fit view frames the whole ring and the zoom pivot drifts; egui-android's media-store
  copy reads a file whole, leaves its pending row on failure and cannot say it failed (EguiMobile);
  a chosen stamp's index goes stale across Undo; the stamp and tool inspectors open on the same
  spot; a seat run lights whole on hover; batch 11's numeric keyboard and OpenCascade decisions.

### Batch 11 status — 2026-09-23 (on master: `m13-exports`, `desktop-rest`, `m12-phone-sketch` merged, integration `58392c8`)

- **The ring is the casting** (`manufacturing::Casting::{Ring, Part}`): `prepare`, the mould study and
  the casting inspect refused every band-plus-part design ("Select one CAD component…", reproduced
  on the claw solitaire, a Join post, and a post beside a Separate spacer) because the Band anchor's
  id sits in `outputs` beside every part. The band with every Join and Cut part is now the default
  casting; a Separate part is poured only when chosen and otherwise named as not in the pattern.
  The Court band with a post pours +3.8989 mm³ against the post's 4.0212; the recipe's combo reads
  "The ring…" and lists only Separate parts beside it.
- **A ring leaves whole and a part comes in**: OBJ writes one named object per `threemf::objects`
  object; Export STEP… (`cad::step::ring`, off the UI thread), CLI `--formats step` and MCP
  `export_step`, with the band and builder parts in the assembly package's STEP too — read back
  exactly (band 604.0818 against 604.0818 built). File ▸ Import part… and MCP `import_part` take STL
  and OBJ (welded, refused unless watertight) and STEP (its faceted solids by `read_solids`; the
  OpenCascade worker reads any under `kernel-occt`) as an `Operation::Stored` part joined at the
  top: an STL post of 4.0148 mm³ grows the band by 4.0089. Shell on a part under `kernel-occt`.
- **A stored mesh is written once**: format 6 (still unreleased) keeps a top-level `stored_meshes`
  table by digest and a reference at each occurrence, so a driven design halves (15.44 to 7.72 MB at
  786k triangles; the save's own reload check costs 35 ms there, reopening 3.2 ms); a plain design
  still writes 5, byte-identical.
- **Nothing heavy on the desktop's UI thread**: the worker builds the ring frame for every design
  (19.4 to 0 ms at preview, 139.5 to 0 at export), the ghost's judge through that frame's tree (a
  seat bur's first cut read 24.5 to 3.8 ms, 151.5 to 4.3) and the selection tint (1.3 and 6.2 to 0).
  Part edges lives in `app.show_part_edges` alone, the session goes through the format ladder, and
  the CAD desktop docks the Report.
- **A head moves by its stone** (`commands::moved_by`): G, R, P and the gizmo on a builder part act on
  its stone's ring placement or `FaceSeat`, and S says the head is sized by its stone. Live: G 12 on
  the claw solitaire's head took stone and head 12° round as one "Place Round 6.5 mm" entry.
- **Face arrays and part marks tell the truth**: a face pattern's copy whose foot falls off its face
  is left out and named (4 copies over 72° on a 14 mm plate lose the 48° and 72° ones) and the
  desktop ghost shows it refused; a bench part's mark on a plate crossing the parting plane moves
  onto it (−59.8° over 0.361 mm² to 0.0000 mm²).
- **Pins travel with the file** (`RingDesign::pins`, core `pins::Pin`, no format step): the desktop
  carries a workspace's pins into a design on open; the lift leaves them out, and both apps and MCP
  carry them over every graph evaluation.
- **The phone sketches by touch** (`workbench::touch::sketch::Pad`): Sketch on a flat face or plane
  squares the camera over the ring, draws polylines, rectangles, circles, arcs and fillets with snaps
  and typed dimensions, and finishes as an Extrude (a drag or a typed height) or a Revolve, one
  History entry (a 2 × 1.5 × 0.8 box on a face adds 2.4 mm³). Work planes are made by touch (on a
  face with an offset, at an angle through the axis); a status line sits under the ring; Findings
  lists the verdict's parts; Isolate shows a part alone as its own closed solid; the pick scene
  builds beside staging (a Detailed build 914.5 to 832.5 ms on rdsmoke); pins live in the design and
  the worker builds the ghost's judge on settled builds.
- Verified: core 652, workbench 177 (191 with glow), gui 141, graph 97, graph-ui 26, mcp 45, cli 10,
  configurator 5, script 5, occt 2, solid 1, assets 4, phone 164; NDK arm64, wasm, `kernel-occt` and
  the locked workspace clean with zero warnings; the desktop live-checked (a session saved before the
  ladder restored intact, Export STEP… and Import part… in their menus, G on the claw head moving
  its stone); the phone's sketch, planes, status line and Findings on rdsmoke from the branch build.
- Closed: batch 10's list but the numeric keyboard and OpenCascade's decisions; from older lists
  the refused band-plus-part manufacturing, one-body OBJ, STEP from the app, part import, the ring
  frame and cut ghost on the UI thread (both apps), the phone's per-part Findings, Isolate and
  serial pick scene, pins outside the file, and the CAD desktop's empty docks.
- Open: the phone's numeric keyboard (EguiMobile's bridge asks only for text; that repo also serves
  the wirelab plugin); OpenCascade's LGPL and Windows shipping; its STEP import blocks the UI thread
  (2.6 s on a whole ring) and the default build's leaves exact solids out; seat solids and stamps
  pick as the band; the twisted sweep stays disabled (the kernel leaves open edges); joins and cuts
  into the band are not cached; a rolled-back document ignores `outputs`; the phone's sketch has no
  Trim, Offset, Chamfer or Mirror, Finish only joins, isolation holds one part and its array ghost
  does not show refused copies; a stored CAD desktop layout keeps its old empty docks until Restore
  default layout; the phone's menu avoidance no longer pans a close-up (a deliberate change).

### Batch 10 status — 2026-09-23 (on master: `m13-seats`, `m12-phone-rest`, `desktop-polish` merged, integration `59c1578`)

- **The sand pattern seats every part where the finished ring does** (`mesh::try_build_poured`,
  which `try_build_pattern` and `manufacturing::prepare` now build through): parts stand on the
  finished ring's band, never on their own raised marks. The claw solitaire's stone went from
  +0.259 mm and 23.65° off to 0.000 mm and 0.00°, with azures and shoulders from +0.378 mm and
  29.50°; nothing fails or is skipped, and every part's frame equals the finished ring's to 1e-6.
  The pattern build costs 11-34 ms more than the finished one at 256×128 (a second sweep).
- **Shoulders stage by where their stone sits** (`cutters::shoulder_stage_for`): Cast within
  `SHOULDER_PARTING_MM` (0.005 mm) of the parting plane, Bench off it — 0.0001 mm² on the plane,
  1.03 mm² 0.1 mm off, 8.04 mm² 0.8 mm off. Right-click a stone ▸ Cathedral shoulders reads the
  built stone's frame on both apps.
- **A stone on a part's face is edited on the face** (`commands::FaceHold`): G slides it along,
  across and off the face (typed values exact to 1e-9), R spins it, the face gizmo's arrows, ring and
  dial drive the seat, and the head built round it follows — no Transform, one undo step; P says why
  it is refused. A ring array of such a stone drops every copy back onto the plate (0.8045 mm over
  it, 0° lean at 12° and 24°, where copies had sunk 0.307 mm and leaned 24°), and the array ghost on
  both apps is the evaluation's own motions (`pattern::copy_motions`; it stood 1.19 mm off). A bench
  head on a plate leaves its locating dot on the plate's top (0.1262 mm³ against the frustum's
  0.1264), not in the band under it.
- **The phone measures, box-selects and shows work planes by touch**
  (`workbench::touch::{measure, boxes, planes}`): taps pair and a third chains to the corner (7.0-7.4
  mm between two band taps 40° apart), a window or crossing box with Replace, Add or Remove, a plane
  taken by its outline or name with a long-press menu (Mirror is one undo step, the copy lands at
  (x, −y, z) to 1e-3), and Work planes in its prefs. A 16×16 sweep of taps over the 3/4 view reads
  every tap on metal and names every miss.
- **The desktop lands a build in under half a millisecond**: the worker prepares the mesh buffer, the
  edges and the pick scene (beside the castability checks), so the UI thread only uploads — 34.5 ms
  to 0.12-0.40 ms at preview, 176 ms to 0.32-0.53 ms at export; dispatch to landed went 190 to
  157-167 ms at preview, and hidden section panes are no longer resliced. The CAD pane draws its
  parts' edges, Part edges and Work planes come back with the workspace, and `cad_edit::apply` goes
  through `workbench::touch::funnel::prepare`.
- **Format 6 only for a stored mesh** (`library::format_version_for`, `cad::stored::carried_by`): a
  design carrying `Operation::Stored` in its document or anywhere in its graph writes 6, and a reader
  stopping at 5 refuses it as saved by a newer RingDesigner; everything else still writes 5, so
  desktop 0.6.0 and phone 0.29.0 keep opening it.
- Verified: core 636 + golden, workbench 179, gui 130, graph 84 + 8 + 2 + 3, graph-ui 26, mcp 43,
  cli 3 + 5, occt 2, configurator 5, script 5, phone 160, assets 4, solid 1; wasm, NDK arm64 and the
  locked workspace clean with zero warnings; the desktop live-checked (a graph parameter's rebuild
  landed through the worker, 6.0 to 6.7 mm wide, posts clean on the wider band); the phone's three
  tools on the rdsmoke emulator from the branch build.
- Closed from earlier lists: face-stone G/R, its array and the bench mark under the plate (batch 8);
  the stone seated on the pattern's own marks, shoulders defaulting Bench, the CAD pane without
  edges, edge staging on the UI thread and the `format_version` step (batch 9).
- Open: a part's mark on a plate face off the parting line is never moved onto it; G on a head built
  round a stone still wraps a Transform; face arrays drop copies past the face's edge unchecked; the
  selection tint re-stages on the UI thread (4.5 ms at export); the Part edges switch lives in egui's
  data, mirrored into the workspace; a session design bypasses the version ladder; the phone has no
  sketching, work-plane creation, status line or numeric keyboard; a driven design still carries a
  stored mesh twice; OpenCascade's LGPL and Windows shipping remain Logan's call.

### Batch 9 status — 2026-09-23 (on master: `m12-shared-renderer`, `m13-cutters`, `m13-occt-spike` merged, and their integration)

- **M12's renderer half** (`workbench::render`): one GL renderer both apps draw through, its shaders
  written once with the header and precision chosen per context (GL 3.3 core, GLES 3.0), every mode
  kept — studio shading and the shade modes, the wall heatmap, the focus and select channels, the
  preview under a model matrix, the cutters ghost, the gems, the clip plane, wireframe. At an exact
  camera pose the only pixels that changed on either app are the new edges; the desktop's paint cost
  is unchanged (411-547 µs p50), the emulator's rose 12-19 µs (its GL calls are pipe round trips).
- **The crisp B-rep edge pass**: every kernel part's edges and every builder's creases drawn as
  screen-space quads (six vertices a segment, a width in points, no wide lines, which GLES ignores),
  lifted 3 px toward the eye and depth-tested, with the chosen and hovered edges in their own colours
  through the same pass and a faint half where metal hides them; a kernel part's seams are left out
  (`cad::seams`), and a Part edges switch sits on both apps (the phone's saved in its prefs). 3.3-4.9
  µs of GPU a frame for the claw solitaire's 1004 segments; staging 46-58 µs and upload 0.45-3.6 ms
  once per build.
- **M13's cutters**: `cutter.pierce` (round, oval, marquise, heart and drop; through or blind; a
  bright-cut chamfer) removes within 0.1% of its area times the wall it crosses; `cutter.azure` cuts
  4-8 windows under a stone clear of the claws, rails and pilot, refusing a stone too small by name;
  `shank.cathedral` raises two arches from the shoulders to the head's gallery rail and follows the
  head it meets. Under sand a side-face piercing casts clean (0.000 mm², Cast), a crown piercing
  locks 8.63 mm² at -88° and azures 59.9 mm² (both Bench), and shoulders default Bench (clean on the
  parting line, 8.04 mm² at -84° 0.8 mm off it). Right-click the band ▸ Cut here, a stone ▸ Azures
  or Cathedral shoulders, on both apps. A builder's head is one of its sources.
- **M13's OpenCascade spike**: go, as a child process (`occt-worker`, one JSON request in, one
  response out, a timeout and crash isolation) behind the off-by-default `kernel-occt`: filleting a
  part's edges matched to ours within 2e-15 mm (B-rep volumes exact), the torus-and-cylinder junction
  the pure kernel refuses (154 ms at preview, 2.5 s at export), shelling a head, and a bought signet's
  STEP (five closed solids in 1.8 s within 0.12% of the vendor STL). The result lives in the design
  as `Operation::Stored` — a packed mesh of about 9.8 bytes a triangle with its recipe and a digest
  for "Run again" — which every build renders and judges with no OpenCascade. **No-go for the
  Windows release as it stands**: the prebuilt OCCT links the dynamic CRT against our static one.
- Verified: core 630 + golden, workbench 168 (glow), gui 126, graph 84 + 8 + 2 + 3, graph-ui 26, mcp
  43, cli 3 + 5, occt 2, configurator 5, script 5, phone 152, assets 4; wasm, NDK arm64 and the
  locked workspace clean with zero warnings; the desktop live-checked (edges drawn, seams gone).
- Open: the sand pattern seats a stone on its own bench parts' raised marks (+0.259 mm and 23.65° on
  the claw solitaire), which is why shoulders default Bench; the CAD pane's view draws no edges; edge
  staging runs on the UI thread; OpenCascade's LGPL and one-file shipping are Logan's call (it stays a
  separate worker), the Windows CRT needs OCCT built from source with the static CRT or a separate
  worker, and a Stored feature needs a `format_version` step and paired releases (0.6.0 and 0.29.0
  refuse it); a driven design carries a stored mesh twice.

### Batch 8 status — 2026-09-23 (on master: `m12-phone-cad`, `batch7-fixes`, `m13-first-cut` merged, and their integration)

- **M12's interaction half, the phone models by touch** (`workbench::touch::{funnel, gesture, hit,
  parts}`, the phone's `cad`): a tap chooses a part and a second tap on the spot walks down to its
  faces and edges, the choice tints the metal (attribute 5), a long press opens `context_items` as a
  thumb-high popup (what the phone lacks greyed with its reason), the timeline strip sits under the
  ring with its chip menu, one finger drags a gizmo handle through the desktop's commands with the
  ghost under a model matrix, typed values go through the dimension bar and the soft keyboard, a lift
  commits one History entry, and every edit leaves through one funnel (`touch::prepare`, the steps of
  `cad_edit::apply`). Verified on the s26ultra emulator; two fingers are host-tested only (adb cannot
  inject them). A stone seats on a part's face where the finger pressed.
- **Batch 7's open items closed**: every builder's reach is floored by the bore with `MIN_WALL_MM`
  kept (`builders::build_in` + `Bore`), and a setting that cannot keep it is refused by name ("Base
  rail would reach 0.61 mm into the finger hole") — the old four-claw builder broke the wall in 31 of
  56 cases on the default band, the worst 0.80 mm in, and 27 of 56 on the Court band, 1.26 mm; the
  array ghost re-drops like the evaluation (0.00000 mm against the old rigid 2.486 on a heart's
  shoulder); work planes are drawn, named, picked on screen and carry Sketch on this plane and Mirror
  the chosen part across it; array (A) and press-pull (Q) are on the rail and the palette; Sweep,
  Twist and Loft take one region; a Cut part's ghost counts only the faces inside the band (a 1 mm
  pilot 2.299 mm² of walls against the built ring's 2.299, where every face read 3.140).
- **M13's first cut**: size runs re-seat CAD parts on each size's band and judge each with its
  build (the claw solitaire 5 to 9 by halves: 9 sizes in 7.3 s, all watertight); 3MF carries the band
  with its joined and cut parts as one object and each Separate part as its own (the app, the phone,
  the engine and the CLI); a stone sits on a part's planar face (`FaceSeat` in the stone builder's
  params: moving the plate 25° carries it within 0.00055 mm, a plate 1 mm thicker lifts it 0.500);
  `cad step --band` writes the band as a closed faceted solid beside the analytic parts (FreeCAD
  1.1.3 reads the claw solitaire as two valid solids).
- **Integration**: the claw fix reached the app only once `build_made` called `build_in` with the
  bore (a claw ghost test that sank its stone flush at 60° on the thin default band is now refused,
  so it seats the stone at the gestures' stand-off); the ghost forgives a facet across the plane
  only as a chord (crease-aware corner normals, the verdict's `chord_lean`); the desktop gizmo
  measures its reach about the part as built (the phone's fix); the three Nocturne tests compare with
  the source as curated (`templates::refine_sources`) and the graph crate is green.
- Verified: core 613 + golden, workbench 151, gui 122, graph 84 + 8 + 2 + 3, graph-ui 26, mcp 43,
  cli 3 + 5, configurator 5, script 5, phone 151, assets 4; wasm, NDK arm64 and the locked workspace
  check clean with zero warnings; the desktop app smoke-run on a restored claw-set design.
- **Released 2026-09-23**: desktop 0.6.0 on GitHub (`desktop-v0.6.0`: Windows MSVC, both Macs and
  Linux, each with its checksum; the GUI suite green on all four runners) and Android 0.29.0 on the
  app store. The first tag found two bugs in never-run CI steps: the release's version check needed
  Python 3.11's `tomllib` (the Ubuntu 22.04 runner has 3.10), and the Windows job's static-CRT check
  looked for `Hostx64/x64` in a backslashed path.
- Open: G/R on a face stone wraps it in a Transform (it should edit the seat's u/v/spin); a bench
  head standing on a plate leaves its mark under the plate; a ring array of a face stone re-drops
  onto the band; the assembly STEP and OBJ are still one body; the work-plane switch is not saved; a
  cut ghost builds its band tree on the UI thread (about 20 ms at preview); the phone needs a numeric
  keyboard (EguiMobile's bridge asks only for text) and has no sketching, box select, Measure or
  drawn work planes yet; the phone's pick scene adds about 144 ms to a Detailed build.

### On master while batch 8 ran — 2026-09-22/23 (`6f7b097` through `309c42d`)

- **The dimension bar stays in its view** and the pointer under it still drives the command: the
  bar holds inside the viewport's rect (the caption's own rule), the Ring viewport and sketch mode
  read the pointer under it while the bar follows only the viewport's hover, so it holds still for
  a click. A move carried under the bar lands there (pinned; without the read it stayed at 90°).
- **A flat wall leaning back across the parting plane locks** (`castability::judge::chord_lean`):
  a spanning facet is forgiven as chord only when its corner normals off the plane face their own
  mould half. A 2 mm block turned 3° locks 2.39 mm² against 3.18 leaning back; the old rule forgave
  0.81 of it. A seam bead's 0.0121 mm² of chord is still forgiven. The ghost keeps the old rule
  until batch 8's `ghost.rs` lands.
- **The verdict is 20x faster**: every section rebuilt the design's reference loop and field
  context (0.5 ms); they are built once and the sections fan out through rayon, bit-identical
  (pinned over 24 angles). The Court band at 192x128: 110 → 5.2 ms; the settled pass's hot-spot
  scan 35 → 1.0 ms; the parting-line export 305 → 4.4 ms.
- **A bench part's mark reads its own patch**: the rows the dot reaches, sections built once for
  both passes. 94 → 13-15 ms a mark, and relief elsewhere round the section no longer blames the
  dot (a boss 2.3 mm down the dome read -63° into a dot standing clean on the crest).
- **Polish**: the tool inspector opens against the viewport's right edge (it stood over the ring in
  small windows); a pattern shows no gizmo of its own and G/R/S/P on it say "follows its source:
  move \"Cylinder\"" (a drag would have wrapped it in a Transform); a driven design's strip shows a
  funnel edit before the rebuild (pinned — the funnel's carried document closed batch 5's one-build
  lag). A Sketch feature needs no cache slot: laying it is a validation and a plane, and its regions
  are solved inside the features that read it, which the body cache keys by signature.

### M2 detail

**Measured 2026-09-21 (`examples/join_probe.rs`)**: a traced kernel cylinder dropped onto the built
surface and joined through `csg::combine` — Court band, Heart signet, Braided band, Cathedral stock:

| build | union bezel | subtract pilot | closure |
| --- | --- | --- | --- |
| preview 256×128 (65k faces) | 1.1–5.0 ms | 2–46 ms | open 0, repeated 0, every case |
| export 1024×384 (786k faces) | 6.6–51 ms | 10–331 ms | open 0, repeated 0, every case |

Volumes are exact (+70.6 mm³ for π·3²·2.5). The same union through `brep::combine` never
returned. `csg::clean(2e-5)` removed 0–626 slivers per operation. The bet holds; M2 is engineering.

- `cad.rs:733`: `tessellate_solid(body, chord) -> (csg::Solid f64, tri_face, edges, vertices)`
  (keeps `BodyMesh.triangle_faces`, which `tessellate` drops today).
- `csg.rs`: `Solid::check()` on the existing `Grid`, `strip_zero_area`, `Snag::Cancelled` polled
  in the pair loop (`:454-528`), `combine_traced` (parents known at `:543-576`), cluster tools by
  box overlap before one band op, 0.05 mm default sink + coplanar/tangent contact test matrix.
- `mesh.rs`: remove the `:349` fork; parts resolve after stamps in `resolve_solids`; `Mesh.origin`
  spans extended so picks name the feature; Live-cuts covers parts and auto-suspends mid-stroke.
- Stop auto-inserting `Operation::Band`; `Band`/`TwistedRing` evaluate to `Value::Mesh`.
- Fix `cad::combined` double-counting overlap volume (grams/cost JSON).
- Fixtures include a factory signet base (`bases/signets/*.ringbase.json`), not only plain bands.

## Reuse (do not rebuild)

`setting.rs:932 apply` (spans, heads-then-cuts, notes) · `csg::Frame`/`Solid::placed` (`csg.rs:73,91`)
· `stones::surface_frame` (`stones.rs:465`, make `pub`) · focus channel + `stage_focus`
(`viewport.rs:267`) · `hover.rs` throttle · `visual.rs Tool` + `canvas.rs Contact` ·
`focus::Turn` · `picking.rs:175` ray maths · `History` (`history.rs:142`) · unused kernel API:
`presspull_face/region`, `planar_face_at_point/profile`, `extrude_region`, `slice_by_plane`,
`loft_with_options`, `SweepPath::Nurbs3`, `geom2d::{trim_spans, fillet_between_rays, snap}` ·
cadkernel pin bump to ≥`c741a69` for its new 2D constraint solver (evaluate against `sketch.rs:352`).

## Stays impossible (said out loud)

B-rep fillet on a fused junction in pure Rust (seam bead instead) · analytic STEP of mesh-domain
results · general shell/offset without OCCT · three-edge corner blends in the bead v1 · CAD on wasm
threads.

## Verification

- Tests under the guard: `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --offline`.
  New pins: torus ∪ cylinder < 1 s watertight; 128×96 band ∪ cylinder finishes, cancel observed
  < 100 ms; provenance after join names the feature; BVH == brute force over 1k rays; each
  `CadEdit` gives the same result through document and graph appliers on the five examples; v4→v5
  migration (document and graph form); Escape-ladder and Enter-in-polyline kittests;
  `every_command_has_icon_palette_and_menu`.
- Real GUI: `EGUI_INSPECTION=1 cargo run --features eframe/inspection --bin ringdesigner` driven
  through `egui-mcp` (`tools/egui_drive.py`): replay the H1-H9
  script each milestone — bezel on band fused in seconds, Esc ×2 keeps work, right-click menus,
  edge multi-pick → Fillet.
- Phone: `cargo test -p ringdesigner_android`, `cargo ndk -t arm64-v8a check`, ARTEMIS pass at M5 and M12.
- wasm: `cargo check --no-default-features --target wasm32-unknown-unknown` for core + configurator.

## Workflow

Branch per GitHub issue, one epic per milestone, PR closes it (existing convention). Start: M0.
