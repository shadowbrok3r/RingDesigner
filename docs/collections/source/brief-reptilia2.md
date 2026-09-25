# Reptilia II — *Cataphracta*

**Sheet subtitle:** EIGHT HIDES · FIVE BANDS · THREE SIGNETS · FOUR STONES · ALL POURED IN SAND

**Primary side of the app:** the height field as a sculptor's layer stack, used under the sand's rules. Each hide is built from live, editable layers: masks, tie-exact SmoothMax, gradient-SVG alphas, warp, kaleidoscope, grading, keyframed bodies and stone-less runs. The draft rule is applied as a clamp, and the figurative parts are struck stamps. Every ring is two-part sand, holds at most one stone, and must come out **Castable** (not "Castable with care") with **zero DFM findings**.

*Cataphracta* means "the mail-clad". The name comes from *Ouroborus cataphractus*, the lizard that bites its own tail to become a ring. It is ring 4.

---

## 0. The bar: how this beats Caiman

Caiman is the reference Logan loved. Here is where it stands, measured from the repo:

| | Caiman (shipped) | Reptilia II target, every ring |
|---|---|---|
| DFM findings | **2**: "Flank granules" gaps 0.05 mm and "Hornback" 0.13 mm, against the 0.30 mm floor (`showcase/stock-masterworks/caiman/report.json` → `detail_findings`) | **0** from `dfm::findings_in` (dfm.rs:27) |
| Field verdict | Not gated by the writer (stock_masterworks.rs:2034 checks only release) | `attributed_field_report(…, 256, 128)` = **Castable**: 0.000% undercut (or inside the noise band), drag < 12% (`DRAG_FRACTION`, castability.rs:31) |
| Release | 0 obstructions, but sand slots down to 0.10 mm | 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm (the Reptilia `--verify` pass). No slot under 0.30 mm |
| Template | `caiman-imported.graph.json` is **12.7 MB**, with 8 `design.set` patches. `/imported_base` alone is 3.04 MB compact (54%) and 5 PNG atlases are 2.5 MB (45%) | **≤ 200 KB**, ≤ 4 patches. Artwork travels as SVG text or scripts, and the base is referenced by id |
| Editability | Atlases are baked bitmaps; changing the scute pitch means re-running the example | Every skin is a parametric node: pitch, grade, heights, masks and stamp rows are all exposed pins |
| Reptilia I verdict | "Review" on all five (README: low-draft plus fine-detail advisories) | Castable |

The template size matters for the request that triggered this workflow ("asynchronous loading of the templates … keep the app speedy"). `TemplateGraph::instantiate` (graph templates.rs:40) parses and evaluates on the calling thread. A 12.7 MB graph stalls there. A 150 KB graph built from SVG sources does not. Reptilia II is designed to load quickly even under the async loader. Its bake stages (SVG raster → SDF → clamp → build → stamp CSG) are the natural progress steps for that loader's progress bar.

---

## 1. The sand grammar these hides are written in

Every construction below reduces to one of these rules. Each ring cites them by number.

| # | Where | What casts | Source |
|---|---|---|---|
| G1 | Side faces (normal ∥ Z) | Anything, at any height: 0.000% up to 1.6 mm | CLAUDE.md "Side faces are where ornament goes" |
| G2 | The crest line itself | Relief straddling the parting plane: bead rows, gables, fins whose flanks face ±Z | CLAUDE.md showcase ("A bead row rides the crest line only"); commissions (gem columns on the parting plane) |
| G3 | A domed crown off the crest | Only relief that rises no faster than the local draft when walking away from the crest. Beads 1.9 mm off-crest lock at −37° over 4.8%. A hammer peen locks at 2.7% / −9° | CLAUDE.md showcase lessons |
| G4 | Any crown or table | Anything that varies only along the ring: joints across the band, loaves, steps, chevrons whose point leads on the parting line | Caiman lessons ("The face's whole rule is one sentence"); Saurian `scutes` (stock_masterworks.rs:1335) |
| G5 | A factory table (zero draft) | Only tiers that step **down** away from the parting line. Anything proud off the line is an undercut | CLAUDE.md "One theme per ring" bullets; `draft_clamp` (stock_masterworks.rs:1303) |
| G6 | Where a section changes **width** | Walls facing round the ring pick up an axial lean (flutes 1–10°, crest beads 45°) | CLAUDE.md "Two masterworks" |
| G7 | v-gate or mask fades across the band | A fade is a wall facing the crest (off-crest rail lean). Masks that vary only along u are free | CLAUDE.md "Two masterworks"; showoff.rs `fade_svg` (:84), which is "constant across the band" |
| G8 | Warped tilings | Must be gated to side faces. Ungated, the warp pushes sampling v onto the shoulder: 0.89% at −45° | serpentarium.rs:465 |
| G9 | Stamps | `along_pull` on walls (0.35 mm tuck otherwise). Nothing within 1 mm of a parting-line fold. Under a stamp the surface may vary only across the band. Strokes ≥ floor (`stamp_finest_mm`, dfm.rs:200). Walls get `draft_deg` ≥ 4. A concave edge on a crown faces back (the crescent) | CLAUDE.md "A stamp is an outline struck onto the surface"; Caiman |
| G10 | Tilted runs | 45° is clean. 30° leans 5–6° at every seat | showoff.rs Chimera comment; serpentarium memory |
| G11 | Detail floor | `min_feature_px` thresholds the **shaped alpha at 0.5** (alpha.rs:524) and ignores masks, windows and remaps. Lands between beads are gaps and are measured. Caiman's granules failed on gaps, not beads | alpha.rs:492, dfm.rs:27 |
| G12 | Drag | The verdict is Marginal when marginal plus vertical area exceeds 12% (castability.rs:1239-1252). Steep along-ring walls near the crest lower n_z | castability.rs:31 |

Two facts found while writing this brief. Both shape the collection.

1. **`VGate::SideFaces` is resolved on the reference section only.** `VGate::mask` calls `ctx.side_faces_std()` (field.rs:442-470), which reads `ctx.surface`, the reference profile. On a keyframed body the side faces' share of the normalized v changes with each station:
   - A station **thinner** or **wider** than the reference has a smaller side-face share, so gated relief spills onto the crown fillet (G3).
   - Workaround used below: every side-face-carrying body keeps **thickness keys ≥ 1.0 and width keys ≤ 1.0** relative to the reference, so the reference is the tightest station.
   - Ouroborus cannot keep that rule, so it needs enabler E7.
2. **`sand_stock` mirrors the upper half.** The shared helper (examples/common/sand_stock.rs:69, "Clip the upper half and reflect it") mirrors every factory master about its parting plane. I measured each plan's asymmetry across the parting plane (max radial difference, mm) from `bases/signets/*.ringbase.json`:

| Symmetric: usable in sand | Asymmetric: sand_stock rewrites the plan |
|---|---|
| 001 0.22 · 002 0.08 · 003 0.04 · 005 0.01 · 006 0.08 · 007 0.10 · 012 0.33 · 013 0.04 · 015 0.04 · 016 0.09 · 017 0.15 | 004 **4.94** · 008 **3.67** · 009 **1.42** · 010 **3.91** · 011 **1.70** · 014 **5.77** · 018 **1.93** · 019 **5.05** · 020 **2.41** |

   So the three signets use 007 Quatrefoil, 016 Star and 001 Cushion. These are also fresh: only Solstice has used 001, and 007 and 016 have not been used at all. Upright plans (Shield, Heart, Trillion, Heater) need an off-centre parting study before they can go to sand. That work is out of scope here.

---

## 2. The eight at a glance

