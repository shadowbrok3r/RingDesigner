# Bestiarium revival: Basiliscus — cloud report

Branch `claude/bestiarium-basiliscus-revival`, cut from `claude/bestiarium-basiliscus`. I merged `origin/master`
three times: at the start (`ad39165`), before round 4's final renders (`f741ef7`, the crisp renders of PR #248),
and at the start of round 5 (`d6fc2f9`).
- Example: `crates/ringdesign-core/examples/bestiarium_basiliscus.rs`.
- Outputs: `showcase/bestiarium/basiliscus/`.
- Nothing was pushed to master or any other branch, nothing was tagged, and no `src/` file was edited.

## Verdict: cut at 7.4

| Round | Verdict | Score | Reviewer's main reasons |
|---|---|---|---|
| 1 | revise | 6.5 | Land census missing; head a smooth bill; crown three dots; flanks combed |
| 2 | revise | 6.8 | Body lost its scales; chopped wall slabs; crown a bar with pins |
| 3 | cut | 7.2 | Head blocky at 3/4; crown balls on pyramids; bare polished walls; ruled scutes |
| **4 (revival)** | **revise** | **7.3** | Round 3's cut reasons largely fixed: the head is sculpted and the walls carry scales. Still failing: ruled palm scutes and planks past the morph, frond-like hackles with stepped ends, a gecko-like head, a seam at the neck, braille-like pits, and sheared corner scales |
| **5 (revival, last)** | **cut** | **7.4** | The best Basiliscus yet. For the first time the 3/4 view names a crowned snake, the field is clean and the neck seam is gone. Still under the bar: the hackles read as fronds with stepped ends; the crown points read as pawns (balls on stalks) with decimation facets; the skull reads smooth with no plates; the coil and boss walls streak; the corner wall scales shear; the palm scutes still read as bars |

- The reviews are `review-round4.json` and `review-round5.json`. The earlier ones are in `previous/` on the seed
  branch.
- Ship needed 7.5. Under TASK.md the reviewer's verdict stands, so the ring is cut.

## Gates (round 5, final build)

Every gate was green at draft and at export. The reviewer checked each one against the JSON files.

| Gate | Draft 768 x 320 | Export 1536 x 448 |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings, notes empty, bore clear | pass (545,250 triangles) | pass (1,790,518 triangles, under 2 M) |
| The same at 384 x 192 | pass (226,336 triangles) | pass |
| Every made part closed and uncrossed (the seat and the sculpted head) | pass | pass |
| Casting pattern watertight, 0 degenerate faces, 0 crossings | pass | pass |
| Lost wax Castable at 0.8 mm fill (thinnest wall 1.669 mm) | pass | pass |
| Land census: every section at or above 0.8 mm, or named | pass | pass |
| 0 DFM findings | pass | pass |
| One stone in report and preview, no warnings or crowding | pass | pass |
| `--verify` cold reload identical | pass | pass |

`report.json` holds the export run and a `draft` block; `draft-gates.json` is the full draft report.

Land census (export). The sculpted-head rows are measured on the decimated head mesh, as round 4's reviewer asked:
each is the narrowest chord through the feature over 40 directions.

| Feature | Land, mm | Status |
|---|---|---|
| Boss wall round the girdle | 0.849 | at or above 0.8 |
| Boss rim at the bright-cut bevel | 0.630 | named: chamfer recut and burnished over the stone |
| Painted body to the tail's tip | 0.800 | at or above 0.8 |
| Crown band height | 1.152 | at or above 0.8 |
| Crown points, plate at mid-height | 1.085 | at or above 0.8 |
| Crown points, waist under the pearl | 0.932 | at or above 0.8 |
| Crown points, side petal | 0.668 | named: investment detail |
| Crown pearls | 0.947 | at or above 0.8 |
| Fang near its tip | 0.718 | named: investment detail |
| Tongue stem | 0.602 | named: investment detail |
| Tongue tines near their ends | 0.399 | named: investment detail |
| Bordure beads and rim beads | 0.440 | named: burnished |

