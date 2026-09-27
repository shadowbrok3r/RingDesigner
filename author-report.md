# Fenrir, round 3: final report

- **Branch** `claude/bestiarium-fenrir-r3`, pushed. The checkout is `/home/user/RingDesigner`; the session had no `/home/user/repo`. The branch was cut from `origin/bestiarium-fenrir` and merged with `origin/master` with no conflicts. The export commit is `1de8db4`, and this report is the last commit on top of it.
- **Source:** `crates/ringdesign-core/examples/bestiarium_fenrir.rs`, the only code file changed. There are no `src/` changes.
- **Renders** are in `showcase/bestiarium/fenrir/`: `hero face palm side shoulder reverse stones bare-vs-finished .png`, `face-300.png`, `hero-300.png` and `section-70/90/110.png`. The other outputs are `design.ring.json`, `report.json`, `mesh.json`, `stones.json`, `verification.json` and `artwork/`. The STLs are git-ignored.
- **New CLI modes:** `--hollow` previews the pocket, `--slice=x,u,h,step,along_u` prints a field slice (with `FENRIR_PROBE`), and `--recensus` recounts creases from the saved design.

## Gates (draft 768 x 320 / export 1536 x 448)

| Gate | Draft | Export |
|---|---|---|
| Watertight, degenerate faces | yes, 0 | yes, 0 (1,257,542 tris, within the 2 M budget) |
| Self-crossings: ring; made parts (stone, Fangs, head, hollow) | 0; [0,0,0,0] | 0; [0,0,0,0] |
| Solids / parts notes | empty / empty | empty / empty |
| Nothing in the finger hole | innermost 9.491 / bore 9.500 | 9.491 / 9.500 |
| Lost-wax field verdict, 0.8 mm fill | Castable, thinnest wall 1.32 mm at 255.9° | same |
| **Closed internal voids (new, P0)** | **0** | **0** |
| Land widths | nothing unnamed; head mass ≥ 0.923 mm; 24 teeth ≥ 0.844 mm | same |
| DFM findings | 0 | 0 |
| Stones report / preview | 1 / 1, 4.147 ct, no warnings | same |
| Investment pattern | watertight, 0 degenerates, 0 crossings | same |
| Cold reload (`--verify`) | n/a | identical vertices, faces, normals |
| Weight and metal over the hollow at θ90 | 31.6 g 18k, 1.12 mm | 31.55 g, 1.116 mm |
| **Sections at θ70 / 90 / 110 (new)** | over 1.75 / 1.12 / 1.66 mm, floor 1.16 / – / 1.17 mm | same |

**Template gate:** 0 `design.set` patches, 39 nodes. The template is 2,414,284 bytes against the painted budget of 3,000,000 (round 2 was 2,683,392). Cold design reload, cold graph reload and source parity all pass, the mesh at 1536 x 448 is identical, and there are 0 detail findings.

**Named land-width exceptions** (all other sections are 0.8 mm or more):
- Ear points: 0.44 and 0.46 mm, within 0.81 mm of the tip.
- The four P6 claws: 0.44–0.46 mm within 1.55 mm of the tip, unchanged from round 2.
- The four canine sheaths: 0.50–0.53 mm within 1.6 mm of the tip. They lie on the dome over their claws and are closed with the claws at the bench.
- The nose rule exists but is not triggered.

**Crease census**, edges of 60° or more by zone (export):

| Zone | Edges |
|---|---|
| Cheeks, brow, muzzle, crown, eyes, ears, ear tips, lower jaw and chin | **0** |
| Nose pad | 2 (0.13 mm long, where the pad meets the bridge) |
| Mouth, gums and lips | 23 |
| Teeth | 16 |
| Head flanks and throat | 1,243 |
| Fang claws | 263 (their feet and buried facets) |
| Hollow under the head | 806 |
| Band and painted layers | 16,021 (the fur texture) |

The longest seam crease run is 0.67 mm. Round 2's zone of 943 edges covering "mouth, teeth, fangs and rail" was mostly the claw facets; those are now counted in their own zone.

## Punch list, item by item

1. **P0, sealed void.** Found and closed.
   - What it was: a tunnel of air between the upper gum and the lip body, 0.25 mm over the table. The new gate found 5 such pockets on round 2's geometry: 0.11 mm³ at (0, 1.30, 0.25) and four smaller ones.
   - Fix: the gums now run out under the lips down to the table, and the muzzle's underside is filled solid to the table.
   - New gate: `internal_voids` counts closed shells of negative volume in the finished mesh, and `report.json` carries `internal_voids: 0`.
   - Further finding: the new θ70 and θ110 sections showed the pocket running on past its mouth over a floor only 0.4–0.6 mm thick above the finger. No gate had measured this. The floor is now 1.16–1.17 mm and has its own gate.
