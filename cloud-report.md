# Sphenodon — the parietal (`sphenodon`): lane report

**Outcome: stopped at the block-out.** All three read tests failed, so under TASK.md the subject needs rethinking before any detailing. **Review rounds used: 0 of 3.** No full review ran, so there is no ship/revise/cut score. Every casting gate is green at draft and at export, and the template gate passed. The ring's problem is that it does not read as the animal; manufacturability is not the issue.

Branch `claude/cataphracta-sphenodon`. Author file: `crates/ringdesign-core/examples/cataphracta_sphenodon.rs`. Outputs: `showcase/cataphracta/sphenodon/`.

## Read tests (block-out, reviewer verdicts verbatim in `read-test-<n>.json`)

| # | What was built | `reads` | What the eye saw (reviewer, condensed) |
|---|---|---|---|
| 1 | The spec as written: Flat 7.5 × 3.4 band, a 44-tooth 1.3 mm sail graded Cosine 0.4, a peridot on a gypsy mound, clear ±13° | **false** | "A plain polished band with a small green cabochon and a thin row of tiny spikes, like a gear rim, a studded collar or a zip … at a push 'dinosaur spine'." |
| 2 | Changes from test 1 (24 → 30 teeth, 2.0 mm, taller nape; side-face tubercles and ventral squares), plus a sculpted tuatara head painted on the crown with the peridot as its parietal, and the crest graded Spiral from nape to tail tip | **false** | "Reads as a reptile at once … a generic lizard or baby dragon lying round a band … 'lizard ring' or 'dragon wrap ring'. Nobody would say tuatara." The reviewer's first change was to remove the head, because the collection's gate list says "no faces and no eyes". |
| 3 | Changes from test 2: head removed, stone back on the spine; symmetric 30-tooth 2.35 mm sail, Cosine 0.35; beaded dorsal courses over the crown; two staggered tubercle rows on the side faces | **false** | "A band covered edge to edge in a fine, even knurl … 'spiked band', 'punk stud ring', 'hedgehog' or 'durian' … Nobody would say tuatara, or even lizard." |

Test 3's unapplied changes were: polished ribbons back on the crown, the sail as one continuous blade with a 0.95–1.0 mm span, and wandering tubercles of mixed size over the full side face.

**For the lead: what the three tests show.**

- **Only the head gave a reptile read.** Test 2 was the only attempt a reviewer called a reptile, and it was the one with a head. Without a head, the reviewers read the crest plus hide as a spiked or knurled band.
- **The concept alone does not carry at 300 px.** A serrated sail on the parting line with a flush stone is a generic "dragon spine". The Bestiarium lesson ("the subject was small where the camera looks") applies here.
- **The "no faces or eyes" rule is stale in the collection file.**
  - Logan's decision of 2026-09-24 allows faces and eyes that read well (`brief.md`, and the note at the top of `cataphracta.md`).
  - The per-ring gate checklist in `cataphracta.md` ("Gates per ring", item 3) still lists "no faces and no eyes", and the test-2 reviewer applied it.
  - Recommendation: update that checklist, then rethink Sphenodon around the one version that read: a tuatara head, with the parietal stone on the skull behind the eyes and the crest running from the nape to the tail. Keep the test-3 fixes: a polished ribbon beside a continuous blade, and wandering side tubercles. Build the head as a clamped painted layer. Test 2's head cut 243 texels, at most 0.037 mm, and every gate stayed green.
- **The spec has two factual problems** (see "Core and doc changes").

## Gates (attempt-3 design: `report.json` is the draft run, `report-export.json` is the export run with `--verify`)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Triangles | 490,466 | 1,370,836 (limit 2 M) |
| Watertight, degenerate faces | yes, 0 | yes, 0 |
| Self-crossings (ring; no CAD parts) | 0 | 0 |
| `built.solids.notes` / stamps | empty / 0 of 0 | empty / 0 of 0 |
| Finger hole: nearest vertex vs bore | −0.00007 mm (limit −0.01) | −0.00001 mm |
| Field, `attributed_field_report` 256 × 128, SandTwoPart Delft | **Castable**, worst draft 0.03°, 0 undercut, drag 2.7% | **Castable**, same |
| Ray release at 0.100 mm | 0 obstructions, 0 unresolved | 0, 0 |
| Ray release at 0.075 mm | 0, 0 | 0, 0 |
| Draft-clamp bite | 0.000 mm (Dorsal scales painted and clamped: 0 texels cut; re-audit over every layer and the composite: 0) | 0.000 mm |
| DFM findings | 0 | 0 |
| Stones: report vs preview | 1 = 1 (peridot, 174 preview faces) | 1 = 1 |
| Casting pattern (`try_build_pattern`) | watertight, 0 degenerate, 0 crossings | watertight, 0 degenerate, 0 crossings |
| `--verify` cold reload, empty library | — | identical vertices, faces and normals |

The ring carries no stamps, so no 384 × 192 run was needed. C-R5 drag by layer: the sail cuts marginal area by 27 mm², because it covers the crest line, and the dorsal scales add 2.7 mm².

## Template gate (`collection_templates … --only sphenodon --verify-export`, class `painted`)

| Measure | Value |
|---|---|
| Source method | lift, 46 nodes, 0 exposed controls |
| `design.set` patches | **1** (`/manufacturing`), limit 4 |
| Graph size | **1,945,757 bytes**, against the painted budget of 3 MB (no size review) |
| Cold source parity | identical |
| Mesh parity | 1,370,836 triangles; vertices, faces and normals identical |
| Result | `template_gate_passed: true` |

