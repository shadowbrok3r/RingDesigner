# Tenebrae: Capsa, the reliquary — cloud report

**Outcome: stopped at the block-out.** All three read tests came back `reads: false`, so under the loop's rule the ring went no further. The subject needs rethinking before anyone spends more time on detail. No detail round ran, so there is no review score, no ship and no cut verdict.

- Branch: `claude/tenebrae-capsa`, from `master` at `8e5a59a`. Master did not move during the session (checked with `git fetch origin master` before writing this), so nothing was merged and no enabler (C-B2, C-V1..C-V5, C-T5..C-T7) was used.
- Read tests used: **3 of 3**, none read. Review rounds used: **0 of 3**.
- Files: `crates/ringdesign-core/examples/tenebrae_capsa.rs` and `showcase/tenebrae/capsa/`. The showcase folder holds the draft renders, `hero-300.png`, `face-300.png`, `design.ring.json`, `report.json` and `read-test-{1,2,3}.json`.
- Reproduce: `cargo build --release -p ringdesign-core --example tenebrae_capsa && target/release/examples/tenebrae_capsa --draft`.

## Read tests (block-out)

| # | reads | What the reviewer saw at 300 px | What I changed next |
|---|---|---|---|
| 1 | false | Hero: "a squat metal cube… studded with thin pins at its corners like rivets or antennae". Face: "a folded envelope or a low pyramid lid". Would say "box ring", or "casket ring" if pressed. | Dropped the cross-gable (it made the X crease). Steep single-ridge gable at 44.6° (rise 3.9 on a 7.9 span), eaves overhanging 0.55 with a 0.2 chamfer. Five fleurs along the ridge and two turned finials; the four corner pinnacles removed. Chest narrowed to 6.8 and walls thickened to 1.3. Three pointed bays 0.5 deep with weathered buttresses on the posts. Pointed portal with a raised 0.8 archivolt. Two-step chamfered plinth. Graded quatrefoils (1.6 / 1.3 / 1.0) pierced through both shoulders. |
| 2 | false | Hero: "a comb, a harmonica or a music-box movement… nothing in the outline is pointed", because "the camera looks down the roof". Face: "a domino, a belt buckle or a circuit chip". The side view "does read as a little gabled chapel", but the judged views never show it. | Re-aimed the hero to a 3/4 view off the gable end, as the reviewer asked: the ring is turned 0.6 rad about the head's axis, pitch 0.38. Arcade rebuilt with two recessed orders (0.2 and 0.5 deep, inset 0.4); centre bay widened to 3.9 so the garnet sits inside it. Sharper portal (arc share 0.9). Sapphires moved into pointed lancet windows sunk into the lid's gables. True fleur-de-lis cresting: a vesica petal, two leaning side petals and a band. Larger finial knops. Slope ornament removed. |
| 3 | false | Hero: "a big step up… the head reads as a building", "a house ring or a little church/chapel ring", and one who knows medieval metalwork "might say 'chasse' or 'reliquary'". But "it does not say Gothic": the pitch reads low ("a classical pediment or a cottage roof"), the openings read round, the cresting reads as "castle battlements", and the finials read as balls. Face: "unchanged in kind… a domino, a buckle or a chip". | Stopped, per the loop. |

The full JSON for each test is in `showcase/tenebrae/capsa/read-test-N.json`.

### Why the subject needs rethinking, not detailing

- **The face view of a gabled chasse lying along the finger is a rectangle with a line down it.** Looking straight down, both slopes catch the same light, and the cresting seen from above is a strip ("ladder", "zipper"). Nothing I put on the slopes changed that in three attempts: the roundels, quatrefoils and trefoils all read as "chevron nicks".
- **The Gothic signs do not survive 300 px at ring scale.** The arches really are pointed: two-centred arcs with shares 0.62 to 0.9. Even so, at 300 px a 2.6–3.9 mm arch shows only its shadowed lower part and "reads round". The 0.8 mm section floor sets the minimum stroke for fleurs, archivolts and buttresses, and at this scale that makes them chunky ("battlements").
- **The reviewers' own fixes conflict with wearability.** Attempt 3 asks for a ridge ≥ 5.0 mm over the eaves, finials 2.5–3.0 mm tall and four corner pinnacles rising past the eaves. Attempt 1 had asked for those pinnacles to be removed. The block-out already stands about **8.8 mm over the crest at the ridge and about 11 mm at the finial tips**, and is **≈ 1460 mm³ of metal (≈ 23 g in 18k)**. The requested roof would put it near 13 mm.
- **Directions for a rethink:**
  1. Stand the shrine **across** the finger, gable facing the face camera. The face view then shows the pointed gable, window and portal, the "chapel" read that read tests 2 and 3 found only off the gable end.
  2. Make the head a **tower or monstrance reliquary**: a micro-architecture with a spire. Its plan from above is a star of gablets and pinnacles. This overlaps Lanterna, so it needs Logan's call.
  3. Keep the coffin-ring chasse, but make the roof **openwork tracery with the skull visible through it**. The face view would then carry the memento mori that Logan chose.

