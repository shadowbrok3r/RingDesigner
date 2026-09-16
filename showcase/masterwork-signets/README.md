# Solstice & Nocturne

Two original signets authored with RingDesigner’s existing profile, layer, artwork, stone-setting and manufacturing tools. Open `index.html` to inspect each view, compare the undecorated structure and casting pattern, play the turntable, and download the source.

All imagery renders actual geometry. Green is the intended stone colour; polished metal and satin/oxidized recesses are finishing choices. Neither ring has been physically cast.

| Design | Route | Approximate cast metal | Editable layers | Radial wall screen |
|---|---|---:|---:|---:|
| Solstice | Sand casting | 20.40 g | 11 | 2.844 mm |
| Nocturne | Investment casting | 22.73 g | 22 | 2.409 mm |

## Solstice

Withdraw the two mold halves along ±Z. Cast the complete solar, lotus, fluted and beaded ornament. Finish the fine satin texture at the bench. The solar alpha includes supports toward the parting line so its rays do not trap sand.

Zero sampled withdrawal obstructions at 0.100 and 0.075 mm; no detail-size warnings. Physical pull trial required.

The additional coarse surface-normal screen sampled 384 rays: minimum 0.633 mm, 9 below the 0.80 mm general limit. Inspect these edge/detail locations in the source; this is a separate measurement from the radial wall screen.


## Nocturne

Cast the metal body in one piece from a sacrificial pattern. The gallery tool creates deep recesses with retained floors, not through-holes. Set one 2.3 mm and two 1.5 mm round stones after cutting actual bearings and pavilion clearance. Engrave NOCTURNE on the palm at the bench.

Investment pattern; three separate stones. 9 fine-detail advisories remain for the caster.

The additional coarse surface-normal screen sampled 384 rays: minimum 0.517 mm, 59 below the 0.80 mm general limit. Inspect these edge/detail locations in the source; this is a separate measurement from the radial wall screen.

## Files and reproducibility

- `design.ring.json`: nominal source; SVGs, generated patterns, brush strokes, text, masks and setting data travel with it.
- `nominal.stl` / `nominal.3mf`: finished metal geometry. Stones are excluded.
- `pattern-package/` and its ZIP: shrink-compensated casting mesh, complete source, recipe, report, mold/feeding diagram and process sheet. Do not compensate these files again.
- `verification.json`: both saved sources rebuilt identical nominal and pattern triangles using empty alpha libraries.
- `release-fine.json`: Solstice’s final 0.075 mm withdrawal screening.
- `wall-screen.json`: additional sparse local-normal screening on a coarser mesh; inspect flagged small edges and details. The radial screen is not a guarantee of minimum thickness everywhere.
- `artwork/`: original SVGs and lossless 16-bit alphas, including withdrawal stock and engraving masks.
- `preview-metal.glb`: lighter metal-only 3D preview. Production meshes retain the full detail.
- `turntable.gif`: actual metal and reference-stone geometry, rendered by the app.

Rebuild in a new output directory:

```sh
RUSTUP_FORCE_ARG0=cargo /home/shadowbroker/.cargo/bin/cargo run -p ringdesign-core --example masterwork_signets -- NEW_DIRECTORY
python tools/catalog_masterwork_signets.py NEW_DIRECTORY
rsvg-convert -o NEW_DIRECTORY/contact-sheet.png NEW_DIRECTORY/contact-sheet.svg
rsvg-convert -f pdf -o NEW_DIRECTORY/contact-sheet.pdf NEW_DIRECTORY/contact-sheet.svg
```

Use `--draft` for faster previews, `--sand` or `--wax` to author one design. The generator never overwrites an existing output directory. App-derived alphas are quantized once to their saved 16-bit representation before meshing, so saved files rebuild exactly.

Creating these rings exposed a stone-preview bug: relief changed the arc-length walk and tilted gems onto pocket walls. Stone previews now share the setting report’s coordinate frames; the new asymmetric-relief regression and four existing gem tests pass. The desktop binary was rebuilt.

Recheck existing geometry with `cargo run -p ringdesign-core --example masterwork_signets -- showcase/masterwork-signets --verify`. The `app-view.png` files show both projects open in the desktop app; neutral blue stones in that viewport correspond to the intended green stones in the gallery.