The editable design is 3.9 MB. Open-time phases are in `template-verification.json`: first build 580 ms, detail 11.5 s cold and 66 ms warm.

## What each layer is, and why (attempt 3)

- **Base.** `ProfileStyle::Flat` 7.5 × 3.4, `crown_mm` 1.2, `flatten_sides`, comfort 0.15, bore 18.6. Thickness-only keys: 1.05 at 0°, 1.18 at 90°, 1.05 at 180°, 1.00 at 270°. Delft two-part, parting at z = 0, gate and sprue from the palm along −Y.
  - The Flat crown exponent is opened from 8 to 3 (`shape_a`). The bare Flat band fields **"Castable with care" at 24.2% drag** (5.6% marginal, 18.5% vertical), because its plateau crown carries under 3° over 57% of its width.
- **The sail.** `reptile::svg::sail` (C-R7), 30 teeth, 2.35 mm tall, 1.2 mm span, `offset_u` 0.5, grade Cosine 0.35 about 90°.
  - Mask "Sail height" is u-only: 0 within ±9°, 0.45 at the mound's skirt, full from 20° to 45°, then down to 0.3 at the palm.
  - The gable's apex is opened to a 0.36 mm plateau (see "Core and doc changes"). A sharp apex put 287 obstructions of 0.02 mm, all round the ring at z = −0.02, because a mesh row tipped the apex off the parting plane.
- **Dorsal scales.** A painted crown layer (2048 × 768 atlas, `skin::draft_clamp`, `hide_layer`, Max). Four courses of beaded plates (pitch 0.95, groove 0.36) step down 0.15 mm per course from the crest. Grooves are never deeper than a step, so nothing rises walking away from the parting line (G5 and G4), and the clamp cuts nothing.
- **Tubercle rows.** Two staggered rows of 0.74 mm domes at 1.2 mm pitch across the 1.95 mm side face. Warped, gated `SideFaces(Both)`, SmoothMax 0.2, masked off the ventral field.
- **Ventral squares.** `reptile::svg::paver`, 1.1 mm cells, 0.45 mm tall, u-only mask over 270 ± 35°, gated `SideFaces(Both)`.
- **Parietal peridot.** Round, 3.0 mm, tint (0.50, 0.78, 0.12). `SeatPadLayer` GypsyMound at (90°, crest), crown 1.0, blend 0.45, `fit_stone`, height 1.05, `Flush`, `through`, and a raised 0.6 mm drill dot on the pattern.

## What I could not do

- **Make the subject read at 300 px within three block-outs.**
- **Put granules on the crown flanks.** Domed granules on this shallow crown were tried painted and clamped, focused outboard, from 0.11 to 0.3 mm tall, at 0.9 to 1.2 mm pitch. Every version either:
  - locked in the ray release at the draft mesh only (0.05–0.2 mm phantoms at z ≈ 2.2; clean at export), or
  - fell under the DFM floor, because the alpha thresholds at 0.5 and low granules shrink to 0.10–0.28 mm.
- **Use the spec's P5 flank granules** (`VGate::Draft { 30°, 6° }`). They locked at the fillet (z ≈ 3.46) and bit 0.068 mm. Stepped courses were the only crown texture that stayed legal.
- **Add side-face granules around the tubercles.** A 1.95 mm face holds two rows of 0.74 mm tubercles with 0.4 mm lands and no room for granules between them.

## Core and doc changes wanted (not made; lanes may not edit `src/`)

1. **`reptile::svg::sail` needs a plateau across the parting line.** A knife-edge gable on z = 0 locks by one mesh row at 768 × 320. In `crates/ringdesign-core/src/reptile.rs`, `sail()`, replace
   ```rust
   let across = page.linear(true, &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]);
   ```
   with
   ```rust
   // A flat 0.3 mm across the parting line, so no mesh row can tip the apex off the plane.
   let flat = (0.5 * 0.3 / h).min(0.2);
   let across = page.linear(true, &[(0.0, 0.0), (0.5 - flat, 1.0), (0.5 + flat, 1.0), (1.0, 0.0)]);
   ```
2. **`reptile::svg::tubercle_rows` draws granules of 0.04 mm at its own test station** (2.2 mm face, 0.4 land, `row = h / 5`). Size the granule rows from the land instead:
   ```rust
   let small = (row - land).max(0.4);
   let rows = (((h - 2.0 * row) / (small + land)).floor() as usize).clamp(0, 3);
   for j in 0..rows { let y = 2.0 * row + (j as f64 + 0.5) * (small + land); /* ... */ }
   ```
   `every_reptile_skin_holds_the_detail_floor_at_its_tightest_station` does not catch this, because vanished granules measure as no feature at all. Add an ink-count assertion.
3. **`docs/collections/cataphracta.md`, "Gates per ring" item 3:** replace "no faces and no eyes" with "faces and eyes only where they read well (Logan, 2026-09-24)".
4. **`docs/collections/cataphracta.md`, Sphenodon "Base":** the bare `ProfileStyle::Flat` 7.5 × 3.4, crown 1.2, fields "Castable with care" (24.2% drag). Say `shape_a` 2–3, or LowDome with squared sides.
5. **Sphenodon's "Traps":** the brief's 0.95 mm sail with a sharp gable cannot pass the 0.100/0.075 mm ray release at draft size (item 1). Also, mesh-resolution phantoms from painted crown granules converge away at export but fail the draft gate. The brief's "phantoms move with resolution" note should say which run the gate is judged on.
