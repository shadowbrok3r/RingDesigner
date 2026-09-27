# Cataphracta — Moloch, the thorn idol: cloud lane report

**Outcome: stopped at the block-out.** None of the three block-out attempts passed the read test (`reads: false` all three times), so under TASK.md the loop stopped before any review round. The subject needs rethinking, not detailing.

- Review rounds used: **0** of 3. No full review was run, so there is no score.
- Read tests: **3 of 3 failed.**
- Every build gate is green at draft (768 × 320), at export (1536 × 448, with `--verify`), and at 384 × 192. The failure is legibility, not castability.

Branch: `claude/cataphracta-moloch`. Author file: `crates/ringdesign-core/examples/cataphracta_moloch.rs`. Artwork: `crates/ringdesign-core/examples/cataphracta/art/moloch/`. Outputs: `showcase/cataphracta/moloch/`.

## Read tests (independent reviewer, read-test mode)

| # | reads | What the eye saw (reviewer's words, condensed) |
|---|---|---|
| 1 | **false** | "A plain polished band with rows of small even spikes… a studded punk bracelet, a spiked dog collar or a mace ring." The top half was bare metal. The head lens read as "an eye or a seed pod". The legs did not show. |
| 2 | **false** | "A crown of thorns or a punk spike band." The capillary-groove flutes read as "a comb, gear teeth or a tyre tread… machined, not hide". The knob's specular crescent read as "a 'C' or a hook… an eye or a buckle". The legs did not register. |
| 3 | **false** | "A burr, a sea urchin, a hedgehog, or a sand-cast 'thorn crown' band." The 0.33 mm knob apexes read as "grain or pitting". The ring has "no front or back… nothing in the silhouette says head, legs or tail". Face-300 read as "a mask, a beetle or a buckle". |

Full verdicts: `showcase/cataphracta/moloch/read-test-{1,2,3}.json`.

What changed between the attempts:
- **Attempt 1 → 2**, applying read test 1:
  - The knob became the tallest station (key 1.76 at 90°).
  - The lens-shaped head and the eye pits were removed.
  - The crest row went to a major-plus-three-minors rhythm.
  - Horn apex 1.6 → 2.4 mm, crown thorn apex 2.2 mm.
  - A granule ground went on the side faces and capillary grooves across the crown flanks.
- **Attempt 2 → 3**, applying read test 2:
  - The grooves were dropped.
  - A packed carpet of 248 along-pull cone knobs went over the crown flanks and side faces.
  - Two fore-thorns went on the knob's steep front.
  - The horns lean out about 25°.
  - The legs grew 40% wider with 1.1 mm domes and cone toes, with a 0.55 mm bare halo round them.
  - The crest grade is spent by about 230°.

Attempt 3's own changes (bigger and sparser knob rosettes, four limbs that break the silhouette, a tapering tail) were not applied, because the loop ends there.

## Gates (block-out attempt 3, the committed design)

| Gate | Draft 768 × 320 | Export 1536 × 448 | 384 × 192 |
|---|---|---|---|
| Watertight, degenerate faces, self-crossings | yes, 0, 0 | yes, 0, 0 | yes, 0, 0 |
| Every stamp solid closed and uncrossed (371 solids) | pass | pass | — |
| `solids.notes` empty; stamped / stamps; `parting_monotone` | [], 371/371, 0 failures | [], 371/371, 0 failures | [], 371/371 |
| Bore: nearest vertex against a 9.300 mm radius | 9.300, 0 inside | 9.300, 0 inside | — |
| Field (`attributed_field_report`, 256 × 128, Petrobond sand) | **Castable**, 0.000% undercut | **Castable**, 0.000% undercut | — |
| Drag (marginal + vertical) | 31.1 of 1396 mm² (2.2%) | same | — |
| Ray release at 0.100 mm (obstructions, unresolved) | 0 / 0 | 0 / 0 | 0 / 0 |
| Ray release at 0.075 mm | 0 / 0 | 0 / 0 | 0 / 0 |
| `draft_clamp` bite ("Side hide" group) | 0 texels, 0.000 mm | 0 texels, 0.000 mm | — |
| DFM findings | 0 | 0 | — |
| Stones reported / previewed | 0 / 0 (no stone) | 0 / 0 | — |
| Casting pattern (`try_build_pattern`) | closed, 0 / 0 | closed, 0 / 0 | — |
| Triangles (budget 2 M) | 553,608 | 1,309,432 | 246,034 |
| Cold reload with an empty library (`--verify`) | — | **identical** (vertices, faces, normals) | — |

