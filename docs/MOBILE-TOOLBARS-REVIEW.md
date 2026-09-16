# Mobile workspace review — Android 0.14.0

Implemented, reviewed and published on 2026-09-12. Android 0.14.0 is available
from `https://appstore.shadowbroker.app`, replacing 0.13.0. The authenticated
store download matches the signed ARM64 build; its signature, update feed and
server changelog were verified.

## Using the workspace

- Drag the inspector's grip to change its height in portrait or width in
  landscape. Double-tap the grip to restore the default size. Hide/Panel closes
  or restores the inspector.
- Drag the **Tools** header to move the rail. Its minus/plus button collapses
  and expands it. Drag a palette's header independently; × closes that palette.
- **Edit** opens properties for the current mode, tool and selected layer. The
  parameter dropdown chooses the numeric control, with its slider and explanation.
  Stone cut, form and setting style are under **Stone & setting**.
- **Layers** selects or enables design layers. **More** groups construction,
  detailed controls, patterns, painting, inspection, cameras and export commands.
- Tap numeric values to type. Swiping over a number scrolls; slider rails and
  viewport handles still drag. Toolbar drags do not orbit, paint or edit the ring.
- Inspector sizes and normalized toolbar positions survive restart. Rotation
  and keyboard bounds constrain placement without replacing the saved layout.
  **View → Reset workspace layout** or the equivalent More command restores it.

## Verification

The native x86_64 APK ran in the isolated Android 36 AVD at 1440 × 3120 pixels,
560 and 720 dpi (411 and 320 logical points wide), using KVM. The existing
Solstice and Nocturne sources exercised shape, ornament and stone controls.

| Check | Result |
|---|---|
| Portrait resize | Changed the inspector from 17.5% to 44.8% of available space; ring source and camera stayed unchanged |
| Landscape resize | Changed the independent width from 36% to 42.7%; portrait size remained intact |
| Persistence/reset | Inspector sizes, rail collapse and both moved positions survived restarts; reset restored defaults |
| Floating input | Moved the rail and properties palette; moving the palette with Paint active created no stroke or camera movement |
| Layer context | Selected Solstice ornament from Layers; selected Nocturne's central stone from Layers and directly from its viewport marker |
| Numeric scrolling | Swiped over the stone-width field in the narrow, expanded palette: content moved 37.6 points; value, design, camera and header position stayed unchanged |
| Keyboard | Used the visible Gboard keys and Select All to enter an exact 18.23 mm opening; one Undo restored the original source |
| Keyboard dismissal | The viewport regained its full height without another touch; saved layout stayed unchanged |
| Slider | Changed Nocturne's central setting angle from 90° to 172° using the slider; Undo restored the original source after autosave settled |
| Submenus | Parameter picker, expanded Toolbox and stone options remained usable; narrow stone options fit a 204-point palette |
| Tools | Accessed Paint, stone clearance and Mould from their contextual rail buttons without changing source merely by opening them |
| Bounds | Recorded horizontal bounds and checked floating surfaces against viewport bounds in portrait, landscape, keyboard and larger-display layouts; long content scrolls vertically |
| Automated checks | 116 Android library tests and 4 shared workbench tests passed; desktop compilation passed with existing warnings |

Review caught and fixed palettes retaining their initial short height after a
submenu expands, stone selectors widening the palette, duplicate numeric widget
IDs between inspector and palette, and the keyboard leaving the viewport short
after dismissal. The idle app stops polling after keyboard transitions settle.

The global mobile numeric policy is a small opt-out in vendored egui 0.36.0;
see [the patch note](../patches/egui/RINGDESIGNER.md). It covers shared Workshop
and graph widgets as well as the ring inspector. Desktop numeric dragging keeps
egui's default behavior.

## Evidence

Screenshots are actual app captures. Matching `.layout.json` and `.ring.json`
files record geometry, camera, workspace preferences and saved ring source.
Autosave can lag an edit while a mesh build finishes; source comparisons use
settled snapshots, including the post-restart check for the stone-slider Undo.

| View | Screenshot |
|---|---|
| Compact contextual stone controls | [Stone palette](../target/releases/ringdesigner-android-0.14.0/review/stone-palette-final.png) |
| Choosing a parameter | [Parameter menu](../target/releases/ringdesigner-android-0.14.0/review/stone-parameters.png) |
| More/less inspector space | [Small](../target/releases/ringdesigner-android-0.14.0/review/final-panel-small.png), [large](../target/releases/ringdesigner-android-0.14.0/review/final-panel-large.png) |
| Relocated rail | [Moved toolbar](../target/releases/ringdesigner-android-0.14.0/review/final-rail-moved.png) |
| Landscape inspector | [Landscape](../target/releases/ringdesigner-android-0.14.0/review/final-landscape.png) |
| Gboard editing and full-height recovery | [Keyboard](../target/releases/ringdesigner-android-0.14.0/review/keyboard-reflow-fixed.png), [dismissed](../target/releases/ringdesigner-android-0.14.0/review/keyboard-dismissed-fixed.png) |
| 320-point width | [Selected stone](../target/releases/ringdesigner-android-0.14.0/review/narrow-stone-picked.png), [expanded submenu](../target/releases/ringdesigner-android-0.14.0/review/narrow-stone-options.png), [numeric swipe](../target/releases/ringdesigner-android-0.14.0/review/narrow-numeric-scroll.png) |

## Build and repeat

```sh
cargo test --offline -p ringdesigner_android -p ringdesign-workbench --lib
cargo check --offline -p ringdesign-gui

# From crates/ringdesigner-android, with Android SDK/NDK and Java configured:
CARGO_NET_OFFLINE=true cargo apk2 build --target x86_64-linux-android --release
# Copy the emulator APK before the ARM build replaces the shared output path.
CARGO_NET_OFFLINE=true cargo apk2 build --target aarch64-linux-android --release
```

Both APKs have versionCode **16780800**, the expected launcher and ABI, valid
signatures matching 0.13.0, and 16 KB ELF/alignment checks. The ARM64 APK is
13,558,655 bytes; SHA-256:
`c355639a48ea16c1ad2b587f6651b20e72fe2785305813579038a2b92de898e9`.

[Release receipt](../target/releases/ringdesigner-android-0.14.0/release.json),
build/test logs and `SHA256SUMS` are beside the APKs.
The release directory also retains the upload receipt, verified store download,
previous listing, current listing, update feed and server changelog.

Use `tools/android_emulator.py` and `tools/mobile_review.py` as described in
[the emulator setup](MOBILE-EDITOR-REVIEW.md#run-the-emulator).
Enable **View → Layout bounds (debug)** for named-control automation:

```sh
python3 tools/mobile_workspace_review.py state
python3 tools/mobile_workspace_review.py drag inspector/resize 0 -80
python3 tools/mobile_workspace_review.py tap floating/Edit
python3 tools/mobile_workspace_review.py snapshot workspace
```

The helper rejects physical-device serials and stale debug reports from another
app process. Restore density to 560 dpi after narrow-screen checks. The seed
helper now gives newly created app-data directories the app's UID, allowing
preferences and autosave to work even when seeding before first launch.

Physical S26 Ultra, Samsung keyboard, S-Pen and hardware performance remain
untested. The emulator validates the native UI and touch routing; the ARM64 APK
was verified as an artifact. The viewport can be kept clear by moving/collapsing
the rail, closing its palette or hiding the inspector; short landscape layouts
scroll the longer tool lists.
