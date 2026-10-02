# Vepres ring: Ilex (`ilex`), cloud report

**Verdict: cut at 6.3 after round 3** (ship bar 7.5). It used 2 block-out read tests and all 3 review rounds. Every gate was green at every reviewed round, and so was the template gate. The ring failed on art, not on a gate.

- Branch: `claude/vepres-ilex` from `master` at `72b9857`.
- Commits: `e78fe13` (block-out), `d10741e` (round 1), `b43a137` (round 2), `ff09d9b` (round 3), then this report.
- Example: `crates/ringdesign-core/examples/vepres_ilex.rs`.
- Outputs: `showcase/vepres/ilex/`. That folder holds the renders, `report.json`, `verification.json`, `template.graph.json`, the read tests and the reviews. The STL files are git-ignored.
- Run: `target/release/examples/vepres_ilex [OUT_DIR] [--draft] [--verify] [--blockout] [--resize-check]`.

## Read tests and reviews

| Step | Result | Score | What the reviewer said |
|---|---|---|---|
| Read test 1 (block-out) | **reads: false** | — | Two stones either side of one 8 × 5 leaf read as "a monster face with a toothy grin". Asked for the berries gathered into a bunch at the sprig's heart, two leaves end to end, and prouder relief. |
| Read test 2 (block-out) | **reads: true** | — | "Holly, at once": two spined leaves end to end with a tight bunch of three red berries where their stems meet. Weakness: "a signet with a holly emblem", small and flat. |
| Round 1 | revise | **5.6** | Holly reads, but the execution is below the bar. The bench-cut wreath frame read as "a torn, dashed border". Leaves were flat, vertical-walled plates with stair-steps. Cheek berries sat in rows on cones and read as rivets. Shoulder leaves were isolated "bat silhouettes". Density was well below Caiman's. |
| Round 2 | revise | **6.2** | Wreath gone. The cheek sprays were now "the best passage". The garland was continuous. Still failing: the face leaves (fold and notch, still 6.3 mm), the matte as two hard panels, stair-steps, the garland as a "crenellated fringe", and the bark reading as combing. |
| Round 3 | **cut** | **6.3** | Identity holds at 300 px, and the cheek sprays and berry triangles are good. Still failing: face leaves "bats or crowns, not curved blades", the matte panels, the unsplayed garland plates, stair-steps, and density short of Caiman's. |

The reviews are `read-test-1.json`, `read-test-2.json` and `review-round1.json` to `review-round3.json`, all in `showcase/vepres/ilex/`.

## Step 1: the 16 × 17 resize (settled)

Factory 006 through the sand master goes straight from its native 16 × 21 face to 16 × 17 with no baked step. It builds watertight with 0 degenerate faces. The envelope fill is **0.016 mm** at 123.5° (limit 0.3), and the bare pull shows 0 obstructions and 0 unresolved rays at 384 × 192 and at 0.075 mm. The native 16 × 21 fill is 0.018 mm. The example measures both on every run and falls back to 16 × 21 (leaf lengthened) only if 16 × 17 stops building clean. The table is 16 × 17, the bore is 18.6 mm and the alloy is 18k yellow gold.

## Gates (final build, round 3)

The process is Delft clay, two-part sand: 3.0° draft, 0.8 mm section, 0.30 mm detail.

| Gate | Draft 768 × 320 | 384 × 192 | Export 1536 × 448 |
|---|---|---|---|
| Triangles | 540,128 | 220,356 | **1,353,792** (limit 2,000,000) |
| Watertight, degenerate faces | yes, 0 | yes, 0 | yes, 0 |
| `csg::self_crossings` (ring; no CAD parts) | 0 | 0 | 0 |
| `solids.notes` / `parts.notes` | empty / empty | empty / empty | empty / empty |
| Bore margin (minimum vertex radius minus bore radius) | −4.7e-7 mm | −4.7e-7 mm | −4.8e-7 mm |
| Field verdict (`attributed_field_report` + `judge_parts`) | **Castable**, 0.000% undercut | Castable | Castable |
| Ray release at 0.100 mm (obstructions / unresolved) | 0 / 0 | 0 / 0 | 0 / 0 |
| Ray release at 0.075 mm | 0 / 0 | 0 / 0 | 0 / 0 |
| `draft_clamp` bites | none (no painted relief) | none | none |
| `parting_monotone`, parting-line stamps | 16 of 16 | 16 of 16 | 16 of 16 |
| `dfm::findings_in` | 0 | 0 | 0 |
| Stones reported / previewed; metal inside stones | 9 / 9; 0 | 9 / 9; 0 | 9 / 9; 0 |