## Template gate (after the last round)

Run with `collection_templates bestiarium … --only basiliscus --verify-export`, class `painted`; the result is in
`template-gate.json`.
- **design.set patches:** 0 (the limit is 4).
- **Graph size:** 2,750,332 bytes against the 3,000,000-byte budget for a painted ring, so no size review is needed.
  The graph has 56 nodes.
- **Cold source:** identical.
- **Cold design and graph reloads:** passed.
- **Export mesh parity:** passed (1,790,518 identical triangles).
- **Round 4 note:** the first run came to 3,432,917 bytes, over budget. I re-decimated the head from 135k to about
  80k faces to bring it under.

## What the ring is now

**Stock:** native 020 escutcheon, unmirrored. Lost wax, `min_section_mm` 0.8, Silver 925 investment.

**Sculpted part, `Basilisk's crowned head`.** A stored mesh made with `ringdesign_core::sculpt` the way Fenrir's wolf
was, and joined at the table. The distance field is meshed at 0.045 mm and decimated to 83,434 triangles (896 KB
packed). The mesh is closed with 0 crossings and stands 4.5 mm over the table.
- **Skull:** a wedge with a flattened snout top and a canthus ridge from each nostril to the brow. A smaller eye
  (socket, ball and slit pupil) sits under a brow shelf. Nostrils, and eight domed head shields parted by V-grooves.
- **Jaws:** low scalloped labials. The lower jaw is set in its own, less-rolled frame, so it drops across the table
  rather than into it.
- **Fangs and tongue:** a fang hangs on each side, and a forked tongue lies on the jaw and the field.
- **Crown:** five points on a flared band that hugs the skull. Each point is a fleur, with the pearl on the leaf's
  tip and two blade petals, and two cabochons sit on the band.
- **Pose:** the skull rolls 28° toward the viewer, so both the face camera and the hero camera see the crown in
  silhouette.
- **Neck:** it copies the painted coil's own surface, scales and all. It stands 1.5× proud behind the head and eases
  down to 0.85 of the paint's height, so the two skins share every scale and meet along one line.

**Painted layers:**
- `Basiliscus`: the coil round the stone. Where it laps the boss it rides the boss's wall as a 0.35 mm fillet.
- `Beaded bordure`: one bead row round a plain polished field. Round 4's pits are removed.
- `Rim beads`.
- `Wall scales`: domed round scales in true mm down the walls under the table. They start with a 1 mm ramp and fade
  out over the last 1.5 mm above the bore.
- `Hackles into scales`, with the bench layer `Graver's barbs and keels`.
- `Belly scutes`: curved, shingled scutes, each lip rolling over the next plate. They hand over to keeled dorsal
  scales at 0.84 mm pitch between 28° and 46° from the palm. The dorsal scales cross over to the mantling between
  60° and 76°.

**Seat:** `Tsavorite, flush`, an 8 × 4 mm marquise in a boss with a bright-cut bevel.

**Stamps:** none. The eye, pupil, nostril, fangs, tongue and crown that used to be stamps are now part of the sculpt.

**Renders:** `stones.png` and the new `head.png` are framed close-ups made with `render::write_png_framed`, not cropped
meshes. I also added `contact-300.png`.

**Format:** the design is written at format 6. `crisp_relief` is off on purpose: on this unmirrored stock it outlined
the wall scales on the right-hand wall, and the scales read better without it.

## What I could not do

- **Hackles.** They were the P1 item in rounds 3, 4 and 5, and I did not redraw them. I only softened their edges (a
  0.36 mm roll), thinned the midrib to 0.035 mm, raised the lift and eased the sickle to 10°. The truncation near
  atlas x 340 (θ ≈ 60) is a deliberate mask. Removing it gave 43 self-crossings on the shoulder wall (θ 57,
  z −2.9), so I put it back with a wider 1.1 mm ramp. A real fix needs the hackle field redrawn so it does not fold
  in that concave corner.
