# RingDesigner implementation plan

Started 2026-09-09. This is the working checklist requested by the user; update it as implementation and verification finish. The older `docs/ROADMAP.md` remains a reference. A checked implementation item means usable code exists; validation and workshop trials have their own checkboxes.

## Intended outcome

A jeweler can design a ring, choose how each part will be made, inspect and correct manufacturing problems, prepare a sand mold or another manufacturing route, and export identifiable patterns, components, and bench instructions. The existing procedural band, ornament library, and graph remain useful throughout.

The first deliverable is the complete desktop workflow: **choose recipe → inspect both mold halves → select a release problem → preview and apply a correction → export a compensated pattern and molding sheet**. Then implement the broader sketch, feature, solid, component, resizing, and interchange workflows.

## Current evidence

- [x] Inspect RingDesigner core, desktop, graph, solid kernel, CLI, mobile casting helpers, and existing audits.
- [x] Inspect OpenCADStudio snapping, working planes, command interaction, solid history, and STEP export.
- [x] Check the user-provided TensorRS and Burn sources.
- [x] Identify existing user edits; preserve the modified/untracked example files.
- [x] Record baseline compilation and relevant tests.

Existing capabilities: parametric swept bands, ornament layers, cross-sections, direct unrolled handles, measurement pins, undo, graph evaluation, draft attribution, presets, stone reports, shrink scaling, and printable reports. Free solids are implemented behind an optional Manifold feature. The current analytic release checker assumes ±Z; its bore exemption and global percentage thresholds do not establish a complete withdrawal sequence. Its wall measurement is radial. New manufacturing checks must communicate their sampling limits.

## Architecture and invariants

- [x] Persist manufacturing setup in the design with backward-compatible defaults.
- [x] Use one shared manufacturing implementation from desktop and CLI; keep geometry and reports independent of egui.
- [x] Keep nominal dimensions, pattern compensation, as-cast stock, and finishing operations distinct.
- [x] Keep display orientation separate from the manufacturing pull direction.
- [x] Identify the exact geometry and setup used by every report and export.
- [x] Never show an old report as current while a rebuild is pending.
- [x] Analyze invalid or empty geometry as unassessed/invalid, never successfully released.
- [x] Report confirmed geometric obstruction separately from draft risk and numerical uncertainty.
- [x] Treat the bore/core strategy explicitly; do not silently exempt arbitrary internal geometry.
- [x] Preserve graph provenance and exposed parameters when editing a graph-driven design.
- [x] Give every new tool a manual test/inspection path and a clear cancel or undo action.
- [x] Keep expensive analysis and exports off the UI thread.

## M1 — Complete sand-casting workflow

### M1.1 Manufacturing setup and recipes

- [x] Add a serializable manufacturing setup: process, recipe name, alloy, shrink, draft/detail/section floors, finishing stock, flask, pull frame, and bore strategy.
- [x] Provide Delft clay and Petrobond starting recipes with editable values and an explicit distinction between defaults and measured shop results.
- [x] Preserve existing designs when the setup is absent.
- [x] Provide recipe selection and numerical editing in an accessible casting workspace.
- [x] Save/load a custom recipe independently of a design.
- [x] Validate finite values, physical ranges, flask dimensions, and normalized pull directions.

### M1.2 Release analysis

- [x] Build a shared mesh-space analyzer for arbitrary pull directions and an explicit parting plane.
- [x] Inspect geometry in both mold halves, including the bore, at the pattern stage.
- [x] Locate individual obstructions with positions, affected halves, and severity.
- [x] Use directional surface/interval checks rather than treating undercut area as a release guarantee.
- [x] Account for tessellation tolerance and report unresolved sampling limits.
- [x] Search candidate parting positions and compare orientations with an explicit apply action.
- [x] Keep draft, obstruction, detail, metal thickness, and mold robustness findings separate.
- [x] Detect local narrow sand gaps/projections and expose the check's limits.
- [x] Provide a selected-finding cross-section and relevant layer attribution where available.

### M1.3 Mold inspection UI

- [x] Add a discoverable Casting workspace and toolbar entry.
- [x] Show mold halves, parting plane, pattern, and pull arrows in a linked visualization.
- [x] Provide closed/open/withdrawn steps plus manual separation controls.
- [x] Allow examination of upper and lower mold surfaces independently.
- [x] Show the finger-hole sand and the chosen core/bench strategy.
- [x] Highlight a selected obstruction and its extent.
- [x] Synchronize finding selection with the ring section and layer inspector.
- [x] Display build progress, invalid setup, stale results, and analysis limits clearly.

### M1.4 Corrective editing

- [x] Offer applicable fixes for the selected layer: reduced relief, move to side faces, and defer to bench.
- [x] Offer parting-plane changes as a separate candidate.
- [x] Evaluate each candidate against the same manufacturing setup.
- [x] Preview the candidate geometry and report changes before committing.
- [x] Apply a correction as one undoable edit; cancel leaves the original unchanged.
- [x] Avoid silently modifying source graph results; bind an exposed parameter or provide an explicit conversion action.
- [x] Show when no supported automatic correction is appropriate.

### M1.5 Pattern preparation and mold layout

- [x] Generate the pattern using the selected shrink compensation exactly once.
- [x] Represent external finishing stock and deferred ornament separately from nominal geometry.
- [x] Check that the compensated pattern fits the flask with the configured sand margin.
- [x] Add editable gate, sprue, vent, and feeder layout records in millimeters.
- [x] Show layout paths in the mold visualization and molding sheet.
- [x] Distinguish channels cut into sand from geometry attached to a printed pattern.
- [x] Estimate ring mass and additional channel/feeder charge separately.
- [x] Reuse the existing section-modulus scan as a labeled feeding aid.

### M1.6 Export and workshop sheet

- [x] Export a clearly named compensated pattern STL/3MF, manufacturing report JSON, and printable molding sheet.
- [x] Include design identity, nominal size, recipe, alloy, scale, parting plane, pull direction, flask dimensions, findings, and finishing instructions.
- [x] Include a dimensioned mold plan and opening/withdrawal instructions.
- [x] Evaluate the actual exported pattern and record mesh quality and analysis resolution.
- [x] Refuse an accidentally clean production export when geometry is invalid or confirmed to lock; allow a deliberately labeled diagnostic export for inspection.
- [x] Provide matching CLI commands for checking, debugging, and exporting a package.
- [x] Avoid partial-looking-success on export errors; preserve existing files unless replacement is explicit.

### M1.7 Verification

