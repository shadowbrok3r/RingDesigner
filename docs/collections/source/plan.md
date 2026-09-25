# Director's plan: Bestiarium, Cataphracta, Tenebrae, Vepres, and the starter gallery

The plan builds async template loading first, then eight shared platform pieces, then the Bestiarium, with a review by Logan before any other collection starts.

The five briefs cite the code accurately: every file:line I checked matched. Their problems are elsewhere:
- Six sand rings are on factory plans that the sand master rewrites.
- Four factory bases are claimed twice.
- The collections carry six winged creatures between them.
- Four structural gaps no brief names: a deep-cloned alpha library, template graphs evaluated without the script engine, reference-only side-face gates, and CAD stones missing from the stone record.

Paths: `core/` = `crates/ringdesign-core/src/`, `graph/` = `crates/ringdesign-graph/src/`, `wb/` = `crates/ringdesign-workbench/src/`, `gui/` = `crates/ringdesign-gui/src/`. "Doctrine" = the project CLAUDE.md.

---

## 0. Facts checked in source that change the briefs

| # | Fact | Evidence | Consequence |
|---|---|---|---|
| F1 | The sand master keeps the upper half of a factory stock and mirrors it | `crates/ringdesign-core/examples/common/sand_stock.rs:69` ("Clip the upper half and reflect it") | Any plan that is asymmetric across the parting plane gets rewritten in sand. |
| F2 | Measured asymmetry of the plans across the parting plane: max \|z+ − z−\| over head vertices, per 0.5 mm station, from `bases/signets/*.ringbase.json` | 004 **7.8**, 008 **6.3**, 009 **2.5**, 010 **7.7**, 011 **3.7**, 014 **9.0**, 018 **1.7**, 019 **7.9**, 020 **4.1**. The other 11 are ≤ 0.6 (001 reads 1.2 on my coarse bins and 0.22 on Reptilia II's) | Only 001, 002, 003, 005, 006, 007, 012, 013, 015, 016 and 017 can go to two-part sand. Basiliscus (020), Ilex (014), Prunus (009), Sigillum (004) and Officina Sigil (004) all break. Reptilia II's table agrees on which plans fall on each side. |
| F3 | 002 "Kite" is a lens, 14 along the ring × 25 across it, with its points at the band edges | `core/imported_base.rs:1096`; scratch `plans.png` | Draco's reading and Porta's are both geometrically right, but only one of them can have it. |
| F4 | `VGate::mask(v, ctx)` has no θ and resolves side faces on the reference section | `core/field.rs:443`, `:457` (`side_faces_std`) | On a keyframed body whose station is wider or thinner than the reference, side-face relief spills onto the crown fillet. This affects Phoenix, Kraken, Rubus, Heloderma, Moloch, Sphenodon and Ouroborus, not only Reptilia II. |
| F5 | Stamps are made against the band as swept, before any join | `core/setting.rs:1418` | Tiers (a stamp on a stamp) and stamps on CAD parts do not exist today. |
| F6 | Pattern copies drop their stone (`gem: None`), and the stone record knows only `Pad` and `Run` | `core/cad/pattern.rs:518`; `core/setstone.rs:9-15`; `core/stonemap.rs:155` refuses "the design sets no stones" | Every CAD-stone ring comes out with a wrong stones report, sheet and stone map: 20+ rings across all five briefs. |
| F7 | `AlphaLibrary` holds `Vec<Alpha>`, each with its own `Vec<f32>`, and derives `Clone` | `core/alpha.rs:74-81`, `:1491-1504` | `evaluate_design` deep-copies the whole library on every template open (`graph/eval.rs:527`), and so does `library_mut` whenever the worker shares the Arc (`gui/app.rs:1207-1209`). No brief proposes the fix. |
| F8 | `TemplateGraph::instantiate` evaluates with `Evaluator::new()`, which has no expression engine | `graph/templates.rs:42`; `graph/eval.rs:447-449` ("no expression engine is attached") | A template carrying an expression pin fails to open from the menu. The Gothic clusters, Vepres counts and Reptilia II exposed pitches would all hit this. Script *nodes* are unaffected. |
| F9 | Template opening throws away the baked library and computes a field verdict that nobody reads | `graph/eval.rs:526-534`; `gui/export.rs:550-552` bakes again | This is free time to remove in the async loader. |
| F10 | The asset index already stores each asset's uncompressed length | `crates/ringdesign-assets/src/lib.rs:33` (`raw_len`) | A byte-accurate decompress bar needs no build-script change. |
| F11 | Caiman carries 2 DFM findings (0.05 mm and 0.13 mm strokes) | `showcase/stock-masterworks/caiman/report.json` → `detail_findings` | "0 DFM findings" becomes a gate for every sand ring. |
| F12 | The stone builder already has a `form` parameter (faceted or cabochon) | `core/cad/builders.rs:155` | The claw enabler names its parameter `style`, not `form`. |
| F13 | Template sizes on disk today | caiman 12.7 MB, varanus 12.7 MB, solstice-imported 13.5 MB (`graphs/templates/`) | These are the benchmarks for async open, and the reason `base.preset` matters. |

---

## 1. Verdicts, per brief and per ring

**Legend:**
- **K** = keep. **R** = revise, with the change given. **X** = replace.
- **Theme:** the ring commits to one theme from face to palm.
- **Caiman:** whether it stands beside Caiman at equal density and legibility on the sheet.
- **Cast:** whether it is castable as claimed.
- **APIs:** whether the named APIs exist as the brief describes.
- **Lift:** whether it lifts into a graph template cleanly today.

### Factory base allocation (resolves F1–F3 and every double claim)

| Base | Sand-safe | Already used | New owner |
|---|---|---|---|
| 001 Cushion | yes | Solstice | Chamaeleo (Cataphracta, sand) |
| 002 Kite/lens | yes | — | Draco (Bestiarium, sand) |
| 003 Clover | yes | — | Viscum (Vepres, sand), moved from 007 |
| 005 Rosette | yes | — | Sigillum (Tenebrae, sand), moved from 004 |
| 006 Square | yes | Aurelia | Ilex (Vepres, sand), moved from 014 |
| 007 Quatrefoil | yes | — | Chelonia (Cataphracta, sand) |
| 009 Drop | no | — | Porta (Tenebrae, wax), moved from 002 |
| 010 Trillion | no | — | Fenrir (Bestiarium, wax) |
| 011 Badge | no | — | Datura (Vepres, wax), moved from 016 |
| 012 Cushion 10 mm | yes | — | Prunus (Vepres, sand), moved from 009 |
| 013 Round | yes | Saurian, Ophidian | Rosa (Tenebrae, wax, resized to 16 mm after a spike); Officina Sigil (lesson, native size) |
| 015 Octagon | yes | Nocturne, Caiman | Lanterna (Tenebrae, wax); the octagon is its subject |
| 016 Star | yes | — | Phrynosoma (Cataphracta, sand) |
| 020 Escutcheon | no | — | Basiliscus: sand if the mirrored master still reads as heraldic, else wax |
| 004, 008, 014, 018, 019 | no | 019 Vesper | Unused. They stay lost-wax stock (see Q1). |

### Bestiarium (Bestiary): 3–4 sand, 4–5 wax, 34 stones

| Ring | V | Why | Change |
|---|---|---|---|
| Draco | K | **Theme:** yes. **Caiman:** yes if the table membrane reads. **Cast:** 002 is symmetric (F2); stepped panels descend from the spine, which is doctrine ("On a signet's face, anything proud off the parting line is an undercut") and is enforced by the draft clamp. **APIs:** `stock_masterworks.rs` Atlas at :40, draft_clamp :1303, Hide :1573, crest_at :1645, Joints :1667, clamped :1700 and hide_layer :1716 all verified. **Lift:** 5 PNG atlases (~2.5 MB), a 3 MB `/imported_base` patch and a `/stamps` patch. | Drop the "head tucked under the tail" at the palm and keep the spade alone: small heads blur (the Serpentarium hydra lesson). Build one wing-membrane tile first and render the face 1:1 before painting the shoulders. The clamp may bite at most 0.05 mm, as the brief itself says. |
| Basiliscus | R | **Theme:** yes. **Caiman:** close cousin (keel stamps, belly scutes, round scales). **Cast:** 020 is upright (4.1 mm, F2), so `sand_stock` rewrites it (F1). **APIs:** verified (`reptile.rs:9/32/55`; marquise `plan_pow` 1.5, `gem.rs:163`). | Run the mirrored-020 spike (batch 0C). If the mirrored cartouche still reads as heraldic, keep sand. Otherwise go lost wax on the unmirrored 020, Fenrir's route. Either way, let the hackles own the table and both shoulders, so the ring reads as plumage before scales. |
| Fenrir | K | **Theme:** yes (wolf, moon, fetter). **Cast:** 010 in wax without a sand master is fine. **APIs:** the claw path is a 2-D polyline per claw fed to `tube()` (`setting.rs:737-840`), so a fang form is a local change. | Needs P6. Draw the jaws as jaw outlines with tooth notches, **not** with `moon_outline`: two crescents round a moonstone reads as celestial, the theme Logan disliked in Zenith. |
| Phoenix | R | **Theme:** yes. **Cast:** at the 90° key, width 1.45 against thickness 1.22 shrinks the side-face share, and the gate is reference-only (F4), so the wing decals spill onto the crown fillet in sand. | Land P5 first, or re-key so `thickness_scale ≥ width_scale` wherever the wings sit (90°: 1.35 w / 1.45 t). A taller breast also gives the wings more face. Everything else stands: crest runs sit on the parting line, and the collet is a bench part under sand. |
| Corvus | K | **Theme:** yes. **Cast:** a bypass measured 0.0085% (doctrine). **APIs:** a correct atlas on a bypass needs P3, because `examples/reptilia.rs:46` samples only the reference loop. | The beaks (5.2 × 2 mm) carry the whole identity. Render-review them in round 1, before any feathers. If they do not read, grow them to 7 mm and drop the tail interleave. The P4 gable top gives the culmen. |
| Kraken | K | **Theme:** yes. **Cast:** wax. **Lift:** entirely nodes (~30 kB), the collection's best teaching template. | Needs P6 (tentacle) and C-B1 (per-point widths and bead rows; `curve.rs:62-79` has neither today). Side-face spill on keyframes is cosmetic in wax, so P5 is only nice to have. |
| Manticora | K | **Theme:** yes. **APIs:** pear `plan_pow` 2.0 (`gem.rs:156`) is an ellipse; `Attach::Separate` plus `Joint` exist. | Use an oval until C-B2 lands. The tergites use P3's `Joints::eccentric`. The 40 quills use P4 rows, not 40 hand-placed stamps. |
| Harpyia | X | It is the fourth winged beast. Its lesson, the CAD pipeline, is Tenebrae's job. Tiered flat region extrudes risk "flat and blocky", which was Logan's complaint about the Workshop set. It is the highest risk (4.5/5) in the first collection to ship. | Replace with **Arachne** (below). The Hypnos homage survives in Officina's "Aile". |

**Arachne, *the weaver* (lost wax):**
- **Base:** LowDome 5.6 × 2.4 with five keys, 90°: 1.35 w / 1.25 t, falling to 0.92 / 0.95 at 270°, so the legs have a swell to grip.
- **Stones:**
  - Black onyx oval cabochon 10 × 8 as the abdomen, in a collet at θ 97.
  - Garnet round cabochon 3.5 as the cephalothorax, in a collet at θ 84.
- **Legs:** eight `Operation::Twist`, each on a planar knuckled path (coxa rise 1.2, femur arc r 3.0 over 70°, tibia arc r 2.2 over 80°):
  - circle section 0.85, `end_scale` 0.35;
  - seated with `Placement::Ring` at θ 90 ± (10, 22, 34, 46) with splayed spin;
  - the second side made by `Pattern Mirror{Band}`;
  - `Join`, `blend_mm` 0.25.
- **Height field:** an SVG web (radial threads plus a spiral, strokes ≥ 0.3) on both side faces, gated `SideFaces(Both)`. The crown and palm stay polished.
- **Needs:** nothing new, since Twist, Mirror and collets all exist. P2 makes the stone report correct.
- **Risk:** medium: eight csg joins, and feet seated on a keyframed crown. The rules: bend radius ≥ local radius, and `self_crossings` is 0.
- Stone total: 0 + 1 + 1 + 15 + 1 + 1 + 13 + 2 = 34.

### Vepres (Botanical): 5 sand, 3 wax

| Ring | V | Why | Change |
|---|---|---|---|
| Rubus | K | **Cast:** the in-plane argument holds. A round tube whose axis lies in z = 0 has n_z = z/r, so each half faces its own mould half, and the gap under the hook is outside the tube's plan, so it clears along ±Z. `frame_on` uses the raw normal (`cad.rs:456-495`), which is radial on a symmetric procedural crest. | Gate on the E11 prickle probe (batch 0C). Its width key of 1.05 at the nodes: keep width ≤ 1.0 or land P5 first. |
| Sentis | K | Unique silhouette (a woven cage). **Risk:** csg with crossing sweeps. **APIs:** Sweep is capped at 128 stations and open (`cad.rs:1849-1870`), as the brief says. | Build it last in Vepres. Needs C-V4, because `parts::assembled` never reads `blend_mm`. If the crossings misbehave, fall back to three canes. |
| Rosa mortua | K | **Theme:** yes. **Cast:** wax. | Needs C-B2 for the pear bud (or an oval), P6 for the sepal claws, and C-V1 `Relative`. Its stones repeat the Toi et moi starter (pear and oval at the arm tips), so make the hip a **round** garnet cabochon. |
| Hedera | K | Botanically right. **Cast:** wax. | The rootlets need C-V2 (Along), or about 80 hand-placed features. Generate the stem path with a script node: script nodes run without the expression engine, expression pins do not (F8). |
| Ilex | R | **Cast:** 014 is upright (9.0 mm) and rewritten in sand (F1). | Move to **006 Square**, resized to 16 × 17, as "the Holly King's standard". Keep the fesswise leaf on the parting line, gated by P4 `parting_monotone`. |
| Viscum | R | Its base 007 is also Chelonia's. **APIs:** `SeatRunLayer` has a fixed `v` (`field.rs:1917`) and cannot follow a stock crest. | Move to **003 Clover**: symmetric and unused, and its four lobes are the four leaves. Seat the berries as individual pads at `crest_at` (P3) until C-V2 lands. |
| Prunus | R | **Cast:** 009 is upright (2.5 mm) and rewritten in sand (F1); Porta also needs 009. | Move to **012 Cushion (10 mm)**. The sloe becomes a **round** onyx cabochon of 7 mm, since a sloe is round, which also drops the pear dependency. The spurs need C-V1 `level`. |
| Datura | R | Its base 016 is also Phrynosoma's, and Phrynosoma is sand and needs a symmetric plan; Datura is wax and does not. | Move to **011 Badge** (its sinuate outline reads as a calyx), fallback 018. The spines need C-V1 `Relative` to survive a resize. |

### Tenebrae (Gothic): 3 sand, 5 wax

| Ring | V | Why | Change |
|---|---|---|---|
| Rosa | R | 001 is Chamaeleo's, and Solstice's radiating sun already sits on 001. Blocking dependency: F6 (the patterned pears lose their stones). | Move to 013 Round resized to 16 mm, after a resize spike (batch 0C). Fallback: keep 001. Needs P2, and C-B2 for pear lights (or oval lights). |
| Oculus | K | A lovely use of the pull. **Cast:** the verdict cannot see axial webs (doctrine, "Two things the model does not yet know"). | Raise the bore-side rail from 0.8 to ≥ 1.0 mm (inner circle r ≥ 10.3): 0.8 sits exactly on Delft's `min_section_mm`. The README marks the sand-core trial. |
| Ogiva | K | The first parts-only sand ring. | The README states the route: no field verdict, ray release only. |
| Arcus | K | It owns the basket + cathedral + azure chain. | Officina's Lantern stops duplicating that chain (see Starters). |
| Sigillum | R | **Cast:** 004 is upright (7.8 mm); the sand master mirrors it. It also overlaps Officina Sigil. | Move to **005 Rosette**: symmetric, unused, and its cusped frame matches the seal's cusped quatrefoil field. Needs C-T5 (bench marks) and C-T6 (text). |
| Lanterna | K | 5/5 difficulty. **APIs:** `Pattern` takes a single `source` today (`cad.rs:120-123`). | Build it last in Tenebrae. Needs P2's multi-source Pattern. |
| Capsa | R | OpenCascade has no Windows build (PLAN.md), and a Stored mesh forces format 6. | Ship native only: a `Chamfer` on the eaves in place of the OCCT fillet. Make the niche walls 1.0 mm. |
| Porta | R | 002 goes to Draco. | Move to **009 Drop**: wax, and its upright lancet plan is the brief's own fallback. |

### Cataphracta (Reptilia II): 8 sand, 3 stones

| Ring | V | Why | Change |
|---|---|---|---|
| Heloderma | K | **Theme:** yes. **Caiman:** yes (two-height beadwork). | Needs C-R1, C-R2, C-R3 and P5. |
| Moloch | K | **Cast:** horns thrust along ±Z out of the side faces follow G1. | Needs P4 crowns and rows. Petrobond depends on Q2. |
| Sphenodon | K | The safest ring; it calibrates grading. | Build it first in Cataphracta. Rename the epithet "the parietal", with no eye language (Logan's no-eyes rule). |
| Ouroborus | K | It *requires* P5: the body runs both wider and narrower than the reference. | Needs P5 and C-R2 (Spiral law). |
| Draco (rib-winged) | X | The name clashes with the Bestiarium's Draco. It would be the sixth winged creature across the collections, and it depends on six enablers. | Replace with **Gekko** (below). |
| Chelonia | K | Keeps 007. | Gate on drag first, measured on the bare 007 stock (the brief's own rule). |
| Phrynosoma | K | Keeps 016. | Needs P3, P4 and C-R1. |
| Chamaeleo | K | Keeps 001. | Needs P3, P4 and C-R1. |

**Gekko, *the tokay* (Delft, no stone):**
- **Base:** Flat 7.0 × 3.4, crown 1.2, `flatten_sides`, thickness-only keys ≥ 1.0 (top 1.2), so the reference stays the tightest station.
- **Layers:**
  - **"Lamellae":** at the palm, across-band plates that vary only along the ring (G4), graded pitch 0.9 → 1.4, with the split-lamella chevron at 270° (point on the parting line).
  - **"Tubercle rows":** on the side faces, on the existing helix `shear` (`tiling.rs:159-162`), gated `SideFaces(Both)`.
  - **"Granules and spots":** on the crown, a terrace-remapped spot mask inside a clamped group (C-R1).
  - **Bench-only** scale lines.
- **Story:** the ring grips the finger the way the gecko grips glass.
- **Needs:** C-R1, C-R2, C-R7. Light template, and no wings.

### Starters and Officina

| Entry | V | Change |
|---|---|---|
| Async template open | K | This is P1. It lands first (Logan's standing request). |
| Court band, Braided band | K | Re-render in studio gold; the braid thumbnail at pitch 0.6. |
| Wishbone wave | R | Take it off the menu until C-S6 `ShankKey::slide`. Do not ship "Saddle wave": the name collides with `ShankKind::Saddle`. |
| Split shank (Y, wax); Split gallery (sand) | K | Keep both; the sand version teaches the pull doctrine. Rails are measured by `manufacturing::inspect`, since the field verdict cannot see them. |
| Stone settings ×8 | K | Needs P2 (stones in thumbnails and reports). Trilogy and Toi et moi pears need C-B2 or ovals. |
| Starter signets ×20 | R | The 11 symmetric plans open as Delft sand with the envelope. The 9 upright plans (004, 008, 009, 010, 011, 014, 018, 019, 020) open as **lost wax** with a note: the envelope fills an off-centre plateau toward z = 0, so the bare stock would change shape. Badge the picker "sand-safe plan" or "upright". |
| Officina: Rivet, Keystone, Aile, Torsade | K | — |
| Officina: Sigil | R | 004 → **013 native**, as a 7-feature lesson; Sigillum is the masterwork version. |
| Officina: Lantern | R | Rename to **"Fenestra"**. Drop `shank.cathedral` and `cutter.azure`, which Arcus and the Cathedral solitaire starter already carry. Keep side-face piercing along the pull, plus arrays and mirrors of cuts. |

---

## 2. Overlap between the collections, and the fix

| Overlap | Where | Fix |
|---|---|---|
| The name "Draco" | Bestiarium wyvern; Cataphracta flying lizard | The Cataphracta ring becomes Gekko. |
| Winged creatures (6) | Draco, Phoenix, Corvus and Harpyia (Bestiarium); Draco (Cataphracta); Aile (Officina) | Harpyia becomes Arachne, and Cataphracta's Draco becomes Gekko. Three remain, each built differently: a painted table membrane, SVG side-face plumage, and a painted bypass. Aile stays as the lesson-sized Hypnos homage. |
| Base plans claimed twice | 002 (Draco, Porta); 007 (Viscum, Chelonia); 016 (Datura, Phrynosoma); 001 (Rosa, Chamaeleo); 004 (Sigillum, Sigil) | The allocation table in section 1. |
| Painted hide on stock with struck crest stamps | Bestiarium Draco and Basiliscus; Cataphracta Chelonia, Phrynosoma and Chamaeleo; Caiman | Cataphracta owns the scale-hide construction. The Bestiarium signets must lead with membrane and plumage, and their scute rows stay secondary (the Draco and Basiliscus changes above). |
| Seal cut at the bench | Sigillum (Tenebrae); Sigil (Officina) | Sigil becomes a 7-feature lesson on 013; Sigillum is the full seal on 005. |
| Cathedral + azure + head chain | Cathedral solitaire (starter), Officina Lantern, Arcus | The starter is the plain version, Arcus the rich one, and Officina's ring becomes Fenestra. |
| Pear and oval at the bypass tips | Rosa mortua; Toi et moi | Rosa mortua's hip becomes a round cabochon. |
| Crescent jaws that read as moons | Fenrir, echoing Zenith | Jaw outlines, not `moon_outline`. |
| Claw builder forms | Fenrir, Kraken, Rosa mortua, Sentis, Arcus (optional) | One enabler (P6), one `style` parameter. |
| Tapered twisted sweeps | Manticora, Arachne, Rubus, Sentis, Lanterna, Torsade | This is platform reuse and is fine; the silhouettes differ. |

---

## 3. Enabling work, deduplicated and ranked

**Sizes:**
- S: under one agent-day.
- M: one to three days.
- L: more than three days.

### 3a. Platform: must land before any ring is built

| ID | Capability | Merges | API sketch | Needed by | Size | Risk |
|---|---|---|---|---|---|---|
| **P1** | **Async template open with staged progress** (Logan's request) | Bestiarium E12, Vepres E9, Tenebrae E1, Starters E1, F7–F10 | See P1 in detail below | every template; the 13 MB ones stall today | M | M: UI-state races, phone |
| **P2** | **CAD stones are stones** | Starters E2+E3, Tenebrae E2+E8 | `StoneSource::Cad{feature}`, with `SetStone` gaining a frame. `set_stones` walks reference `Builder{stone}` outputs. Pattern copies keep a per-copy `gem` (fixing `pattern.rs:518`), and `Pattern{sources: Vec<Id>}` is accepted (serde still reads `source`). The stone schema gains `tint`, which `render::Part::stone` reads instead of `GEM_TINT`. `render::finished(design, lib, params) -> Finished{metal, stones}` | ~22 rings plus every stone-settings starter; all thumbnails | M | M: every stone consumer (report, census, stonemap, section view, reel) |
| **P3** | **Skin in core** | Bestiarium E1+E11, Vepres E4, Cataphracta E2 (chart half)+E8 (master half) | `core/skin.rs`: `Atlas::of(design, w, h)` (stock through `field_surface`; procedural, keyframed and bypass bodies through `stones::surface_frame`, `stones.rs:465`), `Hide{at, crest_at, folds}`, `Joints{new, eccentric}`, `draft_clamp -> ClampReport{texels_cut, worst_mm}`, `hide_layer`. `imported_base::sand_master(Arc<Source>)`, deterministic, promoted from `sand_stock.rs`. | Draco, Basiliscus, Phoenix, Corvus, Manticora, Ilex, Viscum, Prunus, Chelonia, Phrynosoma, Chamaeleo | M | M. Pin: the atlas matches `mesh::try_build` vertices to within a texel on a keyframed band and on a bypass. |
| **P4** | **Stamps, second version** | Bestiarium E2+E14, Cataphracta E3+E4 (presets and rows), Vepres E3 | `Stamp{tier: u8, top: StampTop{Flat, Gable, Ridge, Cone{apex, at, tip}, Dome, Taper}}`. `apply` joins each tier onto the solid built so far (F5). `core/outline.rs`: keel, lanceolate, leaf family, blossom, fork, spiral, rounded triangle, comb lobe, quill. `Stamp::parting_monotone`; `hull_and_bays` (generalising `crescent_cutter`); `stamp_row(RowPath::{PartingLine, ChartV, SideFace}, taper, fold_clear, crest-exact, mirror_shoulders)` | ~16 rings | M–L | M: cone facets must be constraint chords in `cap_faces` |
| **P5** | **Station-aware side-face gates** | Cataphracta E7 (and F4) | `FieldContext::side_faces_at(θ)` (a per-station table from `modulation_at`, like `stretch`). `VGate::mask(θ, v, ctx)` (`Window::mask` already holds the uv, `field.rs:516`). A new `VGate::Draft{min_deg, fade_deg}`. Bit-identical on unmodulated bands. | Phoenix, Rubus, Heloderma, Moloch, Sphenodon, Ouroborus (required), Gekko; Kraken cosmetic | S–M | L–M: a per-sample performance check |
| **P6** | **Claw styles** | Bestiarium E3, Vepres E6, Tenebrae E14 | `head.claw` / `head.basket` gain `style: Wire\|Talon\|Fang\|Tentacle\|Thorn\|Sepal` (not `form`, F12), `grouping: Even\|Feet\|Jaws` and `tip: Dome\|Point`. Only the 2-D path and radii fed to `tube()` change (`setting.rs:813-827`); an oval-section `tube` is added. | Fenrir, Kraken, Rosa mortua, Sentis, Arcus (optional) | M | M. Pin: bend ≥ local radius and `self_crossings == 0` for every style at 3–8 prongs. |
| **P7** | **Template-weight nodes and lift** | Bestiarium E5, Cataphracta E4 (nodes)+E8+E9, Starters E4–E6, Tenebrae E5+E13, Vepres G1+E8 (pin) | `base.preset{id, sand, face_length, face_width, bore}`, with `ImportedBase` serialized by preset reference when the source is bundled (format ladder step). Stamp nodes: `stamp`, `stamp.outline.*`, `stamp.row`, `design.stamps`. `shank.key` (un-hide `keys`, `nodes/shank.rs:52`). `cad.feature` numeric and `placement` pins. The lift emits all of these in place of `design.set` patches (`lift.rs:411-452`). | every template; it removes 3 MB from each stock template | L | M: the byte-for-byte lift and the format migration |
| **P8** | **Collection tooling** | Vepres E10 | `tools/render_collection.py` and `tools/catalog_collection.py` (generalised from reptilia), `ringdesign-graph/examples/collection_templates.rs -- <collection>`, a stone material manifest for Blender, the `thumbnails!` macro | every package step | S | L |

**P1 in detail** (the user's request; merges the Starters design with Tenebrae's build progress):
- **Loader:** `wb/templates.rs` gains `Template::open(reg, lib, ctx) -> Loading`, running on a "template-open" thread and polled per frame like `start_import`/`poll_import` (`gui/app.rs:612-660`). `instantiate_with(.., &Progress, &AtomicBool)` is the synchronous path for tests and the CLI.
- **Phases and progress:**
  - `Unpack`: bytes out of `raw_len`, streamed inflate (F10).
  - `Read`: indeterminate.
  - `Evaluate`: node i/N, from a new `Evaluator::evaluate_observed` with a cancel check between nodes.
  - `Bake`: source j/M, from `bake_all_observed`.
  - `First preview`: held until the worker lands the first mesh of the new design. Phase two adds `BuildCtx::progress` for features, parts, stamps and seats, which is where the Tenebrae and Vepres templates spend their time.
- **Removed:**
  - The instantiate-time field report (`eval.rs:534`): add `evaluate_design_unjudged`.
  - The second bake (`export.rs:551-552`): the loader returns its baked `Arc<AlphaLibrary>` and the UI swaps a pointer.
  - Both deep copies (F7): `AlphaLibrary.entries: Vec<Arc<Alpha>>`, 17 sites in `alpha.rs`, no `get_mut` callers.
- **Expressions:** attach the script engine through `Evaluator::with_exprs`, which fixes F8. It lives in the workbench, which can see `ringdesign-script`.
- **Behaviour:**
  - The old design stays live until adoption, and picking another template cancels the first.
  - The plate shows spinner, name, bar and Cancel.
  - The status line reads, for example, "Opening Caiman — baking artwork 3 / 7".
  - If the library changed during the load, re-bake onto the current library on landing (now cheap).
- **Hosts:**
  - Desktop: `load_catalog_template` (`gui/export.rs:543`) and `panels/graph.rs:122`.
  - Phone: `ringdesigner-android/src/app.rs:1540` and `app/files.rs:158`.
- **Measure first:** `ringdesign-graph/examples/template_open_probe.rs` prints milliseconds per phase for every catalogue entry in release. The bar's weights come from those numbers; no budget is promised before they exist.
- **Tests:**
  - `fraction()` never decreases.
  - Cancelling mid-Evaluate returns cancelled.
  - kittest: after a click the frame returns with the design unchanged, and stepping until it lands finds the new design.
  - An expression-pin template opens.

### 3b. Carried by a ring or its collection (land inside that collection's batch)

| ID | Capability | For | Size | Note |
|---|---|---|---|---|
| C-B1 | `CurveLayer` per-point `widths`/`heights`, `beads: Option<CurveBeads>`, `phase` | Kraken (Rubus runner optional) | S | Owned by Kraken's lane (`curve.rs`). |
| C-B2 | True stone plans: a 256-entry polar table from the facet mesh's girdle for pear, trillion, heart and half-moon | Manticora, Rosa mortua, Rosa, Trilogy, Toi et moi | M | Oval fallback until it lands. It touches `setting::Plan`, `gem.rs` and `field.rs:1302`. |
| C-B3 | Bestiarium SVG art and painters (`plumage::{contour, remex, hackle, morph}`, `beast::{membrane, fur_lock, tergite}`) | each Bestiarium ring | per ring | Code in the ring's own module, promoted to core only if a second ring uses it. |
| C-R1 | Live sand clamp on a group (`GroupLayer::clamp`, `bake_clamps` after `bake_sdfs`, a derived `##clamp` like `##sdf` at `alpha.rs:574`) | Heloderma, Ouroborus, Chelonia, Phrynosoma, Chamaeleo, Gekko | M | Depends on P3. It replaces painted atlases in templates. |
| C-R2 | Graded tilings (`TileGrade{Cosine, Spiral}` on `eccentric_warp`, `field.rs:1267`) | 7 Cataphracta rings | M | Bit-identical at taper 0. |
| C-R3 | Stone-less runs (`SeatRunLayer::bare`) | Heloderma, Phrynosoma | S | Gekko no longer needs it. |
| C-R4 | Hide-space tilings and region masks (`TilingLayer::space`) | Chelonia, Phrynosoma, Chamaeleo | M | Chosen **over** the Bestiarium's E6 `alpha.hide` painters, which are deferred. |
| C-R5..R8 | Drag attribution; DFM reads remaps; reptile SVG generators; `plan_mask` | Cataphracta | S each | — |
| C-T1..T7 | Tracery and `Profile::Regions`; Gothic clusters (script → op JSON); cutter shapes and the harvested outline library; DFM land width for CAD cuts; centred bench marks; Textura and text-to-sketch; `SweepPath::Sketch` | Tenebrae | S–M each | The clusters rely on P1's `with_exprs` if they use expression pins. |
| C-V1 | `Placement::Ring{level}`, `Relative{part, at, rotation}`, `Side{..}` | Prunus (level), Datura and Rosa mortua (Relative) | M | `frame_on` uses the raw normal today (`cad.rs:456-495`). |
| C-V2 | `PatternKind::Along{path, count, pitch, alternate, roll, scale}` plus `Line`; motions that carry scale | Hedera, Viscum, Porta, Capsa, graded spurs | M–L | Lands in Tenebrae's batch (Porta), and Vepres reuses it. |
| C-V3 | 3-D twist paths with a scale law, and closed sweeps | Sentis, Hedera, Rosa mortua | M | Planar and constant fallbacks exist. |
| C-V4 | Seam beads in CAD-only assembly | Sentis | S | — |
| C-S1..S6 | `Source::Stock` with process chosen by symmetry; `cutter.split`; `cutter.window`; fixtures for retired templates; per-template camera; `ShankKey::slide` | Starters | S–M | C-S6 is optional. |
| — | Deferred | — | — | Bestiarium E4 `Operation::Tube`, E6 `alpha.hide`, E9 (covered by C-V1), E10 (covered by C-V2 scale). |

---

## 4. Build order

**Rules for every batch:**
- **Branches:** one GitHub issue, branch and PR per lane (Logan's workflow). A lane merges when `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --workspace --offline` and the wasm check pass.
- **Concurrency:** the global rule allows at most **two lanes in flight**. Lanes are listed in the order to pair them.
- **File ownership:** each lane owns its files exclusively.
  - `lib.rs` `mod` lines are append-only one-liners, and the second merger resolves them.
  - Registration files belong to the **lead** only: `graph/templates.rs`, `wb/templates.rs` groups and previews, the `showcase/<collection>/README.md`, and the collection sheet.

| Batch | Lanes, in pairing order | Owned files | Done when |
|---|---|---|---|
| **0. Platform I** | **0A P1** async open, plus `Arc<Alpha>`. Run it first: it is the user's live request, and it is independent. | `wb/templates.rs` (+ `wb/templates/loading.rs`), `graph/eval.rs`, `graph/templates.rs`, `core/alpha.rs`, `core/lib.rs` (`bake_all_observed`), `ringdesign-assets/src/lib.rs`, `gui/export.rs`, `gui/app.rs`, `gui/panels/graph.rs`, `gui/panels/mod.rs`, `gui/ui_tests.rs`, `ringdesigner-android/src/app.rs`, `app/files.rs`, `graph/examples/template_open_probe.rs` | The probe table is recorded; the loader, cancel and progress tests are green on desktop and phone. |
| | **0C P3** skin, plus spikes | new `core/skin.rs`, `core/imported_base/sand_master.rs`; examples `prickle_probe.rs`, `parting_stamp_probe.rs`, `side_gate_probe.rs`, `stock_spike.rs` (mirrored 020 and 018, 013 at 16 mm, bare renders) | The atlas-vs-mesh pin passes. The probe numbers written into this plan's open cells decide Basiliscus's process, Rubus's claim and Rosa's base. |
| | **0B P2** CAD stones | `core/setstone.rs`, `stones.rs`, `stonemap.rs`, `cad/pattern.rs`, `cad/builders.rs`, `render.rs`, `gems.rs`; examples `template_shots.rs`, `cad_thumbnails.rs`, `stock_review.rs` (its `:24` id bug) | The Cathedral solitaire's stones appear in the report, sheet, map and thumbnail. |
| | **0D P4** stamps | `core/setting.rs` (Stamp section only), new `core/outline.rs`, `core/dfm.rs` | A cone crown on the crest and a tier on a stamp both field clean; `parting_monotone` is pinned by the 0C parting-stamp probe. |
| **1. Platform II + starters** | **1A P6** claws | `core/setting.rs` (claw section), `core/cad/builders.rs` (schema) | `self_crossings` is 0 for every style at 3–8 prongs. |
| | **1C P7** nodes and lift | `graph/nodes/{base,stamp}.rs` (new), `nodes/shank.rs`, `nodes/cad.rs`, `graph/lift.rs`, `graph/file.rs`, `core/imported_base.rs`, `core/library.rs` (format ladder) | Caiman re-lifts at < 3 MB with no `/imported_base` or `/stamps` patch; every template still round-trips byte for byte. |
| | **1B P5** side gates | `core/field.rs` (VGate and FieldContext) | Bit-identical on the golden corpus; a keyframed spill test fails without it. |
| | **1D P8** tooling | `tools/render_collection.py`, `tools/catalog_collection.py`, `graph/examples/collection_templates.rs` | It regenerates the Reptilia sheet identically. |
| | **1E Starters**, part 1 | `core/templates.rs`, new `core/templates/settings.rs` and `fixtures.rs`, `wb/templates.rs` (starter groups and the `thumbnails!` macro), `ringdesign-core/tests/golden.rs`, `ringdesign-py/tests/test_smoke.py`, and the fixture call sites the brief lists | 20 stocks (process chosen by symmetry) and 8 settings, with studio-gold thumbnails showing stones; the menu test count is updated. |
| **2. Bestiarium authoring** | A lead scaffold goes first: `core/examples/bestiarium/main.rs` + `common.rs` (CLI `--draft/--verify/SLUG`, `write_ring`, gates), with one pre-declared stub module per ring. Then pairs: (Draco, Arachne) → (Phoenix, Basiliscus) → (Corvus, Manticora) → (Fenrir, Kraken). In parallel: Starters part 2 (`cutter.split`/`cutter.window` in `cad/builders/cutters.rs` plus their `SPECS` rows, after 1A merges) and `core/examples/officina/`. | Each ring lane owns only `examples/bestiarium/<ring>.rs` and `examples/bestiarium/art/<ring>/`. Kraken's lane also owns `core/curve.rs` (C-B1). | Every ring passes the section 5 gates. |
| **3. Bestiarium packaging, and Logan's review** | **3A** (lead): package it. **3B:** C-B2 true plans (`setting.rs` Plan, `gem.rs`, `field.rs:1302`). **Then 3C:** Cataphracta enablers C-R1..R8 (`field.rs`, `tiling.rs`, `lib.rs` `bake_clamps`, `dfm.rs`, `reptile.rs`, `castability.rs` drag). 3C waits for 3B because both touch `field.rs`. | as listed | Logan has seen the Bestiarium sheet and reel. **Stop here for his answer**: he said he may trim to fewer rings per collection after the first. |
| **4. Cataphracta** | Order: Sphenodon (calibrates C-R2) → Heloderma → (Moloch, Gekko) → Ouroborus → (Chelonia, Phrynosoma) → Chamaeleo. Layout: `examples/cataphracta/` with one module per ring. Templates are **authored builders** where the SVG is text (the brief's ≤ 200 KB bar), lifted otherwise. | per ring module | Section 5 gates, and the collection is packaged. |
| **5. Tenebrae** | Enablers first: (C-T1 sketch tracery and regions: `sketch/edit.rs`, `cad.rs` Profile) with (C-T6 text: `text.rs`, new `sketch/text.rs`); then (C-T3 cutters and outlines: `cutters.rs`, `tools/harvest`) with (C-V2 Along: `cad/pattern.rs`); then C-T4/T5/T7. Rings: Oculus and Ogiva first (they need nothing) → Rosa → Sigillum → Porta → Arcus → Capsa → Lanterna. | per ring module in `examples/tenebrae/`; clusters in `graphs/clusters/` owned by the C-T2 lane | Section 5 gates; clusters open through P1 with expressions. |
| **6. Vepres** | Enablers: (C-V1 placement: `cad.rs` Placement) with (C-V3 3-D twist: `cad/twist.rs`); then C-V4 (`parts.rs`) and C-V5 graph path nodes. Rings: Rubus (after the 0C probe) → (Ilex, Prunus) → Viscum → Datura → (Rosa mortua, Hedera) → Sentis. | per ring module in `examples/vepres/` | Section 5 gates. |

**Why this order:**
- **P1 first:** Logan asked for it in this run, it touches no ring, and every later template (the heaviest yet) benefits.
- **Bestiarium second:** it is his business line (Kings of Alchemy, dark-mythology cast rings). With Harpyia swapped out, all its rings sit on P2/P3/P4/P6 plus one small curve change.
- **Cataphracta third:** reptiles are the theme that has landed with him (Caiman).
- **Tenebrae fourth:** it needs only moderate new CAD.
- **Vepres last:** it carries the most new geometry (C-V1..V3).

---

## 5. How each ring is iterated to quality

**Loop per ring, capped at three review rounds.** A ring still failing after round 3 is cut or deferred, never shipped weak.

1. **Draft build** at 768 × 320, then these automatic gates. A red gate goes back to the author with no review round spent.
   - **Geometry:** watertight with 0 degenerate faces; `self_crossings` 0 on every made part; `built.solids.notes` empty.
   - **Process:** `attributed_field_report(…, 256, 128)` judged against the ring's own `DraftSettings::process`.
     - Sand: **Castable**, not "with care"; release 0 obstructions and 0 unresolved rays at 0.100 and 0.075 mm; clamp bite ≤ 0.05 mm.
     - Wax: fill ≥ 0.8 section on the investment recipe; the author asserts CAD land widths until C-T4.
   - **DFM:** `dfm::findings_in` returns **0 findings** (F11: Caiman had 2).
   - **Stones:** the P2 stone report agrees with the gem preview count; the crowding census is clean or explained.
2. **Render set**, in studio gold with stones set: hero, face, palm, side, a stone close-up, and bare stock against finished.
3. **Review round.** A fresh reviewer agent scores the renders against Caiman's hero, the Reptilia sheet and Logan's ZBrush sheet (`b15/zbrush_top.png`), using this checklist:
   - one theme face to palm, with no scattered motifs (the Palisade rule);
   - figurative motifs are stamps with true outlines, not painted relief (the Zenith lesson);
   - no faces and no eyes;
   - stones sit in made settings and are visible;
   - shoulder ornament is not cut off as it approaches the face;
   - any custom head is at least 13 mm;
   - no flat, blocky CAD;
   - the factory stock keeps its hard wall-to-face angles;
   - the silhouette is distinct from the other seven rings;
   - the ring stands beside Caiman at equal or greater density and legibility.

   The reviewer returns a numbered punch list, and the author applies it. With the two-lane limit, the reviewer and the author alternate within one lane.
4. **Export build** at 1536 × 448 with `--verify`. A cold reload with an empty library must give identical vertices.
5. **Template gate:**
   - The lift, or an authored builder, evaluates cold to the same design byte for byte, with an identical mesh.
   - At most 4 `design.set` patches.
   - Size recorded in `verification.json`, with these budgets after P7: procedural ≤ 300 KB; stock ≤ 1 MB; painted-atlas rings as measured, flagged above 3 MB.
   - Open time per phase from `template_open_probe`, recorded.
6. **Reel:** stamp families named for `stamp_family` ("Dorsal spine, 3" plays as "Dorsal spine"), and layer names as captions. Recorded from the rdsmoke AVD with the known screenrecord recipe.

**Packaging, per collection** (the lead only, following the Reptilia format):
- **Per-ring folder** `showcase/<collection>/<slug>/`:
  - `design.ring.json`, `editable-graph.ring.json`, `artwork/` (SVG as text);
  - `finished-metal.stl`, plus `casting-pattern.stl` (sand: shrink-compensated, no cuts, drill dots) or the investment pattern;
  - `reference-<stone>.stl`;
  - `report.json`, `mesh.json`, `release-fine.json` (sand), `verification.json` (with clamp bite, drag, template bytes and open milliseconds);
  - `hero.png`, `face.png`, `palm.png`, Blender `studio*.png`, and the reel.
- **Collection level:** `README.md` with the reproduce commands under the memory guard, `index.html`, `<Collection>-collection.png` from `catalog_collection.py`, and a renders zip.
- **Templates:**
  - `graphs/templates/<slug>-<collection>.graph.json`, bundled automatically because the directory is the family (`ringdesign-assets/build.rs:29`);
  - a static per collection in `graph/templates.rs`, chained into `catalog()`;
  - a group in `wb/templates.rs` placed after the starters;
  - 160 px thumbnails from `render::finished` through `tools/template_thumbnails.py`;
  - the preview test count updated.
- **Phone:** bump the version, then verify with `cargo test -p ringdesigner_android` and the ndk check, and on the rdsmoke AVD.
- **Names:** Bestiarium, Cataphracta (Reptilia II), Tenebrae, Vepres, Officina.

---

## 6. Questions only Logan can answer

1. **Upright factory plans in sand.** Shield, Heart, Drop, Trillion, Badge, Heater, Butterfly, Jewel and Escutcheon cannot pour in two-part sand without being rewritten, because the sand master mirrors the upper half. The plan uses them only for lost-wax rings, and opens their bare starters as lost wax. Is that acceptable, or should an off-centre parting study (a real enabler, several days) come first, so that shields can be poured?
2. **Your sands.** Do you pour Delft clay only, or Petrobond as well? Moloch and Ouroborus were written to Petrobond's rules; with Delft only, both are re-tuned to 0.30 mm detail and 3° draft.
3. **Harpyia versus Arachne.** The plan swaps the harpy (CAD wings over the band, Hypnos-like, the fourth winged beast and the riskiest ring) for a spider gripping a black cabochon, with its web on the side faces. Keep the swap, or keep the harpy with lofted feathers in place of flat tiers?

---

## 7. Logan's answers (2026-09-24) — these override the questions above

1. **Upright factory plans are lost wax only.** No off-centre parting study. The nine upright plans (004, 008, 009, 010, 011, 014, 018, 019, 020) are used only by lost-wax rings, and their bare starters open as lost wax, badged "upright". Basiliscus on 020 therefore goes **lost wax** (unmirrored), and the 0C mirrored-020 spike is dropped.
2. **He pours Delft and Petrobond.** Moloch and Ouroborus keep Petrobond's rules (0.40 mm detail, 2.5° draft); the rest stay Delft.
3. **Keep both Harpyia and Arachne.** Harpyia comes back with wings as **lofted feathers** (not flat region tiers, which read blocky). The Bestiarium authors nine rings; after the first render round the weakest one is dropped, leaving eight.
4. Names stand: Bestiarium, Cataphracta, Tenebrae, Vepres, Officina. Sphenodon's epithet is "the parietal". Final renders are studio gold throughout.
