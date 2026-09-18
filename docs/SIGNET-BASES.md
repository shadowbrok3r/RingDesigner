# Showcase templates and imported signet bases

Android **File → New → Stock masterworks** contains rebuilt **Nocturne**, **Solstice**, **Aurelia** and **Vesper**. Desktop exposes the same projects through its template graph menu. The source stock, relief maps, editable layers, stone references and manufacturing recipes are bundled; they work offline. New saves use format 4 so older readers cannot silently omit sand support or reduce the height maps; use the current desktop build. Aster Atelier and Thalassa remain under Showcase templates.

These four compositions were authored in a fresh native chart on their imported masters. Painted reserve masks keep raised ornament off the real face rim and bore transition. Cheek ornament is recessed into the stock. They replace the earlier “imported shoulders” transplants in the menu; the old project files are retained in the repository for comparison.

| Template | Imported master | Role |
|---|---|---|
| [Nocturne](../graphs/templates/nocturne-imported.graph.json) | 015, octagonal | Rebuilt night garden with three stone references |
| [Solstice](../graphs/templates/solstice-imported.graph.json) | 001, cushion | Rebuilt solar sand pattern |
| [Aurelia](../graphs/templates/aurelia-imported.graph.json) | 006, large rectangular seal | Floral sand-pattern showcase, 20 × 26 mm face |
| [Vesper](../graphs/templates/vesper-imported.graph.json) | 019, shield | Free-mode celestial showcase, 22 × 22 mm face and 15 stone references |

Each graph exposes US size, face width, palm thickness, face length and face rise. Individual layers retain their own height, blend, opacity, masks, warps, bench flags and tool settings. Projected artwork is editable embedded 16-bit imagery; the original SVGs are also included as authoring references. Editing a reference SVG does not automatically reproject its derived full-ring map. Larger composition changes may need artwork repositioning.

Reproduce the compositions and bundle their exact graphs:

```sh
cargo run -p ringdesign-core --release --example stock_masterworks -- NEW_REVIEW_DIR
cargo run -p ringdesign-graph --release --example imported_templates -- NEW_REVIEW_DIR
cargo test -p ringdesign-graph --test imported_bases
```

See [the stock masterworks review](../showcase/stock-masterworks/README.md) for renders, geometry counts, release checks and workshop limitations. The original procedural showcases can still be regenerated with the `showcase_templates` example for the remaining catalog entries.

## What the supplied presets establish

The local tree contains **448 presets across 29 families**, including **20 signet bases**. It includes multi-rail/open rings, findings and cutters as well as closed bands; these cannot all be represented by the current single-surface band generator.

The signet OBJ caches split vertices at surface seams. Welding to eight decimal places in millimetres closes those seams; six files also contain collapsed or duplicate faces. Removing those faces produces **19 closed, consistently wound, single-component candidates**. Preset **013** retains a three-edge opening and is excluded from the cleaned copies. This is a topology screen, not a minimum-wall or mold-release result.

Ten signets specify different side and palm widths or thicknesses; the existing `cg_signet` example reads only the side dimensions. Cached meshes 007 and 009 also have a minimum vertex radius near 10.48 mm, versus roughly 8.54–8.58 mm for the others. This minimum is not a fitted nominal bore, but it establishes that a single assumed size cannot reproduce the whole collection. Future calibration needs separate face, shoulder and palm dimensions and a bore measured from each source.

Reproduce the measurements and write separate cleaned copies:

```sh
python tools/audit_preset_bases.py assets/decoded/presets --out target/preset-study --clean-signets
```

The report includes per-family parameter ranges, per-file topology before/after cleanup and source hashes. `target/preset-study/cleaned-signets/` contains the 19 candidates. Originals are untouched. Copies retain the source frame: finger axis **Y**, head **+Z**, units **mm**. RingDesigner's frame is finger **Z**, head **+Y**; the rigid conversion is `(x, y, z) → (x, z, -y)`, a −90° X rotation.

