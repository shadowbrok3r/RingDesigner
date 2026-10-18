//! Every kit control in every state, for a live preview of the active theme.

use eframe::egui::{
    Align, Button, ComboBox, DragValue, Frame, Id, Layout, Sense, TextEdit, Ui, Vec2,
    class::HasClasses as _, containers::menu,
};

use super::{
    BarKind, ButtonKind, Dropdown, Icon, IconBox, IconButton, IconLabel, IconLink, ListItem,
    MenuButton, MenuItem, Pill, Preview, ProgressBar, Role, Section, Segment, Slider, SpinnerSize,
    TextArea, TextButton, TextField, Tokens, ValueField, badge, bar, button, callout, card,
    checkbox, count_badge, dialog_buttons_enabled, dialog_contents, dropdown, elided_label,
    empty_state, form, form_row, ghost_button, hairline, heading, icon_button, icon_hyperlink,
    icon_label, icon_link, icon_text_button, icons, kit, loading_row, loading_state, menu_body,
    menu_check_item, menu_danger_item, menu_item, menu_picker, menu_separator, meter, note,
    panel_header, pill, primary_button, search_field, section_header, section_label, segmented,
    sides, spinner, status_label, subheading, submenu, text_field, tinted_spinner, toggle,
    usage_meter, value_field, vline, with_preview,
};
use crate::widget_theme;

/// Width of the gallery's text fields.
const FIELD_W: f32 = 160.0;
/// Width of the gallery's numeric fields.
const VALUE_W: f32 = 76.0;
/// Width of the gallery's dropdowns.
const DROPDOWN_W: f32 = 120.0;
/// Width of the gallery's progress bars and meters.
const BAR_W: f32 = 220.0;
/// Width of the gallery's list.
const LIST_W: f32 = 280.0;
/// Height of the gallery's vertical rules.
const VLINE_H: f32 = 18.0;
/// Size of a sample in the icon strip.
const ICON_SAMPLE: f32 = 20.0;
/// Widest the sample dialog gets.
const DIALOG_W: f32 = 420.0;
/// Inner margin of the sample dialog.
const DIALOG_MARGIN: i8 = 16;

/// An answer on a checklist row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Pass,
    Fail,
    Skip,
}

/// Values the gallery's live controls edit.
#[derive(Clone, Debug)]
struct Samples {
    text: String,
    search: String,
    code: String,
    notes: String,
    size: f32,
    percent: u8,
    threads: u32,
    choice: usize,
    unset: usize,
    on: bool,
    off: bool,
    pill: usize,
    answer: Option<Answer>,
    view: usize,
    scale: f32,
    picked: usize,
    stock_text: String,
    stock_notes: String,
    stock_line: String,
}

impl Default for Samples {
    fn default() -> Self {
        Self {
            text: "Project notes".to_owned(),
            search: String::new(),
            code: "A-1024".to_owned(),
            notes: "Release candidate builds clean.\nBenchmarks are within noise.".to_owned(),
            size: 20.0,
            percent: 85,
            threads: 0,
            choice: 1,
            unset: usize::MAX,
            on: true,
            off: false,
            pill: 0,
            answer: Some(Answer::Pass),
            view: 0,
            scale: 1.5,
            picked: 0,
            stock_text: String::new(),
            stock_notes: String::new(),
            stock_line: "One line".to_owned(),
        }
    }
}

const CHOICES: [(usize, &str); 4] = [
    (0, "Compact"),
    (1, "Regular"),
    (2, "Comfortable"),
    (3, "A very long option label"),
];

fn gallery_id() -> Id {
    Id::unique("egui_theme_kit.controls.gallery")
}

/// Every kit control in collapsible sections, in rest, hover, press, focus and disabled states.
pub fn show(ui: &mut Ui) {
    show_with(ui, &[]);
}

/// [`show`] with `extra` named icons added to the icon section.
pub fn show_with(ui: &mut Ui, extra: &[(&str, Icon)]) {
    let id = gallery_id();
    let mut s = ui.data(|d| d.get_temp::<Samples>(id)).unwrap_or_default();
    ui.push_id(id, |ui| {
        buttons(ui);
        icon_buttons(ui);
        fields(ui, &mut s);
        alignment(ui, &mut s);
        meters(ui, &mut s);
        labels(ui, &mut s);
        sections(ui);
        surfaces(ui);
        lists(ui);
        bars(ui, &mut s);
        empty(ui);
        dialog(ui);
        menus(ui, &mut s);
        stock(ui, &mut s);
        icon_samples(ui, extra);
    });
    ui.data_mut(|d| d.insert_temp(id, s));
}

