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
| M5 | **Command session**: G/R/S in ring frame, axis locks, typed values, Place on ring, Join/Cut, grid/vertex/midpoint snaps, tool rail | 3 wk | Model by mouse + hotkeys with exact numbers |
| M6 | **Ring-frame gizmo**, ring dial, click-drag primitives, shared grips | 2 wk | Slide a head round the shank, spin, tilt |
| M7 | **Ring-aware snaps** (theta, side face, parting plane, stone stations, castability tint), work planes, bench pin, measure | 2 wk | Snap to what matters on a ring |
| M8 | **Sketch in 3D context**: on-face/on-plane, ring underlay, pan, delete, trim/offset/corner fillet, multi-loop regions, in-canvas dimensions | 2-3 wk | Draw a profile where it lives |
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

### M1 spikes (retire before writing refs into saved files)

| Risk | Spike (1-2 days each) |
| --- | --- |
| R1 graph-hosted CAD + snapshot cost | Prototype parts on a plain design beside paint; measure `History` cost with `Arc<Value>` graph |
| R2 uncancellable kernel calls | Time curve of every `Operation` and `brep::combine` 100-4000 faces; pick the gate; decide child-process eval on desktop |
| R3 refs retarget silently | Parameter sweep per `Operation`; per-edge signature (ordinal, dir, adjacent surface kinds, normalized ends) |
| R4 seam bead folds | Cylinder-on-torus and 0.8 mm wire-on-dome through `csg` + variable-section sweep; `self_crossings == 0` over 200 placements |
| R5 anchors ignore relief | 200 anchors on Caiman/Zenith and a 0.8 mm boss; choose mesh-drop placement (as stamps do) |

### M2 detail

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
