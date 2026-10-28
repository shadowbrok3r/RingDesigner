# Historical snapshot - no longer used by Cargo

The mobile picker and interaction compatibility changes are now in the shared
`egui-theme-kit` repository at `09d1e793076357ec5d86df7e1d6771d0306f2e70`.
All targets use that git revision; the Cargo theme-kit patch was removed.
This snapshot is retained to preserve the earlier local checkout.

# egui-theme-kit compatibility patch

Source: https://github.com/shadowbrok3r/egui-theme-kit at
`2788be59a8aee6cd21e3146aaf2419f39f77d4e4` (0.1.0).

The runtime sources include the completed custom title bar, window-manager input
handoff, resize zones, physical-pixel hairlines, saved chrome preferences and
Windows client-area correction from that revision. The manifest expands the
upstream workspace dependency versions and pins the optional blur backend.

RingDesigner-specific compatibility and mobile adjustments:

- Default remaining fields in `glass_interaction()` to support the patched egui
  `Interaction::drag_value_dragging` field.
- Exclude that numeric-input policy from visual preset matching, so mobile themes
  retain their preset identity and semantic colors after restarting.
- Constrain the theme name field, wrap gallery search controls, and keep density
  and text-size controls on separate rows on narrow screens. Android presents
  these as wrapping inline choices, keeping them above the native navigation bar.
  Its color editor uses three columns, and color/glass labels stay on one line.
- Allow the picker to query an external glass renderer. Android supplies its
  existing renderer without enabling eframe's desktop glow integration; the
  desktop-only material preview is omitted on that host.

Keep these adjustments when refreshing the vendored source. Remove the Cargo
patch once upstream includes the equivalent changes. The shared library checkout
is maintained separately; RingDesigner continues to use the shared kit API.
