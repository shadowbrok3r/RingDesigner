# egui-theme-kit compatibility patch

Source: https://github.com/shadowbrok3r/egui-theme-kit at
`ab708cbe629d4b589e604100936e40688039efe1` (0.1.0).

The only Rust source change is `..Default::default()` in `glass_interaction()`.
RingDesigner adds `Interaction::drag_value_dragging` to its egui 0.37 patch;
initializing the remaining fields by default supports that fork and stock egui.
The manifest expands the original workspace dependency versions and resolves
the optional blur backend from the pinned upstream repository. README and licenses are
copied from that revision.

Drop this Cargo patch once an upstream revision includes that initializer.
Do not edit the concurrently developed `/home/shadowbroker/Documents/Rust/egui-theme-kit`
checkout as part of RingDesigner work. Desktop chrome and appearance should
continue to use the shared kit API.
