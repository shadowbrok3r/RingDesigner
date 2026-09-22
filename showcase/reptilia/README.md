# Reptilia

Five editable rings in **File → New → Reptilia collection** on desktop and Android. Open `index.html` for the gallery or `Reptilia-collection.png` for the collection sheet. `Reptilia-final-renders.zip` contains all 15 views and the sheet.

| Preset | Form | Surface | Approximate cast silver |
|---|---|---|---:|
| Ecdysis | 8.5 mm band | Broad ventral plates blended into fine snake scales | 11.57 g |
| Tessera | 9 mm band | Hexagonal shields with broad chevrons | 12.38 g |
| Lorica | 8 mm band | Crocodile scutes, low keels and shallow grain | 10.68 g |
| Ophidian | Factory signet 013 | Keeled snake scales; purple 7 × 5 mm oval amethyst | 10.98 g |
| Varanus | Factory signet 017 | Crocodile plates blended into shield scales; no stone | 19.10 g |

Nominal bore: 18.6 mm. Signets retain the imported factory forms. Silver, darkened recesses and stone colour are intended finishing choices. Studio images render the exported geometry in Blender; `hero.png`, `face.png` and `palm.png` use RingDesigner's renderer.

Each folder contains `design.ring.json`, `editable-graph.ring.json`, the artwork, actual finished-metal STL, a separate shrink-compensated casting-pattern STL, and geometry/manufacturing reports. The amethyst mesh is a separate reference solid and must not be cast. The four tiles are also built into the app's procedural pattern library: Snake keels, Ventral scutes, Crocodile scutes and Reptile shields.

All five finished meshes are watertight, with no boundary edges, non-manifold edges or degenerate triangles. Saved designs and their editable graphs rebuild identical vertices, faces and normals from empty alpha libraries. `verification.json` records each round trip. Patterns have zero sampled withdrawal obstructions and zero unresolved rays at 0.100 and 0.075 mm; the latter is recorded in `release-fine.json`.

The manufacturing verdict remains **Review**: low-draft regions and a fine-detail advisory remain on every design. Cast the supported sculpture; remove the explicitly marked chasing stock at the bench to reveal the finished scale joints and facets, then cut Ophidian's bearing and set the stone. The supplied pattern already includes the 1.9% shrink allowance; do not apply it twice. Shop calibration, local thickness review and a physical pull trial are still required. These rings have not been physically cast.

Reproduce in a fresh directory from the repository root:

```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- env RUSTUP_FORCE_ARG0=cargo /home/shadowbroker/.cargo/bin/cargo run --offline --release -p ringdesign-core --example reptilia -- NEW_DIRECTORY
systemd-run --user --scope -p MemoryMax=4G --quiet -- env RUSTUP_FORCE_ARG0=cargo /home/shadowbroker/.cargo/bin/cargo run --offline --release -p ringdesign-core --example reptilia -- NEW_DIRECTORY --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- env RUSTUP_FORCE_ARG0=cargo /home/shadowbroker/.cargo/bin/cargo run --offline --release -p ringdesign-graph --example reptilia_templates -- NEW_DIRECTORY
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_reptilia.py -- NEW_DIRECTORY
python3 tools/catalog_reptilia.py NEW_DIRECTORY
```

The graph packager also refreshes the five bundled templates in `graphs/templates`. Add `--draft` to the author or renderer for lower resolution. The author refuses to overwrite an existing design folder; the renderer's `--resume` keeps images already completed.
