# Bestiarium revival brief (shared by every revived ring)

You own ONE ring of the Bestiarium (the dark-mythology bestiary, one creature per ring), the first of Logan's four master collections in RingDesigner. Two of its nine rings shipped (Arachne, Manticora); yours was cut close to the bar and is being revived. The lead session owns master, packaging and the other rings.

## Read first
- `docs/collections/bestiarium.md`: your ring's section and the shared method. Every platform piece its status table lists has landed except C-B1 and C-B2, which a lane is landing tonight.
- `docs/collections/README.md`: gates, the review loop, the template gate, working rules.
- `lessons.md` in your seed: why the Bestiarium cut six of nine rings. It changes how you work.
- CLAUDE.md is the casting doctrine. Search it (skin, `Hide`, `draft_clamp`, the sand master, stamps, side faces) rather than reading all of it.

Binding taste (Logan): one theme face to palm, nothing scattered; figurative motifs as stamps or parts with true outlines, or painted relief whose outlines render crisp (painted motifs used to comb into "arrows"); stones in visible made settings, never height-field prongs; no flat, blocky CAD; factory stock keeps its hard wall-to-face angles; density and legibility at least Caiman's; no repeat of the Reptilia tiles. Faces and eyes are allowed when they read well.

Process: lost wax, as the ring was built: `CastProcess::LostWax`, `min_section_mm` 0.8, and every section under 0.8 mm removed or named with its bench treatment in a `land_widths` block.

## Where you work
- ONE example file: `crates/ringdesign-core/examples/bestiarium_<slug>.rs`. Artwork under `crates/ringdesign-core/examples/bestiarium/art/<slug>/`. Outputs in `showcase/bestiarium/<slug>/`.
- No core (`src/`) edits: write any core change you need into your report as exact code, and work around it in your example.
- Build: `cargo build --release -p ringdesign-core --example bestiarium_<slug>`. Run: `target/release/examples/bestiarium_<slug> [OUT_DIR] [--draft] [--verify]`, default output `showcase/bestiarium/<slug>/`. `--draft` is 768 x 320. The default export build is 1536 x 448 and must fit 2 million triangles. `--verify` reloads the saved design cold with an empty alpha library and requires identical vertices, faces and normals.
- Outputs: `design.ring.json` (lean), `report.json` (every gate number, with a `draft` block), `finished-metal.stl`, `casting-pattern.stl` (`mesh::try_build_pattern`: cuts and heads left out, raised drill dots), `reference-<stone>.stl` and `stones.json` for each stone, and studio-gold renders with stones set (`render::finished`): `hero face palm side shoulder reverse stones bare-vs-finished` `.png`, plus `hero-300.png`, `face-300.png` and a `contact-300.png` sheet. STL files are git-ignored under `showcase/`; PNG, JSON and SVG are tracked. `examples/bestiarium_arachne.rs` and `examples/stock_masterworks.rs` (Caiman) show a complete renders and gates block.

## Gates (all green before a round is reviewed)
1. Finished mesh watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and every made part; `built.solids.notes` empty; every stamp resolved.
2. Nothing enters the finger hole: every vertex at least the bore radius minus 0.01 mm from the finger axis, outside the comfort roll.
3. Lost wax: `cad::measure::thickness(&mesh, 0.8)` clean, or every section under 0.8 mm named in `land_widths` with its bench treatment.
4. `dfm::findings_in(&d, &lib)` returns 0 findings.
5. The `stones::report` count equals the gem preview count; the crowding census clean or explained.
6. `--verify` passes at the export build.
7. The casting-pattern mesh is watertight with 0 degenerate faces and 0 crossings.
Where the ring carries stamps, run the gates at 384 x 192 too: phantoms move with resolution; real obstructions converge.

## Template gate
```sh
cargo build --release -p ringdesign-graph --example collection_templates
rm -rf target/tpl-src target/tpl && mkdir -p target/tpl-src
cp -r showcase/bestiarium/<slug> target/tpl-src/<slug>
printf '{"collection":"bestiarium","rings":[{"slug":"<slug>","exposed_controls":[],"template_class":"painted"}]}\n' > target/tpl-src/collection.json
target/release/examples/collection_templates bestiarium target/tpl-src --output-dir target/tpl --only <slug> --verify-export
```
(`template_class`: `procedural` for an unpainted node-built ring, `stock` for bare factory stock, `painted` otherwise.) Record the `design.set` patch count (4 at most), the graph size against the class budget (300 KB, 1 MB, 3 MB), and whether cold source and mesh parity passed.

## Final report
The verdict and score per round, every gate at draft and export, the template-gate numbers, what each layer, stamp, part and stone is and why, what you could not do, and core changes wanted as exact code.
