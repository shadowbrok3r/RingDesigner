# Capsa (Tenebrae): final report

**Outcome: cut at 5.5 after all three review rounds.** The reviewer's verdict stands. Every gate is green at draft and at export, and the template gate passes on the final build. The ring reads as its subject at 300 px, but the reviewer found the modelling too thin for the collection.

Branch `claude/tenebrae-capsa-rethink`. The process is lost wax with two castings, the ring and a separate lid on a bench hinge. Per Logan's rule of 2026-10-03, the ring is judged at a 0.8 mm minimum section with no pull rule, and that decision is recorded in the Capsa section of `docs/collections/tenebrae.md`. No extra rounds were granted, and none were used.

## Step 0: the three options and the read tests

The read tests are numbered across options. Each option got one revision.

| Test | Option | Reads | What the eye saw |
|---|---|---|---|
| 1 | across | false | A Gothic church front on a signet, a flat façade on a slab |
| 2 | openwork | false | A small gabled house on a ring, with a saw-tooth ridge |
| 3 | across-openwork | false | A Gothic church front, a gable with a blue cabochon, two pinnacle towers |
| 4 | across (revised) | false | A small Gothic church front, the top still flat |
| 5 | openwork (revised) | false | A gabled house or shrine on a plinth |
| 6 | across-openwork (revised) | **true** | A Gothic chapel front with a crocketed gable, spired pinnacles, and a skull in a pointed-arch niche |

I took across-openwork (test 6). The records are in `showcase/tenebrae/capsa/{across,openwork,across-openwork}/read-test-N.json`, next to each option's renders.

**How I read the options:**

- **across:** the chasse lies on its back across the finger, so its west front (gable, portal and lancet) faces the face camera.
- **openwork:** the chasse lies along the finger, and its roof is pierced with tracery over the skull.
- **across-openwork:** the west front faces up, its portal is opened into a traceried window over the skull, and the roof is pierced.

## Review rounds (full-review mode, a fresh reviewer each round)

| Round | Verdict | Score | Main complaints |
|---|---|---|---|
| 1 | revise | 5.0 | The skull read as a glyph, the head was a façade without a casket body, the theme stopped at the head, flat fields |
| 2 | revise | 5.5 | The skull was still a glyph, the wall garnets could not be seen, the crockets were cookie-cutter plates, the shank was bare |
| 3 | **cut** | 5.5 | The skull was still a glyph (the new teeth read as "two buttons"), crockets were flat plates, the head read as stacked boxes from the side, rear and palm (house rule 10), the shoulder lancets read as letters, the side walls and palm were bare, there were two notches on the rear, and the design was format 6 |

The records are `showcase/tenebrae/capsa/review-round{1,2,3}.json`. The reviewer's strengths in round 3:

- The subject reads at 300 px without the caption.
- The silhouette is distinct within the collection.
- All six cabochons sit in registered bezels.
- The walls view closed round 2's item about the wall garnets not being visible.
- Every export gate checks out against the JSON.

Round 3 also marked a red **template gate**. That red came from a stale record (target/tpl/capsa/verification.json, 445,742 bytes, written before round 3 reduced the design). I reran the gate on the final build and it passes (numbers below). The reviewer's verdict stands regardless.

## Gates on the final build (from report.json)

| Gate | Draft (768 × 320) | Export (1536 × 448) |
|---|---|---|
| Triangles | 471,576 | 1,270,824 (≤ 2 M ✔) |
| Watertight, degenerate faces, self-crossings | ✔, 0, 0 | ✔, 0, 0 |
| 30 made parts watertight, 0 degenerate, 0 crossings | ✔ | ✔ |
| Solids and parts notes empty, every CAD feature Ok | ✔ | ✔ |
| Bore: nearest vertex / vertices inside the 9.3 mm bore | 9.30002 mm / 0 | 9.30001 mm / 0 |
| Thickness census at 0.8 mm, whole mesh: wall zones between 0.05 and 0.8 mm | 0 | 0 |
| Census: suspected artifacts (< 0.05 mm walls) | none | none |
| Census: edge samples (reported as read) | 1,909 samples, 11.41 mm² | 1,909, 11.41 mm² |
| cut_lands at 0.8 mm (west window, back lights 1 and 2) | 2.85 / 3.50 / 3.59 mm, none under | same |
| DFM findings | 0 | 0 |
| Stones: reported equal previewed, warnings, crowding | 6 = 6, none, none (closest pair 1.54 mm) | same |
| Cold reload with --verify identical | n/a at draft | ✔ |
| Ring pattern (mesh::try_build_pattern) | watertight, 0 degenerate, 0 crossings | same |
| Lid pattern (mf::prepare, component = lid, Part(71), 5,910 triangles) | watertight, 0 degenerate, 0 crossings | same |
| Head over the crest | 6.76 mm (limit about 9) | 6.76 mm |
| Volume | 1,446.6 mm³ | 1,446.6 mm³ |