## Geometry-preserving import

Importing the finished surface preserves a good shoulder. Reconstructing its parameters through `cg_signet` still runs our procedural head generator and therefore cannot promise the original surface. The archived procedural Nocturne and Solstice used **Cut dome = 1**, which takes precedence over Loft. The current stock masterworks evaluate their imported surfaces instead.

The imported-base editor now stores the finished mesh inside `RingDesign.imported_base`. Preview, STL/3MF/OBJ/GLB export and CAD Band features use it. It is independent of the kernel-only `solid.import` node.

Open **Shape → Panel → Imported signet base** on Android, or **Design → Imported signet base** on desktop. Choose one of the 19 audited masters, adjust face length/width, head height, palm thickness or bore, and use **Reset master dimensions** to return exactly to the master. Existing decorations keep their chart positions; measured stones retain their dimensions and follow the stock's surface frame. Selecting stock bakes graph provenance; use Graph's lift command to create a fresh recipe afterward. Ordinary undo includes these changes.

**Base only** suppresses relief and stone preview without deleting the layer stack; it also exports bare stock. **Validate deformation** and **Compare sections** provide manual checks, with original and resized sections overlaid. A sharp edge already present in a master remains sharp.

**Support relief for sand withdrawal** is an optional Shape control. It constructs a radial supporting envelope toward the Z=0 parting plane, adds at most 1 mm of support, and gives the bore a small releasing relief while retaining its calibrated nominal diameter. Its actual output is used for preview and export. The source master and every layer remain editable. It is not a general automatic casting approval: inspect the Casting report again after any edit. Bare-base inspection shows the embedded master before this operation.

Sand masterworks embed a prepared version of the imported stock: a true parting loop, matched halves, and a drafted crown sampled on sufficiently fine source triangles. Their exported sand patterns omit only layers explicitly marked as bench operations.

## The supplied 3DM profiles

Inspected all **146 3DM files** in `assets/User/Profiles/` with `rhino3dm` 8.32.0; all were readable. The document's object type, closure, planarity, bounds, units and source hash are recorded per file.

| Folder | Files | Actual geometry |
|---|---:|---|
| Signet Profiles | 23 | One closed planar NURBS curve or polycurve per file |
| Ring Profiles | 19 | One closed planar NURBS curve or polycurve per file |
| Outside Ring Profiles | 23 | One closed planar NURBS curve or polycurve per file |
| Chain Elements, Ornaments, Patterns | 15 | B-reps; seven report solid, eight report open |
| Other profile folders | 66 | Curves for prongs, cutters, charms, gallery cuts and jalis |

The signet curves supply outlines, not complete head-to-shoulder surfaces. Twenty-two of the 23 signet files declare no units, and their coordinate sizes vary; an importer must fit them to explicit dimensions. The decoded signet OBJ presets supply the full surfaces needed for the first imported-base implementation. More assets are not a prerequisite.

Repeat the 3DM inspection without installing Rhino:

```sh
uv run --no-project --with rhino3dm==8.32.0 python tools/audit_3dm_profiles.py assets/User/Profiles --out target/preset-study/3dm-profile-audit.json
```

