# Tenebrae enablers C-T1, C-T3, C-T4: cloud report

Branch `claude/tenebrae-enablers` from `master` at `a66879e`: `8246c5a` (C-T1), `67a3493` (C-T3),
`0905b88` (C-T4), then this report. Pull request: https://github.com/shadowbrok3r/RingDesigner/pull/239
Every enabler is opt-in: nothing existing changes shape, and every new saved form is fenced at design
format 6 / graph format 2 without a new version.

## C-T1: tracery from a net, and `Profile::Regions`

**What landed**
- `Sketch::tracery(net, bar_mm) -> Tracery { lights, skipped }` (`sketch/edit.rs`). It composes the
  proven calls: `split_at_intersections` on the net alone (every other drawn curve stands aside as
  construction while it splits, so the rest of the sketch is neither split nor moved), the cells from
  `profile_regions`, each rim offset in by `bar/2` and each hole out by as much, then the net marked
  construction. A cell whose offset folds is left out whole and named in `skipped` with the reason. A
  bar no cell takes is refused, and the sketch is left unchanged.
- `Profile::Regions { feature, regions }`, an untagged arm placed before `Feature`. `regions_of` takes
  every picked region once (a pick named twice counts once), and an empty list is refused. A twisted
  sweep, a loft and a sweep take one region and refuse several by name. Wired through every exhaustive
  match (workbench grips, GUI sketch mode, and the GUI profile source, which shows "regions 1, 3 of 5").
- Fence: `cad::picks_regions` / `picks_regions_json` (document and graph JSON, clusters included) join
  `library::format_version_for` and the graph writers' `fenced_json`. An older build would read
  `{feature, regions}` as `Feature` and sweep every region.
- Exposure: a `sketch.tracery` graph node takes a Sketch operation or a bare sketch, the net's ids
  (empty takes every drawn curve) and the bar. It returns the operation, how many lights, and each
  skipped cell with its reason. MCP reaches it through its graph tools.
- CLAUDE.md: the fence sentence, and a "Tracery is drawn from a net" bullet under the CAD rules.

**Tests**
- `sketch::edit::tests::a_polar_net_traces_to_one_light_per_cell_each_a_bar_from_its_neighbours`:
  a 24-cell polar net at bar 0.9 gives 24 loops, and each loop's gap to its neighbour round the wheel is
  0.9 ± 1e-6. The test also checks that the net ends as construction and a circle beside it is untouched.
- `sketch::edit::tests::a_cell_too_narrow_for_the_bar_is_skipped_whole_and_a_bar_no_cell_takes_is_refused`
- `cad::tests::several_regions_of_one_branched_sketch_extrude_together_and_read_back_as_regions`:
  the two outer cells of a three-cell box extrude as (12 + 16) × 2 mm³, the whole branched sketch is
  refused, and each profile shape round-trips through untagged serde as itself.
- `library::tests::several_picked_regions_write_the_design_at_six_and_one_stays_at_five` (document
  and graph), and `nodes::cad::tests::a_tracery_node_draws_one_light_per_cell_of_a_sketch_operation`.

## C-T3: Gothic cutter shapes, the outline library and the artwork set

**What landed**
- `cutters::Shape` gains Lancet, Ogee, Trefoil, Quatrefoil and Mouchette, and `PIERCE_SHAPES` lists all
  ten, which reaches the inspector's choice and the `cad.op.cutter.pierce` node. The workbench's
  right-click "Cut here" list (`PIERCE_KEYS`) carries all ten on desktop and phone. `pierce_at` sizes
  each one, and an arch's point and a trefoil's lobe stand away from the bore.
- The bright cut insets concave cusps. Each Gothic plan is drawn dense, read on fixed rays from a centre
  it is star-shaped about (each ray turned onto the nearest point or cusp), and grown along those rays by
  the true Minkowski offset of the drawn plan. The point count never changes, the fan never folds, and a
  cusp moves straight out along its own ray. The five older plans are untouched.
- Fence: a piercing with a Gothic shape is `geometry_extended`, so it is written at 6 and graph 2. An
  older reader would otherwise cut it as a Round.
- Library: `bundled/sketches/gothic/*.svg`, 16 pieces, all in `import_svg`-clean form. The
  `ringdesign-assets` `SKETCHES` family is swept through subfolders and named by path (for example
  `gothic/fleur-de-lis`). `library::list_sketches()` / `list_sketches_in(dir)` / `sketch_dir()` lay the
  user's `sketches/` folder over the bundled set by name, like `list_outlines`. A `sketch.library` graph
  node serves any of them by name, with a scale.
- Outlines and nets: gallery-ogee, gallery-quatrefoil, gallery-cusped-lozenge,
  ornament-quatrefoil-ring, and the four jalis (lozenge, quatrefoil, honeycomb, intersecting arches) as
  centre lines for `tracery`.
- Artwork: fleur-de-lis, fleur-cresting, crocket-leaf, nave-arcade (three lancet bays),
  gargoyle-silhouette, gargoyle-face (an eye, a brow, a nostril and a fang: Logan now allows faces),
  memento-mori (crossed bones under an open hourglass) and cross-pattee.
- Tools: `tools/author_gothic.py` draws the set, with exact lines and arcs for the geometric
  pieces and shapely polygons for the figurative ones. `tools/harvest_gothic.py` (rhino3dm) is the
  3DM harvester; it was smoke-tested here on synthetic 3DM files (a line+arc polycurve, a circle, a
  B-rep box).
- CLAUDE.md: a "Gothic piercing grows by a true offset" bullet, and the `SKETCHES` family in the assets
  section.

