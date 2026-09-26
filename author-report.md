## Basiliscus round 3: final report

All nine punch-list items are addressed and every gate passes at all three resolutions. One visual flaw is left (item 8, under "What could not be done"). I score it about 7.4 against Caiman's 7.

- **Branch:** `codex/bestiarium-basiliscus`
- **HEAD:** `4df7a53`, on top of `46fac53` (round-3 milestone) and `40ca22d` (round 2).
- **Worktree:** `/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner/.claude/worktrees/bestiarium-basiliscus`
- **Scope:** only `crates/ringdesign-core/examples/bestiarium_basiliscus.rs` and `showcase/bestiarium/basiliscus/` changed. No `src/` edits, master not merged (optional), nothing pushed or tagged. The tree is clean.
- **Renders:** `showcase/bestiarium/basiliscus/{hero,face,stones,side,shoulder,reverse,palm,bare-vs-finished,face-300,hero-300}.png`. The draft set is in `target/basiliscus/final-draft/`, and the acceptance crops I checked are in `target/basiliscus/final-crops/`.

### Gates
| | Draft 768×320 | Export 1536×448 | Coarse 384×192 |
|---|---|---|---|
| Triangles | 504,588 | 1,799,184 (under 2M) | 174,736 |
| Watertight | yes | yes | yes |
| Self-crossings / degenerates | 0 / 0 | 0 / 0 | 0 / 0 |
| Stamps struck | 14/14 | 14/14 | 14/14 |

- **Both resolutions:**
  - Bore clear (0 vertices inside).
  - Every made part (15) closed with 0 crossings.
  - Investment pattern watertight with 0 crossings.
  - Lost-wax field verdict: Castable, thinnest wall 1.670 mm.
  - 0 DFM findings.
  - One stone, no warnings, 0 tight pairs.
  - Cold reload geometry identical.
- **Template gate:** 1,550,359 bytes (limit 3,000,000), 0 patches, 0 detail findings, 74 nodes, 1,799,184 identical triangles, export geometry verified. US size is still deliberately not exposed; the note is kept in `template-gate.json`.

### Per item
1. **Scales on the body:** done.
   - Round 1's `reptile::snake` pattern is back at 0.24 mm relief, full over the inner 70% of the half-girth and faded over 0.7–0.9.
   - Scales are sized to the body's width (0.36 of it, 0.25–0.55 mm), so every station carries the same rows and the half-faded "dashes" on the outer flank are gone.
   - They run from the head down the neck to the tail.
   - Steepest flank is 55.5° (limit 57°), the tail land is 0.800 mm and the round tail cap is kept.
   - Belly scutes are off the face (they stay on the palm), so the part lapping the boss carries the same scales.
2. **Wall skins:** done.
   - No normal-threshold gates remain; the Serpent flanks layer and the cracked-mud cells are gone.
   - The walls under the chief and point are polished, with one line of 46 beads 0.5 mm under the table edge.
   - Hackles are gated by true-mm distance and drawn whole or not at all.
3. **Circlet:** done.
   - Arched band along the skull crest, sagitta 0.68 mm, domed rise 0.45 mm, 6.03 mm across, land 0.86 mm, rounded ends.
   - Three points 1.2–1.3 mm wide at the band with ridge tops; pearls (r 0.41) sit on 0.44 mm necks, 0.71–0.96 mm inside the chief.
   - Two domed 0.8×0.5 oval jewels.
   - The crown reads clearly in stones.png and face.png. In hero-300 it is small: three bumps on an arc.
4. **Tongue:** starts inside the gape and crosses the lower jaw's front lip scales before forking. Ridged tops kept; stem land 0.397 mm and tines 0.297 mm, both named exceptions.
5. **Head at 3/4:**
   - Each jaw is crowned, with bevelled gape edges.
   - Domed lip scales set back from the gape as a scalloped band, not square teeth.
   - Both fangs hang from the upper jaw pointing down and back.
   - At least three closed head plates between the eye and the crown, plus a slit pupil and a nostril cut in.
6. **Hackles as feathers:**
   - Feathers are 4.6:1 with drawn-out tips, barbs at about 35° toward the tip, one notch per vane side and frayed edges.
   - The bare band on the shoulder came from rows being dropped; row spacing is now capped so three rows fit all the way to the morph.
   - In the reverse box, a channel of about 0.7 mm is left between two rows.
   - Frayed feather walls look streaky at grazing angles.
7. **Bordure:** 73 beads run continuously round the whole shield edge. The body stays at least 1.0 mm from the edge, so no gaps are needed.
8. **Plain boss:** broken up by the coil lapping 0.52 mm onto it; the 0.849 mm wall is kept. I used the lap rather than milgrain.
9. **Gates:** all green at every resolution. Land rows were added for the crown points and pearls, circlet, jewels, rim beads, fangs, tongue and jaws; every row is at or above 0.8 mm or named.

### What could not be done
- **Crease under the stone:** where the coil laps the 0.9 mm boss it drapes over the wall and leaves a dark crease under the stone.
  - I simulated tilting the body over the boss instead; it pushes the outer flank to 63–70° wherever the body is 1.0 mm half-width or less.
  - The thin tail cannot drape at all.
- **Not touched:** the palm scutes still read as bars (not on the round-3 list).

### Core changes wanted
- **Stamp reach over tall relief** (`setting.rs:1703`): already fixed on master in `22912a5`, not merged here.
- **Finer stamp cap grid:** so a stamp's top follows the painted relief under it. At `setting.rs:1707`, change `(reach / 14.0).clamp(0.12, 0.35)` to `let pitch = (reach / 28.0).clamp(0.1, 0.2);`.
- **`dfm::worst_arc_ratio`:** should also skip stations where the layer's own painted image is blank, not only where the mask is. Otherwise empty columns set the tightest station.

### Self-score
About 7.4, against Caiman's 7.
- **Gains:** the scaled body, the head, the circlet, clean walls and a continuous bordure.
- **Could keep it under 7.5:** the crease under the stone, streaky feather walls at grazing angles and the small hero-300 crown.