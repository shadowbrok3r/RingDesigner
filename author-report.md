Round 3 is finished and committed, and every gate passes at both resolutions. My honest score is 7.0, on the line for the 7.5 cut. Three things fall short of the letter of the review, all listed under "What I could not do": no 0.3 mm root fillet, the outer claw roots sit at −28°/+30° rather than ±34°, and coil A reads as a hook from the hero camera.

**Branch** `bestiarium-kraken`, HEAD `567365a`, worktree `.claude/worktrees/bestiarium-kraken`. Master (`22912a5` C-B6 / `0087325`) is merged in as `62b2dfe` with no conflicts. My C-B1 fences in `library.rs` survived the merge.

**Renders** are in `showcase/bestiarium/kraken/`: `hero`, `face`, `side`, `palm`, `shoulder`, `reverse`, `stones` and `bare-vs-finished` at 1600 px, plus `contact-300.png` (hero, face, side / palm, shoulder, reverse at 300 px).

## Gates (draft 768×320 / export 1536×448)
- **Mesh:** 941,948 / 1,770,028 triangles. Watertight, 0 boundary, 0 non-manifold, 0 degenerate. 0 self-crossings on the ring and on all 56 parts.
- **Notes and finger hole:** parts and solids notes are empty. 0 vertices in the finger hole.
- **Field:** lost wax, Castable. Wall 1.999 mm at 270°. Two-part undercut 16.9% / 16.7% (reported, not gating).
- **DFM and stones:** 0 findings. 1 stone reported and 1 previewed (4.15 ct). Metal inside the stone 0. Stone warnings are now empty: your holder fix counts the claws, which carry role Head, attach Join and the stone's placement.
- **Land widths:** passed. Thinnest section 0.900 mm. Balls 0.936–1.231 mm. 19 sucker details plus 9 reflected; finest dimple 0.218 mm against the 0.15 mm investment floor. Bed 8.7 mm.
- **Other:** 6 crossings. Tips stand 2.59/2.37 mm proud at draft and 2.12/2.44 mm at export. Cold reload identical at both. Investment pattern: 1,770,028 triangles, watertight, 0 degenerate, 0 crossings, scale 1.0194.
- **Weight and reproducibility:** 23.49 g in 18k, 15.62 g in silver. A fresh draft reproduces `design.ring.json` byte for byte.
- **Control intervals (`--controls`):** US size 7.5 and 9.5, and Tentacle height 1.2 and 1.6, each watertight, clean and Castable with DFM 0.
- **Test suites** (run after the merge, before the final example-only edits): core 795 passed plus 1 golden, 16 ignored. Graph 119 passed, 0 failed.

## Template gate
- **Plain lift (`exposed_controls []`, procedural):** 205 nodes, 0 `design.set` patches, 241,209 bytes against the 300,000 budget, passed. Export geometry identical at 1536×448 (1,770,028 triangles).
- **With the allowlist:** 208 nodes, 0 patches, 242,817 bytes, controls "US size" and "Tentacle height", export geometry identical.

Manifest entry for you:
```json
{"slug":"kraken","authored_graph":"kraken.graph.json","exposed_controls":[{"name":"US size","min":7.5,"max":9.5},{"name":"Tentacle height","min":1.2,"max":1.6}],"template_class":"procedural"}
```
- `kraken.graph.json` is committed beside the design. It is the lift with one `number` node feeding all six arm wires' `height_mm`.
- If the design is rebuilt, regenerate it: lift with `["US size"]`, then run `bestiarium_kraken --author-graph <lifted template.graph.json>`.
- **Withheld controls:**
  - Band width, band thickness and flare: they move the crown under the claws' buried roots and the arms' feet, and the bed and coil anchors with them.
  - Stone size: each claw's plane is solved from the 10 mm dome.

## Per item
1. **Claws**
   - Each claw is a sweep in the plane through its buried start, its flank point and its dome contact, so it wraps round the stone.
   - Rising hand: rises at −28°/0°/30°, touches at 12°/46°/77°. The falling hand is its exact reflection, built as one Mirror pattern part, which halves the part count for the budget.
   - Skin between feet: 1.23 and 1.58 mm.
   - Heights over the girdle: 3.06, 5.38 (curl) and 3.64 mm. Reaches 1.0, 1.8 and 1.5 mm.
   - The middle claw curls 190° back (bends 0.85 and 0.68 mm, tightest bend 1.38× the local radius).
   - Every claw carries 3 dimpled suckers (0.7, 0.6, 0.5 mm) rolled toward the visible flank.
   - Gaps to the stone 0.110–0.115 mm; claws 0.55 mm apart.
2. **Bed:** a flat-topped disc 8.7 mm across under the stone's back, filleted 0.55 mm into the crown. The side.png scan of x 620–980 finds 0 columns of daylight, and the bed is hidden in face.png.
3. **Roots**
   - Every claw and coil starts 1.05 mm under the bare band; the start faces sit 0.54–0.61 mm under it.
   - Each arm's narrowed end (1.6 mm) lies inside its claw's foot. The ring and flat cap are gone.
4. **Stagger:** 0 on both papilla layers. Offsets of 0.37/0.61, plus alternate-row mirroring on the fine layer, break up the rows. The palm crops show no truncated lines; DFM 0.
5. **Coils**
   - Coil A (toward the stone): 440° of turning, radius 2.0→0.95 mm, second sweep leaned 38°, lift 1.36 mm, 15.3 mm long.
   - Coil B (along the band): 360°, radius 1.75→0.9 mm, lift 1.13 mm, 10.7 mm long, so A is 43% longer.
   - Each has 5 dimpled suckers (0.6→0.45 mm) on its inner face. Turns stay 0.70/0.78 mm apart.
6. **Ground:** mantle papillae down to 0.22 mm and fine to 0.10 mm. Arms kept at 1.44 mm; they read as raised braids at 300 px.
7. **Allowlist:** verified as above.

## What I could not do
- **Root fillet:** blend 0.3 mm on claws and coils left pinch or fold notes on every claw but one and on both coils, where the seams cross the arms' creases. Notes fail the gate, so the claws and coils join unfilleted. The balls have no seam with the band, so a blend there only raises a "no seam" note.
- **Bed skirt:** the requested skirt of at least 1 mm left a thin shelf against the claw bases, so the fillet is 0.55 mm.
- **Sucker lips:** on the smallest suckers (0.45–0.5 mm) the lip round the dimple is 0.12–0.13 mm, under the 0.15 mm floor, so those rims will cast soft.
- **Grip:** all six hooks close over the +x half of the dome. The −x half is held only by the flank contacts at the ring ends and by the bed.
- **Coil A:** it needs a 3.5 mm neck to stay 2 mm proud of the braid at export, and from the hero camera it reads as a hook.
- **Claw bearings:** the band edge limits the outer roots to −28°/+30° rather than ±34°.
- **Core changes:** none this round. The only core source changes on the branch come from the master merge.