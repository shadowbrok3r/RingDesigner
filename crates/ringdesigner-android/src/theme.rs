//! Shared Mastertech chrome with one touch-sized control rhythm.
use egui::containers::scroll_area::ScrollBarVisibility;
use egui::{Color32, CornerRadius, Stroke};
use egui_mobile::egui;
use egui_mobile_theme::palette;

pub const PINK: Color32 = palette::ACCENT;
pub const PINK_BRIGHT: Color32 = palette::LINK;
pub const AQUA: Color32 = palette::INFO;
pub const AQUA_BRIGHT: Color32 = palette::INFO;
pub const INK: Color32 = palette::TEXT;
pub const INK_DIM: Color32 = palette::TEXT_DIM;
pub const RIM: Color32 = palette::BORDER;
pub const RIM_BRIGHT: Color32 = palette::BORDER;

pub fn apply(ctx: &egui::Context) {
    apply_config(ctx, &egui_mobile_theme::ThemeConfig::default());
}

pub fn apply_config(ctx: &egui::Context, config: &egui_mobile_theme::ThemeConfig) {
    ctx.add_plugin(crate::editor::pen::PenHover::default());
    // Keep the user's text and explicit sizing choices; compact only the app's
    // default control height and whitespace around otherwise unchanged text.
    let mut compact = config.clone();
    compact.control_height = Some(config.control_height.unwrap_or(MENU_ROW_H));
    compact.apply(ctx, egui_mobile_theme::Density::Touch);
    // RingDesigner's pen workflow uses taps and explicit keyboard entry for numeric values.
    ctx.all_styles_mut(|s| {
        s.interaction.drag_value_dragging = false;
        s.spacing.item_spacing *= 0.8;
        s.spacing.button_padding *= 0.8;
    });
}

pub fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(palette::WINDOW)
        .stroke(Stroke::new(1.0, RIM))
        .corner_radius(CornerRadius::same(5))
        .inner_margin(10)
}

/// Frame for a bar pinned to a screen edge. Kept transparent where the frost
/// runs — the grab-pass tint IS the bar — with just the tap-height margin.
pub fn bar() -> egui::Frame {
    egui::Frame::new().inner_margin(egui::Margin::symmetric(8, 6))
}

/// Selectable button that keeps its frame when unselected — egui drops it, which reads as a plain
/// label rather than a control.
pub fn selectable<'a>(selected: bool, atoms: impl egui::IntoAtoms<'a>) -> egui::Button<'a> {
    egui::Button::selectable(selected, atoms).frame_when_inactive(true)
}

/// [`egui::Ui::selectable_label`] at a fixed size, with a persistent frame plus a selection rim
/// when selected — egui's `interact_selectable` leaves `bg_stroke` off for the selected state.
pub fn selectable_label<'a>(
    ui: &mut egui::Ui,
    selected: bool,
    size: impl Into<egui::Vec2>,
    text: impl egui::IntoAtoms<'a>,
) -> egui::Response {
    let resp = ui.add_sized(size, selectable(selected, text));
    if selected {
        ui.painter().rect_stroke(
            resp.rect,
            CornerRadius::same(3),
            Stroke::new(1.0, egui_mobile_theme::color(ui.ctx(), egui_mobile_theme::Role::Accent)),
            egui::StrokeKind::Inside,
        );
    }
    resp
}

/// Vertical scroll area; scrollbar only when content overflows.
pub fn scroll_vertical() -> egui::ScrollArea {
    egui::ScrollArea::vertical().scroll_bar_visibility(ScrollBarVisibility::VisibleWhenNeeded)
}

// --- Menus -------------------------------------------------------------------

/// Compact default, with unchanged text and configurable sizing in Theme.
pub const MENU_ROW_H: f32 = 32.0;

/// Give a menu's rows a framed, touch-sized look: egui's `menu_style` strips the rest-state fill
/// and every accent rim and squashes `button_padding` to 2×0, so an entry otherwise reads as bare
/// text on an 18px line. Call once at the top of a popup body; children inherit it.
pub fn menu_row_style(ui: &mut egui::Ui) {
    egui_mobile_theme::menu_style(ui);
}

/// A menu / combo-box button whose popup opens *upward* and scrolls.
///
/// `Ui::menu_button` only flips its popup above the button when it wouldn't otherwise fit — but
/// egui's screen rect extends under the Android navigation bar, so a short menu "fits" below and
/// ends up covering the nav bar and the system gesture area. Everything in a bottom control bar
/// uses this instead, which always prefers opening upward.
pub fn up_menu<R>(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    content: impl FnOnce(&mut egui::Ui) -> R,
) {
    let _ = menu_popup(
        ui,
        label,
        None,
        egui::RectAlign::TOP_START,
        &[egui::RectAlign::TOP_END, egui::RectAlign::BOTTOM_START],
        egui::PopupCloseBehavior::CloseOnClick,
        content,
    );
}

