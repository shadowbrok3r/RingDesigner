# Cataphracta — Ouroborus, the girdled wheel: second cloud lane report (lost wax)

**Outcome: stopped at the block-out.** All three of this lane's read tests (4, 5 and 6) came back `reads: false`. Under TASK.md's rule the ring stops here: the subject needs rethinking, not detailing. That makes six failed read tests across both lanes.

- **Review rounds used:** 0. No full review ran, so there is no review verdict or score.
- **Gates:** every gate is green at draft (768 × 320), at 384 × 192, and at export (1536 × 448 with `--verify`).
- **Template gate:** now passes. The graph is 2,206,296 bytes against the 3 MB painted budget; it was 3.89 MB.

| Item | Value |
|---|---|
| Branch | `claude/cataphracta-ouroborus-2`, from `claude/cataphracta-ouroborus` with master `2e11632` merged in. Nothing was pushed to master or any other branch, and nothing was tagged. |
| Author file | `crates/ringdesign-core/examples/cataphracta_ouroborus.rs`, rewritten for the lizard. |
| Artwork | `crates/ringdesign-core/examples/cataphracta/art/ouroborus/` (`whorl.svg`, `whorl-spine.svg`) |
| Outputs | `showcase/cataphracta/ouroborus/` |
| Subject | *Ouroborus cataphractus*, the armadillo girdled lizard, biting its tail. |
| Process | **Lost wax**, under Logan's rule of 2026-10-03: 0.8 mm minimum section, no pull rule. It was Petrobond. |

I recorded both the subject and the process in the ring's section of `docs/collections/cataphracta.md`, where the old note said "snake's head". I also marked the section's original sand build steps (the "Serpent head" part and the "Head shield" group) as superseded, and set the status to "stopped at the block-out". No extra rounds were granted, so no extension line was added.

## Read tests (an independent reviewer each time; full text in `read-test-{1..6}.json`)

Attempts 1–3 are the first lane's, built as a serpent in Petrobond. Attempts 4–6 are this lane's, built as a lizard in lost wax.

| # | reads | What the eye saw (condensed) | Top changes asked |
|---|---|---|---|
| 1 | false | Hero: a plain bypass band with a helmet-like cap. Face: a faceted knuckle. | A real bite; a wedge head; graded girdles. |
| 2 | false | Hero: "serpent ring", but the girdles read as a cog. Face: a knight's helm or a beetle. | A head 1.4× as long as wide; a mouth line; rounded girdles. |
| 3 | false | Hero: nearly passes as "snake ring", with castellated girdles. Face: a beetle, helmet or bullet. | A flat wedge head with eye bumps; imbricate girdles; hide all over. |
| **4** | **false** | Hero: "snake ring" from the ouroboros layout, with square-topped girdles. Face: a turtle carapace, with a "flat-ended ruled cylinder" read as a pipe fitting at the bite. | Remove the "cylinder at the bite"; a flat wedge head with eye bumps; scaled, offset girdles. |
| **5** | **false** | Hero: "snake ring or dragon ouroboros". The legs read as "a stray burr", and the girdles as crocodile scutes. Face: a turtle shell or beetle, with a "pipe pushed into a clamp" in front. | A flat triangular head with large shields, a spiny occipital row and a neck; legs that break the silhouette; a thin tail into the jaws. |
| **6** | **false** | Hero: "the best attempt so far … the bite finally reads", but "dragon ouroboros or snake ring". The legs are "two small pale lumps" inside the silhouette, and the neck slats read as a radiator grille. Face: "a hose clamp or a spark plug". The eyes read as washers, and the shoulders and forelegs as a buckle. | Taper the head's plan to a snout under 40% of the hinge width; domed eyes; occipital spines instead of slats; legs splayed past the silhouette, and at 45° in plan. |

### What the six tests show

1. **The hero improved; the face view never read.** By attempt 6 the hero shows a reptile coiled round the finger, with a head, an eye, a thin tail visibly running into the jaws, and bristling girdles. Every face view was named as hardware or a shell: a helmet, a beetle, a turtle, a clamp, a spark plug.
   - The face camera looks straight down on the head, foreshortened by the ring's curve.
   - The body beside the head (neck, shoulders, forelegs) is seen end-on at a grazing angle, as a squared block. Two reviewers read that block as the thing in the jaws, so they read the head backwards: the occipital spines at the wide back corners looked like open jaws.
