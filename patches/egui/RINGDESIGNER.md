# Numeric touch policy

Vendored from crates.io `egui` 0.36.0, under its original MIT / Apache-2.0 licenses.

The only behavioral change is `style::Interaction::drag_value_dragging`, default
`true`. RingDesigner's Android theme sets it to `false`: inactive numeric fields
use click sense and a text cursor, so vertical swipes reach the parent scroll
area. Tap-to-type, formatting, ranges, keyboard/accessibility input, and slider
rails keep upstream behavior. This also covers slider value fields and controls
inside shared Workshop and third-party graph widgets without replacing their
numeric parsing or silently setting their sensitivity to zero.

Upstream changes are limited to `src/style.rs` and `src/widgets/drag_value.rs`.
Gesture regressions live in the Android editor tests. Desktop behavior keeps the
default unless a caller explicitly opts out of numeric dragging.
