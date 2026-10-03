# Vepres ring: Hedera (`hedera`) — cloud report

The work is on branch `claude/vepres-hedera`, merged with master up to `29babc4`. The ring is `crates/ringdesign-core/examples/vepres_hedera.rs`, and its outputs are in `showcase/vepres/hedera/`.

## Verdict: cut at 6.0 after round 3

The rethink Logan approved on 2026-10-03 passed its block-out on read test 5. All three reviewed rounds ran, and the ring ended below the 7.5 ship bar, so it is a **cut**.

- Every gate is green at draft and at export.
- The template gate passes at 299,166 B against a 300,000 B budget.
- The reviewers' reasons were art, not gates. The host is sparsely covered, the small leaves are unclear, the rootlets look thorny and the collets look like tubes.

**Rounds used: 3 of 3.** No extension was granted, and the collection doc records none.

### Read tests (block-out)

| # | Concept | Reads | What the reviewer saw at 300 px |
|---|---|---|---|
| 1 | original | **false** | A blackberry or mulberry on a vine. The leaves read as holly or thistle, and the berry dome as one compound fruit (this collides with Rubus). |
| 2 | original | **false** | The crown leaves read as puffy five-pointed stars (maple or sweetgum), and the collets as gold tubes. The stem did not register. |
| 3 | original | **false** | Stars or starfish, with black pips in thick gold cups. The rootlets read as a tick ladder. **I stopped here and asked for a rethink.** |
| 4 | rethink | **false** | The leaf stood up like a fin or a paper dart and was too small. The berries read as cups. The stem was a free arc. |
| 5 | rethink | **true** | "A jeweller's first word is 'ivy'." The flat lobed leaf, the clinging stem with rootlets and the black berries carry it; grapevine was the second guess. |

Each test used a fresh reviewer, given only `target/review.md`, the name and slug, read-test mode, the attempt number and the paths. Test 6 was not needed.

### Reviewed rounds

| Round | Verdict | Score | Main points |
|---|---|---|---|
| 1 | revise | 5.6 | The template gate was red (1.34 MB) and `verification.json` was stale. The crown leaf names the ring as ivy. Bare host dominates, the small leaves hang off as tabs, the rootlets read as a sawtooth and the collets as tubes. |
| 2 | revise | 6.0 | The template is fixed (297,754 B). The crown leaf's outline is crisp, and five berries read as a bunch. Coverage, the slab leaves, the bristly rootlets and the tube collets are still open. |
| 3 | **cut** | **6.0** | All gates are green and the template is 299,166 B. The reviewer judged that coverage, the small leaves and the rootlets had not moved. The cabochon reflections are stair-stepped, because the reference spinel mesh is core and coarse. |

**Round 3's changes, for the record.** The reviewer reported most of them as unchanged:

- The berry bunch was turned and pulled onto the crown.
- Every berry has a stalk, and the collets are shallower (berry height 0.6 → 0.3 mm).
- The juvenile leaves now splay off the stem at 40° into open band. They are sized to stay 2.3 mm from the crest, hug the band, and carry only a midrib.
- The rootlets were shortened to 0.4–0.55 mm, pressed down, and set at a 1.3 mm pitch.
- The crown leaf's side ribs now run to the tips, its lobes cup up 0.6 mm, and its margin round grew to 0.3 mm.
- A new `stem-close.png` shows the stem.

I did not add a second stem or more leaves to cover the host: the template had no budget left (see below).

## The design as shipped to review (round 3)

- **Host:** procedural `ShankKind::Uniform`, D-shape 6.0 × 2.4, edge round 0.55 mm, comfort fit 0.15, bore 18.6 mm.
- **Process:** lost wax, under Logan's 2026-10-03 rule; the decision is recorded in Hedera's section of `docs/collections/vepres.md`.
  - Body parts hold the 0.8 mm section.
  - The collets and the rootlet tuft are held at the 0.15 mm detail floor, as stated in the gate's own wording (the Rubus precedent).
  - It was not tried in sand: the leaves' sunk floors and the stalked berries undercut, so there is no sand bonus.
