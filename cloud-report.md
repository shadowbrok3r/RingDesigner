# Cloud report: Cataphracta enablers C-R1, C-R3, C-R5, C-R6

All four landed in one commit on `claude/execute-task-md-36irpi` (the session's assigned branch; see "Could not do"). Every enabler is opt-in and skipped when default. `clamp` and `bare` fence at design format 6 / graph format 2 through `library::format_version_for` and `template_features_in_json`, whose `new_pin` now covers `layer.group.clamp` and `layer.seatrun.bare`. A literal `false` pin does not fence. `core/tiling.rs` and `core/reptile.rs` are untouched.

**Tests:** `cargo test -p ringdesign-core -p ringdesign-graph` all pass: 818 core unit tests, golden, bestiarium, showcase and template tests, the graph suite, and the struct coverage test. `ringdesign-cli` and `ringdesign-mcp` check clean. `ringdesign-gui` and `ringdesign-graph-ui` check clean on rustc 1.95, which egui 0.36 requires; the VM's default 1.94 cannot build them.

## C-R1: live sand clamp on a group
- **What landed:**
  - `field::GroupLayer::clamp: Option<SandClamp { resolution: [u32; 2], slack }>`, default 2048 × 768 and slack 1.
  - `RingDesign::bake_clamps(&mut AlphaLibrary) -> Vec<(String, skin::ClampReport)>`. It runs at the end of every bake that derives fields (`bake_all`, `bake_sdfs`, `unpack_and_bake_observed`), and `bake_units` counts it. It paints the composite over `skin::Atlas`, runs `draft_clamp`, and stores `"{group}##clamp"` (`alpha::CLAMP_SUFFIX`). The ceiling holds values only where the rule bit and is unbounded elsewhere, and it is never saved.
  - `AlphaLibrary::clamp_of` is an allocation-free per-sample lookup, mirroring `sdf_of`.
  - `LayerStack` evaluates a clamped group as `min(composite, field::clamp_ceiling_at(..))`.
  - Inner groups bake first. Bakes are cached by content: the serialized group, profile, shank and base, plus the content keys of every alpha and inner ceiling the group reads.
  - `sdfs_missing` also reports a missing or stale ceiling (`clamps_stale`).
  - The field report notes what each clamp cut, or that it was never baked.
- **Where it is exposed:**
  - The `layer.group` node has `clamp`, `clamp_columns`, `clamp_rows` and `clamp_slack` pins, and they are lifted.
  - The GUI group editor has a "Sand clamp" checkbox and a slack control.
  - `is_derived` keeps `##clamp` entries out of the alpha picker.
- **Tests:** `clamp_tests::*` in `core/src/lib.rs`, `a_clamped_group_fences_graphs_and_presets_while_an_unclamped_one_stays_plain` and `a_clamped_group_and_a_bare_run_lift_exactly`.
  - At every atlas sample the live clamp equals the painted `draft_clamp` of the same composite to within f32 rounding (2e-6 of the height), and re-clamping the live field cuts under 1e-4 mm.
  - A clamped `Max` of two clamped groups cuts 0 texels. It is bit-identical to the same `Max` unclamped on a 720 × 41 grid.
  - With the clamp off, heights are bit-identical whether or not the ceiling is in the library, nothing is written, and the file stays at format 5.
  - An unbaked clamp stands as composed. An edited group reads as stale until rebaked.
- **Adaptations to the plan:**
  - I read "bit-identical to today's painted path" as equality at the atlas samples. `hide_layer` resamples through a tiling, so it cannot match between samples.
  - Ceilings are keyed by group name, so clamped groups need distinct names; duplicates after the first are skipped.
- **Not done:** the P1 loader's "clamping" stage label, and `ClampReport` on the casting sheet. The report and the sheet show only the note's count of held texels, not `worst_mm`.

## C-R3: stone-less seat runs
- **What landed:** `SeatRunLayer::bare` (serde default false, skipped when false).
  - `solve_spacing` skips `fit_stone`, so the row packs by the seat's own `half_extents_mm`.
  - `setstone::set_stones` skips bare runs.
  - `stones::report` judges the authored seat with no gem, sums no carats, and sets `made = "stock only"` (`stones::STOCK_ONLY`). The sheet and GUI report print it.
  - `castability::pattern_parts` and `setting::without_solids` no longer lend the run's gem to a bare seat.
- **Where it is exposed:** the `bare` pin on `layer.seatrun` (marked sparse for the coverage check) and a "Bare / stock only" checkbox in the GUI.
- **Tests:** `stones::tests::a_bare_run_keeps_its_plan_sets_nothing_and_reports_stock_only` and `file::tests::a_bare_seat_run_fences_graphs_and_presets_by_literal_pin_wire_and_exposure`.
  - The seat stays at 0.9 mm while a set run refits to 2 mm, and the bare run packs more than twice the count.
  - No stones are set, 0 ct is reported, and the line says "stock only".
  - The design goes 5 → 6 when bare is set.
  - The graph fences by literal, pin and exposure.
- **Not done:** MCP's `SEAT_RUN_FIELDS` edit tool exposes only count, v and height. I did not add `bare` there, matching `taper` and `centre_phase`.

## C-R5: drag attribution
- **What landed:**
  - `castability::attribute_drag(&RingDesign, &AlphaLibrary, &FieldReport) -> Vec<DragShare { layer, marginal_mm2, vertical_mm2 }>`. It mutes each enabled top-level layer of the pattern at the report's parting plane and resolution. Shares are signed and sorted largest first.
  - `FieldReport::drag_fraction()`, and `DRAG_FRACTION` is now `pub`.
  - When drag gates a sand verdict, `ringdesign-cli check` prints each positive share.
- **Test:** `castability::tests::attribute_drag_names_the_layer_that_drags`.
  - A signet table outranks milgrain, and a disabled layer is not listed.
  - Each share equals the difference measured with that layer muted.
  - `drag_fraction > DRAG_FRACTION` agrees with the "drags on the sand" note.
- **Not done:** it is not shown in the GUI report panel or on MCP. It is a diagnostic, and the verdict never runs it.

## C-R6: DFM reads remaps
- **What landed:**
  - `dfm::tiling_finest_mm_remapped(t, lib, ctx, v_scale, remaps)`. `tiling_finest_mm_at` delegates to it with no remaps, and that path is the plain measurement exactly.
  - `findings_in` collects each tiling's remap chain: its own entry's remap first, then each enclosing group's. It measures the relief through that chain, normalized to the remapped top.
  - When a terrace is last in the chain, each interior tread is also measured as its own ink, and the finding says "treads". An openwork's cut is its mask, so its remap is not read.
- **Test:** `dfm::measured_tests::terrace_treads_are_measured_through_the_remap`.
  - An empty chain equals the plain measure.
  - Pyramids on 24 repeats measures 0.245 mm strokes plain and 0.049 mm treads under an eight-step terrace.
  - With a floor set between the two, a finding appears for a terrace on the layer's own entry and for one on an enclosing group, and none without a remap.
- **Not done:**
  - `fit_to_floor` and `coarsen_to_floor` still solve on the plain mask.
  - The top tread (a peak's cap) and the ground are not read as treads, because they flagged at one texel on every texture.

## Could not do
- **Push branch:** `TASK.md` asks for `claude/cataphracta-enablers-a`. This session may push only to its assigned branch, `claude/execute-task-md-36irpi`, so the work is there, based on current `origin/master`. The old seed branch shared no history with master.
- **Commits:** it is one commit rather than one per enabler, because the enablers share hunks in `lib.rs`, `library.rs` and `field.rs`.
