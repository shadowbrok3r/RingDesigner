# Mobile editor 0.12.0 review

The signed ARM64 candidate, native emulator APK, checksums, build logs and selected
screenshots are in `target/releases/ringdesigner-android-0.12.0/`. This candidate
was superseded by 0.13.0, which includes the mobile overhaul and was published
to the app store on 2026-09-12. See `docs/VIEWPORT-TOOLS-REVIEW.md` for release evidence.

## Verified behavior

| Check | Result |
|---|---|
| Phone display | Android 36, 1440 × 3120 at 560 dpi; approximately 411 logical pixels wide |
| Enlarged display | Restarted at 720 dpi; 320 logical pixels wide; navigation, values and tools remain accessible |
| Landscape | Inspector moves beside the ring; paint view shows ring and unrolled canvas side by side |
| Finger-opening handle | Drag changed 17.30 → 20.15 mm with a steady camera; Undo restored US 7 / 17.30 mm |
| Exact entry | Used Android's visible keyboard to enter 18.23 mm; viewport and saved source agreed |
| Keyboard layout | Active numeric input keeps focus and scrolls above the keyboard; ring remains visible |
| Signet creation | Created a signet through Shape, then applied a pattern through the library |
| Complex shape editing | Solstice face handle changed 14.20 → 16.1156 mm; Undo restored 14.20 mm with no other saved parameter changes |
| Stone selection | Nocturne's central marker opens its 2.30 mm stone and bezel controls |
| Ornament selection | Tapping Nocturne's face selects its botanical group |
| Isolation | The isolated botanical preview removes other ornament visually; saved JSON remains identical with all 19 source layers |
| Before | Holding Before restores the earlier visible mesh; saved project SHA-256 remains unchanged |
| Painting | A touch stroke appears on the unrolled band and saves as one `band` stroke; ring preview rebuilds after the stroke |
| Casting | Solstice displays its axial draft screen and pull guides; Nocturne retains its investment process; colour legends explain the view |
| Source preservation | Merely rendering primary controls does not clamp imported out-of-range values |
| Automated checks | 109 Android library tests pass, including 9 editor regressions; primary inspector bounds checked at 280, 320 and 411 logical pixels |
| Packaging | ARM64 and x86_64 APKs verify; versionCode 16780288; signing certificate matches 0.11.0; ZIP integrity and 16 KB ELF alignment pass |

Corrections found by interacting with the emulator included the inspector's
scrollbar width, overflowing pattern sliders, numeric focus changing when the
keyboard hid navigation, and menus covering the Android status area. Checks
also cover duplicate stone names and nested setting paths.

## Screenshots and source checks

Selected captures are in the candidate's `review/screenshots/` directory:

| Interaction | Capture |
|---|---|
| Signet dimensions | [Solstice editor](../target/releases/ringdesigner-android-0.12.0/review/screenshots/solstice-shape.png) |
| Casting colours and guides | [Solstice draft view](../target/releases/ringdesigner-android-0.12.0/review/screenshots/solstice-casting-final.png) |
| Contextual stone controls | [Nocturne stone selection](../target/releases/ringdesigner-android-0.12.0/review/screenshots/nocturne-selected-stone.png) |
| Temporary ornament isolation | [Nocturne surface view](../target/releases/ringdesigner-android-0.12.0/review/screenshots/nocturne-isolation.png) |
| Keyboard and exact values | [Focused field above keyboard](../target/releases/ringdesigner-android-0.12.0/review/screenshots/keyboard-focus-fixed.png) |
| Enlarged display | [320-point shape inspector](../target/releases/ringdesigner-android-0.12.0/review/screenshots/large-display-shape.png) |
| Landscape painting | [Ring beside the drawing canvas](../target/releases/ringdesigner-android-0.12.0/review/screenshots/landscape-paint.png) |

The same review folder includes saved source before/after edits and isolation,
plus the structured 411-point layout report. Earlier captures document interaction
checks during development; the final casting and shape captures use the packaged
candidate. Files under `target/` are build artifacts and are not committed.

## Run the emulator

Requires the installed Android SDK, its Android 36 Google APIs x86_64 system
image, and KVM access. These commands preserve any existing review AVD and leave
the user's original `s26ultra` AVD alone.

```sh
python3 tools/android_emulator.py init
python3 tools/android_emulator.py start --window
```

Omit `--window` for screenshots and touch automation. The review session uses
`emulator-5580`. Do not start a second instance while it is already running.
In a sandboxed agent session, starting the emulator and using adb require access
outside the sandbox.

```sh
python3 tools/mobile_review.py install target/releases/ringdesigner-android-0.12.0/RingDesigner-0.12.0-emulator-x86_64.apk
python3 tools/mobile_review.py launch
python3 tools/mobile_review.py screenshot portrait
python3 tools/mobile_review.py rotate landscape
python3 tools/mobile_review.py screenshot landscape
python3 tools/mobile_review.py density 720
python3 tools/mobile_review.py launch
```

Restart after changing display density so egui initializes at the new scale.
Restore 560 dpi after the narrow-screen review. Test the IME with the actual
keyboard: adb's injected text does not exercise this app's soft-keyboard bridge.
The helper's `seed` and `project` commands load/read ring JSON only in an emulator.

View → Layout bounds (debug) draws control bounds and writes
`files/ringdesigner/layout-debug.json` inside the app's private storage. Its report
checks horizontal clipping; vertical scrolling is intentional. The debug switch
starts off and changes no design data.

## Scope and remaining device checks

- This AVD approximates the configured phone display. It does not emulate Samsung
  firmware, the S-Pen, or the phone's GPU/NPU. Physical S26 Ultra testing remains.
- The emulator runs a native x86_64 build of the same source. ARM translation
  failed in the JNI bridge, so the ARM64 candidate was verified as an artifact,
  not claimed to have run on this emulator or a physical phone.
- Casting colours screen the nominal ring along ±Z. Workshop checks the prepared
  pattern and its configured withdrawal direction. Radial wall readings are not
  a minimum-wall guarantee.
- Direct 3D painting, movable section cuts, stone clearance envelopes and animated
  mould opening are follow-on tools described in `MOBILE-EDITOR.md`.
