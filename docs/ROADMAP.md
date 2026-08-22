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
- [ ] **M3.3 Desktop integration** (M, #41). `PaneKind::Graph` + a node-inspector dock tool; graph
  state on the app with a sync rule against `design.graph`; evaluation on the build worker;
  history labels; palette commands.
- [ ] **M3.4 Simple ↔ graph bridge** (M, #42). Convert to graph, live banner with panels disabled
  while driven, Bake, "Edit in graph".

## M4 — Scripting, MCP and CLI (L) — epic #20

- [ ] **M4.1 `ringdesign-script` (rhai)** (L). Sandboxed engine with size/operation caps,
  `Value ⇄ Dynamic`, a small math/list/domain API, Expr pins evaluated per item, Script nodes with
  declared pins, diagnostics in the editor.
- [ ] **M4.2 MCP `graph_*` tools** (M). New/load/save/describe/list nodes/add/remove/connect/
  set/expose/clusters/presets/evaluate/from_design, plus graph resources.
- [ ] **M4.3 CLI crate** (S). `ringdesign graph eval|check|describe`; the binary moves to its own
  crate.

## M5 — Idioms, clusters, polish (M) — epic #21

- [ ] **M5.1 List idiom nodes** (M). Weave, entwine, cull, partition, gate, polar array (integer
  lattice — never a relaxation), format, json, if.
- [ ] **M5.2 First hosted cluster** (M). The signet construction as a user-dir cluster whose
  preset evaluates to the lofted head.
- [ ] **M5.3 Editor polish** (M). Live value badges, subgraph navigation and collapse, arrange,
  fit, minimap.

## M6 — Python: `ringdesign-py` (M) — epic #22

- [ ] **M6.1 Crate** (S). `cdylib` via pyo3 0.29 + maturin, abi3; workspace build stays green
  without Python headers.
- [ ] **M6.2 API** (M). Numpy-free `Design`/`Build`/`Library`/`Graph` wrappers with JSON-pointer
  get/set as the escape hatch; builds release the GIL.
- [ ] **M6.3 Tests + notes** (S). pytest smoke; the deviation/crease probes become module-backed
  scripts.

## M7 — Free mode: `ringdesign-solid` (Manifold) and the mandrel merge (XL) — epic #23

- [ ] **M7.1 Kernel crate** (L). Manifold behind `kernel-manifold` (off by default; the default
  build has no C++); `Solid`, frames, tubes, `Parts{add,cut}`, mesh conversion + validation.
- [ ] **M7.2 Free-mode nodes** (L). Primitives, extrude/revolve, from-design, booleans, frames on
  the ring, tubes, mesh import, the mesh verifier, mesh export.
- [ ] **M7.3 mandrel merge** (M). The vine semi-mount as a cluster; carat calibration
  reconciled; its MCP retired in favour of `graph_*`.

## M8 — Phone and web parity (L, later) — epic #24

- [ ] **M8.1 Android graph tab** (M). Touch mechanics (drag classification, long-press menus,
  lock); graphs arrive by the design sync and evaluate locally.
- [ ] **M8.2 Mobile leftovers** (M). Intent filters, durable mirror, export worker, the full
  phone-shaped port.
- [ ] **M8.3 Web configurator** (M). `build-a-ring` on wasm; `compose::Config` reused verbatim.

## M9 — Backlog (parking lot) — epic #25

- A first-class set-stone record shared by report, preview, seats and generators (M); graduated
  tilted stones on a run (S–M, gated on a field check); bezel realism following the girdle (S–M);
  gallery/hollow underside (M–L); the bypass read as a `ShankKind` (M).
- Shelf: stone spacing map SVG; march instances along a guide path; blue-noise scatter
  generator; mm-true mask morphology; alpha granulometry; nesting-depth stepped relief; honest
  rope rail; `stones::probe_seat`; pearl stock; flat-edged seat plans.
- Niceties: gate & sprue advisor; DPI true-scale; reference-mesh import + deviation report as a
  core feature (the exact point-to-triangle measure and the crease census, today script-only);
  comparison views beyond the ghost.
