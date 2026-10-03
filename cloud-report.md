# Tenebrae — Oculus concept spike: final report

**Outcome: none of the three rethinks read.** I blocked out each option and had a fresh independent reviewer read-test it. I then revised each option once with that reviewer's changes, and a second fresh reviewer read-tested the revision. All six read tests came back `reads: false`. The verdicts are evidence for Logan's decision; I chose nothing for him.

Branch `claude/tenebrae-oculus-spike`, with master `2e11632` merged in. Example: `crates/ringdesign-core/examples/tenebrae_oculus_spike.rs <flange|head|stones> [OUT_DIR] [--rev]`. Outputs are in `showcase/tenebrae/oculus-spike/<option>/`: the first block-out at the top level, the revision in `rev/`, and `read-test.json` and `read-test-2.json` beside them. This file replaces the core lane's `cloud-report.md`, which master carried in, on this branch only.

**Process (Logan, 2026-10-03):** every option is judged as **lost wax**: 0.8 mm minimum section, no pull rule. The decision is recorded in Oculus's section of `docs/collections/tenebrae.md`. No option comes out Castable in Delft sand, so there is no sand bonus to report.

## The table

| Option | What it is (one sentence) | Process | Read test 1 | Read test 2 | What the eye saw | Main risk |
|---|---|---|---|---|---|---|
| **flange** | One raised flange on the band's high edge carries the whole wheel: a hub moulding, lancets on spokes and a ring of quatrefoils, pierced through, with 6.9 mm of radial run in block-out 1 and 7.7 mm in the revision. | Lost wax. Sand: **Will not release** (the flange's underside locks the cope, as expected). | **false** | **false** | Test 1: "a daisy or sunflower rosette… ball-bearing cage, perforated flange washer". A primed jeweller "might say wheel window" from the face. Test 2: "flat pierced washer… bearing cage, lamp-shade ring, sunflower medallion". The face suggests "rose window, maybe". | **Read:** the face is close, but the hero shows a washer on a pipe. The cusps the reviewers want are sharp points that fail `thickness(0.8)` at this scale. **Gate:** block-out 1 is clean (minimum 0.848 mm). The revision's two-order rim leaves a sliver at r 15.75 (minimum 0.13 mm, 6 rays). **Wear:** the flange stands 3.9–5.2 mm proud of the crown all the way round, palm included. |
| **head** | The wheel stands up off the crown as a vertical disc across the band, square to the finger like a gable's oculus. Block-out 1 is a 12 mm wheel on a splayed plinth. The revision is a 15 mm wheel standing proud of a drop-arch gable that rises out of the crown. | Lost wax. Sand: **Castable with care**, not Castable. | **false** | **false** | Test 1: "a small round pierced disc standing on top, like a coin or a charm… daisy or sunflower petals… flower medallion". Test 2: "teardrop-shaped plaque… sunflower pendant ring". "Rose window" came only "on a second look". | **Read:** short radial lights in a 12–15 mm wheel read as petals, and the gable reads as a teardrop or a bail. **Gate:** block-out 1 is clean (minimum 0.883 mm). The revision has a seam sliver where the sill and gable meet the crown (minimum 0.03 mm, 3 rays) and one note that a fillet pinches. **Size:** the gable's apex stands 16.6 mm off the crown. |
| **stones** | The wheel's lights and foils are stained-glass stones in collets, each set through. Block-out 1 puts them on the high side face of a 7 × 6 band: 12 marquise sapphires as the lights and 12 rubies as the foils. The revision puts them on the vertical gable wheel: a 3 mm ruby boss, 8 marquise sapphires between raised mullions and 8 ruby foils. | Lost wax. Sand: **Will not release** (the through-burs carry no draft). | **false** | **false** | Test 1: "a jewelled watch bezel, a compass rose or a gem-set flange washer". Test 2: "a gem rosette, a compass rose or a flower plaque on a signet or keyring tab". It "might be called 'a round window' as a guess". | **Read:** stones laid on a plate read as petals or a bezel. The reviewers want them *inside* pierced lights between bars. **Gate:** the collet walls (0.35 and 0.30 mm, normal in jewellery) fail `thickness(0.8)` everywhere, with 82 and 47 rays below the floor. The stone record counts 24 and 17 stones, one per collet built. |

