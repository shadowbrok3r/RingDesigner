# Officina ring: Keystone (`keystone`): cloud report

**Outcome: cut after round 3, at 6.8 (6.6, 6.9, 6.8). Every gate is green at draft and at export.**
The cartouche reads at 300 px. Three rounds did not get the foot, the rim or the bright-cut to the Caiman bar.

- **Branch:** `claude/officina-keystone`, from `master` at `8e5a59a`. Master did not move during the session (fetched before every round), so no merge was needed.
- **Enablers used:** none. C-B2, C-V1 to C-V5 and C-T5 to C-T7 never reached master while I worked.
- **Rounds used:** 2 block-out attempts, then 3 review rounds.
- **Checkout:** the work was done in `/home/user/RingDesigner`, the session's clone. `/home/user/repo` never existed in this environment. Same remote, same branch.
- **Interruption:** the session stopped at the account limit at 03:15 UTC, while the round-3 reviewer was finishing. Its `review-round3.json` was complete on disk (verdict, score, all 13 images viewed). I committed it as it stood and did not re-run the review.

## Read tests and reviews

| Step | Result | What the reviewer saw |
| --- | --- | --- |
| Read test 1 | **reads: false** | A plain band with a small octagonal tag stuck on. The single cut triangle read as an arrow. |
| Read test 2 | **reads: true** | "A cartouche (or plaque) band". The tablet takes about 85% of the width, with round corners and a visible drafted skirt. |
| Round 1 | **revise, 6.6** | Gates green. A washer at the foot, a hex-nut profile from the 0.45 mm chamfer on a 7° wall, combing at the corners, sharp band edges. |
| Round 2 | **revise, 6.9** | Band edges, facet tone and verdict explanation fixed. Washer still there; fan shading on the table; the facet read as a pyramid stud. |
| Round 3 | **cut, 6.8** | Fans gone, rim a fine line. Washer and pocket still there. The four-quarter facet reads as an arrow or envelope in the hero. 11 features is too long, and the timeline's facet frames are faint. |

The verdict stands. Files: `showcase/officina/keystone/read-test-{1,2}.json` and `review-round{1,2,3}.json`.

## Why Keystone stands at "castable with care" (its lesson)

The verdict is the lesson, as the section says. The measurements show it has two sources, and the section's text names only one.

- **The band.** Pulled along the finger in two-part Delft sand, a flat band's broad crest stands square to the pull. The bare Flat 6.0 × 2.0 already reads "castable with care" at 30.2% drag, against the 12% that `castability::DRAG_FRACTION` allows. That is before any feature.
- **The cartouche.** It adds its own zero-draft faces: the flat top, and the two end walls that face round the ring, like a signet's table. Its 12° draft only helps the two long walls that face across the finger. 27.4 of its 60.0 mm² stand vertical, taking the ring to 31.2% drag.
- **Release.** It still releases: 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm, at draft and at export. It drags and wants a longer rap.
- **The cartouche alone, on a domed band.** `report.json` `verdict_cause` computes this live: the same history on Rivet's LowDome 6.0 × 2.1 turns a Castable band (4.4% drag) into "castable with care" (7.0%). The part judgment flips it, not the drag.
- **No draft angle fixes it.** Only draft on the top and end walls (a dome, or ends leaning round the ring), or another pull, would make it Castable.
- **The residual undercut.** The cartouche carries 0.015 mm² judged undercut, worst −0.75°. That is tessellation on the faceted chamfer corners, below the 0.075 mm ray grid. It grows with the foot fillet: a 0.5 mm fillet reached −1.16°, 0.65 mm reached −7.5°, and 0.8 mm reached −75° where it ran off the band. That is why the fillet stayed at 0.45 mm.

A lesson that wants the cartouche alone to carry the verdict should stand on a domed band. That is a decision for the lead.

## Gates

