# Vepres Sentis: cloud report

Branch `claude/vepres-sentis`, from `master` at `8e5a59a`. Example: `crates/ringdesign-core/examples/vepres_sentis.rs`. Outputs: `showcase/vepres/sentis/`.

## Outcome: stopped at the block-out (three read tests, none reads)

Sentis never passed its block-out. The third read test also failed, and `TASK.md` says that means stop: the subject needs rethinking, not more detail. So no review round was run.

| Item | Count |
|---|---|
| Block-out read tests | 3 of 3, all `reads: false` |
| Review rounds | 0 of 3 (never reached) |
| Final verdict | Stopped before review. No score. |

| Step | Verdict | What the reviewer saw (first impression) | Changes asked |
|---|---|---|---|
| Read test 1 | reads: false | "A crown of thorns or a coil of barbed wire bent into a hoop, with a red gumdrop cabochon sitting on top." Even round wire, straight needle thorns, a stone dropped on the tangle. | Rose-hip calyx and stalk; broad, hooked, one-way prickles graded crown to palm; angled, tapered, noded canes, taller shoots and briar leaves |
| Read test 2 | reads: false | "Crown of thorns, or a woven wire wreath, with a red oval stone." The new calyx was "the one real plant clue" but too small. Face-on, "a braided rope or Celtic-knot chain band with a red cabochon". The leaves read as flecks. | Large three-leaflet leaves laid flat facing out; hip raised, with a bigger calyx and a visible stalk; looser crown weave with taper and nodes; taller pruned shoots |
| Read test 3 | reads: false | "Still a crown of thorns … a glossy red oval gem." The calyx read "like a prong or a spark". The leaves now showed, but "as single blades, feathers or flames, not three-leaflet rose leaves". The open weave and the cut stubs gave a better silhouette than attempt 2. | Five-leaflet pinnate leaves on a visible rachis; a matte, garnet-orange upright urn hip with sepals overhanging its shoulders; broader prickles and tapered canes |

### Why I think it failed (for whoever rethinks it)

- **"Thorny ring" is already a known object.** At 300 px, a closed wreath of thorned stems is named before anything else: a crown of thorns. Every attempt lost to that prior. The hip, the leaves and the calyx sit at the crown, and in both cameras they are small. The hip is about 25 px across in `hero-300`. The leaves are 4 mm, about 30 px. In `face-300`, the face camera frames the whole 25 mm ring, so the crown is only about 60 px tall.
- **The plant has to be named by something no crown of thorns has.** For a briar that means the rose leaf (five or seven leaflets on a rachis) and a hip that reads as fruit, not as a gem. Both need more of the crown than a 4-strand weave leaves them. I would rebuild with the leaves and the hip as the large forms and the thicket as the ground. That means fewer canes at the crown (two, parted wide), a pinnate leaf about 8 mm long on each side of the hip, and an opaque fruit (a garnet or carnelian cabochon, or a metal urn hip with an enamel-red stone in its open calyx) standing clear of the canes.
- **Uniform canes read as wire.** The Sweep has no scale law (C-V3 is not on master), so the canes cannot taper. The lofted node swellings were not enough to change the read.

## Master and enablers

`git fetch origin master` showed no new commits during the session: master stayed at `8e5a59a`. So none of C-B2, C-V1 to C-V5 or C-T5 to C-T7 was available, and none was used. From what was already on master I used:
- **P6 `head.claw` with `style: "Thorn"`** (and `tip: "Dome"`, `rails: "None"`);
- **P2 `StoneSource::Cad`** for the stone record and the tint.

Without C-V3 or C-V4 I used these fallbacks:
- **No two-half canes.** The kernel already closes a polyline sweep whose last station equals its first, so each cane is one closed 128-station sweep. That is better than the section's two-half fallback (see "Core changes wanted").
- **Crisp roots.** The thorns, shoots and stalks have no seam beads, because `parts::assembled` never reads `blend_mm`.

## Gates (final block-out state)

The draft is 768 × 320 and the export 1536 × 448 `--verify`. A CAD-only part is tessellated at its own chord, so the two builds are the same 119,320 triangles.

| Gate | Draft | Export |
|---|---|---|
| Finished mesh watertight, 0 degenerate faces, 0 self-crossings | pass | pass |
| Every CAD part 0 crossings | pass | pass |
| Solids and parts notes empty; all 63 features `Ok` | pass | pass |
| Nothing in the finger hole (nearest vertex 9.3003 mm against a 9.3 bore radius) | pass | pass |
| Lost-wax walls (ray-sampled, `cad::measure::thickness`) | **FAIL** | **FAIL** |
| Field verdict | **"Castable with care"** (see below) | same |
| DFM findings | 0 | 0 |
| Stones: 1 reported, 1 previewed; metal inside the stone 0; crowding clean | pass | pass |
| 384 × 192 rebuild | pass | pass |
| Casting pattern watertight, 0 degenerate faces, 0 crossings | pass | pass |
| Triangle budget | 119,320 | 119,320 |
| Cold reload with an empty library | n/a | **identical** |