## The block-out as built (draft, 768 × 320)

**Feature tree, as sentences (62 features).**
- **Plinth and step:** the plinth's base course (14.4 × 8.4 × 2.0, top 0.15 over the crest) is chamfered 0.35 as a moulding, and the step on it (13.8 × 7.8 × 0.6) is chamfered 0.25.
- **Chest:** the chest block (13.0 × 6.8 × 3.8) is hollowed from the top with a native `Shell` (1.35 walls), and its relic floor is pressed up 0.3 with `PressPull`. The chest is a kernel body joined to the band.
- **Arcades:** each long wall gets its arcade's outer and then inner order: three two-centred pointed bays drawn as true arcs, sunk 0.2 and then 0.5 as `Cut` parts. Four buttresses stand against each long wall on the posts, each with a foot, a weathering and an upper stage.
- **Portals:** each end wall gets a pointed portal sunk 0.5 under a raised 0.8 archivolt.
- **Chest stones:** a garnet cabochon sits in each long wall's centre bay, in a bezel (wall 0.35, lip 0.3).
- **Shoulders:** quatrefoils of 1.6, 1.3 and 1.0 mm are pierced through both shoulders at 48°, 60° and 72° off the top (`cutters::pierce_at`, Shape `Quatrefoil`).
- **Lid body:** the gable is drawn at the west end and run 13.6, past both ends. Its eaves are chamfered 0.2 with a native `Chamfer` while it is still B-rep.
- **Lid stones:** a garnet sits on each slope and a sapphire on each gable, all seated on the B-rep faces with `stone_on_face`. The slope bezels are joined, a pointed lancet window (2.8 wide, 3.3 tall, 0.45 deep) is cut into each gable round its sapphire, and then the window bezels are joined.
- **Cresting and finials:** the cresting (a rail and five fleurs, traced from a distance field into one closed sketch) is cut out and joined to the ridge. A finial is turned for each end of the ridge and set on it.
- **Two castings:** the lid is a `Separate` cast part, linked to the chest by `Joint { chest, lid, 0.05, "Three-knuckle hinge, 0.8 mm pin, soldered at the bench" }`.

**Stones:** six oval cabochons, 1.8 × 2.3: four garnets (tint 0.30, 0.02, 0.03) and two sapphires (0.02, 0.06, 0.45), each in a `head.bezel`. A cabochon takes no bur.

**The block-out's own draft numbers.** These are not the round gates, which never ran; see `report.json`.

| Check | Value |
|---|---|
| Finished mesh | watertight, 0 degenerate faces, 0 self-crossings, 512 394 triangles |
| Every CAD feature | `Ok`; solids notes and parts notes empty |
| Made parts | 0 self-crossings each. The lid has 4 sliver faces (area < 1e-10 mm²) where the finials were unioned. They are cleaned out of the finished mesh, but would need fixing before gate 7 and the lid's casting pattern. |
| Bore | nearest vertex 9.29994 mm against r 9.3; 0 vertices inside |
| Stones | 6 reference stones in the record |
| Design file | 232 KB at format 6: the in-plane `Revolve` of the finials fences it at 6, not the 5 the plan wanted |

**Not run, because no round started:**
- the gate set: the `thickness(0.8)` fill, `dfm::cut_lands` and `dfm::findings_in`
- the export build with `--verify`
- the casting patterns for the ring and the lid
- `lid-open.png`
- the skull relief on the relic floor
- the **template gate**, which TASK.md runs after the last round; there was none