/// The gallery's stock egui widgets section on its own.
pub fn show_stock(ui: &mut Ui) {
    let id = gallery_id();
    let mut s = ui.data(|d| d.get_temp::<Samples>(id)).unwrap_or_default();
    ui.push_id(id, |ui| stock(ui, &mut s));
    ui.data_mut(|d| d.insert_temp(id, s));
}

/// `add` drawn once per state: rest, hovered, pressed, focused, disabled.
fn states(ui: &mut Ui, mut add: impl FnMut(&mut Ui, &str)) {
    ui.horizontal(|ui| {
        add(ui, "Rest");
        with_preview(ui, Preview::Hovered, |ui| add(ui, "Hover"));
        with_preview(ui, Preview::Pressed, |ui| add(ui, "Pressed"));
        with_preview(ui, Preview::Focused, |ui| add(ui, "Focus"));
        ui.add_enabled_ui(false, |ui| add(ui, "Disabled"));
    });
}

fn buttons(ui: &mut Ui) {
    if !section_header(ui, "gallery.buttons", icons::SQUARE, "Buttons") {
        return;
    }
    for kind in ButtonKind::ALL {
        let prominent = matches!(kind, ButtonKind::Primary | ButtonKind::Danger);
        states(ui, |ui, label| {
            let _ = TextButton::new(label)
                .kind(kind)
                .prominent(prominent)
                .show(ui);
        });
    }
    states(ui, |ui, label| {
        let _ = icon_text_button(ui, icons::PLUS, label);
    });
    ui.horizontal(|ui| {
        let t = Tokens::get(ui.ctx());
        let _ = TextButton::new("KEY-7F2K")
            .icon(icons::SEARCH)
            .ink(t.warn)
            .mono(true)
            .show(ui);
        let _ = TextButton::new("Order 1000001")
            .kind(ButtonKind::Ghost)
            .ink(t.success)
            .show(ui);
        let _ = TextButton::new("Disabled")
            .icon(icons::X)
            .enabled(false)
            .show(ui);
        let _ = IconButton::new(icons::GEAR, IconBox::Md)
            .tip("Disabled")
            .enabled(false)
            .show(ui);
    });
}

fn icon_buttons(ui: &mut Ui) {
    if !section_header(ui, "gallery.icon_buttons", icons::GRID, "Icon buttons") {
        return;
    }
    ui.horizontal(|ui| {
        for size in [IconBox::Sm, IconBox::Md, IconBox::Lg, IconBox::Tool] {
            let _ = icon_button(ui, icons::GEAR, size, false, "Settings");
        }
        vline(ui, VLINE_H);
        for size in [IconBox::Sm, IconBox::Md, IconBox::Lg, IconBox::Tool] {
            let _ = icon_button(ui, icons::CLOCK, size, true, "History");
        }
    });
    states(ui, |ui, label| {
        let _ = icon_button(ui, icons::PLUS, IconBox::Md, false, label);
        let _ = icon_button(ui, icons::PLUS, IconBox::Md, true, label);
        let t = Tokens::get(ui.ctx());
        let _ = IconButton::new(icons::X, IconBox::Md)
            .tint(t.error)
            .tip(label)
            .show(ui);
    });
    ui.horizontal(|ui| {
        let t = Tokens::get(ui.ctx());
        for (icon, selected, count, tone, tip) in [
            (icons::INFO, false, 3, t.error, "Alerts"),
            (icons::CLOCK, true, 12, t.secondary, "Snoozed"),
            (icons::LIST, false, 120, t.primary, "Threads"),
        ] {
            let response = icon_button(ui, icon, IconBox::Md, selected, tip);
            count_badge(ui, response.rect, count, tone);
        }
    });
}