The gates for all six builds at draft 768 × 320: watertight with 0 degenerate faces, ring self-crossings 0, every CAD feature `Ok`, no vertex inside the bore (minimum radius 9.300 against a bore radius of 9.300), and 0 DFM findings. The numbers are in each folder's `spike.json` and `rev/spike-rev.json`.

### 300 px renders

| | Read test 1 | Read test 2 (revision) |
|---|---|---|
| flange | `showcase/tenebrae/oculus-spike/flange/hero-300.png`, `showcase/tenebrae/oculus-spike/flange/face-300.png` | `showcase/tenebrae/oculus-spike/flange/rev/hero-300.png`, `showcase/tenebrae/oculus-spike/flange/rev/face-300.png` |
| head | `showcase/tenebrae/oculus-spike/head/hero-300.png`, `showcase/tenebrae/oculus-spike/head/face-300.png` | `showcase/tenebrae/oculus-spike/head/rev/hero-300.png`, `showcase/tenebrae/oculus-spike/head/rev/face-300.png` |
| stones | `showcase/tenebrae/oculus-spike/stones/hero-300.png`, `showcase/tenebrae/oculus-spike/stones/face-300.png` | `showcase/tenebrae/oculus-spike/stones/rev/hero-300.png`, `showcase/tenebrae/oculus-spike/stones/rev/face-300.png` |

## What each revision changed (one round, from the first reviewer's list)

- **flange:** 24 lancets became 18 on 1.1 mm spokes, with the arrises splayed at 45° (0.5 mm of flat left on top). The quatrefoils grew to 1.5 mm. The field is sunk 0.6 mm in two orders. A chamfered roll 0.45 mm proud runs round the rim, and a smaller one round the hub. **Not done:** cusped trefoil heads. A cusp is a metal point, and at a 0.8 mm minimum section it cannot sit inside a 3 mm light. The old Oculus report found the same.
- **head:** 8 lancets became 12, the quatrefoils 1.4 mm, and the wheel grew from 12 to 15 mm, with the rim in two orders and a drop-arch gable behind it. **Not done:** lights 2 mm wide (twelve lights with 0.8 mm bars in a 15 mm wheel leave about 1.1 mm each), and ornament on the band.
- **stones:** the reviewer asked for a raised round head facing away from the finger, which is Rosa. To stay distinct from Rosa I used the vertical gable wheel instead, with a ruby boss at the hub, raised mullions and the rim in two orders. **Not done:** arcading on the shoulders.

## What the reviewers kept saying (six tests, three options)

1. **Short radial lights at ring scale read as petals.** Every test said daisy, sunflower or flower medallion first, across three wheels, two sizes and 8 to 24 lights. At 300 px the wheel window has no silhouette of its own. What tells it apart is cusps, a second foiled order and moulded depth, which are the sub-millimetre detail that the 0.8 mm floor and the 300 px read both remove.
2. **Gothic comes from the frame, not the wheel.** Every reviewer asked for an equilateral pointed-arch gable, stepped mouldings, and arcading or buttresses on the shoulders. That is a building round the window.
3. **Colour has to sit in openings.** Stones on a plate read as a bezel or a rosette. Stones that read as glass need pierced cells between bars, which puts collets inside lights roughly 1–3 mm wide.
4. **Every option drifts toward Rosa:** a round window, with stones, on a head.

## If it were my call (a recommendation for Logan, not a decision)

**I would not build Oculus as a wheel-window ring. I would fold the wheel into Rosa and give Oculus's place in the collection to a subject with a silhouette of its own.** Six independent reviewers, across a flange, a head and stones, saw a flower before they saw a window. All their fixes add a pointed gable, cusps and coloured glass in pierced cells. That is Rosa's brief, and Rosa already has the table and the stones to carry it. The one thing Oculus uniquely had was the finger as the oculus, seen along the band. It failed in the original three read tests, and failed again here as the flange ("a washer on a pipe").

If Logan wants to keep Oculus, the strongest lead is the **head**: a 13–15 mm vertical wheel in an equilateral gable, cast in lost wax, with stones set à jour *inside* its pierced lights and blind lancets on the shoulders. Both head reviewers and the second stones reviewer converged on it. The face camera sees it square, which no Rosa table does. It costs a head about 16 mm off the crown, collet walls that need a waiver or 0.8 mm walls, and a design that sits close to Rosa.