- [x] Test a simple pullable ring and a deliberately small local undercut.
- [x] Test both pull directions, an offset parting plane, and a rotated frame.
- [x] Test explicit bore treatment and invalid/empty/non-finite inputs.
- [x] Test fragile-gap findings and analysis resolution limits.
- [x] Test shrink once, finishing stock, and deferred features.
- [x] Test repair preview isolation, cancel, apply, and undo.
- [x] Test old-design load, new-design round trip, and recipe persistence.
- [x] Test export package contents and agreement between report and exported pattern.
- [x] Run desktop/CLI compilation and targeted core/CLI tests.
- [x] Exercise the actual desktop UI or document the environment limitation precisely.
- [ ] Workshop validation: print the diagnostic patterns and compare actual mold release with the report. This requires physical casting equipment and observed results from the shop.

## M2 — Precise sketches and direct editing

### Sketch document and construction

- [x] Introduce named sketches on ring section, head plan, and arbitrary construction planes.
- [x] Support lines, arcs/circles, polylines, and smooth curves as editable source data.
- [x] Add construction geometry and selectable points/segments.
- [x] Implement endpoint, midpoint, center, grid, perpendicular, and tangent snapping.
- [x] Implement dimensional, horizontal/vertical, coincident, symmetry, and tangent constraints with visible unresolved/conflicting states.
- [x] Preserve dimensions in millimeters; document angular conventions.
- [x] Import/export vector profiles without rasterizing manufacturing outlines.

### Direct tools

- [x] Add viewport grips for primitive/head/stone dimensions, twisted-band width/thickness, extrusion height, component translations, and ring placement; retain the procedural section/unrolled handles.
- [x] Support drag → typed dimension → preview → Enter/Escape.
- [x] Show measurements associated with source features and keep them updated.
- [x] Apply numeric constraints consistently to keyboard and pointer edits before accepting geometry.
- [x] Preserve selection and undo semantics through recomputation.
- [x] Include a sketch/constraint debug inspector and reproducible sample sketches.
- [x] Test constraints, coordinate transforms, snapping, cancellation, and persistence.

## M3 — Editable feature history and solid modeling

- [x] Expose a feature tree over the graph with stable identities, names, enabled state, and editable inputs.
- [x] Support feature suppression, rollback/preview, and downstream error localization.
- [x] Bind normal editing controls to graph inputs rather than requiring baking for every change.
- [x] Make general solid modeling reachable through the CAD workspace; retain legacy Manifold graph operations behind their existing optional feature.
- [x] Ship the analytic Rust solid kernel in the default desktop build.
- [x] Expose primitives, extrusion, revolution, union, subtraction, and intersection as ordinary tools.
- [x] Add section-along-path sweeps with predictable frame orientation.
- [x] Add multi-section lofts with validation for incompatible profiles.
- [x] Add geometric fillet/chamfer tools with explicitly supported edge cases.
- [x] Add shell/hollow operations with local thickness validation.
- [x] Preserve analytic source features even when display and manufacturing checks use meshes.
- [x] Build true-twist, gallery, custom-setting, and sculptural-band example projects.
- [x] Measure and validate the final solid result before export.
- [x] Test geometry topology, feature recomputation, suppression, cancellation, and solid exports.

## M4 — Components, stones, and assembly

- [x] Add named components for shank, head, settings, inlays, and reference stones.
- [x] Assign material and manufacturing setup per component.
- [x] Support visibility, selection, isolation, and assembly/exploded views.
- [x] Anchor components to ring/feature references with editable transforms.
- [x] Represent solder/assembly joints and intended clearances.
- [x] Check component interference and stone/metal clearance on evaluated geometry.
- [x] Support measured stone dimensions and preserve stone identity across edits.
- [x] Model stock for setting separately from the finished seat and stone.
- [x] Export components individually with an assembly manifest and bench notes.
- [x] Add inspectable two-part signet, solitaire, and inlay examples.
- [x] Test component persistence, transforms, interference, and assembly export.

## M5 — Intelligent sizing, variants, and manufacturing stages

- [x] Add explicit resizing policies: preserve head dimensions, preserve stone dimensions, preserve ornament pitch, and rebuild shank.
- [x] Resolve angular placements from stable feature anchors after a resize.
- [x] Regenerate stone spacing and live generators with constraints; report impossible arrangements.
- [x] Keep nominal fit, wide-band preferences, comfort bore, and finishing allowance separate.
- [x] Add direct bore diameter/circumference entry and named size-system display.
- [x] Add side-by-side size/parameter variants with weights and manufacturing findings.
- [x] Add batch export with per-variant reports and stable filenames.
- [x] Provide nominal, as-cast, pattern, and finished-state previews.
- [x] Test size changes preserving stone/head dimensions and minimum metal sections.

## M6 — Interchange and documentation

- [x] Export/import usable profile SVG/DXF with units and origin conventions.
- [x] Export component-aware 3MF and mesh exchange with explicit stage names.
- [x] Evaluate an analytic B-rep backend for exact fillets, shelling, and STEP round trips.
- [x] Preserve analytic curves/surfaces in STEP where the selected backend supports them; distinguish tessellated export.
- [x] Expand printable sheets with associative dimensions, sections, material, setting, and assembly information.
- [x] Verify exported files through independent readers and unit/scale checks.

## M7 — ML experiments tied to actual shop data

ML is an optional aid. Geometric validity, release interference, and export checks remain deterministic. No trained model is claimed before it has training data and a measured evaluation.

- [x] Define an opt-in local casting-trial record: design/setup identity, pattern material, alloy, process, measured dimensions, release outcome, detail quality, and defect notes.
- [x] Add a trial-entry/debug panel and JSON/CSV dataset export.
- [x] Establish deterministic baselines for shrink calibration and repair ranking.
- [ ] Evaluate a small regression model for shop-specific shrink/finishing predictions.
- [ ] Evaluate a ranking model for which valid repair a jeweler is likely to prefer.
- [ ] Evaluate image/shape similarity for searching prior designs and casting outcomes.
- [x] Separate training and evaluation by design family/run to avoid leaking near-duplicate rings.
- [x] Record baseline sample counts, training/held-out prediction error, model version, and unsupported conditions; label statistical confidence as unmeasured.
- [x] Compare documented TensorRS/Burn capabilities and define a CPU/binary-size/portability benchmark in `docs/ML-EXPERIMENTS.md`.
- [ ] Run that backend benchmark and evaluate uncertainty on actual workshop observations.
- [x] Keep the app functional without model files or network access; require an optional feature for any future neural backend.
- [ ] Implement only experiments that improve a measured baseline; retain the baseline and manual controls.