**Tests**
- `cutters::tests::a_gothic_plan_grows_by_a_true_offset_so_its_cusps_inset_instead_of_folding`: every
  grown point stands exactly `g` off the drawn plan (1e-6). The ray reading keeps the drawn area, so each
  plan is star-shaped about its centre. A quatrefoil's cusp moves out along its ray, and each point or
  tip lies at −x.
- The existing outline, crown-piercing (volume to 3%), blind, side-face and edge tests now run all ten
  shapes. The side-face test also pins which way the new points face.
- `library::tests::every_bundled_gothic_sketch_sweeps_its_area_or_traces_its_lights`: every file
  imports. Each outline sweeps the area recorded on its root (1e-5). Each net traces to the recorded light
  count with none skipped. A user file overlays a bundled one and a new name joins the list.
- `nodes::cad::tests::a_library_sketch_feeds_tracery_and_names_the_library_when_it_is_missing`, and the
  assets crate's round-trip, length, name and SVG checks now cover `SKETCHES`.

**Could not do**
- The nine harvested pieces (Under Gallery Cuts 001–003, Jalis 000/002/010/016, Ornaments 027/028)
  are drawn stand-ins. `assets/User/Profiles/` is git-ignored (`.gitignore` line 3: `assets/`). It is not
  on master or any branch, so the 3DM files were not in this checkout. Each stand-in says so in its
  `<desc>`. Run `uv run --no-project --with rhino3dm==8.32.0 --with shapely python
  tools/harvest_gothic.py assets/User/Profiles` on the workstation to replace them under the same
  names, then rerun `every_bundled_gothic_sketch`. The file matching (folder keyword plus number) is a
  guess at the folder names; `--dry-run` shows what it would take.
- The gargoyle pieces are a serviceable first pass and have not been through a render review. Hold them
  to that bar, and cut the face variant if it does not read at size.

## C-T4: DFM land width for CAD cuts

**What landed**
- `dfm::cut_lands(design, built, floor_mm) -> Vec<DfmFinding>`, with `CUT_LAND` as the label and
  `dfm::PART` as the layer sentinel. For every Cut extrusion it reports the narrowest land in three
  places: between two of its regions, between it and each copy a Pattern makes, and to the band's or host
  part's edge. Each kind is reported once when it falls under the floor, for example
  `Cut #3 'Pierce the lights': 0.60 mm between lights 1 and 2 (floor 0.8)`.
- How it measures:
  - Region lands are measured between outlines in the sketch's plane (`cad::extruded_regions` gives the
    plane as built, face-anchored sketches included), and carried to copies by their copy motions.
  - The edge land is walked out from each outline in the plane until a line along the normal, within the
    cut's reach, meets no metal in the built ring.
  - Where that line runs through a copy's opening instead, the land is booked to the copy. A ring of
    copies converges toward the bore, so its land at the metal is narrower than in the plane.
- It only runs when asked, so nothing existing changes. It is reachable through `ringdesign export
  --cut-land <mm>` and MCP `manufacturing_check { cut_land_mm }`, which adds `cut_lands` to the report.
- CLAUDE.md: a "CAD cut's lands" paragraph beside the made-part lands.

**Test**
- `dfm::tests::a_cut_names_the_narrowest_land_between_its_lights_its_copies_and_the_edge` covers five
  cases on a Court band:
  - Two 1 mm lights 0.6 mm apart report exactly `0.60 mm between lights 1 and 2 (floor 0.8)`.
  - A lower floor stays silent, and so does the design's own report.
  - Lights a full floor apart pass.
  - A light 0.5 mm in from the side reports 0.5 ± 0.06 mm to the edge.
  - A ring of 48 copies reports a copy land.

**Could not do**
- Revolve and sweep cuts are not measured; only extrusions have a plane to measure in.

## Checks run

All on the final tree, with rustc 1.98.1. The workstation's `systemd-run` guard and `--offline` were
not used here, as TASK.md says.

- `cargo test -p ringdesign-core`: 839 passed, 0 failed, 16 ignored. `tests/golden.rs` passed.
- `cargo test -p ringdesign-graph --no-fail-fast`: 109 lib tests passed, plus `bestiarium_templates`,
  `cad_edits`, `imported_bases`, `showcase_templates`, `template_nodes` and the `collection_templates`
  example (25 more), 0 failed. This includes the struct-coverage and table-consistency tests for the two
  new nodes.
- `cargo test -p ringdesign-assets`: 4 passed.
- `cargo check --no-default-features --target wasm32-unknown-unknown -p ringdesign-core`: clean.
- `cargo check --tests` of graph, workbench, gui, mcp, cli and the Android app: clean. The only warning
  is the existing `COMFY_GATE_KEY` build note.
- Spot suites for the touched exposure points:
  - workbench `viewport::cutters`/`menu`/`grips`: 12 passed. Every one of the ten right-click keys plans
    on a Court band.
  - gui `cutter`/`sweep`: 13 passed.
  - `ringdesign-mcp --lib`: 45 passed.
- Commits `8246c5a` and `67a3493` were each checked on their own (core, graph, workbench and gui, plus
  assets for C-T3), so the history bisects.
- The full workspace test run was not done; I ran the suites TASK.md names plus the crates whose code I
  touched.

Housekeeping: the 30 GB disk allowance ran out once, mid-run, from the example binaries under
`target/debug/examples` (20 GB). I deleted them and reran that step. `tools/harvest/` is git-ignored by
design ("never tracked"), so the two new scripts live at `tools/author_gothic.py` and
`tools/harvest_gothic.py`, beside `audit_3dm_profiles.py`. I stayed out of
`crates/ringdesign-core/examples/tenebrae_*`.