fn fields(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.fields", icons::LIST, "Fields") {
        return;
    }
    ui.horizontal(|ui| {
        let mut empty = String::new();
        let _ = text_field(ui, &mut empty, "Project name", FIELD_W);
        let _ = text_field(ui, &mut s.text, "", FIELD_W);
        let _ = search_field(ui, &mut s.search, "Search", FIELD_W);
    });
    ui.horizontal(|ui| {
        let mut focused = "Focused".to_owned();
        with_preview(ui, Preview::Focused, |ui| {
            text_field(ui, &mut focused, "", FIELD_W)
        });
        let mut hovered = "Hovered".to_owned();
        with_preview(ui, Preview::Hovered, |ui| {
            text_field(ui, &mut hovered, "", FIELD_W)
        });
        let mut disabled = "Disabled".to_owned();
        let _ = TextField::new(&mut disabled)
            .width(FIELD_W)
            .enabled(false)
            .show(ui);
        let _ = Dropdown::new("gallery.disabled", &mut s.choice, &CHOICES)
            .width(DROPDOWN_W)
            .enabled(false)
            .show(ui);
    });
    ui.horizontal(|ui| {
        let invalid = s.code.len() != 6;
        let _ = TextField::new(&mut s.code)
            .hint("Code")
            .width(FIELD_W)
            .icon(icons::FILE)
            .mono(true)
            .char_limit(6)
            .invalid(invalid)
            .show(ui);
        let _ = ValueField::new(&mut s.threads, 0..=64)
            .width(VALUE_W)
            .suffix("thr")
            .formatter(|v| {
                if v == 0.0 {
                    "auto".to_owned()
                } else {
                    format!("{v:.0}")
                }
            })
            .parser(|text| {
                if text.trim().eq_ignore_ascii_case("auto") {
                    Some(0.0)
                } else {
                    text.trim().parse().ok()
                }
            })
            .show(ui);
        let _ = Dropdown::new("gallery.unset", &mut s.unset, &CHOICES)
            .width(DROPDOWN_W)
            .placeholder("Pick a density")
            .show(ui);
    });
    ui.horizontal(|ui| {
        let _ = value_field(ui, &mut s.size, 1.0..=2500.0, "px", VALUE_W);
        let _ = value_field(ui, &mut s.percent, 0..=100, "%", VALUE_W);
        with_preview(ui, Preview::Focused, |ui| {
            value_field(ui, &mut s.size, 1.0..=2500.0, "px", VALUE_W)
        });
        let _ = dropdown(ui, "gallery.density", &mut s.choice, &CHOICES, DROPDOWN_W);
        with_preview(ui, Preview::Hovered, |ui| {
            dropdown(ui, "gallery.hover", &mut s.choice, &CHOICES, DROPDOWN_W)
        });
    });
    ui.horizontal(|ui| {
        let _ = toggle(ui, &mut s.on, "Backdrop blur");
        let _ = toggle(ui, &mut s.off, "Auto-assign");
        with_preview(ui, Preview::Focused, |ui| toggle(ui, &mut s.off, "Focus"));
        ui.add_enabled_ui(false, |ui| toggle(ui, &mut s.on, "Disabled"));
    });
    ui.horizontal(|ui| {
        let _ = checkbox(ui, &mut s.on, "Run on startup");
        let _ = checkbox(ui, &mut s.off, "Clean on exit");
        with_preview(ui, Preview::Hovered, |ui| {
            checkbox(ui, &mut s.off, "Hovered")
        });
        with_preview(ui, Preview::Focused, |ui| checkbox(ui, &mut s.on, "Focus"));
        ui.add_enabled_ui(false, |ui| checkbox(ui, &mut s.on, "Disabled"));
    });
    let _ = TextArea::new(&mut s.notes).hint("Notes").rows(3).show(ui);
    let mut log = "[09:14:02] worker: lane 0 started\n[09:14:03] worker: lane 1 started";
    let _ = TextArea::new(&mut log).mono(true).rows(2).show(ui);
}

/// One row holding every row control.
fn alignment(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.alignment", icons::MENU, "Row alignment") {
        return;
    }
    ui.horizontal(|ui| {
        let _ = button(ui, "Apply");
        let _ = icon_button(ui, icons::PLAY, IconBox::Md, false, "Run");
        let _ = TextField::new(&mut s.text)
            .width(FIELD_W)
            .icon(icons::SEARCH)
            .show(ui);
        let _ = value_field(ui, &mut s.size, 1.0..=2500.0, "px", VALUE_W);
        let _ = dropdown(ui, "gallery.align", &mut s.choice, &CHOICES, DROPDOWN_W);
        let _ = toggle(ui, &mut s.on, "Live");
    });
    let _ = sides(
        ui,
        |ui| subheading(ui, icons::LIST, "Left side"),
        |ui| {
            let _ = button(ui, "Right");
            let _ = icon_button(ui, icons::GEAR, IconBox::Md, false, "Settings");
        },
    );
    form(ui, "gallery.form", |ui| {
        let _ = form_row(ui, "Name", |ui| text_field(ui, &mut s.text, "", FIELD_W));
        let _ = form_row(ui, "Size", |ui| {
            value_field(ui, &mut s.size, 1.0..=2500.0, "px", VALUE_W)
        });
        let _ = form_row(ui, "Density", |ui| {
            dropdown(
                ui,
                "gallery.form.choice",
                &mut s.choice,
                &CHOICES,
                DROPDOWN_W,
            )
        });
        let _ = form_row(ui, "Enabled", |ui| toggle(ui, &mut s.on, ""));
    });
}

