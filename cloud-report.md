# Harpyia — the snatcher, round 3 (final report)

- **Branch:** `bestiarium-harpyia` in `/home/user/rd` (origin/master merged in cleanly at `76faae4`, no conflicts). HEAD is
  the commit that adds this file; its parents are `166c4bb` and `e0d676c`.
- **Push:** every `git push origin HEAD:claude/bestiarium-harpyia-r3` was refused by the session's git proxy with a 403:
  "shadowbrok3r/RingDesigner is not in this session's authorized repository set". Nothing reached GitHub. The commits
  are local only; the repository has to be added to the session's sources before they can be pushed.
- **Example:** `crates/ringdesign-core/examples/bestiarium_harpyia.rs`. No `src/` changes.
- **Renders:** `showcase/bestiarium/harpyia/{hero,face,palm,side,shoulder,reverse,stones,bare-vs-finished,contact-300}.png`
- **Other outputs:** `report.json`, `design.ring.json`, `stones.json`, `artwork/plumage.png`,
  `template-verification.json`. The STLs are git-ignored.
- **Cameras:** unchanged from round 2. `shoulder` is still (-0.9, 0.62).

## Punch list, item by item

1. **Land widths (gate).**
   - **Cause:** the march stopped at 3 mm, shorter than the torso's section, so Torso fell through to the `f64::MAX` sentinel.
   - **Fix:** rays now run to `LAND_REACH_MM = 12`. `land_widths` returns `Result` and fails the build when a part is
     non-finite, ≤ 0 or > 10 mm, or when any authored part has no entry. An unmeasured part can no longer pass.
   - **Result:** Torso measures 2.562 mm. All 73 joined parts are finite, from 0.812 to 2.56 mm, with 0 mm² under the floor.
   - **Thinnest wall:** 3.088 mm at **θ 84.375**, quoted exactly as `report.json` gives it. The new parts moved it from
     θ 28.125.
2. **East crown.**
   - **Removed:** the transverse-scute column, both bead rows and the hex cheeks. The painted band now has no scales
     anywhere. Scutes appear only on the lofted tarsi and the tops of the toes.
   - **Breast and belly plumage:** painted contour feathers that flow east from under the stone, from θ 78 round
     through θ 0 and on under the tail to θ 256.
     - Length grades from 1.6 mm at the stone, by 0.064 mm per mm, to 3.1 mm by θ 330.
     - Crown feathers are 0.72 of their length wide, cheek feathers 0.66. All are softly pointed, with closed
       outlines, a raised rachis and rolled free edges, and point away from the stone.
     - Relief is 0.36 mm.
   - **Painter fix:** the `arc()` helper faded only on its start side, which left the hard ridge past the tail tips.
     It now fades on both sides.
3. **Feet.** Each foot is a fan springing from an ankle pad beside the band's edge (ψ 118, 2.9 mm below the girdle).
   - **Hallux:** roots on the west of the pad and hooks forward over the girdle at ψ 143, facing the front toes.
   - **Front toes:** hook at ψ 117, 94 and 71, spaced about 22° apart. Measured ankle-to-tip lengths are 4.89, 5.93 and
     8.50 mm, so the **middle toe is 1.43× the outer**.
     - Honest caveat: they are graded by reach, so the middle toe hooks farthest forward and not between the other two.
       This is an eagle's lateral view laid round the girdle.
   - **Toes:** 1.24 mm at the base, with knuckle pads to 1.40 mm and a shingled scute over the top of each phalanx.
   - **Sheath:** each toe swells to a collar where its claw leaves it.
   - **Claws:** oval, 0.96 × 1.2 mm, stepping 0.14 mm in the curl plane and 0.26 across under the collar. They taper
     to a blunt 0.92 mm tip.
   - **Curls:** 95, 86, 67 and 87° (span 28.5°).
   - **Gaps:** toes and claws are 0.660 mm apart wherever they run within 1.3 mm of the stone. They meet only at the
     ankle pad, so no ring forms. Claws sit 0.060 mm from the stone, and no metal is inside it.
   - **Tarsus:** a knee tucked under the flank, a feathered drumstick thigh with three lanceolate feathers, then a long
     tarsus thrust forward and down (a strike pose). It has three shingled scutes on its **front face only**, at 0.62 mm
     pitch with a 0.12 mm step.
