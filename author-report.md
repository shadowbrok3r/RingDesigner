# Corvus — Huginn and Muninn, round 3 (final): lane report

## Where it is
- Clone `/home/user/rd`, branch `bestiarium-corvus` (merged `origin/master` cleanly, no conflicts).
- Commits on top of the merge:
  - `f6b0e56` Core request 28: faired bypass arm union (C-B7)
  - `98037ea` Corvus round 3: example rework
  - `bedcf7c` Corvus round 3 outputs
  - plus the commit carrying this report
- **Push failed.** Every `git push origin HEAD:claude/bestiarium-corvus-r3` got HTTP 403 from the session's git proxy: `shadowbrok3r/RingDesigner is not in this session's authorized repository set`. The commits exist only in this container. A git bundle and patches are in the brief repo (`/home/user/repo/corvus-r3.bundle`, `/home/user/repo/corvus-r3-patches/`). To land them, add the repo to the session's sources and push again, or fetch the bundle.

## Renders (studio gold, stone set)
In `showcase/bestiarium/corvus/`:
- `hero.png`, `face.png`, `palm.png`, `side.png`, `shoulder.png`, `reverse.png`
- `stones.png`, `head.png`, `bare-vs-finished.png`
- `hero-300.png`, `face-300.png`

Also written there:
- `report.json`, `design.ring.json` (format 6), `stones.json`
- `template-verification.json`
- `finished-metal.stl`, `casting-pattern.stl`, `reference-onyx.stl` (git-ignored)

## Core change C-B7 (request 28): done
- **Change:** `ShankStyle::bypass_fair_deg`, serde default 0, written only when non-zero. `profile::bypass_span_faired(off, k, fair)` averages `bypass_span` over ±fair with weights `(1−t²)²`, N = 12 each side. It is called in `modulation()`'s Bypass arm. At 0 it returns `bypass_span` bit for bit.
- **Fence:** `library::format_version_for` returns 6 when the value is non-zero. A graph `shank` node with a `bypass_fair_deg` pin is fenced too (`template_features_in_json`). `read_design` became `pub(crate)` for the test.
- **Exposed in:**
  - the graph `shank` node, as a field pin. The new `StructNode::sparse` lets the coverage check accept a field serde skips at its default.
  - the MCP `set_shank` tool (`bypass_fair_deg`, 0–12).
  - the GUI shank panel ("Tip fair", shown for Bypass).
- **Test:** `a_faired_bypass_ramps_its_arm_tips_and_zero_keeps_the_hard_union` checks that:
  - fair 0 equals `bypass_span` bit for bit over ±180°;
  - an old bypass file (no field) reads as 0 and rebuilds identically, and so does the curated toi et moi;
  - 0 is not written and stays at format 5;
  - at 4° the steepest edge change falls below 0.6× the hard union's;
  - the field stays clean (< 0.05% undercut) and the mesh is watertight with 0 degenerates;
  - format 6 is written and a format-5 reader refuses it.
- **Test runs:**

| Suite | Result |
|---|---|
| `cargo test -p ringdesign-core -- --test-threads=4` | 789 lib + golden, all pass |
| `cargo test -p ringdesign-graph` | 101 + integration, all pass |
| `cargo check -p ringdesign-mcp` | clean |
| `cargo +1.95 check -p ringdesign-gui` | clean (egui 0.36 needs rustc 1.95; installed via rustup) |

- **On Corvus:** `bypass_fair_deg = 4.0`. The example's own along-ring fairing of the heads' base is deleted. With the band faired, it was what pushed the culmens off the parting line: removing it cleared a 384 release hit.

## Gates (all green)
Delft clay: 3.0°, 0.8 mm section, 0.30 mm detail.

| Gate | 1536 × 448 export | 768 × 320 draft (`draft_768`) | 384 × 192 (`stamps_384`) |
|---|---|---|---|
| Triangles | 1,342,042 (pattern 1,357,230) | 523,536 | — |
| Watertight, boundary / non-manifold / degenerate | yes, 0 / 0 / 0 | yes, 0 / 0 / 0 | yes, 0 degenerate |
| Self-crossings, ring | 0 | 0 | 0 |
| Made parts (80 stamps + collet), crossings / open / degenerate | 81 parts, 0 / 0 / 0 | 81 parts, clean, 0 | — |
| Solids notes, stamps resolved | [], 80/80, 0 parting-monotone failures | [], 80/80 | [], 80/80 |
| Release 0.100 mm | 0 / 0 (parting −0.0003) | 0 / 0 | 0 / 0 |
| Release 0.075 mm | 0 / 0 | 0 / 0 | 0 / 0 |

Export-only gates:
- **Bore clear:** margin −4.5e−7 mm, 0 vertices inside.
- **Field:** Castable at 0.0181% (0.24 of 1330 mm²), worst −0.96°, thinnest wall 2.28 mm at θ 52°. Advisory notes: 0.1 mm² leaning 1° at θ 40° and at θ 108°.
- **Clamp:** collet bed 0.0017 mm, plumage 0.0114 mm, heads 0.0000 mm; 0 texels cut on every layer.
- **DFM:** 0 findings.
- **Stones:** 1 reported / 1 previewed, 1.194 ct, 0 tight pairs, no warnings.
- **Cold reload** (empty library): vertices, faces and normals identical.
- **Pattern:** 0 open edges, 0 degenerates, 0 crossings. Scale 1.0194. Bench layers: graver's work, 4 eyes, collet.
- **Triangle budget:** under 2 M.

## Template gate
- `exposed_controls ["US size"]` (the lift's name for size), class `painted`.
- 0 `design.set` patches, 142 nodes, 1,826,523 bytes of the 3,000,000 budget.
- Source identical. Cold design, graph and editable-graph reloads pass. Export geometry verified at 1536 × 448 with identical vertices, faces and normals.
- `template_gate_passed` true.

## Punch list, item by item
1. **Skull sculpt: done.**
   - Crown 2.30 mm above the bare crest.
   - One top line: a rounded nape (1.3 mm), a level crown to 2.6 mm, then a single convex fall over the forehead into the culmen to the tip. There is no step or dip at the bill root, and the forehead ramp runs about 2.4 mm to the root.
   - Rear skull (u < 3.0 mm, 55–60%): scalloped contour feathers in arcs round a forehead point. Rows are 1.0 mm apart, lanes fan at 26°, tips are rounded and point back and out, the step at each tip is 0.18 mm, and the drop between lanes is 0.10 mm. That gives about 3 rows × 5 lanes, roughly 15 scallops per head.
   - Forehead and bill stay glossy.
   - Brow: the orbit is sunk under the crown's flank (about 0.3–0.5 mm step on its crest side), and a 0.1 mm brow line runs 2 mm along the ring over it.
   - All of it is parting-legal by construction (it only steps down away from the crest); head clamp 0.0000.
2. **Eye: changed in approach.** The comma came from the bench cut being projected along the bare band's normal onto a steep skull flank.
   - Now each eye sits on a 1.4 × 0.9 almond orbit whose floor is level across the ring. It is sunk under the brow on its inner side and flush with the cheek on its outer side (no raised lip, which would be an undercut).
   - Inside it, a bench-cut almond 1.0 × 0.6 mm, 0.35 mm deep, with its long axis along the ring and the pointed outer corner toward the bill.
   - It stays bench-cut; the render shows it as finished.
   - It reads as a socket and pit in `shoulder.png` and `hero.png`. In `head.png` it is legible but busy; see the self-review.
3. **Bills in the face view: done.**
   - Heads moved about 6° toward the collet. Huginn's nape is at θ 108.3°, bill root 129.7°, tip 159.4° plus a 0.8 mm hook; Muninn mirrors him.
   - Bill root 2.9 mm across, kept wide further out (half-width law t^2.6, minimum 0.45 mm).
   - Tip raised to 1.55 mm; culmen flat ±0.4 mm as the highlight line.
   - `face.png` shows two hooked wedges pointing to 3 and 9 o'clock. At 300 px they read, but faintly.
4. **Fold: done at the source.** Core `bypass_fair_deg = 4`. `shoulder.png` and `side.png` show no crease on either skull's tip side. Hackles on both faces: 28 of 28 (7 per face per bird), struck with facing 0.4 because the faired tip face leans. Stamps 72 → 80.
5. **Collet surround: done, with a caveat.**
   - The rachis dashes are gone. Neck contour feathers are brick-laid half a row apart, with rounded tips, 1.25 mm pitch at the neck, 0.22 mm steps, 0.28 mm lane drops, and barbs like the backs.
   - The necks climb to the collet's drafted base plus the gypsy stock (0.9 mm over the base). No oval plaque rim: the bed stays under the collet and fades over 0.6 mm.
   - Caveat: the collet overhangs the band's rim on its two long sides, so `collet_wall_mm` reads 1.9–2.0 mm in those four sectors, about 1.1 mm on the diagonals and ≤0.56 at the ends. A feathered flange there overhung (−35° undercut in a trial). Sinking the stone 0.35 mm cut it to 1.6 but brought back two nape-dip slots, so I kept the round-2 seat.
6. **Tails: done.**
   - Five rectrices per bird. Each runs in under the one inside it, so only its outer vane and a rounded tip show (the quarter round of radius equal to the visible vane; the central feather's tip is a semicircle).
   - Visible width grows from about 0.9 mm at the root to about 1.3 mm at the tip. Edges splay 1.5°, 5.5° and 8.5°.
   - Steps are 0.30 mm, faired over 0.15 mm. The central tips leave a 0.30 mm gap at θ 270°.
7. **Report: done.**
   - `report.json` carries `draft_768`: watertight, boundary, non-manifold, degenerate, self-crossings, 81 made parts clean, notes, stamped 80/80, both releases with θ/r/z/depth.
   - `template-verification.json` lists `exposed_controls: ["US size"]`.
8. **Sand slots: mostly done.**
   - 11 slots under 0.30 mm at 0.100 mm (27 findings in all), and 12 at 0.075 mm.
   - No nape-dip entries.
   - One beak-zone entry remains: θ 19.8°, a single cell (0.10 mm) at 0.100 and 0.225 mm at 0.075, at one bird's hook. It is named ("file the flash off the beak and recut the hook").
   - The rest are flank feather tips, each named with its bench work. Several are exactly 3 cells (0.2997), at the floor to within sampling.

## Layers, stamps, parts, stone
**Layers (all Max blend):**
- `Collet bed` (3.0 mm) lifts the band to the collet's drafted base under the collet only.
- `Raven plumage` (3.4 mm): graded contour feathers (1.25 → 2.4 mm pitch), brick-laid, rounded tips, necks climbing the collet, tails as above.
- `Raven heads` (3.5 mm): skulls, orbits and bills.
- `Graver's work` (0.1 mm, subtract, bench only): barbs in every contour feather and rectrix, rachises on long feathers, nasal bristles and gape lines.

