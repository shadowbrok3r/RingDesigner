# Numeric touch policy

Vendored from crates.io `egui` 0.36.0, under its original MIT / Apache-2.0 licenses.

The numeric interaction change is `style::Interaction::drag_value_dragging`, default
`true`. RingDesigner's Android theme sets it to `false`: inactive numeric fields
use click sense and a text cursor, so vertical swipes reach the parent scroll
area. Tap-to-type, formatting, ranges, keyboard/accessibility input, and slider
rails keep upstream behavior. This also covers slider value fields and controls
inside shared Workshop and third-party graph widgets without replacing their
numeric parsing or silently setting their sensitivity to zero.

Numeric interaction changes are limited to `src/style.rs` and `src/widgets/drag_value.rs`.
Gesture regressions live in the Android editor tests. Desktop behavior keeps the
default unless a caller explicitly opts out of numeric dragging.

Selectable buttons keep an inactive frame by default (`Button::selectable` in
`widgets/button.rs`). This covers `Ui::selectable_label`, `selectable_value`,
combo options and shared graph controls on desktop and Android. Ordinary text
labels remain unframed; selectable controls are visibly interactive at rest.
`containers/menu.rs` preserves the theme's inactive fill and strokes in popup
menus, so menu styling cannot strip those frames or the hover border.

All buttons, including `small_button`, use the theme's `interact_size.y` as their
minimum height. Small buttons retain compact text and horizontal padding without
shrinking beside menu or icon buttons. RingDesigner uses 28 points on desktop
and 32 points on Android; intentionally larger image/touch targets remain larger.

MenuButton reserves an 8-point custom atom for a painted downward triangle.
The caret works with every font and does not change the accessible menu name.
Submenu buttons keep their existing right arrow.