| Gate | Draft 768 × 320 | Export 1536 × 448 |
| --- | --- | --- |
| Triangles | 488,928 (1.7 s) | 1,344,544 (4.7 s), within 2 M |
| Watertight; boundary / non-manifold edges | yes; 0 / 0 | yes; 0 / 0 |
| Degenerate faces | 0 | 0 |
| `csg::self_crossings`, ring and every made part | 0, all 0 | 0, all 0 |
| `built.solids.notes` / `built.parts.notes` | empty / empty | empty / empty |
| CAD features `Ok` | 11 of 11 | 11 of 11 |
| Inside the finger hole (bore r 9.1, −0.01 tolerance) | 0 vertices, closest 9.1000 mm | 0 vertices, closest 9.1000 mm |
| `judged_field_report` | Castable with care, 31.2% drag (explained above) | Castable with care, 31.2% drag |
| Ray release at 0.100 / 0.075 mm (obstructions, unresolved) | 0, 0 / 0, 0 | 0, 0 / 0, 0 |
| `dfm::findings_in` | 0 | 0 |
| Stones reported / previewed | 0 / 0 (no stones) | 0 / 0 |
| Casting pattern (`try_build_pattern`) | watertight, 0 degenerate, 0 crossings | watertight, 0 degenerate, 0 crossings |
| `--verify` cold reload, empty library | not run at draft | identical vertices, faces and normals |
| Lost-wax checks, recorded only (Keystone is sand) | thickness 0 below 0.8 mm; `cut_lands` 1 | thickness 0 below 0.8 mm; `cut_lands` 1 |

The one `cut_lands` entry is the bright-cut's four mirrored quarters meeting edge to edge: "0.00 mm between light 1 and copy 2's light 1". That is intended, since they are one pyramid. It is a bench cut, and `cut_lands` gates lost wax only.

**Template gate**
- 1 `design.set` patch (`/manufacturing`), within the limit of 4.
- 127,143 bytes against the 300 KB procedural budget.
- Cold source parity, mesh parity and cold graph reload all true, with export geometry verified (1,344,544 identical triangles).
- 18 graph nodes.
- The 11 `cad.feature` nodes are in timeline order: Band, Work plane over the top, Cartouche sketch, Drafted boss, Press-pull the top, Chamfer the rim, Leaning quarter sketch on the top, Bright-cut quarter, Mirror across the band, Mirror round the ring, Mirror the mirror round the ring.
- Recorded in `report.json` under `template_gate`.

## Feature tree

1. **Band.** The plain Flat 6.0 × 2.0 band with squared sides and 0.35 mm broken arrises, bore 18.2 mm, Delft clay set with `SandProcess::DelftClay.apply` and `CastProcess::SandTwoPart.apply`. The reader learns that the band is the anchor every part stands on.
2. **Work plane over the top.** A plane tangent to the band at θ 90°, sunk 0.7 mm so the boss stays joined where the band curves away (sag 0.55 mm over the 3.5 mm half-length). The reader learns that a plane is placed on the ring, not in space.
3. **Cartouche sketch.** A 7.0 × 4.8 mm rounded rectangle on that plane, each r 1.0 corner drawn as 24 straight lines. The reader learns to draw on a work plane.
4. **Drafted boss.** The sketch extruded 1.6 mm with 12° draft, joined with a 0.45 mm seam fillet. The reader learns draft, and what draft cannot do for walls square to the pull.
5. **Press-pull the top.** The boss's planar top moved out 0.8 mm (1.7 mm proud of the crest). The reader learns press-pull of a face picked by signature.
6. **Chamfer the rim.** The native fillet is tried at 0.35, 0.30, 0.25, 0.20 and 0.15 mm and refused at each. The section's fallback, a 0.18 mm chamfer on the 100 rim edges picked by signature, builds instead. The reader learns edges named by signature, and the fallback when the kernel refuses.
7. **Leaning quarter sketch on the top.** A triangle on a plane laid on the cartouche's top through the lozenge's outer edge and its centre, 0.45 mm under the top. The reader learns a sketch on a face, with its plane leaned.
8. **Bright-cut quarter.** That triangle cut away above its plane, at the bench. The reader learns a cut whose floor is a sloped plane, and the Bench stage: the sand pattern leaves it out and keeps drill marks.
9. **Mirror across the band.** The quarter reflected across the band's mid-plane. The reader learns mirroring a cut.
10. **Mirror round the ring.** The quarter reflected across the section at 90°. The reader learns a section mirror.
11. **Mirror the mirror round the ring.** The fourth quarter. It is two single-source mirrors, not one of two sources, so the file stays at format 5. The reader learns why a pattern of several parts is fenced.