Build times: 15 s at draft and 59 s at export. The authoring step is about 3–5 s, most of it the knob placer.

Records: `report-draft.json` and `report.json` in `showcase/cataphracta/moloch/`, each with every number and its own `draft` block.

## Template gate

`collection_templates cataphracta … --only moloch --verify-export`:

| Class | Graph bytes | Budget | `design.set` patches | Cold source parity | Mesh parity | Gate |
|---|---|---|---|---|---|---|
| `painted` (the brief's default command) | 1,207,804 | 3,000,000 | 1 (`/manufacturing`) | identical | 1,309,432 triangles, identical | **passed** |
| `procedural` (the honest class: nothing is hide-painted) | 1,207,804 | 300,000 | 1 | identical | identical | **size review required** (4× over) |

- The graph has 530 nodes, nearly all of them the 371 expanded stamps. It stays over the procedural budget until P7's `stamp.row` node and a scatter node exist.
- First open takes 2.36 s (`detail` 2.34 s).
- Both `verification.json` files are copied beside the ring as `template-verification-{painted,procedural}.json`.

## What each layer, stamp and part is, and why

Base:
- `ProfileStyle::Flat` 7.0 × 3.6, crown 1.4, `flatten_sides()`, comfort 0.15, bore 18.6 (size 8.6).
- Thickness-only keys: 1.76 at the knob (90°), 1.32 at the front legs, 1.30 at the hips, and 1.00 at the palm, which is the reference.
- **Deviation:** `shape_a = 2.0`, a parabolic crown. Flat's own `x⁸` crown is a table standing parallel to the pull. On the bare band it measured 240.7 mm² vertical plus 71.8 marginal of 1403.7 (22.3%), which is "Castable with care". The parabola measures 2.5%.

Process: Petrobond, set up with `mf::Recipe::sand(SandProcess::Petrobond)`. Its floors are a 2.5° draft, a 0.6 mm section and 0.40 mm detail. It parts on z = 0, with a gate off the palm along −Y and a sprue below it.

Layers:
- **"Side hide"**, a clamped group (`SandClamp` 2048 × 768), holding one layer: **"Granule ground"**.
  - It is a `reptile::svg::granules` tiling fitted to the side faces: 43 cells of 1.88 × 1.88 mm, 0.32 mm high, lands 0.45, gated `SideFaces(Both)`.
  - The clamp bites 0 texels.

Stamps (371 in all):
- **"Thorn, crown"** is the knob's cone at 90°: 2.7 mm across, apex 2.2 mm, on the parting line (G2).
- **"Thorn, knob fore 1–2"** are cones of 1.6 and 1.2 mm on the knob's steep front. They stop that front reading as a bare, bright crescent.
- **"Thorn, crest major 1–17" and "Thorn, crest minor 1–51"** run from behind the crown thorn round the body to a tail tip 20° short of the knob.
  - Each is a cone on the parting line (the `v` is solved by a one-station `stamp_row`), in a major-then-three-minors rhythm.
  - Majors grade from 2.4 to 0.8 mm and minors from 1.2 to 0.6 mm, the grade spent by about 230°.
- **"Horn, fingertip/knuckle"** are rounded triangles 3.4 × 3.2 at the middle of the 90° station's own side faces.
  - They are struck along the pull, with a cone apex of 2.4 mm leaning 1.05 mm toward the crest.
  - These are the silhouette the height field cannot make.
- **"Leg, front/hind fingertip/knuckle — limb, shin, toe 1–3"** make four legs of five convex parts each, on the side faces at 134° and 338°.
  - Limb and shin are domed capsules; each toe is a cone to a claw tip.
  - They are separate parts because a Dome over a concave outline triangulates lumpy, and the 4° draft walls of a one-piece hand self-cross in the toe crotches.
- **"Side thorn, fingertip/knuckle"** are graded, jittered cones down the side faces, struck along the pull (G1).
- **"Knob, 1–248"** are cones packed by a placer over the crown flanks and side faces wherever nothing else stands.
  - They are struck along the pull, each sized so its column still meets the crown: its reach toward the crest stays under the crest's radius across the knob's own span.
  - Sizes run 0.6–1.59 mm, apex 0.55 × diameter, gap 0.42 mm.
  - All of them pull clean at both ray pitches and at both mesh resolutions.

No stones, as the brief specifies.

## What I could not do

- **Make it read as a thorny devil at 300 px.** Three reviewers named a spiked band, a thorn crown, a punk collar, a sea urchin or a hedgehog. None named a lizard.
- The sand grammar keeps figurative relief to the crest line (G2) and the side faces (G1), and at 300 px the side faces are seen edge-on in both review cameras.
- A true head (a spade outline with Ridge, Dome, Gable or Cone tops) failed each time:
  - Dome and Cone tops fail `parting_monotone` on any outline that is not exactly mirror-symmetric about the crest.
  - The Ridge head showed a cracked margin wherever the band rose under it.
  - Two domes read as "beads", and the Gable as "a block".
- The spec's thorn rosettes (`rosette_thorn` tiling) fail DFM on the reference side faces (0.11 mm strokes against 0.40 on 1.5 × 1.9 mm cells). Those faces are only 1.9 mm tall.
- The spec's capillary grooves self-cross (about 1,000 crossings) when run over the 0.05 mm crown edge onto the side face. Stopped short of the edge, they read as "machined". They were dropped after read test 2.
- The side-thorn rows came out lopsided: 1 on the fingertip face against 29 on the knuckle face, where the knob placer filled the gap. This was not fixed because the loop had stopped.

## Recommendation (for the rethink)

- At ring scale in a two-part sand mould, the thorny devil's identity (the whole animal as a knobbed lizard) cannot be carried by hide texture on a band. Reviewers read texture as "spikes" whatever its grading.
- The candidate rethink is a figurative ring: the lizard's plan, head, four splayed legs and tail seen from above as the face. That is Arachne's winning formula.
- It needs lost wax, or a face wide enough (a factory signet table) for true-outline stamps on the table under G5.
- Alternatively, keep Moloch as an abstract "thorn idol" and judge it as ornament, not as an animal. That is a collection decision.

## Core changes wanted (exact code)

1. **A public parting-line solver.** Placing one stamp on the crest today needs a one-station `stamp_row`:

```rust
// crates/ringdesign-core/src/setting.rs
/// The chart `v` where the section at `theta_deg` crosses the parting plane, solved as `stamp_row` solves it.
pub fn parting_v(design: &crate::RingDesign, theta_deg: f64) -> Option<f64> {
    let ctx = design.field_context();
    BareSurface::new(design, &ctx).parting_v(theta_deg, ctx.crest_v_mm)
}
```

2. **A tolerance on the monotone test's fall.** `parting_monotone`'s `falls` closure accepts a rise of only 1e-7 mm. Because a Dome's height is the outline's gauge, any polygon that is not bit-symmetric about the crest line fails:
   - A traced ellipse failed.
   - The same ellipse built from exact mirrored points passed.

   The proposal, which I have not measured against the ray release:

```rust
// crates/ringdesign-core/src/setting.rs, in Stamp::parting_monotone
const FALL_TOL_MM: f64 = 1e-4; // far under the release's 0.075 mm pitch
// let ok = h <= last + 1e-7;
let ok = h <= last + FALL_TOL_MM;
```

3. **`stamp.row` and a scatter node (P7).** A scatter node would be `setting::scatter(design, &Scatter { outline, zone: SideFaces | CrownFlank, sizes, gap_mm, seed })`, with the placer in `cataphracta_moloch.rs` (`knob_carpet`, `stamp_discs`) as its reference. The two would bring this template's 1.2 MB graph under the 300 KB procedural budget.
