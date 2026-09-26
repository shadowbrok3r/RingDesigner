# Bestiarium ring lane brief (shared by every ring lane)

Repo: `/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner`. You own ONE ring of the
Bestiarium, the first of four new master collections. A lead session owns integration, registration,
packaging, master and the other lanes. Codex authored part of this collection and ran out of usage on
2026-09-25; its record is in `docs/collections/IN-FLIGHT.md` and `docs/collections/ROADMAP.md`.

## Read first
- `docs/collections/bestiarium.md`: your ring's section, "Method every ring shares", "Collection enablers".
- `docs/collections/README.md`: gates, review loop, template gate, working rules.
- `docs/collections/source/plan.md` sections 0, 1, 5 and 7 (section 7 is Logan's binding answers).
- The project CLAUDE.md (already in your context): its casting doctrine is the law for sand rings.

Binding updates that override older text in those files:
- **Faces and eyes are allowed.** Logan: "feel free to impress me with a face." He banned them only
  because earlier ones were poor. A head is welcome when it reads well; hold it to the review bar.
- Logan's taste: one theme face to palm (no scattered elements); figurative motifs are stamps or parts
  with true outlines (painted figures "look like arrows"); stones in visible made settings, never
  height-field prongs ("quite awful"); no flat, blocky CAD; factory stock keeps its hard wall-to-face
  angles; density and legibility at least Caiman's; "the most complex, well thought out, most insane
  rings we've done". His own dark-mythology ZBrush rings set the house style:
  `.claude/collection-review/zbrush_top.png` and `zbrush_sheet.png` (sources in
  `/home/shadowbroker/jewelry-scan/RING/`).
- Every Bestiarium sand ring is Delft clay: 3.0 deg draft, 0.8 mm section, 0.30 mm detail.
- **Painted figures (lead's ruling, 2026-09-26):** the "motifs are stamps with true outlines" rule
  exists because painted motifs used to blur or comb into "arrows" in sand. Painted figurative relief
  is acceptable when its outlines render crisp and true, with no combing, smearing or stair-steps;
  reviews judge it on the renders, not on the construction.
- P6 claw styles are on master: `builders::CLAW` params `style` in Wire, Talon, Fang, Tentacle, Thorn,
  Sepal; `grouping` in Even, Feet, Jaws; `tip` in Dome, Point (`cad/builders.rs`, `setting::ClawOptions`).

## Context economy (this matters: the account has a weekly usage limit)
- The project CLAUDE.md is about 54,000 tokens and is already in your context. A worktree that holds a
  copy injects it again the first time a file tool touches it. Your lane worktree has **no CLAUDE.md
  in its working tree on purpose** (a per-worktree sparse checkout); git does not see it as deleted.
  Never restore it, and never change the sparse checkout.
- **Never use the Read tool on a file inside another worktree** (`.claude/worktrees/<other>/...`);
  some still carry the copy. Read with Bash (`sed -n`, `rg`) or from the reference copies below.
- Read large files in ranges you need, not whole. Keep command output short (`tail`, `rg -n`).
- `/tmp` is wiped on reboot. Keep logs and scratch output under your worktree's `target/`.

## Reference copies (read-only; the originals belong to other lanes)
In `.claude/collection-review/refs/`:
- `bestiarium_arachne.rs`: Arachne, shipped at 7.5/10 (keyframed lost wax, CAD legs and collets, stamp
  tiers, SVG web, a full gate block with made-part loops and a pattern export). Renders: `refs/arachne/`.
- `bestiarium_phoenix.rs`: Phoenix round 1, scored 6.0 (keyframed Delft sand, painted plumage, a
  `contour` painter, quills, graded seat runs). Renders: `refs/phoenix-round1/`.
- `bestiarium_basiliscus.rs`: Basiliscus draft (factory 020 stock in lost wax, gate block with
  made-part and pattern checks).
In the main checkout: `crates/ringdesign-core/examples/stock_masterworks.rs` (Caiman, Saurian, Zenith:
stock signets, skin painting, stamps, draft clamp) and `examples/serpentarium.rs` (Diamondback's
tilted graded princess run, Uraeus's crossing cabochon).
Reviews: `.claude/collection-review/phoenix-round1-review.json`,
`arachne-final-art-review-20260924.json`, `draco-art-review-20260924.json` (Draco was cut: it did not
read as a wyvern, flat blocky CAD, sparse density).
The bar: `showcase/stock-masterworks/caiman/hero.png` and
`showcase/reptilia/renders/Reptilia-collection.png` in the main checkout.

## Where you work
- Your worktree and branch are in your task. Build in that worktree with its own `target/` (the
  default). Never build in the main checkout or in another worktree.
- ONE example file: `crates/ringdesign-core/examples/bestiarium_<slug>.rs`. Artwork under
  `crates/ringdesign-core/examples/bestiarium/art/<slug>/`. Outputs in `showcase/bestiarium/<slug>/`.
- No `src/` edits unless your task says you own an enabler. Write any core change you need as exact
  code in your report instead, and find a workaround inside your example meanwhile.
- Build unguarded: `cargo build --offline --release -p ringdesign-core --example bestiarium_<slug>`.
  Run under the memory guard:
  `systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_<slug> [OUT_DIR] [--draft] [--verify]`.
  The machine has no swap: never run a ring build, probe or test without the guard. Tests follow
  README "Working rules" (unguarded `--no-run` build, then the guarded run, no edits between).
- CLI: `[OUT_DIR] [--draft] [--verify]`, default output `showcase/bestiarium/<slug>/`. `--draft` is
  768 x 320. The default export build is 1536 x 448 and must fit the 2 million triangle budget
  (Basiliscus's stock hit "Imported relief exceeds the 2 million triangle budget"): choose export
  params that fit and record them. `--verify` reloads the saved design cold with an empty alpha
  library and requires identical vertices, faces and normals.
- Outputs: `design.ring.json` (lean: no stored manufacturing setup or build steps), `report.json`
  (every gate number), `finished-metal.stl`, the casting-pattern STL (sand:
  `mesh::try_build_pattern`, cuts and heads left out, raised drill dots; wax: the prepared investment
  pattern, as Arachne's export does), one `reference-<stone>.stl` per stone plus `stones.json`
  (format in `.claude/collection-review/p8-tooling-contract.md`; tints from `Gem::preview_tint`), and
  studio-gold renders with stones set: `hero face palm side shoulder reverse stones bare-vs-finished`
  `.png` (`render::finished`; copy Arachne's `renders()`). STL files are git-ignored under
  `showcase/`; PNG, JSON and SVG are tracked.

## Lessons from finished lanes
- **Paint on square texels.** DFM measures a painted mask with a round disc in pixels and converts at
  the finer axis, so an atlas whose texels are not square reads strokes as thinner than they are
  (Manticora: 0.3 mm strokes read 0.06 to 0.11 mm on the plan's 2048 x 768). Size the atlas so
  height ~ width x band_v_len / circumference (Manticora used 2048 x 192).
- A seat pad with no stone gets a band-edge feather warning from the stones report; avoid stoneless pads.

## Gates (all must pass before you hand over)
1. Finished mesh watertight with 0 degenerate faces; `csg::self_crossings == 0` on the ring and on
   every made part; `built.solids.notes` empty; every stamp resolved.
2. Nothing enters the finger hole: every finished-mesh vertex stays at least the bore radius minus
   0.01 mm from the finger axis, outside the comfort-fit roll at the two bore edges. (Arachne's leg
   intruded 0.36 mm and no gate caught it. Add this check to your gate block.)
3. The field verdict from `castability::attributed_field_report(&d, &lib, &d.draft, 256, 128)`
   against the ring's own process.
   - Sand: **Castable**, not "with care"; `mf::inspect` ray release 0 obstructions and 0 unresolved
     at 0.100 mm and at 0.075 mm; every `skin::draft_clamp` bite at most 0.05 mm.
   - Lost wax: `CastProcess::LostWax.apply(&mut d.draft)`, then `min_section_mm = 0.8` and
     `min_draft_deg = 0`; the fill verdict passes at 0.8 mm.
   - Lost wax, CAD lands: the field verdict judges only the band, so **assert every made part's
     sections yourself** and write a `land_widths` block into `report.json`: each claw's minimum
     section and the arc length under 0.8 mm (claws taper toward their tips: Tentacle runs 0.60 to
     0.14 of the wire), collet and bezel walls and lips, rails, and every free or sculpted part's
     thinnest section. Each stretch under 0.8 mm is either removed or named as an exception with its
     bench treatment (for example a burnished bezel lip). An unnamed sub-floor section fails the gate
     (Manticora's 0.55 mm collet wall and Kraken's 0.50 mm claw ends both failed review round 1).
4. `dfm::findings_in(&d, &lib)` returns 0 findings.
5. The `stones::report` count equals the gem preview count; the crowding census is clean or explained.
6. `--verify` passes at the export build.
7. Sand only: the pattern mesh is watertight with 0 degenerates and 0 crossings.
Where the ring carries stamps, also run the gates at 384 x 192: phantoms move with resolution and
real obstructions converge.

## Template gate
The P7 platform lifts a design into an editable graph; the P8 tool checks it. Run it into a scratch
directory, never into `graphs/templates`:
```sh
cargo build --offline --release -p ringdesign-graph --example collection_templates
rm -rf target/tpl-src target/tpl && mkdir -p target/tpl-src
cp -r showcase/bestiarium/<slug> target/tpl-src/<slug>
printf '{"collection":"bestiarium","rings":[{"slug":"<slug>","exposed_controls":[],"template_class":"painted"}]}\n' > target/tpl-src/collection.json
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/collection_templates bestiarium target/tpl-src --output-dir target/tpl --only <slug> --verify-export
```
(`template_class` is `procedural` for an unpainted node-built ring, `stock` for bare factory stock,
`painted` otherwise.) Record the `design.set` patch count (4 at most), the graph size against the
class budget, and whether cold source and mesh parity passed. If it cannot pass, say exactly why.

## Self-review before handing over
View every render yourself (Read displays PNGs from your own worktree) beside Caiman's hero, the
Reptilia sheet and the ZBrush sheets. Fix what obviously reads badly before handing over. The ring must
read as its subject at 300 px without a caption, have large, medium and small forms, and a silhouette
unlike the other Bestiarium rings.

## Commits
Commit on your branch at each milestone that builds and passes its checks, so a stopped lane loses
little. Subject line says what changed; end the message with:
```
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DhhBfxWFhPAaJd3cVZw2HV
```
Never push, tag, publish, bump versions, rebase or reset branches you do not own, drop or pop stashes,
or touch emulators. Do not commit `target/` or logs.

## Final report (your last message)
Branch, HEAD and worktree; render paths; every gate's numbers at draft and export; template-gate
numbers; what each layer, stamp, part and stone is and why; what you could not do; core changes you
want (exact code); an honest self-score against Caiman = 7.
