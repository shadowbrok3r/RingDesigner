# Vepres — Rosa mortua (`rosa-mortua`): cloud report

**Outcome: cut at 6.4 after the third reviewed round, with every gate green.** The pear-bud concept failed three read tests on 2026-10-03. Logan then approved the rethink I had proposed, a withered open bloom with the ruby as its heart. The bloom read at its first read test (test 4). Its three reviewed rounds scored 6.0, 6.2 and 6.4, below the 7.5 ship bar, so it is cut.

- Branch `claude/vepres-rosa-mortua`. Master was merged at `2e11632` and then `803a93e`.
- Example: `crates/ringdesign-core/examples/vepres_rosa_mortua.rs`. Outputs: `showcase/vepres/rosa-mortua/`.
- Block-out attempts: 3 for the bud and 1 for the bloom, against the 3 the lead granted. Reviewed rounds: 3 of 3.
- Process: lost wax (0.8 mm section, 0.15 mm detail, no draft). The sand pass was not attempted: the field notes a 0.48% two-part undercut, and the collets and stored parts undercut by design.
- `docs/collections/vepres.md` (Rosa mortua section) records Logan's process rule, the bloom decision, the lead's wall gate, "no extra rounds", and the result.

## Read tests and reviews
Every test and review was run by a fresh agent, given only `target/review.md`, the name and slug, the mode, and the number.

| # | Concept | Verdict | Gist |
|---|---|---|---|
| Read test 1 | Pear bud in a bloom, Sepal-claw hip | reads: false | A spiked sun band; the bloom read as a "flower-shaped bezel" |
| Read test 2 | Pear clasped by sepals, hooked prickles | reads: false | A sawblade crown; a "leaf-trimmed bezel" |
| Read test 3 | Pear stood up in pointed sepals | reads: false | "A gem in a star bezel"; "toi et moi bar ring" |
| Read test 4 | **Withered open bloom** | **reads: true** | "A jeweller would say 'rose' without being told" |
| Round 1 | Bloom with full detail | revise, **6.0** | Census walls recorded; petals extruded; the hip "a can with spider legs" |
| Round 2 | Hip urn, parametric stems, prickle rows along the crest, cupped leaflets, nodes | revise, **6.2** | Census still recorded as failed (automatic revise); the hip a "flying saucer"; prickles a comb |
| Round 3 | Census clear, hip lip without a ledge, browner garnet, reflexed outer petals | **cut, 6.4** | Every gate green; the hip, prickles, leaf and petal walls still read as CAD |

The reviewers' open items at the cut:
- The petals are extruded sheets with vertical walls, and a collet rim still shows round the ruby.
- The hip is a turned, stepped form, and its sepals are round tubes.
- The leaflet teeth are stair-stepped.
- The prickles are blunt pegs near the band's midline.
- The bypass crossing is hidden under the bloom, and a seam step remains near 6 o'clock.