The section planned 8 features. The bright-cut grew to 5 (features 7 to 11) chasing the reviewers' facet read, and round 3 judged that too long. Round 3's own punch list returns to the section's single lozenge with one sloped cut.

## Parts and stones

- **Parts:**
  - The band.
  - One joined cartouche: features 4 to 6, the boss, its press-pull and its rim.
  - Four bench cuts, which are the bright-cut. They are shown finished, and the sand pattern leaves them out with raised drill-start marks.
- **Stones:** none, by design.
- **Alloy:** Gold 14k, Delft clay recipe, gated at the palm in a 70 mm flask.

## What I could not do

- **Fillet the rim.** `Fillet` refuses the rim loop at every radius tried: `AdjacentSelections` on an 8-edge loop, and `RadiusTooLargeOrInteracting` on the 52- and 100-edge loops. The section's Chamfer fallback is in place.
- **Draw the cartouche's corners as arcs.** A drafted lines-and-arcs loop refuses to extrude along its normal, though it builds against it. Once drafted, the arcs are cones, which press-pull, chamfer and fillet all refuse. The corners are 24-segment polylines.
- **Get rid of the foot "washer".** All three reviews saw it: the 0.45 mm fillet catches light against the shaded 12° wall. A larger fillet that would read as a run-in brings the undercut back at the end-wall corners (0.5 mm: −1.16°, 0.65 mm: −7.5°, 0.8 mm: −75°). Raising the plane to −0.45 mm, as asked, floats the boss's ends over the band at 7.0 mm length (sag 0.55 mm). That needs a shorter cartouche, which I did not try in round 3.
- **Make a bright-cut that no reviewer reads as an arrow, stud or envelope.** I tried four forms:
  - one flat triangle (block-out 1): "arrow";
  - two levels (block-out 2);
  - a sunk roof (round 2): "pyramid stud";
  - a four-plane inverted pyramid (round 3): "arrow / envelope" in the hero light.

  A V-trough built from two planes notches the lozenge's tips, because one plane stays deep all along the diagonal. Round 3's punch list asks for one sloped triangle on the section's lozenge.

## Core changes wanted (exact code; `src/` was not edited)
### 1. A drafted loop of lines and arcs refuses to extrude along its normal (`sketch/solid.rs`)

Probe (same sketch, a 6.0 x 4.8 rectangle with r 0.8 corner arcs drawn counter-clockwise):
draft 0 extrudes both ways; draft 3 or 7 fails "Extrusion: unsupported or degenerate geometry"
at +1.4 mm and builds at -1.4 mm. Drawn clockwise it fails both ways. Rectangles, polygons and
circles taper both ways. `winding()` reverses the order of a loop's curves without reversing
the curves, and the kernel's `extrude_tapered` takes the reversed-order loop but not the
counter-clockwise one. Keystone works round it by drawing the sketch's y flipped and extruding
against the normal (block-out attempt 1), then drops the arcs altogether (see 2).

Wanted: retry the taper against a flipped plane before refusing, in `extrude`:

```rust
// crates/ringdesign-core/src/sketch/solid.rs, in `extrude`, the outer-only arm
if r.holes.is_empty() {
    brep::extrude_tapered(plane, &wound(&r.outer), direction, draft_rad).or_else(|| {
        // The kernel tapers some loops of lines and arcs only when they run against the
        // plane's normal: mirror the loop across the plane's x axis, flip the plane's y,
        // and extrude the same solid the other way round.
        let flipped = Plane::from_axes(plane.origin, plane.x_axis, plane.y_axis.map(|v| -v));
        let mirrored: Vec<Curve> = r.outer.iter().map(|c| c.mirrored_y()).collect();
        brep::extrude_tapered(flipped, &winding(&mirrored, -1.0), direction, draft_rad)
    })
}
```

with a new `Curve::mirrored_y` (y -> -y; an arc keeps its centre mirrored and swaps start and end
so it still runs counter-clockwise), and a test: `rounded_rect(6.0, 4.8, 0.8)` drafted 7 deg
extrudes at +1.4 and -1.4 to the same volume. Better still, fix the kernel's taper.

### 2. Tapered arcs are cones the kernel cannot press-pull, chamfer or fillet

With the taper built against the flipped plane, the rounded cartouche drafts, but feature 5
fails "Press-pull cannot move face 0 by 0.400 mm", and a chamfer or fillet of its rim fails
"UnsupportedBodySurface". Keystone draws each corner arc as 24 straight lines (100 rim edges),
so every drafted wall is a plane. That is a kernel (cadkernel) limit: press-pull `Offset` mode
and chamfer/fillet on faces bounded by cones. Ask upstream; until then, the CAD workspace's
sketch tool could offer "arc as N segments" for drafted profiles.

### 3. The native fillet refuses a closed loop of rim edges

On the drafted, pressed cartouche, `Fillet` of the top's rim edges fails at every radius tried
(0.35, 0.30, 0.25, 0.20, 0.15 mm): "AdjacentSelections(#0v0, #7v0)" on an 8-edge loop, and
"RadiusTooLargeOrInteracting" on the 52- and 100-edge loops. Chamfer of the same edges builds. The section
named this risk (`core/cad.rs` fillet); the fallback is in place and recorded in
`report.json` `authored.fillet_refusals`.

### 4. Crease-aware render normals and fine shading triangles (`render.rs`)

`render::Part::metal` shades a CAD part's large flat triangles as wedges of their own tone and
smooths across sharp edges (a facet's ridge, a chamfer), which reviewers read as smearing. The
example fixes it in the pictures only, with two functions that belong in `render.rs`:

```rust
/// `m` with a vertex per face corner, each normal averaged over the faces round it that turn
/// less than `crease_deg` from its own, and every face halved across its longest edge until
/// no edge exceeds `max_edge_mm`. Rendering only: exports keep the built mesh.
pub fn creased(m: &Mesh, crease_deg: f64, max_edge_mm: f64) -> Mesh { /* officina_keystone.rs `creased` + `split_long` */ }

impl Finished {
    pub fn creased(mut self, crease_deg: f64, max_edge_mm: f64) -> Self {
        self.metal = creased(&self.metal, crease_deg, max_edge_mm);
        self
    }
}
```

### 5. A timeline frame for a plane or a sketch

`Document::through` at a `Plane` or `Sketch` builds the same metal as the step before, so a
timeline sheet shows nothing new there. The example lays a 0.06 mm sheet on a plane and each
region of a sketch (`Profile::Region`, one extrude per region, since a branched sketch's regions
share edges and one extrude of them is non-manifold) and draws it blue. A core helper would let
every Officina ring share it:

```rust
/// What feature `id` adds without metal: a sheet on a work plane, or each region of a sketch,
/// `thick_mm` thick, as meshes in the world frame; empty for a feature that builds a body.
pub fn construction_meshes(d: &RingDesign, lib: &AlphaLibrary, id: Id, thick_mm: f64, params: BuildParams) -> Result<Vec<Mesh>>
```