2. **Upper lip.**
   - The frame is replaced by a spline-driven flew. It is narrow under the nose (0.85 mm), hitched up to 2.75 mm over the fang, drawn back to 7.05 mm and 2.0 mm wide over the premolars, and curls down to a rounded cap at the corner.
   - Its section is an elliptical roll with no flat top, and a cleft divides the two flews under the nose.
   - `report.json` jaws: `upper_lip_width_min_mm` 0.85, `upper_lip_width_max_mm` 2.02 (narrowest ≤ half the widest).
   - Three snarl wrinkles fan up from above each fang.
   - A mouth-corner notch parts the two jaws: corner gaps are 4.22 mm (round 2: 3.25).
3. **Teeth and lower jaw.**
   - (a) 24 teeth, all listed in `land_widths` at 0.844 mm or more:
     - two chisel incisors at 0.55 and 0.62 mm;
     - premolars as laterally flattened blades at 0.85 and 0.95 mm;
     - a two-cusped carnassial at 1.1 mm;
     - a broad low molar in the corner.

     Each tooth is a stout body kept at 0.8 mm or more until 0.55 mm from its point. The teeth stand nearly upright, 4–10° outward, with at least 0.35 mm clear of the lip. Leaning them 20–30° opened 0.12 mm slots against the lips, which are fins in investment.
   - (b) The lower jaw's outer face carries 3.6 mm locks sweeping back from the chin to each corner. The lower lip is scalloped over the teeth.
   - (c) The chin's reach from the moon's axis is now 8.0 mm, down from 9.3.
4. **Fangs.** P6 cannot round or curve its claws: the tube is fixed at 14 sides (core change 1 below). As interim, each claw is wrapped in a sculpted canine sheath:
   - a round tube 0.12 mm fuller than the claw, following the claw's own arc;
   - it runs out to a round point 0.13 mm across;
   - it is cut to the moonstone's hull, taken from a quick parts build.

   Reach inside the moon's disc is unchanged at 0.80 / 0.81 / 0.75 / 0.77 mm.
5. **Ruff terraces.** The cause was in `flames`: it searched only ±1 row, but a bowed and tilted lock reaches two rows over, so every lock was cut off along the row lines. Those cuts were the stacked contour rings. Changes:
   - `flames` now searches every row a lock can reach;
   - locks have a cos² cross-section and join by a soft maximum of 0.3 of the relief (0.18 mm);
   - the fold-room cap is a soft minimum instead of a hard clamp;
   - hair lines run only along the leading lock and fade where two locks meet;
   - locks are 5.0 mm long with an S-bow.
6. **Ears.**
   - Each ear is a stadium section with 0.5 mm edges, bowed so the back is convex and the front cupped.
   - It twists 13° outward toward the point, which is tapered and rounded.
   - There is a three-lock tuft at the inner base and an ellipsoidal cup.
   - Sub-floor stretches are only within 0.81 mm of the tip; the ears are 0.8 mm or more beyond that.
7. **Seam.**
   - Along the flanks the head's masses swell into a 1.4 mm fillet that runs down onto the band, 0.35 mm under the stock's surface, with the cheek fur riding on it.
   - The ruff starts at the fillet's toe.
   - The throat locks run up over the jaw fur's lower edge.
   - A first try with separate flank locks read as a comb and was dropped.
8. **Hollow mouth.**
   - The mouth is an oval superellipse, 6.2 x 11.6 mm, with no corner tighter than 2 mm.
   - A 0.5 mm round breaks the edge where it meets the bore.
   - The roof is ball-eroded, blurred 0.6 mm and soft-capped, and the pocket mesh is relaxed and settled so no burrs stand off its walls.
   - The result is 31.55 g (limit 32).
