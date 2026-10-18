# Desktop appearance

RingDesigner uses `egui-theme-kit` for its desktop titlebar, widget styling,
fonts and theme picker. Open **Tools → Theme** to choose a preset, adjust the
control density and text scale, or save and import a theme. The app saves these
preferences with its existing session; older `theme_config` colours migrate
on first launch. Old custom control sizes map to the nearest kit density and
text scale.

The app draws its titlebar by default. Drag empty titlebar space to move the
window, double-click it to maximize or restore, and drag the edges or corners
to resize. Minimize, maximize/restore and close buttons sit at the right.
`ringdesigner --system-titlebar` uses the OS frame for that launch.

The dependency is pinned to a published revision, independent of the active
sibling checkout. The initial integration needs a one-line compatibility
patch for RingDesigner’s egui fork; see
[the patch notes](../patches/egui-theme-kit/RINGDESIGNER.md).
Use the shared kit API for future desktop chrome. CAD diagnostic colours
remain tied to the core geometry report.
