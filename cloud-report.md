# Vepres — Prunus (`prunus`): cloud report

Branch `claude/vepres-prunus` from `master` at `8e5a59a`. Example `crates/ringdesign-core/examples/vepres_prunus.rs`, outputs in `showcase/vepres/prunus/`, plus a 012 row in `examples/prickle_probe.rs`. No `src/` edits.

## Verdict: stopped at the block-out. The subject needs rethinking, not detailing.

| Step | Reviewer | `reads` | What the eye saw (abridged) |
|---|---|---|---|
| Block-out 1 | fresh agent, read-test mode | **false** | "an eyeball or a planet, not a sloe … a row of thin bristles like a fishbone comb … no wood, no bark, no blossom … mechanical CAD" |
| Block-out 2 | fresh agent, read-test mode | **false** | "a big glossy blue-grey ball in four claw prongs … steel-straight needles … 'spiked punk ring' or 'sea urchin' … face-on a beetle, a spaceship or a mace head" |
| Block-out 3 | fresh agent, read-test mode | **false** | "closer … 'thorny branch' does come through. The white blossom does not … the stone still looks like a polished gem or an eye in a frame … face-on a mace head, a beetle or a steampunk crossguard" |

- **Rounds used:** none of the three full-review rounds. All three block-out attempts were used and none read, so TASK.md's rule applied: stop and report.
- **Scores:** none. Read tests are not scored, and no full review ran.
- **Ship or cut:** neither. The ring stopped before review.
- The three verdicts are in `showcase/vepres/prunus/read-test-{1,2,3}.json`.

**Why it does not read.** I believe the cause is the base, not the detail:
- The 012 Cushion is a 10 × 10 square boss, and the camera reads a 7 mm cabochon in it as "an eye in a socket" whatever surrounds it.
- In two-part sand, every thorn has to lie in the parting plane. The face view therefore sees all of them on one horizontal line, as "a lance each side, a crossguard".
- Walls that face the pull, the only place free relief can go, are 1–3 mm tall and nearly edge-on to both cameras. Blossoms there read as "beading or filigree dots".

Each attempt fixed what its reviewer named. Attempt 3 drew "thorny branch" for the first time, but never "blackthorn" or "sloe".

**Rethink, for Logan.** Two possible directions:
1. **Lost wax on a non-square head.** The brief allows lost wax where sand would cost the read. Then spurs can cant out of the plane, blossoms can sit on the twig in the face view, and a sloe can hang on a stalk beside the twig instead of filling a square face.
2. **Keep sand, drop the signet.** Make the ring a twig band (Rubus-like), with the sloe as a small cabochon on a side spur.

## Enablers from master

- I fetched `origin/master` before finishing: it carries no commit past `8e5a59a`, so none of C-B2, C-V1 to C-V5 or C-T5 to C-T7 had landed. I used none of them.
- The "merge master at the start of each round" step never ran, because no review round started.
- **C-V1 `level`** is not needed on 012. The new `prickle_probe` rows (`showcase/vepres/prunus/prickle-probe-012.txt`) measure the raw normal along the finger at six shoulder stations:
  - The worst is 3.6e-3, which is 0.20° off the plane, at 185°.
  - The prickle pulls with 0 obstructions at 0.100 and 0.075 mm at every station.
  - The same seat canted 2° does lock one ray at 0.075 mm, so the plane matters, but the master already keeps it.
- The ring's own report repeats the check under every spur: worst |n_z| is 1.5e-3, and `level` is true.

## Gates (the final block-out design; all green at draft and export)

Every column below reads the same in `report.json`, with a block for each build:

| Gate | Draft 768×320 | Draft at 384×192 | Export 1536×448 | Export at 384×192 |
|---|---|---|---|---|
| Triangles (≤ 2 M) | 701,782 | 317,464 | 1,500,560 | 317,486 |
| Watertight, degenerate faces | yes, 0 | yes, 0 | yes, 0 | yes, 0 |
| `self_crossings`, ring / worst part | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| Solids notes, parts notes; features `Ok` | none; all Ok | none; all Ok | none; all Ok | none; all Ok |
| Stamps resolved | 16/16 | 16/16 | 16/16 | 16/16 |
| Closest vertex to the axis vs bore radius | 8.5813 / 8.5813 | same | same | same |
| Field (`attributed_field_report` + `judge_parts`) | **Castable**, 0.013% | Castable | **Castable**, 0.013% | Castable |
| Ray release at 0.100 mm (obstructions / unresolved) | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| Ray release at 0.075 mm (obstructions / unresolved) | 0 / 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| `draft_clamp` worst bite (bark, 0.42 mm relief) | 0.045 mm, 1,629 texels | — | 0.045 mm | — |
| `parting_monotone` | all pass | all pass | all pass | all pass |
| `dfm::findings_in` | 0 | 0 | 0 | 0 |
| Stones reported / previewed; metal in stones | 3 / 3; 0 | 3 / 3; 0 | 3 / 3; 0 | 3 / 3; 0 |
| Crowding census | clean | clean | clean | clean |

- **Cold reload (`--verify`, export, empty library):** identical vertices, faces and normals.
- **Casting pattern (`try_build_pattern`, export):** watertight, 0 degenerate faces, 0 crossings, 1,449,414 triangles.
- **Earlier phantoms.** The 384 × 192 runs during attempt 2 showed single tip phantoms of 0.02–0.05 mm on the spur tips. They moved between pitches and resolutions, and they cleared once the tip cap was rounded further. The final design has none at any resolution.

## Template gate

`collection_templates vepres … --only prunus --verify-export`, class `stock`. The numbers are in `showcase/vepres/prunus/template-verification.json` and `report.json → template_gate`.

| Check | Result |
|---|---|
| `design.set` patches | **1** (`/manufacturing`), against a limit of 4 |
| Graph size | 83 nodes, **3,020,051 bytes** |
| Size budget (stock, 1 MB) | **fails**: over the 1 MB budget and over the 3 MB flag |
| Cold source | identical |
| Cold design and graph reloads | pass |
| Mesh parity at 1536 × 448 | identical: 1,500,560 triangles, vertices, faces and normals |
| `template_gate_passed` | **false**, on size alone |

The bulk is the embedded 2048 × 768 16-bit bark alpha, together with the five stored blossom meshes and the stored calyx mesh. The core change below (8-bit or procedural hides) is the fix.

## The CAD feature tree, as sentences

1. **Cushion** is the 012 band itself, the anchor every part stands on.
2. **Calyx** is a stored, sculpted mesh: a collar under the sloe's girdle that spreads into five pointed, domed sepals and drapes down the boss onto the face. It is Bench stage: soldered on after the seat is cut, so the sand never sees it.
3. **Twig, left shoulder** is a twisted sweep with no twist. Its round 0.85 mm section is carried on a Catmull-Rom Bézier chain in the parting plane, which follows the crest from under the sloe's boss (3.75 mm along) to 27 mm along.
   - The chain is offset out by 60% of its radius, so it stands proud.
   - It is smoothed three times, keeping the tightest bend at 0.78 mm.
   - It tapers to 0.42 of its start radius and dives back into the band over its last 2.5 mm.
   - Joined, cast, no bead.
4. **Twig, right shoulder** is the same, but it rises out of the head's end wall at 5.2 mm, so the stalk can join it.
5. **Twig, the sloe's stalk** is a 0.36 mm shoot from just outside the girdle (3.95 mm along). It thickens 1.7× into the right twig at 7.6 mm: the fruit hangs off the twig.
6. **Spurs 1–5, each shoulder** are revolved spurs, each on a `Ring` placement on the parting line.
   - Placement: `across_mm` 0, `cant_deg` 0, `spin_deg` 0, `tilt_deg` from 31° to 52°, every spur leaning toward its twig's tip. They start 1 mm past the last fold (9.1 mm along) and stand at offsets of 0, 2.9, 6.1, 9.3 and 12.6 mm from there.
   - Lengths are 3.0, 3.0, 4.2, 2.6 and 3.3 mm.
   - Each has a knuckled side-shoot stub 0.62–0.80 mm in radius and 0.7–1.1 mm long. It necks to a 0.44–0.50 mm shaft, which tapers to a 0.4 mm blunted tip.
   - Each has a concave foot that flares 0.22 mm over its first 0.9 mm.
   - A body of revolution whose axis lies in the parting plane faces away from that plane everywhere. That is why the spurs are revolves with a built-in fillet rather than extrudes with `blend_mm`: the measured seam bead round a leaning spur undercuts (0.09 mm² at 40°) and turned the field to "with care".
