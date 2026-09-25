# Brief: the starter gallery and the Workshop set (File > New from template)

This brief covers four things: the three starter collections (Starter bands, Starter signets, Stone settings), a replacement for the Workshop collection, and opening templates off the UI thread with a progress bar, which Logan asked for. It is based on reading the code only: nothing was built or run. Unless a path is given, `file:line` references are relative to `crates/`. Where the text says "the doctrine", it means the project CLAUDE.md.

---

## 0. What is wrong today

| Entry | What Logan sees | Cause |
| --- | --- | --- |
| Split shank | "not even sure what I'm looking at" | `ShankKind::Split` widens the band by 55% and cuts a groove of up to 1.6 mm into each side face (`ringdesign-core/src/profile.rs:2683-2695`), on a 5.5 × 2.0 band (`ringdesign-core/src/templates.rs:178-187`). The thumbnail camera, yaw 0.55 and pitch 1.12 (`ringdesign-core/examples/template_shots.rs:13`), looks down at the crest. The grooves are on the ±Z faces, so they never show. |
| Cathedral solitaire stock | claws visible, no stone | The prongs are height-field bumps on a gypsy mound (`templates.rs:137-163`), which Logan rejected as "quite awful". `template_shots` renders only the metal mesh (`template_shots.rs:10-14`), so no stone is ever drawn. |
| Toi et moi | two signet plates (oval and heart) | `templates.rs:188-204`. It carries no stones. |
| Heart / Waved hexagon signets | parametric heads, which he finds rough | These are replaced by the 20 factory bases. |
| **Wishbone wave (broken)** | reads as a plain court band | With `waves = 1`, `z_center_frac = 0.6·k·sin(θ−90°)` (`profile.rs:2613-2625`). One sine period in θ puts each edge on a *planar* circle, tilted by about atan(0.76/10.5) ≈ 4°. The band looks like a plain band seen at a slight angle, and no camera can make it read as a wave. |
| All starter thumbnails | flat olive, not studio gold | `write_png` uses plain key-light shading (`render.rs:207,282`). The studio shading lives in `Part::metal` (`render.rs:61`). |
| Workshop (Aster, Tide, Lantern, Aureole) | weak | Aster is a plain cushion signet and Tide is plain reeds. Lantern's head is a box minus a box (`ringdesign-core/examples/workshop_collection.rs:149-272`). Aureole is a bare twisted torus. The menu also holds three "Aster"s: aster-atelier, aster-botanical and aster-workshop. |
| Opening any template | the app stalls | `instantiate` runs on the UI thread on desktop (`ringdesign-gui/src/export.rs:543-548`) and on the phone (`ringdesigner-android/src/app/files.rs:157-158`, `app.rs:1540`). See section 1 for where the time goes. |
| CAD stones | missing from the stones report, casting sheet and stone map | `setstone::StoneSource` has only `Pad` and `Run` (`ringdesign-core/src/setstone.rs:10-15`). A claw solitaire built from CAD parts "sets no stones", so the stone map refuses it. |

Other existing bugs found:
- `ringdesign-core/examples/stock_review.rs:24` uses `&preset.name[..3]` as the file id. Since the presets were renamed, this gives "Cus" for both 001 and 012, so one PNG overwrites the other. Use `preset.id`.
- `examples/cad_thumbnails.rs:9-17` merges every visible component, reference stones included, into one mesh and shades it all as metal.

---

## 1. Opening a template without stalling the app (Logan's request)

### Where the time goes, in order, all on the UI thread

1. **Unpacking the bundle.** Graph templates are inflated from the bundle (`ringdesign-graph/src/templates.rs:27-31`). The largest is 13.4 MB of JSON (`solstice-imported`); the Reptilia templates are 4–13 MB each.
2. **Parsing the JSON** (`templates.rs:33-35`).
3. **Evaluating the graph** (`templates.rs:42`, which calls `eval::evaluate_design`, `ringdesign-graph/src/eval.rs:515`).
4. **Baking a library that is then thrown away.** `evaluate_design` unpacks and bakes every alpha into `baked_library` (`eval.rs:525-532`). `instantiate` discards it.
5. **A field verdict nobody reads.** `attributed_field_report` runs at full field resolution (`eval.rs:534`). The build worker computes the verdict again after adoption.
6. **Baking a second time, after a full copy.** `adopt_template` unpacks and bakes again through `app.library_mut()` (`export.rs:551-552`). That is an `Arc::make_mut` (`ringdesign-gui/src/app.rs:1208`), which first deep-copies the whole 342-alpha library.
7. **Evaluating the graph a second time.** `sync_graph` and `arrange_graph` run, then the build worker re-evaluates the graph before its first build.

### Design (enabler E1, shared by every collection)

All of this goes in `ringdesign-workbench/src/templates.rs`, so the desktop and the phone use one loader.

```rust
pub enum Phase { Unpack, Read, Evaluate, Bake, Ready }
pub struct Progress { phase: AtomicU8, done: AtomicU32, total: AtomicU32 }   // lock-free, read every frame
impl Progress {
    pub fn fraction(&self) -> Option<f32>;   // weighted across phases; None while Read (indeterminate)
    pub fn words(&self) -> String;           // "Evaluating graph — 38 / 118 nodes"
}
pub struct Opened { pub design: RingDesign, pub lib: Arc<AlphaLibrary>, pub name: &'static str }
pub struct Loading {
    pub template: &'static Template, pub started: Instant, pub progress: Arc<Progress>,
    cancel: Arc<AtomicBool>, answer: Receiver<Result<Opened, String>>,
}
impl Template {
    /// Spawns "template-open"; wakes egui on every phase change and on landing.
    pub fn open(&'static self, reg: Arc<Registry>, lib: Arc<AlphaLibrary>, wake: egui::Context) -> Loading;
    /// The same work, synchronous, for tests, the CLI and wasm.
    pub fn instantiate_with(&self, reg: &Registry, lib: &AlphaLibrary, p: &Progress, cancel: &AtomicBool) -> anyhow::Result<Opened>;
}
impl Loading {
    pub fn poll(&mut self) -> Option<Result<Opened, String>>;
    pub fn cancel(&self);
    /// Shared floating plate: spinner, name, ProgressBar, Cancel.
    pub fn plate(&self, ctx: &egui::Context) -> bool;
}
```

