Corvus round 2 is finished and committed. Every gate is green at 384×192, 768×320 and 1536×448, and the template gate passes. I score it 6.5 against Caiman's 7. The heads now read as birds with hooked bills. The weak points are the face view and a fold on one flank of each skull (see "Could not do").

**Where it is**
- Branch `bestiarium-corvus`, HEAD `518df5a`, worktree `/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner/.claude/worktrees/bestiarium-corvus`. The tree is clean.
- Round-2 commits: `ee0670d`, `9e6cb50` (master merge), `f85a6b3`, `8ff81ec`, `63bb259`, `518df5a`. Nothing pushed or tagged.
- Renders are in `showcase/bestiarium/corvus/`:
  - `hero.png` (yaw 0.62, pitch 0.70, Muninn's open side), `face.png`, `side.png`, `palm.png`, `shoulder.png` (Huginn in profile), `reverse.png`, `stones.png`, `head.png` (Muninn close-up) and `bare-vs-finished.png`, all at 1600 px.
  - `hero-300.png` and `face-300.png`.

**Gates**

| Gate | 384×192 | 768×320 | 1536×448 |
|---|---|---|---|
| Watertight, degenerate faces | pass, 0 | pass, 0 | pass, 0 (1,355,766 triangles) |
| Self-crossings (ring and 73 made parts) | 0 | 0 | 0 |
| Stamps placed / notes | 72 of 72, none | 72 of 72, none | 72 of 72, none; 0 parting-monotone failures |
| Release at 0.100 mm | 0/0 (parting −0.025) | 0/0 (−0.024) | 0/0 (−0.006) |
| Release at 0.075 mm | 0/0 (−0.027) | 0/0 (−0.015) | 0/0 (−0.006) |

These results do not depend on build resolution:
- **Field:** Castable at 0.0216% (0.30 of 1,378 mm²), worst −0.99°, thinnest wall 2.28 mm. No undercut is attributed to any layer.
- **Draft clamps:** collet bed 0.0018 mm, plumage 0.0076 mm, heads 0.0000 mm; 0 texels cut.
- **DFM:** 0 findings.
- **Stones:** 1 reported, 1 previewed, 1.19 ct, 0 tight pairs.
- **Bore:** clear (margin −4.5e−7 mm).
- **Cold reload:** identical.
- **Pattern:** 1,360,666 triangles, closed, 0 degenerate, 0 crossings.
- **Collet wall:** 0.47–0.58 mm.

**Template gate:** 0 `design.set` patches, 131 nodes, 1,800,226 bytes (60% of 3 MB). Cold design, graph and editable-graph reloads all pass. Export geometry is identical at 1536×448, with 0 detail findings.

**What each piece is**

Layers:
- **Onyx, made collet:** gypsy pad 0.25 mm, girdle 0.30 mm below its top, made bezel soldered after the pour, 0.8 mm drill dot in the pattern.
- **Collet bed:** fills up to the collet's base, falling 3.5° across, so only a single lip shows.
- **Raven plumage:**
  - crossed necks that gather 1.1 mm into the collet's foot;
  - chevron contour feathers graded from 1.25 to 2.4 mm pitch, each row stepping 0.22 mm;
  - one wedge tail per bird at the palm, 5 rectrices each.
- **Raven heads:**
  - egg-plan skull 5.6 mm long, crown 2.9 mm, flat ±0.45 on the parting line;
  - forehead step 0.53 mm over 0.94 mm; flat crown 3.2 mm; nape dip about 0.95 mm;
  - brow terrace 0.32 mm deep for the eye;
  - painted beak 6.0 mm, arching from 2.35 to 1.25 mm, with a 0.6 mm hook and a ±0.30 flat;
  - painted on the bare surface faired along the ring.
- **Graver's work** (bench-only, 0.10 mm): rachis and barbs in every feather, 6 nasal bristles per beak, and a gape line from the mouth corner under the eye to the beak tip.

Stamps (72):
- 4 bench-cut almond eyes, 1.1 × 0.7 mm, flat floor, 0.4 mm deep.
- 20 throat hackles, 2.0 × 0.6 mm, rising from 0.22 to 0.38 mm.
- 28 primaries, 8.5 × 1.0 mm, 0.25 to 0.45 mm proud, 7 per side face per bird, tips toward the palm.
- 20 neck feathers on the crossing's side faces.

Made part: the collet (1,728 faces). Stone: an 8 × 6 black onyx cabochon.

**Per-item account**
1. **Release — met.**
   - 0/0 at both pitches at all three builds. The draft and 384 release blocks are in `report.json` with θ, r, z and depth fields (all empty).
   - `stamps_384` now requires both releases to be clear.
   - The crest-row mechanism is removed by construction: every painted apex keeps a ±0.30 flat on the interpolated crest, the fan tail is gone, and so is the struck beak.
   - One setting is tuned: the skulls are levelled at 0.15 of the bare crest's lean. At 0.25 a 0.031 mm hit returned on Muninn's skull at 384×192/0.075. At 0 the field went Marginal (0.056%).
2. **Composition — met, with one deviation.**
   - Heads sit at the arm tips with beaks pointing outward. Huginn: skull θ 113.6–139, beak 140–167, hook ending at 169. Muninn is mirrored: skull 66.4–41, beak 40–13, hook ending at 11.
   - The crossed necks are feathered right up to the collet, and the nest smear is gone.
   - Deviation: the beak is painted, not struck. With 0.33 mm edges and a 0.45–0.6 mm ridge, the struck lens rendered as a faceted plate; the painted bill with its hook and gape line reads as a raven's bill.
   - The face view foreshortens the bills (they lie 50–80° from the top), so it reads as two skulls flanking the stone.
3. **Heads — mostly met.**
   - The profile numbers are listed above. The eyes sit in the front third, in a step that falls away from the parting line. There are no lanes on the skull, and the graver is masked out of the head footprint.
   - Short: each head has 7 hackles on one side face but only 3 on the other, where the bypass tip step breaks the face.
4. **Feathers — met.** Primaries, hackles, necks, graded chevron rows and bench-only herringbone are all in place.
5. **Collet — met.** Plain wall 0.47–0.58 mm, single lip, stones 1/1, drill dot kept. In the new hero, Muninn's head and bill cover more of the frame than the collet and stone. I judged this by eye, not by pixel count.
6. **Tails — met.** Two opposed wedges of 5 rectrices each: 1.2 mm wide, 10.5 / 8.6 / 6.7 mm long, stepping 0.22 mm. Release is 0/0 at θ 285–305.
7. **Sand slots — met by naming, not by closing.** There are 27 slots at 0.100 mm (31 at 0.075 mm); 18 and 15 of them are under 0.30 mm. Each is listed in `report.json` under `sand_slots` with θ, r, z, width, zone and bench treatment. By zone: feather tips on the flanks 19, neck and collar feathers 6, nape dips 4, beak tips 4.

**Could not do**
- **Fold on each skull's tip side** (Huginn's +z flank, Muninn's −z flank). The bypass arm ends under the skull, so the bare surface's normal turns about 12° within 2° of ring angle. Relief 2.9 mm tall displaced along those normals shears about 0.6 mm sideways. Fairing the base under the head cannot remove that shear. It shows in `shoulder.png` and `side.png`; the hero and `head.png` are rendered from the open side.
- **Hackles:** 3, not 6–8, on each tip-side face.
- **Sand slots:** named with their bench treatment, not closed.

**Proposed core change** (opt-in, so no existing bypass design moves):
```rust
// profile.rs, ShankStyle (fence at format 6 when non-zero via library::format_version_for)
/// Degrees along the ring a bypass's arm union is faired over; 0 keeps the hard union.
#[serde(default)]
pub bypass_fair_deg: f64,

// profile.rs
pub fn bypass_span_faired(off: f64, k: f64, fair_deg: f64) -> (f64, f64) {
    if fair_deg <= 0.0 {
        return bypass_span(off, k);
    }
    const N: i32 = 12;
    let (mut lo, mut hi, mut sum) = (0.0, 0.0, 0.0);
    for i in -N..=N {
        let t = i as f64 / N as f64;
        let w = (1.0 - t * t).powi(2);
        let (l, h) = bypass_span(off + t * fair_deg, k);
        lo += w * l;
        hi += w * h;
        sum += w;
    }
    (lo / sum, hi / sum)
}
// profile.rs:2710, ShankKind::Bypass in modulation():
//   let (lo, hi) = bypass_span_faired(off, k, self.bypass_fair_deg);
```
Setting this to about 4° on Corvus would turn the arm tip's re-entrant corner into a ramp and remove the skull fold at its source.