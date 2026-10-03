# Tenebrae ring: Sigillum (`sigillum`), cloud report

**Final verdict: cut at 7.3 after extension round 5** (ship bar 7.5). That makes 3 block-out read tests (the third read) and 5 review rounds: 6.3, 6.8, 6.9 (cut), then Logan's extension at 7.1 and 7.3 (cut). Every automatic gate was green at every reviewed round, at draft and at export, and so was the template gate. In rounds 4 and 5 the ring was judged as lost wax under Logan's rule, and it also pulls clean from Delft sand as a bonus. It failed on the ornament below the table, not on a gate.

- Branch: `claude/tenebrae-sigillum`, from `master` at `8e5a59a`. Master was merged twice: at `c116627` (PR #248, crisp edges) and at `c87d38d` (PRs #251 and #252: C-V1 to C-V4 placement and beads, C-B1 curve layers).
- Example: `crates/ringdesign-core/examples/tenebrae_sigillum.rs`. Outputs are in `showcase/tenebrae/sigillum/`.
- Run: `target/release/examples/tenebrae_sigillum [OUT_DIR] [--draft] [--verify] [--blockout] [--probe] [--cross]`.
- Commits: block-outs `847e05a`, `0156221`, `2761ff3`; round 1 `3ddb414`; round 2 `0546961`; round 3 `1e73320`; round 4 `a3c58d8`; round 5 `814290d`. After the session restart, each read test and review was pushed in its own commit; the block-out read tests went in with the next build's commit.
- For the extension, master was merged again at `2e11632`: PR #255 (C-T6 Textura and C-T5 marks), #257, #258 and #259.

## Read tests and reviews

| Step | Result | Score | What the reviewer said |
|---|---|---|---|
| Read test 1 | reads: **false** | — | It read as "a fleur-de-lis signet", not a chapter seal and not Gothic. There was no legend and no border. The fleur's curls over a curved band read as **a smiley face**. The cheek arcade showed only as tick marks. |
| Read test 2 | reads: **false** | — | It now read at once as a seal matrix with a legend, a fleur and a scalloped border. It did not read as Gothic: the lettering looked roman, the border looked like a coin edge, and the quatrefoil was almost invisible. |
| Read test 3 | reads: **true** | — | "Gothic church seal ring / ecclesiastical seal matrix", from three cues: a tracery quatrefoil, a Textura legend and the mirrored matrix. The reviewer asked for a fleur that holds its shape in the hero, pointed cusping and visible architecture below the table. |
| Round 1 | revise | **6.3** | The face is a convincing Gothic seal. The fleur's lower half was ghosted and below the table the ring was bare stock. The template gate was not yet recorded. |
| Round 2 | revise | **6.8** | The fleur now reads in hero and face, the cheek arcade is clean, and the template gate passed. The shank slots read as knurling, the roundels as bubbles and the window as unfinished. The border was still scalloped. |
| Round 3 | **cut** | **6.9** | The face still reads as a Gothic seal at 300 px. The shank lights read as a "rope twist / serrated spine". The fleur still read as a kit of parts, the window as open, the roundels as specks and the arcade as small teeth. "A competent engraved seal on a plain factory signet with token ornament below the table." |
| Round 4 (extension) | revise | **7.1** | The real Textura legend (C-T6) turns the table into "a medieval chapter seal": the biggest gain since round 1. The fleur's upper half and the sharp border darts now read. Still failing: a rectangular band cut deeper than the figure over a "star" foot, the legend cut too shallow, the shank as a "saw-tooth spine", and shrunken cheek lancets. |
| Round 5 (extension) | **cut** | **7.3** | The legend now reads as a dark Textura ring. The cheek arcade is "the first ornament below the table that reads as architecture", and the fleur is a classic silhouette. Still short: the band drawn as two hairline trenches, lumpy fleur floors, cheek lancets of 2.1–2.4 mm (3 mm asked), 6 shank lights a side read as "brackets", small roundels (the lowest touching the bore edge), a small spur at the window's apex, and the ragged bore lip. "A handsome, legible seal face on a plain factory signet, with ornament below the table that is small, sparse and sometimes broken." |

The files are `read-test-1.json` to `read-test-3.json` and `review-round1.json` to `review-round5.json`, all in `showcase/tenebrae/sigillum/`.

## Extension rounds 4 and 5 (Logan, 2026-10-03)

Both lines went into Sigillum's section of `docs/collections/tenebrae.md` before round 4's reviewer started:

- The extension: "Logan granted extension rounds 4 and 5 on 2026-10-03."
- The process decision: judged as lost wax on 017 Tonneau, with the sand verdict reported as a bonus.

### Process: lost wax, with a sand bonus

- **The lost-wax gate** runs on what is poured. Every ornament is bench work, so the casting pattern is plain 017 stock.
  - `cad::measure::thickness(pattern, 0.8)` gives 0 samples below the limit and a sampled minimum of 3.06 mm, at draft and export.
  - The finished ring's thinnest wall is 1.46 mm.
- **Why everything moved to the bench.** On the finished metal, the cast along-the-pull recesses (cheek arcade and roundels) left knife-edges where their zero-draft walls ran under the head's rounded edges and the shoulders' sloping side faces. The sampler read 81 of 384 samples under 0.8 mm. Cutting them at the bench with the seal makes the casting plain and every recess crisp. It is also the section's own idea: a seal is bench work.
- **Sand bonus.** On a copy judged as the Delft pour, the field is Castable with 0.0% undercut. Ray release is 0 obstructions and 0 unresolved at both 0.100 and 0.075 mm, and every parting-line check holds. The same ring still pulls from Delft sand.

### Gates (round 5, final)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Triangles | 556,468 | **1,375,026** |
| Watertight; degenerate faces; self-crossings | yes; 0; 0 | yes; 0; 0 |
| Solids and parts notes | empty | empty |
| Bore margin | −4.7e-7 mm | −4.8e-7 mm |
| Field verdict (lost wax) | Castable | Castable |
| Lost-wax thickness at 0.8 mm (casting pattern) | 0 below; min 3.06 mm | 0 below; min 3.06 mm |
| Sand bonus: field, release at 0.100 and 0.075 mm | Castable; 0/0; 0/0 | Castable; 0/0; 0/0 |
| DFM findings; stones reported / previewed | 0; 0 / 0 | 0; 0 / 0 |

The rest of the record:

- **Cold reload:** `--verify` gives identical vertices, faces and normals.
- **Casting pattern:** closed at 1,376,256 triangles, watertight, 0 degenerate faces, 0 crossings.
- **Design file:** `design.ring.json` is 812,174 bytes at format 6, because of `Pillow` tops and Textura text.
- **Template gate, stock class:** 1 `design.set` patch (`/manufacturing`) and **969,140 bytes** against the 1 MB budget, with 331 nodes. Source is identical, cold reloads pass and mesh parity holds; round 4's graph was 976,626 bytes.
- **`crisp_relief`:** not set, because the ring has no height-field relief.

### What changed in rounds 4 and 5

- **The legend** is real Textura (UnifrakturMaguntia) through `sketch::text::TextLayout`, reading clockwise from a cross pattée at the top, heads outward.
  - Its band is fitted between 4.45 and 5.55 mm, and its tracking is solved to close the circle.
  - Each glyph region becomes one stamp: its counters are joined to the outline by a hairline keyhole, and the outline is mirrored for the seal.
  - Each glyph has a gable floor along the radius through its middle, so it reads head-on: 0.30 mm at the walls, 0.42 mm on the spine.
  - It is not a CAD Extrude, because the table is barrelled and a planar cut cannot keep an even depth across it (see the core-change request below). There are 42 glyph regions.
- **The fleur** is one joined figure: its parts are stood up as prisms, united by `csg::union_all`, sliced and simplified.
  - Its parts are a pointed centre petal, two side petals rooted in the band, a stadium-ended band, and a foot of three rounded lobes.
  - It has one `Pillow` floor (0.60 to 0.95 mm), and the band is drawn by two graved lines.
- **The border's cusp points** run 0.8 mm in, and the legend band moved inward to clear them.
- **The cheek arcade** has five lancets on one level sill, 2.10, 2.25 and 2.40 mm tall with the centre tallest. A graved sill bar runs under them, and they are cut at the bench down over the wall's roll toward the bore.
- **The shank lights** run along the ring with their points toward the table, each with its own sill. There are 6 a side, graded from 1.2 × 2.2 to about 0.95 × 1.7 mm, with 0.6 mm of polished crown either side. The first stands 0.6 mm short of the window's foot.
- **The shoulder windows** are a closed hood on a sill bar (two halves lapping at the apex), with two acute lights and a quatrefoil oculus.
- **The roundels** are three a side (1.7, 1.3 and about 0.9 mm), as large as the narrowing side wall allows.

### What I could not do in the extension

- **Cheek lancets of 3 mm.** The head's cheek on 017 at a 15.6 mm face runs only from about 10.5 mm (a 0.9 mm land over the bore) to 12.9 mm (under the table's rounded edge). A taller light breaks the edge or leaves a knife-edge under it.
- **Roundels of Ø 2.0, 1.6 and 1.2.** The side wall narrows to under 1 mm within 15° of the head.
- **A solid band at the figure's depth.** As a separate flat cut inside the pillowed figure it read as a mask; as a deeper strap it read as a slot. The two graved lines were the compromise, and the reviewer still asked for a solid band.
- **The bore lip.** Raw 017 without the sand master is hollow under the head (the legend shows through the bore), and turning the envelope off leaves the lip as it is. It is the factory stock's own inner edge.
- **Template headroom** is 3%.

