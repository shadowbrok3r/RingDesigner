# Cataphracta — Ouroborus, the girdled wheel: second cloud lane report (lost wax)

**Outcome: stopped at the block-out, twice.**
- This lane's first three read tests (4, 5 and 6, with flank-relief legs) all came back `reads: false`.
- Logan then granted one more block-out with real limbs (2026-10-04). Its three read tests (7, 8 and 9) also came back `reads: false`.
- Across both lanes that makes nine read tests, none of which read. The ring stops here.

| | |
|---|---|
| Review rounds used | 0. No full review ran, so there is no review verdict or score. |
| Gates | Every gate in the lost-wax set is green at draft (768 × 320), at 384 × 192, and at export (1536 × 448 with `--verify`). |
| Wall census | 21 small real wall zones, recorded below and not fixed, because no detailing round ran. |
| Template gate | Passes: 2,032,843 bytes against the 3 MB painted budget. |
| Branch | `claude/cataphracta-ouroborus-2`, with master `2e11632`, then `803a93e`, merged in. Nothing was pushed to master or any other branch, and nothing was tagged. |
| Author file | `crates/ringdesign-core/examples/cataphracta_ouroborus.rs` |
| Outputs | `showcase/cataphracta/ouroborus/` |
| Subject | *Ouroborus cataphractus*, the armadillo girdled lizard, biting its tail. |
| Process | Lost wax (Logan's rule of 2026-10-03): 0.8 mm minimum section, no pull rule. |

The ring's section of `docs/collections/cataphracta.md` records:
- the lizard subject;
- lost wax;
- the 2026-10-04 extension, as a dated line written before read test 7;
- its outcome.

No round extension was granted, and the doc says so.

## Read tests (an independent reviewer each time; full text in `read-test-{1..9}.json`)

| # | Build | reads | What the eye saw (condensed) |
|---|---|---|---|
| 1–3 | First lane: a serpent, Petrobond | false | Hero: from a bypass band up to "nearly snake ring". Face: a knuckle, helm, beetle or bullet. |
| 4 | Lizard, flank-relief legs | false | Hero: "snake ring". Face: a turtle carapace with a "pipe fitting" at the bite. |
| 5 | Head wider than the body, eyes on the outline, toothed girdles | false | Hero: "snake or dragon ouroboros"; the legs are "a stray burr". Face: a turtle shell or beetle, "a pipe pushed into a clamp". |
| 6 | Large-shield flat head, spiny temple rim, bigger feet | false | Hero: "the bite finally reads", but it is still a dragon or snake. Face: "a hose clamp or a spark plug". |
| **7** | **Real limbs:** capsule bones splayed about 45° in plan past the outline, hands gripping the flank | **false** | Hero: the bite reads; the limbs are "pale pegs and nubs". Face: "pencil-thin" struts, "a bracket, a tripod or a beetle's legs". |
| **8** | Limbs 2× thicker, elbows and knees rising over the band's shoulder, five-toed hands; triangular head | **false** | Hero: "a thin diagonal bar" and "an X-shaped strut". Face: "a clevis … a beetle's mouthparts". The head is "a shield or lantern", and the temple spines read as a crest comb. |
| **9** | Limbs posed elbow-back with the hand forward, and knee-forward with the foot back; flat crown; eye set back; girdles to the nape | **false** | Hero: the limbs are "hinge pins or the pins of a hinged bangle clasp", and the head is "a squared block". Face: "a coffin- or lantern-shaped shield", with the tail "like a spring". |

### What the nine tests show

- **The bite is solved.** From attempt 6 on, every reviewer saw the tail thin and run into the jaws.
- **The limbs never read as limbs at 300 px.** Built as straight capsule bones, they read as rods, struts, prongs or pins, however thick or however posed.
  - My own 1000–1600 px renders (`foreleg.png`, `hero.png`) show a bent arm with a five-toed hand. At 300 px the cylindrical, polished bones and their round caps dominate, and the elbow joint reads as a knob.
  - A limb needs organic form: a muscled taper, a wrist, knuckles, and a smooth blend into the shoulder.
  - The 0.5 mm shoulder fillet (`Component::blend_mm`) ran for over 10 minutes against a 30-second draft build. I dropped it, and the reviewer of test 9 asked for it again.
- **The head never left "box, shield, coffin, lantern" in either view.** The sectioned head is a constant-topology sweep. However the plan is tapered, a flat crown with near-vertical side walls reads as a block from the side, and as a tiled shield from above.
- **Reviewers kept repeating girdle asks across tests 4–9:** loaf tops, graded pitch, bristling edges, a smaller tail tip. That suggests the girdles' grade and spines don't read at 300 px either. The flank spines are 0.5 mm, and the grade runs 2.6 → 1.0 mm.

## Gates (block-out 9 as committed)

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Watertight / degenerate faces / self-crossings | yes / 0 / 0 | yes / 0 / 0 | yes / 0 / 0 |
| 33 made parts (the head and 32 limb capsules) closed and uncrossed, as made and as placed | pass | pass | pass |
| Solids and parts notes; parts joined | [] ; 33 of 33 | [] ; 33 | [] ; 33 of 33 |
| Bore: nearest vertex against 9.300 mm | 9.300, 0 inside | — | 9.300, 0 inside |
| Field, lost wax, parts judged in | Castable, thinnest wall 1.08 mm | — | Castable, thinnest wall 1.08 mm |
| DFM findings / stones | 0 / 0 = 0 | — | 0 / 0 = 0 |
| Investment pattern | closed, 0 / 0 | — | closed, 0 / 0 |
| Triangles (budget 2 M) | about 600 k | 283,170 | 1,389,402 |
| Cold reload, empty library | — | — | identical |

- **Ray release and draft clamp:** these do not apply in lost wax.
- **Sand bonus, not earned:** judged as Petrobond, 126 obstructions, up to 2.1 mm deep.

### Lost-wax wall census

`cad::measure::thickness(&built.mesh, 0.8)` at export, read as the lead's interim gate. It is recorded in `report.json` under `wall_census`.

| Measure | Value |
|---|---|
| Rays | 160,213, 0 unresolved |
| Wall samples | 866 (5.99 mm²) |
| Edge samples | 3,010 (13.6 mm²) |
| Wall zones | 23 |

**Suspected census artifacts (under 0.05 mm):**

| Thinnest | Area | Where |
|---|---|---|
| 0.000 mm | 4.43 mm² | at 68°, r 11.03, z −0.02: the snout tip on the tail |
| 0.009 mm | 0.00 mm² | at 91°, r 12.08, z −2.61 |

**Real sections under 0.8 mm, recorded and not fixed:**

| Where | Thinnest | Area |
|---|---|---|
| The gape's lips round the tail, 74–75° | 0.17 mm and 0.20 mm | about 0.65 mm² each |
| Further lip spots at 75° | 0.16–0.65 mm | — |
| Hind-leg toes at 240° | 0.55 mm | — |
| Brow and eye rims at 92–110° | 0.07–0.78 mm | each under 0.01 mm² |
| Spine and toe tips at 118–124° | 0.54–0.56 mm | — |

- These would be the first fix in a detailing round: a thicker waist inside the gape, and blunter tips. No round ran, so they stand as found.
- Per the lead's note, they are reported and do not block on their own.

## Template gate

`collection_templates cataphracta … --only ouroborus --verify-export`, class `painted`. Record: `showcase/cataphracta/ouroborus/template-verification.json`.

| Item | Result |
|---|---|
| Nodes | 72 |
| `design.set` patches | **1** |
| Graph size | **2,032,843 bytes** against the 3,000,000 budget, so `template_gate_passed: true` |
| Lifted source | identical |
| Mesh parity | 1,389,402 triangles, identical |
| Cold design and graph reloads | pass |
| First build | 2.5 s |

`crisp_relief` is left off, because the lift cannot carry it yet.

## Enablers and master moves

- **Used:**
  - C-R2's `Spiral` grade on the girdles.
  - C-R7's `whorl_spine`.
  - P5's station-aware `VGate::SideFaces`.
  - #248's framed close-ups (`render::write_png_framed`).
  - Master #262's wall census (`cad::measure::thickness`).
- **Tried and dropped:** `Component::blend_mm` for the shoulder fillets, as above.
- **Not used:** C-B2 / #257 (there is no stone), C-V1–C-V5, C-T5–C-T7, #255, #258 and #259.

## What each layer and part is

- **Base:**
  - Profile: `Flat` 3.4 × 2.8 reference, crown 1.0, flat sides, bore 18.6.
  - Fifteen keys, giving these widths:

    | Where | Width |
    |---|---|
    | Tail tip inside the jaws | 0.95 mm |
    | Neck | 3.4 mm |
    | Body | 4.8–5.1 mm |
    | Tail | tapering round to the jaws |

- **"Head"** (a stored sectioned solid, snout at 68°):
  - Shape: 10.6 × 7.5 mm (ratio 1.41), a triangle widest at the occipital edge. The crown is flat and rounds over at its edges.
  - Seven large shields cut as 0.28 mm sutures.
  - Four small occipital spines at the back corners, on the upper rim only.
  - A smooth domed eye at 6.2 mm from the snout, under a brow; an ear pit; a nostril.
  - A 0.45 mm mouth line, and a 1.15 mm gape round the tail.
  - The back sinks into the neck.
- **Limbs** (32 stored capsules, joined; tapered cones between tangent spherical caps):
  - Each foreleg (root at 132°): an upper arm out and back to an elbow over the band's shoulder, standing about 2 mm past the flank; a forearm forward and down to a hand pad on the flank under the neck; five clawed toes fanned forward.
  - Each hind leg (root at 228°) mirrors it: knee forward, foot back, toes raking tailward.
- **"Whorls," "Flank whorls" and "Whorl spines":**
  - The girdles run nape to jaws on a `Spiral` grade from 2.6 to 1.0 mm.
  - On the crown each is a rounded loaf with a U-shaped, toothed, spiny free edge, offset half a point girdle to girdle.
  - The girdles continue lower down the flanks, with C-R7 spines at 0.5 mm.

## Rethink (for whoever picks this up)

1. **Sculpt the limbs, don't assemble them.** A swept limb with a muscled profile is the next try. It would run from a shoulder blended into the body, through a wrist, into a broad hand with knuckled, tapering, clawed fingers. It would be one closed sculpt per limb, built like the head's sections, or a sculpted stored mesh. Capsule bones read as hardware at every thickness tried.
2. **Rebuild the head with a rounded, cheeked side profile:**
   - jaw muscles bulging behind the eye;
   - a downturned snout;
   - an overhanging brow;
   - side walls that lean in, rather than near-vertical walls under a flat crown.
3. **Fix the gape's thin lips** (0.17–0.20 mm at 74–75°) before any detailing round.
4. **Make the shoulder fillet affordable.** If `blend_mm` on stored parts can't be made faster, sculpt the fillet into each limb's root directly.

## Core changes wanted

None are required. Two would help:

```rust
// sculpt.rs: let a stored part's packed mesh take a face budget (decimate + settle before encoding).
pub fn packed_within(s: &Solid, max_faces: usize) -> Result<Packed>;
// cad: a cheap root fillet for joined stored parts, local to the contact ring instead of a global blend.
pub struct Component { /* … */ pub root_fillet_mm: f64 }
```