9. **Census.** Split by head zone as tabled above, with every zone listed, including zeros.
10. **Muzzle and nose.**
    - The muzzle is 0.8 mm longer: the stop moved up to u 6.3 and the eyes and brows moved up with it.
    - The muzzle narrows toward the nose.
    - The nose is a domed wedge of leather, 1.96 mm across its back and 1.1 mm across its front (round 2's pad was 2.6 mm), with comma nostrils and a philtrum.

## What each part is

- **Stock:** factory 010 trillion, 17 mm wide, size for a 19.0 mm bore, cast in lost wax. The trillion keeps its hard wall-to-face corners.
- **#2 Moonstone:** a 10 mm round cabochon, 4.147 ct, lifted 0.4 mm and tilted 4°.
- **#3 Fangs:** P6 Fang style, Jaws grouping, Point tip, 1.8 mm wire, railless, rise 0.6.
- **#4 Fenrir's head:** a stored sculpt of 117,492 triangles, marched at 0.09 mm. It holds:
  - cranium, cheeks, jowls, brows and eyes;
  - the muzzle and nose;
  - flews, gums and 24 teeth;
  - the four canine sheaths;
  - ears, chin tuft, throat and flank fillets;
  - fur on the cheeks, jaw, crown and throat.
- **#5 Hollow under the head:** a cut part, 416.5 mm³.
- **Painted layers:**
  - Ruff: 0.6 mm, 262° window around the head.
  - Graver's hair lines: 0.08 mm, cut at the bench.
  - Gleipnir: a 0.6 mm two-strand cord at the palm.
  - Gleipnir's bindings: two 0.5 mm wraps.
- **Stamps:** none.

## What I could not do

- **Fangs are sheaths, not true curved claws.** Rounding the claws themselves needs core change 1.
- **Teeth stay stubby.** At 0.55–1.1 mm tall they are blades from the side, but they still read as small rounded knobs in the straight-down face view. A 0.8 mm land floor at that height leaves little room for a point.
- **The flews are still continuous.** They run from the nose to each corner as a clear rolled band, which a strict eye may still call a frame, though it is no longer constant or flat.
- **Lower jaw vs muzzle ratio is not met.** The lower jaw's length is about 1.45 times the muzzle's (chin 8.0 mm from the moon's axis against a 5.5 mm stop-to-nose muzzle), not 1.3. Both jaws must still wrap a 10 mm stone.
- **Head is not deterministic across runs.** The head is marched fresh on each run with the example's own tools, so it can differ in its last bits between runs; the saved design itself reloads identically. I did not switch to `ringdesign_core::sculpt`, whose tools are deterministic.

## Core changes I would like

1. **Round claws.** Add a sides parameter to claw heads. This would let the sheaths go.

```rust
// setting.rs, ClawOptions
pub struct ClawOptions {
    pub style: ClawStyle,
    pub grouping: ClawGrouping,
    pub tip: ClawTip,
    pub rise: f64,
    /// Sides round each claw's section; 0 keeps the original 14.
    pub around: u32,
}

// setting.rs, where each claw is swept (now `tube(&path, &rs, 14, dome)`)
let around = if options.around == 0 { 14 } else { options.around.clamp(6, 64) as usize };
let mut solid = if oval.is_empty() { tube(&path, &rs, around, dome) } else { tube_oval(&path, &oval, around, dome) };

// cad/builders.rs, schema(): after the "rise" param of CLAW | BASKET
params.push(whole("around", "Sides", 6.0, 64.0, 14));

// cad/builders.rs, claw options from params
let options = setting::ClawOptions {
    style: serde_json::from_value(v.0["style"].clone())?,
    grouping: serde_json::from_value(v.0["grouping"].clone())?,
    tip: serde_json::from_value(v.0["tip"].clone())?,
    rise: v.f("rise"),
    around: v.0.get("around").and_then(Json::as_u64).unwrap_or(0) as u32,
};

// cad/builders.rs, claw_geometry_extended(): a non-default side count is new geometry
rails || chosen("style", "Wire") || chosen("grouping", "Even") || chosen("tip", "Dome")
    || set("rise").is_some_and(|v| v.as_f64() != Some(0.0))
    || set("around").is_some_and(|v| v.as_u64().is_some_and(|n| n != 14 && n != 0))
```

The `defaults(CLAW, round)` test's expected JSON gains `"around": 14`.

2. **A core voids check.** Add a closed-voids check to `mesh`, so every lost-wax gate block can use it. This is the example's own function, moved as it is:

```rust
/// Closed shells of `m` with negative signed volume, air sealed inside the metal: each shell's volume and centre.
pub fn internal_voids(m: &Mesh) -> Vec<(f64, [f64; 3])> {
    // union-find over faces sharing vertices; per shell sum dot(a, cross(b, c)) / 6 and the area-weighted centre;
    // keep the shells whose volume is negative (see bestiarium_fenrir.rs `internal_voids`).
}
```

## Self-score

About **7.0 against Caiman = 7**. I have not reached the 7.5 aim.

- **Better:** it now reads as a snarling wolf at 300 px, with round curved canines, blade teeth, flowing ruff locks, cupped ears, a fillet into the ruff and a clean oval hollow. All gates are green, including two new ones that caught real defects (the void and the thin floor).
- **Holding it below 7.5:** the flews still read as a continuous rolled band, the teeth are small knobs from straight above, the lower jaw is long relative to the muzzle, and the ruff's hair lines still alias slightly at draft resolution.