fn meters(ui: &mut Ui, s: &mut Samples) {
    if !section_header(
        ui,
        "gallery.meters",
        icons::CLOCK,
        "Progress, sliders and choices",
    ) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        let _ = ProgressBar::new(0.62)
            .width(BAR_W)
            .text("stage 3/8")
            .show(ui);
        let _ = ProgressBar::new(0.35)
            .width(BAR_W)
            .percentage(true)
            .show(ui);
        let _ = ProgressBar::indeterminate().width(BAR_W / 2.0).show(ui);
    });
    ui.horizontal(|ui| {
        for (share, tone) in [
            (0.9, t.success),
            (0.55, t.warn),
            (0.2, t.error),
            (0.7, t.info),
        ] {
            let _ = meter(ui, share, BAR_W / 2.0, tone);
        }
    });
    ui.horizontal(|ui| {
        for share in [0.3, 0.75, 0.95] {
            let _ = usage_meter(ui, share, BAR_W / 2.0);
        }
        let _ = ProgressBar::new(0.82)
            .width(BAR_W)
            .percentage(true)
            .usage(true)
            .show(ui);
    });
    ui.horizontal(|ui| {
        let _ = Slider::new(&mut s.scale, 0.1..=10.0).value("x").show(ui);
        with_preview(ui, Preview::Hovered, |ui| {
            Slider::new(&mut s.scale, 0.1..=10.0).show(ui)
        });
        ui.add_enabled_ui(false, |ui| Slider::new(&mut s.scale, 0.1..=10.0).show(ui));
    });
    ui.horizontal(|ui| {
        let _ = segmented(
            ui,
            &mut s.answer,
            &[
                Segment::new(Some(Answer::Pass), "Pass")
                    .icon(icons::CHECK_CIRCLE)
                    .tone(t.success),
                Segment::new(Some(Answer::Fail), "Fail")
                    .icon(icons::ERROR)
                    .tone(t.error),
                Segment::new(Some(Answer::Skip), "N/A").icon(icons::MINUS_CIRCLE),
            ],
        );
        let _ = segmented(
            ui,
            &mut s.view,
            &[
                Segment::new(0, "List").icon(icons::LIST),
                Segment::new(1, "Grid").icon(icons::GRID),
                Segment::new(2, "Menu").icon(icons::MENU),
            ],
        );
    });
    ui.horizontal(|ui| {
        let _ = Pill::new("Passed")
            .icon(icons::CHECK_CIRCLE)
            .selected(true)
            .tone(t.success)
            .show(ui);
        let _ = Pill::new("Failed")
            .icon(icons::ERROR)
            .selected(true)
            .tone(t.error)
            .show(ui);
        let _ = Pill::new("Skipped").icon(icons::MINUS_CIRCLE).show(ui);
    });
}