**The walls gate fails.** The sampled minimum for each part type is below, with the floor it is judged against. Thorns and the calyx are pointed details, judged at the 0.15 mm detail floor; the rest are judged at the 0.8 mm section.

| Part | Sampled min | Floor | Result |
|---|---|---|---|
| Canes | 1.32 mm | 0.8 mm | pass |
| Liner | 1.10 mm | 0.8 mm | pass |
| Nodes | 1.44 mm | 0.8 mm | pass |
| Hip stalk | 0.50 mm | 0.8 mm | fail |
| Hip receptacle | 0.37 mm | 0.8 mm | fail |
| Leaf stalks | 0.40 mm | 0.8 mm | fail |
| Cut shoots | 0.054 mm | 0.8 mm | fail |
| Thorns | 0.147 mm | 0.15 mm | fail (just under) |
| Leaflets | 0.143 mm | 0.15 mm | fail (just under) |
| Hip calyx | 0.096 mm | 0.15 mm | fail |
| Thorn claws | 0.033 mm | 0.15 mm | fail |

The stalks are 0.92 mm round and the shoots 1.24 mm, so their low readings come from rays at their buried end caps and at the pruning cut. That is the same ray noise Rubus reported, but it still counts as a failed gate here.

**The field verdict fails by construction.** `castability::attributed_field_report` returns `Marginal` ("Castable with care") with the note "CAD solids require mesh-space manufacturing inspection" for any design with no procedural band (`castability.rs:1199`). A CAD-only ring can never show "Castable" there. The brief swaps the field for the 0.8 mm wall gate in lost wax, so the walls row above is the gate that matters.

The ring weighs 9.26 g in 18k gold, at US 8.6.

## Template gate

Command: `collection_templates vepres target/tpl-src --only sentis --verify-export`, with `template_class` `procedural`.

| Measure | Result |
|---|---|
| `design.set` patches | **1** (`/manufacturing`) |
| Graph size | **1,381,182 B** against the 300,000 B procedural budget: **over budget, gate failed** |
| Nodes | 71 |
| Cold source | Identical (lift) |
| Graph reload, design reload | Pass |
| Vertex, face and normal parity | Identical |
| Export geometry | Verified |
| First build | 5.6 s |

Where the bytes go, in the compact design JSON:
- 224 KB: the 21 node lofts. Five 24-gon world-space sections each, written at full f64 precision.
- 181 KB: the stored meshes (9 leaflets, the calyx).
- 47 KB: the 4 cane paths and 3 leaf stalks.
- 43 KB: 14 thorn sketches.

To get under budget, round sketch coordinates to 1 µm, cut the node sections to 12-gons or drop them, and coarsen the leaflet meshes.

## The ring as authored (block-out attempt 3)

**Process.** Lost wax, using `probe::wax_setup`: 0.8 mm section, 0.15 mm detail, no draft. The bore is 18.6 mm. The design is CAD only: there is no `Operation::Band`, so `Document::replaces_band()` holds and `parts::assembled` unites every metal part. `profile.thickness_mm` 2.7 sets only the analytic seat radius (12.0 mm) that `Placement::Ring` measures heights from.

**CAD feature tree** (63 features):
1. **Heartwood liner.** A torus (major 9.85, minor 0.55) whose inside is the bore. It is the smooth comfort surface, and every cane dips into it.
2. **Briar canes 1–4.** Each is one closed 128-station sweep of a five-lobed section (radius 0.72 ± 7%). The cane winds three waves round the ring on an elliptical bundle of 0.55 × 2.15 mm half-axes about radius 10.85, with these changes:
   - At the crown the bundle opens 0.35 mm outward and deeper, and swells to 2.6 mm across.
   - Each cane has its own spread (0.90–1.07) and a phase wander of up to 0.4 rad, so the plait is not a rope.
   - Canes 1 and 4 run outside either side of the hip.
   - Opposite canes cross with real overlaps, never at a tangency.
3. **Thorns 1–14.** Each is a twisted sweep: a broad flattened elliptic foot (2.7 × 1.0 mm at the crown) rising 0.35 mm, then hooking 72° round a 1.25 mm bend, with a 0.42 mm tip.
   - Each thorn stands on its cane's outward normal. `Placement::Ring` tilt and cant are solved from that normal.
   - All thorns on one cane hook the same way.
   - Crown thorns are 1.4 times the palm's, on an uneven pitch.
   - Thorns that face the finger or sit under the hip are dropped.
