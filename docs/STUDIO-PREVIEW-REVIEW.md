# Studio preview review — 0.15.0 candidate

The Android and desktop viewports now share reflective studio lighting. Android
refines settled geometry automatically and remembers the selected quality.

| Android mesh tier | Grid | Triangles | Expanded vertex buffer |
| --- | --- | ---: | ---: |
| Fast / active edits | 384 × 144 | 110,592 | 15.2 MiB |
| Detailed (default) | 1024 × 320 | 655,360 | 90 MiB |
| Showcase | 1536 × 448 | 1,376,256 | 189 MiB |

Buffers exclude the indexed CPU mesh, analysis, library and driver allocations.
The existing worker builds the heavier mesh after editing stops; camera movement
alone does not regenerate it. View → Mesh detail changes quality. View → Metal &
polish chooses the alloy and surface finish. These preferences survive restart.
View → Zoom and the floating Camera & display tools provide tap-based 20% zoom
steps, alongside pinch and the existing camera presets.

The material uses colored Fresnel reflection, gradient softboxes, a dark studio
environment, approximate bore occlusion, highlight compression and sRGB encoding.
Stones use a separate dielectric response. This is an analytic environment
approximation, without ray-traced shadows, refraction or dispersion. Diagnostic
draft/wall/normal/parting colors retain their existing shading. The software PNG
and GIF exporter still uses its existing renderer; viewport capture shows the new
lighting. Conductor/dielectric reference:
[Filament material model](https://google.github.io/filament/main/filament.html).

## Verified behavior

- Android 16, S26-profile AVD, 1440 × 3120 at 560 dpi, software graphics.
- ARTEMIS MCP screenshots and hierarchy inspection preceded UI checks. Native
  egui controls are largely absent from the Android hierarchy, so screenshot
  grounding and the app's named bounds report are needed.
- Actual View-menu selection produced 655,360 triangles in Detailed mode and
  1,376,256 in Showcase. Showcase and the selected alloy survived force-stop and
  relaunch. The bounds report showed no horizontal overflow in the checked views.
- Nocturne was loaded as an existing rendering reference, not presented as a new
  creation session. The Aster source was also inspected in the desktop viewport.
- Desktop selection of Aster's 12 mm cushion border previously clamped its width
  to 6 mm and changed the casting result. The width range now follows the actual
  band; displaying layer numeric controls preserves imported values. Android's
  border-width control also covers the actual band.
- 116 Android tests and 21 desktop tests pass, including the regression that
  displays the wide casting border without changing its source. Both native
  release builds compile, and the shaders compile on desktop GL and Android GLES.
- Android's draft overlay was visually checked after the shader change and then
  restored to Metal. Nocturne's saved source matches every original field, allowing
  1e-12 for two scalar values that changed at the last floating-point digit during
  JSON serialization.
- The recording helper supports a settled-mesh gate before capture and validates
  the encoded dimensions. The emulator's guest AVC recorder rejected 1440 × 3120
  and silently fell back to 720 × 1280. That capture was rejected. The helper now
  defaults to the emulator console's native VP9 display recorder at 20 Mbps and
  30 fps, accepts an explicit size, retains raw WebM, and produces H.264 MP4.
  A 1.5-second pre-roll lets initial encoder blocks settle before any UI actions;
  it remains in the raw capture and is trimmed from the presentation clip.
  The optional guest screenrecord path receives an explicit size to prevent its
  implicit fallback. Neither path upscales video.

## ARTEMIS setup

The initial MCP process had no recognized credential. An existing Gemini key in
the user's `.zshrc` was connected to ARTEMIS's private `.env`, then a fresh stdio
MCP session was started. Live diagnosis passed all five required checks. Gemini
validated; an optional OpenAI credential did not. No key is included in artifacts.

The emulator manager assumed a graphical session and Qt aborted before boot.
Its Linux launch path now uses `-no-window` with software graphics when neither
DISPLAY nor WAYLAND_DISPLAY is available. A fresh manager booted `s26ultra` in
20 seconds. The AVD configuration backup is retained locally. The fix is in
`/home/shadowbroker/Documents/Rust/Mobile/artemis/artemis/core/diagnostics/emulator_manager.py`.

Autonomous viewport review trace:
`5dd9989a-48a7-4f21-8935-2f93fa7b30a2` (Pro, 176.4 seconds). Five checkpoints
passed: quality, finish comparison, workspace cleanup, uncropped orbit and final
review. Pinch was **not performed**: the agent's single-touch tools do not support
it. Its final zoom remained 1.0. Agent-authored praise of image quality is subjective;
the retained screenshots provide the actual evidence.

Follow-up trace `6e34cd34-d1b2-4e42-a204-d4879357d9f5` (Flash, 47.3 seconds)
selected Signet 3/4, opened View → Zoom and tapped in/out/in. Device telemetry
confirmed zoom 1.2, Showcase 1,376,256 triangles, no build in progress and no
reported horizontal overflow. The submenu remains open between zoom taps.
Screenshot grounding supplied the native-coordinate action path; hierarchy labels
alone cannot locate this egui interface. Allow rendered frames between popup
transitions: back-to-back ADB taps can miss a menu and hit the viewport.

Both task logs were inspected after completion; neither has an error/traceback
entry. The Pro notes and both final statuses are retained in the candidate review
directory. Device validation used the emulator, not a physical S26 GPU. The
optional OpenAI provider remains unavailable; these tasks ran with Gemini.

## Preview artifacts

These are existing-ring material and viewport reviews. Nocturne retains its
investment-cast design and is not represented as a new sand-cast creation.
The raw recordings, timed UI actions and screenshots at each main operation are
beside the clips under `target/releases/ringdesigner-android-0.15.0/review/clips/`.

| Platform | Presentation clip | Display capture | Duration |
| --- | --- | --- | --- |
| Android | [MP4](../target/releases/ringdesigner-android-0.15.0/review/clips/android/studio-preview.mp4) | 1440 × 3120; 80 px caption added above | 20 s |
| Desktop | [MP4](../target/releases/ringdesigner-android-0.15.0/review/clips/desktop/studio-preview.mp4) | 1600 × 980 app window; 80 px caption added above | 24 s |

Both presentation clips are H.264 at 30 fps. The Android clip shows the settled
Showcase mesh and gentle orbit; the desktop clip compares Polished, Satin and
Rough before returning to Polished and the reference angle. Encoded dimensions,
duration, frame counts and full-stream decoding are checked in
`review/video-verification.json`. This proves the capture resolution, not 30 fps
rendering performance on a physical phone. The mesh gate was checked with bounds
telemetry; debug outlines were then hidden for the unchanged reference mesh.

## Showcase follow-up

Aster Atelier now replaces the earlier M13 composition. Its rounded seal and
shallow flower pass compensated-pattern withdrawal screening at 0.10 and 0.075 mm
with zero detected obstructions or unresolved rays. Low-draft and narrow sand-slot
findings remain; physical casting is unverified. Both apps built the same twelve
editable layers from a blank band using fourteen guide operations. ARTEMIS explored
construction and directly operated the recorded Android layer/mould demonstration.
The final reveals use settled 1,376,256-triangle meshes. Native-resolution creation
reels, independent source/mesh checks and casting limits are documented in
[the Aster Atelier review](../showcase/aster-atelier/README.md).

0.15.0 is a local candidate, not a published store release.