The [rhino3dm library](https://www.rhino3d.com/features/developer/rhino3dm/) reads geometry independently of Rhino. Reading a file does not supply RingDesigner with a deformation or modeling-history engine.

## Deformation and limits

Every evaluation starts from the unchanged master. Quintic smoothstep weights separate the protected bore, palm, shoulder transition and face. Bore changes move the face as a rigid translation while the lower shank changes radially; face edits blend through the shoulders. Ordinary relief subdivides original triangles conformingly, preserves the source's smooth normal field, and displaces along surface normals. The optional sand envelope resamples the native stock sections and adds withdrawal support. It does not rebuild the stock through the procedural head generator. The original artwork chart stays attached across face edits; the circumferential chart follows bore changes.

| Control | Allowed range from the master |
|---|---|
| Face length and width | 70–130% |
| Bore diameter | ±3 mm |
| Palm thickness | ±0.5 mm; minimum 0.8 mm |
| Head height | ±1.5 mm |

Combinations are additionally checked for sampled Jacobian inversion and valid radial sections. Source files must be finite, closed, consistently wound, connected meshes with valid indices and noncollapsed faces. The master limit is 200,000 vertices / 400,000 triangles; generated relief is capped at two million triangles. Source stock with multiple loops in a radial section is rejected. Procedural refinement tolerances are not supported on imported stock; choose a fixed mesh quality tier.

These are editing checks, not a proof against every possible self-intersection or a casting approval. Casting draft and section checks still apply; the field report samples a surface chart, while mesh draft analysis reads the actual triangles. Source bore calibration currently uses the minimum source vertex radius, not a fitted cylinder; polygon chord sag can put the actual opening a few hundredths of a millimetre below that reference. Confirm critical fit dimensions on the exported solid. A damaged or creased master needs repair before import.

The saved design format is now version 4, so older apps reject incompatible projects rather than silently replace their shoulders or omit sand support. Android keeps its last successful preview and reports a failed build; export failures return an error.

## Add more stock and reproduce the review

Package the audited local presets (original assets remain untouched):

```sh
python tools/package_signet_bases.py --ids 001 002 003 004 005 006 007 008 009 010 011 012 014 015 016 017 018 019 020
```

For another OBJ, supply a calibration JSON with `bore_radius_mm`, `face_length_mm`, `face_width_mm`, `palm_thickness_mm`, `head_height_mm`, `shoulder_start_mm` and `shoulder_end_mm`. These are millimetres in the app frame: finger Z, head +Y. Use `--crossgems-axes` for the supplied preset frame and `--scale` to convert file units. The importer welds seam duplicates and removes collapsed/duplicate triangles; it does not fill holes.

```sh
cargo run -p ringdesign-cli -- base import stock.obj --calibration calibration.json --crossgems-axes --out stock.ringbase.json
cargo run -p ringdesign-cli -- base attach design.ring.json --base stock.ringbase.json --out imported.ring.json
cargo run -p ringdesign-cli -- base attach design.ring.json --preset 015 --out imported.ring.json
cargo run -p ringdesign-cli -- base check imported.ring.json
cargo run -p ringdesign-cli -- base render imported.ring.json --bare --out bare.png
cargo run -p ringdesign-cli -- graph lift imported.ring.json --out imported.graph.json
cargo run -p ringdesign-graph --example imported_templates
cargo test -p ringdesign-core imported_base::tests
cargo test -p ringdesign-graph --test imported_bases
```

The shared app panel also accepts pasted `.ringbase.json` content. No external source path is required after import. `target/imported-base-review/renders/` contains procedural/imported/enlarged/bare comparisons and mesh reports. Automated checks cover all 19 masters at baseline and ±20% face sizes, exact reset, save/reload, OBJ seam welding/axis conversion, invalid-source rejection, rigid face movement during bore resizing, graph reload, stone dimensions and STL coordinates. The Android review covers loading, resizing, bare/decorated preview, validation, section overlays and reset.

## Detail and settings

Imported settings use a tangent plane on the actual stock so a millimetre-sized bezel fits its matching stone after face edits. Field sampling shares the native surface chart with stone placement. Higher detail tiers subdivide ornament regions more densely while retaining quieter stock areas.

Embedded height maps preserve 16-bit samples up to a 2048-pixel edge and two million pixels (8 MiB of f32 data per map). Oversized embedded maps resize in 16-bit precision; ordinary repeating-image imports retain their 512-pixel limit. This avoids reducing a complete ring's fine ornament to a tiny patch of a 512-pixel-wide map after save/reload.