## Gates on the final build (export 1536 × 448; draft 768 × 320 in `draft_build`)
All twelve pass at both resolutions.
- **Mesh:** watertight, 0 degenerate faces, 0 self-crossings. All 31 made parts are closed with 0 crossings. Notes are empty and all 32 features are Ok.
- **Bore:** 0 vertices inside the finger hole (nearest 9.2999 mm, bore radius 9.3).
- **Lost-wax verdict:** Castable, thinnest field wall 2.01 mm.
- **Wall census** (the lead's 2026-10-04 gate): `cad::measure::thickness(&built.mesh, 0.8)` finds 0 real walls. It lists 23 specks under 0.02 mm² and 26 suspected census artifacts under 0.05 mm, each with its point, area and part. The two largest artifacts are mirrored zones on the band at the arm crossing, (±2.34, 9.10, ±2.36): 7.58 mm² each at 0.040 mm. They are the bypass seam channel. The reviewer flags that crevice for a check at the bench.
- **DFM:** 0 findings.
- **Stones:** 2 reported and 2 in the preview, no metal inside either, crowding clean.
- **384 × 192 build:** same results.
- **Casting pattern:** closed.
- **Size:** 1,315,744 triangles, within the 2 M budget.
- **Cold reload:** identical. 18.4 g in 18k.

**Template gate** (`collection_templates --verify-export`, procedural class):
- 1 `design.set` patch (`/manufacturing`).
- 258,863 B against the 300,000 B budget (the section's own target of under 200 KB is missed).
- Source identical; cold design and cold graph reload pass; mesh parity identical; export geometry verified; first build 2.0 s.
- `crisp_relief` is off, because the ring has no height-field relief.

## Enablers used
- **C-B2 true pear plan:** used in the bud attempts.
- **C-V1 `Placement::Relative`:** the receptacle, petal rings, dried sepals, the hip urn and the hip's crown all follow their stones.
- **C-V2 `PatternKind::Along { Crest }`:** the two graded, alternating prickle rows.
- **C-V3 points-path twisted sweeps under a scale law:** both stems, the stem nodes and the prickles' flared foot.
- **#248 framed close-ups:** `bloom-close`, `hip-close` and `leaf-close`.
- **P6 Sepal claws:** used in the bud attempts only.

## CAD feature tree, as sentences
1. The stem is a procedural bypass band.
2. A 5.5 mm round ruby sits at θ 97° on arm A, in a 0.9 mm collet.
3. A 6 mm garnet cabochon sits at θ 38° on arm B, in a 0.85 mm collet.
4. Each arm's stem is a twisted sweep over the band. It rises out of the bark, stands 0.75 mm proud over the top and runs under its stone.
5. A turned receptacle sits under the bloom.
6. Four rings of petals (4, 5, 6 and 7) each pattern one stored petal about the ruby. The inner ring wraps the collet. The outer ring hangs reflexed below the bloom's plane. Each petal is 1.0 mm through its blade and 0.9 mm at its margin, with broad dried folds.
7. Five reflexed dried sepals under the bloom pattern one stored sepal about the ruby.
8. The hip's crown is one lens-section twisted wisp, patterned five times about the garnet.
9. The hip urn is a turned fruit that swallows the collet up to its lip.
10. Three stem nodes swell round the palm.
11. The leaf has three cupped, toothed leaflets with sunken midribs on a tapered rachis, and lies under the bloom.
12. Two prickle rows: one twisted-sweep prickle per arm, arrayed along the crest four times, graded 1.0 to 0.8 and alternately spun.

## What I could not do
- I did not reach 7.5. The reviewers wanted lofted petals with rounded rims and no vertical walls, a single swept urn whose own lip is the setting, and fine-pointed prickles. With 0.8 mm floors on every made blade, each of those rebuilds pushed the census back into real walls. Clearing the census took round 3's effort.
- I did not open air between the two arms. Core's bypass is one widened section, not two arms.
- I did not fair core's arm-tip seam near 6 o'clock.
- The garnet's faceted bands come from core's cabochon preview tessellation.
- I did not set the template's exposed controls (stem width and thickness, the stone sizes).
- **Deviation from the spec:** the stem is HighDome 3.6 × **3.0** at bypass amount **0.7**. At the spec's 2.7 mm section and full bypass, the crossing leaves a 0.29 mm field wall and a 0.15 mm census wall at the bore edge.

## Core changes wanted
1. Expose the bypass arms:
   ```rust
   // core/profile.rs
   -fn bypass_arm(off: f64, k: f64) -> Option<(f64, f64)> {
   +pub fn bypass_arm(off: f64, k: f64) -> Option<(f64, f64)> {
   ```
2. `head.bezel` needs a `floor_mm` parameter, so the collet stops at a given depth instead of running down to the bare band. The hip's urn has to swallow a collet wall that runs down to the stem. I have not written this change.
3. A smooth cabochon preview, with more tessellation rings in `gems::built_meshes` for cabochons, to remove the banded dome.