## The base: fallback to 017 Tonneau

The section asks for 005 Rosette at 16 × 18.7, with 017 as the fallback. On 005 through the sand master, the head is a domed four-lobed pillow:

- It sags 0.42 mm 2 mm off its crown, 0.74 mm at 3 mm and 1.12 mm at 4 mm.
- The envelope fill is 0.67 mm (Ilex's limit was 0.3).

That leaves no table to cut a seal in, and the bare render reads as a cloud, not a signet. 017 Tonneau is a true signet table:

- Its envelope fill is 0.011 mm.
- It keeps its hard table-to-wall angle.
- It is barrelled across the finger, falling 0.52 mm at 4 mm across.

I used 16 × 15.6, the largest face 017 resizes to (130% of its native 12 mm width). The bore is 18.6 mm and the metal is Silver 925, in Delft clay: 3.0° draft, 0.8 mm section, 0.30 mm detail. The table stands 5.15 mm over the bore, which leaves 4.20 mm under the deepest cut (0.95 mm). `report.json` records the reasoning in `stock_fallback_why`.

## Gates (round 3, sand rules)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Triangles | 553,688 | **1,363,958** (limit 2,000,000) |
| Watertight; degenerate faces | yes; 0 | yes; 0 |
| `csg::self_crossings` (ring; no made parts) | 0 | 0 |
| `solids.notes` / `parts.notes` | empty / empty | empty / empty |
| CAD features | none (see below) | none |
| Bore margin (minimum vertex radius minus bore radius) | −4.7e-7 mm | −4.8e-7 mm |
| Field verdict (`attributed_field_report` 256 × 128, with `judge_parts`) | **Castable**, 0.0% undercut | **Castable** |
| Ray release at 0.100 mm (obstructions / unresolved) | 0 / 0 | 0 / 0 |
| Ray release at 0.075 mm (obstructions / unresolved) | 0 / 0 | 0 / 0 |
| Draft-clamp bites | none (no painted relief) | none |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed | 0 / 0 | 0 / 0 |
| Metal | 17.3 g of 925 | 17.3 g |

Further gates:

- `--verify` passes: a cold reload with an empty library gives identical vertices, faces and normals.
- The casting pattern (`try_build_pattern`) is watertight with 0 degenerate faces and 0 self-crossings, at 1,376,100 triangles. The bench seal and shank lights are left out of it.
- The release status reads "Review", not "Clear": 389 mm² of the factory table and bore walls lie under the 3° draft, and the bare stock reports the same.
- Every sand slot in the release notes is at least 0.30 mm wide. The narrowest is exactly 0.30 mm, beside the larger shoulder roundel; a 0.10 mm slot from round 2 was cleared by moving the roundels clear of the comfort roll.
- `design.ring.json` is 812,136 bytes at **format 6**. Format 6 comes from the `Pillow` stamp tops (the crisp-edges opt-in from PR #248) and the tiered and fine-cap stamps. `crisp_relief` is not set, because the ring carries no height-field relief.

## Template gate (round 3)

| | Value |
|---|---|
| Class | `stock` (factory 017 through the sand master) |
| `design.set` patches | **1** (`/manufacturing`), within the limit of 4 |
| Graph size | **988,371 bytes** against the 1,000,000-byte stock budget, with 418 nodes |
| Cold source | identical |
| Cold design and graph reload | pass |
| Mesh parity | vertices, faces and normals identical, at 1,363,958 triangles |
| Open time | first build 2.18 s, evaluate 111 ms, detail 15 ms, verdict 11 ms |

The record is `showcase/tenebrae/sigillum/verification.json`, and `report.json` also carries it as `template_gate`. Round 2's graph was 874,982 bytes. Round 1's stamps were 1.34 MB of graph until I thinned the outline sampling.

## The build, as sentences (round 3; see the extension section for rounds 4 and 5)

There are no CAD features. The section's bench extrudes from a Tangent plane cannot cut an even intaglio into a barrelled table: the table falls 0.6 mm across the field. So every cut is a stamp draped on the surface at an even depth. Bench stamps carry `bench: true` and stay out of the pattern; cast stamps carry `along_pull: true`.

1. **The base.** Factory 017 Tonneau goes through the sand master, with the envelope on and its own Flat chart. It is resized to 16 × 15.6 with an 18.6 mm bore.
2. **The seal field** is a pointed quatrefoil: four equilateral foils reaching 4.35 mm on the axes, with inner cusps 2.8 mm out on the diagonals. It is bench-cut 0.40 mm deep with a flat floor, and a V-cut rule follows just outside its wall so the frame reads in any light.
3. **The fleur-de-lis** (6.3 mm tall) is drawn for the seal and mirrored. It has a lanceolate centre petal, two side petals springing from the band and curling out and down, a straight band, a foot and two spikes. Each part is a bench hollow with a `Pillow` floor, 0.6 mm at the wall and 0.95 mm at the heart.
4. **The legend** reads "✠ sigillum capituli ecclesie cathedralis", in Textura quadrata minuscules running clockwise from the top, heads outward, mirrored as a matrix.
   - Every stem is a lozenge-footed minim, and every hairline is a broad-nib stroke. Each is a graver's V-cut: 0.30 mm at the wall and 0.55 mm on the spine. There are 115 cuts.
   - The cross pattée opens the legend as a cone-floored cut.
   - The tracking closes the circle.
5. **The border** is a cusped circle: 12 round foils inscribed in a plain ruled circle, and at each cusp a tapering point 0.55 mm long running in toward the legend. All are V-cuts to 0.50 mm.
6. **The cheek arcade** is five cast acute lancets on each ±z cheek, along the pull, sunk 0.50 with `Pillow` floors. They stand on one level sill, with the centre light tallest (1.99, 1.84 and 1.69 mm), as a chapter house grades them.
7. **The shoulder windows** are bench-cut on each shoulder's crown at 37° and 143°, pointing toward the head. Each has two sunk acute lights with a 0.5 mm mullion left standing, a quatrefoil oculus, and a hood mould and sill.
8. **The shoulder roundels** are cast quatrefoils along the pull on the ±z side faces past the head: Ø 2.0 at 44.5°, and a second at about 0.85 mm where the wall narrows. Each keeps a 0.6 mm land toward the comfort roll.
9. **The shank lights** are 10 sunk bench lancets on each shank's crown, from 22° down to about 50° short of the palm. They are graded from 1.20 × 2.20 to 0.93 × 1.70 mm, kept 0.45 mm inside the crown's edge, with 0.55 mm piers.
10. **The palm** is plain stock, as the section has it.

## Enablers used

- **PR #248 (crisp edges)**: `StampTop::Pillow` on the fleur, the roundels, the oculus and every lancet floor. The close-ups use `render::write_png_framed` with `yaw_facing`, never a cropped mesh: `stones.png`, `cheek-close.png` and `shoulder-close.png`. The smoothed sand-master shading applies automatically.
- **C-T3 / C-T1** were not used directly. I tried the bundled `gothic/fleur-de-lis` stand-in. It read thin and spindly at 300 px and, in its first draft, as a smiley face, so the fleur, quatrefoil and cross are drawn in the example.
- **C-T6 (#255), extension rounds**: `sketch::text::TextLayout` with `TextFont::Textura`, arced clockwise, the cross pattée drawn from the library. It is cut as stamps, not an Extrude (see below). Before #255 reached master (rounds 1 to 3), the legend was hand-cut Textura strokes.
- **C-T5 (#255)**: no `Component::mark` is needed, because there are no CAD parts; bench stamps carry no marks.
- **#259 (CAD fallbacks)**: unused. A planar extrude still cannot cut an even intaglio into the barrelled table.
- **No marks were needed**: no bench cut needs a locating mark, because every cut is a stamp with `bench: true` and the pattern simply omits it.
- **C-B1 and C-V1 to C-V4** are merged but unused.

## What I could not do

- **005 Rosette.** It has no table to cut a seal in (see the base section above).
- **CAD bench cuts.** On the barrelled table a planar extrude would cut 0.3 mm at the centre and nothing at the field's edge. A `Profile`-on-surface or offset-surface cut would let the section's feature tree stand. Every cut became a draped stamp instead, and the template has no `cad.feature` controls (Legend, Seal depth, Face size).
- **Arcade height.** The reviewers asked for 3.0–3.2 mm lancets on the cheek. 017's cheek squarely faces the pull only between about 11.0 and 13.3 mm from the axis, so the tallest light that keeps the table edge whole is about 2.0 mm on a level sill.
- **Three roundels per side.** The shoulder side face narrows from 2.2 mm to under 1 mm within 10° of the head, so only one full roundel and one small one fit with Delft lands.
- **The shank read.** Lancets on a 2.9 mm crown, seen at the hero's grazing angle, read as a serrated spine. The side faces there are under 1 mm tall, too small for cast lancets.
- **Template headroom.** The graph sits at 988 KB of the 1 MB budget, so any further detail needs leaner outlines or P7-style stamp nodes.
- **The bore lip.** The reviewers saw a ragged edge under the head in `palm.png`. I did not trace it; the bare stock through the sand master is the likely source.

## Core changes wanted (exact code)

1. **A cut that follows the surface**, so a seal can stay a CAD feature on a curved table. In `cad.rs`, add a profile placement on `Operation::Extrude`:
   ```rust
   /// Cut `height_mm` deep measured along the band's normal at every point of the profile, not from the sketch's plane.
   #[serde(default, skip_serializing_if = "std::ops::Not::not")]
   pub follow_surface: bool,
   ```
   Fence it at design format 6. Evaluate it by offsetting each swept profile point by the band's surface height under it (`cad::surface_hit`).
2. **Annular stamps (a stamp with holes)**, so a rule ring, a letter's counter or a window frame is one stamp rather than overlapping runs. In `setting.rs`:
   ```rust
   pub struct Stamp { .., #[serde(default, skip_serializing_if = "Vec::is_empty")] pub holes: Vec<Vec<[f64; 2]>> }
   ```
   Triangulate the cap with the holes (`cap_faces` takes only constraint chords today, so it needs a hole list) and fence it at format 6. Half of this ring's crossing hunts came from runs of one rule meeting edge to edge.
3. **Text cut along the surface.** With (1), C-T6's text sketch could be cut as one `Extrude` that follows the table. That would replace the 42 keyholed glyph stamps and restore the Legend control (a `sketch.text` node into the extrude).
4. **Thickness on bench-cut finished metal.** `cad::measure::thickness` reports near-zero sections at the rims of zero-draft stamp recesses on sloping walls, where its rays meet the next surface within microns. A minimum ray length, or a skip for sliver triangles, would let a ring check its finished metal, not only its casting.
