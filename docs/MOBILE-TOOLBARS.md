# Mobile viewport workspace

The lower inspector becomes a resizable reference panel. A movable tool rail
starts on the left of the ring; its contextual palettes expose editing controls
beside the model. This extends the existing jewelry editor and its selection modes.

## Visual design

Keep the established charcoal `#121214`, panel `#141419`, white `#E9E9EF`,
aqua `#2BE2D6`, pink `#FF3D8B` and amber `#EFB368`. The existing sans uses
13-point body text, 12-point buttons and 11-point hints. Aqua marks handles;
pink marks the selected tool. The ring remains the largest visual element.

```text
Project                         Undo  Redo  View
┌────────────────────────────────────────────┐
│ [grip]                                     │
│ Select                                     │
│ Edit                                       │
│ Section                  live ring         │
│ Layers                                     │
│ More                                       │
│ Before   [palette grip / selected feature]  │
│ Guides   Parameter picker → value + slider  │
│ Panel    Explanation / secondary submenus   │
├──────────────── drag to resize ────────────┤
│ Inspector title                       Hide │
│ Full controls, scroll within chosen space  │
└────────────────────────────────────────────┘
Shape       Surface       Stones     Casting
```

Buttons use 32-point rows, the mode bar 36 points, with distinct drag grips.
The rail and palette move only by their headers. Their positions and inspector
extent persist; rotation, display scaling and the keyboard constrain them to
the available area without overwriting the saved placement. View provides a
workspace reset. Palettes use existing edit/undo paths and follow the selected
mode, layer and tool. Long controls scroll inside the palette; submenus expose
less frequent commands. Opening a palette does not force the lower inspector open.

The properties palette initially sits below the ring, to the right of the rail.
It exposes one selected numeric parameter at a time, with its explanation and
retained slider. Stone cut, form and setting type live in a secondary submenu.
Paint, stamp, section, clearance and mould tools show their own compact controls.
The rail's minus/plus button collapses or restores it. More opens grouped paths
to construction, detailed editors, patterns, inspection, cameras and exports.

Numeric controls retain typed values, units, limits, keyboard input and slider
bars. Their displayed numbers do not consume drag gestures or change while
scrolling. The policy applies to all phone UI, including shared Workshop and
graph widgets. Actual slider bars and on-model dimension handles remain draggable.

Review against the brief: the change concentrates controls around the jewelry
viewport, preserves contextual selection and uses clear handles rather than
adding more permanent rows. The lower panel and palettes complement each other;
users can close either and still reach the tools.

## Acceptance

- Resize, collapse and restore the inspector in portrait and landscape.
- Move both floating surfaces, open contextual submenus and reset their layout.
- Select ornament and stones; edit from the palette and undo the resulting change.
- Swipe across numeric controls without changing the design; type exact values
  and drag the retained slider bars successfully.
- Check 411/320-point widths, keyboard, rotation and persistence in the emulator.
- Keep debug bounds available and record screenshots and signed APK evidence.

Implementation and review results: [MOBILE-TOOLBARS-REVIEW.md](MOBILE-TOOLBARS-REVIEW.md).