**Stamps (80):**
- 56 struck along the pull on the side faces:
  - 28 wing primaries (quill 8.5 × 1.0 mm, 0.25–0.45 mm proud)
  - 28 throat hackles (lanceolate 2.0 × 0.6 mm)
- 20 neck feathers (lanceolate 1.4 × 0.5 mm)
- 4 bench-cut almond eyes.

**Part:** the onyx's made bezel collet, soldered after the pour. The pattern carries the gypsy stock and a raised drill dot.

**Stone:** 8 × 6 black onyx cabochon, 1.19 ct: Odin's eye between his ravens.

## Could not do
- Push, as above.
- A face-300 that says "ravens" unmistakably. The bills now read, but the skulls from above still look segmented at thumbnail size.
- Collet side walls under 1 mm, for the overhang reason above.
- 384 × 192 release is sensitive. Single-sample crest-row hits come and go with any global change, because the 0.075 pass picks its own plane. I held the gate with crown 2.30 and bill levelling 0.2, both inside the reviewer's ranges. A convergence table was not run.

## Core changes wanted
None beyond C-B7, which is implemented.

Worth a request later: `castability`/`mf::release` could report crest-row single-sample hits whose depth equals the row's offset from the plane separately. They move with resolution and make stamps_384 a lottery.

## Self-score against Caiman = 7
**6.9.**
- **Up from round 2 (6.2):**
  - the hero and shoulder now carry an unmistakable hooked-bill raven head with an eye socket and a feathered skull;
  - the bills read in the face view;
  - the fold is gone at its source;
  - 28 hackles;
  - rounded, fanned tails;
  - no dashed plaque.
- **Held back by:**
  - skulls that read as segmented from directly above at 300 px;
  - an eye that is a socket-and-pit rather than a crisp almond in `head.png`;
  - plain collet wall on the stone's long sides.
