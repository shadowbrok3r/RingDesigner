# Mastertech theme

Canonical source: `EguiMobile/crates/egui-mobile-theme`.

Uses the supplied Mastertech export unchanged for compact desktop controls and the same colors with 36-point touch controls. `apply` installs both theme variants and a widget provider that keeps small buttons at the same height. Apps retain semantic diagnostic colors and their saved font choices.

App repositories vendor an identical snapshot so standalone and CI builds do not depend on another local checkout or an unpublished git revision. Refresh snapshots from EguiMobile with `python3 scripts/sync-mastertech-theme.py <destination> ...`; run the shared theme tests and each app's UI checks after syncing.

`ThemeConfig` stores palette roles, control height, text scale, spacing, corners and glass. Call `config.apply(ctx, density)` after loading and when `editor(ui, &mut config, density)` reports a change; save using the host app’s preferences. The editor deliberately leaves scrolling to the host. `with_role`, `role_button` and `color` read the live configured role colors. Ordinary widgets stay neutral; main actions use pink, navigation cyan, selection lilac, success mint, warnings amber and destructive actions rose. Existing custom font families/sizes may be applied after the shared style, scaled by `text_scale`.