I ran the census under the lead's interim rule:

- Every wall zone between 0.05 and 0.8 mm across a feature I made was fixed.
- Zones under 0.05 mm are listed as suspected artifacts. There are none.
- No hand-made lip or tip exception census is used.
- Edge zones are written to report.json as read. Some edge readings are as thin as 0.008 mm, at bezel lips and chamfer arrises. They are classed as edges, not walls.

**Design file:** 264,809 bytes, 71 CAD features, **format 6**. Format 6 is forced by profiles of several regions. I needed single multi-region cutters (the niches, the skull face and teeth, the tracery) to keep every analytic Boolean operand under the kernel's 500-face limit (`MAX_ANALYTIC_BOOLEAN_FACES`). With one cutter per region, the boolean chain fails or takes minutes. The plan wanted format 5. See core change 1.

**Sand:** not assessed. Logan's ruling judges this ring as lost wax, and the closed chest, the lid cavity and the undercut niches would not pull from sand anyway. No bonus is claimed.

## Template gate (collection_templates, ringdesign-graph), on the final build

- template_gate_passed: **true**. Class procedural, **291,255 bytes** against the 300,000 budget. size_review_required false.
- Nodes: 98. Source method "lift". Design.set patches: 1 (/cad/joints, node 146).
- source_identical: true. vertices_faces_normals_identical: true. export_geometry_verified: true (1536 × 448, 1,270,824 triangles).
- cold_design_reload: true. cold_graph_reload: true. editable_graph_reload: true. detail_findings: 0.
- editable_design_bytes: 579,820.
- Timing: first build 2.44 s, evaluate 7.5 ms, verdict 17.5 ms.
- Record: `showcase/tenebrae/capsa/template-verification.json`.

To get the template under budget (it started at 445 KB), I rounded every coordinate to 1e-4, lofted the skull from 5 sections of 32 points, capped outlines at 72 points, and used fewer chords on the arches.

## The feature tree, as sentences

1. A procedural shank carries the head.
2. A bed block is laid across the band, and its top is chamfered as a moulding. This is the bed, 2.3 mm deep, its slanted edge kept off the band edge.
3. A plinth block stands on the bed, and its top is chamfered as a moulding.
4. A chest block stands on the plinth. Its front edge is chamfered at the lid line while the block is still convex, and then the chest is hollowed with a cavity that is open to the roof.
5. The skull is lofted from its outline into a dome (5 sections). Its teeth are drawn and cut as V-grooves: five teeth and a jaw line, 21° draft, 0.25 mm deep. Its face is drawn with sloped walls: two bowl orbits and a teardrop nasal aperture, 22° draft, 0.53 mm deep. The face is then sunk into the skull. This is the memento mori, a true skull, joined to the chest.
6. Each of the north and south towers is raised in three stepped stages and webbed to the chest. A spire is lofted to its point, and its crockets and fleur finial are cut as one fin.
7. The west window is drawn as single-light tracery and cut over the skull. Two back lights behind the skull are cut through the bed and back wall to the dark, so the relic stands against black.
8. The front's archivolt is raised as a 1.05 mm band, its sides weathered at a 15° draft.
9. One garnet is set in each of the north and south walls in a bezel. Each wall's lancet bay is sunk around its stone, and this finishes the chest.
10. Three cabochons (garnet, sapphire, garnet) are set in bezels in the plinth front. The niches are drawn as one splayed-jamb, multi-region cutter and sunk around them, and this finishes the plinth.
11. Four blind lancets are sunk into each shoulder at 37°, 51°, 65° and 79° off the top. They are 0.5 mm deep and shrink from 1.70 × 2.0 mm to 1.40 × 1.7 mm.
12. The lid block is the roof, its gable up, with the apex blunted. Its edge is chamfered at the lid line. The rose's eye is a sapphire in a bezel.
13. The roof is hollowed to a skin over a 2.3 mm roof plate, open toward the chest. Each slope is opened in tracery roundels cut through.
14. The gable is crested with a moulding that stands proud of it: five crockets per rake and a fleur finial, starting clear of the towers.
15. A roll runs across the gable's foot.
16. The rose's collet is joined to the lid. **The lid is cast apart** (Attach::Separate, Stage::Cast) and hinged to the chest on a bench hinge (`cad::Joint`).

## Parts, stamps and stones, and why