## What I could not do, and why

- **Kernel booleans on decoration.** A `Boolean::Subtract` of an extrude drawn with true arcs fails with `CutRefused`. One drawn as a polyline succeeds but facets the chest past `MAX_ANALYTIC_BOOLEAN_FACES`: it reached 608 faces after two bays. So the arcades, portals and piercings are `Attach::Cut` parts, and the lid's ornament goes through csg after its first bezel union. The B-rep stays live through the `Shell`, `PressPull` and `Chamfer` steps and for every stone seat, as house rule 3 asks.
- **`Revolve` of a profile that closes on the axis in a point** fails at tessellation ("Kernel could not tessellate 1 faces"). Finials end in a flat 0.42 mm cap instead.
- **The studio camera's yaw turns the ring about the finger's axis**, not the head's, so a 3/4 view off a gable end rolls the head sideways. The example turns the meshes about the head's axis before rendering.
- **The stones conflict with the architecture.** A 1.8 × 2.3 cabochon in a 0.35 bezel needs about 2.5 × 3.0 of opening. That set the centre bay at 3.9 wide and moved the sapphires from the portals into the gable windows.

## Core changes wanted (exact code; not compiled here, since ring lanes do not edit `src/`)

1. A render entry that turns the ring about its head before the camera's yaw and pitch, so a collection can take gable-end and three-quarter heroes without rolling the head (`render.rs`, beside `write_png_parts`):

```rust
/// `parts` turned `turn` radians about the ring's top (world y) before the camera's yaw about the finger and its pitch.
pub fn write_png_parts_turned(path: impl AsRef<Path>, parts: &[Part], turn: f64, yaw: f64, pitch: f64, edge: usize) -> anyhow::Result<()> {
    let (s, c) = (turn.sin() as f32, turn.cos() as f32);
    let r = |v: crate::mesh::Vec3| crate::mesh::Vec3(v.0 * c + v.2 * s, v.1, -v.0 * s + v.2 * c);
    let meshes: Vec<Mesh> = parts
        .iter()
        .map(|p| {
            let mut m = p.mesh.clone();
            m.vertices.iter_mut().for_each(|v| *v = r(*v));
            m.normals.iter_mut().for_each(|v| *v = r(*v));
            m.corner_normals.iter_mut().for_each(|(_, n)| *n = n.map(r));
            m
        })
        .collect();
    let turned: Vec<Part> = parts.iter().zip(&meshes).map(|(p, m)| Part { mesh: m, ..*p }).collect();
    write_png_parts(path, &turned, yaw, pitch, edge)
}
```

2. `cad.rs`, the `Operation::Boolean` arm: when the analytic kernel refuses two kernel bodies (too many faces, or `CutRefused` on arcs), take the same boolean through csg, the way a mesh operand already does. Replace the `for id in [a, b] { … }` check and the `brep::combine(…).map_err(…)` that follows with:

```rust
            let kernel = [a, b].into_iter().all(|id| source(id).is_ok_and(|body| body.faces.len() <= MAX_ANALYTIC_BOOLEAN_FACES));
            let op = match kind {
                Boolean::Union => brep::Operation::Union,
                Boolean::Subtract => brep::Operation::Difference,
                Boolean::Intersect => brep::Operation::Intersection,
            };
            if kernel {
                if let Ok(body) = brep::combine(source(a)?.clone(), source(b)?.clone(), op, 1e-6) {
                    return Ok(Value::Brep(body));
                }
            }
            // The analytic kernel refused: the same boolean through csg, every face keeping the patch it came from.
            let csg_op = match kind {
                Boolean::Union => crate::csg::Op::Union,
                Boolean::Subtract => crate::csg::Op::Subtract,
                Boolean::Intersect => crate::csg::Op::Intersect,
            };
            let named = builders::combined(&named_of(va)?, &named_of(vb)?, csg_op).with_context(|| format!("{} of {} and {}", op_label(*kind), who(*a), who(*b)))?;
            return Ok(Value::Mesh(Arc::new(builders::Made::of(op_label(*kind), named, None)?)));
```

   `named_of` already tessellates a B-rep operand at 0.015 mm. A body the kernel builds returns early as `Value::Brep` and is validated by `build_feature` as before.