Notes on the table:

- **Ray-release status.** It reads "Review", not "Clear". The cause is the stock's own sub-3° table and bore walls (about 539 mm²); the bare stock reports the same.
- **Sand-slot notes.** The release also lists seven sand-slot notes, 0.10 to 0.60 mm wide, along the shoulder garland. These are cautions for the founder, not obstructions. They are recorded in `report.json`.
- **Closest stones.** The closest pair is 0.19 mm apart at the girdle (Cheek berry 2,1 to 2,3). All nine stones weigh 0.245 ct together.
- **Metal.** The ring weighs about 40.8 g of 18k gold.

Further gates:

- `--verify` passes: a cold reload with an empty library gives identical vertices, faces and normals.
- The casting pattern (`try_build_pattern`) is watertight with 0 degenerate faces and 0 self-crossings, at 1,344,334 triangles.
- `design.ring.json` is 708,842 bytes at format 6. Format 6 is needed because the stamps are tiered and use shaped tops and fine caps.

## Template gate (run after the last round)

The gate ran on the stock class with `--verify-export`:

- **1 `design.set` patch** (`/manufacturing`), within the limit of 4.
- The graph is **805,533 bytes** against the 1 MB stock budget, with 137 nodes. It carries the stock as a `base.preset` node; P7 removed the 3 MB mesh patch.
- Cold source is identical, and the cold graph reloads.
- Vertex, face and normal parity holds at 1,353,792 triangles.
- The first build takes 2.2 s.

The record is `showcase/vepres/ilex/verification.json`, and it is also merged into `report.json` as `template_gate`. Round 2's build gave 1 patch and 793,951 bytes.

## The CAD feature tree and the stack, as sentences

There are no CAD features; Ilex is stock, stamps and seats.

- **Base.** The base is factory 006 Square through the sand master, with the envelope on. It uses a Flat profile 17 mm wide, a 16 mm head, an 18.6 mm bore, a 0.3 mm edge round and a 0.1 mm comfort fit. The chart is set from this stock before anything is drawn on it.
- **Face leaves** (2 cast stamps, tier 0). These are two holly leaves end to end on the parting line, placed at `Hide::crest_at`. Each starts 1.25 mm from the head's centre.
  - The outline is ring-local `holly()`: a pointed blade with three sharp spines a side leaning to the tip, concave bays between them, a spined tip and a rounded stalk. Each margin is a function of x, so the monotone rule holds by construction.
  - Each leaf is the longest, in 0.1 mm steps, whose ends stay on the line on both sides: **6.8 × 5.5 mm**. Each leaf is turned by the least that keeps it on the line.
  - The top is a 0.55 mm dome over 0.45 mm eaves, with 4° draft and a fine cap.
  - Why: the leaf is the subject, struck square to the face camera on the only line where sand lets relief stand.
- **Face veins** (18 bench cuts, tier 1). Each leaf has a rounded midrib stroke 0.25 mm deep and four pairs of tapering laterals 0.12 mm deep, leaning 42° to the tip. They are cut after the pour and never enter the pattern.
- **Face berries** (3 garnet cabochons, 2.0 mm, flush gypsy seats).
  - One sits on the line at the sprig's heart and is cast with a raised 0.6 mm drill mark, which pulls on the line.
  - The other two sit 2.05 mm off the line and are wholly bench work (`bench_only`, no mound, no mark): any mound off the line has a flank facing its own mould half.
  - Why: a three-berry bunch where the stems meet is what made read test 2 say holly.