4. **Face.** I committed to a face.
   - **Build:** the head is a 21-slice loft. Each slice is a closed Catmull-Rom loop through a superelliptic front
     carrying relief and an elliptic skull behind.
   - **Features:**
     - brow ridge +0.30 mm, overhanging;
     - sockets −0.50 mm, with the eyeballs +0.20;
     - nose rising to 0.50 mm at the tip;
     - upper and lower lips with a −0.16 mm mouth cleft;
     - chin and cheekbones.
   - **Size:** the lower face is drawn out 10% (`FACE_STRETCH`), so brow to chin is about 2.8 mm (round 2: 1.9).
   - **Pose:** the head bows 30° east and turns 55° toward the high cheek. `side.png` shows it three-quarter, with brow,
     both sockets, nose, lips and chin. `shoulder.png` sees it in profile at the stone.
     - I could not also turn it toward the shoulder camera, which sits west of her.
     - I tried Manticora's east shoulder (yaw 0.75), but the high foot hides the face from there, so I kept the
       original camera.
   - **Hair:** six round locks (0.5 mm radius) parted over the brow, swept back over the skull and falling down her
     back. This replaces the plumes that read as a helmet crest.
   - **Reach:** the head's own reach is under 18.45 mm and the hair's 18.49, both at or under 18.5. The whole ring reaches
     18.52 (wing).
5. **Primaries.**
   - **Shape:** the leading vane is ⅓ of the width and the trailing ⅔. Each vane closes to 0.3 of the width over its
     outer half, with a short round.
   - **Stagger:** tip radii step by +0.45, −0.2, +0.35, −0.25, +0.4 and −0.1 mm.
   - **Fan:** the primaries are 3.2 mm wide, so the fan overlaps more, with a shallower emargination (keeps 0.8).
   - **Channels:** only the two outermost primaries keep vane channels, to fit the triangle and template budgets.
   - **Section:** land width is ≥ 0.83 mm on every primary.
6. **Torso.**
   - **Body:** the torso now lies along the crown from a tail end sunk in the band at θ 160, rising to her chest at
     θ 117.5, and tapers into the mantle and tail. The old upright torso ringed with knobs is gone.
   - **Feathers:** 11 lanceolate breast and mantle feathers, tapered to 0.35, lie flat at 0.55 mm proud and overlap
     down toward the tail.
7. **Palm tab.** The painted cheek feathers now run from θ 84 to 256, and the belly plumage continues under the tail.
   Where the tail covers, the plumage beneath sits at 0.55 of its height, so the rectrices overlap it. No polished field
   remains on either side face between θ 230 and 310.
8. **Template exposure.** None is exposed. Every part is `Placement::Free` in world coordinates, and the eight claws
   are lofts fitted to the 8 mm girdle:
   - a larger stone would swallow the claws, and a smaller one would leave them floating;
   - ring size would move the band out from under every part.

   The template still lifts with 0 patches.

## Gates

| Gate | Draft 768×320 | Export 1536×448 |
|---|---|---|
| Triangles | 1,151,886 | **1,973,164** (budget 2,000,000) |
| Watertight / boundary / non-manifold / degenerate | yes / 0 / 0 / 0 | yes / 0 / 0 / 0 |
| Mesh self-crossings / parts crossed (75 made parts) | 0 / 0 | 0 / 0 |
| Solids notes / parts notes | [] / [] | [] / [] |
| Build time | 12.2 s | 14.7 s (preview 384×144: 5.4 s) |
| Vertices in bore; nearest vs bore r 9.461761 | 0; 9.461683 | 0; 9.461763 |
| Field verdict (lost wax, 0.8 mm) | Castable, wall 3.088 @ θ 84.375 | Castable, wall 3.088 @ θ 84.375 |
| Two-part undercut (reported only) | 21.47 % | 21.47 % |
| Land widths (73 joined parts) | min 0.812, 0 mm² under | min 0.812, 0 mm² under |
| DFM findings | 0 | 0 |
| Stones reported / previewed | 1 / 1, 1.936 ct | 1 / 1, 1.936 ct |
| Cold reload (empty library) | identical | identical |
| Investment pattern (scale 1.01317) | — | watertight, 0 degenerate, 0 crossings |
| Reach / z extent / 18k | 18.52 / 12.14 / 33.21 g | 18.52 / 12.14 / 33.22 g |
| Talon curls / span / toe gap / stone gap | 95, 86, 67, 87 / 28.5° / 0.660 / 0.060 | same |

- **Stones warning:** the one remaining stones warning, "no setting holds this stone", is the report's blind spot for
  free-placed hand-made claws. See the core change below.