fn labels(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.labels", icons::DOT, "Labels and chips") {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let _ = heading(ui, icons::GRID, "Heading");
    let _ = subheading(ui, icons::LIST, "Sub-heading");
    section_label(ui, "Section label");
    ui.horizontal(|ui| {
        let _ = icon_label(ui, icons::FOLDER, "workstation-01");
        let _ = elided_label(
            ui,
            "A label far too long for the room it is given in this row",
            180.0,
        );
    });
    ui.horizontal(|ui| {
        let _ = status_label(ui, icons::CHECK_CIRCLE, "Drivers current", t.success);
        let _ = status_label(ui, icons::WARNING, "Firmware is old", t.warn);
        let _ = status_label(ui, icons::ERROR, "Checksum failed", t.error);
        let _ = IconLabel::new(icons::CLOCK, "2 minutes ago")
            .small(true)
            .tone(t.text_dim)
            .show(ui);
    });
    ui.horizontal(|ui| {
        let _ = IconLabel::new(icons::GEAR, "Eight-core")
            .strong(true)
            .show(ui);
        let _ = IconLabel::new(icons::FILE, "relay://workstation-01")
            .mono(true)
            .show(ui);
        let _ = loading_row(ui, "Loading drivers");
    });
    ui.allocate_ui(Vec2::new(LIST_W, 0.0), |ui| {
        let _ = IconLabel::new(
            icons::ERROR,
            "connection closed before the completion frame arrived",
        )
        .tone(t.error)
        .truncate(true)
        .show(ui);
    });
    ui.horizontal(|ui| {
        let _ = icon_link(ui, icons::FILE, "Order 1000001");
        let _ = icon_hyperlink(
            ui,
            icons::CARET_RIGHT,
            "Project page",
            "https://example.com",
        );
        with_preview(ui, Preview::Hovered, |ui| {
            icon_link(ui, icons::INFO, "Hovered link")
        });
        let _ = IconLink::new("Small link")
            .icon(icons::PLUS)
            .small(true)
            .show(ui);
    });
    let _ = IconLabel::new(
        icons::INFO,
        "A wrapped, selectable note that runs on past the end of the row so it breaks onto a \
         second line beside its icon.",
    )
    .wrap(true)
    .selectable(true)
    .tone(t.text_dim)
    .show(ui);
    ui.horizontal(|ui| {
        let _ = badge(ui, "Passed", t.success);
        let _ = badge(ui, "3 warnings", t.warn);
        let _ = badge(ui, "Failed", t.error);
        let _ = badge(ui, "Info", t.info);
        let _ = badge(ui, "12", t.primary);
        let _ = badge(ui, "Draft", t.secondary);
    });
    ui.horizontal(|ui| {
        for (i, (icon, label)) in [
            (icons::LIST, "Open"),
            (icons::CLOCK, "Waiting"),
            (icons::CHECK_CIRCLE, "Done"),
        ]
        .into_iter()
        .enumerate()
        {
            if pill(ui, icon, label, s.pill == i).clicked() {
                s.pill = i;
            }
        }
        with_preview(ui, Preview::Hovered, |ui| pill(ui, None, "Hover", false));
        let _ = Pill::new("Threads")
            .icon(icons::MENU)
            .trailing(icons::CARET_DOWN)
            .show(ui);
    });
    ui.horizontal(|ui| {
        let _ = spinner(ui, SpinnerSize::Sm);
        let _ = spinner(ui, SpinnerSize::Md);
        let _ = spinner(ui, SpinnerSize::Lg);
        vline(ui, VLINE_H);
        let _ = tinted_spinner(ui, SpinnerSize::Md, t.warn);
        let _ = tinted_spinner(ui, SpinnerSize::Md, t.info);
    });
}

fn sections(ui: &mut Ui) {
    if !section_header(ui, "gallery.sections", icons::MENU, "Sections") {
        return;
    }
    let _ = Section::new("gallery.section.open", "Loaded modules")
        .icon(icons::FOLDER)
        .show(ui, |ui| {
            let _ = icon_label(ui, icons::FILE, "libexample.so");
        });
    let _ = Section::new("gallery.section.closed", "Registers")
        .icon(icons::LIST)
        .default_open(false)
        .show(ui, |ui| {
            ui.label("rax 0000000000000000");
        });
    let t = Tokens::get(ui.ctx());
    let _ = Section::new("gallery.section.rich", "Team")
        .icon(icons::GRID)
        .suffix("(you)", None)
        .suffix("4", None)
        .suffix("\u{2022} 2 working", t.info)
        .default_open(false)
        .header(ui);
    let _ = Section::new("gallery.section.trailing", "Hardware inventory")
        .icon(icons::GEAR)
        .icon_tone(t.secondary)
        .title_tone(t.secondary)
        .suffix("(6 components)", None)
        .trailing(|ui| {
            let _ = IconLabel::new(icons::CLOCK, "1 job in progress")
                .tone(t.warn)
                .show(ui);
        })
        .default_open(false)
        .header(ui);
}

fn surfaces(ui: &mut Ui) {
    if !section_header(ui, "gallery.surfaces", icons::SQUARE, "Cards and callouts") {
        return;
    }
    let t = Tokens::get(ui.ctx());
    panel_header(ui, icons::FOLDER, "Inventory", |ui| {
        let _ = icon_button(ui, icons::PLUS, IconBox::Md, false, "Add");
    });
    card(ui, |ui| {
        let _ = subheading(ui, icons::GEAR, "Primary volume 1 TB");
        let _ = status_label(ui, icons::CHECK_CIRCLE, "SMART healthy", t.success);
    });
    #[cfg(feature = "glass")]
    {
        use super::glass_card;
        glass_card::titled_card(ui, icons::GEAR, "Frosted card", Some("glass"), |ui| {
            let _ = status_label(
                ui,
                icons::CHECK_CIRCLE,
                "Frosts its backdrop on glass presets",
                t.success,
            );
        });
        glass_card::titled_group(ui, icons::LIST, "Row card", Some("no frost"), |ui| {
            let _ = icon_label(ui, icons::FILE, "Cheap enough to repeat per row");
        });
        glass_card::row_card(ui, |ui| {
            let _ = icon_label(ui, icons::FOLDER, "A full-width row card");
        });
    }
    callout(ui, t.warn, icons::WARNING, |ui| {
        ui.label("A scan is still running. Leave it running and keep working.");
    });
    callout(ui, t.info, None, |ui| {
        ui.label("A callout without an icon.");
    });
    let _ = loading_state(ui, "Loading inventory");
}

