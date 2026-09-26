Harpyia round 2 is done and committed, with every gate green at draft and export and the template gate passing. My self-score is **6.8 against Caiman's 7**: all nine punch items are applied, but her face is only legible at full size.

**Branch:** `bestiarium-harpyia` · **HEAD:** `6ce7589` · **Worktree:** `/home/shadowbroker/Documents/Rust/JewelryProjects/RingDesigner/.claude/worktrees/bestiarium-harpyia`
Round 2 commits: `0c8ab9b` (milestone), `b55cb57`, `6ce7589`. The C-B4 enabler is still `ca5c924`.

## Renders and outputs
All in `showcase/bestiarium/harpyia/`:
- **Renders:** `hero.png`, `face.png`, `side.png`, `shoulder.png`, `palm.png`, `reverse.png`, `stones.png`, `bare-vs-finished.png`.
- **300 px sheet:** `contact-300.png` (hero, face, side, shoulder, palm, stones).
- **Data:** `report.json`, `stones.json`, `design.ring.json`, `artwork/plumage.png`.
- **STLs (git-ignored):** `finished-metal.stl`, `casting-pattern.stl`, `reference-sapphire.stl`.
- **Template:** `target/tpl/harpyia/{template.graph.json, verification.json, editable-graph.ring.json}`.

## Gates
| gate | draft 768×320 | export 1536×448 |
|---|---|---|
| triangles (budget 2 M) | 1,086,310 | 1,921,526 |
| build time | 6.1 s | 7.1 s (preview 384×144: 2.5 s) |
| watertight / boundary / non-manifold / degenerate | clean | clean |
| self-crossings: mesh / parts | 0 / 0 | 0 / 0 |
| solids and parts notes | [] [] | [] [] |
| parts joined / cut | 63 / 1 | 63 / 1 |
| cold reload (`--verify`) | — | identical |
| field verdict (lost wax, 0.8 mm fill) | Castable, wall 3.09 mm | Castable, wall 3.09 mm at θ 80.2° |
| DFM findings | 0 | 0 |
| stones reported / previewed | 1 / 1 | 1 / 1 (1.936 ct) |
| land widths, 63 joined parts | ≥ 0.833 mm, 0 mm² under | ≥ 0.833 mm (Face), 0 mm² under |
| talon curl over last 2 mm | 90.2–90.6° | 90.2–90.6° |
| toe / figure gap to stone; metal in stone | 0.040 / 0.121 mm; 0 | 0.040 / 0.121 mm; 0 (nearest 0.043) |
| bore | 0 vertices in | nearest 9.4618 of 9.4618 mm |
| z extent / reach / 18k weight | 12.07 mm / 18.57 mm / 32.38 g | 12.07 mm / 18.57 mm / 32.39 g |

- **Two-part undercut** is reported, not gated, under lost wax: 11.62% on the band surface and 20.0% counting the parts.
- **Investment pattern** (×1.0132): watertight, 0 degenerate faces, 0 crossings.
- **Design file:** 2,872,101 bytes at format 6.
- **One stone warning remains:** "no setting holds this stone". It is explained in `report.json` under `stone_warnings_explained`.

**Template gate: passed.** The design lifts with 0 patches into 80 nodes.
- Size: 2,886,804 of 3,000,000 bytes (96.2%).
- Source identical, and vertices, faces and normals identical at 1536×448.
- Cold design, cold graph and editable graph all reload.
- Detail findings: 0.
- Open times: read 21 ms, evaluate 32 ms, first build 3.32 s, rebuild 4.6 ms, verdict 10.8 ms.

## Per-item account
1. **Land widths — done.** `land_widths` lists all 63 joined parts; the minimum is 0.833 mm, with nothing under the floor.
   - Every claw is cast blunt with a 0.85 mm round tip.
   - The four high talons and the Low foot mirror carry the bench note: "points filed after setting if wanted".
2. **Two raptor feet — done.**
   - Each tarsus is a meshed round loft from her thigh, 1.3–1.5 mm across, ringed every 0.42 mm.
   - Each foot has a hallux and three front toes. Each toe has two knuckle swellings, so three segments.
   - The claws curl about 90° over their last 2 mm onto the girdle.
   - The high foot is on the +z side and its mirror on −z.
   - The six straight pins, the gallery rails and the rod legs are gone.
3. **Figure — partly met.**
   - Built: meshed-loft torso, neck, head and a sculpted face loft, plus three hair plumes, 8 breast and 3 mantle contour feathers. The wings root at her shoulders.
   - `side.png` reads as one hooded creature bowed over the stone. The face shows brow, nose, lips and chin in profile, but only at full size.
   - The face is 1.9 mm from brow to chin because the reach cap is used to 18.57 mm. Its eyes are shallow hollows and do not read.
   - At 300 px it is effectively the review's fallback: a feathered hood with the face in shadow.
   - `shoulder.png` sees the back of the hood, not the face.