7. **Blossoms on the twig** sit at +10.6, +17.0, −6.9, −13.6 and −20.0 mm, between the spurs. Each is a stored, sculpted mesh, 2.9 mm across, set on the twig's top and soldered on at the bench:
   - five petals, each domed along its midline (0.38 mm) over a 0.14 mm margin;
   - petals parted by notches at 42% of the radius;
   - a raised heart ringed by stamen beads.

## Layers, stamps and stones, and why

**Layers**
- **Sloe** is Caiman's flush `Boss` seat, kept low (0.15 mm) so the face stays the fruit's ground.
  - Its stone is an onyx `Gem::cabochon(Round, 7.0)` with a blue-black bloom tint `[0.03, 0.034, 0.055]`.
  - Setting: `SolidKind::Flush`, `through`, with a 1.0 mm drill mark.
- **Parting rails, left and right shoulder** are 1.2 × 0.08 mm round `BorderLayer`s on the parting line, windowed at 140° ± 25° and 40° ± 25°.
  - They correct a flaw in the stock: the bare 012 sand master dips a few microns at the parting line over its shoulders (122–162° and the mirror). The field reads that as 0.21% undercut at 3.1°, "Castable with care", before anything is added to the ring.
- **Blossom heart, near wall and far wall** are 1.7 mm `GypsyMound` pads, 0.95 mm high, with `SolidKind::Bead`.
  - Each carries a `Gem::calibrated(Round, 1.3)` diamond.
  - Only one blossom fits each wall, so the ring sets three stones, the brief's own fallback.
- **Blackthorn bark** is one painted hide (`Atlas` 2048 × 768) under `draft_clamp`, 0.42 mm relief, embedded in the design.
  - **On the walls facing the pull**, the plates stand on a ground at 56% of the relief, so the DFM's station-scaled granulometry never reads a furrow as a gap.
    - Furrows run round the ring, wavering on smoothly varying phases and broken by scattered cracks.
    - Each plate rises gently toward the bore and drops steeply into the furrow below it, so nothing rises walking out from the parting line where the wall leans too little.
    - Bark comes on by distance past the rim (`Hide::at`), not by normal. The stock's faceted normals made a normal-based mask bite 0.35 mm.
  - **On the crest off the head**, the plates are three terraces stepping down from the parting line, cut by fissures that only widen outward. The rule takes nothing there.
  - A narrow polished halo surrounds each flower.

