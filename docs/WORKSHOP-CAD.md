# Workshop and CAD workflows

The desktop toolbar opens **Casting** and **CAD**. Start directly with `ringdesigner --casting` or `ringdesigner --cad`. Version 0/1 design files migrate without changing their geometry. New saves use format version 2 so older version-aware readers refuse CAD documents they cannot interpret. Procedural rings and existing graphs remain supported.

## Sand-casting a ring

1. Open Casting → Recipe & pattern. Choose the alloy/sand recipe, pull direction, bore strategy, flask, stock, and sample pitch. Recipe values are starting assumptions until replaced by measured shop values.
2. For a CAD assembly, choose one metal component. Its own recipe can be selected independently of the shank or other parts.
3. Examine both mold halves, the opening/withdrawal sequence, and the finger-hole sand. Click each obstruction to see its section and possible layer ownership. Compare orientations before changing the pull direction.
4. In Release & repairs, select a layer and a supported correction. Preview the correction; compare the geometry and findings. Apply commits one undoable edit; Cancel leaves the source alone. Graph-driven ornament requires an exposed/source edit or the explicit editable-copy action.
5. Record gates, vents, sprue, feeder, and bench instructions. Channels are a sand-cutting plan; they are not silently attached to the printed pattern.
6. Export a new package. It contains the actual compensated STL/3MF, recipe, report, source project, mold plan, and printable molding sheet. Production export refuses invalid or geometrically obstructed patterns. Diagnostic export names and labels the files explicitly.

```sh
cargo run -p ringdesign-cli -- casting check ring.ring.json --json
cargo run -p ringdesign-cli -- casting check ring.ring.json --pull 1,0,0 --pitch 0.1 --json
cargo run -p ringdesign-cli -- casting export ring.ring.json --out pattern-package
cargo run -p ringdesign-cli -- casting export ring.ring.json --diagnostic --out diagnostic-package
```

Shrink is applied once: pattern scale is `1 / (1 − shrink_percent / 100)`. Bore stock reduces the pattern's unscaled bore; external stock adds metal. Bench-only ornament is omitted from the pattern and listed in the sheet. Nominal geometry is retained in the source project.

## General rings and component assemblies

CAD → Examples provides a twisted band, two-part signet, solitaire stock with a measured stone envelope, inlay stock, and gallery. These are editable starting projects, with source features and bench tasks visible.

- **Features:** primitives, procedural shank, true elliptical-section ring twist, extrusion, revolution, sweep, loft, booleans, supported fillets/chamfers, shelling, and component transforms. Click a visible edge before adding a fillet/chamfer. Unsupported topology/radii produce a located preview error.
- **Sketch:** draw or edit lines, arcs, circles, polylines, and cubic curves. Construction geometry stays out of solids. Dimensions, workplanes, constraints, snapping, and SVG/DXF exchange use millimeters. Arc creation uses center → start → end on the same circle. Shift-click constraint points in the order shown by the tool.
- **Components:** assign roles, material, reference stone identity, ring anchors, recipes, joints, and bench notes. Isolate parts or separate the display with Explode. Interference uses solid intersection where supported; a bounding-box clearance below the target remains unresolved.
- **Sizes:** enter exact bore diameter or circumference, choose preservation policies, preview a graph-owned resize, or compare/export multiple sizes with weights and component manufacturing reports. Measured stones remain fixed. CAD heads stay fixed during automatic shank resizing; edit their own dimensions explicitly.
- **Stages:** compare nominal target, compensated pattern, expected as-cast stock, and finished target for the chosen component. The finished view is the intended result; as-cast assumes uniform shrink.
- **Section:** cut the evaluated solid on X/Y/Z planes. The regular procedural section/unrolled view is not a substitute for a CAD solid section.

Every feature edit is a candidate until Preview succeeds and Apply is selected. Enter evaluates/applies a candidate when a text field is not active; Escape discards it. Preview-through-feature is temporary history inspection; return to the end before applying/exporting. The normal graph inspector remains available, including the optional operation input on CAD feature nodes.

Blue viewport grips edit source dimensions for boxes, cylinders, spheres, toruses, twisted bands, and extrusions, plus component translations and ring anchors. Numeric fields accept exact values. Earlier feature dimensions can be overridden by later features such as Resize; the feature tree and evaluated report show the distinction. Reference stones use the same dimension tools, while automatic ring resizing preserves their measured dimensions.

