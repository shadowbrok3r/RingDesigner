# Tenebrae — Gurgulio, the waterspout: cut at 6.6

Branch `claude/tenebrae-gurgulio`, cut from master `29babc4`. Master was merged at the start of each round: `0c7c8c4` (#261, wall census) and `b03a21d` (#263) in round 2, and `803a93e` (#262) in round 3. Author file: `crates/ringdesign-core/examples/tenebrae_gurgulio.rs`. Outputs: `showcase/tenebrae/gurgulio/`. The ring's section, "Gurgulio — *the waterspout*", is new in `docs/collections/tenebrae.md`. Gurgulio takes Oculus's row in the table there, and the section now records the cut.

**Verdict: cut at 6.6.** Every gate is green at draft and export. The ring used both block-out read tests it needed (of three allowed) and all three review rounds. No extra round was granted, and the doc records none.

## Read tests and reviews

| Step | Verdict | Score | What the reviewer saw |
|---|---|---|---|
| Read test 1 | does not read | — | "A dragon figurine on a pedestal ring". The body was a tube-and-knob armature (wire, toy robot), the plinth read as a lettered plaque, and the face view was unreadable. |
| Read test 2 | **reads** | — | "Gargoyle, Gothic… passes, but only just on the architecture." The hero showed a snarling beast with paws over a ledge. The face view showed a head thrust out past a block with a spout down the finger, "the clearest waterspout read". |
| Round 1 | revise | 6.2 | The subject reads. Two problems: the part's own sections and a dense thickness sampling were not gates, and the sculpt was a "cartoon bulldog" with decimation shards. Also: the perch was a box, the wings a fan, the shoulders were "ziggurat" step blocks, and the band was bare from shoulder to palm. |
| Round 2 | revise | 6.5 | Every gate green, the draft cold reload fixed, the field notes fixed. The theme now runs face to palm (the side-wall arcade), and the bored spout shows in the hero. Still: the bulldog sculpt, the decimation flecks, the wings as chevrons, the perch as a box, no visible gutter, and the arcade reading as "gear teeth". |
| Round 3 | **cut** | 6.6 | Every gate green. The gutter and its rolls run over the palm, and the paws grip the coping with separate digits. Still: inflated capsules, not carved stone; flecks and creases in the close-up; the wings read as slabs from above; the perch still reads as a box; the hero reads as "a cluttered block or robot head". |

Full verdicts: `read-test-1.json`, `read-test-2.json` and `review-round{1,2,3}.json`.

## What the ring is

- **Base:** a procedural `Flat` band, 4.6 × 2.4 mm at the palm, keyed (`ShankKind::Keyframes`) to 1.85× width and 1.05× thickness over the crown (θ 66–118°), bore 18.6 mm (US 8.6). I chose this over a factory signet because the perch *is* the head here: it is sculpted with the beast as one part, so the band only has to rise into the corbel's foot. A factory table under a figure is what read as a "hood ornament" in the Ogiva spike.
- **The part:** one sculpted stored mesh, "Grow the gargoyle and its parapet out of the band" (`sculpt.rs`: `tetra_mesh` at 0.055 mm, then the largest shell kept, then `decimate` to 300k at a cost cap of 0.003, then `settle`, giving 189,134 faces and 1,804 mm³). It is joined with `fillet_into_band` 0.7 mm. It contains:
  - **The parapet.** A corbel tapering out of the band into the wall, with chamfered upright edges.
  - **The wall.** Three blind pointed lancets on the front and on the back, and one blind quatrefoil (lobes run together on a 0.16 round) on each side, all sunk 0.6 mm.
  - **The cornice.** Two steps round all four sides. Each step overhangs 0.85 / 0.8 mm through a cavetto, keeps 0.82 mm of stone over its hollow, and the coping's arris is chamfered.
  - **The pinnacles.** Two at the back corners: a chamfered shaft, gablets, a spire and a knob finial.
  - **The gargoyle,** 1.9× in its frame, facing the fingertip. It has a heavy crouched body with a row of seven dorsal knobs. Its forelegs are braced to the coping, with wrists and three claws per paw gripping the coping's front edge, sunk into it. The hind legs are folded high, and the tail is wrapped into a foot.
  - **The wings.** Two folded bat wings ride the back like a cape. Each is a membrane slab with an arm bone, four finger bones lying 0.32 mm proud, a wrist claw, and a trailing edge scalloped between the fingers.
  - **The head,** 1.6× and raised 38°. It is cut in planes: a brow shelf over deep sockets with the eyes set back, cheekbones, a snout block with a snarl crease and two furrows each side, a dropped jaw, conical horns and ears, and two fangs. A lead pipe lies out of the jaws 12.5° above the finger's line, its end cut square with a rounded lip, and bored 0.73 mm.
- **The band's ornament** (tiling layers, `crisp_relief` on):
  - "Nave arcade": 33 bays of pointed lancets on both side walls under a string course, the ribs 0.95 mm and the course standing 0.4 mm.
  - "Arcade fields": each bay's field sunk 0.25 mm.
  - "Gutter rolls" and "Lead gutter": a 0.9 × 0.4 mm gutter sunk along the crown's centre line all round, between two 0.8 mm roll beads. It is the source of the spout's water.
- **Stones:** none.
- **Process:** lost wax, Silver 925, `CastProcess::LostWax` with `min_section_mm` raised to 0.8. The decision is recorded in the ring's section.
- **Sand bonus:** none. The two-part undercut is 7.9% on the band and 18.2% with the part, so it does not pull from sand.

**The feature tree, as sentences.**
1. "Procedural shank": the keyed band.
2. "Grow the gargoyle and its parapet out of the band": the stored sculpt, joined with a 0.7 mm fillet into the band.

The field layers, in stack order: the nave arcade stands on both side walls; the gutter's roll beads stand on the crown; each bay's field is sunk; the lead gutter is sunk last, so no `Max` fills a carving back in.

## Gates (final build, from `report.json`)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Watertight, 0 degenerate, 0 self-crossings | 651,252 tris; true / 0 / 0 | 1,482,978 tris; true / 0 / 0 |
| Sculpt closed and uncrossed, as made and as placed | 0 open edges, 0 crossings | same |
| Solids/parts notes empty, every feature `Ok`, the part joined | pass | pass |
| Nothing in the finger hole | nearest 9.30001 of 9.3 mm, 0 inside | nearest 9.29999 mm, 0 inside |
| Lost-wax field verdict | Castable (band and with the part), thinnest wall 1.99 mm | same |
| Wall census `cad::measure::thickness(&built.mesh, 0.8)` | assessed, 342,691 samples, 0 unresolved, **0 wall**; 1,596 edge samples (9.37 mm²) read as reported | assessed, 345,401 samples, 0 unresolved, **0 wall**; 1,761 edge samples (10.21 mm²); `clean()` true |
| Suspected census artifacts (zones under 0.05 mm) | none | none |
| `dfm::cut_lands` at 0.8 | clean | clean |
| `dfm::findings_in` | 0 | 0 |
| Stones reported = preview | 0 = 0 | 0 = 0 |
| Casting pattern | watertight, 0 / 0 | watertight, 0 / 0 |
| Within 2M triangles | yes | yes |
| Cold reload, empty library | identical | identical |

The wall gate follows the lead's notes in order:
- **#261:** the census on the finished ring replaced my 384-ray pass and my 20k-ray hand census. Both were removed.
- **The interim correction:** no wall zone that is a real section of 0.05–0.8 mm, and zones under 0.05 mm listed as artifacts. The final build reads 0 wall samples, so it also passes the strict `clean()`.
- **`dfm::part_sections`** on the part alone (its buried foot included) is reported, not gated: thinnest 0.0001 mm, 6.67 mm² under 0.8.

**Template gate**, after the last round, on the final build:
- Class `painted`.
- 0 `design.set` patches.
- **2,063,830 B** against the 3 MB budget (39 nodes).
- Cold source identical, and vertices, faces and normals identical at 1,482,978 triangles.
- Cold design and graph reloads true; first open 9.3 s.

As `procedural` the graph is over its 300 KB budget (1.56 MB in round 1). The stored sculpt is most of the graph, as with Moloch, which ran `painted`. `crisp_relief` is on, and the lift carries it since #260. Numbers are in `template-gate.json`.

## Master enablers used

- **#248 (crisp edges):** I used `render::write_png_framed` for the close-up (`stones.png`) and `crisp_relief` for the band's arcade and gutter.
- **#261 (wall census):** the lost-wax wall gate. It found real walls my own checks had missed: blend webs, the coping lip, the pipe wall and the cavetto shelf. All were fixed.
- **Not used:**
  - C-B2 / #257 true stone plans: there are no stones.
  - C-T5/C-T6 (#255) Textura and marks: there is no lettering, and the part is cast with its default mark.
  - C-V1–V5 / #258 patterns along a path, C-T7 sweeps and #259 CAD fallbacks: the perch and figure are one sculpt, not CAD features.

## What I could not do

- **Carve the figure as stone.** All three reviewers read an inflated cartoon or "vinyl" grotesque. Building a stone-carved face from smooth SDF primitives (ellipsoids, capsules, rounded boxes, plane cuts) did not get there in three rounds. The round-3 attempt at planes read as "blocky / robot" at 300 px. A head built as a CAD loft of carved sections, or a sculpted asset, would be the honest route.
- **Reach the reviewers' decimation target.** They asked twice for ≥450k faces at 0.003. At ~11 B per face the design would pass the 3 MB painted budget, so the template gate would fail. I raised the budget to 300k at 0.003 (189k after settle) and eased the figure's shading normals in the renders only. Flecks remain at the creases.
- **Wings that read as bat wings from above.** They read as a fan in round 1, chevrons in round 2 and slabs in round 3.
- **The perch's cornice at 300 px.** The two-step cavetto cornice is in the geometry, but the beast covers most of it from the hero camera, and the final reviewer still read "a box".
- **The arcade at 300 px.** Its relief has to stay at least 0.8 mm across every rib, measured all the way to the bore edge. At 2 mm pitch on a ~2.2 mm wall that leaves small lancets, which read as "cog teeth".
- **A floating fragment.** The meshing's shells left one small positive-volume piece (0.77 mm³) apart from the body, and keeping the largest shell dropped it. The figure as built is therefore one solid, but one small feature (likely an eye ball or a claw tip) may be missing from that spot.

## Core changes wanted

1. **`sculpt::tetra_mesh` closes sealed voids inside a thick body.** On this figure it made inner shells of 36.5 mm³, 3.2 mm³ and smaller deep in the torso, where the field reads solid (probe: field −3.4 mm). These are blocks the coarse pass skipped, and they cast as voids. Until the skipped blocks take the coarse sample's sign, a public helper lets callers drop them. This is the example's own, moved as is into `sculpt.rs`:

```rust
/// The mesh's largest connected shell, and the others dropped (their face counts and enclosed volumes, mm³).
pub fn largest_shell(m: &Solid) -> (Solid, Vec<(usize, f64)>) {
    let n = m.v.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for t in &m.f {
        for k in 0..2 {
            let (a, b) = (find(&mut parent, t[k] as usize), find(&mut parent, t[k + 1] as usize));
            if a != b {
                parent[a] = b;
            }
        }
    }
    let root: Vec<usize> = (0..n).map(|i| find(&mut parent, i)).collect();
    let mut faces: std::collections::BTreeMap<usize, (usize, f64)> = Default::default();
    for t in &m.f {
        let [a, b, c] = t.map(|i| m.v[i as usize]);
        let vol = (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0]) + a[2] * (b[0] * c[1] - b[1] * c[0])) / 6.0;
        let e = faces.entry(root[t[0] as usize]).or_default();
        e.0 += 1;
        e.1 += vol;
    }
    let keep = faces.iter().max_by_key(|(_, (f, _))| *f).map(|(r, _)| *r).unwrap_or(0);
    let dropped = faces.iter().filter(|(r, _)| **r != keep).map(|(_, v)| *v).collect();
    let mut index = vec![u32::MAX; n];
    let mut v = Vec::new();
    for i in 0..n {
        if root[i] == keep {
            index[i] = v.len() as u32;
            v.push(m.v[i]);
        }
    }
    let f = m.f.iter().filter(|t| root[t[0] as usize] == keep).map(|t| t.map(|i| index[i as usize])).collect();
    (Solid { v, f }, dropped)
}
```

   The root fix: in `tetra_mesh`, a block the coarse pass skips should be filled with the sign of its coarse sample, rather than read as outside.

2. **`sculpt::relax_clean` is unusable on fields with chamfered or slab features.** Here it put back 691k vertices in 279 s. A cheap early-out would help: skip the relax when a first pass's crossing count exceeds a share of the vertices, and say so in the return value.

3. **A template class for stored sculpts.** `painted` (3 MB) is used by precedent (Moloch, and now Gurgulio), but the README names it for painted relief. Name a `sculpt` class with the 3 MB budget in `collection_templates`, so the next sculpted ring needn't argue the point.

## Rounds used

Two block-out read tests (no, then yes) and three review rounds (6.2, 6.5, 6.6): cut. Every commit on this branch is pushed to `claude/tenebrae-gurgulio`. No pushes to master or any other branch, and no tags.