fn lists(ui: &mut Ui) {
    if !section_header(ui, "gallery.lists", icons::LIST, "Lists and trees") {
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.allocate_ui(Vec2::new(LIST_W, 0.0), |ui| {
        let _ = ListItem::new("Desktop").icon(icons::GRID).show(ui);
        let _ = ListItem::new("Downloads")
            .icon(icons::FILE)
            .selected(true)
            .show(ui);
        with_preview(ui, Preview::Hovered, |ui| {
            ListItem::new("Documents").icon(icons::FOLDER).show(ui)
        });
        let _ = ListItem::new("Root").icon(icons::FOLDER).show_node(
            ui,
            "gallery.tree.root",
            true,
            |ui| {
                let _ = ListItem::new("system")
                    .icon(icons::FOLDER)
                    .detail("24.1 GB")
                    .show_node(ui, "gallery.tree.system", false, |_| {});
                let _ = ListItem::new("swapfile")
                    .icon(icons::FILE)
                    .icon_tone(t.text_faint)
                    .detail("16 GB")
                    .reserve_caret(true)
                    .show(ui);
            },
        );
        let _ = ListItem::new("Build output")
            .icon(icons::FILE)
            .subtitle("Compiling 3 crates")
            .progress(0.45)
            .action(icons::X, "Cancel")
            .show(ui);
        let _ = ListItem::new("Inbox")
            .icon(icons::MENU)
            .unread(true)
            .trailing(icons::CHECK_CIRCLE, t.success)
            .show(ui);
        let _ = ListItem::new("Syncing").spinner(None).show(ui);
        for (i, label) in ["First stripe", "Second stripe", "Third stripe"]
            .into_iter()
            .enumerate()
        {
            let _ = ListItem::new(label)
                .icon(icons::FILE)
                .striped(i % 2 == 1)
                .show(ui);
        }
    });
}

fn bars(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.bars", icons::MENU, "Bars") {
        return;
    }
    bar(ui, BarKind::Tool, |ui| {
        let _ = icon_button(ui, icons::GRID, IconBox::Lg, true, "Overview");
        vline(ui, VLINE_H);
        let _ = icon_button(ui, icons::PLAY, IconBox::Md, false, "Run");
        let _ = icon_button(ui, icons::PAUSE, IconBox::Md, false, "Pause");
        let _ = icon_button(ui, icons::STOP, IconBox::Md, false, "Stop");
        vline(ui, VLINE_H);
        let _ = search_field(ui, &mut s.search, "Filter", FIELD_W);
        let _ = ghost_button(ui, "Clear");
    });
    bar(ui, BarKind::Status, |ui| {
        let t = Tokens::get(ui.ctx());
        let _ = icon_label(ui, icons::CHECK_CIRCLE, "Connected");
        let _ = badge(ui, "Relay", t.info);
        let _ = elided_label(ui, "Last sync 2 minutes ago", 200.0);
    });
    bar(ui, BarKind::Footer, |ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let buttons = [(Role::Cancel, "Cancel"), (Role::Primary, "Save")];
            dialog_buttons_enabled(ui, &buttons, &[true, false]);
        });
    });
}

fn empty(ui: &mut Ui) {
    if !section_header(ui, "gallery.empty", icons::FILE, "Empty state") {
        return;
    }
    empty_state(
        ui,
        icons::FOLDER,
        "Nothing here yet",
        "Items you add show up here.",
        &[(Role::Primary, "Add item"), (Role::Secondary, "Import")],
    );
}

fn dialog(ui: &mut Ui) {
    if !section_header(ui, "gallery.dialog", icons::SQUARE, "Dialog") {
        return;
    }
    let width = ui.available_width().min(DIALOG_W) - 2.0 * DIALOG_MARGIN as f32;
    let buttons = [(Role::Cancel, "Keep"), (Role::Danger, "Delete")];
    let title = "Delete the project?";
    let body = "Every file in the project is removed and cannot be recovered.";
    Frame::window(ui.style())
        .inner_margin(DIALOG_MARGIN)
        .show(ui, |ui| {
            dialog_contents(ui, Some(icons::WARNING), title, width, &buttons, |ui| {
                ui.label(body);
            });
        });
}

