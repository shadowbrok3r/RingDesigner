# Roadmap

The trackable plan. A task is ticked in the commit that ships it; each milestone is a GitHub
milestone and each task group an issue (`gh issue list --milestone "<name>"`). Effort tags:
S < 1 day · M a few days · L 1–2 weeks · XL multi-week. Verification for every milestone:
`cargo build --workspace`, then `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test
--offline` (this machine has no swap — never run tests unguarded).

## Where this is going

The design is one height field over a swept band, guaranteed to pull from a two-part sand
mould, and `castability::analyze_field` is the verdict (`CLAUDE.md`). What the project lacks is
what every parametric jewellery tool has: a dataflow runtime with list semantics, reusable
clusters with parameter panels, script nodes for the last 10%, and a second geometry kernel for
free-form work. This roadmap adds them as layers *above* the castable model — the sand-cast ring
stays its own mode with the verdict as its gate — and tracks the finishing work on the lofted
signet head first.

Fixed decisions: Grasshopper-style **implicit lists** (a node fed N items runs N times,
longest-list matching); **Manifold mesh CSG** first for the free mode; **rhai in-app** plus a
**PyO3 module** of the core (Python is never embedded); desktop first, phone later. A design
carries its graph (`RingDesign.graph`, live until baked — the `GroupLayer::recipe` pattern one
level up); standalone graph/cluster/preset files exist for libraries; the graph is truth and the
node editor is a view; code says `Graph`.

Nothing derived from proprietary assets enters the repository (`assets/`, `tools/harvest/`,
decoded material stay gitignored); only our own graphs are committed.

## M0 — Today: finish the signet loft, and seed the tracking

Order: M0.1 → M0.2 → M0.3 → M0.4 → M0.0.

