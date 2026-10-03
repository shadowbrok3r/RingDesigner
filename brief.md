# Officina ring brief (shared by every Officina session)

You own ONE ring of Officina, six short lessons in RingDesigner's CAD workspace that replace the retired Workshop collection (Aster, Tide, Lantern, Aureole: Logan called those "flat and blocky"). Each ring is a short feature history with one idea, and a jeweller should learn that idea by dragging the timeline's rollback marker from the first feature to the last. The lead session owns master, packaging and the other rings.

## Read first
- `docs/collections/starters-and-officina.md`: section 0 ("What binds this file", including the process floors and the gates), section 2 ("Officina": the format, build order and author example), and your ring's section. Its platform table predates batch 16: P2, P5, P6, P7 and P8 have all landed, so `base.preset`, the `cad.op.*` and placement pins, and the collection tooling (`collection_templates`) exist.
- `docs/collections/README.md`: gates, the review loop, the template gate, working rules.
- `lessons.md` in your seed: why most collection rings have been cut. Most of it is about creatures, but the workmanship lessons (crisp outlines, no flat cut plate, fillets not cups, no panels) bind you too.
- CLAUDE.md is the casting and CAD doctrine. Search it (CAD parts, `Placement::Ring`, `csg`, seam beads, builders, cutters, side faces) rather than reading all of it.

Binding taste (Logan): one idea face to palm, nothing scattered; no flat, blocky or mechanical CAD (break every edge, blend every seam); stones in visible made settings; factory stock keeps its hard wall-to-face angles; final renders in studio gold, stones set. A lesson is simple, never crude: it must still be a piece he would sell.

Process: as your ring's section says (Delft: 3.0 deg draft, 0.8 mm section, 0.30 mm detail; lost wax: `CastProcess::LostWax.apply`, then `min_section_mm` 0.8). Write the process with `apply`, never by assigning `d.draft.process` alone. Bore 18.2 mm, as the section says.

## Where you work
- ONE example file: `crates/ringdesign-core/examples/officina_<slug>.rs`. The plan's shared `examples/officina/main.rs` and the `cad::examples::NAMES` registration are for packaging: the lead folds the six together later, so the six branches merge cleanly. Outputs go in `showcase/officina/<slug>/`.
- No core (`src/`) edits: write any core change you need into your report as exact code, and work around it in your example.
- Build: `cargo build --release -p ringdesign-core --example officina_<slug>`. Run: `target/release/examples/officina_<slug> [OUT_DIR] [--draft] [--verify]`, default output `showcase/officina/<slug>/`. `--draft` is 768 x 320. The default export build is 1536 x 448 and must fit 2 million triangles. `--verify` reloads the saved design cold with an empty alpha library and requires identical vertices, faces and normals.
- Outputs: `design.ring.json` (lean), `report.json` (every gate number, with a `draft` block), `finished-metal.stl`, `casting-pattern.stl` (`mesh::try_build_pattern`), `reference-<stone>.stl` and `stones.json` for each stone, and studio-gold renders with stones set (`render::finished`): `hero face palm side shoulder reverse stones bare-vs-finished` `.png`, plus `hero-300.png`, `face-300.png`, a `contact-300.png` sheet, and `timeline.png`: one contact sheet of the ring after each feature (`Document::through`), labelled with the feature's name. STL files are git-ignored under `showcase/`; PNG, JSON and SVG are tracked. The CAD examples (`examples/cad_*.rs`) and `examples/bestiarium_arachne.rs` show complete feature-tree, render and gate blocks.

## Gates (all green before a round is reviewed)
1. Finished mesh watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and every made part; `built.solids.notes` and `built.parts.notes` empty; every CAD feature `FeatureStatus::Ok`.
2. Nothing enters the finger hole: every vertex at least the bore radius minus 0.01 mm from the finger axis, outside the comfort roll.
3. Sand rings: `castability::judge::judged_field_report(.., Some(&built))` **Castable**, not "with care" (Keystone alone, whose lesson is that verdict, may stand at "with care" and must say why in its report); `manufacturing::inspect` ray release 0 obstructions and 0 unresolved at 0.100 mm and at 0.075 mm. Lost-wax rings: `cad::measure::thickness(&mesh, 0.8)` clean, and `dfm::cut_lands` clean at 0.8 mm.
4. `dfm::findings_in(&d, &lib)` returns 0 findings.
5. The stone record's count equals the gem preview count; the crowding census clean or explained.
6. `--verify` passes at the export build.
7. The casting-pattern mesh is watertight with 0 degenerate faces and 0 crossings.

## Template gate
```sh
cargo build --release -p ringdesign-graph --example collection_templates
rm -rf target/tpl-src target/tpl && mkdir -p target/tpl-src
cp -r showcase/officina/<slug> target/tpl-src/<slug>
printf '{"collection":"officina","rings":[{"slug":"<slug>","exposed_controls":[],"template_class":"procedural"}]}\n' > target/tpl-src/collection.json
target/release/examples/collection_templates officina target/tpl-src --output-dir target/tpl --only <slug> --verify-export
```
(`template_class`: `procedural` for a node-built or CAD ring, `stock` for factory stock.) Record the `design.set` patch count (4 at most), the graph size against the class budget (300 KB, 1 MB, 3 MB), and whether cold source and mesh parity passed. The graph must list the features in the timeline's order, as `cad.feature` nodes.

## Final report
The verdict and score per round, every gate at draft and export, the template-gate numbers, the feature tree as sentences (one per feature: what it does and what the reader learns from it), what each part and stone is and why, what you could not do, and core changes wanted as exact code.