fn menus(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.menus", icons::MENU, "Menus") {
        return;
    }
    let (t, _) = kit(ui);
    states(ui, |ui, label| {
        let _ = MenuButton::new(label).icon(icons::MENU).show(ui, |ui| {
            let _ = menu_item(ui, icons::GEAR, "Settings");
        });
    });
    ui.horizontal(|ui| {
        let _ = MenuButton::new("Tools")
            .icon(icons::GEAR)
            .tint(t.primary)
            .show(ui, |ui| {
                let _ = menu_check_item(ui, icons::GEAR, "Tools", true);
            });
        let _ = MenuButton::new("")
            .icon(icons::GRID)
            .tip("Layout")
            .show(ui, |ui| {
                let _ = menu_item(ui, icons::PLUS, "Save layout");
            });
        let _ = menu_picker(
            ui,
            icons::FOLDER,
            "Workspace",
            &mut s.picked,
            &[(0, "Personal"), (1, "Team"), (2, "Archive")],
        );
    });
    Frame::menu(ui.style()).show(ui, |ui| {
        menu_body(ui, |ui| {
            let _ = menu_check_item(ui, icons::LIST, "My tasks", true);
            let _ = menu_check_item(ui, icons::GRID, "Team tasks", false);
            menu_separator(ui);
            let _ = menu_item(ui, icons::SEARCH, "Open session");
            with_preview(ui, Preview::Hovered, |ui| {
                menu_item(ui, icons::PLAY, "Run benchmark")
            });
            let _ = submenu(ui, icons::GEAR, "Firmware", |ui| {
                let _ = menu_item(ui, icons::CARET_DOWN, "Download update");
            });
            menu_separator(ui);
            let _ = menu_item(ui, None, "Properties");
            let _ = MenuItem::new("Archive 0 idle")
                .icon(icons::FOLDER)
                .enabled(false)
                .show(ui);
            let _ = MenuItem::new("Batch actions")
                .icon(icons::GRID)
                .tone(t.warn)
                .submenu(ui, |ui| {
                    let _ = menu_item(ui, icons::PLAY, "Restart all");
                });
            let _ = menu_danger_item(ui, icons::X, "Delete file");
            with_preview(ui, Preview::Hovered, |ui| {
                menu_danger_item(ui, icons::X, "Delete folder")
            });
        });
    });
}

