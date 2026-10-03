# Vepres — Rosa mortua (`rosa-mortua`): cloud report

**Outcome: stopped at the block-out.** All three read tests came back `reads: false`. TASK.md says that after a third failed read test the work stops, because the subject needs rethinking rather than more detail. So rounds 1 to 3 never started: no full review, no score, and no ship or cut verdict.

- Branch: `claude/vepres-rosa-mortua`, from `origin/master` at `8e5a59a`. Master was merged in at `2e11632` (merge `579728c`) before read test 1.
- Example: `crates/ringdesign-core/examples/vepres_rosa_mortua.rs`
- Outputs: `showcase/vepres/rosa-mortua/` (renders, `report.json`, `draft-gates.json`, `design.ring.json`, `read-test-{1,2,3}.json`)
- Rounds used: **0 of 3**. Block-out attempts used: **3 of 3**.
- Process: lost wax (0.8 mm section, 0.15 mm detail, no draft). Under Logan's 2026-10-03 rule it is judged as lost wax with no pull rule. The process was never in question, and this is recorded in the ring's section of `docs/collections/vepres.md`. No extra rounds were granted, and the doc says so.

## Read tests

Each test used a fresh reviewer agent. It was given only `target/review.md`, the ring's name and slug, "read-test mode", the attempt number and the listed paths.

| Attempt | Commit | What was built | Verdict | What the reviewer saw |
|---|---|---|---|---|
| 1 | `796e610` | True pear ruby in a bezel ringed by a three-ring rose **bloom** (15 petals at golden-angle offsets). Garnet cabochon in five P6 Sepal claws with five short Twist wisps patterned `About` it. Two swept stems crossing over the bypass. 14 hooked prickles. A three-leaflet leaf on arm A's shoulder. | **false** | "Spiky band with two red stones." The spikes read as a sun-ray or gear band. The bloom read as "a flower-shaped bezel or little crown". The hip read as a solitaire. The leaf could not be seen. |
| 2 | `c152e17` | Flat pear in a bezel, clasped by five pointed sepals on a receptacle. Hip body under the Sepal-claw hip, with wisps rising from the claws' gallery. Seven broad hooked prickles per arm. A larger serrated leaf beyond the bud. | **false** | The spikes still read as a sawblade crown. The bud read as a "leaf-trimmed bezel". The hip crown read as "stray prongs or a tendril". The leaf blades looked the same size as the spikes. |
| 3 | `8da0467` | Pear stood up 25°, point out, in seven lanceolate sepals. Hip in a collet with five 0.7 mm wisps that lean in over the dome and curl out. Five uneven prickles per arm, with the palm third bare. Leaflets 6.0 and 5.0 mm on a 2 mm rachis. | **false** | Better than attempt 2: the lower band is now plain, and the hip is "the strongest rose cue". Even so, the bud read as "a gem in a crown or star bezel". The prickles read as needles, the leaf as "a fern frond, a thistle leaf or a dragon's dorsal fin", and the face as "a toi et moi bar ring with thorny trim". |

The three tests agreed on these points:
- **The bud never read as a bud.** A faceted pear, table up, in any made setting reads as a gem in a decorative bezel. The reviewer asked three times for a pear standing 30 to 45° with long sepals rising two thirds of its height. Tilting a pear in a bezel turns the collet's drum toward the face camera. That was seen at 35 and 50° in builds that were never tested, and still at 25° in attempt 3.
- **The prickles never read as rose prickles at 300 px.** Twist hooks on a 3.6 mm round stem show in silhouette as fins or needles, whatever their foot size.
- **The hip came closest.** The cabochon with a curled crown got "rose hip?" in attempt 3.

## Why I believe the subject needs rethinking

The read the section asks for depends on a closed bud on a 3.6 mm stem. A 7 × 5 faceted ruby is the bud's whole mass, and a gem lying table-up cannot be a teardrop seen from the side. The face camera looks straight down on the table, and the hero camera sees it at about 35°. The one form that read as a flower at all was attempt 1's open bloom. The reviewer marked it down for not being the bud the spec names, and for reading as a bezel. A rethink should choose one of these:
1. **Bloom, not bud.** Rosa mortua as a dead rose head: a withered open bloom with the ruby as its heart. That changes the section's concept. The bloom code is kept in the example (`petal`, `petal_layout`) behind `#![allow(dead_code)]`.
2. **A cabochon bud.** A red pear cabochon standing in a tall, sculpted calyx, with sepal geometry wrapping its lower half. It needs a builder that seats a stone on end, which core has no head for today.
3. **A bigger stem.** A thicker or keyframed stem (about 5 mm) at the shoulders, so the prickles read as broad triangles and not as needles.

## Gates (draft build of the final block-out, 768 × 320)