What each lower crate needs:

| Crate | Change |
| --- | --- |
| `ringdesign-assets` | `Asset::text_observed(&dyn Fn(out_bytes, total))`, streaming inflate in chunks. This needs the uncompressed length in the index: check whether it is already there, and add it in the build script if not. |
| `ringdesign-graph` | `Evaluator::evaluate_observed(.., on_node: &mut dyn FnMut(done, total) -> ControlFlow<()>)`. The topological order is known before evaluation starts, so `total` is exact. Cancel is checked between nodes. Also add `evaluate_design_unjudged` (steps 1–4 above without step 5). `TemplateGraph::instantiate` switches to it. |
| `ringdesign-core` | `RingDesign::bake_all_observed(&self, lib, &mut dyn FnMut(done, total))`. It counts embedded PNGs, drawn, texts, svgs, recipes and SDF layers, so each unit of progress is one source. |
| Loader output | `Opened.lib` is the baked library, returned as an `Arc`. On landing, the UI thread swaps `app.lib = opened.lib`, a pointer swap: no `make_mut` and no second bake. |

Wiring the hosts:
- **Desktop.** `load_catalog_template` (`export.rs:543`) becomes `app.loading = Some(t.open(..))`. Add `RingDesignerApp::poll_loading(ctx)` next to `poll_import` (`app.rs:642`). On landing, `adopt_opened` does the swap and then the current tail: `history.reset`, `sync_graph`, `arrange_graph`, `mark_dirty`. The plate reuses `import_plate`'s placement and style (`export.rs:626-645`). The graph pane's "Open a template graph" (`panels/graph.rs:121`) takes the same path.
- **Phone.** The same `Loading` is polled in the frame loop. It replaces the synchronous calls at `app/files.rs:157-158` and `app.rs:1540`. The plate sits above the bottom sheet.
- **Behaviour while loading.** The current design stays live and editable. The menu row being opened shows a spinner. Picking another template cancels the first. Cancel discards the result. The status line says what is happening, for example "Opening Caiman — baking artwork 3 / 7".
- **Browser build.** There are no threads, so `instantiate_with` is stepped per frame (the configurator's `Engine::pump` pattern). The per-node and per-source checkpoints above are the slice points.
- **Optional, later.** Seed the build worker with the loader's evaluation, meaning its design, library and `Evaluator` cache. The worker's first pass then does not re-evaluate the graph, which is step 7.
- **Measure before tuning.** Add a `ringdesign-graph/examples/template_open_probe.rs` that prints the milliseconds per phase for every catalogue entry in release. The progress weights come from its numbers. No timings are claimed in this brief.

Tests:
- In the workbench: `Loading` reaches Ready on a starter and on `solstice-imported`, `fraction()` never decreases, and cancelling mid-Evaluate returns `Err("cancelled")`.
- In the GUI (kittest): after clicking a template the frame returns, `loading.is_some()` holds and the design is unchanged; stepping until it lands then finds the design swapped. This extends `file_menu_has_preview_collections_and_opens_the_selected_template` at `ringdesign-gui/src/ui_tests.rs:259`.

---

## 2. Starter bands (5 entries)

| Entry | Decision |
| --- | --- |
| Court band | Keep. Re-render the thumbnail with studio shading. |
| Braided band | Keep. The braid sits on the side faces, so render the thumbnail at pitch 0.6 rather than 1.12. |
| Wishbone wave | **Broken, see section 0.** The proper fix is E14 below (`ShankKey::slide`): a V dip at the top only, with neighbouring keys at ±25° set to 0. Interim fix: `waves = 2`, `amount = 1.0`, renamed "Saddle wave". |
| Split shank | **New.** A true split, cast in lost wax (2.1 below). |
| Split gallery | **New.** A split that casts in sand (2.2 below). This replaces `ShankKind::Split` as the starter. The kind itself stays: it is a row in the golden corpus. |

### 2.1 Split shank (true split, lost wax)

- **Concept:** a plain band that parts into two rails from the shoulders to the top, open to the air between them. Seen from above it is a Y at each shoulder.
- **Theme, face to palm:**
  - Top: two rails, 2.0 mm each, with a 2.6 mm gap between them.
  - Shoulders: the rails close to a point at 42° and 138°.
  - Palm: a plain 2.4 mm band.
  - Bore: plain comfort fit.
- **Base:** size 7. `ProfileStyle::LowDome`, 2.4 × 1.8, `comfort_fit_mm` 0.1. `ShankKind::Keyframes`, `amount` 1.0, with these keys (`ShankKey`, `profile.rs:1747`):

  | θ | width_scale | thickness_scale |
  | --- | --- | --- |
  | 270 | 1.0 | 1.0 |
  | 200 / 340 | 1.0 | 1.0 |
  | 150 / 30 | 1.35 | 1.0 |
  | 125 / 55 | 2.0 | 1.05 |
  | 90 | 2.75 (6.6 mm) | 1.1 |

- **Process:** lost wax. `CastProcess::LostWax.apply(&mut d.draft)` (`ringdesign-core/src/castability.rs:117`). A real split is two crests, which is "a valley no single parting plane clears" (doctrine; `profile.rs:2684-2688`). Each rail's inner wall faces the far mould half, so the part judge reports it as an undercut. Under lost wax that is reported but does not gate.
- **Construction (CAD document):**
  1. `Operation::Band`.
  2. `cutter.split` (enabler E7). It cuts a slot from the outer surface through to the bore over θ 42–138°, 2.6 mm wide at the centre, closing to a point at each end. Its walls are radial and the rail edges are rounded 0.3 mm. `Attach::Cut`, `Stage::Cast`.
  - **Version 0 with existing operations:** an `Operation::Loft` (`ringdesign-core/src/cad.rs:82`; it takes 2–32 sections with matching curve counts, `cad.rs:1893-1910`) through 7 rectangle sketches. Each lies on a radial section plane at θ = 42, 58, 74, 90, 106, 122, 138 (`Workplane { x: [cosθ, sinθ, 0], y: [0, 0, 1] }`). Each spans r from 7.85 to 11.45, which is past the bore and the crest. The gaps are 0.05, 1.1, 2.1, 2.6, 2.1, 1.1, 0.05 mm. `Attach::Cut`, `blend_mm` 0.25: the seam bead (`cad.rs:293`) rounds the cut rims.
- **App features shown:** Keyframes, CAD cut parts on a procedural band, seam bead, how the verdict behaves under lost wax.
- **Castability traps:**
  - Fill: the rails are 2.0 × 1.98 mm at the top, well above the investment section of 0.5.
  - Chord sag between 16° sections: 0.15 mm, covered by the 0.8 mm radial padding.
  - The 0.05 mm end sections are slivers; a loft may refuse them. That risk is why E7 exists.
- **Thumbnail:** yaw 0, pitch 1.35, from above, so the Y reads (E11).
- **Template form:**
  - Code starter: replace the builder at `templates.rs:178-187`, calling a core function `templates::split_doc(&RingDesign) -> cad::Document`.
  - Graph builder: `band.profile` + `shank`. The keyframes go in a `/shank/keys` patch until E5 exists. The document comes from `nodes::cad::chain_document` (`ringdesign-graph/src/nodes/cad.rs:254`). Set `g.next_id` above the feature ids first, as `lift.rs:327-331` does.
- **Difficulty:** medium. **Risk:** medium on version 0 (the loft), low with E7.

### 2.2 Split gallery (a split that casts in sand)

- **Concept:** seen side-on, the top of the band opens into two rails, an outer rail arching over an inner rail, with daylight between. The opening runs through the band along the finger axis, which is the pull direction.
- **Theme, face to palm:** the top is a crescent window between two 1.0 mm rails. The palm is a plain 1.8 mm band.
- **Base:** `LowDome`, 4.2 × 1.8. `Keyframes`, width 1.0 throughout. `thickness_scale` 1.0 at 270/200/340, 1.35 at 150/30, 1.85 at 120/60 and 2.3 at 90, which makes the top 4.1 mm deep.
- **Process:** sand, `SandProcess::DelftClay`. The window runs along the pull. That is the bore's case in the doctrine: "a straight through-hole … reported as a vertical wall, never an undercut". `cut_stage` returns `Cast` for any cut within 10° of the pull (`builders/cutters.rs:24,1067`). A 2° draft either way from the parting plane lets each half of the mould lift its sand out on its own side.
- **Construction:**
  1. `Operation::Band`.
  2. `cutter.window` (enabler E8): θ 38–142°, inner and outer rails 1.0 mm, `draft_deg` 2, tips rounded 0.35.
  - **Version 0 with existing operations:**
    - `Operation::Plane { base: PlaneBase::Parting }` (`cad/pattern.rs:70-84`).
    - A sketch on that plane (`FaceAnchor { feature: plane, face: FaceRef::bare(0) }`; work planes are resolved at `cad.rs:1596-1600`). It holds a lens of two `Geometry::Arc`s. The inner arc has r 9.65 about the finger axis. The outer arc runs through both tips and through r 11.75 at 90°: a circle of radius ≈ 8.1 whose centre sits ≈ 3.6 mm above the axis.
    - `Extrude` of that sketch, height 2.6, `draft_deg` −2 (widening toward the +Z face; `cad.rs:59-65`). `Attach::Cut`, `Stage::Cast`.
    - `Pattern { kind: Mirror { plane: MirrorPlane::Band } }` of the extrude (`cad/pattern.rs:37-66`).
    - Derive the thickness keys from the outer arc plus 1.0, so the outer rail stays 1.0 mm all along.
- **App features shown:** the pull-direction doctrine made visible, keyframed thickness, draft either side of the parting plane, and a mirrored cut.
- **Castability traps:**
  - **(a) The verdict cannot see the rails.** "The verdict is radial and cannot see the axial web" (doctrine). The field verdict does not see the window, so rail thickness has to come from `manufacturing::inspect`, which samples local walls. Put that in the template's test.
  - **(b) Sand strength.** The sand filling the window is a slab about 2.1 × 17 × 4.1 mm. Mark it "mould trial required" (docs/WORKSHOP-CAD.md: "sand-slot checks do not calculate sand strength").
  - **(c) Rail thickness.** Rails of 1.0 mm clear the Delft minimum section of 0.8.
  - **(d) Walls facing round the ring.** "Walls facing round the ring lean wherever the section is still changing width" (doctrine) does not apply here: the window walls are extruded along Z and drafted, never facing round the ring.
- **Thumbnail:** yaw 0.25, pitch 0.35, side-on, so the window shows daylight.
- **Template form:** a code starter plus a graph builder, as in 2.1.
- **Difficulty:** medium. **Risk:** low with E8, medium on version 0 (the arc fitting).

---

## 3. Stone settings (8 entries, each with its stone in the thumbnail)

Rules common to all eight:
- **Made settings only.** Stones go through the CAD builders (`ringdesign-core/src/cad/builders.rs:15-35`, specs at `:60-70`) or through `SeatPadLayer::solid` (`ringdesign-core/src/setting.rs:21-33`; the fields `solid`, `through` and `mark_mm` are at `ringdesign-core/src/field.rs:1188-1196`). No height-field prongs.
- **Placement.** Every stone is placed with `builders::stone_feature(id, gem, Placement::ring(90, builders::stand_off_mm(key, gem)))` (`builders.rs:916`, `:487`). Heads use `feature_on` or `setting_features(key, stone, gem, sand, next)` (`builders.rs:928`, `:933`). Under sand, `setting_features` stages the head and bur as `Bench`; under lost wax, `Cast`.
- **Castability test.** Each template is held to `castability::judge::judged_field_report(.., Some(&built))` (`ringdesign-core/src/castability/judge.rs:118`), not just `analyze_field`, so CAD parts are judged too. The test also asserts `built.solids.notes` is empty, because a part that fails to resolve is dropped with a note.
- **Code shape.** Each document comes from a core function such as `templates::settings::<ring>(&RingDesign) -> cad::Document`. The code starter and the graph builder (via `chain_document`) share it, so they stay equal byte for byte.
- **Thumbnails** need enabler E2 (finished metal plus stones). **Stone reports** need E3 (CAD stones in the stone record).

| # | Ring | Process | Base | Stones and setting | Builders and layers |
| --- | --- | --- | --- | --- | --- |
| 1 | **Cathedral solitaire** | lost wax | DShape 2.1 × 1.8, `ReverseTaper` 0.45 (1.7 mm at the top) | Round 6.5 (1.00 ct) in six claws | stone, then `head.claw` {prongs 6}, then `shank.cathedral` {head, spread 30, rise 0.7} (`cutters.rs:1176`), then `cutter.azure` {count 4, Teardrop, head} (`cutters.rs:1167`), then `seat.bur` {through} |
| 2 | **Bezel solitaire** | sand; collet soldered | LowDome 2.4 × 1.7, Uniform | Oval 7 × 5 in a collet | stone at `stand_off_mm("bezel")`, then `head.bezel` (default wall and lip), then `seat.bur` {through false}. Both staged `Bench` by `setting_features(.., sand=true)`. The sand pattern is the plain band. |
| 3 | **Halo** | lost wax | DShape 2.0 × 1.8, `ReverseTaper` 0.3 | Cushion 6.0, four claws; halo of about 16 × 1.2 mm melee; pavé shoulders of Round 1.3 | `setting_features("halo")`: four-claw head, halo {melee_mm 1.2, gap 0.3, bridge 0.25}, bur. Plus two `SeatRunLayer`s (`field.rs:1917`) on the crest, `Window::around(90±42, 44)`, `seat.solid = Bead`, `solve_spacing(&ctx)` |
| 4 | **Trilogy** | lost wax | DShape 2.2 × 1.9, `ReverseTaper` 0.3 | Round 5.5 centre; Pear 5 × 3.5 either side, points toward the centre | stones at θ 90, 62 and 118. The side stones get `spin_deg` ±90 (the stone's x axis runs along the finger, `cad.rs:392-406`). Three `head.claw`s and three burs. |
| 5 | **Toi et moi** | lost wax | LowDome 3.0 × 1.9, `ShankKind::Bypass` 1.0 | Pear 7 × 5 on arm A, Oval 6 × 4 on arm B | Pear at θ 107, `across_mm` +0.7. Oval at θ 73, across −0.7. Both lie along the ring (spin 90). Claw heads and burs. |
| 6 | **Split-shank basket** | lost wax | the Split shank of 2.1 | Oval 8 × 6 (≈1.5 ct) | stone, then `head.basket` {prongs 4, rails 2}, then `seat.bur` {through}. The basket's feet land on the rails (inner edges at ±1.3, outer at ±3.3 mm). |
| 7 | **Half eternity** | sand | LowDome 2.8 × 2.1 | Round 2.0 × about 15, bead set | One `SeatRunLayer` on the crest line (`v = ctx.crest_v_mm`), `Window::around(90, 160)`, `bridge_mm` 0.35, seat Boss (height 0.2, crown 1.0, blend 0.45), `solid = Bead` |
| 8 | **Gypsy trio** | sand | LowDome 6.0 × 2.4 | Round 4.0 centre, Round 2.5 at ±18° | Three `SeatPadLayer`s: GypsyMound (0.55 / 0.4 high, crown 1.0, blend 0.6 / 0.5), `solid = Flush`, `through = true`. This follows Oriel's centre (`ringdesign-core/examples/atelier_masterworks.rs:167-171`). |

Per ring: traps, what it shows, what it needs, and risk.

1. **Cathedral solitaire.**
   - Traps: the claw feet must reach metal (`FOOT_SINK_MM`, `setting::breaks_wall`); the culet sits 0.2 mm clear (`CULET_CLEAR_MM`, `builders.rs:44`); the azures must pass the `head` parameter so their windows clear the claws.
   - Shows: the whole claw-head builder chain in the timeline.
   - Needs: E2 and E3. E6 would let Stone / Cut / Claws be graph controls.
   - Difficulty low, risk low: `cad::examples` "claw-solitaire" (`cad/examples.rs:204-213`) is already this minus the arches and azures.
2. **Bezel solitaire.**
   - This is the one starter that teaches the two-stage model: the ring is shown finished, the pattern exports the bare band, and the verdict says the collet is a bench part (judge: a bench part under sand is not judged). Switching the process to lost wax flips both parts to `Cast`, a lesson in itself.
   - Trap: the collet overhangs a 2.4 mm band. That is fine for a soldered head; `under_wall` reads the metal under it (`builders.rs:496-507`).
   - Needs: E2, E3. Risk low.
3. **Halo.**
   - Traps: the melee against the shoulder runs is exactly the crowding census's job, which does not see CAD melee (E3). The shoulder runs ride the crest line ("a bead row rides the crest line only", doctrine).
   - Shows: CAD builders and a height-field run in one ring.
   - Difficulty medium, risk medium: this is a halo on a 2 mm shank.
4. **Trilogy.**
   - Spacing, computed rather than guessed: girdle radii ≈ 12.95 (centre) and 12.15 (sides). At ±28° the centre-to-side distance is 6.12 mm against 2.75 + 2.5 + claw ≈ 5.95. At 21° they collide, at 5.70 mm.
   - Needs E3 for the crowding pair between the stones. Risk medium.
5. **Toi et moi.**
   - Geometry: the bypass arms sit at ±0.45·half-width (`profile.rs:2321`, `bypass_arm` at `:2338`). At 17° past the top both arms are present and the section is 4.35 mm wide, so there is metal under both heads. At 14° the stones would collide (6.5 mm needed, 6.35 mm available).
   - Risk medium, from the claw feet on a crossing.
6. **Split-shank basket.** Depends on 2.1 (E7). Risk medium: a basket's base rail straddling the gap needs a test render.
7. **Half eternity.**
   - A stone column must run "along the parting plane", and the crest line is that plane (doctrine: commissions). The seat skirt of 0.45 reads 0.38 at the chart's 0.85 metal scale, above the 0.30 Delft floor (doctrine: collection3). Pavilion ≈ 0.8 plus the `MIN_WALL_MM` of 0.5 fits within 2.1.
   - The pattern is the band with seat stock and a raised drill-start dot per stone (`mark_mm`). The burs and beads are bench work.
   - Risk low.
8. **Gypsy trio.**
   - Everything sits on the crest line. The drill-start dots are raised, not pits, because "a pit's far wall faces back into its own mould half" (doctrine).
   - Risk low.

**Thumbnails:** render at 700 px with studio shading, using E2's metal and stone parts, at yaw 0.55. Pitch is 1.12, except the toi et moi at 1.25 and the gypsy trio and half eternity at 1.0. `tools/template_thumbnails.py` then pads each to 160 px.

**Collection card subtitle:** "Eight made settings · three pour in sand, five in lost wax".

---

## 4. Starter signets: the 20 factory bases, bare, plus Shouldered cushion

### How each becomes a template

- **As code starters built from `PRESETS` when picked** (`ringdesign-core/src/imported_base.rs:1093`). They are **not** bundled designs (that would carry the 8.8 MB of masters twice) and **not** graph templates yet: a lift carries `/imported_base` as a 3 MB `design.set` patch per ring. `caiman-imported.graph.json` is 12.7 MB, and 3.0 MB of that is this one patch (checked).
- **New source variant.** Add `Source::Stock(&'static Preset)` to the workbench `Source` enum (`ringdesign-workbench/src/templates.rs:16`). `instantiate`:

  ```rust
  let mut d = RingDesign::default();
  ImportedBase::attach(&mut d, preset.load()?)?;          // imported_base.rs:472; resets size, width, thickness, head from calibration
  d.imported_base.as_mut().unwrap().sand_envelope = true;  // the stock samples clean only with the envelope (doctrine: stock masterworks)
  SandProcess::DelftClay.apply(&mut d.draft);
  d.name = format!("{} signet · {}", preset.name, preset.id);
  ```

  `attach` clears `d.graph` (`imported_base.rs:493`), so a stock starter opens with the panels editable, the same as every code starter today.
- **Graph templates later.** Once `base.stock` (E4) exists, "Convert to graph" lifts each stock to about 10 nodes with the five stock controls exposed: US size, face width, palm thickness, face length and face rise, which is what the imported lift already exposes (`ringdesign-graph/tests/imported_bases.rs:16`).
- **Names and slugs.** Names like "Octagon signet · 015"; the two cushions become "Cushion signet · 001 (20 mm)" and "· 012 (10 mm)". Slugs like `stock-015-octagon`. The description is `Preset::label()` plus "factory stock, hard angles where wall meets face; bare, ready for a theme".
- **Menu order.** Shouldered cushion first (the parametric one), then by family, with a weak label per family inside the scroll area:
  - Round and square: 013, 012, 001, 017, 006, 015.
  - Shields: 004, 014, 020, 011, 019.
  - Lobed: 003, 007, 005, 016, 018, 008.
  - Pointed: 002, 009, 010.
- **Remove.** Heart and Waved hexagon come out of core `TEMPLATES`, graph `BUNDLED` (`ringdesign-graph/src/templates.rs:60-70`, `build()` at `:311-325`), `graphs/templates/*.graph.json` and `preview_bytes`. The cluster presets `heart-signet.preset.json` and `cushion-signet.preset.json` stay; they are cluster presets, not templates.
- **Phone.** The phone's "Imported signet base..." entry opens only `PRESETS[0]` (`app/files.rs:141-155`). Point it at this submenu.

### Thumbnails

- Extend `template_shots.rs` with a second loop over `PRESETS`: attach and envelope as above, `mesh::try_build(.., 512 × 192)` (`mesh.rs:362`), then `render::write_png_parts(dir/stock_<id>.png, &[Part::metal(&mesh, GOLD)], 0.55, 1.12, 700)` (`render.rs:261`).
- Add 20 entries to `ringdesign-workbench/assets/templates/sources.json` and run `tools/template_thumbnails.py`.
- Replace the 60-line `preview_bytes` match (`ringdesign-workbench/src/templates.rs:65-100`) with a `thumbnails!{ "slug", … }` macro that emits both the match and a `SLUGS` list.

### What the tests need

- **Workbench** (`ringdesign-workbench/src/templates.rs:138-154`):
  - Change the count assertion to the new total, 58 (section 7).
  - Add: all 20 `PRESETS` appear once in "Starter signets", names and slugs are unique, and every preview is 160 × 160 and not blank.
- **New test `every_stock_starter_opens_clean`**, one test over all 20 at 192 × 96, under the memory guard. For each: `imported_base.source.name == preset.stock_name()`, the envelope is on, the sand is Delft, the build is watertight with 0 degenerate faces, and the verdict is not NotCastable.
- **GUI** (`ui_tests.rs:259`): open "Octagon signet · 015" through the loader and step until it lands; `imported_base` must be `Some`.
- **Golden corpus** (`ringdesign-core/tests/golden.rs`, `corpus.json`): drop the retired `template/…` rows, add the new ones, and add `stock/001…020` rows so the masters themselves are regression-pinned. Rewrite with `RD_WRITE_GOLDEN=1`.
- **Retired-name fixtures (E10).** These call sites look up "Heart signet" or "Cathedral solitaire stock" by name:
  - `ringdesign-core/src/castability/judge.rs:674`
  - `ringdesign-core/src/cad.rs:3579`
  - `ringdesign-core/src/parts.rs:690,1025`
  - `ringdesign-core/src/cad/pattern.rs:790`
  - `ringdesign-core/src/interaction/pick.rs:982`
  - `ringdesign-gui/src/pattern_tests.rs:331`
  - `ringdesign-workbench/src/touch/parts.rs:306`
  - the examples `bead_probe`, `pick_probe` and `join_probe`
  - `ringdesign-py/tests/test_smoke.py:25,81`, which also asserts `len(names) == 9`

  Move the retired builders to `templates::fixture(name)`, which is test-visible and not in the menu.
- **Configurator.** `ringdesign-configurator/src/compose.rs:52-54` uses these names as labels only. It is unaffected.

---

## 5. Workshop becomes **Officina**: six lessons in the CAD workspace

- **Subtitle:** "Six lessons in the CAD workspace · three pour in sand, three in lost wax".
- **Format.** Each ring is a short feature history. The graph (`cad.feature` nodes via `chain_document`) and the CAD timeline chips list the same features in the same order. The README tells the reader to drag the rollback marker (`Document::through`, `cad.rs:1290`; timeline at `ringdesign-workbench/src/timeline.rs:573-600`) from feature 1 to the end.
- **Bore.** Nominal 18.2 mm, keeping `workshop_collection.rs`'s `setup()` and `base()` (`:15-77`) for the manufacturing recipe and channels.
- **Retire.** `aster-workshop`, `tide-workshop`, `lantern-workshop` and `aureole-workshop`, and their `DESIGNS` bundles. That also ends the three-"Aster" collision.
- **Relation to the Gothic collection.** Lessons are short, one idea each. Depth belongs to the Gothic collection.

| # | Ring | Process | Lessons, in order | Features |
| --- | --- | --- | --- | --- |
| 1 | Rivet | sand | primitive on the ring, Join, seam bead, ring array | 5 |
| 2 | Sigil | sand, stock 004 | work plane on factory stock, T-junction regions, cut a region, Bench stage | 7 |
| 3 | Keystone | sand | drafted extrude, press-pull, fillet by edge signature, sketch on a face | 8 |
| 4 | Aile | lost wax | builder chain, Bezier regions, per-region heights, Boolean union, mirror | 12 |
| 5 | Torsade | lost wax | work-plane offset, path and section sketches, twisted sweep, mirror across the band, cabochon collet | 9 |
| 6 | Lantern | lost wax | every stone builder, side-face piercing along the pull, arrays and mirrors of cuts | 11 |

### 5.1 Rivet: a riveted strap

- **Base:** LowDome 6.0 × 2.1.
- **Features:**
  1. `Band`.
  2. `Sphere { radius_mm 0.9 }`, `Placement::Ring { θ 90, height −0.35 }` (`cad.rs:385-406`), `Join`, `blend_mm` 0.15.
  3. `Pattern { Ring { count 16, span 360 } }` (`pattern.rs:37-45`), `Join`, blend 0.15.
  4. `Sphere 0.5` at θ 90 + 11.25 (half a pitch).
  5. `Pattern Ring 16` of it.
- **Teaches:** placement and stand-off; Join against Separate; seam bead radius; why integer counts close seamlessly (doctrine: Seamlessness). The step is 360/count when the span is a full turn (`pattern.rs:133`).
- **Traps:**
  - Beads on the crest line split cleanly between cope and drag. The same row 1.9 mm off the crest locks at −37° (doctrine: showcase). So `across_mm` stays 0.
  - The 4.4 mm pitch leaves gaps of at least 0.9 mm, above the detail floor.
- **Needs:** nothing new. Risk low.

### 5.2 Sigil: a quartered seal on factory Shield 004

- **Base:** stock 004 (18 × 18) with the envelope on, Delft sand.
- **Features:**
  1. `Band` (the stock is the band).
  2. `Plane { Tangent { θ 90, across 0 } }` on the table.
  3. A sketch on 2: a shield outline (`Polyline` plus two `Arc`s, inset 1.2 mm), crossed by two `Line`s that end on the outline. Those are T-junctions, making 4 regions (`ringdesign-core/src/sketch/region.rs:1-2`, `RegionRef` at `:57`).
  4. and 5. `Extrude { Profile::Region { feature 3, region NE }, −0.30 }`, then the same for SW. `Attach::Cut`, `Stage::Bench`.
  6. A sketch "Bordure": two concentric outlines, making one region with a hole.
  7. `Extrude { Profile::Feature 6, −0.20 }`, Cut, Bench.
- **Teaches:**
  - Regions and holes, and picking a region by `RegionRef`.
  - A negative extrude is a cut.
  - The two-stage model: a signet's seal is cut into the cast blank afterwards (doctrine). The bench cuts are in the render, not in the pattern, and the verdict skips them ("The verdict judges the pour, not the bench").
- **Needs:**
  - **Verify** that a `Tangent` work plane and `Placement::frame_on` resolve on an imported base's built mesh (`pattern.rs:582`; `pierce_at` requires `band_is_procedural`).
  - E4, so the graph does not carry the 3 MB stock patch.
- Risk medium.

### 5.3 Keystone: a drafted cartouche

- **Base:** Flat 6.0 × 2.0, `flatten_sides`.
- **Features:**
  1. `Band`.
  2. `Plane { Tangent { 90 }, offset −0.5 }`. Sinking the plane keeps the boss joined where the band curves away: sag over 6 mm ≈ 0.44 mm.
  3. Sketch: a rounded rectangle, 6.0 round the ring × 4.8 across, drawn with `Line` and `Arc`.
  4. `Extrude` of 3, 1.4 mm, draft 7°, `Join`, blend 0.25.
  5. `PressPull` of the top face of 4, +0.4 (`cad.rs:131`). Author the face with `FaceRef::signed(body, i, &frame)` after one evaluation (as at `cad.rs:3491`).
  6. `Fillet` of the four top rim edges, r 0.35, with `EdgeRef`s named by signature.
  7. A sketch on the pressed top face: a lozenge plus one diagonal, making 2 regions.
  8. Extrude one triangle −0.35, Cut, Bench (a bright-cut facet).
- **Teaches:** draft, press-pull, and that edges are named by signature, not position (doctrine), which keeps the fillet attached when 5 changes.
- **Traps:**
  - The walls facing round the ring and the flat top are zero-draft. The expected verdict is "castable with care", the same case as a signet's table (doctrine: "A flat table is a zero-draft plane…"). The fillet adds draft at the rim. State the expected verdict in the README; that is the lesson.
  - Risk: native `brep::fillet_edges` on a drafted prism (`cad.rs:1941-1953`); fall back to `Chamfer`.
- Risk medium.

### 5.4 Aile: wings clasping a bezel (homage to Logan's Hypnos)

- **Base:** DShape 2.4 × 1.8, `ReverseTaper` 0.4.
- **Features:**
  1. `Band`.
  2. Stone Oval 7 × 5, placed with the bezel stand-off.
  3. `head.bezel`.
  4. `seat.bur`.
  5. `Plane { Tangent { θ 58 }, offset −1.0 }`.
  6. Sketch "Wing": three feather loops drawn with `Bezier`, sharing their separating curves, making 3 regions.
  7.–9. Extrude each region to 1.8, 1.6 and 1.4 mm (0.8, 0.6 and 0.4 mm proud at the tangency, lifting toward the tips).
  10. `Boolean Union` of 7 and 8.
  11. `Boolean Union` of 10 and 9, `Join`, blend 0.12. Mesh operands go through csg (`cad.rs:1912-1924`).
  12. `Pattern { Mirror { Section { θ 90 } } }` of 11, `Join`.
- **Teaches:** building round a stone; regions from T-junctions; one sketch with three heights; union into one part; mirroring round the ring.
- **Traps:** in sand, wings off the crest lean or overhang (doctrine: off-crest rails; a leaf off the crest), hence lost wax. Feather tips must be at least 0.15 detail and 0.5 section under investment.
- Risk medium: Bezier regions and csg union.

### 5.5 Torsade: a rope-edged collet

- **Base:** Flat 4.4 × 1.8, `flatten_sides`.
- **Features:**
  1. `Band`.
  2. `Plane { Parting, offset +1.55 }`.
  3. Sketch "Path" on 2: an `Arc` of r 10.6 about the axis, from 112° the long way round to 68°.
  4. `Plane { Section { θ 112 } }`.
  5. Sketch "Section" on 4: a 1.0 mm diamond centred on the path's start point (x 10.6, y 1.55), square to the path, as `twist::sweep` requires (`cad/twist.rs:361-378`).
  6. `Twist { sketch 5, path 3, degrees 7200, end_scale 1.0 }`, `Join`, blend 0.08.
  7. `Pattern Mirror { Band }` of 6.
  8. Stone `Gem::cabochon(Round, 7.0)`, placed with the bezel stand-off.
  9. `head.bezel`: its lip comes from the cabochon's dome ("The bezel stands on its stone"). The rope ends at ±22° butt into the collet wall.
- **Teaches:** paths against sections, the twist parameters, and mirroring across the band's mid-plane.
- **Traps:**
  - "A true helix locks in the sand" (doctrine), hence lost wax.
  - The station and triangle limits are fine: about 2400 stations, which is ≤ `MAX_STATIONS` 8192 (`twist.rs:21`).
  - A twist is a mesh, so fillet and press-pull refuse it by name (doctrine).
- Risk medium: csg join time along a 59 mm rope, not measured.

### 5.6 Lantern: a gallery solitaire (capstone)

- **Base:** Flat 2.4 × 2.4, `flatten_sides`, `ShankKind::Cathedral` 0.8. The swell gives the side faces room for piercings.
- **Features:**
  1. `Band`.
  2. Stone Oval 8 × 6.
  3. `head.basket` {4, rails 2}.
  4. `shank.cathedral` {head}.
  5. `cutter.azure` {6, Teardrop, head}.
  6. `seat.bur` {through}.
  7. `cutter.pierce` Drop through the +Z side face at θ 48, via `pierce_at(.., hit with n.z > 0.8, Shape::Drop)`. This is the side-face mode: `cant −90`, Stage `Cast` along the pull (`cutters.rs:1099-1164`).
  8. `Pattern Ring { count 3, span −32 }` of 7, giving 48°, 32° and 16°. A partial span steps span/(count−1) (`pattern.rs:133`).
  9. `Mirror { Section 90 }` of 7.
  10. `Mirror { Section 90 }` of 8.
  11. (Optional) `Document::through` shipped at 3, so the timeline opens mid-lesson.
- **Teaches:** every stone builder in dependency order; that a piercing along the pull is legal in sand while the basket is not (per-part stages); arrays and mirrors of cuts.
- **Traps:** the arches land at the spread of 32°, so the piercings start at 48°. The side face must be at least 2·(`MIN_EDGE` + 0.2) + 0.3 (`cutters.rs:1105-1118`).
- **Needs:** E2, E3. Risk medium.

### Template form, files and reproduce

- **Author example.** `ringdesign-core/examples/officina.rs` replaces `workshop_collection.rs`. It keeps `write_ring`, manufacturing packages and `--verify`, and saves `showcase/officina/<ring>/design.ring.json`.
- **Graph packager.** `ringdesign-graph/examples/officina_templates.rs`, modelled on `reptilia_templates.rs:1-38`, lifts each design to `graphs/templates/<ring>-officina.graph.json` with `cad.feature` chains. It must hold that the geometry and source survive the lift identically, and writes `verification.json`.
- **Menu.** `Source::Graph`, so the graph pane and the timeline show the same list.
- **Per-ring folder** (Reptilia format): design, editable graph, `hero.png` / `top.png` / `side.png`, `nominal.stl`, `pattern-package/`, `report.json`, Blender renders via `tools/render_reptilia.py` generalised to `tools/render_collection.py`, and a README with the reproduce commands run under `systemd-run --user --scope -p MemoryMax=4G`.
- **CAD Examples gallery** (`cad::examples::NAMES`, `ringdesign-workbench/src/cad_tools.rs:216-240`): add the six as examples. Keep the existing ones, which tests use (`timeline.rs:827,875`; `lift.rs:506`).

---

## 6. Enablers, ranked (✱ = shared with the four master collections)

| Rank | Enabler | API sketch | Unblocks |
| --- | --- | --- | --- |
| E1 ✱ | **Async template open with progress** | section 1: `Template::open`, `Loading`, `Progress`, `Evaluator::evaluate_observed`, `bake_all_observed`, `evaluate_design_unjudged`, the returned `Arc<AlphaLibrary>` | Logan's request; every collection, since the stock and art templates are 4–13 MB |
| E2 ✱ | **Finished-ring render parts** | `render::finished(design, lib, params) -> Result<Finished { metal: Mesh, stones: Option<Mesh> }>`. `metal` is `try_build(..).mesh` (heads joined, seats cut). `stones` is `gems::preview_mesh` (`ringdesign-core/src/gems.rs:36`) plus every reference `stone` component (the pattern at `cad/examples.rs:229-238`). Also fix `cad_thumbnails.rs` | every stone thumbnail and hero; `template_shots`; configurator |
| E3 ✱ | **CAD stones in the one stone record** | `StoneSource::Cad { feature: Id }`. `SetStone.seat` becomes `Option<SeatPadLayer>` or gains a `frame: StoneFrame`. `set_stones` walks enabled reference `Builder{stone}` features in the outputs, taking the gem from `builders::gem_of` (`builders.rs:248`) and the frame from the evaluated placement | stones report, crowding, casting sheet, stone map (which refuses today), carat totals, reel |
| E4 ✱ | **`base.stock` graph node and lift recognition** | Pins: `design`, `stock` (select "001"…"020"), `sand_envelope`. Eval: `PRESETS…load()` and `ImportedBase::attach`. The lift emits it when `source.name == preset.stock_name()` and the fingerprints match. `attach` resets the dimensions from calibration, so place the node before `band.profile` and `head`, or give it a no-reset mode. A differing chart stays a small `/imported_base/chart` patch | stock starters as light graphs; Sigil; about 3 MB less in each of 9 bundled templates |
| E5 ✱ | **Keyframes in the graph** | `shank.key` node {theta_deg, width_scale, thickness_scale, crown_scale} into a `keys` list pin on `shank`. Remove `"keys"` from the hidden list at `ringdesign-graph/src/nodes/shank.rs:52`. The lift emits one key node per station | both Split starters; every keyframed sculptural body |
| E7 | **`cutter.split` builder** | `SPLIT = "cutter.split"`, `Attach::Cut`, role Shank. Schema: `theta_deg`, `spread_deg` (20–80, default 48), `gap_mm`, `rail_round_mm` (0.3), `tip` ("Point" or "Round"). Built as a closed loft of radial rectangles at no more than 2° per station, read through `Bore::of` (`builders.rs:568`). It refuses, naming the station, where a rail falls below `MIN_EDGE_MM` | Split shank; Split-shank basket |
| E8 ✱ | **`cutter.window` along the pull** | `WINDOW = "cutter.window"`. Schema: `from_deg`, `to_deg`, `rail_in_mm`, `rail_out_mm`, `draft_deg` (0–5, default 2), `tip_round_mm`. The outline is r_in(θ) = bore crossing + rail_in and r_out(θ) = outer crossing − rail_out, from `Bore.crossings`. It becomes two frusta meeting at the parting plane; stage `cut_stage([0,0,1], sand)`, which is Cast | Split gallery; Lantern galleries; tracery and openwork in other collections |
| E6 ✱ | **Typed builder nodes** | `cad.op.<key>` nodes generated from `builders::schema` (`builders.rs:147`), outputting operation JSON into the existing `operation` pin of `cad.feature` (`nodes/cad.rs:63`). Add a `placement` JSON pin to `cad.feature`. The lift wires them for `Builder` features | "Stone", "Cut", "Claws", "Spread" as exposed graph controls |
| E9 | **Stock starters in the menu** | `Source::Stock(&'static Preset)`, the `thumbnails!` macro, the `PRESETS` loop in `template_shots`, the fix at `stock_review.rs:24` | section 4 |
| E10 | **Retired-template fixtures** | `templates::fixture(name) -> Option<RingDesign>` holding Heart, Waved hexagon, Cathedral stock, old Toi et moi and old Split | about 12 test call sites (section 4) |
| E11 | **Per-template thumbnail camera** | `Template::view: (f64, f64)`, read by `template_shots` | split from above, gallery from the side, bands low |
| E14 | **`ShankKey::slide`** | axial slide per station (`z_center_frac`), interpolated like the other keys, with Wave's crest-straddle so the crest stays on the parting plane | a true Wishbone (V at the top only) |
| E12 | Lesson notes (optional) | `Feature::note: String` (serde default), shown in the timeline chip's hover | Officina |

**Build order:**
1. E1 and E2 first. They are independent, and every screenshot of this work needs E2.
2. Then E3 and E9.
3. Then E5, E7 and E8, which the split starters need.
4. E4 and E6 before the Officina graphs ship.
5. E10 lands in the same commit as the removals.

---

## 7. Final gallery (58 entries; the test count moves from 31 to 58)

| Collection (suggested order) | Entries |
| --- | --- |
| Starter bands | Court band · Braided band · Wishbone wave (fixed) · Split shank · Split gallery |
| Starter signets | Shouldered cushion · 20 factory stocks (001–020) |
| Stone settings | Cathedral solitaire · Bezel solitaire · Halo · Trilogy · Toi et moi · Split-shank basket · Half eternity · Gypsy trio |
| Officina | Rivet · Sigil · Keystone · Aile · Torsade · Lantern |
| Reptilia, Stock masterworks, Atelier, Original masterwork signets | unchanged (5, 7, 4, 2) |

Starters go first because a new user reaches for a blank before a masterwork. The collection row's icon is its first template's thumbnail (`ringdesign-workbench/src/templates.rs:115`), so Court band, Shouldered cushion, Cathedral solitaire and Rivet become the row icons.

**Numbers that do not match across files:**
- Core `TEMPLATES` goes from 9 to 14: 5 bands, the shouldered cushion, and 8 settings.
- Graph `BUNDLED` goes to 14, pinned byte for byte (`ringdesign-graph/src/templates.rs:682`).
- The py smoke test's `len(names) == 9` becomes 14.

---

## 8. Risks to raise with Logan before building

- **Split gallery's shape.** It is honest sand, but it reads as a split only from the side. Confirm he wants it offered beside the lost-wax Y split, rather than instead of it.
- **Bezel solitaire's process.** It is sand plus a soldered collet by default. Confirm, or make it lost wax for consistency with the other heads.
- **Stock starters on sand-envelope builds.** The first build of a bare stock runs `pull::build`. Measure it in `template_open_probe` and E1's first-build timing before promising "instant".
- **Sigil depends on CAD standing on an imported base.** It needs one spike test before authoring.
- **Graph size.** Without E4 and E5, every stock or keyframed template lifted to a graph carries whole-field patches (`/imported_base` 3 MB, `/shank/keys`). That works, but they are unreadable as lessons.