- **Weight and stones:** 18k gold, 14.59 g. Five black spinel round cabochons totalling 0.42 ct.
  - Three are 3.0 mm and two are 2.2 mm. The two 2.2 mm stones fall below the rethink's 3–4 mm; I kept them because round 1 asked for a fuller bunch than three stones.
- **Layers:** none. **Stamps:** none.

### CAD feature tree (40 features)

- **#1 Host band:** the procedural band.
- **#2 Ivy stem:** a stored tube, Ø 1.7 and 42% sunk.
  - It runs 252° from a cut end at 122° on the shoulder, round the palm, to a growing tip just past the crown at 14°.
  - It weaves ±1.7 mm across the band on 1.6 waves.
  - It tapers to Ø 0.88 over its last fifth.
  - Its bark is eight shallow wavering striae with node rings every 16 stations.
- **#3 Crown leaf:** a stored solid, 10.2 mm from the notch to the tip, with its hub at 110° and lying across the whole crown.
  - **Outline:** a polar outline about the hub, with a long terminal lobe, two short laterals at half its length, rounded basal ears and a heart notch. The edges are bowed and the tips rounded about 0.5 mm.
  - **Top:** a low dome 0.45–0.52 mm over the band, with a raised midrib and side ribs running to every lobe tip and a fine web recess between them.
  - **Shape:** the lobes cup up to 0.6 mm, and the margin has a 0.3 mm top-edge round.
  - **Floor:** sunk 0.8 mm into the band.
  - **Over the edges:** past the crown's lip, the blade leaves the band on a gently drooping surface, so it breaks the band's outline.
  - **#4** is its petiole: a Ø 0.88 tube running straight from the stem into the notch.
- **#5 to #22: nine juvenile leaves and their petioles.**
  - The leaves use the same outline at 3.35–4.75 mm and carry only a midrib.
  - Each leaves the stem at 40° to its line on a 1.5 mm petiole, into whichever side has room.
  - Each is sized so every lobe stays within 2.3 mm of the crest, and hugs the band up to a 30° normal tilt.