- **Sand-only gates and 384×192:** the sand-only gates do not apply (lost wax), and the 384×192 rerun does not apply
  (no stamps).

**Template gate**, run into `target/tpl` with class painted and `exposed_controls: []`:

| Check | Result |
|---|---|
| `template_gate_passed` | true |
| Source method | lift |
| `design.set` patches | 0 (`[]`) |
| Nodes | 90 |
| Template bytes | 2,811,605 of 3,000,000 (93.7%) |
| Source identical | true |
| Vertices, faces and normals identical at 1536×448 | true |
| Cold design, cold graph and editable graph reloads | true |
| `detail_findings` | 0 |
| First build | 5.45 s |

The verification is saved as `showcase/bestiarium/harpyia/template-verification.json`.

## What each part is

- **Band:** Flat 7.2 × 3.2 mm, comfort 0.2 mm, US 9, lost wax in Gold 18k.
- **Stone:** an 8 mm round sapphire standing 1.3 mm, with a through seat bur (the one cut part).
- **Layer "Plumage":** one height field on a 1024-column square-texel atlas, 0.36 mm, containing:
  - the mantle, west;
  - cheek feathers under the wings, running to the palm;
  - the graded breast and belly, east;
  - the nine-rectrix tail at the palm.
- **Figure:**
  - Torso: an oval loft along the crown.
  - Neck.
  - Head: the 21-slice face loft.
  - Six hair locks.
  - Eleven lanceolate breast and mantle feathers.
- **High foot (15 parts), and "Low foot", one mirror of them:**
  - thigh and three thigh feathers;
  - scaled tarsus;
  - four toes (circle-section lofts with top scutes and sheath collars);
  - four oval claws.
- **High wing:** 36 lofted feathers, rooted 12° further back so they clear her face from the high cheek:
  - 6 secondaries;
  - 7 asymmetric primaries;
  - 5 greater coverts;
  - 5 lesser coverts;
  - 13 marginal coverts in two staggered rows.

  "Low wing 1–3" mirror them.

To fit the 2 M triangle budget and the 3 MB template, I trimmed stations that don't show:

- a single dome section on the flight feathers;
- one fewer station on the primaries, coverts and marginals;
- vane channels on only the two outermost primaries.

## What I could not do

- **Reticulate scales behind the tarsus scutes.** The tarsus is a circle-section loft, so its back stays smooth.
- **A face turned to the west shoulder camera.** The face turns to the high cheek instead: three-quarter in `side.png`,
  profile in `shoulder.png`.
- **Pointed claws.** The 0.8 mm floor keeps every claw a blunt 0.92 mm tip. Seen from the stones camera, the claws
  still read somewhat as fingertips.
- **A bigger head.** The 18.5 mm reach cap keeps her head small at 300 px (about 25 px tall in `side.png`).
- **Stone size exposed as a template control** (item 8).
- **The push** (see the top).

## Core change I'd like

A hand-made head that is `Placement::Free` should count as the stone's head when it declares
`ComponentRole::Head`. That would clear the stones report's "no setting holds this stone" for lofted claws. In
`crates/ringdesign-core/src/setstone.rs`, `hand_head`:

```rust
fn hand_head<'d>(doc: &'d Document, stone: &Feature) -> Option<&'d Feature> {
    let at = &stone.component.placement;
    if *at == Placement::Free {
        return None;
    }
    doc.features.iter().find(|h| {
        h.enabled
            && h.id != stone.id
            && h.component.role == cad::ComponentRole::Head
            && h.component.attach == cad::Attach::Join
            && (h.component.placement == *at || h.component.placement == Placement::Free)
            && !from_builder(doc, h)
            && !moved_away(doc, h.id)
    })
}
```

The example would then set `role: ComponentRole::Head` on its eight claw parts.

## Self-score against Caiman = 7

**7.3.**

- **Strengths:**
  - One theme now runs from face to palm: a feathered figure, feathered breast and belly, and a tail.
  - Half the ring no longer reads as a serpent.
  - At 300 px, `side.png` reads as a woman's head bowed over a gem, a crouched winged body, and a scaled leg striking
    at the stone.
  - The face is a real face, with brow, sockets, nose, lips and chin.
  - From above, the feet are two separate four-toed grips with opposed halluces and no ring.
  - Every gate is green at both resolutions, and the template passes with room to spare.
- **What holds it under 7.5:**
  - The head is small at 300 px.
  - The claws are blunt and read somewhat as fingertips from the stones camera.
  - The leg, thrust forward, can still read as an arm at a glance.