/// Stock egui widgets beside their kit equivalents, under the kit-look switch.
fn stock(ui: &mut Ui, s: &mut Samples) {
    if !section_header(ui, "gallery.stock", icons::SQUARE, "Stock egui widgets") {
        return;
    }
    let mut on = widget_theme::enabled(ui.ctx());
    if toggle(ui, &mut on, "Kit look for stock widgets").changed() {
        widget_theme::set_enabled(ui.ctx(), on);
    }
    if ui.visuals().override_text_color.is_some() {
        let _ = note(
            ui,
            icons::INFO,
            "This theme sets a text colour, so stock labels use it and the primary class takes the selected look.",
            Tokens::get(ui.ctx()).text_dim,
        );
    }
    let kinds = [
        (ButtonKind::Neutral, "Neutral"),
        (ButtonKind::Primary, "Primary"),
        (ButtonKind::Danger, "Danger"),
        (ButtonKind::Ghost, "Ghost"),
    ];
    ui.horizontal(|ui| {
        for (kind, label) in kinds {
            let button = match kind.class() {
                Some(class) => Button::new(label).with_class(class),
                None => Button::new(label),
            };
            let _ = ui.add(button);
        }
        vline(ui, VLINE_H);
        for (kind, label) in kinds {
            let _ = TextButton::new(label).kind(kind).show(ui);
        }
    });
    ui.horizontal(|ui| {
        let prominent = Button::new("Prominent")
            .with_class(widget_theme::PRIMARY)
            .with_class(widget_theme::PROMINENT);
        let _ = ui.add(prominent);
        let _ = ui.small_button("Small");
        let _ = ui.add(Button::new("Frameless").frame(false));
        let _ = ui.selectable_label(true, "Selected");
        let _ = ui.selectable_label(false, "Unselected");
        vline(ui, VLINE_H);
        let _ = primary_button(ui, "Prominent");
        let _ = segmented(
            ui,
            &mut s.view,
            &[Segment::new(0, "Selected"), Segment::new(1, "Unselected")],
        );
    });
    ui.horizontal(|ui| {
        let current = CHOICES
            .iter()
            .find(|(value, _)| *value == s.choice)
            .map_or("", |(_, label)| *label);
        ComboBox::from_id_salt("gallery.stock.combo")
            .width(DROPDOWN_W)
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (value, label) in CHOICES {
                    let _ = ui.selectable_value(&mut s.choice, value, label);
                }
            });
        let _ = ui.add(
            DragValue::new(&mut s.size)
                .range(1.0..=2500.0)
                .suffix(" px"),
        );
        vline(ui, VLINE_H);
        let _ = dropdown(
            ui,
            "gallery.stock.dropdown",
            &mut s.choice,
            &CHOICES,
            DROPDOWN_W,
        );
        let _ = value_field(ui, &mut s.size, 1.0..=2500.0, "px", VALUE_W);
    });
    ui.horizontal(|ui| {
        let edit = TextEdit::singleline(&mut s.stock_text)
            .hint_text("Project name")
            .desired_width(FIELD_W);
        let _ = ui.add(edit);
        let mut fixed = "Read-only";
        let _ = ui.add(TextEdit::singleline(&mut fixed).desired_width(FIELD_W));
        vline(ui, VLINE_H);
        let _ = text_field(ui, &mut s.stock_text, "Project name", FIELD_W);
        let mut fixed = "Read-only";
        let _ = TextField::new(&mut fixed).width(FIELD_W).show(ui);
    });
    ui.horizontal_top(|ui| {
        for text in [&mut s.stock_notes, &mut s.stock_line] {
            let edit = TextEdit::multiline(text)
                .hint_text("Describe the change")
                .desired_rows(3)
                .desired_width(FIELD_W);
            let _ = ui.add(edit);
        }
        vline(ui, VLINE_H);
        for text in [&mut s.stock_notes, &mut s.stock_line] {
            let _ = TextArea::new(text)
                .hint("Describe the change")
                .rows(3)
                .width(FIELD_W)
                .show(ui);
        }
    });
    ui.horizontal(|ui| {
        let _ = ui.checkbox(&mut s.on, "Run on startup");
        let _ = ui.checkbox(&mut s.off, "Clean on exit");
        vline(ui, VLINE_H);
        let _ = checkbox(ui, &mut s.on, "Run on startup");
        let _ = checkbox(ui, &mut s.off, "Clean on exit");
    });
    let _ = ui.separator();
    let _ = hairline(ui);
    ui.horizontal(|ui| {
        let _ = ui.menu_button("Stock menu", |ui| {
            let _ = ui.button("Open");
            let _ = ui.button("Save as");
            let _ = ui.separator();
            let _ = ui.checkbox(&mut s.on, "Wrap lines");
        });
        vline(ui, VLINE_H);
        let _ = MenuButton::new("Kit menu").show(ui, |ui| {
            let _ = menu_item(ui, icons::FOLDER, "Open");
            let _ = menu_item(ui, icons::FILE, "Save as");
            menu_separator(ui);
            let _ = menu_check_item(ui, None, "Wrap lines", s.on);
        });
    });
    ui.horizontal_top(|ui| {
        Frame::menu(ui.style()).show(ui, |ui| {
            ui.set_width(super::layout::MENU_MIN_W);
            ui.with_layout(Layout::top_down_justified(Align::Min), |ui| {
                menu::menu_style(ui.style_mut());
                let _ = ui.button("Open");
                let _ = ui.button("Save as");
                let _ = ui.separator();
                let _ = ui.checkbox(&mut s.on, "Wrap lines");
            });
        });
        Frame::menu(ui.style()).show(ui, |ui| {
            ui.set_width(super::layout::MENU_MIN_W);
            ui.vertical(|ui| {
                menu_body(ui, |ui| {
                    let _ = menu_item(ui, icons::FOLDER, "Open");
                    let _ = menu_item(ui, icons::FILE, "Save as");
                    menu_separator(ui);
                    let _ = menu_check_item(ui, None, "Wrap lines", s.on);
                });
            });
        });
    });
}

fn icon_samples(ui: &mut Ui, extra: &[(&str, Icon)]) {
    if !section_header(ui, "gallery.icons", icons::GRID, "Icons") {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let strip = |ui: &mut Ui, set: &[(&str, Icon)]| {
        ui.horizontal_wrapped(|ui| {
            for (name, icon) in set {
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::splat(ICON_SAMPLE + 8.0), Sense::hover());
                icon.paint(ui, rect, ICON_SAMPLE, t.icon);
                response.on_hover_text(*name);
            }
        });
    };
    strip(ui, &icons::ALL);
    if !extra.is_empty() {
        strip(ui, extra);
    }
}
