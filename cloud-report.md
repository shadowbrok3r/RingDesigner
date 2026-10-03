# Cataphracta: Phrynosoma, *the horned crown*: cloud report

**Outcome: stopped at the block-out. All three read tests failed, so no detailing round was run (0 of 3 rounds used).** Under TASK.md this means the subject needs rethinking, not detailing.

Branch `claude/cataphracta-phrynosoma`, from master `8e5a59a`, with master `2e11632` merged before block-out 3. Author file: `crates/ringdesign-core/examples/cataphracta_phrynosoma.rs`. Outputs are in `showcase/cataphracta/phrynosoma/`.

**Base and process.** The base is the factory 016 Star at a 17 × 17 face on an 18.6 mm bore, unmirrored, in lost wax (Fallback B). Following Logan's 2026-10-03 rule, the ring is judged as lost wax: 0.8 mm minimum section, no pull rule. I recorded this in the ring's section of `docs/collections/cataphracta.md`, with a status line saying the block-out failed.

## Read tests (each by a fresh, independent reviewer agent)

| # | reads | What the eye saw | What changed before the next attempt |
|---|---|---|---|
| 1 | **false** | "A spiky sunburst or a punk-maned beast head … a jeweller would call it a spiked crest or a sun ring." | A straight comb of horns on the rear edge with the occipitals dominant, a flat wedge skull with cephalic plates, and fringe and crest tubercles down the shoulders. |
| 2 | **false** | "A spiky clump … like a pine cone, a hedgehog or a thistle head … spiked dragon, hedgehog or thistle ring." | Asked for a low skull about 1 mm proud, a plate mosaic in tiers of 0.55, 0.35 and 0.15 mm with 0.35 mm V-joints, six separate countable horns on one root line, granules and tubercles on every bare area, and granules down to the palm. |
| 3 | **false** | "A six-lobed star signet covered all over in round granules, like a starfish or sea urchin, with a tortoise-shell patch set in the middle … turtle, starfish or urchin." | Stopped: this was the third attempt. |

The JSON verdicts are `read-test-1.json`, `read-test-2.json` and `read-test-3.json`. I agree with the third verdict from the 300 px renders. At that size the six horns sink into the granule field and read as a few short spikes. The plate mosaic is a carapace, and the granulated eight-pointed star reads as a starfish or urchin. The 2× close-up (`head.png`) does show a plated skull with eyes, nostrils and a horn comb, but the read is judged at 300 px, and there it fails.

## What block-out 3 is (the last state, all one sculpt system, no height-field paint)

- **Horned head** (a stored sculpt, joined).
  - **Skull:** a low plaque on the pillowed table, 0.45 mm of ground plus plates.
  - **Plates:** 57 jittered Voronoi plates in three tiers of 0.55, 0.35 and 0.15 mm, with 0.35 mm V-joints whose floor is the plaque's ground. Each step between tiers falls inside a joint.
  - **Face:** two eyes on the skull's edge and two nostril pits.
  - **Crown:** six straight horns rooted on the straight back edge. The two occipitals are 4.2 mm, the four temporals 2.5 and 2.0 mm, all splayed back and out, with 0.24 mm points.
  - **Table and cheeks:** 501 granules and 66 enlarged tubercles laid on the stock's atlas by Poisson spacing.
  - **Fit to the table:** the head's frame stands on a height map of the real table read off the atlas. A quadric fit missed by 0.25 mm and exposed a slab, so I replaced it.
- **Shoulder hide**, one part for each shoulder, both stored sculpts and joined.
  - Twelve crest tubercles on the parting line, graded from 0.6 to 0.35 mm radius.
  - About 43 fringe scales along both rims. They are flattened, pointed blades from 1.35 to 0.7 mm long, leaning 58° out past the wall and toward the palm.
  - About 170 granules, graded finer toward the palm.

## Gates at block-out 3 (draft, 768 × 320; `report-draft.json`)

| Gate | Result |
|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings; parts 0 crossings; notes empty; 3 parts joined | pass |
| Gates at 384 × 192 | pass (the stored parts and imported stock give the same 574,796 triangles) |
| Field verdict | lost wax **Castable** (band Castable, thinnest wall 1.57 mm) |
| DFM findings | 0, pass |
| Stones | 0 = preview 0, pass |
| Casting pattern | watertight, 0 degenerate faces, 0 crossings; pass |
| Cold reload | identical, pass |
| Triangles | 574,796, within 2 M; pass |
| **Nothing enters the finger hole** | **FAIL**: 80 vertices, nearest at 9.23 mm against a 9.30 mm bore. These are hide beads placed on atlas samples near the comfort roll. |
| **Sections under 0.8 mm named** | **FAIL**: the skull plaque's rounded edge (2.7 mm² of part-alone sections) has no named treatment. It is low relief fused to the table, and the census should say so. |

Both failures are bookkeeping or placement, not form. The export build and the 384 × 192 rerun of the gates were not run as a separate pass, because no review round started.

**Sand bonus:** none. The two-part undercut is 5.14% on the band and 10.70% with the parts. As lost wax this is reported only, never gated.

## Template gate (run on the block-out 3 design, class `painted`)

| Measure | Result |
|---|---|
| `design.set` patches | 1 (`/manufacturing`), pass (at most 4) |
| Source method | lift; source identical, cold design and graph reload identical |
| Mesh parity | vertices, faces and normals identical, 574,796 triangles |
| Size | **12,003,931 bytes against the 3 MB painted budget: FAIL** (`template_gate_passed: false`) |

The three stored sculpt meshes, about 1.2 M faces between them, are what make it heavy. `crisp_relief` is not used, so the lift-gap note does not apply. The figures are in `template-verification.json`.

## Enablers used

- **Used:** #248's framed close-ups (`render::write_png_framed` with `Framing`) for `stones.png` and `head.png`.
- **Not used:**
  - C-B2 and #257 (true stone plans): there is no stone.
  - C-V1 to C-V5, C-T5 to C-T7, #255 (Textura), #258 (patterns along a path) and #259 (CAD fallbacks): they had no use before the read passed.
  - `crisp_relief`: there is no height-field relief.

## Why it failed, and what I would rethink

1. **The 016 star fights the animal.** Its eight soft points read as a starfish once granulated, and as a sunburst round a head with horns. Of the three bases offered, a horned lizard's head wants a plain shield or cushion. Fallback A ("CG Star" lofted) would carry the same trap.
2. **The horned crown does not survive 300 px at signet scale.** Horns long enough to see (attempts 1 and 2) read as a sunburst, a mane or a thistle. Horns short enough to read as a comb (attempt 3) vanish into the texture.
3. **A plated head seen from above is a carapace.** Without the body, a horned lizard's head reads as a turtle shell. The animal's other signature is its round, flat, fringed body, which is what Moloch's reviewers saw as "thorny devil or horned lizard".

**Proposal:** a band or saddle composition in which the whole flattened, fringed body lies across the face, with the head and its crown at one end. Use smooth ground, not granules, around it, so the outline reads. That is the opposite of attempt 3's all-over texture, which drowned the outline.

## What I could not do

- I ran no review round, so there are no scores, as TASK.md requires.
- The two red block-out gates are not fixed: the bore beads and the skull plaque's treatment label.

## Core changes wanted

None were needed. Everything was built in the example file.
