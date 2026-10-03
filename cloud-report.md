# Vepres — Datura (`datura`): cloud report

**Outcome: stopped at the block-out.** Three read tests, three times `reads: false`. TASK.md says a third failed block-out means
the subject needs rethinking, not detailing, so no review round was started (0 of 3 rounds used). No extension is recorded in the
collection doc, so none was taken.

Branch `claude/vepres-datura` (merged with master `2e11632`). Head before this report: `e1bd1b3`.
Ring: `crates/ringdesign-core/examples/vepres_datura.rs`. Outputs: `showcase/vepres/datura/`.
Build: `cargo build --release -p ringdesign-core --example vepres_datura`. Run: `target/release/examples/vepres_datura [OUT] [--draft] [--verify] [--blockout] [--bare]`.

## Read tests (independent reviewer agents, `target/review.md`, read-test mode)

| # | Commit | reads | What the eye saw |
|---|---|---|---|
| 1 | `4be75ba` | false | "An urchin or a spiked ball", a chestnut burr or a morning-star mace. Too many long spines hid the egg, and the split and seeds did not register. Cheeks read as bare metal. |
| 2 | `295a1a2` | false | "Thistle, pine cone, spiky bud". The pointed head read as a closed bud or artichoke, the spines as scales and the seeds as set gems. Serrate leaves in an X read as holly or a thistle badge. Buds invisible. |
| 3 | `211e6d1` | false | "Chestnut burr, urchin, cracked pod" ("the right family"). The quartered crown read as a hot-cross bun or Celtic boss. The mouth-on trumpet read as a starfish, the bud as a candle flame, the leaves as crumpled foil or bananas. |

The full JSON, with each reviewer's three changes and acceptance tests, is in `showcase/vepres/datura/read-test-{1,2,3}.json`.

**What moved and what did not.** Each attempt applied the previous reviewer's changes. Attempt 2 brought the 24 short stout spines, the flared lips and the foliage. Attempt 3 brought the round egg, a 2 mm gap 4 mm deep, the trumpet flower and broad unserrated leaves. The read improved from "urchin" to "cracked seed pod / chestnut, the right family". Three things never landed:
1. **Spine count pulls two ways.** Reviewer 1 asked for fewer, shorter spines (24 to 28, about 1 mm) so the egg shows. Reviewer 3 asked for the full 40 or more at 1.4 to 1.8 mm. A thorn-apple sits between urchin and chestnut, and no single spine density read as datura to both.
2. **The renderer's top light makes the split floor bright.** Any floor a seed can sit on faces up, so it catches light. The cross read as an inlay, not a dark crack.
3. **The flower and leaves are small on this table.** Past the frill, the table leaves about 4 to 6.5 mm of run on any bearing. Seen face-on, the trumpet mouth is a star, not a flower.