4. **West-only wings — done.**
   - The through-stone mirrors are dropped; the wings are mirrored across the band only.
   - They sweep back from her shoulders toward the palm, and `face.png` no longer reads as a bow tie.
   - The east shoulder now carries one column of transverse scutes: shingled, bowed toward the foot and narrowing from 5.6 to 4.2 mm, with small reticulate scales on its flanks. The thigh feathers flow into it.
   - Weight fell from 41.1 g to 32.39 g.
5. **Tail — done.**
   - Nine painted rectrices fan from a rump at θ228, each with its own outline, rachis and rounded tip. They stand 0.36 mm proud.
   - The mantle covers the crown to θ249, then the fan and the thigh feathers take over, so there is no blank polish between θ230 and 310.
6. **No zigzag near the stone — done.** Everything within 8 mm of the stone is a lofted part, a closed-outline contour feather, a scute or a scale.
7. **Primaries and coverts — done.**
   - Seven primaries fan across headings 185→278°: 93° against round 1's 64°, so +29°.
   - Lengths are graded 4.6 / 6.0 / 7.2 / 7.6 / 7.2 / 6.4 / 5.4 mm.
   - The outer five are emarginated, with domed tips and gaps between them.
   - Marginal coverts run in two staggered rows of 7 and 6, graded 2.5 → 1.2 mm from shoulder to wrist, tips pointing back.
8. **Weight and size — done.** 32.39 g of 18k, 12.07 mm along the finger, 18.57 mm reach. The band is now 7.2 × 3.2 (was 3.8 thick).
9. **Report fields — done.**
   - `report.json` has a `draft` block, `preview_build_s`, and `triangles_at_1536x448`: 1,921,526, under 2 M.
   - The export now builds at 1536×448 itself.

## What the ring is made of
- **One painted layer, "Plumage" (0.36 mm):**
  - contour feathers on the mantle, thighs and west cheeks
  - the tail fan at the palm
  - the scute column on the east crown
  - hex scales on the east cheeks
- **The stone:** an 8 mm round sapphire standing 1.3 mm off the crest, with a seat bur cut through (the one cut part).
- **Lofted parts:** Torso, Neck, Head, Face, 3 hair plumes, 11 breast and mantle feathers, the high tarsus with 4 talons, and 36 feathers per wing (6 secondaries, 7 primaries, 5 greater, 5 lesser, 13 marginal).
- **Mirrors:** "Low foot" and three "Low wing" mirrors onto the other side face.

## What I could not do
- The face stays small and its eyes do not read, for the reach reason in item 3.
- Seen from above in `face.png`, the breast plumes read as knobs round the upright torso.
- The talons cast blunt; sharp points exist only as a bench step.
- The stones report does not see hand-made claws as a setting, so the warning stays.
- The template sits at 96% of its budget, so there is little room for more lofted parts. Loft sketch points dominate the JSON.

## Core changes wanted
**1. Let a hand-made part hold a stone.**
- `cad.rs`, `Component` (and its wire struct and Default):
  ```rust
  /// Stone features this part holds when it is a setting made by hand.
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub holds: Vec<Id>,
  ```
- `setstone.rs`:
  ```rust
  pub fn holder(doc: &Document, id: Id) -> Option<&Feature> {
      held_by(doc, id, false, 0).or_else(|| {
          doc.features.iter().find(|f| f.enabled && f.component.attach == Attach::Join && f.component.holds.contains(&id))
      })
  }
  ```
- `stones.rs`, in `made_by`:
  ```rust
  match &head.operation {
      Operation::Builder { key, .. } => Some((head.name.clone(), builders::spec(key)?.key)),
      _ => Some((head.name.clone(), "made by hand")),
  }
  ```
- Pin with a Join loft that lists the stone (warning clears) and a Separate one that does not. An older build that drops the field only shows the warning again, so it needs no fence.

**2. Move land-width measurement into core.** Something like `dfm::part_sections(solid, up: Option<P3>) -> (min_mm, under_floor_mm2)`. Every lost-wax lane currently writes its own ray version.

**3. Store loft sections more compactly**, for example sharing identical sketches, to free room under the template budget.

## Enabler (C-B4) tests
From Part 1, commit `ca5c924`:
- core: 770 passed, 15 ignored, plus golden 1
- graph: 119 passed
- workbench: 218 passed
- workspace check: 0 warnings
- wasm check: OK

`git diff ca5c924` over `crates/` shows no change outside the example file, so these still hold.

## Self-score: 6.8 against Caiman 7
- **Up from 5.8:** she now has a body, a head, real clutching feet and a direction to the wings, and the tarsus shoulder and tail make it one creature from face to palm.
- **Short of 7.5:**
  - At 300 px the head reads as a hood rather than a face.
  - From above, the torso is knobbly.
  - The blunt claws soften the snatch.