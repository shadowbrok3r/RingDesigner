# Workspace review — 2026-09-19

Desktop 0.2.0 and Android 0.25.0 share coloured Atelier icons, framed selectable controls, aqua hover feedback and explicit selection clearing. Desktop has one command row, viewport-local display controls, remembered Model/Graph/Surface/Casting/CAD layouts and a larger Node inspector. Android remembers each mode's inspector and gives the graph its own contextual workspace.

| Check | Evidence |
| --- | --- |
| Desktop chrome | Reviewed 1600 × 980 and minimum 1100 × 700 layouts, including four viewports. Compact panes group display controls into a menu. |
| Native OpenGL viewport | Opened Ophidian's editable graph; verified aqua feature hover, pink selection and active Node border, Edit graph routing, and Escape clearing. Screenshots in `target/workspace-review/desktop-native-*.png`. |
| Desktop workspace and update behavior | Executable tests cover graph visibility, workspace restoration/persistence, menu/layout reachability, Escape priority, session preservation, compatible release selection, checksums and failed workers. |
| Android interaction review | ARTEMIS Pro `806226b6-6775-4157-a92c-7d9b8ffe27a5`: **8 passed, 0 failed** on `emulator-5554` (Android 16, 1440 × 3120). Cleared graph/node/feature selection, hid cutters independently, then verified free orbit and empty-space deselection. |
| Repeatable device check | `python3 tools/check_mobile_selection.py --serial emulator-5554 --feature 735 430`: six checks passed after installing 0.25.0. Requires the documented Model/Shape view, layout telemetry and a rooted emulator; the feature point depends on the camera pose. |
| Menu visibility | Vendored egui preserves inactive fills and border/hover strokes inside popup menus as well as selectable buttons. |

Automated validation: **213 tests passed** (33 desktop, 20 graph UI, 29 shared workbench, 131 Android). The desktop suite includes rendered layout review images. `git diff --check` is clean.

The initial ARTEMIS run passed five workspace checks but flagged remaining pink geometry after clearing. Inspection distinguished the cutter overlay from selection; the follow-up verified the separate Cutters button and the cleared selection fields. No geometry was edited during verification. ARTEMIS reports and deterministic results are saved in `target/workspace-review/`.

The Escape regression also verifies that an open menu closes before selection is cleared. The final Android APK was reinstalled, the six device checks passed again, and ADB verified that Escape dismisses the framed File menu without changing the cleared selection; layout telemetry reported no overflow.

The native desktop checked the real GitHub feed successfully and reported no newer compatible desktop release. The release workflow and asset contract are in [DESKTOP-UPDATES.md](DESKTOP-UPDATES.md). The first desktop release is not published; live replacement from a published release and Windows/macOS execution remain unverified. Checksums, version/platform filtering, session persistence and the Linux release build are tested locally.

Release builds succeeded for Linux x86_64 and Android ARM64/x86_64. **Android 0.25.0 is published** to the existing authenticated app store, version code `16783616`. The downloaded APK matches the 46,769,023-byte ARM64 build and SHA-256 `491333e925b2dc5e0b8be1a1520bd8221f9cf8a6b5f2f5378a3ae9d928d25407`. Metadata, checksums and logs are in `target/workspace-review/`. Desktop binary: `target/release/ringdesigner`; no desktop GitHub release was published.
