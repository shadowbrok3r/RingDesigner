# Phoenix — authoring checkpoint

First independent art review pending. These are draft meshes (768 × 320), not a ship decision; export-resolution checks and the 7.5/10 art gate remain.

The fire opal is an 8 × 6 mm cabochon in a made collet. Two solved trails keep seven orange sapphires each, 2.124–1.223 mm in this composition. All 15 stones appear in the renderer’s disconnected-component census. The closest pair leaves 1.951 mm at the girdle and 1.822 mm at depth. Pad-stock bridges are 0.92 mm: above Delft’s 0.80 mm fill floor, below its 1.20 mm safer recommendation. The report preserves this bench caution.

The measured design differs from the starting brief:

- A squared LowDome with a 1.4 mm crown replaces the near-vertical Flat crown, whose contour relief lost 0.371 mm in the draft clamp.
- Crown relief is 0.24 mm. Contour pitches run 4.4–3.4 mm; smaller pitches failed the 0.30 mm detail floor. The last clamp removes at most 0.004750 mm.
- The ember windows extend over the shoulders to keep seven stations per trail. The original windows kept only three each. Requested bridges increased from 0.75 to 0.90 mm.
- The opal’s pattern boss is domed. The collet, stone cuts and beads are finished at the bench; the pattern carries stock and drill dots.
- Wings and streamers are true-outline quill stamps conformed to each keyed cheek. A curved raptor profile and swept crest meet each wing. Six central rachis cuts and two eyes are bench operations; the crown also carries shallow bench barbs.

Draft gates are recorded in `draft-gates.json`: closed mesh, zero degenerates, zero final self-crossings, empty solids notes, 103 clean raw solids, DFM zero, own-process Castable, zero obstructions and unresolved rays at 0.10 and 0.075 mm. All 32 stamp projections have byte-identical topology and coordinates with and without the crest settings, validating their separation from the settings’ build phases. Empty-library reopen reproduces vertices, faces and normals exactly.

The software images use smooth studio shading; the source mesh retains its original corner normals. `studio-draft.png` is a Cycles material proof of those same exported triangles. Cast material is Silver 925 in Delft clay; the gold images are the requested presentation finish.

No dimension controls are claimed safe. The painted atlas and conformed outlines bind this composition to its authored body; the lead owns packaging and exposed controls.

Reproduce after an unguarded release build:

```sh
cargo build --offline --release -p ringdesign-core --example bestiarium_phoenix
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_phoenix OUT --draft --verify
```

Omit `--draft` for 1536 × 448. `--layout` writes the cold source and measured stone census without a mesh build.
