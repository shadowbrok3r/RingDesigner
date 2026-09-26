# Bestiarium

Two creatures in **File → New from template → Bestiarium** on desktop and Android. Open `index.html` for the gallery or `Bestiarium-collection.png` for the collection sheet. `renders/` holds the eight studio views and the sheet; the catalog step also packs them as `Bestiarium-final-renders.zip`.

| Ring | Form | Surface | Stones | Approximate cast 18k gold |
|---|---|---|---|---:|
| Arachne, *the weaver* | Keyframed low dome 5.6 × 2.4 mm, US 7 | A spider at the hub of her orb web; eight jointed legs clasp the band and the silk runs round the shoulders to the palm | Onyx 9 × 7 mm and garnet 5 × 3.5 mm oval cabochons in collets | 15.79 g |
| Manticora, *the tail that throws* | Keyframed high dome 5.4 × 2.2 mm, US 8.6 | A scorpion's tail; graded tergites, pleural folds and quills rise to a venom bulb, and the hooked sting curls over the stone | Ruby 7 × 5 mm oval in a beaded collet; ten graded black spinel princesses | 17.12 g |

Both rings are cast in lost wax and judged against the investment recipe: 0.8 mm fill floor, 0.15 mm detail. The `studio*.png` images render the exported meshes in Blender; `hero.png`, `face.png`, `palm.png`, `side.png`, `shoulder.png`, `reverse.png`, `stones.png` and `bare-vs-finished.png` use RingDesigner's renderer.

## Each folder

- `design.ring.json`: the editable source (format 6; each stored part is carried once).
- `editable-graph.ring.json`: the design with its graph, as the menu opens it. `template.graph.json` is the bundled graph, byte for byte `graphs/templates/<slug>-bestiarium.graph.json`.
- `artwork/`: Arachne's silk as SVG; Manticora's painted masks as PNG.
- `finished-metal.stl`: the finished ring at nominal size.
- `casting-pattern.stl`: the investment pattern, shrink-compensated for Gold 18k (1.3%). Do not apply the shrink again. Manticora's aculeus is its own casting and is not in the pattern; `aculeus.stl` is the sting at nominal size.
- `reference-*.stl`: the stones, for reference only; never cast them. `stones.json` gives their render materials.
- `report.json`: every gate. `mesh.json`: mesh statistics and metal weights. `pattern-report.json` (Arachne): the pattern setup.
- `verification.json`: the cold reload and the template gate (bytes, patches, nodes, open times).

STL, ZIP and Blender files are not tracked; the commands below rebuild them.

## Verification

Measured at 1536 × 448; every gate passes.

| | Arachne | Manticora |
|---|---|---|
| Finished mesh | 1,323,510 triangles; watertight; 0 degenerate faces; 0 self-crossings | 1,372,332 triangles; watertight; 0 degenerate faces; 0 self-crossings |
| Made parts, stamps, seats | 0 crossings; no solids or parts notes | 0 crossings; 24 of 24 stamps and 10 seats resolved; no notes |
| Finger hole | clear | clear |
| Field verdict (lost wax) | Castable; thinnest wall 1.88 mm | Castable; thinnest wall 1.43 mm |
| DFM findings | 0 | 0 |
| Stones reported / previewed | 2 / 2 | 11 / 11 |
| Cold reload, empty library | identical | identical |
| Template graph | 137 nodes, 0 patches, 671,142 bytes | 114 nodes, 1 patch (`/cad/joints`), 1,244,173 bytes |
| Template, cold at 1536 × 448 | source and mesh identical | source and mesh identical |

Both templates carry embedded artwork and stored parts, so they are judged in the painted class (3 MB budget). Neither exposes a control: the painted relief and the stored parts are bound to the body they were made for. Open times are in each `verification.json`.

## Bench notes

- **Arachne:** invest the body and limbs together. Clean the investment from the web and limb clearances. Finish the two collet bearings to the actual garnet and onyx, then set.
- **Manticora:** invest the tail with its collet, knob, seats and quills; cast the aculeus apart. Bead-set the spinels, set and burnish the ruby, then solder the aculeus into the knob's groove. The graver lines are cut at the bench. The hook clears the collet by 1.2 mm and the ruby by 1.6 mm, so it can catch knitwear and hair: put that on the care card.

Shrink allowances are starting values; confirm them with the caster's alloy and trials. Neither ring has been physically cast.

## Reproduce

In a fresh directory, from the repository root:

```sh
cargo build --offline --release -p ringdesign-core --example bestiarium_arachne --example bestiarium_manticora
cargo build --offline --release -p ringdesign-graph --example collection_templates
mkdir -p NEW_DIRECTORY && cp showcase/bestiarium/collection.json NEW_DIRECTORY/
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_arachne NEW_DIRECTORY/arachne --verify
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_manticora NEW_DIRECTORY/manticora --verify
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/collection_templates bestiarium NEW_DIRECTORY --verify-export --check-bundled --templates-dir NEW_DIRECTORY/templates
systemd-run --user --scope -p MemoryMax=8G --quiet -- env CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_collection.py -- NEW_DIRECTORY --collection bestiarium
systemd-run --user --scope -p MemoryMax=4G --quiet -- python3 tools/catalog_collection.py NEW_DIRECTORY --collection bestiarium
```

`--check-bundled` fails unless each graph matches the bundled template byte for byte. Without `--templates-dir`, the packager rewrites the bundled templates in `graphs/templates` instead. Add `--draft` to a ring example (768 × 320) or to the renderer (first view, fewer samples) for a quick check. The menu thumbnails come from each ring's `hero.png`: `python3 tools/template_thumbnails.py arachne-bestiarium manticora-bestiarium`.