| # | Ring | Species | Base | Silhouette | Sand | Stone | Headline technique |
|---|---|---|---|---|---|---|---|
| 1 | **Heloderma**: *the beaded one* | Gila monster | Procedural HalfRound 8.0 × 3.2, Keyframes (fat-tail swell) | Domed swell over the top | Delft | Spessartite 3.0 round, flush, the one bead on the spine | Two bead skins at one lattice, SmoothMax'd under a reticulation mask, draft-gated and clamped |
| 2 | **Moloch**: *the thorn idol* | Thorny devil | Procedural squared 7.0 × 3.6, thickness-only keyframed hump | Hump (false head) with two horns thrust along the finger | Petrobond | — | Cone-crowned stamp rows plus graded thorn-rosette fields and capillary grooves |
| 3 | **Sphenodon**: *the third eye* | Tuatara | Procedural squared 7.5 × 3.4, crown 1.2 | A serrated sail standing on the parting line | Delft | Peridot 3.0 round, flush at the nape | A u-only height-field sail on the crest; warped tubercle rows |
| 4 | **Ouroborus**: *the girdled wheel* | Armadillo girdled lizard | Procedural squared 7.0 × 3.2, asymmetric keyframes from head to tail | Tail-biter: a broad head against a thin tail | Petrobond | — | Spiral-graded whorl tiling with side-face spines and a terraced head shield |
| 5 | **Draco**: *the rib-winged* | Flying dragon lizard | Procedural squared 7.2 × 3.8, thickness keyframes | Tall winged top | Delft | Ruby 3.5 round, flush at the wing root | Ribbed wing stamps on the side faces plus a tilted, stone-less diamond crest run |
| 6 | **Chelonia**: *the carapace seal* | Sea turtle (hawksbill) | Factory **007 Quatrefoil**, 16 × 17 | The table *is* the turtle: a shell with a flipper in each lobe | Delft | — | Tiered carapace with terraced growth annuli (clamp plus hide chart) |
| 7 | **Phrynosoma**: *the horned crown* | Horned lizard | Factory **016 Star**, 17 × 17 | Star with six horns thrust along the finger | Delft | — | Along-pull cone horns, tiered cephalic plates, fringe tiling |
| 8 | **Chamaeleo**: *the casque* | Chameleon | Factory **001 Cushion**, 17 × 13 | Long helmeted head | Delft | Alexandrite 5 × 4 cushion, flush on the casque | Casque ridge on the parting line, spiral tail stamps, heterogeneous granules |

Common to all eight:
- Bore 18.6 mm (`resize::size_from_bore(18.6)`, as in Reptilia).
- Build at 1536 × 448, with a 768 × 320 draft.
- `setup.auto_parting = false`, `parting_mm = 0`.
- Studio-gold renders (`render::GOLD`).
- Stones are shown set. STL/3MF export the pattern through `mesh::try_build_pattern` (mesh.rs:411): no cuts, with the raised `mark_mm` drill dot (field.rs:1196).

The two Petrobond rings have bold features. Petrobond's floor is 0.40 mm and its draft 2.5° (castability.rs:180-181). The six fine hides use Delft (0.30 mm, 3.0°). Every feature is designed to ≥ 0.40 mm at its tightest station, regardless of sand.

---

## 3. The rings

### 1 · HELODERMA — *the beaded one*

**Concept.** The one venomous lizard wears beadwork: a thousand domed osteoderms in two heights. The Gila's black and salmon banding becomes high beads and low beads, over a tail swollen with stored fat. One bead in the thousand is a spessartite.

**Face to palm.**
- **Top (±40°):** the fat-tail swell. The dorsal bead row runs the crest, and the spessartite is its central bead.
- **Crest line:** a graded row of round bead-mounds, largest at the top.
- **Dome flanks:** the beadwork, in reticulated high and low bands. Beads lie as flattened shingles near the crest and stand as full domes toward the edges.
- **Palm (270° ± 55°):** belly pavers, square flat-topped beads in transverse rows. They fuse into single plates across the crest ribbon, as the Gila's ventral shields do.
- **Bore:** plain comfort fit. No side faces: this is a dome.

**Base.**
- `ProfileStyle::HalfRound` (profile.rs:38), width 8.0, thickness 3.2, `comfort_fit_mm 0.2`, `edge_round_mm 0.3`.
- `ShankKind::Keyframes` (profile.rs:1206), `amount 1.0`.
- Keys (`ShankKey`, profile.rs:1747):

| θ | width | thickness | crown |
|---|---|---|---|
| TOP | 1.22 | 1.28 | 1.05 |
| TOP ± 55° | 1.08 | 1.10 | 1.0 |
| TOP ± 120° | 0.96 | 0.96 | 1.0 |
| 270° | 0.90 | 0.92 | 1.0 |

**Process.** Delft two-part. The beads need the 0.30 floor's margin at the palm.

**Stone.** Spessartite, `Gem::calibrated(GemCut::Round, 3.0)` with an orange `preview_tint`.
- `SeatPadLayer` (field.rs:1106) at `(TOP, crest_v)`, `SeatStyle::GypsyMound`, `crown 1.0`, `height_mm 0.55`, `blend_mm 0.45`.
- `solid: SolidKind::Flush`, `through: true`, `Blend::Max`.
- The mound is about 4.8 mm across, on a 9.8 mm swell top ("a stone is sized to its face", collection3).

**Construction, in stack (reel) order.**
1. **Group "Beadwork" with a sand clamp (E1)**, window `around(TOP, 300)`, fade 12:
   1. *"Low beads — salmon bands"*: `TilingLayer` (tiling.rs:114) of SVG *Gila bead lattice* (E12 `bead_lattice_svg`). One period per tile: a 2 × 2 hex-staggered cell of radial-gradient domes with the gradient **focus offset toward the band edge**. That makes each bead a gentle ramp on its crest side and a steep drop on its edge side ("everything falls away from the parting line"). Explicit lands: bead Ø at the 0.5 iso = 0.58 × pitch, land = 0.42 × pitch.
      - `mirror_v true` (tiling.rs:150), with the lattice spanning crest→edge, so shingles face outward on both halves.
      - `grade: Cosine { taper 0.30, theta TOP, isotropic }` (E5).
      - Pitch 1.40 mm at the top, 0.98 at the palm.
      - `height_mm 0.24`, `remap Remap::cushion(0.24)` (field.rs:1670), `VGate::Draft { min_deg 24, fade_deg 6 }` (E7), `Blend::SmoothMax`, `soft_mm 0.18`.
   2. *"High beads — black bands"*: the same alpha and the same lattice parameters, so the beads coincide. `height_mm 0.36`, no remap.
      - `mask "Reticulation"`: SVG of forking transverse bands, `feGaussianBlur` 1.2 mm edges, one image over the whole unrolled band (sampled as u/circ, v/len, field.rs:879-884).
      - `SmoothMax soft 0.18`. The two skins tie on the lattice, so SmoothMax crossfades them rather than stacking (smax, field.rs:386; the Apex idiom, showoff.rs:186).
   3. *"Belly pavers"*: SVG *rounded-square pavers* in transverse rows, one period per tile, lands 0.45.
      - `Remap::Terrace { steps 1, span_mm 0.20, riser 0.35 }`, window `around(270, 110)`, fade 20 (a u-only fade, G7).
      - Across the crest ribbon the paver alpha drops its along-ring joints, so rows fuse into plates (G4).
2. **"Dorsal bead row"**: `SeatRunLayer` with `bare: true` (E6).
   - Seat `GypsyMound`, `diameter_mm 1.1`, `height_mm 0.42`, `blend_mm 0.15` (commissions: skirted seats at column pitch merge into a ridge).
   - `v_mm = crest_v`, `taper 0.35` at TOP, `bridge_mm 0.35`, `window except(TOP, 16)` for the stone.
   - Fallback today: `MilgrainLayer` (field.rs:2207), uniform rather than graded.
3. **"Spessartite"**: the seat above.
4. **"Graver: bead lands"** (`bench_only`, `Blend::Subtract`, 0.06 mm): deepen the lands on the flanks after the pour. The verdict and DFM skip it (castability.rs "The verdict judges the pour").

**Showcases.** Masks, tie-exact SmoothMax, gradient-SVG with focal offset, grading, the draft gate, the group clamp, a stone-less run, keyframes.

**Traps and how each is avoided.**