**Stamps (16, all along the pull on the head's walls, all `parting_monotone`)**
- On each wall, a 3.0 mm five-petal blossom with a dome top 0.42 + 0.22 mm.
- Five tier-1 stamens on each blossom, placed through the blossom stamp's own frame.
- A 1.9 mm small blossom and a 1.6 × 1.05 mm bud on each wall, completing a spray of three.

**Renders**
- Studio gold with the stones set.
- The house's "recesses darkened" finish, done in the example:
  - band faces on the bark take an oxidised tint, dark on the plates and darker in the furrows;
  - the twigs, the stalk, the calyx and the spurs' stubs take a darker wood tint;
  - everything else is gold, with creases darkened by depth under a relaxed copy of the mesh.

## What I could not do

- **Make it read in three attempts.** See the verdict above.
- **Straif on the palm.** This bench-cut Ogham stamp was a detailing step for round 1, which never came.
- **The template budget.** The embedded hide alone breaks the stock class.
- **Graded spurs from one source.** Rigid pattern motions cannot grade, and C-V2 had not landed. I used one revolve per spur instead.
- **A twig that thickens and tapers in one sweep.** `Twist` only scales linearly. The stalk is a second shoot.

## Core changes wanted, as exact code

**1. 8-bit hides, so a painted hide does not cost 2 MB in a template.** In `library.rs`, a hide-only embed (callers opt in):

```rust
/// Embed the named alphas at 8 bits: enough for a hide whose relief steps are coarser than 1/255 of its height.
pub fn save_design_embedded_8bit(path: impl AsRef<Path>, design: &RingDesign, lib: &crate::AlphaLibrary) -> anyhow::Result<()> {
    let mut design = design.clone();
    let names: Vec<String> = design.layers.referenced_alphas().into_iter().map(str::to_owned).collect();
    let mut eight = crate::AlphaLibrary::default();
    for n in &names {
        if let Some(a) = lib.get(n) {
            let q = crate::Alpha::new(a.name.clone(), a.width, a.height, a.data.iter().map(|v| (v * 255.0).round() / 255.0).collect());
            eight.insert(q);
        }
    }
    design.embed_alphas(&eight);
    write_atomic(path, design_json(&design)?.as_bytes())
}
```

This needs `Alpha::to_png8` in the embed path when every value is a multiple of 1/255. A cold rebuild stays identical, because the build reads the same quantised alpha.

**2. The studio's antiqued finish in `render.rs`**, so every collection renders "recesses darkened" the same way. This is the example's `antiqued`, moved to core:

```rust
/// The metal split by depth under a copy of itself relaxed `passes` times: (mesh, tint) for polished, shallow and deep.
pub fn antiqued(m: &Mesh, passes: usize, shallow_mm: f64, deep_mm: f64, tints: [[f32; 3]; 3]) -> Vec<(Mesh, [f32; 3])> {
    let n = m.vertices.len();
    let mut nb: Vec<Vec<u32>> = vec![Vec::new(); n];
    for f in &m.faces { for k in 0..3 { let (a, b) = (f[k], f[(k + 1) % 3]); nb[a as usize].push(b); nb[b as usize].push(a); } }
    nb.iter_mut().for_each(|l| { l.sort_unstable(); l.dedup(); });
    let start: Vec<V3> = m.vertices.iter().map(|v| [v.0 as f64, v.1 as f64, v.2 as f64]).collect();
    let mut p = start.clone();
    for _ in 0..passes {
        p = (0..n).map(|i| if nb[i].is_empty() { p[i] } else {
            let k = nb[i].len() as f64;
            std::array::from_fn(|c| 0.5 * p[i][c] + 0.5 * nb[i].iter().map(|&j| p[j as usize][c]).sum::<f64>() / k)
        }).collect();
    }
    let depth: Vec<f64> = (0..n).map(|i| { let v = m.normals[i]; v_dot([p[i][0] - start[i][0], p[i][1] - start[i][1], p[i][2] - start[i][2]], [v.0 as f64, v.1 as f64, v.2 as f64]) }).collect();
    let mut out: Vec<(Mesh, [f32; 3])> = tints.iter().map(|t| (Mesh { vertices: m.vertices.clone(), normals: m.normals.clone(), ..Default::default() }, *t)).collect();
    for f in &m.faces {
        let h = f.iter().map(|&i| depth[i as usize]).sum::<f64>() / 3.0;
        out[if h > deep_mm { 2 } else if h > shallow_mm { 1 } else { 0 }].0.faces.push(*f);
    }
    out
}
```

**3. C-V3's scale law on `Twist`**, as `docs/collections/vepres.md` §3 already specifies: `#[serde(default)] scale: Vec<[f64; 2]>`, applied in `twist.rs` where `scale` is computed per station:

```rust
let scale = if law.is_empty() { 1.0 + (end_scale - 1.0) * s.share } else { piecewise(&law, s.share) };
```

With it, the stalk and the twig become one shoot that thickens from the fruit and tapers to the tip.

**4. 012's sand master.** `imported_base::sand_master` could crown the mirrored crest by a few microns, so the bare 012 is Castable without a rail. A fix would need to be tested on all eleven sand stocks, so I have only recorded the measurement (0.21% at 3.1°, 122–162° and the mirror) and the rail workaround here.

## Files

- `crates/ringdesign-core/examples/vepres_prunus.rs` (new) and `crates/ringdesign-core/examples/prickle_probe.rs` (the 012 rows).
- `showcase/vepres/prunus/`:
  - `design.ring.json`, `report.json`, `template-verification.json`, `stones.json`;
  - `read-test-{1,2,3}.json`, `prickle-probe-012.txt`;
  - renders: `hero face palm side shoulder reverse stones bare-vs-finished`, `hero-300`, `face-300` and `contact-300` (`.png`).
- STL files are written locally and git-ignored.