- **Table matte** (2 bench-only tiling layers). A procedural "Hammered" stipple 0.04 mm deep is cut into the table above and below the sprig, from 2.3 to 5.9 chart-v off the line, over 50° of the head. Why: to part the polished sprig from the field.
- **Cheek sprays** (each head wall that faces the pull).
  - Three leaf stamps a side are struck along the pull (`along_pull`): two of 6.0 × 3.2 mm either side of the bunch and one of 3.9 × 2.4 mm angled down at the lower corner. Each has a 0.4 mm eave and a 0.45 mm gable, plus a bench vein comb.
  - Three 1.8 mm garnets sit in a touching triangle in gypsy mounds 2.2 mm across and 0.45 mm proud, each with a raised mark.
  - The wall is a crescent over the bore, so the bunch rides its widest band.
- **Shoulder garland** (14 cast stamps on the parting line, plus 2 curve layers).
  - `stamp_row` places seven 4.2 × 2.4 mm leaves a side with `RowPath::PartingLine`, a 0.24 taper, `fold_clear_mm` 1.0 and mirrored shoulders. The row starts 1 mm past the last fold, where the line turns over the head's end walls, and runs 19 mm.
  - A station the line will not take as struck is moved along it, by 0.5° steps up to 2°, and levelled by turning it at most 4°.
  - Under the leaves, a 0.9 × 0.42 mm round `CurveLayer` stem runs on the line from the head's end wall down each shoulder.
  - The alternate ±20° splay the round-2 review asked for was tried on every station. The line refused it everywhere, so all 14 leaves lie along the line.
- **Palm.** It is bare, polished factory stock, as the plan has it.

## What I could not do

- **Face leaves at 7.5 mm.** The face is 16 mm long and the berry bunch takes its middle, so the leaves cannot reach 7.5 mm. The longest leaf both ends of the parting line accept is 6.8 mm; at 6.9 mm and above the tip leaves the line where the table turns down, and it locks. Raising the leaf over 0.7 mm made the stamp fail to join the band (degenerate CSG).
- **Stair-steps.** The stair-stepped margins come from the stamp cap's grid: `cap_pitch` is at least 0.1 mm even with `fine_cap`. A finer outline (0.015 mm sampling, over 512 points) broke the joins of the face and cheek leaves. The stamps have no top-edge round.
- **A seamless matte.** One uniform matte field with a halo round the sprig needs a mask. Struck under the leaves, the texture made the face stamps fail to join, and a lean design has no mask alpha to carry the halo. Two bands either side of the sprig were the workaround; the reviewer still read them as panels.
- **The garland splay.** No splayed garland leaf passes `parting_monotone` on this shoulder.
- **CSG fragility.** Several otherwise harmless placements failed to join ("two cuts cross inside a face"), so the cheek leaves were nudged until they joined at every build size.
- **Bark on the flanks.** The procedural Bark tiling aliased into combing and was removed. With it went most of the small-scale density that Caiman has.

## Core changes wanted (exact code)

1. **A finer stamp cap for spined outlines.** In `core/setting.rs`, `Stamp::cap_pitch`:
   ```rust
   fn cap_pitch(&self, reach: f64) -> f64 {
       if self.fine_cap { (reach / 56.0).clamp(0.04, 0.2) } else { (reach / 14.0).clamp(0.12, 0.35) }
   }
   ```
   `fine_cap` is already fenced at format 6, so no saved file changes. The point is to let a 6 mm holly leaf's cap reach its 0.03 mm outline instead of stepping at 0.11 mm.
2. **A cushioned top with eaves.** A dome falling to a margin height, not to the eaves. In `StampTop`, add:
   ```rust
   /// A dome from `margin_mm` over the eaves at the outline to `crown_mm` over the origin.
   Cushion { crown_mm: f64, margin_mm: f64 },
   ```
   Its lift would be `margin_mm + (crown_mm - margin_mm) * (1.0 - (d / reach).powi(2)).max(0.0)`, where `d` is the distance to the origin and `reach` is the outline's reach along that ray. It would be monotone from the origin on the parting line, as `Dome` already is.
3. **A stamp-shaped layer mask.** A `LayerEntry::mask_stamps: Option<f64>` (halo width in mm) that zeroes a layer within that distance of any tier-0 stamp's plan. That gives one matte field round a cast sprig without an embedded alpha. In `field.rs`, where an entry's mask is applied, multiply by `1.0 - smoothstep(halo, halo + 0.2, dist_to_stamp_plans(uv))` when the field is set.

The example works around all three: it relies on a fine cap and keeps the matte off the sprig's band.
