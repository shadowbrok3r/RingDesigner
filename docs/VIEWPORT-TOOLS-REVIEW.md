# Viewport tools and Thalassa review

M12 implements direct 3D paint/stamps, section planes, clearance envelopes and
mould-opening studies in both native applications. Thalassa’s portable source,
exports and separate application recordings live in [showcase/thalassa](../showcase/thalassa/README.md).

## Runtime checks

| Check | Result |
|---|---|
| Android bounds | Android 36 x86_64 emulator, 1440 × 3120, 560 and 720 dpi: 411 and 320 logical pixels; all tool inspectors remain inside the screen and scroll to additional controls |
| Android landscape | Inspector moves beside the viewport; navigation and long project names remain accessible at increased display scaling |
| Keyboard | Stamp search and numeric controls scroll into view above the native IME; alpha dropdown truncates long names |
| Paint and stamps | Real touch strokes and Mariner placement retained in the portable source; desktop mouse engraving and stamp edits exercised with Undo |
| Sections | X/Y/Z clipping, welded cut contours, caps preserving the bore, draggable offset and local wall chords; view-only source preserved |
| Stone clearance | Seven envelopes; 2.89 mm closest gap. Raising requested clearance to 3 mm highlights crowded pairs. Desktop central-stone width 3.4 → 4.2 mm reduced the closest gap to 2.39 mm; Undo restored the source |
| Mould opening | Built the compensated prepared pattern, played and scrubbed separation, and independently hid an upper half. Obstruction markers and investment-process explanation remain visible |
| Desktop panes | Single and two-across layouts checked. Camera fits the shorter viewport dimension; picking and panning use the same projection |
| Cold Android launch | Switched editing modes and opened section/clearance while fine-detail analysis was pending; screenshots show input taking effect and no new ANR was created |

The phone profile is an emulator approximation. Physical S26 Ultra, Samsung
keyboard and S-Pen pressure/tilt have not been tested. Emulator timing does not
predict performance on the phone.

## Problems caught by actual recordings

- Fine-detail alpha measurement ran on Android’s native UI thread and triggered
  an ANR. A separate detail worker now computes these findings, leaving geometry
  previews and input responsive. Generation checks reject stale results. The
  failed recording and trace are retained under the candidate’s `review/failures/`.
- Near-plane triangles were incorrectly treated as crossings, creating striped
  section caps. Exact side classification plus endpoint welding fixes the cap;
  dense annular and near-plane regression cases cover it.
- Section dragging could consume mouse movement from before the press. Dragging
  now anchors to the press position and initial plane offset, and holds orbit
  capture until release. A real egui input sequence tests the same-frame jump.
- Desktop multi-pane mould views repeatedly restored the saved camera. Restoration
  now follows the active pane/tool transition. Narrow panes also retain the full
  fitted model after adding the inspector.

## Source and geometry

Thalassa has 27 total layer entries / 24 top-level entries, seven stones, five
custom SVG sources, two procedural sources, a drawing and a text source. Nocturne
has 22 / 19 layer entries and three stones. One engraved stroke and the final
Mariner stamp were authored in the Android viewport after the initial Rust-tool
composition; the finished portable file is the export source.

The nominal STL is watertight with 737,280 triangles. Stone inspection reports
zero tight pairs. This is an investment design: the detailed prepared-pattern
inspection found 162 sampled obstruction regions, while the coarser interactive
study found 231. Different sampling resolutions produce different region counts;
neither is evidence that this design releases from a two-part sand mould.

Android snapshots before and after section, clearance and mould inspection have
the same SHA-256. Desktop stamping and stone resizing restore the source with
Undo. A subsequent Android reload introduced only a 1.8e-15 mm JSON floating-point
roundoff in one decal coordinate. Desktop MCP readback also expands stored f32
stroke coordinates; comparisons normalize those coordinates to their source type.

## Automated checks and release

| Check | Result |
|---|---|
| Core library | 428 tests passed before the two final section regressions were added |
| Final core interaction tests | 12 passed, including both new section cases |
| Shared workbench | 4 passed, including bounds/source preservation and pointer-event drag regression |
| Android library | 106 passed |
| Desktop camera | 7 passed, including narrow-pane fit, ray picking and pan consistency |
| Native release builds | Linux desktop, Android ARM64 and Android x86_64 |
| APK verification | Package/version, matching signing certificate, ZIP integrity, 16 KB ZIP and ELF load alignment |

Packaged APKs and verification evidence are in
`target/releases/ringdesigner-android-0.13.0/`; the desktop package is in
`target/releases/ringdesigner-desktop-0.13.0/`. Android 0.13.0 was published to
`https://appstore.shadowbroker.app` on 2026-09-12. The store download matches the
packaged ARM64 APK's size and SHA-256; its version, signing certificate, update
feed and server changelog were verified. The release receipt is `release.json`
in the Android package directory. Logs, screenshots, the cold-start trace
comparison and source-preservation results remain in its `review/` directory.

Sections measure the displayed mesh, not a global minimum wall. Clearance uses the
existing stone-report model with conservative drawn envelopes, not exact fancy-cut
Boolean collision. Mould cavities are sampled inspection surfaces, not production
mould solids. None of these overlays modifies saved ring geometry.

## Recordings

- [Android reel](../showcase/thalassa/highlights/thalassa-android.mp4): 2:11,
  1080 × 2420, H.264, 30 fps.
- [Desktop reel](../showcase/thalassa/highlights/thalassa-desktop.mp4): 2:19,
  1600 × 1060, H.264, 30 fps.

Each has seven chapters: finished ring, engraving, alpha placement, section
movement, local wall measurement, clearance and animated mould opening. Captures
come from `adb screenrecord` and an isolated X11 app window. Captions occupy an
80-pixel band above the capture. Android’s final actual frame is held through
idle capture intervals; the desktop stamp chapter trims a wait using the retained
edit list. No rendered turntable replaces application footage.

Both complete videos decoded without errors. Chapter frames were visually checked
for bounds, visible tool results and dialogs. Wall readings in the recorded
examples are 4.26 mm on Android and 4.28 mm on desktop at their respective cut
positions. The raw takes, action files, edit list, chapter timing and hashes are
retained beside the reels. `tools/window_reel.py` records a take;
`tools/assemble_reels.py` assembles and verifies the final MP4s.
