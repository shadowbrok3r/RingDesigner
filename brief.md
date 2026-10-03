# Tenebrae ring brief (shared by every ring session)

You own ONE ring of Tenebrae (the Gothic cathedral collection), the third of Logan's four master collections in RingDesigner. The lead session owns master, packaging and the other rings.

## Read first
- `docs/collections/tenebrae.md`: the note at the top (Logan's later decisions override the text: **faces are allowed** when they read well, so house rule 11 no longer binds), your ring's section, "House rules for all eight", "How every ring is judged". The platform table predates batch 16: P5 to P8 have landed since (`collection_templates` exists). C-T1, C-T3 and C-T4 are on master (PR #239): `Sketch::tracery` and `Profile::Regions`, the Gothic pierce shapes (Lancet, Ogee, Trefoil, Quatrefoil, Mouchette), the `gothic/*` sketch library (`library::list_sketches`, the `sketch.library` and `sketch.tracery` nodes) and `dfm::cut_lands`. C-T2, C-T5, C-T6, C-T7 and C-V2 are not. Build your ring with what master has, and where your section waits on an enabler, use its interim form or build the equivalent in your example.
- `docs/collections/README.md`: gates, the review loop, the template gate, working rules.
- `lessons.md` in your seed: why the Bestiarium cut seven of nine rings and the Cataphracta pilot cut all three. It changes how you work.
- CLAUDE.md is the casting and CAD doctrine. Search it (CAD parts, `Placement::Ring`, `csg`, cutters, side faces, stamps) rather than reading all of it.

Binding taste (Logan): one theme face to palm, nothing scattered; figurative motifs as stamps or parts with true outlines; stones in visible made settings, set à jour, never height-field prongs; no flat, blocky CAD; factory stock keeps its hard wall-to-face angles; density and legibility at least Caiman's. Final renders in studio gold.

Process: as your ring's section says. Delft clay is 3.0 deg draft, 0.8 mm section, 0.30 mm detail. Lost wax: apply `CastProcess::LostWax` and then raise `min_section_mm` to 0.8. If a sand ring cannot pass its sand gates without losing what makes it read, it may become lost wax (Logan: "if we can still cast via lost wax, then just make it a lost-wax ring"); say so in the report.

## Where you work
- ONE example file: `crates/ringdesign-core/examples/tenebrae_<slug>.rs`. Artwork under `crates/ringdesign-core/examples/tenebrae/art/<slug>/`. Outputs in `showcase/tenebrae/<slug>/`.
- No core (`src/`) edits: write any core change you need into your report as exact code, and work around it in your example.
- Build: `cargo build --release -p ringdesign-core --example tenebrae_<slug>`. Run: `target/release/examples/tenebrae_<slug> [OUT_DIR] [--draft] [--verify]`, default output `showcase/tenebrae/<slug>/`. `--draft` is 768 x 320. The default export build is 1536 x 448 and must fit 2 million triangles. `--verify` reloads the saved design cold with an empty alpha library and requires identical vertices, faces and normals.
- Outputs: `design.ring.json` (lean), `report.json` (every gate number, with a `draft` block), `finished-metal.stl`, `casting-pattern.stl` (`mesh::try_build_pattern`), `reference-<stone>.stl` and `stones.json` for each stone, and studio-gold renders with stones set (`render::finished`): `hero face palm side shoulder reverse stones bare-vs-finished` `.png` (plus `section.png` for Oculus and Ogiva), `hero-300.png`, `face-300.png` and a `contact-300.png` sheet. STL files are git-ignored under `showcase/`; PNG, JSON and SVG are tracked. `examples/bestiarium_arachne.rs`, `examples/stock_masterworks.rs` (Caiman) and the CAD examples (`examples/cad_*.rs`) show complete render, gate and feature-tree blocks.

## Gates (all green before a round is reviewed)
1. Finished mesh watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and every made part; `built.solids.notes` empty; every CAD feature `FeatureStatus::Ok`; every stamp resolved.
2. Nothing enters the finger hole: every vertex at least the bore radius minus 0.01 mm from the finger axis, outside the comfort roll.
3. Sand rings: field verdict from `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)` **Castable**, not "with care"; `manufacturing::inspect` ray release 0 obstructions and 0 unresolved at 0.100 mm and at 0.075 mm. Lost-wax rings: `cad::measure::thickness(&mesh, 0.8)` clean, and `dfm::cut_lands` clean at 0.8 mm.
4. `dfm::findings_in(&d, &lib)` returns 0 findings.
5. The stone record's count equals the gem preview count; the crowding census clean or explained.
6. `--verify` passes at the export build.
7. The casting-pattern mesh is watertight with 0 degenerate faces and 0 crossings.

## Template gate
```sh
cargo build --release -p ringdesign-graph --example collection_templates
rm -rf target/tpl-src target/tpl && mkdir -p target/tpl-src
cp -r showcase/tenebrae/<slug> target/tpl-src/<slug>
printf '{"collection":"tenebrae","rings":[{"slug":"<slug>","exposed_controls":[],"template_class":"procedural"}]}\n' > target/tpl-src/collection.json
target/release/examples/collection_templates tenebrae target/tpl-src --output-dir target/tpl --only <slug> --verify-export
```
(`template_class`: `procedural` for a node-built or CAD ring, `stock` for factory stock, `painted` for painted relief.) Record the `design.set` patch count (4 at most), the graph size against the class budget (300 KB, 1 MB, 3 MB), and whether cold source and mesh parity passed.

## Final report
The verdict and score per round, every gate at draft and export, the template-gate numbers, the feature tree as sentences, what each part, stamp and stone is and why, what you could not do, and core changes wanted as exact code.
