# Heloderma — the beaded one: cloud lane report

**Outcome: stopped at step 0. The block-out failed all three read tests, so the subject needs rethinking before any detailing.**

- Rounds used: **0 of 3.** The loop never left the block-out. TASK.md says that when the third block-out still does not read, the lane stops and reports.
- There were no full reviews, so there is no score and no ship/revise/cut verdict.
- Branch: `claude/cataphracta-heloderma`.
- Files:
  - Author file: `crates/ringdesign-core/examples/cataphracta_heloderma.rs`.
  - Artwork: `crates/ringdesign-core/examples/cataphracta/art/heloderma/`.
  - Renders, `report.json` and the three read tests: `showcase/cataphracta/heloderma/`. The renders and `report.json` are from the last block-out, at draft resolution.

## Read tests (independent reviewer, read-test mode, 300 px hero and face)

| # | reads | What the eye sees (reviewer's words, shortened) | Changes the reviewer asked for |
|---|---|---|---|
| 1 | **false** | "A plain domed gold band covered all over in one even size of round bobbles, like a sea-urchin shell, a golf ball or a hammered 'bubble' texture … many bobbles read as pits … the small orange stone looks like a loose bead." | 1. Banding in relief: high 0.36–0.40 mm against low 0.10–0.14 mm, in forking bands 2.5–3.5 mm wide. 2. Face anatomy: the swell, a graded crest row, the stone in a made mound. 3. Round cushioned beads, lands ≤ 0.42 × pitch. |
| 2 | **false** | "Very little has changed … still reads as sea-urchin shell … smooth patches look like worn spots … no crest row … the swell is not visible." Flagged that the clamp's 0.24 mm bite erased the high/low step. | 1. Carry the banding by texture state (full domes against near-smooth ground), keep the bite ≤ 0.05 mm. 2. Crest anatomy: visible swell, a raised mound with a collar, a dorsal row of larger beads, fix the DFM finding on that row. 3. Beads, not pits. |
| 3 | **false** | "The stone is the one clear gain: it now sits centred in a visible bezel collar … The rest still reads as a sea-urchin shell … the beads fall into evenly spaced vertical columns and diamond clusters … like the tubercle rows on an urchin test, rather than irregular, forking Gila bands … 'beaded band with an orange cabochon', or 'urchin ring'." | 1. Five to seven irregular, forking, non-periodic saddles over θ 30–150, beaded against plain; bite ≤ 0.05 mm. 2. A dorsal row visibly larger than the field, 1.1 mm tapering 35 %, 0.42 mm proud. 3. A visible fat-tail swell in the hero; convex beads with no craters and no teardrops. |

The full JSON is in `showcase/cataphracta/heloderma/read-test-{1,2,3}.json`.

## What was tried, and what I learned

The subject failed at 300 px for one physical reason and one compositional one.

1. **On a sand crown, the round beads a Gila needs cannot stand where the camera looks.**
   - The ring's own half-round (8.0 × 3.2) keeps under 24° of base draft for 2.1 mm either side of the crest. I measured this with `FieldContext::draft_at` at θ 30, 90, 180 and 270.
   - Any bead off the crest line has a flank that rises away from the parting line. The live clamp (C-R1) shaves that flank:
     - bites measured 0.17 mm, then 0.24 mm, 0.27 mm and 0.19 mm;
     - it leaves half-beads, which the reviewers read as pits, teardrops or, with an offset focus, pointed scales ("arrows").
   - The generator's `bead_lattice` uses a quarter-circle dome, which stands vertical at its rim. No flank short of a side face releases that.
2. **Height contrast does not read at 300 px.** The spec's high and low beads (0.36 against 0.24 mm, later 0.40 against 0.12 mm) render as one even texture. The bands only began to show when the black bands were beads and the salmon bands were near-smooth ground (attempt 3). Even then the reviewer read the regular 16-band period as "columns and diamond clusters", like an urchin.
3. **The crest ribbon cannot carry off-crest beads in sand.**
   - Transverse "shingle" loaves vary only round the ring (G4), so they are legal there. They rendered as combed ribs, and I dropped them.
   - Raised saddle plates were also legal. They read as "worn spots".