- [x] **M0.1 Harvest tools into the repo, gitignored** (S, #1). `tools/harvest/` + `tools/venv/`
  (recreated), `requirements.txt`, `README.md`; references in CLAUDE.md, core and examples
  updated; no scratchpad paths left in the tree.
- [x] **M0.2 Custom-outline asymmetry** (S + S-M, #2). Imported plans can read fuller on one side at
  their ends — the source curve's own tilt, not the raycast (measured symmetric to 1e-4 on a
  symmetric superellipse). Add opt-in `CustomOutline::symmetrize(across_band, along_ring)`
  (mirror-average the polar table), and importer parity in `from_points` (arc-length densify +
  circular smoothing, as the exporter already does). Never raise `OUTLINE_STEPS` (serialized).
  Tests: a symmetric superellipse reads `|lo+hi| < 1e-3` at x ∈ {0.9, 0.95, 0.98, 1.0};
  `symmetrize` removes a drawn tilt; the existing outline tests stay green.
- [x] **M0.3 Smooth-table apex loft** (L, #3). The reference construction's domed table: the loft
  starts at an apex point `h` above the table centre and passes a 0.6-scaled outline at that
  height before the outline at the plane; the flat table and its rim row are gone. Knob: reuse
  `table_dome_mm` (cab on prism/dome heads, apex height on lofted heads; no serde change).
  `Tent` generalized to N rows; `Tent::at` takes the ridge path at every angle under a cap with a
  depth law continuous at the rim; GUI slider range 0–3 mm; MCP gains `head_table_dome_mm`;
  `cg_signet` maps the preset flags. Test: crest at the centre = plane + cap, crest height
  non-increasing off the centre, field verdict clean, thinnest wall > 1 mm. Accept: the two
  domed factory presets rebuild within 0.15 mm mean at the head; the flat one is unchanged.
  Brief 01 (2026-08-26): the factory loft is a loose `CreateFromLoft` = a clamped cubic blend of
  the row curves on **uniform** knots; ours moved from chord-length knots (0.053 mm mean off the
  001 surface) to uniform (0.034; 005 head vs cached mesh 0.088 → 0.045).
- [x] **M0.4 Loft as the default for new signets** (S + render review, #4). `apply_signet` sets
  `loft = 1.0` (every "new signet" path funnels through it); `SignetHead::lofted()` for extra
  heads; `Default`/serde default untouched so existing files keep the prism. MCP `set_shank`
  applies it when switching to Signet; the cut dome takes precedence over the loft
  (`SignetHead::mix`), so `suggest_dome` keeps its authority for lobed plans; templates
  and configurator bases go lofted unless their verdict test says otherwise; examples that are
  *about* the prism or the cut dome pin `loft = 0.0`. Test: a new signet is lofted and fields
  clean; a head JSON without `loft` still deserializes to 0.
- [x] **M0.0 Tracking seed** (S, #5). This file; GitHub milestones M0–M9 and labels; one issue per
  task group for M0–M2, one epic per later milestone.

## M1 — Graph runtime: `ringdesign-graph` (L)

A core-only crate that evaluates a graph to a `RingDesign` with implicit-list semantics.

- [x] **M1.1 Skeleton** (S, #6). Workspace member, `parallel` forwarded to core, wasm check passes,
  doctrine section in CLAUDE.md.
- [x] **M1.2 Value** (M, #7). Closed `Value` enum with `Arc` domain handles (Design, Profile, Shank,
  Head, Outline, Gem, Window, Remap, Layer, Entry, Stack, Recipe, AlphaSource/AlphaRef, Build,
  Field, Stones, Mesh, Solid, Path, Json) + serde `Literal`; `ValueKind::accepts` + `coerce`, the
  coercion table pinned by a test.
- [x] **M1.3 Graph** (M, #8). Stable `NodeId(u64)`, `Node { kind, params, inputs, pos }`, `Wire`,
  `Exposed`, `Mode { SandRing, Free }`; `add/remove/connect/set_param/set_input/validate/topo`.
- [x] **M1.4 Registry** (M, #9). `PinSpec`/`NodeSpec`/`Category`/`Registry`, instance-resolved pins
  for script and cluster nodes.
- [x] **M1.5 Evaluator** (L, #10). Longest-list matching with last-item repeat; per-item `Null` with
  attributed errors; nested lists pass whole; caps on list items, nodes and cluster depth;
  recipe-signature cache so one edit re-runs one chain; `Targets`; side-effect nodes only on
  demand; `evaluate_design` returns the design **with** its field report.
- [x] **M1.6 Source + util nodes** (S, #11). Numbers, series/range, math, basic list ops.
- Accept: `[1,2,3]+[10] → [11,12,13]`; empty in → empty out; a failed item does not abort its
  siblings; cache hits after one edit equal the chain length; cycles and fan-in refused; an
  oversize series clamps and warns.

## M2 — The model as nodes, sinks, modes, files, template graphs (XL)

- [x] **M2.1 `struct_node!`** (M, #12). Existing serde structs become nodes with one line per pin;
  enum pins by serde name; a coverage test so a new core field cannot be forgotten.
- [x] **M2.2 Band/shank/head/outline nodes** (M, #13).
- [x] **M2.3 Layer nodes** (M, #14). One per `Layer` variant, the fitters, the `entry` wrapper
  (window/blend/opacity/soft/mask/remap), windows and remaps.
- [x] **M2.4 Assembly, generators, alphas** (M, #15). Stack and assemble; pavé/halo/channel nodes
  emitting live groups (the evaluator never regenerates); procedural/text/SVG/drawn alphas.
- [x] **M2.5 Sinks, verdict gate, modes** (M, #16). Output, verdict, gate, build, refine, stones, DFM,
  sheet, exports, render, save; SandRing refuses `NotCastable` on file-writing sinks; Free adds
  solid nodes and the mesh verifier.
- [x] **M2.6 Files, ladder, clusters, presets** (M, #17). `*.graph.json`, `*.cluster.json`,
  `*.preset.json` with a migration ladder like the design's; cluster nodes with exposed
  parameters; presets as parameter sets; user-dir resolution.
- [x] **M2.7 Design field, template graphs, golden tests** (M, #18). `RingDesign.graph`; every
  template re-expressed as a committed graph whose evaluation equals the code template
  byte-for-byte and fields castable; `Graph::from_design` round-trips every template.

## M3 — The editor: `ringdesign-graph-ui` and desktop integration (L) — epic #19

- [x] **M3.1 Vendoring** (S, #39). The egui-0.36 `egui-snarl` + `egui-scale` under `patches/` with
  root `[patch.crates-io]` entries; exactly one egui in the tree; a diff guard against the sibling
  copy.
- [x] **M3.2 Editor core** (L, #40). Snarl payload `NodeCard`, `build_snarl`/`extract_graph` (truth
  = the graph), pin widgets by value kind, type-checked wiring, category palette, node menu,
  diagnostics on the node frame.
- [x] **M3.3 Desktop integration** (M, #41). `PaneKind::Graph` + a node-inspector dock tool; graph
  state on the app with a sync rule against `design.graph`; evaluation on the build worker;
  history labels; palette commands.
- [x] **M3.4 Simple ↔ graph bridge** (M, #42). Convert to graph, live banner with panels disabled
  while driven, Bake, "Edit in graph".

## M4 — Scripting, MCP and CLI (L) — epic #20

- [x] **M4.1 `ringdesign-script` (rhai)** (L, #47). Sandboxed engine with size/operation caps,
  `Value ⇄ Dynamic`, a small math/list/domain API, Expr pins evaluated per item, Script nodes with
  declared pins, diagnostics in the editor.
- [x] **M4.2 MCP `graph_*` tools** (M, #48). New/load/save/describe/list nodes/add/remove/connect/
  set/expose/clusters/presets/evaluate/from_design, plus graph resources.
- [x] **M4.3 CLI crate** (S, #49). `ringdesign graph eval|check|describe`; the binary moves to its own
  crate.

## M5 — Idioms, clusters, polish (M) — epic #21

- [x] **M5.1 List idiom nodes** (M, #53). Weave, entwine, cull, partition, gate, polar array (integer
  lattice — never a relaxation), format, json, if.
  Brief 02 (Rhino 8.34, 2026-08-26) re-pinned: negative Series count → empty + warning; Partition
  size 0 → empty chunk. Polar Array's partial-arc law and the Weave/Cull/List Item cases came back
  unusable (panels read inputs) — re-run list in `SharedVM/answers/followup-brief02.txt`.
- [x] **M5.2 First hosted cluster** (M, #54). The signet construction as a user-dir cluster whose
  preset evaluates to the lofted head.
- [x] **M5.3 Editor polish** (M, #55). Live value badges, subgraph navigation and collapse, arrange,
  fit, minimap.

## M6 — Python: `ringdesign-py` (M) — epic #22

- [x] **M6.1 Crate** (S, #59). `cdylib` via pyo3 0.29 + maturin, abi3; workspace build stays green
  without Python headers.
- [x] **M6.2 API** (M, #60). Numpy-free `Design`/`Build`/`Library`/`Graph` wrappers with JSON-pointer
  get/set as the escape hatch; builds release the GIL.
- [x] **M6.3 Tests + notes** (S, #61). pytest smoke; the deviation/crease probes become module-backed
  scripts.

## M7 — Free mode: `ringdesign-solid` (Manifold) and the mandrel merge (XL) — epic #23

- [x] **M7.1 Kernel crate** (L, #65). Manifold behind `kernel-manifold` (off by default; the default
  build has no C++); `Solid`, frames, tubes, `Parts{add,cut}`, mesh conversion + validation.
- [x] **M7.2 Free-mode nodes** (L, #66). Primitives, extrude/revolve, from-design, booleans, frames on
  the ring, tubes, mesh import, the mesh verifier, mesh export.
- [x] **M7.3 mandrel merge** (M, #67). The vine semi-mount as a cluster; carat calibration
  reconciled; its MCP retired in favour of `graph_*`.

## M8 — Phone and web parity (L, later) — epic #24

- [x] **M8.1 Android graph tab** (M, #73). Touch mechanics (drag classification, long-press menus,
  lock); graphs arrive by the design sync and evaluate locally. Shipped in the EguiMobile
  workspace as ringdesigner-android 0.9.0.
- [x] **M8.2a Export worker and a durable copy** (S, #75). Exports on their own threads, never
  dropped behind a preview build; the design file copied to Downloads through MediaStore.
- [ ] **M8.2b Intent filters and the phone-shaped port** (M). Opening a design file from another
  app needs incoming-intent plumbing in the shared egui-android crate (Java + JNI, none exists)
  and a device to test; the layer stack editor, section view and report stay desktop-only for now.
- [x] **M8.3 Web configurator** (M, #71). `build-a-ring` on wasm; `compose::Config` reused verbatim.

## M9 — Backlog (parking lot) — epic #25

- [x] A first-class set-stone record shared by report, preview, seats and generators (M, #77):
  `setstone::SetStone`, enumerated once, read by the census, the preview and the section view.
- [x] Graduated tilted stones on a run (S–M, gated on a field check, #79): `SeatRunLayer::tilt_deg`,
  clean on the crest and on a side face.
- [x] Bezel realism following the girdle (S–M, #81): the collar's height from the stone's crown, a
  bearing ledge with a dished floor, the section view's girdle at the stand-off.
- [x] The bypass read as a `ShankKind` (M, #83): two explicit arms with rounded tips, the section
  their union, the crest on the parting plane, Split's side-face seam.
- [x] Gallery/hollow underside (M–L, #85): `SignetHead::hollow_mm`, a scoop from the finger hole
  into the head's belly carried as `ShankMod::bore_lift_mm`; the bore is a vertical wall at any radius.
- [x] Alpha granulometry (#90): `Alpha::min_feature_px` and `dfm::findings_in` — the measured
  finest stroke and gap of a texture at the layer's cell scale, against the sand floor.
- [x] Stone spacing map SVG (#92): `stonemap::write_stone_map_svg` — plan and unrolled chart at 2:1
  with every stone to scale and the census's tight gaps; File menu, MCP, CLI `stonemap`, phone Share.
- Shelf: march instances along a guide path; blue-noise scatter generator; mm-true mask
  morphology; nesting-depth stepped relief; honest rope rail; `stones::probe_seat`; pearl stock;
  flat-edged seat plans.
- [x] The graph editor in the comfyui-android look (#87): `ringdesign_graph_ui::style`, shared by the
  desktop pane and the phone tab.
- Niceties: gate & sprue advisor; DPI true-scale; reference-mesh import + deviation report as a
  core feature (the exact point-to-triangle measure and the crease census, today script-only);
  comparison views beyond the ghost.

**The parking lot is closed.** The 2026-08-30 audit gave every Shelf and Nicety item a milestone,
so nothing sits here unowned any more: march instances along a guide path and the blue-noise
scatter generator to **M16**, along with mm-true mask morphology, nesting-depth stepped relief and
the honest rope rail; `stones::probe_seat`, pearl stock and flat-edged seat plans to **M20**; the
gate & sprue advisor to **M19**; DPI true-scale to **M13**; reference-mesh import and the deviation
report to **M24**; comparison views beyond the ghost to **M17**.

---

# The second half: M10 — M24

Everything above is shipped. What follows comes from the **2026-08-30 audit**
(`docs/AUDIT-2026-08-30.md`): thirteen domain auditors reading the tree from a bench jeweller's
and a foundry hand's chair, each finding checked by an adversarial verifier, then two independent
sequencing passes. 243 findings, none dropped as already-existing. Findings are cited as `#n`
against that document, which carries the file and symbol every claim was read from.

## Where this is going, restated

M0–M9 built a model that can prove a ring pulls from a two-part sand mould. The audit's verdict on
it is that the *pull* is world-class and everything *after* the pull is missing: nothing says where
to gate, how much to melt, what polish eats, or whether the ring survives a week on a hand. Three
structural holes account for most of the rest:

- **The model has one stage where the trade has two.** Every layer is cast geometry judged against
  the sand's 0.30–0.40 mm detail floor, so bright-cut, wriggle, guilloché, intaglio seals and
  inside lettering — the fine work a signet shop actually sells — are judged NotCastable or
  measured as mush (F93). One concept, `LayerEntry::stage`, unlocks four families at once.
- **The verdict is radial and blind to the axial web.** `thinnest_wall` and `bore_span_wall` both
  subtract bore radius from surface radius at one `z`. Nothing measures metal *across* the band,
  which is exactly where the doctrine sends every deep carve — and `OpenworkLayer`'s own cap is
  documented as opening up there. Four dimensions found this independently (F115).
- **The app states numbers that are not true.** `CastProcess::apply` never restores the sand floors
  (F21); stone crowding is judged against a hardcoded 0.3 mm while the sand's floor is 0.7–0.8
  (F58); the report panel prints the retired mesh analyzer's numbers under the field verdict's
  banner (F123). And there is no CI (F155).

So the order is: **truth, then measure, then variety.** M10 stops the lying and stands up CI. M11
gives the verdict the section it cannot currently see. M12 is the cheapest visible variety win —
19 findings, no new subsystem — and only then do the multipliers land.

Fixed decisions, as before: the sand-cast ring stays its own mode with `analyze_field` as the gate;
lost wax gets honest immediately (M10, M18) and complete later (M22); nothing derived from
proprietary assets enters the repository.

## Dependency order

```
M10 ──> everything
 ├─> M11 ──> M14 ──> M15     (a bench cut, an inside cut and a pierce may not
 │      ├──> M20              remove metal before the web that stops them is measured)
 │      ├──> M22
 │      └──> M19
 ├─> M12 ──> M14              (a seal needs a plan to sit in)
 │      ├──> M16
 │      └──> M17              (a grid over parameters that do not exist is
 ├─> M13                       one ring shown nine times)
 └─> M18 ──> M19 ──> M23
M20 + M19 + M21 ──> M23
M24 last: it exports what the earlier milestones made true
```

Two things are deliberately late despite being `critical`. The **gating plan** (F1) sits in M19,
not M10: every input exists, but they are calibrated against three unsourced sand constants (F7),
so a gate advisor shipped now manufactures a confident wrong number at the flask — source the sand
first. The **variant explorer** (F168) sits in M17, after M12 and M16 have given it axes worth
enumerating.

## M10 — One verdict, spoken once (L, 47 findings, epic #154)

Accept: CI is green; a golden corpus pins verdict, `volume_mm3` and thinnest wall for every
template; Sand→LostWax→Sand restores `min_section_mm` 0.7 / `min_detail_mm` 0.35 exactly; and no
sink — GUI export, `--sizes`, the kiosk's `save_order` — writes a file the field verdict refuses.

- [ ] **M10.1 The process switch tells the truth** (M, #169). F21≡F114, F22, F23, F25, F5.
  `CastProcess::apply` gains its else arm; `DraftSettings` gains `sand: SandProcess` (which today
  derives no serde at all); the field banner stops telling a lost-wax design it "clears a two-part
  pull" while its own note says the opposite; `analyze` stops speaking sand out loud in Free mode;
  `min_detail_mm` either gates or the tooltip stops promising it does; the drag downgrade to
  Marginal explains itself. Test: a round-trip identity on the floors.
- [ ] **M10.2 One verdict everywhere** (S, #170). F123≡F166, F127. The report panel, the status chip and
  the Draft shading stop reading the retired mesh analyzer; the three headline castability
  guarantees are re-pinned on `analyze_field`, which the doctrine already says is the verdict.
- [ ] **M10.3 Numbers that are not true** (M, #171). F58, F83, F91, F105, F109, F110, F143, F144, F147,
  F151. Crowding and run bridges against `min_section_mm` at all five call sites — `spec.rs` and
  `stonemap.rs` included, or the sheet and the setter's map disagree; the transposed `Flutes`
  footprint, which lets reeding evade the arc-scale correction entirely; `edge_mm` discarding
  contrast and bias; the flat square-edged shank every new lofted signet now gets, which makes a
  shipped M0.4 accept criterion false; tiling DFM ignoring the shank modulation; `RingSize::display`
  printing "US 7."; a shrink-scaled export labelled with the nominal size; an inside diameter that
  is measured rather than echoed; the SDF re-baked when `edge_mm` changes.
- [ ] **M10.4 Stop losing work** (M, #172). F152, F153, F154, F163, F164, F165, F167, F203. Embedded
  alphas unpacked on session restore and not only on File▸Open; undo re-bakes drawn, text and SVG
  alphas, so painted metal stops surviving Ctrl+Z; an atomic design write with one backup; alpha
  name collisions between two designs in one session; the build worker restarts after a panic
  instead of leaving the app read-only; a cap on what a design file may rasterize at load — the
  67 GB lesson, applied to the other door; `FORMAT_VERSION` unfrozen; profile and outline library
  files get a version.
- [ ] **M10.5 Nothing non-finite leaves the app** (S–M, #173). F149, F150, F156, F196, F197. The
  `is_finite` guard `mesh::build` lacks and its three sibling consumers already have — one line,
  and it ships before the refactor, not behind it; then the four displacement copies become one
  function with refine pinned against the sweep; `whole_faces` requires finite vertices, which
  fixes STL, OBJ and PLY at once and brings them in line with 3MF and GLB; `--steps` stops being
  silently ignored; GUI export is gated on the verdict and on `hit_cap`/`saturated_leaves`.
- [ ] **M10.6 CI and the golden corpus** (M, #174). F155, F128, F139, F223. GitHub Actions running the
  workspace build, the wasm check and the test suite under the cgroup memory guard — this machine
  has no swap and the guard is not optional; a golden corpus pinning verdict, volume and thinnest
  wall for every template and showcase design, so a silent geometry regression fails something;
  the size run re-checks stones and DFM per size instead of gating on the field verdict alone; the
  kiosk refuses what the graph sinks refuse.
- [ ] **M10.7 Hygiene and the hot path** (M, #175). F17≡F30, F19, F81, F157, F158, F159, F160, F161,
  F162, F172. The two disagreeing hard wall floors; the build's wall clamp divorced from the
  fill floor; `Alpha::make_seamless` is dead code and CLAUDE.md calls it the import mechanism;
  stop cloning the whole design per layer inside undercut attribution; resolve each layer's alpha once per build
  instead of once per sample; cut `outline.rs` out of `field.rs` and `signet.rs` out of
  `profile.rs`; `sizing.rs` gets tests and a range guard — it is the only core module with none;
  CLAUDE.md's head-draft numbers contradict the code and themselves; track `tools/harvest/`, which
  is the evidence for the doctrine and is gitignored; the default band is a HalfRound the doctrine
  itself says "honestly has no side".

## M11 — The band has a thickness across it, too (L, 20 findings, epic #155)

Accept: an axial web is reported per slice beside `thinnest_wall_mm`; two opposing 0.9 mm
side-face carves on a 2.0 mm band report a 0.2 mm web and fail; the openwork cap on a side face is
bounded by that measure instead of by `keep_mm`.

- [ ] **M11.1 The axial web** (L, #176). F115≡F4≡F226≡F47. `min_local_thickness_mm` and its (theta, v) on
  `FieldReport`, computed in `analyze_field`'s per-slice loop from the closed `Section` polygon it
  already builds — the minimum over surface-sample pairs whose connecting segment stays inside the
  loop — gated on `min_section_mm` alongside `thinnest_wall_mm`. `stones::check_seat` already reads
  across the band on a side face; generalize that idea rather than inventing one. Then give
  `OpenworkLayer` and `FlutesLayer` a real cap on side faces, where `depth_mm` is currently the only
  limit and a carve can drive the low face past the high one unnoticed.
- [ ] **M11.2 Measure what was built, not the bare profile** (M, #177). F66, F72. The seat report samples
  `profile.sample_mod` with no layers evaluated, so a seat over a carve over-credits its metal; a
  bezel's collar wall and a pad's prong post are never checked against the sand's detail floor.
- [ ] **M11.3 Strength is not fill** (M, #178). F118≡F136, F130, F140, F141. Every wall number answers
  "will the sand fill this?" and none answers "will this survive a week on a hand?" — a 6 × 0.8 mm
  sterling band fields 0.000% and bends out of round in a month. `modulus_scan` already walks every
  section polygon; add the second moment in the same loop, reduce to an equivalent radial thickness
  over the palm arc, and report it against a per-alloy floor. Never gate — a wire ring is
  legitimate. Plus wearability (a proud prong catches), balance (a heavy head spins on the finger),
  and the wear floor under a culet, which is currently `MIN_WALL_MM` — a mould constant doing a
  second job.
- [ ] **M11.4 Section ratio, and relief that stands on a wall** (M, #179). F3, F11. A thin shank feeding a
  heavy head shrinks porous at the junction and nothing flags the progression; nothing tells a user
  their relief walls are vertical, and the fix that exists is wired to one layer type.
- [ ] **M11.5 Undercut, said usefully** (M, #180). F120≡F178, F121, F122. Attribution names the culprit
  layer and stops at "muting it clears it" — but muting is not a fix, and the largest relief that
  still clears is a monotone one-dimensional bisection over ten field passes. The in-flight
  `dfm::fit_to_floor` is the precedent, and its own test is named *the solver is the checker read
  backwards*. Also: severity by depth and angle rather than an area fraction, so a sand-tearing
  gouge and a burnishing drag stop reading alike; cluster in `v` as well as theta; locate drag.
- [ ] **M11.6 Corners and fins** (M, #181). F126, F10≡F116. A sharp internal corner in the metal is a
  sharp external edge in the sand: it erodes under the pour and is a stress riser in wear, and the
  doctrine asks for a radius repeatedly without measuring one. And DFM measures how *wide* a feature
  is and never how *deep*, so the fin aspect ratio — the thing that decides whether a stroke washes
  out of the mould, and whether polish will eat it — cannot be computed.

## M12 — Draw the plan, notch the band (M, 19 findings, epic #156)

The cheapest visible variety win: no new subsystem, nearly all S/M, and it widens the shop window
on three axes at once. Accept: a head plan is drawn or imported without touching the graph and
saved to the outline library; a `ShankKey` axial offset produces a notched band; "14 × 12 head on a
3 mm shank" is typed in millimetres and printed on the sheet; a milgrain row holds its inset from
the band edge to within 0.05 mm across a size 5→9 run.

- [ ] **M12.1 A head plan without the graph** (M, #182). F100≡F175≡F187, N7. `CustomOutline::from_svg` and
  `from_alpha` beside the existing `from_points`, so the recentre/raycast/rolling-ball containment
  guarantee and `fair_r_for(hull_defect(..))` are inherited on every path rather than re-proved; the
  missing `library::save_outline` wrapper over the dead `save_outline_in`; an Import/Draw button
  beside the Face combo; MCP `import_outline`; a CLI verb; and a reference-image underlay with
  two-point millimetre calibration, which is how a sketch becomes a plan in the first place. Fix
  CLAUDE.md in the same change — it already claims these imports exist.
- [ ] **M12.2 The axial keyframe** (M, #183). F40, F45. `ShankKey::z_offset_frac`, carried as a fourth
  Catmull-Rom channel through `ShankStyle::modulation` into `ShankMod::z_center_frac` under Wave's
  measured 0.6-of-half-width cap. `width_scale` plus the offset place the two band edges
  independently, which is the contoured / notched / chevron wedding band — the largest bridal
  category after the engagement ring, off one serde field. Ship two presets. And rebuild
  `ShankKind::Cathedral`, which today is a shoulder swell and nothing else, as a crest rise plus a
  bore lift plus a shoulder arc — `SignetHead::hollow_mm` already casts exactly that void through
  `ShankMod::bore_lift_mm` with the field clean.
- [ ] **M12.3 The groove leaves Split** (S–M, #184). F48, F51. `ShankMod::side_groove_mm` is consumed with
  its own cap and only `Split` and `Bypass` ever set it. Promote it to the band with a position
  along the wall, a width and a count: the grooved, stepped and double-rail men's band family,
  already proved to field 0.000% because a groove's floor faces along the pull and its walls stand
  radial. Inlay channels come with it as a concept — a material, a fill volume, and the honest note
  that the retention undercut is a bench cut.
- [ ] **M12.4 Placed relative, not absolute** (M, #185). F49, F113, F148, N10. `MilgrainLayer::v_mm` and
  `BorderLayer::v_mm` are absolute millimetres in the reference chart, so a beaded edge cannot
  follow the band's edge; shoulder windows are hand-typed degrees that go stale the moment the head
  moves; angular windows drift against millimetre-placed ornament across a size run or a matched
  pair. And one boolean over the chart's `u` mirrors a design for the left hand.
- [ ] **M12.5 Say it the way a jeweller says it** (S, #186). F102, F103, F104. A signet is "14 × 12 on a
  3 mm shank" and the app cannot express it — the head's extent across the band *is*
  `profile.width_mm`. Add millimetre authoring, the trade head-size table to suggest from (8×6
  child, 10×8 / 11×9 / 12×10 ladies, 13×11 / 14×12 / 16×14 gents), and state the head on the sheet,
  which today prints everything about the band and nothing about the ring.
- [ ] **M12.6 Head dressing** (M, #187). F99, F101, F112. Milgrain, rope and bright-cut borders around the
  head's rim, following the outline — this is *cast* relief needing only M12.4's edge-relative
  placement, and must not be bundled with the bench-stage seal work in M14 or it waits three
  milestones for nothing. Plus head rotation in plan, and the stepped, bevelled and bordered table
  treatments the family stops short of.
- [ ] **M12.7 Shapes as assets** (S, #188). F55≡F237, F57. `ShankKind::Keyframes` is the most expressive
  shank in the model — the cloud ring is one — and an authored band shape can be neither saved,
  shared, nor built from the graph. And a Claddagh, a class ring and a crest ring have every piece
  they need and no path and no example.

## M13 — The first ten minutes (M, 10 findings, epic #157)

Accept: a new install opens on a visual gallery, at least three shipped templates carry stones,
every user library has one browser, Ctrl+S saves the file it opened, and the phone has design-level
undo. Owns F56, F171, F173, F174, F176, F181, F184, F185, F239, F240, plus **N1** (nothing is ever
shown at actual size — no life-size lock, no DPI calibration, no scale reference in any render).
The load-bearing ones: there is no Help and nowhere the sand doctrine is explained once (F171); the
user owns seven kinds of library and only alphas have a browser (F176); a fresh machine gets 28
procedural patterns, 9 templates, 2 clusters and 2 presets, and the shippable libraries cannot grow
(F240).

## M14 — Cast is a stage; bench is the next one (L, 12 findings, epic #158)

The single largest multiplier in the audit, and it must land after M11: a bench cut removes metal,
and the wall it must leave is the axial one. Accept: `LayerEntry::stage` is `Cast | Bench`,
serde-defaulted to `Cast` so no file migrates; a Bench entry is skipped by `analyze_field` and
`dfm::findings` but drawn in the viewport and the unrolled editor; a mirrored intaglio on a 12 mm
head renders its own wax negative; the engraver's sheet prints artwork mirrored at 1:1 with a depth
column, built the way `stonemap.rs` builds the setter's.

Owns F93≡F77 (the stage itself, first), F94 (intaglio: mirrored, recessed, wax-releasing — `grep -i
intaglio` across all crates returns nothing today, and `Alpha::flipped` already exists and is
tested, so the mirror is a bake-time flip rather than a sampling change), F95 (the wax-impression
preview), F96 (the engraver's sheet), F97 (monogram layout and engraver's alphabets), F98 (a crest
library), F86 (curve wires have one constant width — no swell, no graver-angle V), F79 (surface
finish is not modelled: the only `Finish` in the codebase is a viewport RGB triple, and a bright-cut
or matte face is applied after the pour, so it is a bench attribute), F89 (paint mode has no guides
and a subtly elliptical brush), F90 (two bundled fonts, no user font), F108 (the brush refuses the
table and can only raise).

## M15 — The inside of the ring is a surface (L, 6 findings, epic #159)

The largest duplicate cluster in the audit — five dimensions independently found that nothing can
exist on the inside of the ring (F41≡F78≡F111≡F220≡F224). Accept: `v` runs through the bore; a
0.15 mm inside inscription on a 1.6 mm band leaves a web the verdict reports; the bore still reads
as vertical wall throughout, because a bore surface's normal is radial at any radius; and both bore
edges carry a ≥0.2 mm break by default (F135, **N9** — the reference signet has them, and a sharp
arris at the bore is the commonest complaint about a shop-made band).

The honest part: almost nothing on the bore is castable. The sand column in the finger hole is one
rigid piece pulled along Z, so any pocket or ridge locks it. So the inside surface is built as
**bench** geometry (M14's stage), not as a new cast region — an inside inscription is cut or lasered
after casting anyway. Plus **N5**, the punch check: you cannot strike a hallmark into a 1.0 mm shank
without bellying the ring, you need a flat land about 2.5 mm wide, and the app knows the wall
thickness at every angle and never says it. No competitor makes that check.

## M16 — Patterns get knobs and a seed (L, 14 findings, epic #160)

Accept: every one of the 28 builtins exposes its defining parameter plus a seed; the same seed
rebuilds byte-identical geometry; `every_pattern_tiles_seamlessly` sweeps the parameter ranges;
mask booleans and mm-true morphology exist; and a rope rail measures a real over/under stroke
≥ `min_detail_mm` at 2.7 mm cells.

Owns F82 (`draft_limited` is a destructive one-shot bake rather than a layer property), F75 (all
28 generators have their defining parameter hardcoded — Rope is always 3 cords and 6
twists, Starburst always 24 rays, and the noise patterns call `hash01`/`fbm` with literal seeds, so
there is exactly one Hammered texture in the world), F76 (wire `dfm::fit_to_floor` — the in-flight
work — so the app can *fix* the mush finding it already prints), F80 (the graph has six alpha
sources and zero operators), F82, F84, F85≡F232 (`BorderProfile::Rope` is a fake rope and its own
comment says so), F87, F88, F177 (the tiling editor never shows the cell size in millimetres, which
is the one number the sand cares about), F233, F234 (the cluster publish loop is dead code — nothing
can publish a cluster or a preset), F235 (no node anywhere emits a `Path`, though four pins take
one), F236 (no seeded randomness anywhere in the product).

## M17 — Many combinations, one grid (L, 7 findings, epic #161)

The literal answer to "many combinations", built after M12 and M16 have given it axes. Accept: a
variant grid renders with stones in the chosen metal, each cell carrying its own field verdict
encoded by shape as well as hue, with the size ladder available as an axis.

Owns F168 (the app's whole comparison story is one manually-pinned translucent ghost;
`examples/gallery.rs` already builds N designs, renders each and judges each — lift that loop into
a pane rather than writing it twice), F183 (the size run is a variant axis: do not build the batch
runner twice), F169 (hero renders and turntables leave the stones out, though `render.rs` was
deliberately given a `Part` list because "a finished piece is metal *and* stones"), F170 (the
casting sheet has no picture of the ring), F182 (the verdict is carried by colour alone, in red and
green — a grid of thumbnails is exactly where hue-only encoding fails), F214 (the renderer cannot
produce a product listing image), plus **N1**'s scale reference.

## M18 — The sand is a recipe, not three numbers (L, 17 findings, epic #162)

Accept: `SandProcess` is a serde field on `DraftSettings` with a cited source per number;
`pattern_scale` differs by process; the design carries its intended metal; and the casting sheet's
subtitle is derived rather than hardcoded "sand-cast pattern".

Owns F6, F7 (the whole sand is three scalars asserted without a measurement, in a codebase that
cites every other constant), F8 and **N8** (`metal.rs` models weight and shrink, not casting
behaviour — and the fill floor is a property of the *pair* (sand, alloy, pour temperature): bronze
fills sections sterling will not), F9≡F117 (no finishing stock: the verdict judges as-designed while
the shop polishes 0.05–0.1 mm off, which is exactly the height of a milgrain bead), F12 (no venting
or blind-pocket reasoning), F15≡F26 (the sheet is a sand document on every lost-wax print), F20 (say
plainly what LostWax changes, because today it is four lines), F24 + F129 + F217 (the process is
unreachable from the desktop's collapsed panel, from MCP, and from the kiosk — ship all three
together or the field gets three treatments), F27 and **N3** (the sand patternmaker's allowance
applied to a wax is about half a ring size of error, and a repeat-production master needs the whole
ladder: master → rubber → wax → investment → metal, compounded), F31, F50, F192≡F209 (the design
cannot say what metal it is, so the sheet cannot name one).

## M19 — Gate it, sprue it, ram it up (XL, 13 findings, epic #163)

The foundry half, after M18 has sourced the sand it would otherwise calibrate against. Accept: the
app names a sprue diameter, a gate angle and a riser from the modulus scan; states pour weight in
grams *and* dwt including sprue and button against finished weight; exports a pattern with a stub
and a parting register beddable to a stated depth; and records an actual pour against its predicted
verdict.

Owns F1≡F124≡F242 (the gating plan — `modulus_scan` computes the whole solidification curve 64
times per build and survives as one grey label; gate a signet at the shank and the head it already
knows freezes last comes out spongy under the engraving), F2 + F18 (pour weight and yield: the sheet
explicitly disclaims sprue, button and finishing loss and then computes none of them), F13≡F201 (the
export is a ring, not a pattern: no sprue stub, no parting register, no handling boss, no print
orientation), F14, F16, F28, F29, F119 (the parting surface is one flat `z` though `parting_line`
already computes the contoured one), F221 (**the moat**: nothing records an actual pour against a
predicted verdict — the claim is entirely internal), plus **N2**, the stock list and stretch-out:
cut length is `π(ID + t)` and the app knows both exactly at every angle, and it is the most-used
calculation at a bench.

## M20 — A stone is stock with a species (XL, 22 findings, epic #164)

Accept: a stone has a species with its own density and hardness; calibrated stock is quoted L×W;
a pear's seat follows its own girdle instead of a symmetric superellipse; a half-eternity is
authored as an arc of eleven rather than a full ring of twenty-two cropped; and the setter's sheet
gives a bur size per seat.

Owns F59≡F193 (there is no material anywhere — no grep hit for ruby, sapphire, Mohs or hardness —
so a 6 mm sapphire cabochon reports 14% light, a CZ 62% light, and the report cannot say the one
thing that destroys stones: the default seat style is a gypsy mound, and flush-setting an emerald is
how you lose a client's stone), F42 and F43 (a windowed run fades its end stones' *stock* instead of
ending the row, and every row is solved over the full 360°), F44≡F230 (no pearl — a half-drilled
pearl on a cup and peg is one of the commonest jobs at a bench and the model refuses it), F52≡F63 (a
channel-set ring reports no stones at all), F60, F61, F64≡F243, F65, F67 and F229 (the setter is
handed no bearing angle, bur size, seat depth or proud height), F68≡F231, F70, F71, F73 (**the
refusal registry** — the honest no, generalised from the two places the app already does it well),
F74, F211, plus **F62 bar setting, which the finding itself says is provably castable in a two-part
mould — build it rather than refuse it.**

## M21 — A size is a fit, not a number (M, 11 findings, epic #165)

Accept: the app accepts US, UK, EU or a measured millimetre; compensates a wide comfort-fit band by
the trade's convention; re-solves every generator on a size change; prints a calibrated sizer at
1:1; and takes a bangle at 60–70 mm.

Owns F132 (the bore is the stated size, full stop — and the two terms pull in *opposite* directions:
a wide standard-fit band runs tight so the cut bore goes up, while a comfort-fit dome rides easier
so it goes down. The configurator makes it concrete by giving a 14 mm signet the same bore as a 4 mm
court band), F133≡F142≡F228, F134 (comfort fit is a constant rise, so its feel changes with band
width), F137 (nothing reasons about whether a design can be resized — a full eternity cannot), F138
(a size change never re-solves stone spacing or live generators, across a 20% circumference change
from size 5 to 9), F145≡F215, F146, F227 (**bangles are an `S` hiding in a `high|M`** — the sweep
already works at 60–70 mm; it is a range check on `RingSize`. Do not merge this with M22's torus
work or a one-day win is buried under a multi-week one).

Land this after M10.6: wide-band compensation silently changes the bore of every existing design,
and without the golden corpus the regression is invisible.

## M22 — More than one body (XL, 19 findings, epic #166)

Accept: the shipped default build reaches Free mode with no C++ toolchain; a Free-mode body gets a
manufacturing verdict before any sink writes; the kernel sweeps a section along a path; and a mask
pierces a band clean through with the web measured either side.

Owns F33 (**Free mode is unreachable from the shipped application** — `ringdesign-gui` declares
`default = []` and nothing in any UI ever sets `Mode::Free`; a whole documented, tested mode ships
dark, which is the worst of the three available answers), F34, F35 (a Free-mode ring gets no
verdict at all — `judge()` returns `Ok(None)` and every file sink writes whatever it is handed),
F36 (twelve primitives and one hand-ported vine), F37≡F205 (the mesh importer welds on exact float
bits, so it rejects most real-world STLs), F38, F39, F32≡F92≡F225 (piercing — stated precisely: a
hole whose axis is ±Z casts, a hole through the band's thickness is a horizontal core and does not),
F46 (assembly: spinner, puzzle, ring guard, stacking set, matched pair), F53 (**open, adjustable and
cuff rings are not a casting problem** — a C-shaped band with a domed section pulls fine; it is that
`mesh::build` hard-wires a closed torus), F54≡F69 (free-standing prong heads, baskets, peg heads,
six-claw, tension — all need a void under the girdle that no single ±Z plane clears; these belong to
lost wax, said out loud through M20's refusal registry), F106 (head and shank as different stock —
the missing fourth signet construction), F107, F238 (the kernel has no loft and no sweep, the one
primitive every reference body is built from).

## M23 — The shop floor: quote it, order it, catalogue it (XL, 15 findings, epic #167)

Accept: a quote states metal, stones, labour and yield; two orders from the same customer never
collide; a kiosk order reaches the shop; and every catalogue entry is a record with an id, not a
Rust literal.

Owns F206 (an order overwrites the last one from the same customer — the slug is the name, the
filenames are constants), F207 (the web configurator hands the customer a zip and stops: no
submission, no contact, no price, no conversion event), F208 (an order records unstable ordinals, so
an old order decodes to a different ring), F210≡F125 (price is `per_gram × casting_weight`
everywhere; a real quote is metal at *pour* weight + loss + stones + casting + cleanup + setting per
stone + polishing + plating + margin, and every input already exists), F212 (there is no catalogue —
five hand-written collections in Rust, none of them a product with a SKU, a variant axis or a
price), F179, F180, F198, F199, F213, F216, F218 (no production planning: nothing knows what pours
together), F219 (no pattern inventory — a sand shop's real capital is its stock of rigid patterns),
F222 (the sheet is not a record: no date, no identity, no digest), F131, plus **N4**: the trade
quotes in pennyweight and spot is per troy ounce, and there is no melt/alloy calculator for the
commonest torch-side job there is.

## M24 — File doors (L, 11 findings, epic #168)

Last, because it exports what the earlier milestones made true. Accept: STEP comes out of the swept
grid; a 1:1 plan, section and unrolled band as DXF; MCP works over HTTP with no server-side paths;
and the deviation probe is a core measurement rather than a gitignored script.

Owns F186 (**STEP** — every 3D writer in the tree is a triangle writer, and a mesh ring arrives at a
casting bureau or CNC house as something they cannot fillet, section or resize. Our surface is a
regular closed lattice in both directions, so one `B_SPLINE_SURFACE_WITH_KNOTS` closed in u and v
needs no trimming curves, no seam edges and no caps — hand-rolled ISO 10303-21 the way `threemf.rs`
hand-rolls its zip), F188, F189 (MCP is missing most layer kinds and every generator), F190≡F241
(reference-mesh import and deviation are gitignored Python, though they are the measurement that
settled the whole lofted-head port), F191 (no 1:1 drawings of the ring at all), F194, F195, F200
(Rhino `.3dm` is in this repo as data and as a Python dependency and not at all as a capability),
F202, F204, plus **N6**: no document declares which way is up. One arrow — "top toward fingertip",
seam at `TOP_DEG` — on every printed sheet.
