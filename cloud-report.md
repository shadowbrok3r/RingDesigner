# Cataphracta — Ouroborus, the girdled wheel: cloud lane report (Petrobond)

**Outcome: stopped at the block-out.** All three read tests came back `reads: false`, so under TASK.md's rule the ring stops here. The subject needs rethinking before any detailing. Review rounds used: **0**, so there is no full-review verdict or score. Every gate is green at draft (768 × 320), at 384 × 192 and at export (1536 × 448, with `--verify`). The template gate **failed on size only**: 3.89 MB against the 3 MB painted budget.

- Branch: `claude/cataphracta-ouroborus` (from master `8e5a59a`). I merged no master moves, because no round after the block-out ran.
- Author file: `crates/ringdesign-core/examples/cataphracta_ouroborus.rs`.
- Artwork: `crates/ringdesign-core/examples/cataphracta/art/ouroborus/` (`whorl.svg`, `keel.svg`).
- Outputs: `showcase/cataphracta/ouroborus/`.
- Enablers used: **C-R2** (the `Spiral` grade law, on the whorls). I took C-R7's `whorl` and then replaced it (see below). I did not use C-R1: there is no clamped group, since the head is a made part. I did not use P5 either: the side-face spines were not built.
- Subject, as Logan confirmed in TASK.md: a serpent's head with closed jaws on its own tail tip, with eyes and head scales, in Petrobond. I recorded this as a note at the top of the ring's section in `docs/collections/cataphracta.md`. The section still read "lizard, plates only, no eyes", and two reviewers cited it.

## Read tests (an independent reviewer each time; full text in `read-test-{1,2,3}.json`)

| # | reads | What the eye saw (condensed) | Changes asked |
|---|---|---|---|
| 1 | **false** | Hero: "a plain polished bypass band with a squared, helmet-like cap". The tail runs under the head like a bypass. Face: "a faceted knuckle or buckle". 90% bare metal. | (1) Bite at about 90° with a real gape, and the tail tapering to 1 mm or less inside it. (2) A wedge head with a neck pinch, terraced plates with 0.3 mm risers, no ruled grooves, and no eye (a brief conflict, see below). (3) Graded girdles, a keel and a taper. |
| 2 | **false** | Hero: "a jeweller would probably guess serpent ring", but the even, square girdles read as a cog or millipede, and the bite is not legible. Face: "a knight's helm, a beetle's carapace or a lamp", with a ruled grid and no snout or eyes. | (1) A head 1.4× as long as wide, widest at the jaw at 1.35× the band, a rounded snout at 0.55× the band, a crown 30% flatter, a colubrid plate layout with no full-width grooves, eye bumps on the outline, and a neck pinch to 0.8×. (2) A mouth line with an upturned corner, and the girdles at their smallest pitch right up to the lips. (3) Rounded overlapping girdles split into 3–5 offset scales, and spines no more than 0.3 mm proud. |
| 3 | **false** | Hero: "the classic ouroboros … a jeweller would very likely say snake ring", "nearly passes", though the girdles still read as castellated scutes. Face: still "a symmetric, rounded torpedo divided by straight ruled lines", read as a beetle, helmet or bullet. No eyes show on the outline, there is no mouth line, and a third of the frame is polished metal (the bore seen past the 1 mm tail). | (1) A flat wedge head seen from above, with eye bumps 0.3–0.4 mm proud of the outline, a mouth line from above, and no grid. (2) Rounded imbricate girdles. (3) Hide over the whole crown visible in face view, and a tail that tapers into the jaws. |

What the three tests show: the **hero** now reads as an ouroboros. The **face view never did**. The head is seen almost straight down, foreshortened by the ring's curve, and every plate edge sand allows reads as a ruled grid. Sand forbids grooves that run along the head off the crest, so plate edges must be transverse grooves or step-down terraces. The face camera also looks past a 1 mm tail into the polished palm bore.

**Brief conflict, not argued with the reviewer:** read test 1 asked for the eye to be removed. The eye stayed, because TASK.md (Logan) explicitly requires eyes. I updated the doc section instead.

## Gates (block-out design as committed)

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Watertight / degenerate faces / self-crossings | yes / 0 / 0 | yes / 0 / 0 | yes / 0 / 0 |
| Head part closed, crossings as made and as placed | 0 open edges, 0 / 0 | 0 | 0 open edges, 0 / 0 |
| Solids and parts notes; head joined | [] ; joined 1 | [] ; joined 1 | [] ; joined 1 |
| Bore: nearest vertex against 9.300 mm | 9.300, 0 inside | — | 9.300, 0 inside |
| Field (`attributed_field_report` 256 × 128 plus `judge_parts`, Petrobond) | **Castable**, undercut 0.000%, drag 1.8% | — | **Castable**, undercut 0.000%, drag 1.8% (band 1.0%) |
| Head part judged | 112 mm², 0 undercut, 0 locking | — | 112.1 mm², 0 undercut, marginal 5.6 mm², vertical 2.9 mm² |
| Ray release at 0.100 / 0.075 mm | 0 / 0 obstructions, 0 unresolved | 0 / 0 obstructions, 0 unresolved | 0 / 0 obstructions, 0 unresolved |
| Draft-clamp bite | none (no clamped group) | — | none |
| DFM findings | 0 | — | 0 |
| Stones reported / previewed | 0 / 0 | — | 0 / 0 |
| Casting pattern | closed, 0 / 0 | — | closed, 0 / 0 |
| Triangles (budget 2 M) | 757,428 | 433,982 | 1,587,556 |
| Cold reload, empty library | — | — | **identical** |