**Recommendation for the rethink.** The signature that names datura is the long white trumpet flower, not the capsule. At 300 px a spined fruit always lands in burr, urchin or chestnut. Build the ring around one large trumpet, 18 to 22 mm, in three-quarter profile: lying along a shoulder, or wrapping the shank, with its mouth flaring past the badge. Keep the capsule as a supporting element at a smaller scale. Alternatively, move to a base with more table (018 Butterfly is the doc's fallback), so the foliage can reach 9 to 10 mm leaves.

## Process

Lost wax on native factory 011 Badge (18 x 20, not mirrored), 18k yellow gold, bore 18.6 mm. Logan's 2026-10-03 rule is recorded in Datura's section of `docs/collections/vepres.md`: judged as lost wax, 0.8 mm minimum section, no pull rule. Sand bonus: not pursued. Spines radiate from a dome, and the field reports a 10% undercut in a two-part pull.

## Gates (final block-out, attempt 3 geometry, `--blockout --verify`)

Every gate is green at draft (768 x 320) and at export (1536 x 448).

| Gate | Draft | Export |
|---|---|---|
| Watertight, degenerate faces, self-crossings, shells | yes, 0, 0, 1 | yes, 0, 0, 1 |
| Self-crossings on every made part (40 features) | 0 | 0 |
| `solids.notes`, `parts.notes`, features not `Ok` | empty, empty, none | empty, empty, none |
| Nothing in the finger hole: least margin to the bore | -9e-8 mm (the bore itself) | same |
| Lost-wax verdict, thinnest wall (field) | Castable, 1.58 mm | Castable, 1.58 mm |
| DFM findings | 0 | 0 |
| Stones reported / previewed, metal in stones, seat warnings | 8 / 8, none, none | 8 / 8, none, none |
| Triangles (budget 2 M) | 339,170 | 339,170 |
| Casting pattern (`try_build_pattern`) | n/a | watertight, 0 degenerate, 0 crossings |
| `--verify` cold reload, empty library | n/a | identical vertices, faces, normals |

Weight: 33.9 g in 18k (the capsule alone is about 500 mm³). Design format 6, 4.03 MB.

Notes on the numbers:
- **Triangle count.** It is the same at both resolutions because the imported stock and the stored meshes do not depend on the band's theta and profile steps.
- **Crowding census.** It lists each split's seed pair at 0.46 mm at the girdle and 0.36 mm deep, and adjacent inner seeds at 0.58 / 0.50 mm. Explained: two 1.5 mm seeds share one 2 mm split arm by design. No pair touches and no metal enters a stone.
- **Thin details.** Spine tips are 0.32 mm across (lost-wax detail floor 0.15). The trumpet wall is 0.85 mm. The leaf margins stand 0.16 mm proud of the table. These are detail, judged at the detail floor; the field's 1.58 mm is the section gate.

## Template gate

Run with `collection_templates vepres … --only datura --verify-export`, as class `procedural` and as class `stock`:
- **`design.set` patches:** 1 (`/manufacturing`).
- **Nodes:** 74, with 0 exposed controls.
- **Cold source:** identical, through the lift. Cold graph reload and editable reload pass.
- **Export mesh parity:** vertices, faces and normals identical at 1536 x 448, 339,170 triangles.
- **Size:** 4,053,828 bytes, against a budget of 300 KB (procedural) or 1 MB (stock). It is over the 3 MB flag, so **`template_gate_passed: false`**.
- **Open:** first build 3.4 s.

The weight is the stored meshes: the capsule (about 258k triangles), four leaves, the flower and the bud, each packed into the graph. See core change 2.

## CAD feature tree, as sentences

1. **Badge** is the procedural band, carrying factory 011.
2. **Thorn-apple capsule** is a stored mesh seated by `Placement::Ring` at 90° on the table's centre and joined (cast). It is one closed solid in the capsule's own frame, made of four pieces:
   - a 4.5 mm egg, blunt-crowned at 9 mm, sunk 0.9 mm into the table;
   - a five-lobed reflexed calyx frill (r 4.9) with its fillet modelled into its edge;
   - four V-section splits holding a 2 mm clear gap, running 4 mm deep under the crown, with lips flared 0.5 mm;
   - 24 Poisson-scattered conical spines, 0.9 to 1.1 mm long, 0.84 mm at the root and 0.32 mm at the tip, kept 0.8 mm clear of every lip and unioned by `csg::union_all`.
3. **Datura leaves 1 to 4** are stored meshes, `Placement::Relative` to the capsule and joined. They lie at bearings 35°, 150°, 228° and 345°, from 3.3 mm out, up to 8.6 mm long and 6 mm wide. Each is a sculpted pillow solid:
   - a domed two-slope top, highest on a raised midrib;
   - a side vein into each lobe;
   - a sinuate margin of irregular rounded lobes, standing 0.16 mm proud;
   - a blade that curls down past the table's edge.
4. **Datura flower** is a stored mesh, Relative to the capsule and joined. It is an open trumpet 10.6 mm long on bearing 300°, toward the hero camera:
   - a five-ribbed calyx tube;
   - a shelled bell with a 0.85 mm wall, flaring to a 6.4 mm mouth pleated into five points;
   - a bend of 30° up off the table from 5.8 mm along it.
5. **Datura bud** is a stored mesh, Relative and joined: a furled corolla 8 mm long on bearing 100°, spiral-twisted shut to a point.
6. **Seed pads 1 to 8** are kernel cylinders (r 0.98, h 1.0), each Relative to the capsule with its axis turned onto its split floor's normal by `rotation_deg`, joined, and standing 0.05 mm proud.
7. **Seeds 1 to 8** are black spinel round 1.5 mm stones (tint 0.03, 0.03, 0.04). Each sits on its pad's top planar face through a `FaceSeat`, two per split arm.
8. **Seed seat 1 to 8** are `seat.bur` cuts (cast).
9. **Seed thorns 1 to 8** are `head.claw` heads with 3 prongs, `style: Thorn`, `tip: Point` and `rails: None`. They ground on the pad, so their legs stay short instead of running down to the table.

## Enablers used from master

- **C-V1 `Placement::Relative`** (and the new `level` field). Once it landed, the capsule seats by `Ring` and the leaves, flower, bud and pads all follow it, so the head follows a resize. Built `Free` until then.
- **P6 claw styles** (`Thorn`).
- **#248 render API:** `write_png_framed`, `yaw_facing`, `Framing` for the close-ups.

Not used: C-B2 and #257 (round stones only), #255 (no lettering), #258 and #259 (the forms are stored meshes), `crisp_relief` and `StampTop::Pillow` (no height-field relief or stamps). C-V2 to C-V5 and C-T5 to C-T7: none were needed by what was built.

## What I could not do

- **The read.** See above.
- **The doc's CAD recipe as written.** It called for a Revolve capsule, an Extrude slot cut and `About` spine arrays. The reviewers' shape changes (wedge splits that gape, flared lips, scattered spines) needed a sculpted parametric surface, so the capsule is one stored mesh. The template therefore carries meshes rather than editable features.
- **The template budget.** See core change 2.
- **Leaves on the cheeks.** The 011 side walls are 3 mm tall, so the leaves sit on the table and curl over its edge.

## Core changes wanted (exact code)

**1. Setting builders ground on the part their stone stands near, not only the band.** `cad.rs`, `build_made`: a claw or bezel on a stone placed above a joined part reaches past it to the table. I worked around it with kernel seat pads. Change:

```rust
// after: grounds.extend(on.and_then(|stone| stood_on(doc, stone, values)).map(|mesh| Ground::new(&mesh, &frame, reach, below)));
if spec.on_stone {
    for (id, v) in values.iter() {
        let joined = doc.feature(*id).is_some_and(|f| f.component.attach == Attach::Join && f.component.stage == Stage::Cast);
        if let (true, Value::Mesh(m)) = (joined, v) {
            grounds.push(Ground::new(&m.mesh(), &frame, reach, below));
        }
    }
}
```

**2. Stored meshes weigh the template down.** Pack `stored::Packed` with a quantised, delta-coded and deflated vertex stream, which takes it from about 24 bytes per vertex to about 6:

```rust
// cad/stored.rs
pub fn encode_compact(v: &[[f64; 3]], f: &[[u32; 3]], quantum_mm: f64) -> Packed {
    let q: Vec<i32> = v.iter().flatten().map(|x| (x / quantum_mm).round() as i32).collect();
    let deltas: Vec<i32> = q.chunks(3).scan([0i32; 3], |p, c| { let d = [c[0] - p[0], c[1] - p[1], c[2] - p[2]]; *p = [c[0], c[1], c[2]]; Some(d) }).flatten().collect();
    Packed::deflated(quantum_mm, &deltas, f)
}
```

Alternatively, add a `Stored` recipe that regenerates from its parameters through a registered example kernel.

**3. Crease-aware normals in `render::finished_from`.** After a union, the table's large triangles share vertex normals with the joined parts' seams, so flat faces smear. My example re-normals the render mesh with a 35° crease (`creased()` in the example). Suggested in core:

```rust
pub fn finished_from(design: &crate::RingDesign, lib: &crate::AlphaLibrary, built: crate::BuildResult) -> Finished {
    let stones = crate::gems::built_meshes(design, lib, &built);
    Finished { metal: crate::mesh::creased(&built.mesh, 35.0), stones }
}
```

#248 may already cover this; I have not re-checked without it.