- **#23 Berry peduncle:** a tube from the stem's growing tip to the bunch's centre at 37°.
- **#24 to #28: five berry stalks**, Ø 0.84, from the centre toward each berry's foot.
- **#29 Rootlet tuft:** a stored source of four tapering filaments, splayed ±40° and pressed into the band beside the stem.
- **#30 Rootlet tufts along the stem:** `Operation::Pattern` with `PatternKind::Along`.
  - It follows the path `AlongPath::Chart` (the stem's chart path) at a 1.3 mm pitch.
  - `alternate_deg` is 180, so every other tuft goes to the far side.
- **#100 to #109: five spinel berries and their collets.**
  - Each stone is a `stone_feature` with a `Placement::Ring` solved from its girdle centre and its axis. The tables lean out from the bunch's centre by up to 14°.
  - Each has a `head.bezel` with a 0.25 mm wall and a 0.15 lip.

### Enablers used from master

| Enabler | Used? |
|---|---|
| C-V2 `Along` with `AlongPath::Chart` | Yes, for the rootlet tufts. |
| #248 framed renders (`write_png_framed`) | Yes, for `crown-close`, `crown-hero`, `stones` and `stem-close`. |
| `crisp_relief` / `StampTop::Pillow` | No: the ring has no height-field relief or stamps. |
| `Placement::Ring.level` | Set to false, as the new field requires. |
| C-V3 (sweep scale laws), C-B2 | No. |

## Gates (draft 768 × 320, export 1536 × 448)

| Gate | Draft | Export |
|---|---|---|
| Watertight, 0 degenerate faces, 0 self-crossings | pass | pass (1,157,104 triangles) |
| Every CAD part closed, 0 crossings | pass | pass |
| Solids and parts notes empty, every feature Ok | pass | pass |
| Nothing in the finger hole | 0 inside (nearest 9.300 vs 9.3) | 0 inside |
| Lost-wax verdict | Castable; field thinnest 1.89 mm | Castable |
| Ray-sampled walls | body ≥ 0.801 mm; collets 0.247 mm, rootlets 0.169 mm (detail floor 0.15) | same |
| DFM findings | 0 | 0 |
| Stones reported = previewed; metal inside | 5 = 5; 0 in every stone | same |
| 384 × 192 rebuild | pass | pass |
| Casting pattern closed | pass | pass |
| Triangle budget (2 M) | — | 1,157,104 |
| Cold reload with an empty library | — | identical |

## Template gate (after the last round)

Run as `collection_templates vepres target/tpl-src --output-dir target/tpl --only hedera --verify-export` with class `procedural`. The record is in `showcase/vepres/hedera/verification.json`.

| Check | Result |
|---|---|
| Gate | **passed** |
| Graph size | **299,166 B** against 300,000 B |
| `design.set` patches | 1 (`/manufacturing`) |
| Nodes | 56 |
| Source (lift) | identical |
| Mesh parity | vertices, faces and normals identical (1,157,104 triangles) |
| Cold design and graph reload | pass |
| First build | 1.07 s |

**The budget is what capped coverage.** Each stored leaf or tube costs about 17 B per vertex. Getting under 300 KB meant thinning the stem to 22 sides, the crown leaf to 90 columns and the stalks to 11 sides. That left 0.8 KB, not enough for the second stem and the extra leaves round 2 asked for.

## Disclosure

**A scripting bug in block-out attempt 3.** An edit helper bound `s.replace` once (`R = s.replace`), so most of attempt 3's leaf edits were silently lost before read test 3. The test judged a build that was partly the attempt-2 leaf. The bug was found and fixed in the rethink, and every later edit asserts its match.

## What I could not do

- **Cover the host as densely as Caiman** within the 300 KB procedural budget, because the conforming leaves and tapering stems are stored meshes.
- **Make small (3–5 mm) leaves read clearly** at 0.8 mm minimum section. A three-lobed outline that small needs blunt tips and a thick blade, and it shades as a lump. The reviewers asked for 0.2 mm margins, which lost wax at this floor does not allow.
- **Smooth the cabochon reflections:** `reference-spinel.stl` comes from core gem tessellation (about 5,800 triangles).
- **Low collets on stalked berries:** the bezel always runs down to the metal under it, so a raised berry gets a tube.

## Core changes wanted (exact code)

1. **A bezel that keeps its own depth.** In `crates/ringdesign-core/src/cad/builders.rs`, add a parameter to `BEZEL`:

   ```rust
   BEZEL => vec![
       number("wall_mm", "Wall", "mm", 0.25, 1.5, setting::collet_wall_mm(gem)),
       number("lip", "Lip", "of crown", 0.1, 0.8, setting::collet_lip(gem)),
       Param { key: "own_depth", label: "Own depth", unit: "", min: 0.0, max: 1.0, kind: Kind::Flag, default: json!(false) },
   ],
   ```

   Then, in `build_in`'s `BEZEL` arm:

   ```rust
   let metal = if v.b("own_depth") { seat.surface_z } else {
       floor.and_then(|f| under_wall(gem, v.f("wall_mm"), f)).unwrap_or(seat.surface_z).min(seat.surface_z)
   };
   ```

   Set `own_depth: true` from `geometry_extended`, so older readers are fenced.

2. **Lift stored meshes as their recipes.** In the graph lift, a `Stored` recipe whose `kernel` names a registered ring-local generator should become a parameter node, not a packed mesh:

   ```rust
   // ringdesign-graph/src/lift.rs
   Operation::Stored { recipe, .. } if generators::has(&recipe.kernel, &recipe.op) => {
       graph.node("stored.recipe", json!({ "kernel": recipe.kernel, "op": recipe.op, "params": recipe.params }))
   }
   ```

   With that, Hedera's 20 leaf and tube meshes would cost about 4 KB rather than about 220 KB.

3. **Cabochon tessellation.** In `gems.rs`, let a cabochon's dome follow the build resolution instead of a fixed count:

   ```rust
   let (seg, rows) = (params.theta_steps.clamp(64, 192) / 8 * 8, 48.max(params.profile_steps / 8));
   ```

## Commits on `claude/vepres-hedera` (since the rethink)

- The rethink doc line and read tests 4 and 5.
- Round 1, and its review (revise, 5.6).
- The master merge, round 2, and its review (revise, 6.0).
- Round 3, and its review (cut, 6.0).
- This report and the doc's status line.