### Findings worth keeping for whoever rethinks it

- **An ogive crown makes beads legal almost everywhere.**
  - The half-round's superellipse sharpened to `shape_a = 1.1`, `shape_b = 1.2` (style `Custom`, no drop curve) holds 28–40° of draft from 0.4 mm off the ridge to the edge. The half-round holds under 24° within 2 mm of its crest.
  - The dorsal row straddles the ridge (G2). The section reads as a lizard's back and gives a silhouette distinct from the other rings.
  - The Gila's beads need a flank of about 35° to stand 0.34 mm tall at a 1.5 mm pitch.
- **A raised-cosine bead with its peak 0.3 of its radius toward the band edge nearly fits a 30° flank.** This is the ring's own `bead_svg`. In the last block-out, the bead layers alone bite at most 0.070 mm, on 1.5 k texels in the first row off the ridge. The 0.188 mm group bite is the palm pavers' (0.184 mm).
- **Clamp slack 0.8 turned the field verdict from "Will not release" to Castable.** At slack 1.0 the clamp leaves walls at exactly zero draft, and the field sampler reads them 1–2° under.
- **A `SolidKind::Bezel` collet on a sunk gypsy mound** (height 0.45, `set_depth_mm` 0.35) gave the "visible made setting" the reviewers asked for.

## The last block-out (read test 3)

| Layer or part | What it is | Why |
|---|---|---|
| Base | Procedural 8.0 × 3.2, crown sharpened to an ogive (1.1, 1.2), comfort fit 0.2, edge round 0.3, bore 18.6. `Keyframes` swell: 1.22 / 1.28 / 1.05 at 90°, 1.08 / 1.10 at 35° and 145°, 0.96 at 210° and 330°, 0.90 / 0.92 at 270° | The fat tail. The ogive gives draft for beads |
| **Beadwork** (group, `SandClamp` 2048 × 768, slack 0.8) | The C-R1 live clamp over the group's composite | Makes the `Add` and `SmoothMax` composite legal |
| ↳ Beads (sub-group): **Low beads — salmon bands** (0.02 mm) and **High beads — black bands** (0.34 mm lift, masked by **Reticulation**) | Ring-owned raised-cosine bead lattice, 2 × 2 hex cell, 1.50 mm pitch graded Cosine 0.30 (C-R2) to 1.05 mm at the palm, lands 0.42 × pitch. One row pitch off the ridge, mirrored. `VGate::Draft { min 3, fade 22 }` (P5), `except(90, 14)` round the stone, `except(270, 70)` for the palm | Gila beadwork in black and salmon bands |
| ↳ **Belly pavers** | `reptile::svg::paver`, 78 round × 10 across, terraced at 0.20 mm, `around(270, 70)` | Gila belly scales |
| **Reticulation** (SVG mask) | 16 transverse bands at half share, drawn at the grade's own stations, straight across a 0.8 mm crest ribbon (G4), wandering and forking on the flanks | Gila banding |
| **Dorsal bead row** | Bare `SeatRun` (C-R3) of gypsy mounds, 1.8 mm, 0.6 mm tall, taper 0.35, bridge 0.35, `Add`ed over the group, `except(90, 22)` | Spine beads |
| **Spessartite** | 3.0 mm round, preview tint (0.95, 0.38, 0.06), on a gypsy mound 0.45 mm, sunk 0.35 mm, in a `Bezel` collet, raised 0.5 mm drill dot, `through` | The salmon bead, set in a made setting |

There are no stamps and no CAD parts. The "Graver: bead lands" bench layer was not built, because detailing never started.

### Gates at draft (768 × 320) for the last block-out, from `report.json`

