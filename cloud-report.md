# Vepres Sentis: cloud report

Branch `claude/vepres-sentis`. The rethink started from `master` at `2e11632`, and round 3 merged `29babc4` (#260). Example: `crates/ringdesign-core/examples/vepres_sentis.rs`. Outputs: `showcase/vepres/sentis/`.

## Outcome: cut at 6.4 after round 3

The rethink read on its second block-out (read test 5). Three full reviews followed, and every gate was green at draft and export in each. The art scores rose (5.6, then 6.2, then 6.4) but never reached the 7.5 ship bar. Round 3 was the last round: the doc records no extension, so the three-round cap applied, and a score under 7.5 is a cut. The outcome is recorded in the Sentis section of `docs/collections/vepres.md`.

| Item | Result |
|---|---|
| Block-out read tests | 6 allowed; 5 used. Tests 1–3 (four-cane thicket) failed, test 4 (the rethink) failed, test 5 read. Test 6 was not needed. |
| Review rounds | 3 of 3 |
| Final verdict | **Cut, 6.4** |
| Gates at the cut | All green at draft and export, template gate included |
| Weight | 7.30 g in 18k gold, US 8.6, bore 18.6 mm |

## Every read test and review

| Step | Verdict | What the reviewer saw | Main ask |
|---|---|---|---|
| Read test 1 | reads: false | "A crown of thorns or a coil of barbed wire", with a gumdrop stone | Hip calyx, hooked prickles, tapered noded canes, leaves |
| Read test 2 | reads: false | "Crown of thorns, or a woven wire wreath"; the leaves read as flecks | Large leaves, a raised hip, a looser weave |
| Read test 3 | reads: false | "Still a crown of thorns"; the leaves read as feathers or flames | Pinnate leaves on a rachis, a matte urn hip |
| *Rethink (Logan)* | — | Two canes, two large rose leaves, an opaque hip; lost wax at 0.8 mm | — |
| Read test 4 | reads: false | "No longer reads as a crown of thorns"; the 6–7-a-side leaflets read as fern, laurel or olive | Exactly five broad leaflets per leaf, with gaps on a visible rachis |
| Read test 5 | **reads: true** | "Wild rose with a hip (dog rose, briar)"; hawthorn and pomegranate came second | The calyx read as a cage: thin sepals springing from a boss |
| Round 1 | revise, **5.6** | The plant reads, but it is a thin wire bramble with a caged bead. About 85% of the ring is bare cane, and the calyx reads as a ship's wheel. | 7 items: leaflets and sepals, a calyx off the stone, a true opaque stone, density all round, palm prickles, cane forks |
| Round 2 | revise, **6.2** | The cage is gone; the hip is clear and orange. Prickles went from 10 to 29 and weight from 4.7 g to 7.07 g. The leaflets were thickened into puffy pillows, and the wall gate did not cover every part. | 6 items: full wall coverage, flat bevelled leaflets, bezel and tessellation, cane bark and nodes, bud-tipped shoots, leaf 2 a variant |
| Round 3 | **cut, 6.4** | Every gate is green, wall coverage is complete, and the shoots have buds. The leaflets are still pillows with stepped margins. The dome shows tessellation bands and an apex star, the bezel is a plain collar, and one sepal stands up like a claw. There is no bark, and there is an even row of palm prickles. Leaf 2 is a rigid copy of leaf 1. | (Only if reopened) modelled leaflet blades, a re-tessellated dome and a flared cup, bark and irregular prickles |

The full JSONs are `read-test-1.json` … `read-test-5.json` and `review-round1.json` … `review-round3.json` in `showcase/vepres/sentis/`.

## Gates

The draft runs at 768 × 320 and the export at 1536 × 448 (`--verify`). A CAD-only part is tessellated at its own chord, so both builds give the same 209,326 triangles.

| Gate | Draft | Export |
|---|---|---|
| Finished mesh watertight, 0 degenerate faces, 0 self-crossings, 1 shell | pass | pass |
| Every CAD part (67 made parts) closed with 0 crossings | pass | pass |
| Solids and parts notes empty; all 68 features `Ok` | pass | pass |
| Nothing in the finger hole (nearest vertex 9.350 mm, bore radius 9.300 mm) | pass | pass |
| Lost-wax walls, ray-sampled on the finished mesh (see below) | pass | pass |
| DFM findings | 0 | 0 |
| Stones: 1 reported, 1 previewed; no metal inside the stone; crowding clean | pass | pass |
| 384 × 192 rebuild | pass | pass |
| Casting pattern watertight, 0 degenerate faces, 0 crossings | pass | pass |
| Triangle budget (2 M) | 209,326 | 209,326 |
| Cold reload | identical | identical (vertices, faces, normals) |
| Field verdict | "Castable with care" (by construction; see below) | same |

**Walls (Logan's lost-wax rule: 0.8 mm section, no pull rule; there is no sand bonus, since Sentis was never a sand candidate).** There are 66 `ray_walls` entries, one per metal made part, each with 203 to 320 rays. No part reads below its floor. The worst count of hard-edge rays skipped is 292 (the target was under 1000).

| Class | Floor | Sampled minimum |
|---|---|---|
| Canes | 0.8 | 1.33 |
| Nodes | 0.8 | 1.39–1.73 |
| Stipules | 0.8 | 0.87–0.90 (one sampled on its own solid, as declared) |
| Side shoots | 0.8 | 1.08 |
| Hip receptacle and cup | 0.8 | 0.85 |
| Hip stalk | 0.8 | 1.75 |
| Calyx boss | 0.8 | 1.60 |
| Sepal bodies | 0.8 | 0.83 |
| Leaflets | 0.8 | 0.85–0.91 |
| Petiole and rachis | 0.8 | 1.00 |
| Prickles | 0.15 | 0.25–0.41 |
| Sepal points | 0.15 | 0.27 |
| Shoot-bud points | 0.15 | 1.25 |

The field verdict reads "Castable with care" for every CAD-only design (`castability.rs:1199`: there is no procedural band to sample), so the wall gate above stands in for it, as the brief says.

## Template gate

Command: `collection_templates`, then `--only sentis --verify-export`, with `template_class` `procedural`.

| Measure | Result |
|---|---|
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **297,607 B** of the 300,000 B budget: **pass** |
| Nodes | 75 |
| Cold source | Identical (lift) |
| Graph reload, design reload | Pass |
| Vertex, face and normal parity | Identical |
| Export geometry | Verified |
| First build | 3.0 s |

To fit the budget I shared one prickle foot sketch, used Points hook paths, cut the stations, rounded placements to 1e-4, stored the node and urn meshes, and replaced copies with About and Mirror patterns. The block-out had been 1.38 MB.

## The ring as authored (round 3)

**Process.** Lost wax, using `probe::wax_setup`: 0.8 mm section, 0.15 mm detail, no draft. The bore is 18.6 mm. The design is CAD only, with no `Operation::Band`, so `parts::assembled` unites every metal part and lays C-V4 seam beads wherever `blend_mm` is set.

**CAD feature tree** (68 features):
1. **Briar canes 1 and 2.** Each cane is one closed `Twist` (C-V3) that winds twice round the finger on a twine of radius 10.85. The twine opens 0.95 mm across the finger toward the crown so the two canes part round the hip. A scale law tapers each cane from 0.7 mm at the palm to 0.9 mm at the crown, and the canes cross at the shoulders.
2. **Prickle foot.** One shared flattened-teardrop `Sketch` serves every prickle.
3. **Prickles 1–28.** Each is a hooked `Twist` along a Points path, with a scale law, standing on its cane's outward normal. Where the cane runs inside the twine, the prickle stands off the cane's side instead. Each prickle is placed with C-V1 `level` (tilt, cant and spin solved from a world frame) and bedded on a 0.2 mm C-V4 seam bead. Prickles are dropped if they face the finger, fall within 66° of the crown, sit by a shoot, or crowd the other cane. A probe build then finds the rest: prickles whose bead the kernel cannot lay are re-authored with a crisp root (prickles 7 and 20), and prickles that leave a fin are left out (prickle 4). The probe repeats until no fin remains.
4. **Nodes and stipules.** There are 7 stored-mesh node swellings, shoulders first and at least 34° apart. They carry 12 stored stipule wings that lean back along the cane; a stipule that would come near the bore is left off.
5. **Side shoots 1 and 2.** Each is a short `Twist` at the crown ± 80° that rises 1.4 mm, ending in a separate tapered bud point.
6. **The hip.** All of it shares one frame at the crown:
   - **Hip receptacle and cup:** a stored urn that swells under the stone, tapers into the stalk, rises round the girdle and turns in over the dome with a rounded rim.
   - **Hip stalk:** a `Twist` down into a cane.
   - **Oval cabochon 6.4 × 4.8:** opaque orange-red (tint [0.72, 0.20, 0.06], transmission 0.05). Its long axis runs round the ring.
   - **Calyx:** a round knop (a sphere, 1.6 mm across) on the hip's long axis. Five sepals come from one stored sepal body and one stored sepal point, each turned five times round the knop's axis by an About pattern.
7. **Rose leaf 1.** A filleted petiole and rachis `Twist` with a scale taper. Two stored paired leaflets are mirrored across the rachis by a `Mirror { Band }` pattern, giving four, plus a stored terminal leaflet. Each leaflet is a bevelled, cupped blade with a midvein and eight teeth a side.
8. **Rose leaf 2.** An About pattern turns leaf 1's leaflets and rachis round the hip.

**Enablers used:** C-V1 placement `level`; C-V3 closed, tapered, twisted sweeps with a scale law; C-V4 seam beads in CAD-only assembly; #248 `write_png_framed` for the face, stones and leaf close-ups; and the `About` and `Mirror` patterns. Not used: P6 claws (dropped, because round 1 read them as a cage), C-V2 `Along` (each prickle needs its own normal and spin, which `Along` cannot vary), #255 Textura and `crisp_relief` (they would not fit the 300 KB budget). #260 landed before round 3 and was merged, but I did not use it.

## What I could not do

- **Flat, sharply serrated leaflet blades.** Every leaflet is a lofted stored mesh. To keep its walls at 0.8 mm, each blade is about 0.85 mm thick across its whole width, and that reads as a pillow. Finer teeth needed more loft stations than the template budget allows.
- **A smooth cabochon dome.** The preview mesh is fixed in core at `SEG 48`, `RINGS 14` (`gems.rs:350`), with a linear z step and a fan to the apex. That produces the bands and the apex star. I cannot fix it from an example.
- **Leaf 2 as a 0.85× folded variant.** A Pattern has no scale or per-copy fold, and a second set of stored leaflets would push the template past 300 KB.
- **Bark fissures** were left out for the template budget.
- **A "Castable" field verdict.** It is refused by construction for a CAD-only design.

## Core changes wanted (exact code)

**1. Cabochon preview tessellation** (`core/gems.rs`, `cabochon` and `girdle_cabochon`). Step the rings by angle so they space evenly over the dome, and close the top with a small ring instead of a long fan:

```rust
const SEG: usize = 128;
const RINGS: usize = 40;
// ...
let at = |ring: usize, i: usize| -> [f64; 3] {
    // Even steps of the section's angle, so facets are equal over the dome and the last ring sits close under the apex.
    let phi = ring as f64 / RINGS as f64 * std::f64::consts::FRAC_PI_2;
    let (scale, z) = (phi.cos(), h * phi.sin());
    // ... unchanged ...
};
```

Pass the same `RINGS` and angle step to `girdle_cabochon`. That gives 128 × 80 triangles per stone, which is negligible.

**2. A mesh-space lost-wax verdict for CAD-only rings** (`core/castability.rs`). This is the `judge_cad_only` from my first report, unchanged. Call it after the build, and let it set `Castable` when every part's wall clears its floor (the section, or the detail floor for parts whose `bench_notes` start with "pointed").

**3. A hard-edge-aware thickness measure** (`core/cad/measure.rs`, `thickness`). Skip ray starts on faces adjacent to an edge whose dihedral exceeds 50°, and stop counting hits nearer than 0.03 mm as walls. Report both counts:

```rust
pub struct Thickness { pub sampled_min_mm: f64, pub rays: usize, pub hard_edge_skipped: usize, pub seam_folds: usize }
// in thickness(): let hard = hard_edge_faces(mesh, 50f64.to_radians());
// for each start face f: if hard.contains(&f) { t.hard_edge_skipped += 1; continue; }
// for each hit: if d < 0.03 { t.seam_folds += 1; continue; }
```

The Sentis example carries this as `sample_walls`; moving it into core would let every CAD-only ring use one gate.

**4. A scalable, foldable pattern copy** (`core/cad.rs`, `PatternKind::About`):

```rust
About {
    part: FeatureId,
    count: u32,
    span_deg: f64,
    /// Uniform scale applied to copy k as scale.powi(k), about the part's origin; 1 keeps the size.
    #[serde(default = "one", skip_serializing_if = "is_one")]
    scale: f64,
    /// Extra turn, in degrees, of copy k about its own local x axis (a fold), times k.
    #[serde(default, skip_serializing_if = "is_zero")]
    fold_deg: f64,
},
```

This would let leaf 2 be a 0.85× folded variant of leaf 1 for a few bytes.
