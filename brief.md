# Vepres ring brief (shared by every ring session)

You own ONE ring of Vepres (the thorned botanical collection), the fourth of Logan's four master collections in RingDesigner. The lead session owns master, packaging and the other rings.

## Read first
- `docs/collections/vepres.md`: the note at the top (Logan's later decisions override the text: **faces are allowed** when they read well), your ring's section, "0. Read this first", and "4. Shared scaffolding and the gates every ring passes". Its platform table predates batch 16. Since then P5 (station-aware side-face gates, `FieldContext::side_faces_at`) and P7 (`base.preset`, the stamp nodes) have landed, and so have Tenebrae's C-T1, C-T3 and C-T4. Not on master: C-B1's `CurveLayer::phase` and every Vepres enabler (C-V1 to C-V5). Where your section offers a fallback for one of those, use it.
- `docs/collections/README.md`: gates, the review loop, the template gate, working rules.
- `lessons.md` in your seed: why the Bestiarium cut seven of nine rings, and why five more failed since. It changes how you work.
- CLAUDE.md is the casting doctrine. Search it (stamps, `parting_monotone`, `stamp_row`, the sand master, `Hide`, side faces, CAD parts, seam beads) rather than reading all of it.

Binding taste (Logan): one plant face to palm, nothing scattered; figurative motifs as stamps or parts with true outlines; stones in visible made settings, never height-field prongs; no flat, blocky CAD; factory stock keeps its hard wall-to-face angles; density and legibility at least Caiman's. Final renders in studio gold, stones set.

Process: as your ring's section says. Delft clay is 3.0 deg draft, 0.8 mm section, 0.30 mm detail. If a sand ring cannot pass its sand gates without losing what makes it read, it may become lost wax (Logan: "if we can still cast via lost wax, then just make it a lost-wax ring"): apply `CastProcess::LostWax`, raise `min_section_mm` to 0.8, and say so in the report.

## Where you work
- ONE example file: `crates/ringdesign-core/examples/vepres_<slug>.rs`. Artwork under `crates/ringdesign-core/examples/vepres/art/<slug>/`. Outputs in `showcase/vepres/<slug>/`. The plan's template ids (`<slug>-vepres`) are for packaging; use the short slug for files and the branch.
- Another Vepres ring is being built at the same time. Section 4 puts shared helpers in `examples/common/vepres.rs`: keep yours in your own example file (or under `examples/vepres/<slug>/`) instead, so the two branches merge cleanly. The lead will factor a shared module at packaging.
- No core (`src/`) edits: write any core change you need into your report as exact code, and work around it in your example.
- Build: `cargo build --release -p ringdesign-core --example vepres_<slug>`. Run: `target/release/examples/vepres_<slug> [OUT_DIR] [--draft] [--verify]`. `--draft` is 768 x 320. The default export build is 1536 x 448 and must fit 2 million triangles. `--verify` reloads the saved design cold with an empty alpha library and requires identical vertices, faces and normals.
- Outputs: `design.ring.json` (lean), `report.json` (every gate number, with a `draft` block), `finished-metal.stl`, `casting-pattern.stl` (`mesh::try_build_pattern`: cuts and heads left out, raised drill dots), `reference-<stone>.stl` and `stones.json` for each stone, and studio-gold renders with stones set (`render::finished`): `hero face palm side shoulder reverse stones bare-vs-finished` `.png`, plus `hero-300.png`, `face-300.png` and a `contact-300.png` sheet. STL files are git-ignored under `showcase/`; PNG, JSON and SVG are tracked. `examples/bestiarium_arachne.rs` and `examples/stock_masterworks.rs` (Caiman, Aurelia, the `Skin` helpers and `flush_seat`) show complete render, gate and stock blocks; `examples/prickle_probe.rs` and `examples/parting_stamp_probe.rs` measured Vepres's two sand claims.

## Gates (all green before a round is reviewed)
1. Finished mesh watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and every made part; `built.solids.notes` and `built.parts.notes` empty; every CAD feature `FeatureStatus::Ok`; every stamp resolved.
2. Nothing enters the finger hole: every vertex at least the bore radius minus 0.01 mm from the finger axis, outside the comfort roll.
3. Sand rings: `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)` then `castability::judge_parts(&mut field, &d, &built)`: **Castable**, not "with care"; `manufacturing::inspect` ray release 0 obstructions and 0 unresolved at 0.100 mm and at 0.075 mm; every `skin::draft_clamp` bite at most 0.05 mm; every stamp on the parting line passes `parting_monotone`. Lost-wax rings: thinnest wall at least 0.8 mm (`cad::measure::thickness`).
4. `dfm::findings_in(&d, &lib)` returns 0 findings.
5. The stone record's count equals the gem preview count; no metal inside a stone; the crowding census clean or explained.
6. `--verify` passes at the export build.
7. The casting-pattern mesh is watertight with 0 degenerate faces and 0 crossings.
Where the ring carries stamps, run the gates at 384 x 192 too: phantoms move with resolution; real obstructions converge.

## Template gate
```sh
cargo build --release -p ringdesign-graph --example collection_templates
rm -rf target/tpl-src target/tpl && mkdir -p target/tpl-src
cp -r showcase/vepres/<slug> target/tpl-src/<slug>
printf '{"collection":"vepres","rings":[{"slug":"<slug>","exposed_controls":[],"template_class":"<class>"}]}\n' > target/tpl-src/collection.json
target/release/examples/collection_templates vepres target/tpl-src --output-dir target/tpl --only <slug> --verify-export
```
(`template_class`: `procedural` for a node-built or CAD ring, `stock` for factory stock, `painted` for painted relief.) Record the `design.set` patch count (4 at most), the graph size against the class budget (300 KB, 1 MB, 3 MB), and whether cold source and mesh parity passed.

## Final report
The verdict and score per round, every gate at draft and export, the template-gate numbers, the CAD feature tree as sentences, what each layer, stamp, part and stone is and why, what you could not do, and core changes wanted as exact code.
