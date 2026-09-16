# Vendored crates

Forks this workspace builds against through the root `[patch.crates-io]`
table for version compatibility and the mobile numeric-input policy.

| crate | upstream | what changed |
| --- | --- | --- |
| `egui` 0.36.0 | https://github.com/emilk/egui | opt-in text-only numeric interaction; see [patch notes](egui/RINGDESIGNER.md) |
| `egui-snarl` 0.11.0 | https://github.com/zakarumych/egui-snarl | repinned from egui 0.35 to 0.36 |
| `egui-scale` 0.5.0 | https://github.com/zakarumych/egui-scale | repinned from egui 0.35 to 0.36 |

The snarl and scale crates are byte-identical copies of the forks under
`EguiMobile/patches/` and `wirelab/patches/` in the sibling trees; the
graph-ui crate's test `vendored_copies_match_the_siblings` checks that
when a sibling is present. The next egui bump repins all three copies
together (follow-up: publish the fork to `shadowbrok3r/egui-snarl` and
use a git patch like egui-phosphor).
