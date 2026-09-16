# Mobile visual editor

The ring stays visible while its properties change. A selection mode determines
what a tap selects; one inspector shows the controls for that selection.

## Layout

Portrait:

```text
Design name                 Undo  Redo  View
┌──────────────────────────────────────────┐
│                                          │
│                  live ring               │
│            dimensions / handles          │
│                                          │
│ selected feature       Before / Guides   │
└──────────────────────────────────────────┘
Selection                                  −
Primary controls, values and a short explanation
Shape     Surface     Stones     Casting    Tools
```

Landscape puts the inspector beside the ring. Drag its grip to resize the panel;
portrait height and landscape width persist separately. Hide it for a larger
viewport. The movable left tool rail and contextual properties palette provide
editing access within the viewport, including when the inspector is hidden.
See [MOBILE-TOOLBARS.md](MOBILE-TOOLBARS.md) for the current 0.14 layout.

Buttons use 32-point rows and the mode bar uses 36 points, as requested for the
more compact workspace. Secondary controls disclose on demand. Numeric values
accept typed input only; slider bars and on-model handles remain draggable.
The Android safe area and keyboard define available space. Keyboard transitions
keep layout repainting until the viewport has recovered its available height.

## Selection modes

| Mode | Tap selects | Visual feedback | Main edits |
|---|---|---|---|
| Shape | Band or signet head | Bore/width/wall/head dimensions and handles | Fit, width, thickness, shoulder shape, face proportions |
| Surface | Ornament contributing at that point | Picked point and isolated layer preview | Pattern scale, relief, rotation, placement and window |
| Stones | Stone or setting | Seat outline, stone dimensions, selected setting | Stone size, setting and position |
| Casting | Surface for inspection | Draft/wall colours, pull arrows and parting guide | Casting process, related findings and workshop setup |

Mode changes affect interaction and guides. They never silently remove geometry
from a saved design or export. Include/exclude a design layer remains an explicit
edit with undo. A tap with overlapping ornament offers the contributing layers;
hidden/disabled layers must not intercept selection.

## Feedback

- Label dimensions in millimetres on the object. A handle and its numeric field
  edit the same source parameter; exact entry remains available.
- Explain each primary parameter in a sentence beside its control. Prefer “face
  height” to an unexplained “rise”; preserve technical terminology in details.
- Keep the camera steady during an edit. Changing width must not trigger a refit
  that visually cancels the change.
- Build lightweight previews during continuous dragging, followed by the settled
  casting analysis. Discard obsolete results and keep the UI responsive.
- Let the user hold Before to compare with the model before the latest edit.
- Separate “pending check” from a completed finding. A previous safe result must
  not appear to validate a changed design.
- Keep Undo/Redo reachable without opening a tool page.

## Visual system

| Role | Treatment |
|---|---|
| Viewport | Existing neutral charcoal `#121214`, polished metal as the main visual |
| Text | Existing cool white `#E9E9EF`; default sans, 13-point body, 11–12-point secondary |
| Selected feature | Existing pink `#FF3D8B` |
| Dimensions and handles | Aqua `#2BE2D6` |
| Manufacturing advice | Amber `#EFB368`; red for a failed check |
| Structure | Quiet dividers; no repeated cards around individual settings |

The visual emphasis belongs to the jewelry and its editing handles. This retains
the app's established palette while replacing the crowded tab-and-sheet layout.
There is no dashboard or introductory screen between opening the app and editing.

## Verification

Use an isolated Android 36 AVD based on the existing `s26ultra` display profile:
1440 × 3120 pixels at 560 dpi, approximately 411 × 891 logical points before
system insets. This matches the configured screen target; it does not emulate
Samsung firmware, an S-Pen or the phone's GPU/NPU performance.

Exercise a native x86_64 emulator APK with touch input and capture screenshots.
Build ARM64 separately for the phone. Check portrait, landscape, larger display scaling, text entry,
long layer names, loaded signets and overlapping sheets. Verify that a visible
parameter edit changes the model, picking opens the correct controls, Undo restores
the prior design, and hiding a guide does not change exported geometry.

Use KVM outside the filesystem sandbox; sandbox visibility of `/dev/kvm` is not
evidence that the host lacks acceleration. Emulator timing is not a performance
measurement for the physical phone. Results and reproducible commands are in
[MOBILE-EDITOR-REVIEW.md](MOBILE-EDITOR-REVIEW.md).

## Direct viewport tools

Implemented in M12 on Android and desktop: seam-aware 3D painting and alpha
stamps, draggable section planes with local wall chords, stone-clearance
envelopes, and animated sampled mould cavities with obstruction markers.
Controls use the existing mode and inspector structure. See
[VIEWPORT-TOOLS.md](VIEWPORT-TOOLS.md) and
[VIEWPORT-TOOLS-REVIEW.md](VIEWPORT-TOOLS-REVIEW.md).

M14 adds resizable inspectors, floating contextual controls and text-only numeric
fields. Verification is recorded in [MOBILE-TOOLBARS-REVIEW.md](MOBILE-TOOLBARS-REVIEW.md).

Further work: exploded CAD assemblies with per-part visibility, mould obstruction
to repair-preview shortcuts, saved camera/selection workspaces and a searchable
command palette. Physical S-Pen pressure and Samsung keyboard testing remain.