/// How tall a menu opened from `anchor` may grow: the room actually available on the side it opens
/// toward, less a small margin.
pub fn menu_height_cap(ctx: &egui::Context, anchor: egui::Rect, align: egui::RectAlign) -> f32 {
    let screen = content_bounds(ctx);
    let down = anchor.bottom().max(screen.top());
    let below = screen.bottom() - down;
    let above = anchor.top().min(screen.bottom()) - screen.top();
    let (room, flipped) = match align.parent.y() {
        egui::Align::Max => (below, above),
        egui::Align::Min => (above, below),
        egui::Align::Center => (screen.height(), screen.height()),
    };
    // The floor is `min`'d rather than clamped: a landscape phone with the keyboard up leaves the
    // content rect under 160pt tall, and `f32::clamp` panics outright on min > max.
    let floor = 160.0_f32.min(screen.height());
    (room.max(flipped) - 24.0).clamp(floor, screen.height().max(floor))
}

/// How wide a popup may grow. egui clips a popup's painting at `content_rect` but never shrinks
/// it, so a row wider than the screen cuts the menu off at the right edge instead of wrapping.
pub fn menu_width_cap(ctx: &egui::Context) -> f32 {
    (content_bounds(ctx).width() - 24.0).max(80.0)
}

/// egui-mobile applies Android insets to its root Ui. Popups are separate Areas
/// and need the same bounds explicitly, including the keyboard's visible area.
pub fn set_content_bounds(ctx: &egui::Context, rect: egui::Rect) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::unique("mobile-safe-content"), rect));
}

pub fn content_bounds(ctx: &egui::Context) -> egui::Rect {
    ctx.data(|d| d.get_temp(egui::Id::unique("mobile-safe-content")))
        .unwrap_or_else(|| ctx.content_rect())
}

/// A menu button whose popup is bounded to the screen on both axes.
pub fn menu_popup<R>(
    ui: &mut egui::Ui,
    label: impl Into<egui::WidgetText>,
    min_size: Option<egui::Vec2>,
    align: egui::RectAlign,
    alternatives: &'static [egui::RectAlign],
    close_behavior: egui::PopupCloseBehavior,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::Response {
    use egui::containers::menu::MenuConfig;
    let label = label.into();
    let name = label.text().to_owned();
    let response = if name == "Tools" && min_size.is_some_and(|size| size.x < 96.0) {
        ui.add_sized(min_size.unwrap(), egui::Button::new(&name).truncate())
    } else {
        ringdesign_workbench::icons::button(
            ui, ringdesign_workbench::icons::Icon::for_label(&name),
            if name == "View" { "" } else { &name }, false,
            min_size.unwrap_or(egui::vec2(0.0, ui.spacing().interact_size.y)),
        ).response
    };
    let clicked = response.clicked();
    crate::editor::layout::record(ui, format!("menu/{name}"), response.rect);
    let config = MenuConfig::default().close_behavior(close_behavior);
    let cap = menu_height_cap(ui.ctx(), response.rect, align);
    let width_cap = menu_width_cap(ui.ctx());
    let popup = egui::Popup::menu(&response)
        .open_memory(if clicked {
            Some(egui::SetOpenCommand::Toggle)
        } else {
            None
        })
        .align(align)
        .align_alternatives(alternatives)
        .gap(4.0)
        .close_behavior(config.close_behavior)
        .style(config.style.clone())
        .info(
            egui::UiStackInfo::new(egui::UiKind::Menu)
                .with_tag_value(MenuConfig::MENU_CONFIG_TAG, config),
        );
    let safe = content_bounds(ui.ctx()).shrink(4.0);
    let size = popup
        .get_expected_size()
        .unwrap_or(egui::vec2(width_cap, cap + 16.0));
    let size = egui::vec2(size.x.min(width_cap), size.y.min(cap + 16.0));
    let desired = align.align_rect(&response.rect, size, 4.0).min;
    let position = egui::pos2(
        desired
            .x
            .clamp(safe.left(), (safe.right() - size.x).max(safe.left())),
        desired
            .y
            .clamp(safe.top(), (safe.bottom() - size.y).max(safe.top())),
    );
    popup
        .at_position(position)
        .align(egui::RectAlign::BOTTOM_START)
        .align_alternatives(&[])
        .show(|ui| {
            // egui sizes a popup's Area on a one-off sizing pass seeded from
            // `spacing.default_area_size`, and that becomes the Ui's `max_rect` forever after.
            // Claiming the full cap during the sizing pass only lets the Area record a tall
            // enough size; the list still shrinks to its content afterwards.
            if ui.is_sizing_pass() {
                ui.set_min_height(cap);
            }
            // A consistent row width makes every action easy to tap, while the
            // safe-area cap keeps long labels inside narrow phone screens.
            let width = (width_cap - ui.spacing().menu_margin.sum().x).min(300.0);
            ui.set_width(width);
            scroll_vertical()
                .max_height(cap)
                .max_width(width)
                .show(ui, |ui| {
                    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                    menu_row_style(ui);
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), content).inner
                })
                .inner
        });
    response
}