2. **Side-face legs are invisible at 300 px.** The legs are height fields lying along the flank. They stand 1.3–1.75 mm proud of the flank, but they stay inside the outer silhouette in the hero, and from above they show as rounded "barrels".
   - The reviewers asked for legs that break the silhouette, and that splay at about 45° in plan behind the neck.
   - That is a different construction: limbs standing out sideways from the band, not relief on its flank.
3. **The reviewers kept citing "colubrid", "Serpent head" and "Head shield"**, the old snake note and the old sand plan, even after I corrected the doc before attempt 5. Their changes still treated the subject as half-snake.

## Gates (block-out 6 as committed)

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Watertight / degenerate faces / self-crossings | yes / 0 / 0 | yes / 0 / 0 | yes / 0 / 0 |
| Made parts (head and four legs): open edges / crossings, as made and as placed | 0 / 0 | 0 | 0 / 0 |
| Solids and parts notes; parts joined | [] ; 5 of 5 | [] ; 5 | [] ; 5 of 5 |
| Bore: nearest vertex against 9.300 mm | 9.300, 0 inside | — | 9.300, 0 inside |
| Field, **lost wax**, parts judged in | **Castable**, thinnest wall 1.08 mm (≥ 0.8) | — | **Castable**, thinnest wall 1.08 mm |
| DFM findings | 0 | — | 0 |
| Stones reported / previewed | 0 / 0 | — | 0 / 0 |
| Investment pattern (`mf::prepare`) | closed, 0 / 0 | — | closed, 0 / 0 |
| Triangles (budget 2 M) | 600,072 | 292,952 | 1,384,528 |
| Cold reload, empty library | — | — | **identical** |

- **Ray release and draft clamp:** these do not apply in lost wax (no pull rule).
- **Sand bonus, which this design does not earn:** judged as a Petrobond pour, the ray release at 0.100 mm finds 132–143 obstructions, up to 2.12 mm deep. The head and legs undercut freely.

## Template gate

`collection_templates cataphracta … --only ouroborus --verify-export`, class `painted`. Record: `showcase/cataphracta/ouroborus/template-verification.json`.

| Item | Result |
|---|---|
| Nodes | 44 |
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **2,206,296 bytes** against the 3,000,000 budget, so `template_gate_passed: true` |
| Lifted source | identical |
| Mesh parity | 1,384,528 triangles; vertices, faces and normals identical |
| Cold design and graph reloads | pass |
| First build | 1.6 s |

The size fix: the head's stored mesh fell from 395 k to 154 k faces (240 stations × 160 section points). The four legs are about 17 k faces each.

`crisp_relief` is left **off**, because the graph lift cannot carry it yet.

## Enablers and master moves

- **Used:**
  - C-R2's `Spiral` grade on the girdles, with the seam hidden in the jaws.
  - C-R7's `whorl_spine` for the flank spines.
  - P5's station-aware `VGate::SideFaces` for the flank whorls and spines.
  - #248's framed close-ups (`render::write_png_framed` with `Framing`) for `stones.png`, `head-top.png` and `foreleg.png`; no cropped meshes.
- **Not used:**
  - C-B2 / #257 (stone plans): there is no stone.
  - C-V1–C-V5 and C-T5–C-T7: nothing in a block-out called for them.
  - #255 (Textura and marks), #258 (patterns along a path) and #259 (CAD fallbacks).
  - `crisp_relief`: see the template gate above.
- **Master merges:** I merged master `2e11632` once, before the first build. The only conflict was `cloud-report.md`, and I kept the branch's copy. No rounds ran after the block-out, so there were no further merges.

## What each layer and part is, and why