Research sources checked 2026-09-09: [TensorRS documentation](https://docs.rs/tensorrs/latest/tensorrs/) describes a lightweight CPU-oriented training library; [Burn](https://github.com/tracel-ai/burn) provides a tensor/deep-learning framework. Backend selection will follow a working experiment, not precede it.

## M8 — Integration and release verification

- [x] Route shared checks through GUI, CLI, graph exports, and MCP where supported.
- [x] Keep project loading backward compatible and add explicit version migrations where needed.
- [x] Audit desktop/Android/web feature availability and document platform gaps.
- [x] Port the principal workshop controls to Android/web and review the browser layout at phone width.
- [ ] Verify physical phone interaction and APK installation; extend desktop-only grips, history inspection, and size-comparison controls if needed on mobile.
- [x] Run relevant workspace tests and target checks.
- [x] Review UI screenshots at normal and narrow window sizes.
- [x] Add regression fixtures for each demonstrated workshop/CAD workflow.
- [x] Record completed work, remaining limitations, and exact validation evidence below.

### M8.1 — Portable workshop

Keep the Android palette and the browser's existing typography. Use a single-column layout below 720 points, wrapped actions, and 44-point touch targets. The geometry preview and explicit Preview / Apply / Cancel transaction remain central. Heavy evaluation runs on native background threads or a browser worker.

- [x] Share in-memory manufacturing and assembly packages with the existing filesystem exporters.
- [x] Add a portable workshop controller with stale-result rejection and reversible edits.
- [x] Expose recipe, pull, parting, stock, component selection, release findings, repairs, and manufacturing stages.
- [x] Expose CAD source features, numeric editing, exact-bore resizing, sketch point/source editing, and component settings.
- [x] Integrate the workspace in Android and the browser configurator, with project import and package delivery.
- [x] Validate controller transactions, package gates, host/wasm/Android compilation, and narrow-screen browser interaction.

### M9 — Four showcase rings

Requested after the portable work: one signet and one non-signet for each manufacturing route. Editable sources, images, and manufacturing evidence accompany every design.

- [x] Design and inspect Aster, a cushion signet with a central parting plane, straight Z withdrawal, and a preliminary head-feeding layout.
- [x] Design and inspect Tide, a band with twelve broad straight reeds and the same withdrawal requirements.
- [x] Design Lantern, a pierced octagonal signet with separately cast head/shank and explicit fitting and joining instructions.
- [x] Design Aureole, a continuous half-turn band with a documented investment-casting route.
- [x] Export editable projects, three views each, manufacturing packages, and a catalog/contact sheet; record unresolved physical checks explicitly.

### M10 — Two richly ornamented signets

- [x] Author Solstice: a one-piece sand-casting signet with sculpted solar relief, decorated cheeks, patterned shoulders, and a withdrawable compensated pattern.
- [x] Author Nocturne: an investment-casting signet with botanical engraving, framed seal, textured ground, recessed gallery, and calibrated stone-setting stock.
- [x] Inspect and refine real mesh renders; retain editable layers, source artwork, masks, and generator recipes.
- [x] Verify final mesh topology, wall screening, source round trips, sand withdrawal at two sampling pitches, and manufacturing packages.
- [x] Publish a local gallery with multiple views, artwork, editable projects, setting maps, and manufacturing notes.

## UI design direction

Retain the application's existing theme and typography. Use the mold itself as the central visual: subdued translucent sand, the existing metal color for the pattern, directional arrows, and the existing red/amber/green analysis colors. Numeric recipe controls sit beside the inspection, and selecting a finding opens the relevant cross-section. Use existing panel spacing and sentence-case labels. Avoid decorative dashboards: geometry, controls, and measurements carry the visual hierarchy.

## Implementation and validation log

- 2026-09-09: Created this plan from the current source and user-approved scope. Implementation begins with M1's shared manufacturing data and analysis, followed by the desktop workspace and export path.

- 2026-09-09: Implemented shared recipe/setup, pattern preparation, interval release inspection, mold slots, repair candidates, Casting pane, and atomic package export. Core manufacturing tests: 14 passed. Desktop and CLI build successfully; GUI review and CLI integration tests are next. Existing baseline: 31 castability tests passed.

- 2026-09-09: M1 automated verification: 17 manufacturing tests and 2 CLI integration tests passed. Desktop exercised under isolated Xvfb: mold opening, test obstruction, reduced-relief preview, cancel, defer-to-bench preview (20 → 0 obstructions), apply, and toolbar undo (20 restored). Sidebar/status consistency corrected; six pull-direction comparison added. Physical workshop trial remains open.
- 2026-09-09: Added a pinned MPL-2.0 Rust CAD kernel (the independently distributed dependency used by OpenCADStudio), analytic feature recipes, graph feature nodes, workplanes, curves, constraint solver, and snapping. Three CAD and three focused sketch tests pass; GUI integration and broader topology fixtures are in progress.

- 2026-09-09: CAD workspace compiles and has been exercised with an extrusion and editable sketch under Xvfb. Added graph-owned feature previews, component isolation/explosion, edge selection, interference inspection, vector SVG/DXF profiles, assembly export, exact-bore resize operations, and five inspectable CAD examples. Geometry regression cases now pass for analytic primitives, revolution, loft, fillet, chamfer, sweep, shell, subtraction, and half-twist closure.
- 2026-09-09: Full core run: 404 passed, one sweep test failed; switched to the kernel's general sweep API and its targeted regression passes. New modules/tests added since this run still need the final combined run.
- 2026-09-09: Added opt-in local shop observations, CSV/JSON export, measured scale regression, and held-out design-family error. No model weights or neural dependency added: comparing a neural model against workshop outcomes requires actual recorded trials.

- 2026-09-09: Independent OpenCascade reader accepted all 14 STEP fixtures as valid solids; solid counts match and mesh/STEP volumes differ by at most 0.46% (curved-surface tessellation). Added shared size batches, stage previews, rollback inspection, component joints, sampled local wall checks, and actual CAD sections. Hollow-box wall regression measures the intended 1 mm. Full combined test run is in progress.
- 2026-09-09: Added matching MCP manufacturing inspection/package and CAD inspection tools, graph inspection nodes, optional graph-bound operation inputs, and deterministic repair comparisons. Calibration held-out families now also exclude shared casting runs. No workshop outcomes or trained neural-model performance are claimed.

- 2026-09-09: Finished desktop integration. CAD Apply records the evaluated candidate immediately; exact 18.123 mm bore survives Apply and one Undo restores 17.800 mm with the head unchanged. Direct grips were exercised through drag, typed 1.500 mm tube radius, Enter preview/apply, and Undo to 1.200 mm. Grip motion is measured from the press origin to avoid a jump when the pointer approaches and presses within one frame.
- 2026-09-09: History inspection now evaluates the selected graph node before downstream resizing/property edits, including when a later resize fails. The regression also checks that the source graph is unchanged. Actual desktop rollback shows only the earlier shank and exposes the Return to end control. Stage previews show the shank changing from 22.600 × 22.600 × 2.400 mm nominal to 23.038 × 23.038 × 2.446 mm compensated pattern. Actual solid sections and three simultaneous size previews were reviewed.
- 2026-09-09: Reviewed 1600 × 1000 and 1100 × 800 desktop layouts under isolated Xvfb. Moved finish/light controls to the second toolbar row and clipped GL painting to the pane. Source grip measurements, candidate status, and report staleness remain visible. Review captures are in `/tmp/ringdesigner-review/`; no user desktop/session settings were used.
- 2026-09-09: Fixed portable source artwork resolution for pattern preparation, CAD procedural bands, size variants, and saved packages. A pattern exported with an empty session library reloads its embedded artwork and rebuilds identical vertices. Saved design format is now 2; version 0/1 migration, newer-version refusal, malformed versions, and large version integers are covered.

### Final validation evidence

| Check | Result | Evidence |
|---|---|---|
| Combined core, graph, CLI, MCP run | 532 passed: core 416, core integration 1, graph 67, CLI 5, MCP 43 | `/tmp/ringdesigner-combined-final-tests.log` |
| Graph after the final rollback regression | 68 passed | `/tmp/ringdesigner-history-tests.log` |
| File-version and CLI checks after format 2 | Library 8 passed; CLI 5 passed | `/tmp/ringdesigner-version-tests.log`, `/tmp/ringdesigner-version-cli-tests.log` |
| Final casting comparison/source-art checks | Manufacturing 23 passed | `/tmp/ringdesigner-casting-final-recheck.log` |
| Final sketch/constraint/profile checks | 5 passed | `/tmp/ringdesigner-sketch-final-recheck.log` |
| Desktop build and host workspace check | Passed | `/tmp/ringdesigner-history-build.log`, `/tmp/ringdesigner-workspace-final-check.log` |
| Browser configurator target | `wasm32-unknown-unknown`, no default features: passed | `/tmp/ringdesigner-wasm-final-check.log` |
| Android core target | `aarch64-linux-android`, no default features: passed | `/tmp/ringdesigner-android-core-check.log` |
| Independent STEP reader | OpenCascade accepted 16 valid models with matching solid counts; maximum mesh/STEP volume difference 0.4561% | `cad_exchange_probe`, `tools/check_cad_exchange.py`, `/tmp/ringdesigner-cad-final-5/independent-reader-results.json` |
| Independent DXF reader | ezdxf accepted both profiles; millimeter units, dimensions, and audits passed | `/tmp/ringdesigner-independent-reader.log` |
| Desktop sand-casting interaction | Test rail: 20 obstructions; reduced relief remained blocked; defer-to-bench preview reached 0; Cancel and Apply/Undo verified | `/tmp/ringdesigner-review/bench-preview.png`, `/tmp/ringdesigner-review/undo-toolbar.png` |
| Desktop precision editing | Exact bore, source dimensions, Enter/Cancel/Undo, history, stages, sections, and size comparison reviewed | `/tmp/ringdesigner-review/exact-bore-applied-final.png`, `/tmp/ringdesigner-review/exact-bore-undo-final.png`, `/tmp/ringdesigner-review/grip-typed-preview.png`, `/tmp/ringdesigner-review/grip-undo-final.png`, `/tmp/ringdesigner-review/history-before-resize-final.png` |

The remaining unchecked work is the physical mold trial, evaluating optional learned models on real shop observations, and physical-phone runtime verification. Desktop-only history/grips/size-comparison tools remain candidates for further mobile work. Supported solid operations, sampling limits, STEP import limits, and workflow instructions are documented in `docs/WORKSHOP-CAD.md`; the ML evaluation protocol is in `docs/ML-EXPERIMENTS.md`.

### Portable workshop and authored collection — 2026-09-09

- Added `ringdesign-workbench`, used by Android and Build a Ring. Native jobs run on a background thread; browser jobs run in a dedicated worker. A ready handshake queues the initial job; source/request identities reject stale results; canceled browser jobs terminate. Large ZIPs transfer as binary buffers instead of JSON arrays.
- Added responsive casting/CAD/files controls, editable snapshots for graph-driven imports, exact-bore resizing, source/sketch-point editing, component metadata, actual solid sections, and stage previews. Browser import accepts file selection, drop, or pasted JSON. Explicit export actions deliver browser downloads or Android share files; native configurator exports land in `workshop-exports/`.
- Refactored pattern/assembly export into shared in-memory packages. Filesystem and portable exporters use the same geometry gates. Investment sheets now give investment-pattern instructions; they do not instruct two-part mold withdrawal.
- Created `showcase/workshop-collection/index.html`, editable sources, STL/3MF/STEP packages, three real geometry renders per ring, and SVG/PNG/PDF contact sheets. Reproduction commands are in the collection README and `crates/ringdesign-core/examples/workshop_collection.rs`.
- Aster and Tide: zero sampled obstructions and unresolved rays at 0.100 and 0.075 mm pitch; both fit the flask. They retain **Review** status for low-draft surfaces and need physical trials. Screened radial walls are 1.280 and 1.875 mm. Their approximate sterling weights are 9.98 and 8.30 g.
- Lantern and Aureole: their relevant components remain blocked in all six tested axis-aligned sand orientations. Lantern's head has a 1.200 mm sampled minimum section; Aureole's sampled minimum is 1.993 mm. Approximate 14k weights are 15.06 g before fitting stock and 7.44 g. The nominal bore is 18.200 mm for all four.
- Lantern's attempted analytic union was not supported by the current kernel. The final design is an explicit two-part assembly. OpenCascade independently measures 24.749 mm³ (about 0.323 g) of contact stock to remove while fitting. The app's own interference operation remains unresolved and is labeled accordingly. Its head placement and joints need review when resizing.
- Lantern's STEP preserves analytic surfaces. Aureole's half-turn feature exports planar facets; its 203 MB STEP is valid but substantially larger. The editable project is the preferred way to revise that design.

| Verification | Result | Evidence |
|---|---|---|
| Portable controller | 2 tests passed: preview/cancel/apply/undo/redo, external edits, and stale jobs | `/tmp/ringdesigner-portable-final-tests.log` |
| Configurator regressions | 5 passed | `/tmp/ringdesigner-portable-final-tests.log` |
| Manufacturing packages and process sheets | 24 passed, including portable/native identity, export refusal, and investment instructions | `/tmp/ringdesigner-portable-package-final-tests.log` |
| Workspace check | Passed | `/tmp/ringdesigner-portable-workspace-final.log` |
| Native app builds | Desktop RingDesigner and Build a Ring: passed | `/tmp/ringdesigner-portable-native-build-final.log` |
| Full Android app target | `aarch64-linux-android`: passed | `/tmp/ringdesigner-portable-android-final.log` |
| Browser target and bundle | `wasm32-unknown-unknown`, no defaults; Trunk development bundle: passed | `/tmp/ringdesigner-portable-wasm-final.log`, `/tmp/ringdesigner-portable-trunk-final.log` |
| Chromium runtime | Graph-backed CAD: 30,720 triangles; blocked production export refused; diagnostic ZIP transferred as a 26,769,883-byte buffer; timer kept advancing; no uncaught errors | `/tmp/ringdesigner-portable-browser-review/results.json` |
| Browser delivery and touch | Downloaded production ZIP: 7 valid entries and Review status; 1100/390 px layouts reviewed; emulated touch selected CAD | `/tmp/ringdesigner-portable-browser-review/workshop-download.png`, `workshop-phone-cad.png` |
| Saved collection sources | 10 package sources rebuild identical pattern fingerprints; finer sand screening passes | `showcase/workshop-collection/verification.json` |
| Independent exchange | All four pattern STL identities and 3MF archives verified; OpenCascade accepts Lantern's 2 solids and Aureole's 1 solid; maximum STEP/mesh volume difference 0.0301% | `showcase/workshop-collection/independent-checks.json` |

### Ornamented signets — 2026-09-09

- Created `showcase/masterwork-signets/` and installed both editable projects in `~/.local/share/ringdesigner/designs/masterwork-signets/`. The collection has eight views per ring, including turntables, undecorated bodies and casting patterns, original artwork, layer inventories, setting data, STL/3MF, lighter GLB previews, and compressed manufacturing packages.
- Solstice: sterling silver, approximately 20.40 g before finishing, 11 editable layers. Broad solar rays include axial withdrawal stock; lotus cheeks, sunseed tiling, masks, warp, remapping, vines, rails, beading and reeds remain in the casting pattern. Only fine satin texture is deferred to the bench. The final 2,457,600-triangle pattern has zero sampled obstructions/unresolved rays at 0.100 and 0.075 mm spacing, fits the flask, and has no manufacturing detail-size findings. Its radial wall screen is 2.844 mm; low-draft surfaces retain Review status.
- Nocturne: 14k yellow gold, approximately 22.73 g before setting and finishing, 22 editable entries including the seal group's children. Original botanical SVGs, double framing, pearl edging, hand-drawn stars, masked guilloche, acanthus shoulders, recessed galleries and three calibrated stone seats. The palm inscription is a bench operation. Recesses retain floors; they are not through-holes. The ring remains obstructed in all six axis-aligned sand pulls and uses investment casting. The radial wall screen is 2.409 mm; nine fine-detail advisories remain.
- Sparse normal-ray screening on coarser meshes found minima of 0.633 mm for Solstice and 0.517 mm for Nocturne, with 9/59 samples below the general 0.8 mm limit. These small edge/detail locations need section and workshop inspection; the radial measurements are not minimum-wall guarantees. Neither ring has a physical casting trial or calibrated shrink allowance.
- Both project sources and both packaged sources rebuild identical nominal and compensated triangles from empty alpha libraries. Derived support/resist masks are quantized to their saved 16-bit samples before building. An independent numpy/trimesh reader confirms four closed, consistently wound, single-body STL meshes with the intended 18.200 mm nominal bore and matching volumes. 3MF payload CRCs survive delivery compression; ZIP contents match their folders.
- Creating the stone-heavy design exposed a preview error: displaced arc length shifted stones away from their intended coordinates and could tilt them onto bezel walls. Previews now use the setting report's frames. Five gem tests and thirteen stone-setting tests pass; the desktop binary builds. Saved inspection floors now match the recipe, and the desktop report distinguishes investment casting from sand casting.
- Chromium exercised all 16 gallery view controls and local download links, with no browser exceptions; the 390 px layout has no horizontal overflow and retains 44 px buttons. Both saved designs were opened and inspected in an isolated desktop session. Browser captures are in `/tmp/ringdesigner-masterwork-gallery/`; native captures are in `/tmp/ringdesigner-masterwork-native/`.

Reproduction: `cargo run -p ringdesign-core --example masterwork_signets -- NEW_DIRECTORY`; validate an existing collection with `-- EXISTING_DIRECTORY --verify`. Catalog and independent-check scripts are `tools/catalog_masterwork_signets.py`, `tools/check_masterwork_signets.py`, and `tools/check_masterwork_gallery.mjs`.

### Android store release — 2026-09-10

- [x] Bump Android to **0.11.0**, versionCode **16780032**, and record the mobile Workshop/CAD/casting changes and gemstone preview fix in its changelog.
- [x] Build the signed ARM64 release APK. Verify package/launcher metadata, APK integrity, ZIP alignment and 16 KB native-library alignment; match the signing certificate against the downloaded store release 0.10.0.
- [x] Pass all **100 Android library tests** and **2 shared workbench tests**.
- [x] Publish to `https://appstore.shadowbroker.app`, then download the released APK and verify its size, SHA-256, signature, update feed and server changelog. APK: **13,001,599 bytes**, SHA-256 `015ea06be791e08e09d834d1b000cfdbd3c0426374fc366b47e1b956070c1955`.

The verified APK, release receipt and build/test logs are in `target/releases/ringdesigner-android-0.11.0/`. Physical-phone installation and runtime testing remain open.

## M11 — Mobile visual editor overhaul

Design and interaction specification: `docs/MOBILE-EDITOR.md`.

- [x] Run the native APK in an isolated S26 Ultra display-profile emulator, capture revised layouts and audit earlier clipping in the source.
- [x] Replace overflowing navigation and stacked sheets with compact navigation and one bounded inspector; account for landscape and keyboard space.
- [x] Add Shape, Surface, Stones and Casting selection modes with visible explanations and access to the existing tools.
- [x] Connect primary shape parameters to on-model dimensions and draggable handles.
- [x] Select contributing ornament and stone settings from the viewport and expose relevant controls.
- [x] Show casting guides and selectable inspection feedback in the ring view.
- [x] Keep the camera steady, show previews during dragging and provide a Before comparison.
- [x] Verify narrow layouts, larger display scaling, keyboard entry, picking, editing and undo using the emulator and focused interaction tests.
- [x] Build an ARM64 candidate APK and record screenshots, checks and remaining physical-device limitations.

Evidence: `docs/MOBILE-EDITOR-REVIEW.md`; 109 Android library tests pass.
The Android 36 emulator ran at 411 and 320 logical pixels wide with KVM.
Solstice's face handle and Nocturne's stone/layer selection were exercised with
touch input; exact keyboard entry, paint, Before and Undo were checked against
saved source. Release candidate: `target/releases/ringdesigner-android-0.12.0/`.
Physical S26 Ultra, Samsung keyboard and S-Pen verification remain; this candidate
was superseded by 0.13.0, which includes these changes and was published on
2026-09-12. The follow-on viewport tools are tracked in M12 below.

## M12 — Direct viewport tools and a recorded masterwork

Scope: implement the four proposed tools on Android and desktop, create a new
investment-cast signet comparable to Nocturne, and deliver two recordings of the
actual applications. Interaction specification: `docs/VIEWPORT-TOOLS.md`.

- [x] Share surface painting/stamping, section geometry and inspection state between platforms.
- [x] Paint and place alpha stamps on the ring, with seam handling, pressure input and undo.
- [x] Drag a section plane through the mesh and read the cut geometry without changing the source.
- [x] Display stone clearance envelopes and the existing quantitative crowding findings while editing.
- [x] Animate sampled mould cavities along the configured pull, with obstruction markers and an explicit prepared-pattern preview.
- [x] Keep controls usable at 320/411 logical pixels and in the desktop viewport.
- [x] Build Thalassa, a layered marine signet with a similar feature count to Nocturne; save portable source and inspection results.
- [x] Exercise all four tools in the emulator and desktop app; verify source preservation and editing history.
- [x] Record and review separate Android and desktop highlight reels from actual application windows.
- [x] Re-record both reels as explicit in-app creation sessions beginning with a blank band and showing shape, ornament, stone-setting and casting decisions.
- [x] Package the APK, desktop build, ring source, videos and verification notes.
- [x] Publish Android 0.13.0 to the app store and verify the downloaded APK, signing certificate, update feed and server changelog (2026-09-12).

M12 evidence: `docs/VIEWPORT-TOOLS-REVIEW.md` and `showcase/thalassa/README.md`.
Thalassa contains 27 layer entries and seven stones. Separate chaptered Android
(2:11) and desktop (2:19) window recordings demonstrate all four tools plus local
wall readings. Final packages are in `target/releases/ringdesigner-android-0.13.0/`
and `target/releases/ringdesigner-desktop-0.13.0/`. Android 0.13.0 was published to
`https://appstore.shadowbroker.app` on 2026-09-12, replacing 0.11.0. The downloaded
ARM64 APK matches the packaged build: versionCode 16780544, 13,124,479 bytes,
SHA-256 `b735d58f5913f7344093fda193f4b58f596958bdf45a57b7c2024253783c5de2`.
Its signing certificate matches the previous release; the live listing, update
feed and server changelog agree. Receipts and verification evidence are in the
Android release directory. Physical phone/stylus testing and actual shop casting
remain outside this software review.

Creation-cut evidence: `showcase/thalassa/highlights/creation/thalassa-creation-android.mp4`
(2:03, 1080×2420) and `showcase/thalassa/highlights/creation/thalassa-creation-desktop.mp4`
(2:18, 1600×1060). Both recordings preserve the action timelines and raw captures
beside their captioned clips. The Android cut forms a cushion signet, applies and
sizes Greek-key relief, adds an emerald bezel setting, paints and stamps the mesh,
and switches the manufacturing check to investment casting. The desktop cut selects
the signet shank, edits its face, applies Celtic-knot relief, builds and refines a
five-millimeter emerald bezel, and exercises the mould-opening workflow.

## M13 — Sand signet showcase revision

The creation cuts above were rejected: their proportions, ornament and stone
placement did not match the original masterworks. Replacement acceptance:

- [x] Compose and visually review Aster Atelier with a rounded seal, eight sculpted petals, paired palmettes, textured cheeks and shallow reeding.
- [x] Verify its actual compensated pattern at 0.10 and 0.075 mm sampling: no detected withdrawal obstructions or unresolved rays; retain low-draft and sand-web warnings. All twelve ornament layers belong to the casting pattern.
- [x] Create the same design inside Android and desktop, showing actual construction operations and inspecting each visible result.
- [x] Review both recordings at construction checkpoints and chapter boundaries for framing, successful edits and accurate captions; decode every frame and retain a sustained final reveal.
- [x] Deliver editable source, verified casting package, final renders and both replacement reels; Taildrop the videos and renders to the S26.

M13 evidence: `showcase/aster-atelier/README.md`. Both native app saves reproduce
the reviewed twelve-layer source. Independent STL/3MF checks confirm one
watertight ring body and the intended bore/compensation. The Android creation
reel is 4:45 at 1440 × 3200; desktop is 3:55 at 1600 × 1060, including caption
strips. Both pass complete stream decoding and end with 45-second reveals.
The two creation MP4s and three native viewport PNGs were sent successfully to
the S26 on 2026-09-12; `showcase/aster-atelier/review/taildrop-receipt.json`
records file hashes and successful transfer results. Casting status remains
Review for low draft and narrow sand slots; physical shop casting is unverified.

## M14 — Resizable mobile workspace and floating tools

Specification: `docs/MOBILE-TOOLBARS.md`.

- [x] Make the lower inspector resizable, retain a usable viewport and persist portrait/landscape sizes.
- [x] Compact navigation, buttons and numeric-control rows.
- [x] Add movable left-side toolbars and contextual palettes with submenus, selection-aware controls and layout reset.
- [x] Make every mobile numeric value text-edit-only while preserving slider bars and viewport handles.
- [x] Verify scrolling, typing, resizing, toolbar movement, selection, undo, keyboard and screen bounds with focused checks and the Android emulator.
- [x] Build signed ARM64/x86_64 APKs and record the review evidence and release status.
- [x] Publish Android 0.14.0 to the app store and verify the downloaded APK, signing certificate, update feed and server changelog (2026-09-12).

M14 evidence: `docs/MOBILE-TOOLBARS-REVIEW.md`. All **116 Android library tests**
and **4 shared workbench tests** pass; desktop compilation passes. Native Android
36 review covered 411/320-point widths, portrait/landscape resizing, Gboard entry,
numeric scrolling, contextual tools, selection, Undo and saved layout recovery.
Signed 0.14.0 APKs and publication receipts are in `target/releases/ringdesigner-android-0.14.0/`.
VersionCode **16780800**; ARM64 SHA-256
`c355639a48ea16c1ad2b587f6651b20e72fe2785305813579038a2b92de898e9`.
Android 0.14.0 is published to `https://appstore.shadowbroker.app`, replacing
0.13.0. The downloaded APK matches the packaged ARM64 build, including its
signing certificate; the live listing, update feed and server changelog agree.
Physical phone, Samsung keyboard and S-Pen checks remain outside this emulator review.

## M15 — Studio rendering and a verified creation workflow

- [x] Configure ARTEMIS using the existing private Gemini credential and fix headless emulator launch; pass all five required diagnosis checks.
- [x] Explore Android quality, polish, toolbar collapse and camera controls through ARTEMIS; retain verified action paths and independently compare the saved source.
- [x] Add tap-based zoom in/out to View and the floating camera tools; verify in/out/in through ARTEMIS and confirm 1.2× zoom in device telemetry.
- [x] Explore the revised ring's actual construction operations through ARTEMIS before recording: blank band, sizing, cushion head, rounded draft layer, inspector resizing and four camera views; five checkpoints passed.
- [x] Persist Android detail tiers; refine settled meshes to 655k by default and 1.38M for showcase capture, using a lighter mesh during edits.
- [x] Share reflective studio metal and separate gemstone shading across Android and desktop; expose alloy and polish controls.
- [x] Compare native viewport images at matching camera angles, check fine ornament, the draft overlay, runtime shader compilation and mesh settling.
- [x] Prevent imported wide borders from being clamped merely by displaying their controls; cover the regression in desktop tests.
- [x] Record and inspect native-resolution preview clips; reject encoder downscaling and retain the raw display captures.
- [x] Rebuild and review the M13 sand-signet creation reels with sustained detailed reveals; deliver after source and sampled withdrawal checks pass, retaining the casting Review findings.

M15 evidence: `docs/STUDIO-PREVIEW-REVIEW.md`. ARTEMIS's initial credential and
headless launch failures were resolved locally; no user credential setup remains.
Pro viewport review completed with five passed checkpoints; its single-touch
tools could not pinch. A subsequent Flash run successfully exercised the added
zoom buttons. Native Android/desktop source checks and 137 library tests pass.
0.15.0 APKs are local candidates. Nocturne remains an existing investment-cast
rendering reference. The subsequent Aster Atelier creation sessions complete M13;
their evidence is in `showcase/aster-atelier/README.md`. Store publication of
0.15.0 is separate from this video delivery.


## Desktop and mobile workspace overhaul — 2026-09-19

- [x] Inspect desktop/mobile chrome, graph routing, shared SVG icons and Mastertech's GitHub updater.
- [x] Define shared interaction styling: coloured Atelier icons, visible frames on every selectable button, accent selection and hover strokes.
- [x] Consolidate desktop commands into one row: File, View, workspace/tools menus, right-aligned build/preview/export/update controls.
- [x] Move viewport display toggles into a local bottom toolbar; rename layouts Single, Split Vertical, Split Horizontal and Four.
- [x] Add persistent workspace-specific panels for model, graph, surface, casting and CAD work; make Edit graph switch the visible viewport.
- [x] Move graph parameters into a spacious Node inspector; highlight active panels and make resize boundaries and clickable nodes clear.
- [x] Preview selectable features on hover; add explicit clear selection/navigation controls and Escape behaviour on both apps.
- [x] Adapt mobile panel visibility and command menus to the active workspace; disable unavailable and running actions with explanations.
- [x] Add nonblocking GitHub release checks, compatible asset selection, verified downloads, safe replacement and preserved-session restart; supply release packaging/workflow.
- [x] Verify desktop layout and graph/selection behaviour; explore and verify mobile interactions with ARTEMIS before writing mobile tests.
- [x] Run relevant tests and release builds, publish the Android update, and verify its downloaded hash.

Design tokens: canvas `#0D0D12`, panel `#16141D`, ink `#E8E8E8`, selection `#E66C99`, hover `#67D9D5`, metal `#E8BF70`. Existing proportional UI font: 12 px supporting text, 14 px controls, 17 px workspace titles.

Design direction: retain the jewellery workbench's black/violet surfaces and pink selection accent; use aqua for hover, warm gold for metal/shape, violet for stones/graphs, blue for files and green for successful actions. Existing UI typography remains compact and readable. One global command row sits above workspace-specific panels; view controls belong inside the viewport. Strong panel edges and generous inspector rows distinguish resize handles from content. No decorative dashboard chrome.

Update behaviour: automatically check and download compatible stable releases without blocking editing. Validate size and SHA-256 before replacement; preserve the working design and workspace before a requested restart. No GitHub release exists yet, so include the publishing contract/workflow and handle the empty release feed clearly.

Verification: 213 Rust tests pass; ARTEMIS follow-up passed 8/8 checkpoints, and the final x86_64 APK passed six repeatable device checks. Native Linux review verified feature hover/selection, Edit graph, context panels and popup frames. Android 0.25.0 is published (version code 16783616); the authenticated store download exactly matches the ARM64 build. Linux desktop 0.2.0 and both Android ABI builds succeeded. Evidence: `docs/WORKSPACE-REVIEW.md`.

## Graph workspace refinement — 2026-09-19

- [x] Review panel/frame overlap, split layout architecture, graph inputs, workspace restrictions and inspection support.
- [x] Add inner padding to inspector frames and consistent button heights across desktop/shared controls.
- [x] Replace fixed viewport splits with a persistent, resizable egui_tiles tree; default Graph to 35% stacked previews and 65% graph.
- [x] Keep a free 3D preview above a locked orthographic feature preview; update both in real time and focus the lower view on the selected node.
- [x] Add Graph View/Edit controls, default View, and restore inline inputs in Edit; make node titles drag without selecting their text.
- [x] Make the active workspace explicit; scope tools to the working surface and explain graph-driven geometry with an actionable route to editing.
- [x] Add a bug-report/feature-request form that opens a prefilled GitHub issue for the user to submit.
- [x] Keep command search above viewport navigation overlays.
- [x] Keep session storage stable across releases and migrate existing saved designs/layouts without overwriting them.
- [x] Verify with the inspection-enabled desktop, regression tests and visual review; build desktop and publish any Android changes after device checks.

Retain the established canvas `#0D0D12`, panel `#16141D`, ink `#E8E8E8`, pink selection `#E66C99`, aqua hover `#67D9D5`, and gold `#E8BF70`. Keep the existing proportional fonts. Use 8 px inspector padding and one standard button height per platform, preserving mobile touch targets.

Default Graph layout: `[ free 3D / focused orthographic | graph (65%) ] [Node inspector]`. Both dividers resize; layouts and focus settings persist per workspace. The workspace label states the editing context, while per-pane headers state the displayed surface. Geometry generated by a graph stays protected from silently losing its recipe; explain that distinction when surface tools are unavailable. View mode keeps node fields in the inspector, Edit restores inline fields. This follows the requested modeling workflow without adding floating windows or decorative chrome.

Verification: 220 Rust tests passed. Native inspection verified both splitters, live mesh updates, title dragging, panel padding, button heights and command-search stacking. Final ARTEMIS review passed 8/8 checks on Android 16. Desktop 0.3.0 and both Android 0.26.0 ABI release builds succeeded. Android 0.26.0 is published (version code 16783872), with its downloaded ARM64 SHA-256 matching the local APK and store metadata. Evidence: `docs/GRAPH-WORKSPACE-REVIEW.md`.

## Visual sources and complete template library — 2026-09-19

- [x] Inspect alpha source editing, template assets and graph dependencies; identify redundant nodes and their effect on output.
- [x] Replace raw PNG/base64 editing with a cached visual alpha picker in the graph and Node inspector.
- [x] Include every authored template in a shared categorized library, with collection and template preview thumbnails.
- [x] Show the active file/template name centered in the desktop top bar; preserve meaningful names when opening and saving.
- [x] Remove unused template nodes and redundant inputs without changing their rendered designs; verify the generated files and builders agree.
- [x] Darken button and selectable backgrounds while retaining readable text, icons and clear selected/hover states on both apps.
- [x] Verify native UI interactions and regressions: 300 tests pass; inspect both source editors, template menus and title behavior at two window sizes.
- [x] Verify Android device behavior, complete release builds and publish the Android update: ARTEMIS passed 8/8 checks; desktop 0.4.0 and Android 0.27.0 builds succeeded; the published APK's downloaded hash matches.
- [x] Fix the final manual review finding: reopening after a short search restores complete thumbnails and captions; keyboard/rotation bounds are respected. The regression test and 11/11 targeted device checks pass. Android 0.27.1 is published and its downloaded hash matches; desktop 0.4.0 includes the same fix.

## CAD workspace and interaction polish — 2026-09-19

- [x] Audit CAD end to end: identify working sketch, primitive, revolve, sweep, loft, transform, boolean and export operations; fix broken routes and explain or disable unavailable actions.
- [x] Give CAD a clear starting workflow and bring its viewport, top controls and local bottom toolbar in line with the modeling workspace.
- [x] Reuse the interactive navigation cube and camera controls in CAD: face/edge/corner views, orbit, pan, zoom, fit and orthographic views.
- [x] Add colored SVG icons for every CAD feature and operation, including twisted ring, cylinder, revolve and sweep; provide small geometry/example previews for choosing tools.
- [x] Group CAD commands into useful menus and show a down caret on menu buttons throughout desktop and mobile.
- [x] Make command-search results fill the popup width, keep the scrollbar at its right edge, and support Up/Down selection plus Enter activation.
- [x] Add a paint-position/brush preview and smooth 3D camera tracking while dragging across the paint surface, with a clear way to control following.
- [x] Align parameter labels left and drag values/sliders right in consistent rows, preserving usable widths and touch targets.
- [x] Stabilize geometry hover at the editable-feature level; stop advertising individual mesh triangles as editable and remove flickering labels/highlights.
- [x] Validate real CAD workflows, camera controls, search keyboard behavior, paint following, parameter alignment and hover stability in the native app; explore affected mobile interactions with ARTEMIS before writing mobile UI tests.
- [x] Complete relevant regression tests and release builds, update the CAD usage/review notes, and publish validated Android changes.

Retain the workbench's dark violet surfaces, readable ink `#E8E8E8`, pink selection `#E66C99`, aqua hover `#67D9D5`, gold shape controls `#E8BF70` and colored SVG vocabulary. Keep existing fonts and uniform control heights. CAD layout: `[Create ▾ | Modify ▾ | Sketch ▾ | View ▾]`, a spacious geometry viewport with the shared navigation cube, a local view/display footer, and an aligned feature inspector/history. Previews explain an operation's resulting shape. Menus disclose their behavior with carets; unavailable actions state their prerequisite. Camera following is smooth and subordinate to deliberate navigation. Preserve concurrent edits by reviewing the current file immediately before each focused patch.

Keep the established jewellery workbench palette and proportional fonts. Darker control surfaces use ink-violet `#191620`, hover `#292131`, and selected plum `#3B2845`; text remains `#E8E8E8`, with pink `#E66C99` selection and aqua `#67D9D5` hover outlines. Image choices display their actual alpha or ring thumbnail next to a short name. Collections group the full library; long names truncate only in the centered title and retain a hover tooltip. Decode only visible images and cache them; never put embedded image payloads into editable text widgets.

Coordination: the other agent owns node-field alignment, dock dragging/closing, graph focus, preview popup behavior, and material/camera/shank/imported-signet thumbnails. CAD and regular inspector edits here preserve those changes. CAD kernel audit: 10/10 existing tests pass. All enabled Create defaults and all seven Modify defaults build valid positive-volume solids; generic Twisted sweep is disabled with its kernel limitation explained. Regression suites pass: desktop 43, shared workbench 33, Android host 132 (218 tests including the core CAD audit). Native release inspection verified aligned sliders/fields, full-width keyboard search, CAD material changes, sketch dimension editing, resizing, paint following, stable feature hover, and Escape closing a menu before cancelling a CAD candidate. ARTEMIS passed 14/14 initial and 7/7 final checks on Android 16; the scrolling cube and live pink brush cue were inspected in screenshots. Desktop 0.5.0 is built locally. Android 0.28.0 is published (code 16784384); the downloaded APK SHA-256 matches the validated ARM64 build: `30027ea2759f634a8ffe9fe0da8d7eab83b7fc06974474d1067445cf41d8e0c3`. Usage, limits and evidence: `docs/CAD-WORKSPACE-REVIEW.md`.
