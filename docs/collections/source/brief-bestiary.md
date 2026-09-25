# Bestiarium — design brief

Eight beasts from the dark bestiary, one per ring. Each creature is read from its hide, its plumage and its weapons, face to palm. None of them is read from a face. This collection shows the app's **sculpting depth**: hides and plumage painted in metal millimetres on factory stock and keyframed bodies, emblems struck as true outlines, and CAD weapons (talons, fangs, tentacles, a stinger, wings) holding made-set stones.

**Sheet subtitle:** EIGHT BEASTS · FOUR IN SAND · FOUR IN WAX · THIRTY-THREE STONES

Citation legend: `core/` = `crates/ringdesign-core/src/`, `sm:` = `crates/ringdesign-core/examples/stock_masterworks.rs`, `ex/` = `crates/ringdesign-core/examples/`, `graph/` = `crates/ringdesign-graph/src/`, `wb/` = `crates/ringdesign-workbench/src/`. "Doctrine §" points to a section of the project CLAUDE.md.

---

## 1. The collection at a glance

| # | Ring | Creature | Base | Process | Stones | Silhouette | Main construction |
|---|---|---|---|---|---|---|---|
| 1 | **Draco** | wyvern, seen from above | factory **002 Kite**, 13 × 19 mm | Delft sand | none | head wider across the finger than it is long | stepped membrane wings on the table, dorsal spines, scaled body |
| 2 | **Basiliscus** | cockatrice | factory **020 Escutcheon**, 16 × 16.5 | Delft sand | 1 marquise tsavorite 8 × 4 | heraldic shield with a comb standing on the spine | hackles turning into serpent scales in one skin; struck comb |
| 3 | **Fenrir** | the wolf and the moon | factory **010 Trillion**, 16 × 17 | lost wax | 1 moonstone cabochon 10 mm | fangs standing over the table | fang head, crescent jaws, fur ruff, Gleipnir braid |
| 4 | **Phoenix** | phoenix | keyframed Flat 6.6 × 3.4, flaring to 9.6 | Delft sand | 1 fire-opal cab 8 × 6 + 14 graded orange sapphires | flared breast, wings fanned on the sides | flame plumage, graded bead-set fire trail, side-face wings |
| 5 | **Corvus** | Huginn and Muninn | Bypass LowDome 6.0 × 2.8 | Delft sand | 1 onyx cabochon 8 × 6 | two arms passing | feathered bypass, struck beaks, onyx at the crossing |
| 6 | **Kraken** | kraken | keyframed DShape 6.0 × 2.7, flaring to 9.0 | lost wax | 1 labradorite cabochon 10 mm | tentacle-wrapped, stone held high | curve-layer tentacles with suckers, tentacle head |
| 7 | **Manticora** | manticore's tail | keyframed HighDome 5.4 × 2.2, rising to 3.4 | lost wax | 1 pear ruby 7 × 5 + ~12 graded black spinel princesses | tail arching to a hooked stinger | graded tail segments, twisted-sweep stinger, tilted graded run |
| 8 | **Harpyia** | harpy | Uniform Flat 7.2 × 3.8 | lost wax | 1 sapphire 8 mm | wings rising above the band, as on Hypnos | tiered sketch-region wings, talon head, cathedral legs |

Stones: 0 + 1 + 1 + 15 + 1 + 1 + 13 + 1 = **33**.

**How the eight differ.**
- **Bases.** Three are factory signets never used by an earlier collection: 002, 010 and 020. Reptilia, Stock masterworks and Caiman used 001, 006, 013, 015, 017 and 019. Four rings use keyframed or modulated procedural bodies. One is a uniform band carrying a CAD assembly.
- **Processes.** The four sand rings are Draco, Basiliscus, Phoenix and Corvus: every surface falls away from the parting line. The four lost-wax rings are Fenrir, Kraken, Manticora and Harpyia. Each of those carries something that overhangs by design: a claw over a stone, wings above the band, or a stinger over a pear. Each verdict is judged against the ring's own `DraftSettings::process`.
- **Wings three ways.**
  - Draco: membrane painted on a zero-draft table.
  - Phoenix: SVG plumage on keyframed side faces.
  - Harpyia: CAD sketch regions extruded as tiers.
  - A template user can compare all three.
- **Weapons three ways.**
  - Claw forms: talon, fang and tentacle, through one builder extension.
  - A twisted-sweep stinger.
  - Struck spines, beaks and a comb.
- **Stone work.** Every stone is made, not drawn:
  - flush burs: Basiliscus;
  - collets: Phoenix, Corvus and Manticora;
  - bead-set graded runs: Phoenix and Manticora;
  - claw-form heads: Fenrir, Kraken and Harpyia.

**Sheet layout.** The sand row is Draco, Basiliscus, Phoenix and Corvus. The wax row is Fenrir, Kraken, Manticora and Harpyia. Captions follow the Reptilia sheet: name, then epithet, then base and stone.

**Excluded, and why.**
- **Hydra:** its identity is its heads. Serpentarium's hydra heads were "a bit rough", because small lofted heads blur.
- **Gorgon:** a face. Her serpent hair would only be a texture, and one texture is not a ring.
- **Chimera:** a composite reads as Palisade's "random elements placed in various spots". The name is also taken by `ex/showoff.rs`.
- **Thunderbird:** a sacred figure to several Indigenous nations, and a poor fit for a commercial line. Its storm role goes to Harpyia, the Greek storm-snatcher.
- **Wyrm:** folded into Draco, which has wings.

---

## 2. The shared kit these rings are built from (all verified)

| Idiom | Where it lives | What it gives the bestiary |
|---|---|---|
| Stock atlas: 2048 × 768 3-D samples with normals | `sm:40-125` (`Atlas::new`, `face`, `cheek`, `shoulder`) | painting in metal mm on factory stock |
| Hide coordinates: `along` the parting line, `across` from it, per-column `rim` and `wall` | `sm:1573-1663`; `crest_at` interpolates the true z=0 crossing at `sm:1645` | rows that keep their size down a head's end wall |
| Draft clamp: relief may only fall walking away from the parting line | `sm:1303-1324`; `clamped()` reports the texels cut at `sm:1700` | the sand guarantee for every painted skin |
| Pointed scute: chevron on the parting line, drafted fall ≥ 0.45 mm | `sm:1331-1346` | the castable free edge that feathers and scales reuse |
| Round scales for walls facing the pull | `sm:1349-1363` | flank and cheek scales |
| Graded joints | `sm:1667-1696` | segment and plate rows |
| Max-joined painted layers keep the clamp's guarantee | `sm:1714-1722`, Doctrine § One theme per ring | layers stacked without re-proving release |
| Procedural band skin | `ex/reptilia.rs:43-149` (`Skin::new`, `supported`) | the same approach on a band, but it samples `d.reference_loop()` only (`ex/reptilia.rs:46`), so it is wrong on keyframed and bypass bodies |
| Sand stock | `ex/common/sand_stock.rs:53` | a drafted factory master with a real parting loop |
| Stamps | `core/setting.rs:897-925`: outline, height, sink, draft, `cut`, `bench`, `along_pull`; frame squared across the parting plane at `:932-959`; split on the parting line at `:1006-1043`; moons and crescent cutters at `:1071-1100` | struck spines, beaks, comb lobes, quills, jaws |
| Made settings on seats | `core/setting.rs:21-33` (`SolidKind` None / Flush / Bead / Prong / Bezel); per-seat `solid`, `through`, `mark_mm` at `core/field.rs:1188-1196` | flush, bead-set and collet stones |
| Graded, tilted runs | `core/field.rs:1917-1952` (`taper`, `taper_theta_deg`, `shared_prong_mm`, `tilt_deg`); eccentric-warp stations at `core/field.rs:1267`, `:2046` | fire trail, venom spinels |
| Keyframed bodies | `core/profile.rs:1747` (`ShankKey`: width, thickness, crown); capped at **16 keys** (`core/profile.rs:2504`); Catmull-Rom parameterized by knot angle (`:2515-2527`) | Phoenix, Kraken, Manticora |
| Bypass arms | `core/profile.rs:2321-2363` (`BYPASS_OFFSET` 0.45, `BYPASS_TIP_DEG` 35, tip length 30, `bypass_span`) | Corvus |
| CAD document | `core/cad.rs:33-143` (Extrude with negative cuts; Twist with `end_scale`; Loft; Pattern; Plane; Builder); `Placement::Ring` at `:392-404`, seated on the built band by `frame_on` (`:456`); `Attach` / `Stage` / `blend_mm` at `:293-316`; `Joint` at `:1277` | stinger, wings, heads |
| Builders | `core/cad/builders.rs:60-70`: `stone`, `head.claw`, `head.bezel`, `head.basket`, `seat.bur`, `halo`, `cutter.pierce`, `cutter.azure`, `shank.cathedral`; parameter schemas at `:147-195` | talon, fang and tentacle heads through a new `form` parameter |
| Sketch regions and Bézier profiles | `core/cad.rs:149-154` (`Profile::Region`); `core/sketch.rs:116-122` (Line, Polyline, Circle, Arc, Bezier); a Bézier extrusion is tested watertight at `core/cad.rs:3274-3303` | Harpyia's wing tiers |
| Work planes and mirrors | `core/cad/pattern.rs:37-83` (`PatternKind::Ring` / `About` / `Mirror`; `MirrorPlane::Band`; `PlaneBase::Parting` / `Tangent` / `Section`); ring copies re-seated per angle at `:252-278`; at most 120 copies (`:20`) | the second wing; stinger planes |
| Twisted sweep | `core/cad/twist.rs:361` (planar path, turned and scaled to `end_scale`); a sweep that crosses itself is refused at `:498` | the aculeus |

