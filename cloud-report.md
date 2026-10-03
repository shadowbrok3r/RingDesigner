# Fenrir revival: final report

**Verdict: cut at 7.0** after the two revival rounds (round 4 revise 7.0, round 5 cut 7.0). The ship bar is 7.5. Every gate is green at draft and at export, and the template gate passes on the submitted design.

- **Branch:** `claude/bestiarium-fenrir-revival`, pushed. It was cut from `claude/bestiarium-fenrir-r3` and merged with `origin/master` three times: at the start, before round 4 (PR #248, crisp edges), and at the start of round 5. Conflicts arose only in this file, and this report was kept each time.
- **Checkout:** `/home/user/RingDesigner`. The session had no `/home/user/repo`, so a symlink points there.
- **Code:** `crates/ringdesign-core/examples/bestiarium_fenrir.rs` is the only code file changed. There are no `src/` edits.
- **Outputs:** in `showcase/bestiarium/fenrir/`.
  - Renders: `hero face palm side shoulder reverse stones bare-vs-finished` `.png`, `hero-300.png`, `face-300.png`, a new `contact-300.png` sheet, and three framed close-ups: `close-head.png`, `close-ruff.png` and `close-binding.png`.
  - Data and sections: `design.ring.json`, `report.json` (with a `draft` block), `mesh.json`, `stones.json`, `verification.json`, `section-70/90/110.png` and `artwork/`.
- **Reviews:** `review-round4.json` and `review-round5.json` sit beside the outputs. Rounds 1–3 are in `target/previous/` on the seed branch.

## Verdicts, all five rounds

| Round | Verdict | Score | What held it |
|---|---|---|---|
| 1 | revise | 5.5 | Grille muzzle, goggle brow, porthole lips, speckled skin; no land-width gate |
| 2 | revise | 6.5 | Lip frame, bead teeth, terraced ruff, a sealed void |
| 3 | cut | 7.0 | Bulldog/gargoyle face: short muzzle, horseshoe flews, floating lower jaw; combed ruff |
| **4 (revival 1)** | **revise** | **7.0** | Head moved the right way and face-300 now reads as a wolf. The ruff, cheek fur and bindings did not move. verification.json was stale (a gate fail). |
| **5 (revival 2)** | **cut** | **7.0** | Flews and muzzle closed and the template gate is green. The ruff still shows terraced, sawtooth rims, the cheek fur is melted blobs, and the bindings are jagged lozenges. The nose pad reads as a faceted block. Seam crease run is 1.09 mm against 0.7. |

The round-5 reviewer's own words: the 300 px hero and face "read as a wolf's head biting a moon". The face-read problem the revival was opened for is solved. What cut the ring is the small-scale work: the painted ruff, the cheek fur and the bindings.

## Gates (round 5, the submitted design)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, degenerate faces | yes, 0 (417,318 tris) | yes, 0 (1,226,212 tris, within 2 M) |
| Self-crossings: ring; parts (stone, Fangs, head, hollow) | 0; [0,0,0,0] | 0; [0,0,0,0] |
| Solids / parts notes | empty / empty | empty / empty |
| Nothing in the finger hole | innermost 9.491 / bore 9.500 | same |
| Lost-wax verdict, 0.8 mm fill | Castable, thinnest wall 1.32 mm at 256° | same |
| Land widths | nothing unnamed, head mass ≥ 0.887 mm | same |
| DFM findings | 0 | 0 |
| Stones report / preview | 1 / 1, 4.147 ct, no warnings | same |
| Investment pattern | watertight, 0 degenerate faces, 0 crossings | same |
| Cold reload (`--verify`) | identical | identical vertices, faces and normals |
| Closed internal voids | 0 | 0 |
| Hollow, θ70 / 90 / 110 | over 1.21 / 1.11 / 1.20 mm, floor 1.16 / – / 1.17 mm | same |
| Weight, 18k | 31.77 g (limit 32) | 31.78 g |

**Named land-width exceptions.** Every other section is 0.8 mm or more.

| Feature | Min section | Sub-floor from its tip | Bench treatment |
|---|---|---|---|
| Ears, left / right | 0.58 / 0.55 mm | within 0.76 mm | Ear point: a thick root tapering to the tip; cast in place and polished |
| Fang sheaths 1–4 | 0.50–0.52 mm | within 1.65 mm | Lie on the moon's dome round their claws; closed onto it at the bench |
| Fang claws 1–4 | 0.44–0.46 mm | within 1.55 mm | Claw tips; closed over the girdle |
| Lower right premolar | 0.72 mm | within 0.11 mm | Tooth point; polished |
| Nose | 0.61 mm | at the nostril rims | Nostril commas; polished |

**Determinism (round-3 item 10).** The head is now built with `ringdesign_core::sculpt` (`tetra_mesh`, `relax`, `clean_decimate`, `settle`, `open_shells`, `packed`, `Stock`, `Heights`). The example's own copies of those tools are gone. The draft and export runs gave the same packed head, FNV-1a `5fd2acdb59fcbfc4`, which `report.json` records under `composition.head.packed_fnv1a`.

## Template gate (after the last round)

`collection_templates bestiarium target/tpl-src --only fenrir --verify-export`, with class `painted`:

| | |
|---|---|
| `design.set` patches | **0** (4 allowed) |
| Graph size | **2,326,787 bytes** against the painted budget of 3,000,000, in 39 nodes |
| Cold design reload | pass |
| Cold graph reload | pass |
| Source and mesh parity | pass: 1,226,212 identical triangles at 1536 × 448 |
| Detail findings | 0 |
| `template_gate_passed` | true |

`verification.json` was written from this run, after the final `design.ring.json`.

## What each part is

- **Stock:** factory 010 Trillion, 17 mm wide, 19.0 mm bore. Lost wax, `min_section_mm` 0.8, Gold 18k.
- **#2 Moonstone:** a 10 mm round cabochon, 4.147 ct, lifted 0.4 mm and tilted 4°.
- **#3 Fangs:** P6 Fang style, Jaws grouping, Point tip, 1.8 mm wire, no rails, rise 0.6. These are the four canines curving over the dome.
- **#4 Fenrir's head:** a stored sculpt of 117,772 triangles marched at 0.09 mm. It is the wolf.
  - Muzzle and nose:
    - The muzzle is a tapering wedge from the stop at u 7.3 to the nose, 7.05 mm long, with a top plane.
    - The nose pad is 2.6 × 1.7 mm and stands off the bridge behind a crease, with comma nostrils.
  - Eyes, brows and ears:
    - Slanted eyes sit under the brows at the stop.
    - The ears are tall, 3.9 mm, each with a deep cup and three strands rising from the tuft.
  - Cheeks: the cheek bulk sits back by the eyes and narrows toward the mouth's corners. It carries three tiers of pointed, grooved locks sweeping back toward the ears.
  - Upper jaw:
    - One smooth flew each side, dipped 0.65 mm at the canine.
    - It hangs over the cheek teeth behind the fang, so only incisors and fangs show above.
  - Lower jaw:
    - Tied to each cheek by a masseter at the corner.
    - Chin reach 7.22 mm with fur; lower-jaw-to-muzzle ratio 1.02.
    - The lower teeth alternate high and low and are drawn out into blades.
  - Throat, flank fillets, and crown fur that stops short of the skull's back.
- **#5 Hollow under the head:** a cut part of 21,244 triangles, giving 1.11–1.21 mm of metal over it.
- **Painted layers:**
  - **Ruff** (0.6 mm, 262° window): flame locks 7.5 mm long on a 1.6 mm pitch. They lean 42° beside the head and 20° along the shoulders, and the alpha is blurred by 1.5 texels.
  - **Graver's hair lines** (0.08 mm): bench only.
  - **Gleipnir** (0.6 mm): a two-strand cord in a seamless tiling at the palm.
  - **Gleipnir's bindings** (0.5 mm): two SVG decals of five wraps each at θ236 and θ304, feathered 0.35 mm.
- **Stamps:** none.
- **Format:** the design is written at format 6 because it carries stored meshes. It uses no `crisp_relief` and no `Pillow` top; see below.

## What changed in the revival, by punch item

**Round 4, against round 3's list:**
1. **Muzzle.** The stop moved from u 6.3 to 6.9 and the nose to u 0.95. Stop to nose went from 5.5 to 6.66 mm, and the muzzle became a wedge.
2. **Flews.** They were broken at the fang. A notch first left 0.06 mm slivers against the sheath, so it became a smooth dip in the crest.
3. **Jaw.** A hinge mass now joins each cheek to the lower jaw. The chin's measured reach went from 8.0 to 7.22 mm, and the ratio from 1.45 to 1.08.
4. **Ruff.** Locks are wider and fewer and sweep 24° along the band.
5. **Seam.** The ruff is painted up the fillet.
6. **Lower teeth.** They alternate in height and are longer.
7. **Cheek fur.** Three tiers of pointed, grooved locks replace the log-polar squiggles.
8. **Ears.** Deeper cups with strands inside.
9. **Bindings.** Wider gaps between the wraps.
10. **Determinism.** The head is built on `ringdesign_core::sculpt` and checked by its digest.

Round 4 also added a `contact-300.png` sheet, framed close-ups, measured chin reach and muzzle ratio in `jaws`, and a `draft` block in `report.json`.

**Round 5, against round 4's list:**
1. **Template gate.** Re-run on the submitted design.
2. **Ruff.** Locks lengthened to 7.5 mm on a 1.6 mm pitch with a 20° sweep, and the alpha blurred.
3. **Cheek locks.** Broader and fewer, with a shallower groove.
4. **Muzzle and nose.** 7.05 mm, a tapered bridge, and a distinct nose pad with a crease and nostrils.
5. **Flews.** One smooth lip covering the cheek teeth.
6. **Seam.** The head's fur runs further down its flank.
7. **Bindings.** Feather raised to 0.35 mm.
8. **Lower jaw.** The fur on its rim is roughly halved.

**Gate defects found and fixed on the way.** All of them came from the reshaping, and all were caught by the land-width gate:
- Fur floating over the crease between cheek and lip. The guard that keeps fur out of hollows now fades in gently instead of switching off along a line.
- A thin flange under the slimmed mandible.
- The moon's keep-out cylinder running on below the table and slitting the throat. The keep-out now stops 0.8 mm under the table.
- A sharp corner on the gums' flat floor, now rounded.
- Crown-fur ledges on the skull's back slope.
- One attempted fix made things worse and was backed out: a shell over the stock produced 279 thin samples.

## What I could not do

- **The ruff's outline.** The punch list asked three rounds running for smooth lock rims: painting at 2–3× the atlas resolution, or striking the locks as stamps. I did neither.
  - At 2–3× the atlas, the embedded ruff alpha would take the 2.33 MB template past the 3 MB painted budget.
  - Twenty-odd lock stamps on curved walls was beyond the time left.
  - Blurring the alpha smoothed the sawtooth but turned each lock's flanks into visible contour terraces.
  - This is the main reason for the cut.
- **The cheek fur.** It still reads as melted blobs at close range. Locks raised from a field along normals that converge on a curved cheek smear. A sculpted lock set, built as separate round cones on the cheek, would likely read cleanly.
- **The bindings.** They are still small jagged lozenges in the close-up. Stamps would be the honest fix.
- **The nose.** Its pad reads faceted. The trapezoid plan with rounded corners shows straight edges, and an ellipsoidal or pillow pad would fix it.
- **Seam crease run** is 1.09 mm against the 0.7 asked for.
- **`crisp_relief` is off.** It made the ruff and the bindings straighter. With it on, every gate passed, but the template gate failed, which is core change 1 below.
- **Round-trip with the lead.** I made no core edits. The claw `around` parameter (round 3's core change) has still not landed, so the canine sheaths remain.

## Core changes wanted

1. **`design.set` must accept `/crisp_relief`.** Without this, any design carrying it cannot be lifted to a template. Serde skips the field when it is false, so the lift's base design has nothing at that pointer, and the node refuses it: "a design has nothing at /crisp_relief". Node #44 then fails upstream. In `crates/ringdesign-graph/src/nodes/assembly.rs`, `design_set`:

```rust
let optional = matches!(pointer, "/cad" | "/manufacturing" | "/casting_trials" | "/imported_base" | "/stamps" | "/crisp_relief")
    || (pointer.ends_with("/bench_only")
        && json.pointer(pointer.trim_end_matches("/bench_only")).is_some_and(|v|v.get("layer").is_some()));
```

   Better still, make every `skip_serializing_if` field on `RingDesign` optional here, or have the lift express `crisp_relief` as its own node input. A test should lift a design with `crisp_relief = true` and hold it to byte parity.

2. **Round claws** (carried from round 3), so the canine sheaths can go. Add a sides count to `setting::ClawOptions`:

```rust
pub struct ClawOptions { pub style: ClawStyle, pub grouping: ClawGrouping, pub tip: ClawTip, pub rise: f64, pub around: u32 }
// in claw_head: let around = if options.around == 0 { 14 } else { options.around.clamp(6, 64) as usize };
// tube(&path, &rs, around, dome) / tube_oval(&path, &oval, around, dome)
// builders.rs schema: params.push(whole("around", "Sides", 6.0, 64.0, 14)); read with v.0.get("around").and_then(Json::as_u64).unwrap_or(0)
```

   A count other than 0 or 14 should fence the design at format 6.

3. **A super-sampled painter.** Add `skin::Atlas::paint_ss(name, k, f)`, which averages a k × k grid of sub-texel samples, each interpolated between neighbouring `Sample`s. A painted pelt's crease lines between overlapping locks would then anti-alias at the atlas's own size, with no 4× alpha and no budget hit. This is what the ruff needed.