| Trap | Avoidance |
|---|---|
| Beads off the crest lock (G3: −37° / 4.8%) | Focal-offset beads have an inner flank ≤ 20°. The gate starts at 24° of base draft, and the clamp guarantees the rest (target bite ≤ 0.02 mm) |
| SmoothMax does not preserve the draft rule: the crossfade weight varies with a − b | The clamp is on the **group composite**, not per layer. Caiman's "layers joined by Max keep the guarantee" holds only for Max |
| Mask fades across the band are walls facing the crest (G7) | Reticulation fades are ≥ 1.2 mm, so the bead-height step is 0.12 mm over 1.2 mm (6°), below the 24° gate |
| The swell changes width, so walls lean (G6) | The clamp walks the **modulated** sections. The gate is station-aware (E7) |
| DFM gaps (G11): Caiman's granules measured 0.05 mm | Lands are explicit, at 0.42 × pitch ≥ 0.41 mm at the palm station (pitch 0.98). Assert with `min_feature_px` in the generator's test |
| Tight seats merge into a ridge | Bead mound `blend_mm 0.15`, bridge 0.35 |

**Needs.** E1, E5, E6, E7, E12 (`bead_lattice_svg`, `reticulation_svg`, `paver_svg`). E9 makes the body editable.

**Template.** `band.profile` → `shank` (keys as E9 `shank.key` nodes, otherwise a `/shank/keys` patch) → `layer.group` (clamp pin) holding three `layer.tiling` with `alpha.svg` sources. Also `remap.curve`, `remap.terrace`, `window` ×4, `layer.seatrun` (bare), `gem` + `layer.seat`, and an `entry` for the bench layer. Patches: `/draft`, `/manufacturing`, `/build`. Estimated 60 KB.

**Difficulty / risk.** High / medium. The shingle-slope versus gate calibration is the unknown. Measure bite per column before tuning.

---

### 2 · MOLOCH — *the thorn idol*

**Concept.** The thorny devil is named for the god that devoured children. Every scale is a thorn. A false head rises on its nape, and the grooves of its skin drink the dew and carry it to its mouth.

**Face to palm.**
- **Top:** the false head, a keyframed hump that rises but does not widen. It is crowned by the largest crest thorn, and two great horns thrust along ±Z out of the side faces.
- **Crest:** a graded row of cone thorns on the parting line down both shoulders, shrinking to studs at the palm.
- **Crown flanks:** capillary grooves running straight across the band.
- **Side faces:** thorn rosettes (a central cone ringed by six granules), graded, with the capillary lands running between them.
- **Palm:** rosettes give way to plain granules through a u-only SmoothMax handover.
- **Bore:** plain.