---

## 3. The rings

### 3.1 DRACO — *the wyvern displayed*

**Concept.** A wyvern lies along the ring with its spine on the parting line and its wings spread across the Kite's table to the two points at the rims. Its body coils round the finger, and at the palm its head is tucked under the spade of its own tail.

| Zone | What is there |
|---|---|
| Face (table) | Two membrane wings, one to each point of the kite. Three tall dorsal spines stand between them on the parting line. |
| Shoulders | The dorsal scute series graded down both shoulders, with a spine struck on each plate. |
| Walls and cheeks | Round imbricated scales where the wings meet the body, graded down the shank walls. |
| Palm | Ventral scutes. The lanceolate spade of the tail lies across the last scutes, over the tucked head. |
| Bore | Plain comfort fit. |

**Base.**
- `PRESETS` "002" Kite (`core/imported_base.rs:1096`).
- Put through `sand_stock` (`ex/common/sand_stock.rs:53`), then attached with `ImportedBase::attach` (`core/imported_base.rs:472`) and `sand_envelope = true` (`:60-62`).
- `head.length_mm = 13.0` along the ring (native 14). `profile.width_mm = 19.0` across the finger (native 25).
- Flat profile, `edge_round_mm 0.3`, `comfort_fit_mm 0.1`, bore 18.6.
- Take `SurfaceChart` from this stock before any painting (`sm:451-455`).
- 002 is the only factory plan whose long axis runs across the band (Table Width 14, Table Length 25 in `assets/decoded/presets/Signet Ring/002/params.json`). Its points sit at the rims, which is where the wing tips of a creature whose spine is the parting line belong.

**Process.** Sand, `mf::Recipe::sand(SandProcess::DelftClay)`, sterling, parting fixed at z = 0 (`sm:462-509`). Sand works because every form falls from the spine.

**Stones.** None. As with Varanus, the hide is the jewel.

**Construction.** Painted on the stock atlas and hide. Each layer goes through `clamped()` and then `hide_layer()` with `Blend::Max` (`sm:1700-1722`).
1. **"Wing membranes"** (table, relief 0.95 mm, window 90°, span = head). Four finger-bones per wing fan from a wing root near the spine: the leading bone runs straight to the kite's point, and the trailing bones sweep aft. The membrane is a stack of **stepped panels**:
   - Each panel descends away from the spine (sag 0.10 mm, concave).
   - Panels step down 0.14 mm from the leading edge to the trailing edge.
   - Each bone is the rounded **lip on the high side of a step**, standing 0.12 mm proud. It is never a free-standing ridge.
   - Walking outward from the spine, a column only ever steps down, so the clamp should cut close to nothing. Log it the way Caiman does (`sm:1710`).
   - The trailing edge is scalloped between the bone tips, each scallop a 1.2 mm bite dropping to the base. The panels stop at the hide's `rim` (`sm:1617-1619`).
2. **"Dorsal scutes"** (shoulders, 1.0 mm, window 90°, span 290°). Caiman's three-series dorsal armour (`sm:1826-1874`) redrawn as the wyvern's back:
   - pitch 3.0 mm at the head's ends, falling to 1.5 mm by the palm, rows from `Joints` (`sm:1745-1746`);
   - joints aligned across the band so every joint can run to full depth (Doctrine § One theme per ring, "joints aligned");
   - a flat "wing root" plate under the table spines.
3. **"Wing-root scales"** (cheeks and shank walls, 0.45 mm). `round_scales` (`sm:1349`), graded 1.9 mm to 1.1 mm. Walls facing the pull take any shape (`sm:1432`).
4. **"Belly scutes"** (palm, 0.30 mm). Caiman's belly: six a side at 2.1 mm, closing on a joint at 270° (`sm:1939-1954`).
5. **"Graver's veins and keels"** (`bench_only`, `Blend::Subtract`, 0.12 mm). Branching veins between the bones, scale keel lines and the spade's midrib. This follows the Zenith lesson that a sky's fine detail belongs to the graver.

**Stamps.**
- **"Dorsal spine, n"**:
  - Caiman's keel hexagon outline, points every 0.1 mm (`sm:1780-1792`), placed by `crest_at` (`sm:1645`).
  - Three on the table at 0.55 mm (half-length 1.3), then one per scute plate down both shoulders, grading 0.40 → 0.22 mm (`sm:1799`).
  - Sink 0.3, draft 4°.
  - Skip any plate within 1 mm of the end-wall fold (`sm:1757-1775`).
  - Each spine stands 0.6 mm clear of its plate's ends (`sm:1764-1769`).
- **"Spade tail, blank"** at 270°: a convex lanceolate, 5.4 × 2.8 mm, straddling the parting line.
- **"Spade tail, barbs cut at the bench"**: two `cut: true, bench: true` stamps take out the barb notches. This is the crescent method: cast the convex hull and cut the concavity (`sm:1467-1473`, `core/setting.rs:1091`).

**Showcases.**
- the stock atlas and hide at their fullest;
- draft-clamped figurative relief on a zero-draft table;
- graded struck spines;
- the cast-then-bench-cut concave outline;
- five layers joined by Max;
- a bench layer.

**Traps and how they are avoided.**
- **Anything proud off the parting line on a signet face locks** (Doctrine § One theme per ring). Bones are drawn as step lips, not ridges, and the draft clamp enforces the rule. If the clamp cuts more than about 0.05 mm anywhere, redraw the fan. Do not accept the comb marks.
- **The table is a zero-draft plane.** The sand envelope stays on as the guarantee (`sm:1404-1405`). Nothing is left for it to fill.
- **Stamps on the crest need the crest exactly.** Use `crest_at` (0.023 mm obstructions otherwise, Doctrine § One theme per ring).
- **The surface under a stamp may vary only across the band.** Plates stay flat along the ring (`sm:1845-1859`), which avoids the 0.03–0.07 mm phantoms.
- **The fold at the head's end wall.** No spine within 1 mm of it (0.06 mm otherwise, `sm:1770-1775`).
- **The concave spade notch faces back across the parting line** (the crescent lesson, Doctrine § A stamp is an outline). Cast the blank and bench-cut the notches.
- **Thin wall under a head 19 mm across.** Check `thinnest_wall_mm` against Delft's `min_section_mm`, and pull the width back toward 18 mm if it binds.

**Needs** (see §4).
- Buildable today inside an example, like Caiman.
- Templates and reuse need **E1** (hide in core) and **E5** (`base.preset` / stamp nodes).
- **E2** gives the spines a gable ridge; the fallback is Caiman's flat keel.
- **E7** (`beast::membrane`) keeps the painter reusable.

