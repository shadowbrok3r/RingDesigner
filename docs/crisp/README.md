# Crisp edges

The collection reviewers failed "figurative motifs have true outlines that render crisp, with no combing, smearing or stair-steps" on nearly every ring. Four platform causes sit behind it. All four were reproduced from the rings' own `design.ring.json` at export resolution (1536 x 448) and 1600 px renders.

`crates/ringdesign-core/examples/crisp_probe.rs` rebuilds every crop here. Each image puts the branch's own render (before) beside the same camera on this branch (after). The `synthetic-wall` before is master.

| Artifact | Ring, render | Cause | Fix | How a ring turns it on |
| --- | --- | --- | --- | --- |
| Stair-steps along every stamp margin, vein cut, calyx and part wall in close-ups; a "torn foil" leaf; a stepped rim round the picture | Ilex `stones.png`; Rubus `crown-close.png`, `stones.png`, `cane-close.png` | (c) Each example cropped the mesh to a sphere or wedge before rendering. The copy kept whole triangles, so its rim is the build grid, and it dropped `Mesh::corner_normals`, so every boolean crease shades as one smooth roll whose fringe follows the band's triangles. | `render::Framing`, `render_parts_framed`, `write_png_framed`, `yaw_facing`: frame the camera on a point and draw the whole ring. Render only. | Replace the crop and `write_png_parts` with `render::write_png_framed(path, &fin.parts(GOLD), yaw, pitch, Framing::new(centre, half_width_mm), edge)`. For a radial look at ring angle `theta`, `yaw = yaw_facing(theta)`, `pitch = PI / 2`. |
| Stepped highlight blocks across the shank; striations on the head's slopes | Ilex `reverse.png`, `shoulder.png` | (d) The sand master is the factory mesh refined by midpoint splits into 0.55 mm flat facets. The envelope took its shading normals from its own grid, smoothed over five cells (about 0.1 mm), so each facet shaded flat. | The envelope's stock normals are blurred over 0.45 mm of surface, each walk stopping where the normal has turned 12 degrees, and the relief's normal delta is added back as before. Render only: no vertex moves. | Automatic. |
| Crumpled, notched face-leaf tops, zigzag folds running to every spine, a ragged lower margin | Ilex `face.png`, `stones.png` | (b) `StampTop::Dome` divides by the reach along the ray from the crown. On a non-convex outline that reach kinks at every spine and jumps where a spine shadows the ray, so the top folds along rays that the cap triangulation does not hold, and each fold zigzags across the cap's grid. | `StampTop::Pillow { crown_mm }`: a membrane pressed up from the outline, solved on a fine triangulation of the outline itself. It equals `Dome` on a circle and has no crease on any outline. Opt-in; a design carrying one is written at format 6, and a graph carrying one at 2. | `top: StampTop::Pillow { crown_mm }` in place of `Dome` on leaf bodies, petals and cushions. Keep `Dome` or `Flat` on thin strokes such as vein cutters. |
| Walls that step a row at a time where they cross the sweep grid obliquely; crinkled bead and crest highlights | Heloderma `side.png`, `palm.png` band borders; the synthetic star | (a) The height field is point-sampled at each sweep vertex. A wall narrower than one cell lands on whole rows and columns, whatever the alpha's own resolution. `BuildParams::refine` does not remove it: the quadtree stops at its own depth and leaves the same steps irregular. | `RingDesign::crisp_relief`: where the relief is not linear across a cell, the sweep reads it through a tent filter one cell wide either way, matching the mesh's own linear interpolation. Walls lie straight, about two cells wide. Opt-in; written only when set, and fenced at design format 6 and graph format 2. The field verdict still reads the true surface. | `d.crisp_relief = true` on the design. It applies at every swept resolution, in the procedural sweep and both stock paths; refined builds keep their own sampling. |

## What the platform does not cause

- **Castellated serrations (Rubus).** The side-face leaves are extrusions with vertical walls. Seen at a grazing angle, a serrated outline with vertical walls reads as battlements. A stamp with a `Pillow` top, or an extrusion with draft, gives the leaf a modelled surface.
- **Stepped bead-field edges (Heloderma).** The fields are masked on the bead lattice, so their edges step by one bead.
- **Lumpy blackberries (Rubus).** The berries are stored sculpt meshes, shaded on their own tessellation.
- **Tile seams (e).** None appear in the current renders. Moloch's round-1 seams were in the tile itself, and Heloderma's sector decals already overlap by 0.6 mm.

## Crops

| Crop | Shows |
| --- | --- |
| `ilex-close-up-veins.png` | Face leaf and vein cuts in the stones close-up: cropped (dropped crease normals) against framed, with `Pillow` on the leaf |
| `ilex-close-up-rim.png` | The close-up's rim: the cropped mesh's grid staircase against the whole ring framed |
| `ilex-face-leaf.png` | Right face leaf in `face.png`: `Dome` against `Pillow` |
| `ilex-shank.png` | Shank side face in `reverse.png`: facet banding against the smoothed stock shading |
| `rubus-cane-close.png`, `rubus-crown-close.png`, `rubus-stones.png` | Rubus's three wedge close-ups, cropped against framed alike |
| `heloderma-side.png`, `heloderma-palm.png` | Band borders and bead fields with `crisp_relief` off and on |
| `synthetic-wall.png` | A star decal's walls at 1:1 in a full 1600 px view, point-read against `crisp_relief` |

## Tests

- `render::framed_tests::a_framed_close_up_is_metal_edge_to_edge_where_the_band_fills_it`
- `imported_base::tests::a_sand_masters_shading_hides_its_facets_and_follows_its_shape`: on preset 006, the 90th-percentile change of turn falls from 0.79 to 0.21 degrees a column, and the shading stays within 12 degrees of the surface at the 99th percentile.
- `setting::tests::a_pillow_is_the_dome_on_a_circle_and_creaseless_on_a_holly_leaf`: within 0.01 mm of `Dome` on a circle; under a quarter of `Dome`'s worst second difference on a holly leaf; strikes closed and is fenced.
- `mesh::crisp_tests::crisp_relief_lays_an_oblique_wall_straight_and_leaves_the_verdict_alone`: the wall's half-height line strays from straight by under 0.4 of the point-read figure, and the field verdict is identical.