- **The bed, plinth and chest** are the reliquary's carcass. The plinth and bed are mouldings with chamfered tops, the Gothic base-and-socle language. The chest is hollow, so the lid has something to close.
- **The skull** is the relic, a true skull rather than a symbol. It has a domed cranium, bowl orbits, a nasal aperture and a cut tooth row, framed by the west window and backed by open lights, so it reads as something kept inside.
- **The towers and spires** are flanking pinnacles. They give the head its Gothic skyline and its silhouette at 300 px.
- **The archivolt** is the pointed door frame of the shrine.
- **The lid** is the roof. It is a separate casting so the reliquary truly opens, and it carries the crest, the crockets, the finial, the slope roundels and the rose.
- **The shoulder lancets** carry the arcade language down the shank. They are blind, to stay above the 0.8 mm section.
- **The stones:** six oval cabochons, 1.8 × 2.3 mm, all in head.bezel settings (wall 0.9 mm, lip 0.3 mm).
  - Four **garnets**: two in the side-wall bays and two in the outer plinth niches.
  - Two **sapphires**: the rose's eye in the gable and the middle plinth niche.
  - Why: red and blue is the palette of stained glass, and cabochons rather than faceted stones suit both a reliquary and lost wax. The census shows no crowding (closest pair 1.54 mm).
- **Stamps:** none. I used no Textura or marks stamp.

## What I could not do

- **Modelled leaf crockets.** The crockets are traced fins, and a drafted fin produced census walls, so I reverted it. The reviewer called them "cookie-cutter plates".
- **A carved skull with brow and cheekbones.** The analytic kernel cannot take a richer loft through the face cut under the 500-face limit. Anything finer than the sockets, nasal aperture and 0.25 mm teeth goes under the 0.8 mm floor or past the face limit.
- **Bed arcades.** They cost 9.5 minutes of kernel time and produced self-crossings, so I dropped them.
- **Cusping in the tracery, and a petalled rose.** The crest's triangle filled in the gable, and the petals vanished in every render.
- **The roof slopes as seen from the hero view.** The slope roundels show in walls.png but barely in the hero.
- **Ornament on the side walls and palm, and a head that does not read as stacked boxes from behind.** Not reached in three rounds.
- **Format 5.** Blocked by the multi-region profiles (see core change 1).
- **Two rear notches.** The back lights exit through the rear wall by design, to darken the skull's ground. The reviewer read them as stray notches.

## Enablers and master changes used

- Master was merged before building: 2e11632, b03a21d and 803a93e.
- Used: #248 (`render::write_png_framed` for the close-ups), #259 (CAD fallbacks, through csg resolution of refused booleans), and #261 / #262 (the whole-mesh thickness census, under the lead's interim rule).
- Merged but not used: #260 crisp lift.
- Not used: C-B2, C-V1..5, C-T5..7.

## Core changes wanted (exact code)

**1. `crates/ringdesign-core/src/cad.rs`: send an oversized Boolean operand to csg instead of refusing it.** Right now a faceted operand with more than 500 faces is a hard error. That forced me into multi-region cutters (and so format 6), dropped the bed arcades, and limited the skull. Replace the `ensure!` loop in `Operation::Boolean` (the `for id in [a, b]` block that checks `MAX_ANALYTIC_BOOLEAN_FACES`) with:

```rust
            let mut oversized = false;
            for id in [a, b] {
                oversized |= source(id)?.faces.len() > MAX_ANALYTIC_BOOLEAN_FACES;
            }
            if oversized {
                // Past the analytic kernel's budget, csg resolves on the tessellated operands, as a refused combine does.
                let op = match kind {
                    Boolean::Union => crate::csg::Op::Union,
                    Boolean::Subtract => crate::csg::Op::Subtract,
                    Boolean::Intersect => crate::csg::Op::Intersect,
                };
                let named = builders::combined(&named_of(va)?, &named_of(vb)?, op)
                    .with_context(|| format!("{} of {} and {}", op_label(*kind), who(*a), who(*b)))?;
                return Ok(Value::Mesh(Arc::new(builders::Made::of(op_label(*kind), named, None)?)));
            }
```

**2. A render-turn helper in `crates/ringdesign-core/src/render.rs`**, so a ring's example can render a view turned about the band axis without hand-writing camera vectors:

```rust
/// A view direction turned `deg` about the band axis (x) from `dir`.
pub fn turned(dir: [f32; 3], deg: f32) -> [f32; 3] {
    let (s, c) = deg.to_radians().sin_cos();
    [dir[0], dir[1] * c - dir[2] * s, dir[1] * s + dir[2] * c]
}
```

## Reproduce

```sh
cargo build --release -p ringdesign-core --example tenebrae_capsa
# The final ring: draft and export gates, renders and report.json.
target/release/examples/tenebrae_capsa showcase/tenebrae/capsa --verify
# One option: across, openwork or across-openwork, written into its subfolder.
target/release/examples/tenebrae_capsa showcase/tenebrae/capsa --option across
```

The only code is `crates/ringdesign-core/examples/tenebrae_capsa.rs`. `src/` was not touched.