**Template.** It lifts as `caiman-imported.graph.json` does:
- `layer.tiling` + `alpha.png` × 5;
- a 3.04 MB `/imported_base` patch (measured on Caiman's graph);
- a `/stamps` patch holding about 19 stamps.
With E5 and E6 it becomes `base.preset` + `stamp.*` + `alpha.hide` nodes and weighs a few tens of kB. Expose: US size, face length, face width, spine height.

**Difficulty / risk.** 4/5, medium. The risk is how clearly the wings read at 0.95 mm once they are forced monotone.

---

### 3.2 BASILISCUS — *king of serpents*

**Concept.** The cockatrice: rooster hackles on the head that become serpent scales down the shoulders, ending in a snake's belly at the palm. Its comb, the crown that gives the basilisk its name, is struck along the spine, with a marquise as the comb's jewel.

| Zone | What is there |
|---|---|
| Face | Lanceolate hackles radiating from the spine to the rims. Six comb lobes on the parting line around a flush marquise. |
| Shoulders | The same skin morphing from hackle to keeled scale between 25° and 75° from the head. |
| Walls and cheeks | Round and keeled serpent scales. |
| Palm | Ventral scutes. |
| Bore | Plain. |

**Base.** `PRESETS` "020" Escutcheon (`core/imported_base.rs:1114`) through `sand_stock`. `head.length_mm 16.0`, `profile.width_mm 16.5`, bore 18.6. The escutcheon's cusped upper edge is the heraldic shield the cockatrice belongs on.

**Process.** Delft sand, sterling.

**Stones.** One tsavorite **marquise 8 × 4**. The marquise is chosen because its plan is native to the seat system (`GemCut::plan_pow` = 1.5, `core/gem.rs:163`). A trillion would be seated as a superellipse (3.2, `core/gem.rs:157`); see E13.

**Construction.**
1. **"Hackles"** (table, 0.70 mm):
   - lanceolate feathers with their long axis across the band and their tips leading to the rims;
   - rows shingled so each feather sits under the one nearer the spine, which gives monotone steps outward;
   - a rachis keel, and the free edges drafted like the scute's `fall` (`sm:1340-1343`);
   - pitch 1.6 mm on the table;
   - a flat "comb seat" strip, 1.2 mm wide, on the parting line under the comb.
2. **"Hackle to scale"** (shoulders, 0.55 mm, window 90° ± 80°). The same series continues while each feather's length-to-width ratio morphs from 3.2 to 1.2 and its rachis becomes a keel, ending in `reptile::snake`'s form (`core/reptile.rs:9-28`) at 0.84 mm pitch. The hand-over happens inside one skin, not as layers side by side (`sm:1215-1219`).
3. **"Serpent flanks"** (walls, 0.40 mm). `round_scales` blending into `reptile::shields` (`core/reptile.rs:55`) toward the palm.
4. **"Ventral scutes"** (palm, 0.30 mm). `reptile::ventral` (`core/reptile.rs:32`) pitched at 2.0 mm.
5. **"Graver's barbs and keels"** (`bench_only`, Subtract, 0.10 mm).

**Stamps.** **"Comb lobe, n"** × 6, three either side of the stone:
- Caiman keel outline with blunt, rounded ends; half-length 1.1, half-width 0.55, `tip` 0.09 (`sm:1777-1784`);
- heights 0.55 / 0.45 / 0.32 mm going outward, which gives the comb's profile when seen side-on;
- placed by `crest_at` on the comb seat, 0.6 mm clear of the seat's ends, and not on the fold.

**Stone seat.**
- `SeatPadLayer`: `style Boss`, `crown 0.15`, `blend_mm 0.4`, `metal_true`, `solid SolidKind::Flush`, `through: true`, `height_mm 0.55`. The boss is exactly the comb's height, so it reads as the comb's central lobe.
- `rot_deg 0`, so the long axis lies along the ring. `Blend::Max`.
- This is Caiman's emerald idiom (`sm:2008-2025`).
- Show it finished with the stone. Under sand the pattern carries the raised drill mark (`core/field.rs:1196`).

**Showcases.**
- two creatures in one skin, handed over inside a single painter;
- a struck comb whose profile is carried by graded stamp heights;
- flush made setting;
- heraldic factory stock.

**Traps.**
- **Hackle tips must lead away from the parting line** (the pointed-scute rule, `sm:1331-1334`). Tips turned back onto the spine lock.
- **Walls facing round the ring lean wherever the section is still changing width.** Flutes leaned 1–10° on a widening shoulder (Doctrine § Two masterworks). The hackle and scale free edges carry drafted falls ≥ 0.45 mm, and the shoulder rows are verified with `castability::attribute_undercuts` (`core/castability.rs:1562`).
- **A gem column must run along the parting plane** (Doctrine § collection3 / commissions). The marquise straddles it.
- **Crest precision and surfaces flat along the ring under stamps.** The comb seat strip handles both (see Draco).

**Needs.** E1, E5, E7 (`plumage::hackle`, `plumage::morph`). E2's `Dome` top for rounded comb lobes is optional.

**Template.** As Draco: five painted alphas, stock patch, stamps patch, seat node. Expose: US size, face length, face width, comb height.

**Difficulty / risk.** 3.5/5, medium-low. The morph is the new part; everything else repeats Caiman.

---

### 3.3 FENRIR — *the wolf and the moon*

**Concept.** Fenrir's line swallows the moon. Two crescent jaws close fore and aft on a moonstone, and four fangs curve over its dome. Fur streams from the jaws over the head's walls and down both shoulders. At the palm the wolf is bound by Gleipnir, the silken fetter.

| Zone | What is there |
|---|---|
| Face | Crescent upper and lower jaws (stamps) with their concave sides embracing the stone; four fangs (claw head); moonstone. |
| Head walls, shoulders | Ruff of S-curved fur locks, and finer guard hairs over them. |
| Palm | The braided Gleipnir, knotted at both ends where it meets the fur. |
| Bore | Plain. An optional CAD hollow under the head (see below). |

**Base.**
- `PRESETS` "010" Trillion (`core/imported_base.rs:1104`) **without** `sand_stock`, as for the lost-wax Nocturne and Vesper (`sm:417-439`).
- `head.length_mm 16`, `profile.width_mm 17`, bore 19.0.
- The trillion's wedge is the wolf's head.

**Process.**
- Lost wax: `CastProcess::LostWax`, `min_draft 0`, `min_detail 0.15`, `min_section 0.8` (`sm:477-482`).
- The fangs overhang the stone. The crescent jaws' concave walls would lock in sand ("A crescent cannot be cast raised", Doctrine § A stamp is an outline).

**Stones.** One moonstone round **cabochon 10 mm** (`Gem::cabochon`, `core/gem.rs:250`). A cabochon standing proud needs no bur (`core/cad/builders.rs:757-760`).

**Construction.** Painted on the stock atlas. There is no draft clamp in wax, but every strand tip is at least 0.25 mm for the 0.15 mm detail floor.
1. **"Ruff"** (0.8 mm, window 90° ± 110°). S-curved lanceolate locks, each with a centre groove, in three offset rows. They run from the table's rim down the walls and back along the shoulders, painted in hide coordinates (`along`, `across`, `wall`).
2. **"Guard hairs"** (0.35 mm, `Blend::SmoothMax`, `soft_mm 0.2`, `core/field.rs:386`). Finer strands on the crests of the locks.
3. **"Gleipnir"** (palm, 0.45 mm). `Procedural::Braid` (`core/alpha.rs:644`) tiled with `Window::around(270, 70)` and repeats set so every strand is ≥ 0.3 mm. DFM measures texture strokes by granulometry (Doctrine § Manufacturing analysis, `dfm::findings_in`).
4. **"Binding knots"** (0.4 mm). A `DecalLayer` (`core/field.rs:1730`) of `Procedural::CelticKnot` (`core/alpha.rs:654`) at 235° and 305°, where the fetter's ends meet the fur.
5. **"Graver's hair lines"** (`bench_only`, Subtract).

**Stamps.** **"Jaw, upper"** and **"Jaw, lower"**:
- `moon_outline(5.8, 0.30, 0.62)` (`core/setting.rs:1071`), turned 0° and 180° so the horns point at each other round the stone;
- height 0.7, sink 0.35, draft 2°, straddling the parting line fore and aft;
- the jaws are two crescent moons closing on the full one.

**CAD document.**
- F1 `Band`.
- F2 `builders::stone_feature(gem, Placement::ring(90, stand_off_mm(CLAW, gem)))` (`core/cad/builders.rs:916`, `:487`).
- F3 `head.claw` `{prongs: 4, form: "Fang", grouping: "Jaws", wire_mm: 1.5}`, `Attach::Join`, `Stage::Cast`, `blend_mm 0.25`. This needs **E3**.
- Optional F4: a hollow under the head. A sketch of the trillion inset by 1.2 mm on a `Plane{Section…}`, extruded 2.0 mm up from the bore as `Attach::Cut`. A signet this size in 18k needs it; it is lost wax only, and it must hold `min_section 0.8`.

**Showcases.**
- claw builder as literal fangs;
- stamps that are concave on purpose, which lost wax allows;
- a procedural braid as a narrative element;
- painted fur on stock;
- a CAD cut on imported stock.

**Traps (wax).**
- **Fang tubes fold wherever their rings tilt more than they stand apart** (Doctrine § A seat can carry a made part). Every bend radius must be at least the local tube radius, and `csg::self_crossings` must be 0.
- **Fangs must not break the bore wall.** `claw_head_within` refuses them (`core/setting.rs:697-709`).
- **Cabochon crown slope.** Claws meet it at inward 0.30 and rise 0.71 of the crown (`core/setting.rs:749-752`).
- **Strand tips under the 0.15 mm floor read as mush.** Taper to 0.25 mm and confirm that `dfm::findings_in` is clean.
- **The jaws' concavity is only legal in wax.** The template's process must stay LostWax. Its sand counterpart would be the crescent-cutter method.

**Needs.** **E3** (Fang form and Jaws grouping) is critical. E1, E7 (`beast::fur_lock`), E5.

**Template.**
- an `/imported_base` patch of the unmodified preset (~0.45 MB, the 8.8 MB / 20 average), which `base.preset` removes;
- `alpha.png` × 2 and `alpha.proc` × 2;
- a `cad.feature` chain (`graph/nodes/cad.rs:254`);
- a small `/stamps` patch.

**Difficulty / risk.** 3.5/5, medium. It depends on E3.

---

### 3.4 PHOENIX — *reborn*

**Concept.** A phoenix wraps the finger. Its flaming breast is at the top, around a fire opal. A trail of graded embers runs down its spine on both shoulders. Its wings enfold the band in opposite directions on the two side faces, and its tail streamers cross at the palm.

| Zone | What is there |
|---|---|
| Top | Fire-opal cabochon in a made collet, on the flared breast. |
| Crown, shoulders | Flame contour feathers as chevrons pointing toward the palm; a graded bead-set sapphire run down the parting line on each shoulder. |
| Side faces | Low face: the left wing sweeping west. High face: the right wing sweeping east. Primaries, secondaries and coverts in three stepped tiers. |
| Palm | Tail streamers from both wings crossing on both faces; small contour feathers on the crown. |
| Bore | Comfort 0.2. |

**Base.**
- Procedural. `ProfileStyle::Flat` 6.6 × 3.4, then `flatten_sides()` (`core/profile.rs:548`). Square faces are where the wings go, and their usable height is thickness minus crown.
- `ShankKind::Keyframes`, `amount 1.0`, ten keys, within the 16-key cap (`core/profile.rs:2504`). Values are width / thickness / crown:

| θ (°) | width | thickness | crown |
|---|---|---|---|
| 90 | 1.45 | 1.22 | 0.85 |
| 62 and 118 | 1.36 | 1.18 | 0.90 |
| 28 and 152 | 1.16 | 1.07 | 1.00 |
| −20 and 200 | 1.00 | 1.00 | 1.00 |
| −60 and 240 | 0.92 | 0.95 | 1.00 |
| 270 | 0.88 | 0.92 | 1.00 |

**Process.** Delft sand, sterling.

**Stones.**
- **Fire opal, oval cabochon 8 × 6**: `Gem { l_mm: 8.0, ..Gem::cabochon(GemCut::Oval, 6.0) }`.
- **14 orange sapphires**, rounds graded from 2.2 mm to about 1.1 mm.

**Construction.**
1. **"Flame plumage"** (crown, relief 0.45 mm, window 90°, span 250°). Painted on the modulated body's atlas, which needs **E1**. `ex/reptilia.rs:46` samples only the reference loop, which is wrong on keyframes.
   - Rows are chevrons whose point rides the parting line and leads toward the palm, following Saurian's scute rule (`sm:1418-1428`).
   - Each free edge is a flame-tongue scallop, drafted with `fall = (0.45/pitch).min(0.3)` (`sm:1340`).
   - Graded from 2.2 mm at the breast to 1.2 mm at ±120°.
   - A flat "ember seat" strip, 1.8 mm wide, runs on the parting line under both runs.
   - `draft_clamp`, `Blend::Max`.
2. **"Wings"** (side faces, 1.0 mm). A `DecalLayer` of an `alpha.svg`, **"Phoenix wing"**:
   - gradient-shaded feathers in three stepped tiers; gradient SVGs are the texture family that holds the detail floor (Serpentarium lesson);
   - one decal per face, `size_mm ≈ 24`, centred at θ 30° on one face and θ 150° on the other;
   - `LayerEntry.window.v_gate = VGate::SideFaces(Low)` or `(High)` (`core/field.rs:420-431`);
   - `Decal::flip = true` on the high face (`core/field.rs:1710`); the chart reads true from −Z (Doctrine § The showcase is the measured tour);
   - the SVG is drawn squashed by `FieldContext::station_stretch` (`core/field.rs:153`) at the decal's centre, the cloud-curl move, because decals stretch with a keyframed section.
3. **"Tail streamers"** (side faces at the palm, 0.8 mm). A `DecalLayer` of `alpha.svg` **"Phoenix tail"**: long flame-tipped plumes crossing at 270° on both faces.
4. **"Barbs"** (`bench_only`, Subtract, 0.10 mm). Vane striations on the crown feathers.

**Seats.**
- **"Fire opal, collet"**:
  - `SeatPadLayer{ theta 90, v crest, style Boss, crown 0.3, height 0.7, blend 0.55, metal_true, solid SolidKind::Bezel }` plus `fit_stone` (`core/field.rs:1349`);
  - finished with the collet; the sand pattern leaves the head out and raises `mark_mm` (`core/setting.rs:1295`; Doctrine § Shown finished, exported as the pattern).
- **"Fire trail, east"** and **"Fire trail, west"**: two `SeatRunLayer`s (`core/field.rs:1917`):
  - `gem` Round 2.2, `taper 0.5`, `taper_theta_deg 90`, `bridge_mm 0.75`;
  - `seat` GypsyMound, height 0.5, crown 1.0, blend 0.5, `seat.solid = SolidKind::Bead`;
  - `solve_spacing` (`core/field.rs:1999`);
  - each windowed `Window::around(90 ± 50, 70)` with fade 6, so the gap at the top is the opal's;
  - neighbours share beads (`core/setting.rs:1380-1407`).

**Showcases.**
- keyframed sculpture;
- a modulated-body atlas;
- SVG plumage on side faces with flip and stretch handled;
- made collet;
- graded bead-set runs;
- bench layer.

**Traps.**
- **Walls facing round the ring lean on a widening section.** The flare runs from ±28° to ±118°. Keep the plumage relief at 0.45 mm or less with drafted falls, and check with `attribute_undercuts`, which names the culprit layer (Doctrine § Manufacturing analysis).
- **A stone is sized to its face.** A gypsy seat carries 1.8 mm of stock round its stone. 2.2 + 1.8 = 4.0 mm, against a crown about 8 mm wide at the shoulders, which is fine (Doctrine § collection3).
- **The seat's skirt is its finest DFM feature.** A 0.4 mm skirt measured 0.34 against the 0.35 floor, so use `blend_mm ≥ 0.5`.
- **Bridge at the small end of the grade.** At 0.5 the smallest stations bridged 0.57 mm against a 0.7 mm fill, so ask 0.75 (`ex/serpentarium.rs:648-649`).
- **A gem column must run along the parting plane.** Both runs sit on the crest (Doctrine § collection3 / commissions).
- **A v-gate's fade is a wall facing the crest.** Side-face gates use the inward `SIDE_GATE_FADE_MM` (`core/field.rs:416`). Never use a crown `Band` gate for the plumage.

**Needs.**
- **E1 with modulated sampling**, which is critical for the crown plumage.
- E7 (`plumage::contour`).
- SVG set **"Phoenix wing"** and **"Phoenix tail"**: new art, gradient family.
- Everything else exists.

**Template.**
- The procedural body lifts fully: profile node, shank node carrying keys.
- `alpha.png` × 2 (plumage, barbs) and `alpha.svg` × 2.
- `layer.seat` and `layer.seatrun` × 2.
- Estimated 1.5–2 MB.
- Expose: size, width, thickness, flare (key 90 width). Until E6 exists, the painted plumage is bound to the flare it was painted on, so exposing the flare only makes sense once the plumage is re-painted at evaluation.

**Difficulty / risk.** 4/5, medium.

---

### 3.5 CORVUS — *Huginn and Muninn*

**Concept.** Odin's two ravens fly past each other on a bypass. Each arm is one bird, feathered from beak to tail. Their beaks, struck on the spine either side of a black onyx, point away from it: *thought* and *memory* sent out. Their tails cross at the palm.

| Zone | What is there |
|---|---|
| Top | Onyx cabochon in a collet at the crossing; two struck beaks on the parting line at ±14–30° pointing outward. |
| Crown | Contour feathers flowing from each beak back along its arm, graded from 0.9 to 2.2 mm. |
| Side faces | Folded primaries laid along the ring; the bypass's own seam channel at the crossing (`BYPASS_GROOVE_MM` 1.0, `core/profile.rs:2332`). |
| Palm | Two wedge tails interleaved. |
| Bore | Comfort 0.2. |

**Base.**
- `ProfileStyle::LowDome`, 6.0 × 2.8, comfort 0.2.
- `ShankKind::Bypass`, `amount 1.0`.
- Each arm slides to ±0.45 of the half-width over 100° to 30° before the top, and rounds to its tip over the 30° before 35° past it (`core/profile.rs:2321-2345`).

**Process.** Delft sand, sterling. A bypass measured 0.0085% at −0.8° on a low dome (Doctrine § Pavé… Bypass).

**Stones.** One black onyx **oval cabochon 8 × 6**.

**Construction.** Painted on a modulated atlas, which needs **E1**. The arms slide, so a fixed-v strip rides a moving flank; Uraeus measured 2–4° of lean (`ex/serpentarium.rs:417-420`).
1. **"Contour feathers"** (crown, 0.5 mm):
   - chevron rows pointing toward the palm, rachis on the crest;
   - graded 0.9 mm at the heads, rising to 2.2 mm on the backs;
   - each bird's feathers flow from its own beak (arm ownership from `bypass_span`, `core/profile.rs:2351`);
   - a flat "head plate" under each beak, flat along the ring;
   - `draft_clamp`.
2. **"Folded primaries"** (side faces, 0.8 mm). Long remiges along the ring, shingled, tips toward the palm, `VGate::SideFaces(Both)`. The Serpentarium lesson: a moving side face needs the gate.
3. **"Crossed tails"** (palm crown, 0.5 mm). The rectrices lie along the ring, shingled so the central pair is highest and each outer feather steps down. That is the only arrangement in which a feather's edge running along the ring falls away from the parting line. Two tails interleave at 270°.
4. **"Graver's barbs"** (`bench_only`, Subtract).

**Stamps.** **"Beak, Huginn"** and **"Beak, Muninn"**:
- lanceolate, 5.2 × 2.0 mm, blunt tip;
- straddling the parting line at TOP −(14…30)° and TOP +(14…30)°;
- height 0.45, sink 0.3, draft 4°;
- placed with a `crest_at` computed on the modulated atlas;
- a gable top along the parting line reads as the culmen, which needs **E2**. Each facet faces its own mould half, as Caiman's keel does (`sm:1845-1847`).

**Seat.** **"Onyx, collet"**:
- `SeatPadLayer{ theta TOP, v crest, style GypsyMound, height 0.55, crown 1, blend 0.45, metal_true, solid SolidKind::Bezel }` with a cabochon gem;
- Uraeus's crossing cab is the precedent (`ex/serpentarium.rs:389-399`); "a cab has no pavilion, so the crossing wedge costs it nothing".

**Showcases.**
- bypass shank as two creatures;
- painting on a sliding modulated body;
- struck beaks with a gable culmen;
- made collet;
- side-face gating.

**Traps.**
- **Arms slide.** Everything is painted in 3-D and draft-clamped, with no fixed-v rows. Milgrain on wandering crests measured 3% at 50° (Doctrine § The configurator), so there are no bead lines.
- **The surface under a stamp may vary only across the band.** The union section changes along the ring at ±14–30°, so the painted head plate flattens it along the ring. Verify at 384 × 192 and at build resolution, as Caiman does, because phantoms move with resolution while real obstructions converge.
- **A bypass crossing needs room under a stone.** Clean at 5.0 mm (Doctrine § collection3); 6.0 is used.
- **The tails' along-ring edges** are legal only as outward-descending shingles (see layer 3).

**Needs.** **E1 (modulated)** is critical. **E2** for the culmen; the fallback is a flat keel, which reads as a mask plate rather than a beak. E7 (`plumage::contour`, `plumage::remex`, `plumage::tail_fan`).

**Template.** A procedural body with a Bypass shank node; `alpha.png` × 4; a seat node; a two-stamp patch. Estimated 2–3 MB. Expose: size, width, thickness.

**Difficulty / risk.** 3.5/5, medium. The risk is how clearly the beaks read.

---

### 3.6 KRAKEN — *the deep's grip*

**Concept.** Six arms wrap the finger, crossing over the crown. Their tips rise at the top and curl round a labradorite as its claws. The mantle's cellular skin covers both side faces.

| Zone | What is there |
|---|---|
| Top | 10 mm labradorite cabochon in a six-tentacle claw head, raised on the flared mantle. |
| Crown, shoulders, palm | Six tapering tentacles, three curve layers each mirrored across the band, crossing diagonally with sucker rows on their inner curl. |
| Side faces | Mantle skin in fine Voronoi cells. |
| Bore | Comfort 0.2. |

**Base.**
- `ProfileStyle::DShape`, 6.0 × 2.7, comfort 0.2.
- `Keyframes`, nine keys (width / thickness): 90: 1.5 / 1.3 (crown 0.9); ±30: 1.3 / 1.15; ±80: 1.05 / 1.02; ±140: 0.92 / 0.96; 270: 0.86 / 0.92.

**Process.** Lost wax, sterling or 14k. Tentacles crossing the crown diagonally would lean in sand (off-crest rails, 5.8% at 29°, Doctrine § Templates are code). Tentacle claws overhang the stone.

**Stones.** One labradorite round **cabochon 10 mm**, sitting proud. No bur is needed.

**Construction (height field).**
1. **"Tentacles A / B / C"**: three `CurveLayer`s (`core/curve.rs:62-80`):
   - `repeats_around 1`, `mirror_v true`, which gives six arms;
   - up to 64 control points (`core/curve.rs:17`), spiralling from each claw foot across the crown to the palm and round;
   - `profile Round`, a cosine dome, because a circular section has a vertical wall (`core/curve.rs:44-58`);
   - width 2.6 → 0.45 mm and height 1.2 → 0.3 mm **per point**, which needs **E8**;
   - `Blend::SmoothMax`, `soft_mm 0.25`, so crossings are filleted by the tie-exact `smax` (`core/field.rs:386`).
2. **"Suckers"**: bead rows along each tentacle, pitch 0.75, Ø 0.55 → 0.22 mm, offset toward the curl's inside. This needs **E8**.
3. **"Mantle skin"**: `Procedural::Voronoi` (`core/alpha.rs:669`), side faces only (`VGate::SideFaces(Both)`), height 0.2, cells about 1.2 mm.

**CAD.**
- F1 `Band`.
- F2 stone at `Placement::ring(90, stand_off_mm(CLAW, gem))`.
- F3 `head.claw{prongs: 6, form: "Tentacle", wire_mm: 1.4}` (**E3**), `Attach::Join`, `Stage::Cast`, `blend_mm 0.25`. The seam bead is laid along every seam the boolean reports (`core/parts.rs:1-6`).
- Claw feet sit at `Plan::claw_angles(6)` (`core/setting.rs:139-147`). Each curve layer starts at its foot's (θ, v), so the arms in the height field become the claws.

**Showcases.**
- curve layers as anatomy;
- tapered wires with bead rows;
- procedural Voronoi skin;
- tentacle claw head;
- the whole ring built from nodes, with no painted atlas. This is the collection's lightest and most instructive template.

**Traps (wax).**
- **Tentacle claws curl tighter than their base.** This is legal only because the radius shrinks along the curl: a bend radius must stay at least the local radius (Doctrine § A seat can carry a made part), with `self_crossings` 0.
- **Crossing tentacles under plain `Max`** leave knife valleys under the 0.15 mm floor. Use SmoothMax.
- **Sucker beads under 0.22 mm** fail granulometry.
- **Claws must keep the bore wall** (`claw_head_within`).

**Needs.** **E3** (Tentacle form) and **E8** (width and height profile, bead rows) are both critical.

**Template.**
- Everything lifts as nodes: profile, shank keys, `layer.curve` × 3, `alpha.proc` Voronoi, a `cad.feature` chain.
- About 30 kB.
- Expose: size, flare, tentacle height, stone size (the builder's stone parameters).

**Difficulty / risk.** 3/5, medium-high. The ring is only as good as E3 and E8.

---

### 3.7 MANTICORA — *the tail that throws*

**Concept.** The ring is the manticore's scorpion tail. Its segments grow from the palm to a swollen venom bulb at the top, where a pear ruby sits in a collet like a drop of venom. The hooked stinger rises behind it and curls over the stone. Its throwing quills bristle down both flanks, and a graded line of black spinels runs down the spine on the diagonal.

| Zone | What is there |
|---|---|
| Top | Pear ruby in a collet, in the keyframed telson bulb; the twisted-sweep stinger hooked over it. |
| Crown | Graded tergites with two dorsal keels; a tilted, graded princess run on each shoulder. |
| Flanks | Quills struck two per segment per side. |
| Side faces | Pleural folds aligned to the segment joints. |
| Palm | The smallest segments. |

**Base.**
- `ProfileStyle::HighDome`, 5.4 × 2.2.
- `Keyframes`, 11 keys (width / thickness): 270: 0.92 / 0.90; ±150: 0.96 / 1.0; ±105: 1.04 / 1.14; ±60: 1.10 / 1.30; ±25: 1.16 / 1.48; 90: 1.20 / 1.55.
- Keys cannot carry the segments themselves (16-key cap), so segments are relief.

**Process.** Lost wax, 14k or 18k. The stinger overhangs.

**Stones.**
- **Pear ruby 7 × 5.** Today a pear is seated as an ellipse (`core/gem.rs:156`; `core/setting.rs:105-113`); see E13. The fallback is an oval.
- **About 12 black spinel princesses**, 2.0 mm, graded.

**Construction.**
1. **"Tergites"** (0.6 mm):
   - graded plates from `Joints`, pitch 3.2 → 1.7 mm, spaced by the eccentric law the seat run uses (`core/field.rs:1267`) so the series closes (E11);
   - each plate a loaf (`sm:1850-1859`) with a posterior lip and two carinae either side of the crest;
   - joints 0.35 mm deep.
2. **"Pleural folds"** (side faces, 0.3 mm): transverse wrinkles aligned to the same joints.
3. **"Granulation"** (0.12 mm): granules ≥ 0.25 mm on the plates.
4. **"Graver's carina lines"** (`bench_only`).

**Stamps.** **"Quill, n"**, about 40 of them:
- lanceolate, 1.4 × 0.45 mm, height 0.35, sink 0.25, draft 0;
- bearing turned back toward the palm;
- two per tergite per flank, graded with the tergites.

**Seats.**
- **"Venom spinels, east"** and **"Venom spinels, west"**: two `SeatRunLayer`s:
  - Princess 2.0, `tilt_deg 45`, `taper 0.55`, `taper_theta 90`, `bridge 0.8`;
  - seat GypsyMound 0.45, `solid Bead`;
  - windowed `around(90 ± 70, 90)`.
  - This is Diamondback's construction (`ex/serpentarium.rs:648-668`), which Logan loved.

**CAD.**
- F1 `Band`.
- F2 stone (pear ruby), at `Placement::ring(90, stand_off_mm(BEZEL, gem))`.
- F3 `head.bezel{wall_mm 0.55, lip 0.35}`.
- F4 `seat.bur{through: false}`.
- F5 `Plane{Tangent{theta 97, across 0}}`.
- F6 `Sketch` "Aculeus section" on F5: a keeled Bézier teardrop, 1.8 × 1.3 mm, square to the path's start as the twist requires (`core/cad/twist.rs:360`).
- F7 inline path sketch on the default workplane, which *is* the parting plane: a 1.2 mm radial rise at θ 97°, then an `Arc` of radius 3.6 over 120° curving forward over θ 90°, so its tip hangs 1.2 mm above the pear's point.
- F8 `Twist{sketch: Profile::Feature{F6}, path: F7, degrees: 18, end_scale: 0.14}` (`core/cad.rs:76-81`).
- The aculeus is **`Attach::Separate`**, with `cad::Joint{a: band, b: F8, method: "Solder after the pear is set"}` (`core/cad.rs:1277`; precedent `core/cad/examples.rs:66-72`). Under lost wax a `Stage::Bench` part is still cast in place (`core/castability.rs:1455-1458`). Only Separate gives the setter a clear bezel to burnish.

**Showcases.**
- keyframed tail;
- graded relief segments;
- twisted sweep with taper;
- Separate part with a solder joint;
- tilted graded made-bead run;
- many struck quills (reel family "Quill", `crates/ringdesigner-android/src/reel.rs:75-80`).

**Traps (wax).**
- **A twisted sweep that crosses itself is refused** (`core/cad/twist.rs:498`). Ease the arc and keep `degrees` at 20 or less.
- **The stinger tip under the 0.15 mm floor.** `end_scale 0.14` on a 1.3 mm section leaves about 0.18 mm, so blunt the tip.
- **Pear seated as an ellipse** leaves gaps at the shoulder and the point poking through the collet. Wait for E13 or use an oval.
- **A tilted run at an oblique bearing leans.** Oblique at 30° leaned 5–6° (Showoff lesson), so stay at 45°.
- **`bridge_at` reports the solver's figure only within 3%** (Doctrine § Pavé). Ask 0.8 against the 0.8 section floor.

**Needs.** **E13** (true pear plan) or the oval fallback; E11 (a public eccentric graded `Joints`); E1, E7 (`beast::tergite`). E10 would give CAD quills in place of stamps; optional.

**Template.**
- Procedural body nodes, `alpha.png` × 3, `layer.seatrun` × 2;
- a `cad.feature` chain of 8 features including the two work planes and the twist;
- a `/stamps` patch of about 40 quills (E5 turns it into a `stamp.array`).
- Estimated 1.5 MB.

**Difficulty / risk.** 4/5, medium.

---

### 3.8 HARPYIA — *the snatcher*

**Concept.** A harpy perches at the top with her talons locked on a storm-blue sapphire and her legs standing as two arches. Her wings rise from beside the stone above the band, as Hypnos's rise from his temples. They sweep back along both side faces in stepped tiers of feathers. Her back feathers cover the crown, her tail fans at the palm, and scaled legs run down the forward shoulder.

| Zone | What is there |
|---|---|
| Top | 8 mm sapphire; six talons (two feet of three); cathedral arches as legs. |
| Side faces | Two CAD wings, the second a mirror of the first across the band: coverts, secondaries and primaries as tiered extrusions, rising up to 3.5 mm above the crest at the wrist. |
| Crown (west) | Contour back feathers. |
| Crown (east) | Scutellate leg scales under the forward arch. |
| Palm | Tail fan. |

**Base.**
- `ProfileStyle::Flat`, 7.2 × 3.8, comfort 0.2, `flatten_sides()`, `ShankKind::Uniform`, bore 19.0.
- The band is **uniform on purpose**. Its side faces are planes at z = ±3.6, so a sketch on a work plane lies flush on them everywhere. On a varying band that would need E9.

**Process.** Lost wax, 18k. A wing plate standing beyond the band's radius above the parting plane has an underside facing the drag, which is an undercut in sand. Only a full-depth flare is legal there.

**Stones.** One sapphire, round **8 mm** (`Gem::calibrated`).

**CAD document.**
- F1 `Band`.
- F2 stone at `Placement::ring(90, stand_off_mm(CLAW, gem))`.
- F3 `head.claw{prongs: 6, form: "Talon", grouping: "Feet", wire_mm: 1.3}` (**E3**).
- F4 `shank.cathedral{spread_deg 34, rise 0.7, wire_mm 1.3}` (`core/cad/builders.rs:188-192`): "two wire arches rising from the band's shoulders … to the underside of its head's gallery rail", which here are her legs.
- F5 `seat.bur{through: true}`.
- F6 `Plane{PlaneBase::Parting, offset_mm: 3.3}`, 0.3 mm inside the side face so the plate joins (`core/cad/pattern.rs:523-574`).
- F7 `Sketch` "Wing", anchored to F6 through `FaceAnchor{feature: F6}`; a work plane resolves as a frame (`core/cad.rs:1596-1599`). The outline is drawn in world XY:
  - the root runs along r 10.2 from θ 104°;
  - the leading edge rises to the wrist at r 16.8, θ 118°;
  - nine primary scallops as `Bezier` fall from θ 120° to 228°;
  - the trailing edge returns at r 12.6 → 10.4.
  - Interior lines T-junction into the outline and divide it into a coverts region, a secondaries region and nine primary regions (`core/sketch/region.rs:1-2`, "where its curves branch, the faces they divide the plane into").
- F8 `Extrude{Profile::Region{F7, coverts}, 1.55, draft 2°}`, F9 secondaries at 1.15, then F10–F18 each primary from 0.95 down to 0.75 toward the tip. `RegionRef` names each region by an outer-loop entity and an interior point (`core/sketch/region.rs:57-68`).
- F19 a union of the tier extrudes (kernel booleans of analytic prisms), then F20 `Pattern{source: F19, kind: Mirror{plane: MirrorPlane::Band}}` for the second wing. If the kernel union is slow, mirror each tier instead.
- All parts `Attach::Join`, `Stage::Cast`, `blend_mm 0.15`.

**Height field.**
1. **"Back feathers"**: `alpha.svg` gradient contour-feather tile on the crown, `Window::around(165, 120)`, 0.4 mm.
2. **"Tail fan"**: `alpha.svg` rectrix fan, `around(250, 50)`, 0.5 mm.
3. **"Scaled tarsus"**: `Procedural::ReptileShields` (`core/alpha.rs:652`), `around(30, 90)`, 0.3 mm.

**Showcases.**
- the CAD modelling pipeline at its fullest: work plane, a sketch with regions and T-junctions, Bézier, region extrudes, draft, mirror pattern, joins with seam beads, cathedral builder, bur;
- talon claws;
- a node-only template, like Kraken.

**Traps (wax).**
- **Kernel booleans of faceted operands past 500 faces are refused** (Doctrine § CAD parts stand on the band). The tiers are analytic Bézier prisms, not faceted. Keep them as kernel parts until the join, which runs through `csg` in milliseconds.
- **Talon tubes, the fold rule and `self_crossings`** (as Fenrir).
- **Wing root against talons and arches.** The root starts at 104°; the talons stand within ±6° and the arches at ±34° on the crown. Check the distances in the viewport's section pane.
- **Plate thickness beyond the band.** The free part is 0.9 + 0.3 mm, over the 0.8 mm section floor.
- **Draft on Bézier walls.** `draft_deg` must be accepted on curved region extrudes. If the kernel refuses it, drop to 0°; lost wax does not need it.

**Needs.** **E3** (Talon form and Feet grouping) is critical. E9 is only needed for a non-uniform variant.

**Template.**
- Profile node, `alpha.svg` × 2, `alpha.proc` × 1;
- a `cad.feature` chain of about 20 features, including the sketch that users open in the CAD workspace;
- about 80 kB.
- Expose: size, width, stone size, talon wire.

**Difficulty / risk.** 4.5/5, high. It carries the most CAD features of any ring so far, and relies on the talon builder and on how region extrudes behave at scale.

---

## 4. Enablers, ranked

Ranked by rings unblocked, times how shared they are, divided by cost. **[S]** means shared with the other three collections designed alongside this one.

| Rank | Enabler | Cost | Rings | Shared |
|---|---|---|---|---|
| E1 | Hide and atlas in core, with sampling on modulated bodies; `sand_stock` in core | S–M | Draco, Basiliscus, Fenrir, Phoenix, Corvus, Manticora | [S] |
| E3 | Claw forms Talon / Fang / Tentacle and groupings | M | Fenrir, Kraken, Harpyia | [S] |
| E2 | Stamp tiers and top profiles | M | Draco, Corvus, Basiliscus, Manticora | [S] |
| E5 | Graph nodes `base.preset` and `stamp.*` | M | all signets and stamp rings | [S] |
| E12 | Asynchronous template instantiate with staged progress (the user's request) | M | all eight are its benchmark | [S] |
| E8 | CurveLayer width/height profile and bead rows | S | Kraken | [S] vines, serpents |
| E13 | True plans for pear, trillion, heart and half-moon | M | Manticora (Basiliscus switches to trillion if wanted) | [S] |
| E7 | `plumage.rs` and `beast.rs` painters plus procedural tiles | M | Phoenix, Corvus, Basiliscus, Draco, Fenrir, Manticora | partly |
| E14 | Stamp outline library | S | Draco, Basiliscus, Corvus, Manticora, Fenrir | [S] |
| E6 | `alpha.hide` procedural node | L | template weight and editability for all painted rings | [S] |
| E11 | Public eccentric graded `Joints` | S | Manticora | — |
| E9 | Side-face placement for CAD parts | M | a non-uniform Harpyia | [S] |
| E10 | Graded ring pattern | S | CAD quills and spines | [S] |
| E4 | `Operation::Tube` (tapered 3-D wire) | M | alternative tentacles and stingers | [S] |

**E1: a hide module in core.**
- Today the machinery lives only in `sm:40-125` and `sm:1221-1712`, plus `ex/common/sand_stock.rs`.
- The procedural copy (`ex/reptilia.rs:43-103`) samples the reference loop at every θ, so it ignores keyframes and the bypass.
- Promote it to `core/skin.rs`:
```rust
pub struct Sample { pub p: [f64; 3], pub n: [f64; 3], pub theta: f64, pub v: f64, pub i: usize }
pub struct Atlas { pub width: usize, pub height: usize, pub samples: Vec<Sample>, pub top: f64, pub bore: f64 }
impl Atlas {
    /// Imported stock through `ImportedBase::field_surface`; a procedural body through
    /// `stones::surface_frame(design, ctx, theta, v)` per sample, so keyframes and the bypass are honoured.
    pub fn of(d: &RingDesign, width: usize, height: usize) -> Result<Self>;
    pub fn paint(&self, name: &str, f: impl Fn(&Sample) -> f64 + Sync) -> Alpha;
}
pub struct HidePoint { pub along: f64, pub across: f64, pub rim: f64, pub wall: f64 }
pub struct Hide { /* along, across, rim, wall, crest as sm:1573-1581 */ }
impl Hide {
    pub fn of(a: &Atlas) -> Self;
    pub fn at(&self, s: &Sample) -> HidePoint;
    pub fn crest_at(&self, a: &Atlas, along: f64) -> (f64, f64); // sm:1645
    pub fn folds(&self, a: &Atlas, deg_per_mm: f64) -> Vec<f64>;  // sm:1751-1760
}
pub struct Joints(Vec<f64>); // sm:1667, plus `Joints::eccentric` (E11)
pub struct ClampReport { pub texels_cut: usize, pub worst_mm: f64 }
pub fn draft_clamp(a: &Atlas, alpha: &mut Alpha, height_mm: f64) -> ClampReport; // sm:1303
pub fn hide_layer(d: &RingDesign, alpha: &str, height_mm: f64, window: Window) -> LayerEntry; // sm:1716
pub fn sand_stock(source: Arc<Source>) -> Result<Arc<Source>>; // ex/common/sand_stock.rs:53
```
- The existing authors (stock masterworks, reptilia) then drop their private copies.
- Pin: an atlas of a keyframed band matches `mesh::try_build` vertices to within an atlas texel.

**E3: claw forms.**
- `head.claw` and `head.basket` gain `form` and `grouping` parameters (schema at `core/cad/builders.rs:147-158`).
- `setting::claw_parts_reach` (`core/setting.rs:737-854`) swaps only the 2-D path and radii that feed `tube()` (`:813-827`). The notch by the stone's envelope (`:706`) and the wall check (`:697-709`) are unchanged.
```rust
pub enum ClawForm { Wire, Talon { hook_deg: f64, knuckles: u8 }, Fang { curve_deg: f64 }, Tentacle { turns: f64 } }
pub enum ClawGrouping { Even, Feet { toes: u32 }, Jaws } // Jaws/Feet cluster claw_angles at ±x (plan's long axis)
// setting::tube gains an oval ring: tube_oval(path, radii, aspect, around), for the keeled talon and fang sections.
```
- Rule to pin: every bend radius ≥ local tube radius, and `csg::self_crossings == 0` on every form and count from 3 to 8.
- **[S]**: a botanical collection's petal, leaf or flame prongs are the same generalisation.

**E2: stamp tiers and tops** (`core/setting.rs:897-925`, `:971-1003`, `:1418-1434`).
```rust
pub struct Stamp { /* … */ #[serde(default)] pub tier: u8, #[serde(default)] pub top: StampTop }
pub enum StampTop { #[default] Flat, Gable { ridge_mm: f64, axis_deg: f64 }, Dome { crown_mm: f64 }, Taper { tip_mm: f64 } }
```
- `apply()` joins tier 0 onto the swept band, re-reads the solid, then drops tier 1 onto it. Today every stamp is made against the band as swept (`:1418`).
- `assemble` adds `top(x, y)` to the cap's z (`:976`).
- Sand rule to assert: a gable's ridge lies on the parting line or runs away from it, so no top facet faces back across the plane. This is Caiman's keel gable (`sm:1845-1847`).
- **[S]**: tiered emblems and domed seals.

**E5: base and stamp nodes.**
- Today `design.set /imported_base` carries the whole sand-stocked mesh: **3.04 MB** of Caiman's 5.3 MB compact graph (measured). `/stamps` is one opaque patch (`graph/lift.rs:411-449`, `graph/nodes/assembly.rs:102`).
- New nodes:
  - `base.preset{id, sand: bool, face_length_mm, face_width_mm, bore_mm}`: deterministic from `PRESETS` and E1's `sand_stock`.
  - `stamp.outline.{keel, lanceolate, moon, crescent_cutter, polygon}` → an outline.
  - `stamp{name, theta, v | crest_along_mm, rot, height, sink, draft, cut, bench, along_pull, tier, top}`.
  - `stamp.array{outline, alongs[], heights[]}`.
  - A `stamps` list pin on `design.assemble`.
- The lift emits them, and the round-trip test stays byte-for-byte.
- **[S]**.

**E12: asynchronous template instantiate.**
- Today `TemplateGraph::instantiate` (`graph/templates.rs:40-49`) and `Template::instantiate` (`wb/templates.rs:17-27`) run on the UI thread from `load_catalog_template` (`crates/ringdesign-gui/src/export.rs:543-548`) and from the phone (`crates/ringdesigner-android/src/app.rs:1540`, `app/files.rs:158`).
- The stall is: decompress, parse, base64 PNG decode, graph evaluation, then `unpack_embedded` and `bake_all` (`export.rs:550-552`).
- Proposal:
```rust
pub enum LoadStage { Decompress, Parse, Alphas, Evaluate, Bake }
pub fn instantiate_with(&self, reg: &Registry, lib: &AlphaLibrary, progress: &dyn Fn(LoadStage, f32)) -> Result<RingDesign, GraphError>;
// wb/job.rs:25 Action gains Instantiate { slug }; the menu shows the stage and a bar; the first build already runs on the worker.
```
- The Bestiarium signets are the heaviest templates yet: about 5 MB compact each before E5 and E6. They make a good benchmark for that loading path.

**E8: CurveLayer profile and beads** (`core/curve.rs:62-80`).
```rust
#[serde(default)] pub widths: Vec<f64>,   // per control point, × width_mm; empty = uniform
#[serde(default)] pub heights: Vec<f64>,  // per control point, × height_mm
#[serde(default)] pub beads: Option<CurveBeads>,
pub struct CurveBeads { pub pitch_mm: f64, pub diameter_mm: f64, pub height_mm: f64, pub offset: f64 /* -1..1 across the wire */, pub graded: bool }
```
- `feature_footprints` reports the smallest bead, both for refinement seeding and for DFM.

**E13: true stone plans.**
- `setting::Plan` is a superellipse (`core/setting.rs:105-113`). The pear is 2.0, an ellipse; the trillion is 3.2 (`core/gem.rs:154-157`).
- The preview meanwhile draws the true facet meshes (`core/gems.rs:107-128`). The drawn stone and the metal cut for it differ, which is the divergence Doctrine § A seat is the stone's plan forbids.
- Fix: `Plan` gains a 256-entry polar table taken from the true mesh's girdle. It is used by `envelope`, `bur`, `collet`, `claw_angles`, `plan_half_extents_mm` (`core/field.rs:1302`) and the stones report.
- **[S]**: hearts and drops belong to dark mythology.

**E7: `plumage.rs` and `beast.rs`.** These follow the `reptile.rs` pattern ("shared by the tile library and collection author", `core/reptile.rs:1-2`).
- `plumage::{contour, remex, hackle, morph, tail_fan}`
- `beast::{membrane(bones), fur_lock, tergite, sucker}`
- Procedural tiles: `ContourFeathers`, `FlightFeathers`, `Hackles`, `Membrane`, `FurLocks`, `Tergites`.
- `Procedural::Feather` (`core/alpha.rs:1237`) carries 12 barbs per tile, which falls under the sand floor at ring scale. The new tiles carry one form each, with barbs left to the bench.

**E14: stamp outlines.**
- `setting::outlines::{keel(a_half, b_half, tip, pitch), lanceolate(len, w, tip), comb_lobe, spade_blank, quill}`.
- `keel` is Caiman's inline hexagon factored out (`sm:1777-1792`).

**E6: `alpha.hide`.**
- A registered painter plus a JSON recipe, painted on `Atlas::of(design)` at evaluation on the worker.
- Draft-clamped when the design's process is sand; cached by recipe signature and surface epoch.
- The design carries `hides: Vec<HideRecipe>`, baked in `bake_all` as a fourth source beside drawn, text and SVG.
- A painted template shrinks from MB to kB, and an exposed flare or width re-paints the hide rather than stretching it.

**E11: graded joints.** Make `eccentric_warp` public (`core/field.rs:1267`) and add `Joints::eccentric(start, end, p0, p1)`. A segment series then closes on an integer count, as graded runs do.

**E9: side-face placement.** `Placement::Face{theta_deg, rho_mm, side, height_mm, spin_deg}` with an axial ray. `surface_hit` is radial only (`core/cad.rs:842-898`), so no CAD part can stand on a side face of a varying band today.

**E10: graded ring pattern.** `PatternKind::Ring{count, span_deg, grade: Option<Grade{end_scale, about_deg}>}`. The copies are already re-seated per angle (`core/cad/pattern.rs:252-278`); each would also be scaled about its seat.

**E4: `Operation::Tube{path: Vec<[f64; 3]>, radii: Vec<f64>, aspect: f64}`.**
- Built by `setting::tube` (`core/setting.rs:359`) as a mesh value, like `sweep.twist` (`core/cad/twist.rs:13`).
- Refused on self-crossing.
- For 3-D tentacles and serpent hair off the band.

**Build order.**
1. Draco and Basiliscus can be authored today in `ex/bestiarium.rs` with local copies of the hide helpers, as Caiman was.
2. E1 and E3 next: they unblock four rings and two templates.
3. Then E2 and E8; Kraken and Harpyia close with them.
4. E5, E6 and E12 last, for the templates.

---

## 5. Deliverables and verification (the Reptilia format)

Per ring, under `showcase/bestiarium/<slug>/`:
- `design.ring.json`, `editable-graph.ring.json`, `artwork/`;
- `finished-metal.stl`, and `casting-pattern.stl`, shrink-compensated with no cuts and drill marks (`mesh::try_build_pattern`, `core/mesh.rs:411`);
- a reference stone STL marked "do not cast";
- `report.json`, `mesh.json`, `verification.json`, and `release-fine.json` for the sand rings;
- `hero.png`, `face.png`, `palm.png`, and studio-gold Blender renders.

Collection level: `README.md`, `index.html`, `Bestiarium-collection.png`, and a renders zip.

Gates, all taken from `ex/reptilia.rs:309-353` and `crates/ringdesign-graph/examples/reptilia_templates.rs`:
- watertight, with 0 degenerate faces;
- a cold reload with an empty library rebuilds identical vertices;
- sand rings: 0 release obstructions and 0 unresolved rays at 0.100 and 0.075 mm;
- every ring: field verdict judged against its own process, and DFM findings clean or explained;
- made parts: `self_crossings` 0;
- the graph evaluates back to the source design byte for byte, with the mesh identical.

Reproduce (tests run under the memory guard):
```sh
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example bestiarium -- NEW_DIR [--draft] [SLUG]
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-core --example bestiarium -- NEW_DIR --verify
systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo run --offline --release -p ringdesign-graph --example bestiarium_templates -- NEW_DIR
CUDA_VISIBLE_DEVICES=0 blender -b --factory-startup -P tools/render_bestiarium.py -- NEW_DIR
python3 tools/catalog_bestiarium.py NEW_DIR
```

Catalogue:
- Add `group("Bestiarium", &["draco-bestiarium", "basiliscus-bestiarium", "fenrir-bestiarium", "phoenix-bestiarium", "corvus-bestiarium", "kraken-bestiarium", "manticora-bestiarium", "harpyia-bestiarium"], "Creatures told by hide, plumage and weapons, face to palm.")` to `wb/templates.rs:31-58`.
- Add the eight previews to `preview_bytes` (`wb/templates.rs:65`).

Reel:
- Stamp names follow `stamp_family` ("Dorsal spine, 3" → "Dorsal spine", `crates/ringdesigner-android/src/reel.rs:75-80`), so each struck family plays as one beat.
- Layer names are the captions.