A block-out is not reviewed on its gates, and these were never brought green.

| Gate | Result |
|---|---|
| Watertight, 0 degenerate, 0 self-crossings | pass (510,726 triangles) |
| Every CAD part closed, 0 crossings | pass |
| Notes empty, features Ok | **fail**: fillet notes on prickles A3, A4, B1 and B3 (bead folds or pinches) |
| Nothing in the finger hole | **fail**: 297 vertices inside, nearest at 8.75 mm against a 9.3 mm bore radius (the lifted bud's receptacle and stem) |
| Lost-wax Castable, wall ≥ 0.8 mm | **fail**: "Castable with care", thinnest wall 0.29 mm at 86°, in the bypass's seam channel on a 3.6 mm HighDome |
| Part walls at their floors | **fail**: bud sepal tips 0.043 mm, terminal leaflet 0.087 mm, prickle tips A3/A4/B3/B4 0.12 to 0.14 mm |
| 0 DFM findings | pass |
| Stones = preview (2), no metal in a stone, crowding clean | pass (after replacing the elliptical stone model with a pear- and crown-aware one) |
| 384 × 192 | **fail** (same notes) |
| Casting pattern closed | pass |
| Export within 2 M triangles | pass at draft; no export build was run |
| Cold reload identical | not run (`--verify` runs at export) |

**Template gate: not run.** TASK.md places it after the last round, and no round was run.

## Enablers

- **C-B2, true pear plan:** used from the master merge onward (`GemCut::Pear`, 7 × 5).
- **C-V1, `Placement::Relative`:** used for the bud's receptacle and sepals, the hip body and the hip's wisp (with its `About` pattern), so each follows its stone.
- **P6 Sepal claws:** used on the hip in attempts 1 and 2. The bud used them in an interim build that was never read-tested.
- **#248 framed close-ups:** used for `bud-close.png`, `hip-close.png` and `leaf-close.png` (`render::write_png_framed`).
- **Not used:** C-V2 `Along` (the prickles stay individual features, because the reviewer wanted an uneven, hand-placed spacing), C-V3 scale laws, C-V4, C-V5, C-T5 to C-T7, Textura and marks. I set no `crisp_relief`, because the ring has no height-field relief.

## What each part is

- **Stem (#1):** procedural `ShankKind::Bypass`, HighDome 3.6 × 2.7, bore 18.6.
- **Stems of arms A and B (#6, #7):** stored tubes of radius 1.05 mm. Each follows its arm's slide (`BYPASS_OFFSET` × half-width), rises out of the band over 30° to stand 0.75 mm proud, and runs under its stone. Core's bypass is one widened section with a seam groove, not two arms, so these tubes are what make the crossing read.
- **Bud:** pear ruby 7 × 5 at θ 115°, tilted 25°, in `head.bezel` (wall 0.45, lip 0.3). Under it, a stored ellipsoid receptacle. Around its rounded end, seven stored lanceolate sepals that climb the collet and curl off it.
- **Hip:** garnet round cabochon 6.0 at θ 65° in `head.bezel` (wall 0.6). Under it, a stored ellipsoid hip body. On top, a crown of one Twist wisp (a 1.3 × 0.7 lens that leans 1.1 mm in over 3.1 mm of rise, then curls out 130° and tapers to 0.4), patterned `About` the hip five times.
- **Leaf:** three serrated, domed leaflets and a tube rachis, mapped onto the band's crown through a chart. Past the crown they hang at a gentle droop. Each leaflet is sunk a little differently so that their overlapping feet never share a face.
- **Prickles:** ten `Twist` hooks: a 3.6 × 1.9 foot, 1.0 rise, a 1.5 bend, a 70° hook, tapering to 0.14. They stand at uneven offsets 50 to 95° from the top on each arm, graded 1.0 to 0.6, alternately spun ±25° and canted ±18° about 0.55 mm off the crest. Their seam beads are 0.25 mm.

## What I could not do

- I could not get a closed-bud read from a table-up faceted pear in a made setting (see above).
- I did not bring the gates green. That work would follow a read, not come before it.

## Core changes wanted

1. **Expose the bypass arms.** Every example that seats parts on a bypass has to re-derive `bypass_arm` (`core/profile.rs`). It should be public:
   ```rust
   // core/profile.rs
   -fn bypass_arm(off: f64, k: f64) -> Option<(f64, f64)> {
   +pub fn bypass_arm(off: f64, k: f64) -> Option<(f64, f64)> {
   ```
2. **A head that seats a stone on end.** A bud needs a `head.calyx { rise_deg, sepals, reach }` builder: a stone stood on its rounded end, with sepal blades swept from a receptacle to `reach` of its length. I have not written that builder. It is a design question for the lead, not a one-line change.