- **Base:**
  - Profile: `ProfileStyle::Flat`, 3.4 × 2.8 reference (the tail tip's width), crown 1.0, `flatten_sides()`, comfort 0.15, bore 18.6.
  - Fifteen keys, giving these widths:

    | Where | Width |
    |---|---|
    | Tail tip inside the jaws (74°) | 0.95 mm |
    | Lips (66°) | 1.0 mm |
    | Neck behind the head (114–126°) | 3.4 mm |
    | Body (150–228°) | 4.8–5.1 mm |
    | Mid-tail (300°) | 3.9 mm |
    | Tail (40°) | 2.1 mm |

  - The crown scale falls toward the tail so its section stays above the 0.8 mm fill (thinnest wall 1.08 mm).
- **"Head"** (a made part, stored sectioned solid, joined):
  - Placement and size: the snout is at 68°, so the face camera at 90° looks onto the middle of the skull. The skull is 10.1 mm to the back of the temple spines and 7.4 mm across the jaws (ratio 1.36). The plan is a straight-flanked triangle.
  - Crown: a flat crown that rounds over at its edges.
  - Shields: seven large ones (internasal, paired prefrontals, frontal, supraoculars, interparietal, parietals), cut as 0.28 mm V sutures. Lost wax lets the sutures run in any direction.
  - Spines: a spiny temporal and occipital rim of five backward-raking spines a side, standing 0.4–0.75 mm proud.
  - Face: a domed eye 0.95 mm in radius with a round pupil, under a brow; an oval ear pit behind the jaw; a nostril.
  - Mouth: a 0.45 mm mouth line with an upturned corner, and a 1.15 mm gape round the tail for its first 2–3 mm.
  - The back sinks into the neck over the last 2.2 mm.
- **"Foreleg, near/far"** at 140° and **"Hind leg, near/far"** at 226° (made parts, joined):
  - Each is a height field over the flank (bones as capsules), standing up to 1.75 mm proud, closed by a slab buried 0.3 mm under the flank.
  - Shape: a fat upper limb back from the shoulder, the lower limb bent down to the bore, a broad hand or foot, and four splayed tapering toes.
- **"Whorls"** (tiling, `Spiral` grade 2.6 → 1.0 mm, nape to the jaws, gated to the crown, 0.6 mm):
  - Alpha: `whorl.svg`, two girdles per tile. Each girdle is a rounded loaf rising to a U-shaped free edge that bows tailward, and the edge is toothed into five spiny scale points.
  - Alternate girdles are offset by half a point, so the hide reads imbricate.
  - A 0.07 mm blur rounds the trailing edge.
  - The fills stay above the 0.5 iso, so DFM sees no slivers.
  - The window fades out by 60°, which leaves the tail tip smooth into the jaws.
- **"Flank whorls"** (the same lattice at 0.35 mm, `VGate::SideFaces(Both)`) and **"Whorl spines"** (C-R7 `whorl_spine`, one spine per girdle on both flanks, 0.4 mm): the girdles carried down the sides.

## What I could not do

- **Make the face view read.** In both lanes, three different head designs (serpent, plated lizard, large-shield lizard) all read as hardware from directly above.
- **Make side-face legs read at 300 px.**

### Rethink (for whoever picks this up)

1. **Build the legs as limbs, not flank relief.** In lost wax there is no pull rule.
   - Splay the forelegs outward from behind the neck at about 45° in plan, past the band's outline, with the elbow out and the hand gripping the flank.
   - The hind legs can do the same at the hips.
   - From above, four splayed limbs are the one thing that says "lizard" before any surface. That needs a swept or lofted limb part, not a height field.
2. **Turn the face view's framing problem around.** Put the forelegs and the head both inside the face camera's useful span: the head's back at about 100°, the forelegs at 105–125°, the snout at about 55°, and the tail entering from the right.
   - Or, as the first lane suggested, a face view framed at an angle for this ring.
3. **Shrink what flanks the head in the face view.** The shoulders, seen end-on, became the "clamp" every time. A narrower neck and shoulders, with the limbs carrying the width, would remove it.
4. **Finish the brief's cleanup.** Strip the remaining sand-era build steps from the ring's doc section, so reviewers stop citing "Serpent head", "Head shield" and "colubrid".

## Core changes wanted

None are required. One would help the template budget of any stored sculpt:

```rust
// sculpt.rs: a stored part's packed mesh is the dominant template cost; let `packed` take a face budget and
// decimate the closed sectioned solid before encoding (the same `decimate` + `settle` chain sculpt parts use).
pub fn packed_within(s: &Solid, max_faces: usize) -> Result<Packed>;
```