**Base.**
- `ProfileStyle::Flat` 7.0 × 3.6, `crown_mm 1.4` (more crown than the squared default; Chimera's lesson), `flatten_sides()`, comfort 0.15.
- Keyframes, **thickness only** (G6, and the side-face rule in §1). Width and crown stay at 1.0:

| θ | thickness |
|---|---|
| TOP | 1.50 |
| TOP ± 30° | 1.25 |
| TOP ± 75° | 1.00 |
| 270° | 1.00 |

   So the reference is the palm, the tightest station.

**Process.** Petrobond two-part. Every feature is ≥ 0.5 mm and the lower draft floor (2.5°) helps the drag gate. **Stone:** none. The thorns are the jewel.

**Construction.**
1. **"Thorn rosettes"**: `TilingLayer` of SVG *rosette thorn* (E12). The central cone is a **linear** radial gradient with a 0.3 mm flat tip. Six granule domes surround it; lands are ≥ 0.45.
   - `fit_to_side_faces(ctx, SIDE_FACE_MIN_DRAFT_DEG)` (tiling.rs:221), `mirror_v`, `VGate::SideFaces(Both)`.
   - `grade Cosine { taper 0.45, TOP }` (E5), `height_mm 0.80`, `Blend::Max`, window `around(TOP, 330)`, fade 12.
   - The cones point along ±Z, so they cast at any height (G1).
2. **"Palm granules"**: SVG granules at 0.9 mm pitch (Ø 0.5, land 0.4), side faces, `SmoothMax`.
   - `mask` = `fade_svg`-style u-only bump centred on the palm. The rosettes carry the complementary u-only mask, so the handover is free (G7).
3. **"Crest keel"**: `CurveLayer` (curve.rs:62), closed, at `crest_v`, `WireProfile::Knife`, width 1.2, height 0.15. This is Pangolin's "knife keel on the crest". It gives every crest thorn a sharp gable to stand on (G9: the cap copies the surface, and a gable is the one surface that does not tilt facets across the plane).
4. **"Capillary grooves"**: `FlutesLayer` (field.rs:1846), count 48 (integer, seamless), `FluteProfile::Vee`, width 0.5, height 0.12, `lean 0.0`, `Blend::Subtract`.
   - `VGate::Band` over the crown only, fade 0.3.
   - A straight flute's walls face round the ring (0.000% at lean 0, field.rs:1856-1865). Stopping short of the fillet avoids the round-flute rib-end lean (0.7% / −5.6°).
5. **Stamps: "Thorn, crest N"**: `stamp.row` (E4) on `RowPath::PartingLine`.
   - 19 per shoulder, `mirror_shoulders`, graded by `taper 0.62` from TOP (Ø 2.4 → 0.9, apex 1.3 → 0.35).
   - Circle outlines with a point every 0.1 mm, `crown: Cone { apex_mm, tip_mm 0.3 }` (E3), `sink 0.3`, `draft_deg 4`, clear of each other by 0.6 mm.
6. **Stamps: "False-head horn, fingertip" / "…knuckle"**: one per side face at TOP, mid-face v, `along_pull: true`.
   - Rounded-triangle root 3.0 × 2.6, `Cone { apex_mm 1.6, at: 0.2 mm toward TOP-side, tip 0.35 }`.
   - They thrust 1.6 mm beyond the band along the finger. This is the ring's silhouette, close to Logan's Odin and Valkyrie spikes.

**Showcases.** Stamp rows with cone crowns, graded side-face fields, a u-only SmoothMax handover, a thickness-only keyframed hump, flutes as grooves, a knife keel.

**Traps.**
- *Off-crest cones lock (G3).* Cones sit only on the crest (straddling, G2) and on side faces (G1).
- *Grooves across a widening section lean (G6).* The hump never widens.
- *Stamp caps on a surface curving both ways give 0.03–0.07 mm phantoms (Caiman).* The knife keel supplies a gable. The hump's along-ring curvature is gentle (a 1.8 mm rise over ±30°); verify at 384 × 192 as `stock_masterworks` does.
- *Feather tips will not fill (MIN_EDGE_MM 0.2).* All apexes are 0.3 mm flats.
- *Side-face relief on both faces thins the axial web (CLAUDE.md "Two things the model does not yet know").* All side-face relief is additive; nothing is carved there.
- *Drag.* Flatten_sides faces have perfect draft, and crown 1.4 keeps the low-draft strip narrow. Measure with E11.
- *Build time.* About 40 crest stamps plus 2 horns. Oriel's 37 stones resolved in 4.3 s at 1.38M faces, so budget about 5 s at export.

**Needs.** E3, E4, E5, E12 (`rosette_thorn_svg`). Fallback without E3: horns as CAD features. Sketch a circle, `Extrude` with taper (cad.rs:59), place with `Placement::Ring`, attach Join, stage Cast. The field verdict does not judge CAD parts, so release inspection must.

**Template.** Tilings, flutes, curve and group nodes lift. Stamps lift as `stamp.row` nodes (E4). Without E4 they ride a `/stamps` patch (lift.rs:445). The row's `taper` and `count` are exposed as "Thorn grade" and "Thorns per shoulder". About 40 KB.

**Difficulty / risk.** High / medium (stamp count and CSG time).

---

### 3 · SPHENODON — *the third eye*

**Concept.** The last of the beak-heads, older than the dinosaurs' fall. A sail of spines stands on the parting line itself, the one place a fin is two side faces. Where the sail begins, the parietal stone: a peridot on the spine.

**Face to palm.**
- **Top:** the peridot on the crest.
- **Crest:** the sail. Its spines rise behind the stone on both sides, are tallest at ±20°, and grade down both shoulders to a low saw at the palm.
- **Crown flanks:** a polished ribbon either side of the sail (G3). The crown is not textured, so the sail reads.
- **Side faces:** granular skin carrying wandering longitudinal rows of enlarged tubercles (warped). Toward the palm they hand over to squarish ventral scales.
- **Bore:** plain.

**Base.**
- `ProfileStyle::Flat` 7.5 × 3.4, `crown_mm 1.2`, `flatten_sides()`.
- Keyframes, thickness only, all ≥ 1.0: TOP 1.18, TOP ± 90° 1.05, 270° 1.0 (the reference is the palm).

**Process.** Delft two-part. The sail's teeth need the finer floor.

**Stone.** Peridot, `GemCut::Round 3.0`, `GypsyMound` at `(TOP, crest_v)`, `Flush`, `through`, `crown 1.0`, height 0.6.
- *Naming risk:* "third eye" is a lizard's parietal organ, and the stone has no lid or pupil. If Logan reads it as an eye, drop the word from the copy. The ring works without the stone too.

**Construction.**
1. **"The sail"**: `TilingLayer` of SVG *sail* (E12 `sail_svg`): u-profile triangular teeth with 0.35 mm blunted tips and rounded 0.4 mm valleys; v-profile a gable peaked on the crest.
   - `repeats_around 44`, `rows 1`, `v_center = crest_v`, `v_span 0.95`, `feather 0`, `height 1.30`.
   - `grade Cosine { taper 0.4 }` (E5), so the teeth space out toward the palm.
   - `mask "Sail height"`: a u-only gradient SVG, peaked at ±20°, 0.35 at the palm, **0 over TOP ± 6°** so the stone stands free (G7 holds because the mask is u-only).
   - Castable by construction: every section of the sail falls away from the crest. Its flanks face ±Z; its teeth faces face round the ring (G2 + G4).
2. **"Tubercle rows"**: SVG with three rows of small granules and one row of large domed tubercles per period (lands ≥ 0.4).
   - `fit_to_side_faces`, `mirror_v`, `height 0.45`, `grade 0.3`.
   - `warp: WarpField` (tiling.rs:40): 8 guide points, ±0.4 mm, `strength 0.8`, `falloff 2.5`.
   - `VGate::SideFaces(Both)` is mandatory under a warp (G8). `SmoothMax soft 0.2`.
3. **"Ventral squares"**: rounded-square pavers on the side faces at the palm. SmoothMax with a u-only fade mask against the tubercles.
4. **"Flank granules"** (crown edge fillet only): fine granules with `VGate::Draft { min 30 }` (E7), where the crown's fillet has the draft for them (G3).
5. **"Parietal peridot"**: the seat.

**Showcases.** The sail is the collection's proof that a fin on the parting line is legal. Also: u-only masks, warp, grading, SmoothMax handover, a flush stone on the spine.

**Traps.**
- *Fill.* The sail is 0.95 mm thick against `min_section_mm 0.8` (castability.rs:180). Tips are 0.35 mm, above MIN_EDGE.
- *Drag from the teeth's round-facing walls (G12).* The area is small (about 44 × 2 × 1.3 × 0.95 mm² ≈ 110 mm² of roughly 1500). Verify with E11.
- *Warp spills past the face (G8).* Gated.
- *Granules on a low crown (G3).* Gated at 30° and confined to the fillet.
- *The v-gate fade on the crown granules is a wall facing the crest (G7).* A 0.6 mm fade inside a ≥ 30° draft zone.
- *Side-face web (M11.1).* Additive only.

**Needs.** E5, E7, E12 (`sail_svg`, `tubercle_rows_svg`). E10 (kfold phase) is optional for a strictly mirrored sail.

**Template.** All layers lift natively; there are no stamps. The sail is the graph lesson: expose "Sail height", "Teeth", and "Sail grade". With the script engine, the sail SVG can be a `script` node output. Rhai caps are 200k ops and 64 KB strings (ringdesign-script lib.rs:24-26), ample for 44 teeth. About 25 KB.

**Difficulty / risk.** Medium / low. This is the safest ring and should be built first to calibrate E5.

---

### 4 · OUROBORUS — *the girdled wheel*

**Concept.** The lizard that is named for the ring. Threatened, it takes its own tail in its mouth and becomes a wheel of spines. The band is its body: an armoured head, girdled whorls graded from nape to tail, and the tail's tip meeting the jaw at the top.

This is a sequel to Serpentarium's Ouroboros, which was a snake with warped mail and which Logan loved. Here the reptile is a lizard, and the skin is whorls.

**Face to palm, going round.**
- **TOP + 3° → TOP + 35°:** the head shield, terraced cephalic plates stepping down from the crest.
- **TOP + 35° → TOP − 10° (going round through the palm):** the whorls, transverse girdles each rising to a trailing edge, graded large at the nape to small at the tail.
- **Crest:** a knife keel from the nape to the tail tip.
- **Side faces:** each whorl's trailing edge breaks into triangular spines pointing tailward.
- **Palm:** the mid-tail, where the whorls are already small.
- **The bite at TOP:** the head's front meets the tail tip.

**Base.**
- `ProfileStyle::Flat` 7.0 × 3.2, `crown_mm 1.3`, `flatten_sides()`.
- Keyframes, asymmetric (an extra key at TOP + 12° tames Catmull-Rom overshoot across the jump):

| θ | width | thickness | crown |
|---|---|---|---|
| TOP + 3° | 1.25 | 1.35 | 0.9 |
| TOP + 12° | 1.22 | 1.30 | 0.95 |
| TOP + 40° | 1.05 | 1.10 | 1.0 |
| TOP + 100° | 1.10 | 1.15 | 1.0 |
| TOP + 190° | 0.95 | 1.00 | 1.0 |
| TOP + 280° | 0.72 | 0.85 | 1.0 |
| TOP − 8° | 0.55 | 0.70 | 1.0 |

**Process.** Petrobond two-part; the whorls are ≥ 1 mm. **Stone:** none.

**Construction.**
1. **"Whorls"**: `TilingLayer` of SVG *whorl* (E12). One period is one girdle: a u-sawtooth rising to the trailing edge (a gentle loaf, a steep 0.4 mm drop) that is constant across the crown (G4).
   - `grade: Spiral { seam_deg: TOP }` (E5, new law). Pitch runs monotonically 2.6 mm → 1.0 mm from the nape round to the tail tip. The lattice kink lands under the bite, so the integer count stays seamless.
   - `height 0.55`, window from TOP + 35° to TOP − 12°, `VGate::Off` on the crown.
2. **"Whorl spines"**: the same `grade` and `repeats_around`, so the spines align with the whorls by construction. SVG triangular pyramids on the trailing edge pointing −u.
   - `fit_to_side_faces`, `mirror_v`, `VGate::SideFaces(Both)` **station-aware (E7)**. Width is > 1 at the head, so the reference-only gate would spill onto the crown there (§1).
3. **"Head shield"**: group with a clamp (E1). SVG of polygonal plates (a Voronoi from mirrored seeds), `Remap::Terrace { steps 3, span 0.5, riser 0.3 }` (field.rs:1628), window `around(TOP + 19, 32)`.
   - The terraces step down away from the crest. The clamp takes whatever the fast width change leans (G6).
4. **"Dorsal keel"**: `CurveLayer` knife, closed, `crest_v`, width 1.0, height 0.18, `window except(TOP + 19, 34)`.
5. **"Scale divisions"** (`bench_only`, Subtract 0.08): the along-ring lines dividing each girdle into scales on the crown. They lock if cast (G3), so the graver cuts them, like Caiman's belly tiles.

**Showcases.** An asymmetric keyframed body, the new spiral grade, aligned multi-layer grading, a terrace remap, a bench-only stage, a knife keel.

**Traps.**
- *Width jump at the bite (G6).* Whorls are windowed off the bite by ±12°, and the head is clamped.
- *Catmull-Rom overshoot.* The intermediate key; confirm every station's `sample_spaced` section stays single-crest.
- *Reference-only side-face gate on a body wider and narrower than the reference (§1).* E7 is **required**.
- *Sawtooth drops are walls facing round the ring:* vertical, so they count toward drag (G12). The drop spans 0.4 mm of a 1.0–2.6 mm pitch; measure with E11.
- *DFM at the tail tip (G11).* The smallest whorl pitch is 1.0 mm, with a loaf of 0.6 and a drop of 0.4. The spines' base is 0.55 wide at the tail.
- *The bypass lesson that "a seam along the crest is a valley".* The keel is a ridge, not a seam.

**Needs.** E5 with the Spiral law, E7, E1, E9 (the body *is* the keys), E12 (`whorl_svg`, `whorl_spine_svg`, `plate_voronoi_svg`).

**Template.** Keys as `shank.key` nodes, exposed as "Head", "Body", "Tail tip". Tilings and group lift natively. About 35 KB.

**Difficulty / risk.** High / medium. The spiral grade is new maths, but it is the eccentric-warp idea (field.rs:1267) with a monotone law instead of a symmetric one.

---

### 5 · DRACO — *the rib-winged*

**Concept.** The only dragon that flies, and it does so on its ribs: patagia folded along the shoulders, five ribs to a wing. A ruby sits at the wing root, where the heart is.

This echoes Logan's own Hypnos: wings wrapping the band.

**Face to palm.**
- **Top:** the body's back at the wing root. The ruby sits on the crest, with the rib roots either side.
- **Shoulders:** four wing plates, one per side face per shoulder. Each is a membrane with a scalloped trailing edge between five radiating ribs, sweeping back from the top to about ±100°.
- **Crown:** the dorsal crest, a graded row of stone-less diamond scales (45° tilted) running from the wing root to the palm.
- **Side faces outside the wings:** fine keeled scales, keels pointing tailward.
- **Palm:** the tail. The diamond row continues tiny; the side faces carry annulated tail scales.

**Base.**
- `ProfileStyle::Flat` 7.2 × 3.8, `crown_mm 1.3`, `flatten_sides()`.
- Keyframes, thickness only, all ≥ 1.0: TOP 1.35, TOP ± 40° 1.25, TOP ± 100° 1.05, 270° 1.0. The top is tall, so the wing roots have about 4 mm of side face.

**Process.** Delft two-part; rib tips are 0.45 mm.

**Stone.** Ruby, `GemCut::Round 3.5`, `GypsyMound` on the crest at TOP, `Flush`, `through`.

**Construction.**
1. **"Keeled scales"**: SVG *keeled rhomb* (keels along u; free on a side face), `fit_to_side_faces`, `mirror_v`, `grade 0.4`, `height 0.28`.
   - `mask "Wing shadow"` (E16, rasterized from the wing stamps' footprints), so the membrane caps copy a flat face (G9).
2. **"Tail annuli"**: across-band rings on the side faces at the palm, SmoothMax'd in with a u-only fade.
3. **"Dorsal crest"**: `SeatRunLayer` `bare: true` (E6). Seat `GypsyMound`, `plan_pow 4` (princess plan), `diameter 1.5`, `tilt_deg 45` (G10), `taper 0.55` at TOP, `bridge 0.3`, `height 0.45`, `crest_v`, `window except(TOP, 14)`.
4. **"Ruby at the wing root"**: the seat.
5. **Stamps: "Wing membrane, right/left, fingertip/knuckle"**: four `Stamp`s (setting.rs:897), `along_pull: true`, `height 0.35`, `draft_deg 5`.
   - Outline (E4 `OutlinePreset::Wing { ribs 5, sweep_deg 95, root_mm 4.0, scallop 0.35 }`) is drawn in the side face's own plane, which is flat, so a planar outline is exact over its whole arc.
   - The outline must stay inside the side face's radial run at every station it crosses. That needs `side_faces_at(θ)` (E7).
6. **Stamps: "Wing rib, …, N"**: 20 ribs, `crown: Ridge { apex_mm 0.30, from root, to tip, tip_mm 0.2 }` (E3), `height 0.35` over the band (so 0.30 proud of the membrane at the ridge). Width 0.9 → 0.45 at the tip (Zenith's trails reached 0.36).

**Showcases.** Large stamps on flat side faces, ridge crowns, a footprint mask, a tilted and graded bare run, keyframes.

**Traps.**
- *Stamps are outside the field verdict* (`analyze_field` judges the field; stamps resolve later). Release inspection at 0.100 and 0.075 mm is their judge. Every stamp needs `built.solids.notes` empty and 0 self-crossings.
- *Stamp walls along the pull are vertical.* `draft_deg 5` keeps them out of the release report's low-draft area.
- *A wing crossing onto the crown fillet* would be a stamp across a normal flip. Clip the outlines to `side_faces_at(θ)` minus 0.3 mm.
- *30° tilts lean (G10).* Exactly 45°.
- *Membrane caps copying scale texture give noisy caps.* The footprint mask removes the texture under them.
- *Web (M11.1).* Wings are additive on both faces.
- *CSG.* 24 stamps.

**Needs.** E3 (Ridge), E4 (Wing preset plus stamp nodes), E6, E7 (`side_faces_at`), E16, E5, E12 (`keeled_rhomb_svg`).

**Template.** Wing and rib stamps as `stamp.outline(Wing)` → `stamp` nodes, with ribs from one `stamp.row`-like fan node (`stamp.fan`, part of E4). Expose "Wing sweep", "Ribs", "Membrane height". About 45 KB.

**Difficulty / risk.** High / medium-high. The wing-within-side-face clipping on a keyframed body is the unknown.

---

### 6 · CHELONIA — *the carapace seal* (factory 007 Quatrefoil)

**Concept.** The turtle is the seal. The quatrefoil's four lobes are its four flippers and its middle is the shell. Every scute is ringed with the years it grew. The shoulders carry the hawksbill's overlapping tortoiseshell, and the palm is the plastron.

**Face to palm.**
- **Table:**
  - Vertebral scutes along the parting line (5 along the ring, straddling, tallest tier).
  - Costal scutes stepping down on each side (4 per side). Their areolae sit on the parting-line edge, as real costal areolae sit dorsally.
  - Marginals as the lowest tier at the rim.
  - Every scute carries 3 terraced growth annuli.
  - The four lobes carry flipper-scale tiers stepping down along each flipper.
  - The quatrefoil's two along-ring cusps are the neck and tail notches.
- **Cheeks (walls):** the shell's side, with vertical serration grooves under each marginal (G1).
- **Shoulders:** hawksbill imbricate plates. Rounded free edges lead toward the palm, graded 2.4 → 1.4 mm.
- **Palm:** the plastron, broad plates with transverse seams (G4). Its central seam is bench-cut, because a seam on the crest is a valley no mould parts (the Bypass lesson).

**Base.**
- `PRESETS` id "007" (imported_base.rs:1101), with a core sand master (E8; today `sand_stock`, examples/common/sand_stock.rs:53) and `sand_envelope = true`.
- `head.length_mm 16`, `profile.width_mm 17`, bore 18.6, `profile.apply_style(Flat)`, edge 0.3, comfort 0.1.
- `chart = SurfaceChart { … }` (imported_base.rs:46), set before any layer (the Stock-masterworks rule).
- Quatrefoil asymmetry across the parting plane measures 0.10 mm, so it is safe (§1).

**Process.** Delft two-part. **Stone:** none. The carapace is the jewel.

**Construction.** Everything runs in hide space (E2): along = millimetres along the parting line from the head's centre; across = millimetres across from it. Everything sits inside one clamped group (E1).

1. **Group "Carapace"** (clamp), window `around(TOP, 110)`:

| Layer | Placement | Relief |
|---|---|---|
| *"Vertebral scutes"* | Hide-space tiling, along pitch 3.1, across ±2.2. SVG hexagonal scute with a centred radial-gradient areola | `Remap::Terrace { 3, span 0.18, riser 0.35 }`, tier top 0.55 → edge 0.40 |
| *"Costal scutes"* | Across 2.2 → 5.4, four per side, mirrored across the parting line in hide space. Areola at the crest-side edge (focal-offset gradient) | Terrace 3 steps, 0.30 → 0.15. The seam to the vertebrals is a **step down**, never a groove: a groove then a rise locks (G5) |
| *"Marginals"* | Across ≥ rim − 1.2, rectangles, serrated trailing edge toward the tail notch | 0.08 → 0 |
| *"Flippers"* | Masked to the lobes by `plan_mask(007)` (E8 helper from `Preset::plan`) | Rows across each flipper's axis, stepping down away from the shell. The lobes point away from the parting line, so this is natural |
| Costal-to-costal seams | Across the band | 0.1 mm V grooves (G4, free) |

2. **"Shell side"**: cheek tiling (walls face ±Z), vertical grooves under the marginals, height 0.25, `Blend::Max`.
3. **"Hawksbill plates"** (clamped group), window `except(TOP, 110)` minus the palm:
   - Hide-space tiling of SVG *shingle*: rounded (U-shaped) free edges leading toward the palm, with the U's apex on the parting line. The wall faces palmward and away from the line (G4).
   - Full width across the shank. `grade Cosine { taper 0.4 }`.
   - Unlike Saurian's pointed chevrons, these are rounded tortoiseshell with growth striae.
4. **"Plastron"**: plates with transverse seams, window `around(270, 100)`.
5. **"Graver: growth striae and the plastron's seam"** (`bench_only`): fine lines between the terraces, plus the central plastron seam.

**Showcases.** The clamp as a live constraint on a factory table, the hide chart, terrace remaps as growth rings, a plan-derived mask, tiers.

**Traps.**
- *Table rule (G5).* Every tier descends from the parting line, including the vertebral and costal seam. Growth rings centred on or beside the line descend with |z| by geometry (distance from an areola on or beside the line grows with |z|).
- *The fold.* The quatrefoil's along-ring cusps are where the parting line turns over the end walls. Nuchal and supracaudal scutes stop 1 mm short (Caiman's horn rule).
- *Column-wise clamp combing (the Zenith lesson).* Design the tiers so the clamp bites about 0; print the bite (Caiman's `clamped`, stock_masterworks.rs:1700).
- *DFM does not see remaps (G11).* Terrace treads must be ≥ 0.40 by hand: 3 steps on a 1.5 mm scute radius gives 0.5 mm treads. E17 fixes this properly.
- *Flat-table drag (G12; Palisade: about 15% of a flat-table signet is zero-draft).* The sand master's crown gives the table 0–11° of draft (sand_stock.rs:132). Measure drag first on the bare stock. If the bare 007 exceeds 12%, raise the master's crown term before drawing a single scute.

**Needs.** E1, E2, E8 (plus `plan_mask`), E12 (`scute_svg(areola)`, `shingle_svg`, `plastron_svg`). E17 is nice to have.
- **Ship path today:** Caiman's atlas method (`Atlas`, `Hide`, `draft_clamp`, `hide_layer`; stock_masterworks.rs:40/1573/1303/1716). It works now, but the template comes out around 10 MB and cannot be edited.

**Template.** `base.preset("007", sand)` (E8) → `layer.group(clamp)` → hide-space `layer.tiling` nodes → `remap.terrace`. Expose "Scute pitch", "Growth rings", "Tier drop". About 50 KB instead of about 10 MB.

**Difficulty / risk.** Very high / medium-high. It depends on E1, E2 and E8, and it is the ring most likely to become Logan's favourite.

---

### 7 · PHRYNOSOMA — *the horned crown* (factory 016 Star)

**Concept.** The horned lizard wears its crown at the back of its skull and weeps blood when cornered. The star's points become horns, the table its plated skull, and its fringe runs down both shoulders.

**Face to palm.**
- **Table:** cephalic plates. A midline row of large plates (frontal, interparietal) straddles the parting line; supraocular and temporal plates step down each side. They are polygonal and drafted, in 3 tiers.
- **Star points:** six become horns. The across-band pair are the great occipital horns (2.4 mm), thrust along ±Z. The four diagonals are temporal horns (1.6 mm). The two along-ring points sit on the fold and stay low and polished, as snout and occiput.
- **Cheeks:** enlarged tubercles among granules.
- **Shoulders' walls:** the lateral fringe, pointed fringe scales in a row along each shoulder's rim, graded toward the palm.
- **Shoulders' crest:** a graded bare run of round tubercles on the parting line.
- **Palm:** fine ventral granules.

**Base.** `PRESETS` "016" (asymmetry across the parting plane 0.09, so it is safe), sand master, `sand_envelope`, 17 × 17, bore 18.6.

**Process.** Delft two-part. **Stone:** none. The horns are the crown.

**Construction.**
1. **Group "Skull plates"** (clamp, E1), in hide space (E2):
   - SVG *tiered Voronoi plates* (E12 `plate_voronoi_svg(seeds mirrored across the line)`). Each plate is a drafted plateau whose height is set by its tier: 0.55 / 0.35 / 0.15.
   - Joints are 0.35 mm V; tier edges are steps down (G5).
2. **"Cheek tubercles"**: cheek-masked tiling (Caiman's `cheek`-style region as an E2 region mask) of SVG *rosette tubercle* with lands ≥ 0.45. This avoids Caiman's 0.05 mm gap finding (G11). Height 0.35.
3. **"Lateral fringe"**: a **height-field** tiling on the shank walls, not stamps: texture, not a figurative motif. Pointed triangular scales tipped palmward, `grade 0.4`. The walls face the pull (G1).
4. **"Crest tubercles"**: `SeatRunLayer` `bare` (E6), round GypsyMounds Ø 1.2 → 0.7, on the shank's parting line from 1 mm past the fold to the palm. Placed by `crest_at`-exact v (G9; E4 exposes the same helper).
5. **Stamps: "Horn, occipital, fingertip/knuckle" and "Horn, temporal, …"**: six stamps, `along_pull: true`, rounded-triangle roots 2.6 × 2.0 (occipital 3.0 × 2.4).
   - `crown: Cone { apex_mm 2.4 or 1.6, at: inside the root offset toward the star point, tip 0.35 }` (E3). An apex inside the outline keeps every facet facing its own half.
   - `draft_deg 4`. `rot_deg` aligns each root with its star point.

**Showcases.** Along-pull cone horns (a silhouette the height field cannot make), a clamped Voronoi tier mosaic on a factory table, region masks in hide space, a bare run on a factory shank's parting line.

**Traps.**
- *Horns on the fold.* None at the along-ring points (Caiman: 0.06 mm from a horn on the kink).
- *Leaning cheeks tuck a square-struck stamp by 0.35 mm.* `along_pull`.
- *A horn whose apex leans beyond its root overhangs.* E3 refuses an `at` outside the outline.
- *Voronoi joints of every orientation on a zero-draft table (G5).* Only tier descents cross columns. Same-tier neighbours meet only along across-band joints; the seed layout is constrained to make that true, and the clamp guarantees it.
- *DFM (G11).* Plates ≥ 1.2 mm; joints 0.35 ≥ 0.30. Horns are stamps measured by `stamp_finest_mm`; tips are slivers and "cost nothing".

**Needs.** E1, E2, E3, E6, E8, E12 (`plate_voronoi_svg`, `fringe_svg`, `rosette_tubercle_svg`). E4 for the horn nodes.

**Template.** `base.preset("016")`, a clamped group, tilings, a bare seat run, and six `stamp` nodes. Expose "Horn length" (one number scales all six), "Plate tiers", "Fringe grade". About 40 KB.

**Difficulty / risk.** High / medium.

---

### 8 · CHAMAELEO — *the casque* (factory 001 Cushion)

**Concept.** The creature that changes colour carries the stone that does: an alexandrite set on its casque. Its prehensile tail is coiled on each cheek, and a crest of cones runs down its spine.

**Face to palm.**
- **Table** (a long head, 17 along the ring × 13 across):
  - The alexandrite sits flush on the parting line at the table's front third.
  - Behind it the **casque** rises: a broad, smooth, helmet-shaped ridge (2.8 mm wide) on the parting line, peaking at the occiput and falling to the table's end.
  - The temporal crests step down either side of it (tiers).
- **Cheeks:** the coiled tail, one thick spiral stamp per cheek, ringed by heterogeneous tubercles.
- **Shoulders:**
  - Crest: the dorsal crest of small cones on the parting line, graded.
  - Walls: heterogeneous granules (three sizes), with the **lateral stripe**, a warped band of larger flat tubercles, SmoothMax'd in.
- **Palm:** fine granules.

**Base.** `PRESETS` "001" (asymmetry 0.22, safe), sand master, `head.length_mm 17`, `profile.width_mm 13` (the stock scales per axis, as Aurelia's 20 × 26 did on 006), bore 18.6.

**Process.** Delft two-part.

**Stone.** Alexandrite (the colour-change stone for the colour-change animal), `GemCut::Cushion` 5 × 4.
- `SeatStyle::Boss`, `crown 0.15`, height = casque base, `metal_true`, `Flush`, `through`. This is Caiman's emerald idiom: a boss the plate's own height (stock_masterworks.rs:2008-2026).

**Construction.**
1. **Group "Casque and crests"** (clamp E1, hide space E2):
   - *"Casque"*: an across-band gable (monotone from the line, G2) × an along profile rising from the stone to the occiput and falling at the table's end. It varies only along the ring apart from its gable (G4). Height 0.9, width 2.8.
   - *"Temporal crests"*: two tiers each side, stepping down.
2. **"Granules, three sizes"**: SVG *granule Voronoi* (E12, periodic, seeded, three radii, lands ≥ 0.4) on the cheeks and shank walls. Region mask (E2), height 0.35.
3. **"Lateral stripe"**: SVG *flat tubercles* (a terraced cushion remap).
   - `warp` along a wavy guide on the shank walls.
   - Masked to the walls (the factory equivalent of G8's side-face gate: a hide-space `wall` region mask, E2).
   - `SmoothMax soft 0.2` with the granules at equal height, so the stripe reads by shape, not by stacking.
4. **"Alexandrite"**: the seat.
5. **Stamps: "Tail coil, fingertip/knuckle"**: `OutlinePreset::Spiral { turns 2.5, r0 0.6, r1 3.6, w0 0.45, w1 0.9 }` (E4), `along_pull`, `height 0.45`, `draft_deg 4`.
   - Concave edges are fine here because a cheek's walls stand along the pull (G9). The crescent rule applies only on a crown.
6. **Stamps: "Dorsal cone, N"**: `stamp.row` on the shank's parting line, `Cone` (E3), 10 per shoulder, graded 1.3 → 0.6, fold-clear 1 mm, crest-exact.
   - The row stands on a knife keel. On a factory shank that means a hide-space gable layer, because a chart-space Curve cannot follow the stock's varying crest v.

**Showcases.** A crest-line casque on a factory table, heterogeneous granules, a warped stripe, spiral stamps, a stone as a boss inside the casque group.

**Traps.**
- *Table rule.* The casque is a crest feature (G2 + G4); the temporal crests are tiers (G5).
- *Spiral stroke floor.* The inner turn is 0.45 wide, measured by `stamp_finest_mm`.
- *Cheek curvature under a stamp (G9).* The cheeks of 001 are near-planar walls; `along_pull` keeps the walls square to the pull.
- *Cones near the fold.* 1 mm clear.
- *Caiman's granule-gap finding (G11).* Explicit lands.
- *The boss inside a clamped group.* The seat is a separate top-level `Max` entry **after** the group, as Caiman's emerald was, so the clamp never shaves the seat's own stock.

**Needs.** E1, E2, E3, E4 (Spiral preset, stamp nodes), E8, E12 (`granule_voronoi_svg`, `flat_tubercle_svg`).

**Template.** `base.preset("001")`, a clamped group, tilings, stamp nodes, the seat. Expose "Casque height", "Coil turns", "Cone grade". About 45 KB.

**Difficulty / risk.** High / medium.

---

## 4. Enablers, ranked

"Shared" means at least one other collection being designed now needs it, for example mythic horns and spikes, feathered wings, or scale-mail signets.

| Rank | Enabler | Shared | Rings | API sketch | Notes |
|---|---|---|---|---|---|
| **E1** | **Sand clamp as a live group property** | **Yes**: every sand collection | 1, 4, 6, 7, 8 (and 2's crown) | `GroupLayer { stack, recipe, #[serde(default)] clamp: Option<SandClamp> }` (field.rs:566). `pub struct SandClamp { resolution: [u32; 2] /*2048×768*/, slack: f64 /*1.0*/ }`. New `RingDesign::bake_clamps(&self, lib)` called by `bake_all` (lib.rs:261) **after** `bake_sdfs`: rasterize each clamped group's composite over the chart, walk each column from the crest outward along the **true** section (`FieldSurface::point`, imported_base.rs:135, or the modulated `section_at`), allow a rise of `tan(lean)·step` as `draft_clamp` does (stock_masterworks.rs:1303), and store a derived `"{group}##clamp"` ceiling that is never saved (like `##sdf`). Store the ceiling only where the rule bit (sentinel elsewhere), so the exact composite shows wherever it was legal. Group height = `min(composite, ceiling)`. `ClampReport { texels_cut, worst_mm }` goes to the report panel and the sheet | Cut back, never fill: the opposite of `pull.rs`'s envelope, which fills (pull.rs:6). Cache by the signature of the serialized group, profile, shank and base. It makes Caiman-class hides parametric. Also fixes the non-guarantee of SmoothMax and Add under the rule |
| **E2** | **Hide chart** | **Yes** | 6, 7, 8, and procedural bands via the same path | Promote `Hide` and `Skin` (stock_masterworks.rs:1573, :1241) into core as `FieldContext::hide() -> Option<&HideChart>`: per texel `along_mm` (signed from the head), `across_mm` (signed from the parting line), per column `rim`, `wall` and `crest_v`, and the fold list (:1757). `TilingLayer::space: ChartSpace::{Chart, Hide}` and `LayerEntry::mask_space`. Region masks `Region::{Table, Rim, Cheek, Wall, Shoulder, Palm}` as built-in mask names | On a procedural band the hide chart is `(u·crest_scale, (v − crest_v)·station_stretch)` (field.rs:153/161). That also retires the "squash decals by k" workaround |
| **E3** | **Stamp crowns** | **Yes**: horns and spikes | 2, 5, 7, 8 | `Stamp::crown: StampCrown` with `#[serde(default)] Flat \| Cone { apex_mm, at: [f64; 2], tip_mm } \| Ridge { apex_mm, from, to, tip_mm } \| Dome { apex_mm }`. Cap height = `height_mm + apex·(1 − ρ)`, with ρ measured along the ray from `at` to the outline. In `stand` (setting.rs:1045), pass the apex and spokes to `cap_faces` (setting.rs:1124) as constraint chords so the facets are true cone sectors. Refuse `at` outside the outline, and refuse an off-line apex on a straddling stamp | Keeps G2 and G9: monotone from its apex, so a single-crest terrain on the crest, and every facet facing its own half on a wall |
| **E4** | **Stamp rows, presets, graph nodes** | **Yes** | 2, 5, 7, 8 | `setting::stamp_row(&RingDesign, &StampRow) -> Vec<Stamp>` with `RowPath::{PartingLine, ChartV{v}, SideFace{pick, frac}}`, a `taper` using the SeatRun law (`theta_of_station`, field.rs:2046), `fold_clear_mm` (Caiman's fold detection, :1757), `mirror_shoulders`, crest-exact v (Caiman `crest_at`, :1645). `OutlinePreset::{Circle, RoundedTriangle, Hexagon, Moon, Spiral, Wing, Custom}` joins `moon_outline` (setting.rs:1071). Graph nodes `stamp.outline`, `stamp`, `stamp.row`, `stamp.fan`, `design.stamps`. Lift emits them instead of the `/stamps` patch (lift.rs:445, assembly.rs:102) | Families are named for the reel ("Thorn, crest 3") so Play build reel strikes them as one step |
| **E5** | **Graded tilings** | **Yes**: feathers, scales | 1, 2, 3, 4, 5, 6, 7 | `TilingLayer::grade: Option<TileGrade { taper, theta_deg, law: GradeLaw::{Cosine, Spiral{seam_deg}}, isotropic }>`. The lattice index runs on φ(u); `Cosine` is `eccentric_warp` (field.rs:1267, make it `pub(crate)`). `Spiral` is a monotone circle map with φ′ ∝ 1/pitch, kinked at the seam. `isotropic` scales v about `v_center` by the local pitch ratio, so rows converge like a real tail. DFM: `tiling_finest_mm_at` × (1 − taper) | The integer count stays seamless because φ maps the circle onto itself. At taper 0 it is bit-identical, as with SeatRun |
| **E6** | **Stone-less runs** | **Yes** | 1, 5, 7 | `SeatRunLayer::bare: bool` (serde default false). Seats use their authored plan (no `fit_stone`) and are spaced by the seat's own `plan_half_extents`. `set_stones` (setstone.rs:106) skips them. The report says "stock only" | This is the only way graded and tilted runs appear in one-stone rings |
| **E7** | **Station-aware v-gates and a draft gate** | **Yes**: any keyframed body with side-face relief | 1, 3, 4, 5 (4 requires it) | A per-station table of side-face runs and drafts built from `modulation_at` (lib.rs, like `stretch`), used by `VGate::SideFaces`. New `VGate::Draft { min_deg, fade_deg }`. Also `FieldContext::side_faces_at(θ)` | Fixes the reference-only gate (§1). Until it lands, keep thickness keys ≥ 1 and width keys ≤ 1 |
| **E8** | **Factory base by id and a core sand master** | **Yes**: every signet collection | 6, 7, 8 | Promote `sand_stock` into core as `imported_base::sand_master(Arc<Source>) -> Result<Arc<Source>>`, deterministic. `ImportedBase::preset: Option<PresetRef { id, sand_master: bool }>` serializes instead of the embedded `Source` (imported_base.rs:52) when the source is bundled, and rebuilds on load. Graph node `base.preset`. Picker badge: "sand-safe plan" versus "upright, will be rewritten" (table in §1). Helper `plan_mask(id) -> Alpha` from `Preset::plan` | Removes the 3.04 MB `/imported_base` patch, 54% of Caiman's template. This is the single biggest contribution to fast async template loads |
| **E9** | **Keyframe nodes** | **Yes** | 1, 2, 3, 4, 5 | `shank.key` node (theta, width, thickness, crown). Un-hide `keys` on `shank` (shank.rs:52) as a list pin. The lift emits one node per key | The body becomes an editable lesson, not a patch |
| **E10** | **kfold phase** | Yes (small) | 3 (optional) | `TilingLayer::kfold_phase_deg`. Today the mirror lines fall at u = 0, i.e. θ = 0° and 180° (tiling.rs:296–303), not at the head | Mirrors the shoulders about TOP |
| **E11** | **Drag attribution** | **Yes** | all | `castability::attribute_drag(&RingDesign, lib, &FieldReport) -> Vec<(layer, marginal_mm², vertical_mm²)>`, muting per layer like `attribute_undercuts` (castability.rs:1562) | Castable versus "with care" is decided by drag on relief-heavy sand rings (castability.rs:1249). No collection has measured it per layer yet |
| **E12** | **Reptile SVG generator set** | Collection-specific | all | Extend `reptile.rs` with `reptile::svg::{bead_lattice, reticulation, paver, rosette_thorn, granule_voronoi, sail, tubercle_rows, whorl, whorl_spine, plate_voronoi, scute(areola), shingle, plastron, keeled_rhomb, fringe, flat_tubercle}`, each `-> String`, one period per tile (templates lesson). Each has a test asserting `min_feature_px` ink and gaps ≥ 0.40 mm at the design's tightest station | Tiny, text-carried artwork. Each can also be a `script` node (64 KB and 200k ops fit) so graphs can expose pitch, land and dome |
| **E16** | **Stamp footprint mask** | Small | 5 | `setting::footprint_mask(&[Stamp], &FieldContext, w, h) -> Alpha`, baked like an SDF, never saved | Caps copy a flat face |
| **E17** | **DFM reads remaps** | Small | 4, 6 | Measure `remap.apply(shaped)` in `tiling_finest_mm_at` | Terrace treads are invisible to DFM today (dfm.rs:27 measures shaped alphas only) |

**Assumed from this batch:** asynchronous template instantiation with progress (the user's request that triggered this run). Reptilia II's stages map onto it one to one: parse → `bake_svgs` → `bake_sdfs` → `bake_clamps` (E1) → build → stamp CSG.

**Build order.**
1. Sphenodon (E5, E7): calibrates grading.
2. Heloderma (+E1, E6).
3. Moloch and Draco (+E3, E4, E16).
4. Ouroborus (Spiral grade, E9).
5. The signets last (E2, E8): Chelonia, Phrynosoma, Chamaeleo.

---

## 5. Collection packaging (the Reptilia format)

- **Author:** `crates/ringdesign-core/examples/reptilia2.rs -- NEW_DIR [--draft] [SLUG] [--verify]`. Refuse to overwrite.
  - Per ring it gates: `attributed_field_report(…, 256, 128).verdict == Castable`, `dfm::findings_in` empty, `mf::inspect` release 0/0 at 0.100 and 0.075, clamp bite ≤ 0.02 mm, watertight with 0 degenerate faces, and a cold reload that is vertex-identical.
- **Graphs:** `crates/ringdesign-graph/examples/reptilia2_templates.rs`. Prefer **authored builders** over the lift, so every alpha is an `alpha.svg` or `script` node with exposed knobs, as the nine BUNDLED templates are built from `templates.rs` builders.
  - The golden check holds the evaluation byte for byte against the author's design (the pattern at reptilia_templates.rs:17-24).
  - Register `REPTILIA_II` beside `REPTILIA` (graph templates.rs:97) and chain it in `catalog()`.
- **Per-ring folder** `showcase/reptilia-ii/<slug>/`:
  - `design.ring.json`, `editable-graph.ring.json`, `artwork/*.svg` (text, not PNG)
  - `finished-metal.stl`, `casting-pattern.stl` (shrink-compensated, no cuts, drill dots), `reference-stone.stl` (4 rings)
  - `report.json`, `mesh.json`, `release-fine.json`, `verification.json` (adds `clamp_bite_mm`, `drag_fraction`, `template_bytes`)
  - `hero/face/palm.png`, Blender `studio*.png` in gold (`tools/render_reptilia.py` gains a `--gold` material), and a build reel
- **Sheet:** `tools/catalog_reptilia.py NEW_DIR --title "Reptilia II" --subtitle "EIGHT HIDES · FIVE BANDS · THREE SIGNETS · FOUR STONES · ALL POURED IN SAND"`.
- **README:** reproduce commands under `systemd-run --user --scope -p MemoryMax=4G`, as in `showcase/reptilia/README.md`.

## 6. Open questions for Logan

1. Sphenodon's peridot: is "third eye" acceptable copy, given his "no snake eyes" rule? The stone has no lid or pupil. Otherwise it is "the parietal stone".
2. Should Draco's wings stay on the side faces (sand-safe, echoes Hypnos)? Or does he want them to rise off the band? That would be lost-wax CAD and outside this collection's brief.
3. Petrobond for Moloch and Ouroborus, Delft for the rest? Or all Delft for one shop setup?
