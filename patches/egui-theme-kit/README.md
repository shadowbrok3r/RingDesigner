# egui-theme-kit

A theme and styling system for [egui](https://github.com/emilk/egui) apps, installed with one call. It publishes a color palette and a sizing table on the egui `Context`, restyles the stock widgets from them, and ships fonts, dock/table/toast chrome, presets, saved themes, frosted glass and a debug panel.

![The basic example: dock cards, stock widgets, themed toasts and a token palette](docs/images/basic.png)

## What you get

- **Tokens and metrics.** `Tokens` (semantic colors and radii) and `Metrics` (control heights, paddings, icon sizes) are derived from the installed `egui::Style` and read back anywhere with `Tokens::get(ctx)` / `Metrics::get(ctx)`.
- **Density and text scale.** Three densities (compact, regular, comfortable) and four text scales, switchable at runtime.
- **Stock widget theme.** Buttons, checkboxes, text edits and separators take the kit look without wrapper widgets; classes pick a button kind or opt out.
- **Fonts.** Inter (Regular, Medium, SemiBold) bundled, medium and semibold weight families, and a `FontConfig` builder for your own faces and icon fonts.
- **Presets and saved themes.** A preset registry with 19 built-in looks (Graphite, Studio, Noir, Noir Glass, Frost, five neon glass, four soft glass, Tokyo Night and Storm, Rerun and Rerun OLED, Default egui), `register_preset` for your own, JSON import and export, bare `egui::Style` files accepted.
- **Theme picker.** An embeddable settings panel (`picker::show` / `picker::window`): preset gallery with live miniatures, saved user themes, customization of density, text size, glass and token colors with contrast warnings, import and export. Persistence goes through a small non-blocking `ThemeStore` trait you can implement over any database; `MemoryStore`, `JsonDirStore` and an eframe-storage store are included.
- **Control kit, title bar and logger (optional).** Painted buttons, toggles, fields, list rows, meters, section headers and a glass card; a custom window title bar with caption buttons, drag and resize edges; an in-app log viewer. Icons are caller-provided, so any icon crate works.
- **Chrome for companion crates.** `egui_dock` styling, `egui-data-table` selection colors, `egui-toast` notifications.
- **Frosted glass.** Real GPU backdrop blur behind floating surfaces, with a kill switch and a flat-fill fallback.
- **Debug panel.** Ctrl+Alt+T opens a Theme window: presets, density, text size, glass dials, token swatches, widget gallery, JSON editor and frame timings.
- **CLI switches.** `--theme <file>`, `--theme-dump`, `--theme-panel`, `--no-frost`.

## Workspace crates

| Crate | Role |
|---|---|
| `egui-theme-kit` | the theme system described here |
| [`egui-icon-pack`](crates/egui-icon-pack/README.md) | SVG icon packs: `Icon` painting, widgets, gallery, optional bundled Phosphor Regular set |
| `egui-icon-pack-build` | `build.rs` generator that turns a folder of SVGs into typed `Icon` constants |
| `backdrop-blur-*` | vendored GPU backdrop-blur crates used by the `glass` feature |

## Install

```toml
[dependencies]
egui-theme-kit = { git = "https://github.com/shadowbrok3r/egui-theme-kit" }
```

Requires Rust 1.95 or newer (edition 2024), egui and eframe 0.37. The `egui_dock`, `egui-data-table` and `egui-toast` features pull the author's forks of those crates (branch `egui-037`) from Git, so the dependency is not publishable to crates.io.

Pin a revision for reproducible builds:

```toml
egui-theme-kit = { git = "https://github.com/shadowbrok3r/egui-theme-kit", rev = "<commit>" }
```

### Cargo features

| Feature | Default | Adds |
|---|---|---|
| `glass` | yes | GPU backdrop blur through a grab-pass backend; enables eframe's `glow` feature |
| `files` | yes | native import/export dialogs (`rfd`) and the user theme directory (`directories`) |
| `dock` | yes | `dock_style` for `egui_dock` |
| `toast` | yes | themed `egui-toast` notifications and the panel's toast tester |
| `data-table` | yes | `table_style` selection colors for `egui-data-table` |
| `picker` | yes | the theme picker window |
| `controls` | no | painted control kit (buttons, toggles, fields, list rows, meters, glass card) |
| `title-bar` | no | custom window title bar with caption buttons, drag area and resize edges (implies `controls`) |
| `logger` | no | in-app log viewer (implies `controls`) |

The defaults match the author's apps, which use every one of these companions. For a smaller dependency tree use `default-features = false` and enable what you need; the core (tokens, metrics, fonts, widget theme, presets, saved themes, debug panel) needs only `egui`, `eframe`, `egui_extras` and a few small crates (`serde`, `serde_json`, `anyhow`, `log`).

`glass` turns on eframe's `glow` feature, which changes the signature of `eframe::App::on_exit`. An app on the wgpu backend should build with `default-features = false`.

## Quick start

```rust
use egui_theme_kit::{KitConfig, debug_panel, glass};

struct App {
    ctx: egui::Context,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        egui_theme_kit::init(cc, KitConfig::default());
        Self { ctx: cc.egui_ctx.clone() }
    }
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        glass::poll_outcome(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let started = std::time::Instant::now();
        let ctx = ui.ctx().clone();
        glass::frost_open_windows(&ctx);

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Hello");
            if ui.add(egui_theme_kit::primary_button("Save")).clicked() {}
        });

        debug_panel::show(&ctx);
        debug_panel::record_ui_time(&ctx, started.elapsed());
        debug_panel::record_frame_cpu(&ctx, frame.info().cpu_usage);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        egui_theme_kit::save(&self.ctx, storage);
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(gl) = gl {
            glass::shutdown(gl);
        }
    }
}
```

The per-frame calls are optional: skip `glass::*` and the panel timings if you use neither frosted glass nor the debug panel's readouts. `crates/egui-theme-kit/examples/basic.rs` is a complete app with a dock, toasts and the panel:

```sh
cargo run -p egui-theme-kit --example basic
cargo run -p egui-theme-kit --example basic -- --theme-panel
```

## Theming concepts

| Concept | What it is | Where it lives |
|---|---|---|
| `Tokens` | Semantic colors (base, raised, field, text, primary, success, warn, error, and so on) and three corner radii. Every color is opaque. | Derived from the installed style; `Tokens::get(ctx)` |
| `Metrics` | Control height, button height, paddings, gaps, window margins, icon sizes, row and bar heights. | Computed from the density; `Metrics::get(ctx)` |
| `Density` | Compact, Regular or Comfortable. Changes geometry only. | `egui_theme_kit::set_density(ctx, d)` |
| `TextScale` | 90, 100, 115 or 130 percent. Changes glyph size on the UI faces by rebuilding the font atlas; it is not a zoom factor. | `egui_theme_kit::set_text_scale(ctx, s)` |

Tokens are colors, metrics are sizes, and the two user knobs are independent: a preset sets the colors, density sets the geometry, text scale sets the type. A `SavedTheme` is the style plus the glass material, density and text scale, serialized as JSON (`saved::theme_to_json` / `theme_from_json`); a bare `egui::Style` file loads too, with Regular density, Normal text scale and the glass material of the matching preset.

Read tokens instead of hard-coding colors:

```rust
let t = egui_theme_kit::tokens(ui.ctx());
ui.painter().rect_filled(rect, t.radius_md, t.raised);
```

### Presets

`presets::apply_preset(ctx, preset)` installs a preset's style, semantic colors and glass material together and keeps the current density and text scale. `presets::apply_saved_theme(ctx, theme)` applies a loaded theme and recognizes which preset it matches.

### Stock widget theme

`widget_theme` registers a theme for egui's built-in `Button`, `Checkbox`, `TextEdit` and `Separator`, so stock widgets already look right. Classes adjust one widget:

```rust
use egui::class::HasClasses as _;
use egui_theme_kit::widget_theme;

ui.add(egui::Button::new("Save").with_class(widget_theme::PRIMARY));
ui.add(egui::Button::new("Delete").with_class(widget_theme::DANGER));
ui.add(egui::Button::new("More").with_class(widget_theme::GHOST));
ui.add(egui::Button::new("Apply").with_class(widget_theme::PROMINENT));
ui.add(egui::Button::new("Plain").with_class(widget_theme::STOCK));
```

`primary_button`, `danger_button`, `ghost_button` and `kit::button(ButtonKind, text)` are shorthands. `widget_theme::set_enabled(ctx, false)` switches the kit look off until the context's memory is reset.

## Fonts and icons

`init` installs Inter as the first `Proportional` face, adds the `medium` and `semibold` families (`fonts::medium(size)`, `fonts::semibold(size)`, `RichText::medium()` and `.semibold()` from `RichTextExt`), and keeps egui's default faces as fallbacks. Inter's Private Use Area mappings are removed in memory so an icon font placed behind it owns those codepoints.

Add your own faces and an icon font through `KitConfig`:

```rust
use egui_theme_kit::{FontConfig, KitConfig};

static MONO: &[u8] = include_bytes!("../assets/MyMono-Regular.ttf");

let fonts = FontConfig::new()
    .face("MyMono", MONO)
    .monospace_first("MyMono")
    .family("Display", ["MyMono"])
    .installer(|fonts| egui_phosphor::add_to_fonts(fonts, egui_phosphor::Variant::Regular));

egui_theme_kit::init(cc, KitConfig::default().fonts(fonts));
```

Order of application: extra faces, the monospace list, named families, `installers` in order, then Inter. Installers are plain `fn(&mut FontDefinitions)` hooks, so an icon pack's `install_fonts` plugs in directly. A family that names a face nobody registered is pruned with a warning rather than crashing. `FontConfig::face_stripped` removes a face's Private Use Area mappings the same way Inter's are.

The registered config survives an egui memory reset and is reused whenever the text scale changes, so icon fonts stay installed across rescales.

## Frosted glass

Surfaces frost when the active theme carries a glass material (`glass::params(ctx)`): `blur_radius`, a `tint` film, `corner_radius` and `presence`. Floating windows frost automatically once `glass::frost_open_windows(ctx)` runs each frame; for your own panels use `glass::glass_frame` or `glass::frost(ui, rect)`.

Caveats:

- **Glow only.** The backend is a grab-pass over eframe's glow renderer. Under wgpu, or without the `glass` feature, every frost call is a no-op and surfaces paint flat fills.
- **Needs a GL 3.3 / GLES 3.0 context.** `glass::install(cc)` builds the backend from `cc.gl`; `init` calls it. Software GL works but is slow.
- **Wire the frame hooks.** `glass::poll_outcome(ctx)` in `App::logic`, `glass::frost_open_windows(ctx)` at the top of `App::ui`, and `glass::shutdown(gl)` in `App::on_exit`.
- **Kill switch.** Three consecutive failed frames disable frosting for the process; `glass::disable(reason)` and `glass::reenable()` do it by hand.
- **Opt out.** `--no-frost` on the command line or `EGUI_THEME_KIT_NO_FROST=1` in the environment turns frosting off for the run.

## Command line and environment

| Switch | Effect |
|---|---|
| `--theme <file.json>` or `--theme=<file.json>` | Apply a theme file at startup; never written back to storage |
| `--theme-dump` | Print the resolved tokens, metrics and fonts as JSON and exit (needs `cli::handle_dump`) |
| `--theme-panel` | Open the Theme window at startup |
| `--no-frost` | Disable the glass pass for this run |

| Variable | Effect |
|---|---|
| `EGUI_THEME_KIT_THEME` | Theme file path, the same as `--theme` when the flag is absent |
| `EGUI_THEME_KIT_NO_FROST` | Any value other than empty, `0`, `false`, `off` or `no` disables frosting |

`init` reads the process arguments itself unless you installed options first (set `KitConfig::parse_cli` to `false` to keep it from reading them). To support `--theme-dump`, handle it before opening a window:

```rust
let options = egui_theme_kit::cli::Options::from_env().unwrap_or_default();
if egui_theme_kit::cli::handle_dump(&options, &fonts)? {
    return Ok(());
}
egui_theme_kit::cli::install(options);
```

## Persistence and startup order

`init` applies the first available theme from:

1. the `--theme` file (or `EGUI_THEME_KIT_THEME`),
2. the active theme of `KitConfig::store`,
3. the theme saved in eframe storage,
4. `KitConfig::default_theme`,
5. the preset named by `KitConfig::default_preset`,
6. the shipped default.

`egui_theme_kit::save(ctx, storage)` from `App::save` writes the live theme to eframe storage. It writes nothing while the theme came from a `--theme` file, and clears the entry while the theme equals the configured default, so a later change of the default reaches users who never customized.

`KitConfig::store` takes an `Arc<dyn ThemeStore>`: any backend that can return and record the active theme, such as a database table. The store is read at startup, and `egui_theme_kit::startup::store(ctx)` hands it to UI code that saves themes.

## Migrating from an app-local copy

- `theme::bootstrap(cc)` becomes `egui_theme_kit::init(cc, KitConfig::default().fonts(...))`. `bootstrap` and `bootstrap_context` remain as deprecated wrappers.
- App-specific fonts and icon faces move into a `FontConfig` (faces, monospace order, installers). `fonts::definitions(scale)` still builds the default stack and `fonts::definitions_with(scale, &config)` builds a configured one.
- Temp-data ids use the `egui_theme_kit.` prefix and the environment variables are `EGUI_THEME_KIT_*`; theme files written by an app-local copy still load.
- Widget class names changed with the prefix; use the `widget_theme::PRIMARY`, `DANGER`, `GHOST`, `PROMINENT` and `STOCK` constants rather than the strings.
- The data-table, dock and toast helpers are behind features; enable the ones you used.

## Development

```sh
cargo test -p egui-theme-kit --all-targets
cargo test -p egui-theme-kit --all-targets --no-default-features
cargo test -p egui-theme-kit --all-targets --all-features
```

Use `--all-targets`: eframe needs a windowing feature on Linux, which the dev-dependencies enable, so a bare `cargo check -p egui-theme-kit --lib` does not build outside an app that depends on eframe with its platform features.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE) at your option. Bundled and vendored components are listed in [THIRD_PARTY.md](THIRD_PARTY.md); the styling code is derived from the author's own application styling code.