- **Head plates and crown read.**
  - The eight head shields (0.1 mm domes, 0.09 mm V-grooves) are in the field, but the reviewer could not see them on
    the decimated 83k-face mesh.
  - The crown's fleurs still read as pawns, and the band shows decimation facets.
  - Fixing both needs either more faces (the template budget has about 250 KB left) or decimation that preserves
    curvature.
- **Grazing streaks.** The coil's outer wall and the boss wall still streak at grazing light. The fillet onto the boss
  only acts where the coil laps it.
- **Right-hand wall.** The wall under the chief shows sheared, outlined scales on the right-hand side and not on the
  left. The stock is unmirrored, and the painted relief resolves worse on that side.

## Core changes wanted (exact code)

1. **Measure land on a closed mesh**, for any sculpted or stored part. This is the example's `chord_land`; it would
   go in `crates/ringdesign-core/src/cad/measure.rs`:

```rust
/// The narrowest chord through `p` of the closed mesh `m` over forty directions on a golden spiral, mm: the land a
/// thin feature has at `p`. `f64::MAX` when `p` is not enclosed.
pub fn chord_land(m: &crate::csg::Solid, p: [f64; 3]) -> f64 {
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let hit = |o: [f64; 3], d: [f64; 3]| -> f64 {
        let mut best = f64::MAX;
        for t in &m.f {
            let [a, b, c] = t.map(|i| m.v[i as usize]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let h = cross(d, e2);
            let det = dot(e1, h);
            if det.abs() < 1e-14 {
                continue;
            }
            let s = sub(o, a);
            let u = dot(s, h) / det;
            let q = cross(s, e1);
            let v = dot(d, q) / det;
            let t = dot(e2, q) / det;
            if (0.0..=1.0).contains(&u) && v >= 0.0 && u + v <= 1.0 && t > 1e-9 {
                best = best.min(t);
            }
        }
        best
    };
    (0..40)
        .map(|k| {
            let z = (k as f64 + 0.5) / 40.0;
            let (r, th) = ((1.0 - z * z).sqrt(), k as f64 * 2.399_963);
            let d = [r * th.cos(), z, r * th.sin()];
            hit(p, d) + hit(p, [-d[0], -d[1], -d[2]])
        })
        .fold(f64::MAX, f64::min)
}
```

2. **Decimation that preserves curvature**, for sculpts that must fit a template budget. `decimate`'s `max_turn_deg`
   is fixed at 18° today; this lets a caller set it, in `crates/ringdesign-core/src/sculpt.rs`:

```rust
pub fn clean_decimate_turn(raw: &Solid, target: usize, max_turn_deg: f64) -> Solid {
    for (k, cap) in [2e-3, 1e-3, 5e-4, 2e-4].into_iter().enumerate() {
        let nets = decimate(raw, target + 20_000 * k, cap, 2.0 + k as f64, max_turn_deg, 35.0);
        if csg::self_crossings(&nets) == 0 {
            return nets;
        }
    }
    decimate(raw, raw.f.len(), 0.0, 0.0, 0.0, 180.0)
}

pub fn clean_decimate(raw: &Solid, target: usize) -> Solid {
    clean_decimate_turn(raw, target, 18.0)
}
```

   A tighter turn limit (8–10°) on the crown and plates is the most direct route to round 5's punch item 2.

3. **A per-layer opt-out of `crisp_relief`** (not written). A ring could then keep crisp walls on its table and stamps
   but leave one painted wall layer smooth. The shape I have in mind is a `crisp: Option<bool>` on `LayerEntry`, read
   where `imported_base.rs:1056` checks `d.crisp_relief`. I have not written it because it touches the build's cell
   logic.