4. **Cut shoots 1–3.** Round tapered stems (Ø1.24 → 0.97) rise 3.3 mm and lean away from the hip. Each end is pruned on a 38° slant by its own **pruning cut**, an `Attach::Cut` extrude in the shoot's frame.
5. **The hip.** All of it shares one frame: the girdle 2.2 mm over the seat at the crown, its long axis round the ring, tilted 28° up out of the thicket. It has four parts:
   - **Hip receptacle:** an oval urn lofted through six ellipses under the stone.
   - **Hip stalk:** a 0.92 mm sweep from inside the receptacle's low end, curving into the cane below.
   - **Hip calyx:** a stored mesh of five slender sepals 5.0 mm across their points, curling 1.1 mm back. It stands just past the stone's tip, turned a further 45° up so the star faces the eye.
   - **Oval cabochon 6.5 × 5:** ruby, tinted [0.55, 0.03, 0.08], as a P2 CAD stone. It is gripped by **Thorn claws** (P6: four claws, `style: Thorn`, `tip: Dome`, `rails: None`).
6. **Briar leaves 1–3.** Each has a terminal leaflet (4.2 × 2.7 mm) and two laterals (3.2 × 2.2 mm), stored meshes laid over the top of the tangle and bent round the ring with it.
   - The leaflets are domed from a 0.3 mm margin to 0.62 mm over a sunk 0.1 mm midrib groove, with eleven forward teeth.
   - Each leaf stands on a 0.92 mm **stalk** sweep down into the outermost cane.
   - The leaves sit at θ 73° and 107°, either side of the hip, and at 44°.
7. **Nodes 1–21.** Lofted spindles that swell each cane by 0.16 mm over 2.2 mm, at an uneven pitch, none under the hip.

There are no layers and no stamps. The ring has one stone.

## What I could not do

- **Tapered canes and star sections.** Taper needs C-V3, which is not on master. The kernel refuses a scale law on a closed path.
- **Seam beads at thorn, shoot and stalk roots.** These need C-V4, also not on master. The roots are crisp.
- **A "Castable" field verdict for a CAD-only ring.** It is refused by construction (see Gates).
- **The read itself.** See "Why I think it failed".

## Core changes wanted (exact code)

**1. A closed sweep with a per-station scale (C-V3).** In `core/cad.rs`:

```rust
Operation::Sweep {
    sketch: Profile,
    path: Vec<[f64; 3]>,
    /// The path closes on itself; its last station need not repeat the first.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    closed: bool,
    /// (share of the path, scale) pairs, share rising from 0 to 1; empty keeps the section's own size.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    scale: Vec<[f64; 2]>,
},
```

Evaluate it with `brep::SweepPath::Polyline3d { points: path, closed: *closed }`. When `scale` is not empty, sweep each span between stations separately, scaling from `scale(share_k)` to `scale(share_k+1)`, then union the spans. This is needed because `cadkernel::brep::sweep_path` refuses `closed && scale != 1`. An alternative is a cadkernel `SweepOptions::scale_law: Option<&dyn Fn(f64) -> f64>`. Fence it with `library::format_version_for` like the other format-6 fields.

**2. A mesh-space lost-wax verdict for CAD-only rings.** In `core/castability.rs`, replace the early return at `:1199` with a verdict the caller completes:

```rust
if !design.band_is_procedural() {
    return FieldReport { verdict: Verdict::Marginal, notes: vec![CAD_ONLY_NOTE.into()], ..empty };
}

/// A CAD-only ring judged in mesh space: Castable when every part's sampled wall clears the section, or the
/// detail floor for parts the author marks pointed (`Component::bench_notes` starting "pointed").
pub fn judge_cad_only(field: &mut FieldReport, design: &RingDesign, built: &crate::mesh::BuildResult) {
    if design.band_is_procedural() { return; }
    let Some(e) = built.parts.evaluated.as_ref() else { return };
    let mut thinnest = f64::MAX;
    for c in e.components.iter().filter(|c| !c.settings.reference && c.attach != crate::cad::Attach::Cut) {
        let floor = if c.settings.bench_notes.starts_with("pointed") { design.draft.min_detail_mm } else { design.draft.min_section_mm };
        let t = crate::cad::measure::thickness(&c.mesh, floor);
        thinnest = thinnest.min(t.sampled_min_mm);
        if t.sampled_min_mm < floor { field.verdict = Verdict::Marginal; return; }
    }
    field.thinnest_wall_mm = thinnest;
    field.verdict = Verdict::Castable;
    field.notes.retain(|n| n != CAD_ONLY_NOTE);
}
```

**3. Seam beads in CAD-only assembly (C-V4).** In `core/parts.rs` `assembled`, after each `Ok(t)` union, when `metal[g].settings.blend_mm > 0`, call `blend::bead_seam(&tool, &solids[g], metal[g].settings.blend_mm)` and union the bead in. This is the same path `resolve_with` uses for band joins.

**4. `cad::measure::thickness` should skip rays that start within one chord of an open cap or a hard edge.** On thin parts the sampled minimum swings from 0.03 to 0.15 mm on the same geometry, which makes the 0.8 mm and 0.15 mm wall gates noisy. The ray start in `cad/measure.rs` should drop samples whose face neighbours turn more than 60° within 0.1 mm.