```sh
cargo run -p ringdesign-cli -- cad example two-part-signet --out signet.ring.json
cargo run -p ringdesign-cli -- cad check signet.ring.json
cargo run -p ringdesign-cli -- cad export signet.ring.json --out signet-assembly
cargo run -p ringdesign-cli -- cad step signet.ring.json --out signet.step
cargo run -p ringdesign-cli -- cad resize signet.ring.json --bores 17.3,18.123,19.0 --out signet-sizes
cargo run -p ringdesign-cli -- cad profile-import head.svg --out head-sketch.json
cargo run -p ringdesign-cli -- cad profile-export head-sketch.json --out head.dxf
```

An assembly package contains separate nominal metal STLs, component-aware 3MF, STEP, manifest, source project, and an assembly sheet with measured sections. Reference stones stay out of metal files. Included casting subpackages are diagnostic review patterns; use Casting for a production pattern package.

## Inspection and automation

MCP provides `manufacturing_check`, `manufacturing_export`, and `inspect_cad`. Checks resolve the attached graph and use a snapshot outside the engine lock. The manufacturing report includes snapshot generation and the prepared geometry fingerprint. Graphs have `manufacturing.inspect` and `cad.inspect` nodes.

The geometry fingerprint identifies STL coordinates/connectivity; it is not a cryptographic signature. SVG/DXF import rejects unsupported transforms, ambiguous units, elliptical SVG arcs, and DXF bulges rather than changing an outline silently. STEP exports the kernel's analytic surfaces; procedural and twisted-ring source meshes remain planar B-rep faces. STEP import into RingDesigner is not implemented.

Display/STL tessellation approximates curves. Shared vertices are welded at 0.000001 mm. The spline tessellator can leave a planar triangular seam between differently sampled adjacent faces; only seams no wider than the requested chord tolerance are filled, on a validated source body. Larger gaps and nonmanifold results are rejected. This does not change the analytic STEP solid.

Release checks sample finite rays; local wall checks sample surface normals; sand-slot checks do not calculate sand strength. Physical mold release, fill, shrink, finishing, and setting still need the shop trial in `plan.md`. These checks expose their resolution and unresolved cases instead of certifying a casting.

## Independent exchange verification

`tools/check_cad_exchange.py` documents its isolated Python test dependencies. Generate fixtures with the `cad_exchange_probe` example, then run the script to check STEP with OpenCascade and DXF with ezdxf. These readers are not application dependencies.

Android and Build a Ring now expose **Workshop**. It includes manufacturing setup, pull/parting, stock/flask/channels, component selection, actual release findings, repair candidates, stage views, solid sections, numeric CAD features, sketch points, exact-bore resizing, component settings, and project/package export. Below 720 points the preview and controls stack vertically. Browser imports support a file picker, dropping a project, and pasted JSON; Android also has its existing Files workspace.

Edits require **Preview → Apply**; **Cancel**, **Undo**, and **Redo** preserve the original. Imported graph-driven designs evaluate their graphs; **Make editable snapshot** explicitly creates a direct-feature candidate. Advanced feature source exposes operations, constraint definitions, and assembly parameters beyond the compact numeric controls. Desktop retains the richer graph history, sketch canvas, grips, and size-comparison tools.

Native Workshop calculations run on a background thread. Browser calculations run in a dedicated worker, with a ready handshake, stale-result rejection, and surfaced worker failures. Pattern and CAD assembly downloads use the same in-memory packages and manufacturing gates as the filesystem exporters. Project exports preserve the source; assembly files are nominal and the per-component pattern review folders are diagnostic.

Build the browser bundle with `trunk build` in `crates/ringdesign-configurator`; both Rust binaries in `index.html` are required. Worker bundling follows the official [Trunk asset workflow](https://trunk-rs.github.io/trunk/guide/assets/index.html). The matching wasm-bindgen tool must be installed or downloadable. Native configurator files go to `workshop-exports/`; browser files download; Android files go to app storage and open the share sheet. Browser workshop projects also persist through eframe storage.

Full Android target compilation and Chromium worker/layout review have passed. Physical phone interaction and APK installation remain unverified. The browser check is reproducible with `tools/check_portable_browser.mjs` and a Trunk output directory; its evidence includes 1100- and 390-pixel captures and a graph-backed CAD job that runs while the page's timer continues.

## CAD backend provenance

The project uses the independently distributed [cadkernel Rust library](https://github.com/HakanSeven12/cadkernel), pinned to `920ff591028ab4ba13648b920d9d333ec3b7d44b`, under MPL-2.0. OpenCADStudio was studied for workflow conventions (workplanes, snapping, source features, and preview/cancel interaction); its GPL application source was not copied into this project. The kernel is an ordinary dependency with its own license, not relicensed under the application's license.
