# RingDesigner CAD: from a feature list to a viewport you can model in

## Context

Logan asked for the next moves on the CAD section ("very limited"), pointed at OpenCADStudio as
the Rust CAD reference, asked me to drive the real GUI to find the limits, and added mid-session
that there is "just about no way to drive the cad from the 3d viewport, like blender or fusion
360, or rhino" — tools should be mouse / right-click / snap driven.

Decisions Logan made this session:

| Question | Answer |
| --- | --- |
| Kernel | Pure-Rust default; optional OpenCascade behind an off-by-default desktop feature |
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
10. **OCCT is late, optional, desktop-only, child-process** (`ringdesign-occt` behind `kernel-occt`,
    cadrum): fillet/shell/general sweep/STEP import; results cached as meshes in the design file
    so default, phone and wasm builds still render and judge them.

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
- Open: the band surface for the ring frame is built on the UI thread; a Sketch feature has no
  cache slot; which regions a `Profile::Feature` extrudes is a schema decision
  (`regions: Option<Vec<usize>>`); a sketch strip on a driven design lags its graph by one build.
- In flight: M11's core verdict for parts; batch 6 — M6 (gizmo, ring dial, click-drag primitives,
  shared grips), M8's UI (sketching in the Ring viewport) and M10 (gem-driven builders and the
  three-gesture solitaire), on hooks pre-cut in `5f1f6c4`.

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
