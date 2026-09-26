# core-graded-centre-phase — report

**Branch:** `claude/core-graded-centre-phase` (from master)
**Change commit:** `5b84bce` — core: centre_phase for mirror-true graded seat runs
(this report is committed on top of it as the branch's last commit)

## What changed
- `SeatRunLayer::centre_phase: Option<f64>` (`crates/ringdesign-core/src/field.rs`).
  `Some(p)` sets the station phase to `p · 2π/n` in the warped angle measured from
  `taper_theta_deg`: 0 stands a station on the centre, 0.5 straddles it. The eccentric
  warp is odd about the centre, so the row is mirror-true in theta at any taper
  (ungraded rows too, where the warp is the identity). `None` keeps the existing
  0-degree anchor and the ungraded `k·360/n` fast path. Non-finite values read as `None`.
- Serde: `#[serde(default, skip_serializing_if = "Option::is_none")]`, so an unset run
  writes no new key and every existing design/template/example is byte for byte.
- Fence (`crates/ringdesign-core/src/library.rs`): `format_version_for` walks the layer
  stack (groups included) for a set `centre_phase` → format 6;
  `template_features_in_json` treats a non-null `centre_phase` key as fenced, which
  covers a design's embedded graph and, via the graph crate's existing writers
  (`graph_version_for`, `preset_version_in`), graph/cluster/preset files → graph format 2.
  No new version numbers.
- `examples/atelier_masterworks.rs`: its exhaustive struct literal gains
  `centre_phase: None` (no output change).
- CLAUDE.md: one short paragraph under "Graduated runs".

## Tests
New:
- `field::tests::a_centred_graded_run_is_mirror_true_about_its_taper_centre`:
  count 13, centre 90°. Asserts `None` is *not* mirror-true (today's lopsided grading).
  Asserts `Some(0.0)` and `Some(0.5)` are mirror-true at taper 0, 0.3 and 0.85.
  Asserts 0 puts a station on the centre and 0.5 gives an equal flanking pair.
  Checks `station_of_theta` inverts `theta_of_station`, and that an unset value
  is not serialized.
- `library::template_source_tests::a_centred_seat_run_fences_its_design_and_graph_while_an_anchored_one_stays_plain`
  (flat, grouped and embedded-graph designs: plain at 5 unset; 6 when set, refused by a v5 reader, round-trips).
- `ringdesign-graph` `file::…::a_centred_seat_run_fences_graphs_and_presets_while_an_anchored_one_stays_plain`.

Results (`CARGO_INCREMENTAL=0`, `--test-threads=4`):
- `cargo test -p ringdesign-core`: lib 790 passed, 0 failed, 16 ignored (all 16 are ignored on master too); golden 1 passed; examples compile.
- `cargo test -p ringdesign-graph`: 127 passed, 0 failed.

## Not done / notes
- The first push attempts were refused (403, repository not yet attached to the session); pushed after it was attached.
- No GUI or graph-node pin for `centre_phase`. The `layer.seatrun` StructNode coverage check
  requires every *serialized* field to be a pin. A skipped-when-`None` field cannot
  be a pin without serializing `null` into every existing file, and that would break
  byte identity. For now a graph can set the field only through a JSON layer literal,
  which is fenced. Giving the field a GUI control would be a separate small change.
- `examples/bestiarium_manticora.rs` still uses its `c = tan(mπ/2n)` workaround and was
  left untouched to keep its output byte-identical. It can move to `centre_phase` when
  the collection is next regenerated.
- The GUI and Android crates were not rebuilt. Neither has an exhaustive `SeatRunLayer`
  literal (all use `..Default`).
