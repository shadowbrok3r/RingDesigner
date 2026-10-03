# Cataphracta: Chamaeleo, cloud report

Branch `claude/cataphracta-chamaeleo`: `f337575` (block-out, read tests 1 and 2), `a55dc47` (merge of master `2e11632`), `4f9b152` (block-out attempt 3, read test 3), then this report. Nothing was pushed to master and nothing was tagged.

## Outcome

**Stopped at the block-out: three read tests failed.** Per TASK.md, the subject needs rethinking, not detailing. No review round was run, so **0 of 3 review rounds were used** and there is no review score. The template gate was not run, because it follows the last round and no round started.

| Read test | Design | `reads` | What the reviewer saw |
|---|---|---|---|
| 1 | The head in profile, struck on the parting line: a domed head plate, a cranium tier whose lower edge is the mouth, a casque tier, and a ringed turret eye (three cone tiers) holding the alexandrite cabochon as its pupil. Delft. | false | "An abstract stepped signet with a cabochon". The ringed eye read as "a target or a single eye", with no helmet in the silhouette. Changes: casque on the line, 1.8 mm high and 4 mm wide; remove the concentric rings; a 5 mm cushion boss; a tail coil 3.6 to 5 mm in radius on each cheek; granules on the table. |
| 2 | The plan above, in Delft: casque (Dome, 0.25 + 1.55 mm) on two closed temporal-crest tiers, a 5 × 4 cushion on a boss at along −4, granules cut at the bench on the table flanks, cast granules on the cheeks in a C-R1 clamped group, a dorsal keel and cones, and a tail curling on each cheek. | false | "A tribal or industrial signet": a stud-framed plate, a bullet-shaped boss, and "rivet rows". Changes: a broad helmet casque 1.8 to 2.0 mm high; a tail 4 to 5 mm in radius; open crest steps instead of a closed frame; pebbled granules; bigger cones. |
| 3 | Judged as **lost wax** (Logan, 2026-10-03). Broad domed casque (0.35 + 1.65 mm, 2.8 to 4 mm wide); two 0.3 mm temporal steps each side, beside it rather than under it; bench-cut granules on the flanks; coil grown to 2.1 mm in radius at the cheek's tallest corner; cones 0.6 + 0.9 mm. | false | "A carved cartouche, like a Maya glyph or a robot mask … perhaps a beetle, a fish or a dinosaur toy, but not a chameleon." The casque reads as a flat pill, the tail as a scroll, and the texture as pits and drips. |

The reviews are in `showcase/cataphracta/chamaeleo/read-test-{1,2,3}.json`. The renders in that folder are attempt 3's draft set. The attempt 1 and 2 renders were overwritten by later runs; the attempt 1 code is in `f337575`'s history.

## Why it does not read, and what I would rethink

- **The table is the only place both cameras look, and the factory table limits the subject.** In two-part sand, every form on the table must step down away from the parting line. That ruled out a plan-view head with its turret eyes, cast granules, and a coil on the table. The profile head (attempt 1) obeyed the rule, but its eye then had to sit on the line in the middle of the head. That gave the "target" read, and the turret's concentric steps are a target.
- **The tail cannot be big.** The 001 cheek at 17 × 14.5 is a crescent over the bore, 1.9 mm tall at its middle. The largest circle it holds has a radius of 1.78 mm (measured from the atlas; side faces with |n_z| > 0.85). All three reviews asked for a coil 3.6 to 5 mm in radius at the centre of the cheek. That is physically absent on this base, so the tail never reaches the size that names a chameleon.
- **Recommendation:** lost wax (now allowed) on a **taller procedural or keyed head**, or on a stock whose cheeks are not a thin crescent. Sculpt the head as a part (`sculpt.rs`), not as stacked plates: a real casque silhouette that rises above the table, turret eyes standing out on both sides, and the tail coiled on a side face at least 9 mm tall. The plates-on-a-flat-table approach reads as a glyph three times running.

## Gates (attempt 3, lost wax)

Lost wax judges fill and detail. The sand items are reported as a bonus and gate nothing.

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | yes, 0, 0 | yes, 0, 0 | yes, 0, 0 |
| Solids notes empty, every stamp struck | yes (13 of 13) | yes | **no**: "Tail, fingertip: could not be joined (two cuts cross inside a face)" |
| Bore margin (≥ −0.01 mm) | −0.00007 | −0.00018 | −0.00002 |
| Field verdict (lost wax), thinnest wall ≥ 0.8 mm | Castable, 1.296 mm | Castable, 1.296 mm | Castable, 1.296 mm |
| DFM findings | 0 | 0 | 0 |
| Stones reported = preview; metal inside the stone | 1 = 1; 0 | 1 = 1; 0 | 1 = 1; 0 |
| Triangles | 503,008 | 172,672 | 1,350,252 (≤ 2 M) |
| Casting pattern closed | — | — | watertight, 0 degenerate, 0 crossings (1,346,420 triangles) |
| Cold reload with an empty library (`--verify`) | — | — | identical |
| **Bonus: Delft pull** | blocked: 2 obstructions at 0.100 mm, 3 at 0.075 mm, deepest 1.03 mm; two-part undercut 0.015% | same | blocked; 7 of 13 stamps fail `parting_monotone` (the off-line crests, cones and tails) |