| Gate | Result |
|---|---|
| Watertight, 0 degenerate faces | pass (492,714 triangles, 0 degenerate) |
| 0 self-crossings | pass (0) |
| Solids notes empty; every stamp resolved | pass (no notes, no stamps) |
| Nothing in the finger hole | pass (closest vertex 9.29998 mm against a 9.30 mm bore, 0 inside) |
| Field verdict Castable | **fail**: "Castable with care", 0.102 % undercut, worst −6.4° |
| Ray release 0 / 0 at 0.100 and 0.075 mm | **fail**: 50 and 66 obstructions, 0 unresolved |
| Clamp bite ≤ 0.05 mm | **fail**: 0.188 mm: the palm pavers 0.184 mm, the beads 0.070 mm |
| 0 DFM findings | pass (0) |
| Stones reported = previewed | pass (1 = 1) |
| Casting pattern closed | pass (watertight, 0 degenerate, 0 crossings) |
| Export build (1536 × 448), `--verify`, 384 × 192 | not run: step 0 renders at draft |
| Template gate | not run: TASK.md runs it after the last round, and the lane stopped before round 1. `design.ring.json` is 107.5 KB, a painted-class design with no atlas PNGs |

The failing gates were detailing work, and detailing does not start until the block-out reads. None of them were chased.

## What I could not do

- Make a pattern-only Gila read at 300 px in sand on this base, in three attempts.
- Meet the brief's bite target and keep beads round near the crown's parting line. The two pull against each other on any dome.
- Core changes were not needed to build the block-out; everything ran on master's C-R1, C-R2, C-R3, P5 and C-R7.

## Recommendation for the rethink

1. **Give it a figurative anchor.** The Bestiarium shipped on one iconic silhouette (Arachne, Manticora). A pattern-only Gila was read three times as "urchin". A Gila head at the face (faces are now allowed), or the whole lizard lying along the crest with the tail swell under the stone, would name the animal. The beadwork would then say which lizard.
2. **Or pour Heloderma in lost wax,** as Logan did for Chelonia and Phrynosoma. Round, full, touching beads anywhere on the dome are what a Gila's hide is. In sand they are only legal on the parting line and the side faces.
3. **If it stays in sand, keep the ogive crown and the raised-cosine bead.** Break the 16-band period into irregular saddles, which is the third reviewer's point 1, and redesign the palm pavers with ramped crest-side edges; they cause the largest bite.

## Core changes wanted (exact code)

1. **A cosine fall for the reptile bead generators,** so a bead off the crest can ramp under the draft. This is the fall the ring's own `bead_svg` uses. It goes in `core/reptile.rs` in `mod svg`:

```rust
enum Fall {
    Dome { bevel: f64 },
    Cone { tip: f64 },
    /// A raised cosine from peak to rim: no vertical wall at the rim, so an off-crest bead's
    /// crest-side flank is a ramp no steeper than (pi/2) * height / run.
    Cosine,
}
// in Fall::iso
Fall::Cosine => 0.5,
// in Fall::stops
Fall::Cosine => (0..=16).map(|k| { let t = k as f64 / 16.0; (t, 0.5 + 0.5 * (std::f64::consts::PI * t).cos()) }).collect(),
// in bead_lattice (and granules, if Moloch wants it): the dome parameter picks the fall
let fall = if p.dome() >= 1.0 { Fall::Cosine } else { Fall::of(p) };
page.bead(q[0], q[1], d, fall, p.focus);
```

2. **Where the clamp bit, in the report,** so an author need not repaint the composite to find it. This goes in `core/skin.rs`:

```rust
pub struct ClampReport {
    pub texels_cut: usize,
    pub worst_mm: f64,
    /// Ring angle and chart v of the deepest cut, when anything was cut.
    pub worst_at: Option<[f64; 2]>,
}
// in draft_clamp, replacing the final loop
let (mut texels_cut, mut worst, mut at) = (0usize, 0.0f32, None);
for (i, (b, c)) in before.iter().zip(&alpha.data).enumerate() {
    if b - c > CLAMP_NOTICE { texels_cut += 1; }
    if b - c > worst { worst = b - c; at = Some([a.samples[i].theta, a.samples[i].v]); }
}
Ok(ClampReport { texels_cut, worst_mm: worst as f64 * height_mm, worst_at: at })
```