## Template gate

`collection_templates cataphracta … --only ouroborus --verify-export`, class `painted`:
- 41 nodes, **1** `design.set` patch (`/manufacturing`).
- Lifted source identical; mesh parity 1,587,556 triangles, vertices, faces and normals identical.
- Cold design and graph reloads pass.
- First build 3.0 s.
- Graph is **3,889,203 bytes against the 3,000,000 budget**, so `template_gate_passed: false` (review required).
- Fix, not applied because the lane stopped: the head's stored mesh is 395 k faces (380 stations × 520 section points). About 250 stations × 320 points would bring it to about 160 k faces and the graph under 3 MB.
- Record: `showcase/cataphracta/ouroborus/template-verification.json`.

## What each layer and part is, and why

- **Base:**
  - `ProfileStyle::Flat`, 3.4 × 2.8 reference (the tail tip's width), crown 2.2, `shape_a` 2.0 for a round serpent body, `flatten_sides()`, comfort 0.15, bore 18.6.
  - Process: Petrobond, parting fixed at z = 0 (`auto_parting` off).
  - Keys run from the jaw (1.62 × 1.12) through the nape and body, tapering round to a tail of 1.0 × 1.5 mm at the snout (86°). The tail widens again inside the head.
  - The width keys clamp at 0.3, which is why the reference width is the tail's.
- **"Serpent head"** is one stored part (`Operation::Stored`, recipe kernel `sections`), joined to the band.
  - It is built station by station as a solid whose every cross-section is a single span across the parting plane: a dorsal height function that never rises away from the crest, plus side walls and a jaw given as z against height. Every facet therefore faces into its own mould half by construction (least draft off the plane ≥ 0°, 0 facets leaning back). That is how a figurative head pours in two-part sand without a clamp.
  - The head has:
    - a spade plan 11.6 mm long by 6.6 mm at the jaw hinge, with a neck pinch;
    - cephalic plates in three tiers with 0.26 mm risers (internasals, prefrontals, frontal and parietals; then the loreal margin, supraoculars and temporals);
    - transverse sutures confined to their tier and shallower than its riser, and a median ridge where paired plates meet (a groove on the crest would lock);
    - an eye with a slit pupil and orbit under a brow, and a nostril;
    - a gape 1.2 mm high around the tail for its first 1.8–2.7 mm, then closed lips with a groove and an upturned mouth corner.
- **"Whorls"** (tiling, C-R2 `Spiral` grade, seam at 92° under the snout): 47 girdles, 2.8 mm at the nape grading to 1.0 mm at the lips, 0.36 mm tall.
  - The alpha is the ring's own (`whorl.svg`): a rounded loaf with a steep trailing drop, split across the band into five scale rows, odd rows staggered half a girdle, each row out from the crest a step lower. The steps never rise away from the crest, so sand releases them.
  - C-R7's `whorl` sawtooth was tried first. Its trough and leading step read as gear teeth.
- **"Dorsal keel"** (tiling, Add): a 1.0 mm × 0.18 mm gable on the crest from the nape to about 25°. It is kept off the tail's tip, where the 384 × 192 release and the DFM gap floor failed.

## What I could not do

- **Make the face view read.** The face view stayed a "helmet, beetle or bullet" through three block-outs.
- **Satisfy the "no polished stretch" ask.** In the face view the camera looks past the 1 mm tail into the palm bore, so this ask conflicts with the 1 mm tail-tip ask.
- **Build the side-face spines.** Not attempted, because the block-out never passed.

### Rethink

Two suggestions for whoever picks this up:

1. Place or turn the head so the face camera sees it in profile or three-quarter, as in the hero, instead of straight down onto its crown. Possible ways:
   - a head turned 90° to lie across the band, biting a tail that crosses the top;
   - or a face view framed at an angle for this ring.
2. Accept lost wax for the head's dorsal plates. Then plate outlines can be grooves in any direction, not just terraces.

## Core changes wanted

None are required. One would help the template budget:

```rust
// sculpt.rs: a stored part's packed mesh is the dominant template cost; let `packed` take a face budget and
// decimate the closed sectioned solid before encoding (the same `decimate` + `settle` chain sculpt parts use).
pub fn packed_within(s: &Solid, max_faces: usize) -> Result<Packed>;
```
