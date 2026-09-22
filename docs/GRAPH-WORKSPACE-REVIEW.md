# Graph workspace review — 2026-09-19

Desktop 0.3.0 opens Graph with 35% stacked ring previews and 65% node canvas. The upper camera stays free; the lower orthographic camera follows the selected feature. Both dividers resize, and every viewport arrangement persists per workspace. The Node inspector has 8 px of inner padding.

Graph defaults to View, with parameter editing in the inspector. Edit enables fields on the nodes. Changing modes preserves parameter values and repairs overlapping cards after measurement. Node titles drag without selecting text. The minimap sits at the desktop graph's lower right.

The accent-coloured workspace menu identifies the active editing context. Tile Layout explains unavailable surface tools and offers Edit graph or Make editable; the latter preserves the evaluated design and can be undone. Command search is modal and appears above the viewport cubes. Buttons use the host's standard height: 28 points on desktop, 32 on Android.

Tools → Feature request / bug report on desktop, or File → Feature request / bug report on Android, opens the shared form. Review on GitHub is disabled until title and details are present. It opens a prefilled issue for the user to review and submit; no issue is submitted automatically. Long reports can be copied for pasting. The form includes version, platform and workspace without attaching design files or logs.

Desktop persistence now uses `RingDesigner Desktop` across versions. First launch copies the latest legacy session atomically, retains the original, never overwrites an existing stable session, and falls back to the original path if migration cannot write its destination.

| Check | Evidence |
| --- | --- |
| Inspection-enabled native desktop | OpenGL app controlled through egui inspection at loopback port 5721, isolated Xvfb display and data directory. |
| Graph splits | Dragged both dividers; workspace round-trip test preserves their dimensions and serialized tree. |
| Live preview | Changed band width from 6.00 to 8.50 mm in the inspector; both visible meshes and dimension overlays updated. |
| Title dragging | Dragged Band profile by 44 × 24 screen pixels; only that node moved, with no text selection. |
| Search stacking | Native 1100 × 700 Four layout: command search covers the overlapping navigation cubes. |
| Button heights | Desktop accessibility bounds show File, View, workspace, Tools, History, Undo/Redo, Select and Display at 28 points. |
| Session migration | Isolated native launch copied the saved design and workspace byte-for-byte; original retained. Failure and overwrite protection covered by tests. |
| Regression tests | 220 passed: desktop 38, graph UI 20, shared workbench 30, Android 132. Includes input preservation across View/Edit, resizable split persistence, search stacking, standard heights, report validation and migration failure handling. |
| Initial Android review | ARTEMIS Pro `cc7f837c-e45c-4e4f-8779-451febf71c13`: form, keyboard, orientation, highlighting and clear/orbit verified. A locked graph initially prevented dragging; the final checkpoint passed after unlocking. The cumulative result retains that first failure (9 passed, 1 failed). |
| Final Android review | ARTEMIS Pro `420a34b2-09c8-4383-a182-4a59a65767ad`: 8 passed, 0 failed. Verified 32-point header, floating and workspace controls in portrait/landscape, reachable report controls, disabled empty submission and title dragging with the graph unlocked. |

Native screenshots and Android captures are under `target/graph-workspace-review/`. Inspection launch:

```sh
EGUI_INSPECTION=1 cargo run --features eframe/inspection --bin ringdesigner
```

Linux x86_64 release build succeeded and the optimized binary was launched through the inspection interface. Device telemetry on the final Android build reports File, Undo, Redo, Guide and Panel at exactly 32 points with no horizontal overflow. Both Android ABI release builds succeeded; the final device review used the x86_64 APK on the authorized Android 16 emulator.

Android 0.26.0 is published to the app store, version code `16783872`. The authenticated ARM64 download, local signed APK and store metadata match: 46,781,311 bytes, SHA-256 `83316e7ad5bc4eb8111d999b8c8429acc40e8e8c46e4c93c30ee2af4e8d97395`. The changelog also reports 0.26.0. Release APKs are in `target/releases/ringdesigner-android-0.26.0/`; publication and verification evidence is in `target/graph-workspace-review/android/`.

References: [egui_tiles](https://github.com/rerun-io/egui_tiles), [GitHub issue creation](https://docs.github.com/en/issues/tracking-your-work-with-issues/using-issues/creating-an-issue), [desktop updater contract](DESKTOP-UPDATES.md).