The export tail join is a fragile boolean that moves with resolution. It passed at draft and 384, and passed at export in an earlier configuration, but it is not fixed. Attempt 2 (Delft) was green on every sand gate at draft and 384: Castable, release 0 and 0 at both steps, clamp bites 0.000, DFM 0, monotone all true. It still failed its read.

**Step 0, the bare base:** 001 through the sand master at 17 × 14.5 is watertight, has 0 degenerates and pulls 0 and 0. In Delft it fields "Castable with care", 0.098% at −1.75° on the shoulder crest at 20–40° and 130°. In attempt 2, the knife keel down both shoulders lifted it to Castable. In lost wax the bare base reads Castable.

**Template gate:** not run. TASK.md runs it after the last round, and none started.

## What each part is (attempt 3)

- **Alexandrite:** a 5 × 4 cushion (`Gem { l_mm: 5.0, ..calibrated(Cushion, 4.0) }`), tinted [0.18, 0.50, 0.38], on a boss 0.8 mm high (crown 0.85, skirt 0.4) at along −4. Flush, through, with a drill mark; a top-level `Max` entry.
- **Casque:** a stamp from the occiput's station, `Dome { 1.65 }` over 0.35 mm eaves, a shield from along −0.9 to +7.6, 2.8 mm wide at the brow and 4.0 mm over the occiput.
- **Temporal crests:** four `Pillow` strips at 0.3 mm, two a side, the upper resting 0.25 mm in from the lower. In lost wax they need not cross the line.
- **Granules:** my own seeded largest-fit packing (`hide_tile`: radii 0.7, 0.5 and 0.33, lands 0.25). They are cast on the cheeks where |n_z| > 0.97, with masks drawn as SVG in the chart so they travel in the design. On the table they are cut at the bench (lands 0.35 mm deep), because the sand master's envelope fills a cast granule's shadow along the finger into a "drip".
- **Keel and dorsal crest:** knife `CurveLayer`s on the parting line down both shoulders (13.2 to 36 mm along), and eight graded cone stamps (`stamp_row`, `PartingLine`) on the occiput's shoulder.
- **Tail:** one stroke per cheek, 0.95 mm tapering to 0.42 mm, running along the crescent and coiling 1.6 turns at the cheek's tallest corner. Along the pull, 0.8 mm high.

## Enablers used

- **C-R1** (clamped group): attempt 2, cheek granules. Bites 0.000 after masking to faces square to the pull. Dropped in lost wax.
- **C-R4**: hide-space tilings and `##region:` masks (attempt 2); replaced by atlas-drawn SVG masks.
- **C-R7**: `granule_voronoi`, tried and replaced, because it packs too sparsely (below).
- **#248**: `StampTop::Pillow` (crests). `crisp_relief` was tried and changed nothing, because the drips were the envelope. It is left off, since the template lift cannot carry it yet.
- **Not used:** C-B2, C-V1 to C-V5, C-T5 to C-T7, #255, #258, #259.

## Core changes wanted

1. **`granule_voronoi` packs sparsely.** About 18 granules land in a 5 × 5 tile, because it throws each radius in turn and the large ones jam first. Choose the largest radius that fits each throw (`reptile.rs`, in `scatter`):
   ```rust
   for _ in 0..60_000 {
       let c = [next() * w, next() * h];
       let room = placed.iter().map(|(q, rq)| wrap_dist(c, *q, w, h) - rq - land).fold(f64::MAX, f64::min);
       if let Some(r) = radii.iter().map(|k| k * base).find(|r| *r <= room) { placed.push((c, r)); }
   }
   ```
   Draw consecutive coordinates from SplitMix64, not an LCG: consecutive LCG pairs lie on lattice planes.
2. **The sand envelope runs whatever the process is.** In lost wax it still fills every relief's shadow along the pull. In `imported_base.rs`, where the mesh build reads `sand_envelope`:
   ```rust
   let envelope = base.sand_envelope && design.draft.process != CastProcess::LostWax;
   ```
   Without the envelope, though, the 001 sand master self-crosses (364 crossings at draft). The master's own section also needs closing without it.
3. **DFM squashes hide-space cells.** `tiling_finest_mm_remapped` multiplies the cell height by `station_stretch` even when `t.space == ChartSpace::Hide`, whose cells are already true millimetres:
   ```rust
   let ch = if t.space == ChartSpace::Hide { ch } else { ch * v_scale.clamp(0.05, 8.0) };
   ```
4. **`parting_monotone` cannot pass a ridge off a symmetric station.** A stamp framed 5 mm from the head's centre has `frame.x[2] ≈ 0.004`, so its ridge sits 0.02 mm off the line at its far end. That fails the 1e-7 fall test until the frame is turned by hundredths of a degree. `frame_on` could project `x` onto the parting plane, as it already does for `y` across the line.

## Process note

Logan's 2026-10-03 rule, which judges the ring as lost wax, is recorded in the Chamaeleo section of `docs/collections/cataphracta.md`, with the stop. No extra rounds were granted or used.
