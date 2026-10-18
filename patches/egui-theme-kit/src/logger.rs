//! In-app log viewer styled by the theme tokens, fed by a `log` backend with a bounded ring buffer.

use std::collections::{HashMap, VecDeque};
use std::sync::{LazyLock, Mutex, OnceLock};

use log::SetLoggerError;

use eframe::egui::{Align, Color32, FontSelection, RichText, Style, text::LayoutJob};
use regex::{Regex, RegexBuilder};

use crate::controls::{self, BarKind, Icon, IconBox, MenuButton, Metrics, icons};

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimePrecision {
    Seconds,
    Milliseconds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum TimeFormat {
    Utc,
    LocalTime,
    SinceStart,
    Hide,
}

struct LoggerStyle {
    enable_regex: bool,
    enable_ctx_menu: bool,
    enable_log_count: bool,
    enable_copy_button: bool,
    enable_search: bool,
    enable_max_log_output: bool,
    enable_levels_button: bool,
    enable_categories_button: bool,
    enable_time_button: bool,
    time_precision: TimePrecision,
    show_target: bool,
    time_format: TimeFormat,
    include_target: bool,
    include_level: bool,

    warn_color: Color32,
    error_color: Color32,
    highlight_color: Color32,
}

impl Default for LoggerStyle {
    fn default() -> Self {
        Self {
            show_target: true,
            enable_regex: true,
            enable_ctx_menu: true,
            include_target: true,
            include_level: true,
            time_format: TimeFormat::LocalTime,
            time_precision: TimePrecision::Seconds,
            warn_color: Color32::PLACEHOLDER,
            error_color: Color32::PLACEHOLDER,
            highlight_color: Color32::PLACEHOLDER,
            enable_log_count: true,
            enable_copy_button: true,
            enable_search: true,
            enable_max_log_output: true,
            enable_levels_button: true,
            enable_categories_button: true,
            enable_time_button: true,
        }
    }
}

/// The log viewer; [`logger_ui`] returns a default instance.
pub struct LoggerUi {
    loglevels: [bool; log::Level::Trace as usize],
    search_term: String,
    regex: Option<Regex>,
    search_case_sensitive: bool,
    search_use_regex: bool,
    max_log_length: usize,
    style: LoggerStyle,
}

impl Default for LoggerUi {
    fn default() -> Self {
        Self {
            loglevels: [true, true, true, false, false],
            search_term: String::new(),
            search_case_sensitive: false,
            regex: None,
            search_use_regex: false,
            max_log_length: 1000,
            style: LoggerStyle::default(),
        }
    }
}

impl LoggerUi {
    /// Regex search toggle; on by default.
    #[inline] // i think the compiler already does this
    pub fn enable_regex(mut self, enable: bool) -> Self {
        self.style.enable_regex = enable;
        self
    }

    /// Per-record context menu; on by default.
    #[inline]
    pub fn enable_ctx_menu(mut self, enable: bool) -> Self {
        self.style.enable_ctx_menu = enable;
        self
    }

    /// Target line in the context menu; on by default.
    #[inline]
    pub fn show_target(mut self, enable: bool) -> Self {
        self.style.show_target = enable;
        self
    }

    /// Target column in the records; on by default.
    #[inline]
    pub fn include_target(mut self, enable: bool) -> Self {
        self.style.include_target = enable;
        self
    }

    /// Level column in the records; on by default.
    #[inline]
    pub fn include_level(mut self, enable: bool) -> Self {
        self.style.include_level = enable;
        self
    }

    /// Copy button; on by default.
    #[inline]
    pub fn enable_copy_button(mut self, enable: bool) -> Self {
        self.style.enable_copy_button = enable;
        self
    }

    /// Retained and displayed line counts; on by default.
    #[inline]
    pub fn enable_log_count(mut self, enable: bool) -> Self {
        self.style.enable_log_count = enable;
        self
    }

    /// Search field; on by default.
    #[inline]
    pub fn enable_search(mut self, enable: bool) -> Self {
        self.style.enable_search = enable;
        self
    }

    /// Max-lines field; on by default.
    #[inline]
    pub fn enable_max_log_output(mut self, enable: bool) -> Self {
        self.style.enable_max_log_output = enable;
        self
    }

    /// Levels menu; on by default.
    #[inline]
    pub fn enable_levels_button(mut self, enable: bool) -> Self {
        self.style.enable_levels_button = enable;
        self
    }

    /// Categories menu; on by default.
    #[inline]
    pub fn enable_categories_button(mut self, enable: bool) -> Self {
        self.style.enable_categories_button = enable;
        self
    }

    /// Time format menu; on by default.
    #[inline]
    pub fn enable_time_button(mut self, enable: bool) -> Self {
        self.style.enable_time_button = enable;
        self
    }

    /// Set the color for warning messages.
    #[inline]
    pub fn warn_color(mut self, color: Color32) -> Self {
        self.style.warn_color = color;
        self
    }

    /// Set the color for error messages.
    #[inline]
    pub fn error_color(mut self, color: Color32) -> Self {
        self.style.error_color = color;
        self
    }

    /// Set the color for log messages that are neither errors nor warnings.
    #[inline]
    pub fn highlight_color(mut self, color: Color32) -> Self {
        self.style.highlight_color = color;
        self
    }

    /// Shown levels, from `ERROR` first to `TRACE` last.
    #[inline]
    pub fn log_levels(mut self, log_levels: [bool; log::Level::Trace as usize]) -> Self {
        self.loglevels = log_levels;
        self
    }

    /// Shows or hides `category`.
    #[inline]
    pub fn enable_category(self, category: impl ToString, enable: bool) -> Self {
        LOGGER
            .lock()
            .as_mut()
            .expect("could not lock LOGGER")
            .categories
            .insert(category.to_string(), enable);
        self
    }

    /// Set the maximum number of log messages that should be retained.
    #[inline]
    pub fn max_log_length(mut self, max_length: usize) -> Self {
        self.max_log_length = max_length;
        self
    }

    pub(crate) fn log_ui(self) -> &'static Mutex<LoggerUi> {
        static LOGGER_UI: OnceLock<Mutex<LoggerUi>> = OnceLock::new();
        LOGGER_UI.get_or_init(|| self.into())
    }

    /// Draws the viewer with state shared process-wide, seeded from this builder on first use.
    pub fn show(self, ui: &mut eframe::egui::Ui) {
        if let Ok(ref mut logger_ui) = self.log_ui().lock() {
            logger_ui.ui(ui);
        } else {
            controls::empty_state(
                ui,
                icons::WARNING,
                "Log unavailable",
                "Something went wrong loading the log.",
                &[],
            );
        }
    }

    /// Draws the viewer with state owned by this value.
    pub fn show_mut(&mut self, ui: &mut eframe::egui::Ui) {
        self.ui(ui);
    }

    pub(crate) fn ui(&mut self, ui: &mut eframe::egui::Ui) {
        self.style.warn_color = crate::colors::warn(ui);
        self.style.error_color = crate::colors::error(ui);
        self.style.highlight_color = crate::colors::weak_text(ui);
        let Ok(ref mut logger) = LOGGER.lock() else {
            return;
        };

        {
            let dropped_entries = logger.logs.len().saturating_sub(self.max_log_length);
            drop(logger.logs.drain(..dropped_entries));
        }

        let m = Metrics::get(ui.ctx());
        controls::bar(ui, BarKind::Tool, |ui| {
            if controls::icon_text_button(ui, icons::MINUS_CIRCLE, "Clear")
                .on_hover_text("Clear the retained log")
                .clicked()
            {
                logger.logs.clear();
            }
            controls::vline(ui, m.control_h);

            if self.style.enable_levels_button {
                MenuButton::new("Levels")
                    .icon(icons::MENU)
                    .keep_open(true)
                    .show(ui, |ui| {
                        for level in LEVELS {
                            let on = self.loglevels[level as usize - 1];
                            if controls::menu_check_item(ui, level_icon(level), level.as_str(), on)
                                .clicked()
                            {
                                self.loglevels[level as usize - 1] = !on;
                            }
                        }
                    });
            }

            if self.style.enable_categories_button {
                MenuButton::new("Categories")
                    .icon(icons::LIST)
                    .keep_open(true)
                    .show(ui, |ui| {
                        if controls::menu_item(ui, icons::CHECK, "Select all").clicked() {
                            for enabled in logger.categories.values_mut() {
                                *enabled = true;
                            }
                        }
                        if controls::menu_item(ui, icons::SQUARE, "Unselect all").clicked() {
                            for enabled in logger.categories.values_mut() {
                                *enabled = false;
                            }
                        }
                        ui.add_space(m.pad.y);
                        controls::hairline(ui);
                        ui.add_space(m.pad.y);
                        for (category, enabled) in logger.categories.iter_mut() {
                            if controls::menu_check_item(ui, None, category, *enabled).clicked() {
                                *enabled = !*enabled;
                            }
                        }
                    });
            }

            if self.style.enable_time_button {
                MenuButton::new("Time").icon(icons::CLOCK).show(ui, |ui| {
                    for (format, icon, label) in TIME_FORMATS {
                        let on = self.style.time_format == format;
                        if controls::menu_check_item(ui, icon, label, on).clicked() {
                            self.style.time_format = format;
                        }
                    }
                    ui.add_space(m.pad.y);
                    controls::hairline(ui);
                    ui.add_space(m.pad.y);
                    for (precision, label) in TIME_PRECISIONS {
                        let on = self.style.time_precision == precision;
                        if controls::menu_check_item(ui, icons::CLOCK, label, on).clicked() {
                            self.style.time_precision = precision;
                        }
                    }
                });
            }

            if self.style.enable_max_log_output {
                ui.with_layout(eframe::egui::Layout::right_to_left(Align::Center), |ui| {
                    controls::value_field(
                        ui,
                        &mut self.max_log_length,
                        0..=MAX_RETAINED_LOGS,
                        "lines",
                        MAX_LINES_W,
                    )
                    .on_hover_text("Most log lines kept on screen");
                    ui.label("Max log output");
                });
            }
        });

        if self.style.enable_search {
            ui.add_space(m.pad.y);
            ui.horizontal(|ui| {
                let response =
                    controls::search_field(ui, &mut self.search_term, "Search logs", SEARCH_W);

                let mut config_changed = false;

                if controls::icon_button(
                    ui,
                    CASE_ICON,
                    IconBox::Md,
                    self.search_case_sensitive,
                    "Case sensitive",
                )
                .clicked()
                {
                    self.search_case_sensitive = !self.search_case_sensitive;
                    config_changed = true;
                }

                if self.style.enable_regex
                    && controls::icon_button(
                        ui,
                        REGEX_ICON,
                        IconBox::Md,
                        self.search_use_regex,
                        "Use regex",
                    )
                    .clicked()
                {
                    self.search_use_regex = !self.search_use_regex;
                    config_changed = true;
                }

                if self.style.enable_regex
                    && self.search_use_regex
                    && (response.changed() || config_changed)
                {
                    self.regex = RegexBuilder::new(&self.search_term)
                        .case_insensitive(!self.search_case_sensitive)
                        .build()
                        .ok()
                }
            });
        }

        ui.add_space(m.pad.y);

        let mut logs_displayed: usize = 0;

        let time_padding = logger.logs.back().map_or(0, |record| {
            format_time(record.time, &self.style, logger.start_time).len()
        });

        let filtered_logs: Vec<&Record> = logger
            .logs
            .iter()
            .filter(|r| self.loglevels[r.level as usize - 1])
            .filter(|record| !matches!(logger.categories.get(&record.target), Some(false)))
            .collect();

        eframe::egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .max_height(ui.available_height() - m.header_h - ui.spacing().item_spacing.y)
            .show(ui, |ui| {
                filtered_logs.iter().for_each(|record| {
                    let layout_job = format_record(logger, &self.style, record, time_padding);

                    let raw_text = layout_job.text.clone();

                    // Skips records the search or the level filter rejects.
                    if (!self.search_term.is_empty() && !self.match_string(&raw_text))
                        || !self.loglevels[record.level as usize - 1]
                    {
                        return;
                    }

                    let response = ui.label(layout_job);

                    if self.style.enable_ctx_menu {
                        response.clone().context_menu(|ui| {
                            if self.style.show_target {
                                ui.label(&record.target);
                            }
                            response.highlight();
                            let string_format = format!("[{}]: {}", record.level, record.message);

                            ui.vertical(|ui| {
                                ui.monospace(string_format);
                            });

                            if controls::menu_item(ui, icons::COPY, "Copy").clicked() {
                                ui.ctx().copy_text(raw_text);
                            }
                        });
                    }

                    logs_displayed += 1;
                });
            });

        controls::bar(ui, BarKind::Status, |ui| {
            if self.style.enable_log_count {
                controls::icon_label(ui, icons::LIST, &format!("Log size: {}", logger.logs.len()));
                controls::icon_label(ui, icons::SEARCH, &format!("Displayed: {logs_displayed}"));
            }
            if self.style.enable_copy_button {
                ui.with_layout(eframe::egui::Layout::right_to_left(Align::Center), |ui| {
                    if controls::icon_text_button(ui, icons::COPY, "Copy")
                        .on_hover_text("Copy the retained log")
                        .clicked()
                    {
                        let mut out_string = String::new();
                        logger
                            .logs
                            .iter()
                            .take(self.max_log_length)
                            .for_each(|record| {
                                out_string.push_str(
                                    &format_record(logger, &self.style, record, time_padding).text,
                                );
                                out_string.push_str(" \n");
                            });
                        ui.ctx().copy_text(out_string);
                    }
                });
            }
        });
    }

    fn match_string(&self, string: &str) -> bool {
        if self.search_use_regex {
            if let Some(matcher) = &self.regex {
                matcher.is_match(string)
            } else {
                false
            }
        } else if self.search_case_sensitive {
            string.contains(&self.search_term)
        } else {
            string
                .to_lowercase()
                .contains(&self.search_term.to_lowercase())
        }
    }
}

/// A default [`LoggerUi`]; call [`LoggerUi::show`] to draw it.
pub fn logger_ui() -> LoggerUi {
    LoggerUi::default()
}

fn format_time(
    time: chrono::DateTime<chrono::Local>,
    style: &LoggerStyle,
    start_time: chrono::DateTime<chrono::Local>,
) -> String {
    let time = match (style.time_format, style.time_precision) {
        (TimeFormat::Utc, TimePrecision::Seconds) => time
            .to_utc()
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        (TimeFormat::Utc, TimePrecision::Milliseconds) => time
            .to_utc()
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        (TimeFormat::LocalTime, TimePrecision::Seconds) => time.format("%T").to_string(),
        (TimeFormat::LocalTime, TimePrecision::Milliseconds) => time.format("%T%.3f").to_string(),
        (TimeFormat::SinceStart, TimePrecision::Seconds) => {
            let duration = time - start_time;
            let h = duration.num_hours() % 24;
            let m = duration.num_minutes() % 60;
            let s = duration.num_seconds() % 60;
            match (h, m, s) {
                (0, 0, s) => format!("{s}s"),
                (0, m, s) => format!("{m}m {s}s"),
                (h, m, s) => format!("{h}h {m}m {s}s"),
            }
        }
        (TimeFormat::SinceStart, TimePrecision::Milliseconds) => {
            let duration = time - start_time;
            let h = duration.num_hours() % 24;
            let m = duration.num_minutes() % 60;
            let s = duration.num_seconds() % 60;
            let ms = duration.num_milliseconds() % 1000;
            match (h, m, s, ms) {
                (0, 0, 0, ms) => format!("{ms}ms"),
                (0, 0, s, ms) => format!("{s}s {ms}ms"),
                (0, m, s, ms) => format!("{m}m {s}s {ms}ms"),
                (h, m, s, ms) => format!("{h}h {m}m {s}s {ms}ms"),
            }
        }
        (TimeFormat::Hide, _) => String::new(),
    };
    if style.time_format == TimeFormat::Hide {
        time
    } else {
        time + " "
    }
}

fn format_record(
    logger: &Logger,
    logger_style: &LoggerStyle,
    record: &Record,
    time_padding: usize,
) -> LayoutJob {
    let level_str = if logger_style.include_level {
        format!("[{:5}] ", record.level)
    } else {
        String::new()
    };
    let target_str = if logger_style.include_target {
        format!(
            "{: <width$}: ",
            record.target,
            width = logger.max_category_length
        )
    } else {
        String::new()
    };
    let mut layout_job = LayoutJob::default();
    let style = Style::default();

    let mut date_str = RichText::new(format!(
        "{: >width$}",
        format_time(record.time, logger_style, logger.start_time),
        width = time_padding
    ))
    .monospace();
    match record.level {
        log::Level::Warn => date_str = date_str.color(logger_style.warn_color),
        log::Level::Error => date_str = date_str.color(logger_style.error_color),
        _ => {}
    }

    date_str.append_to(&mut layout_job, &style, FontSelection::Default, Align::LEFT);

    let highlight_color = match record.level {
        log::Level::Warn => logger_style.warn_color,
        log::Level::Error => logger_style.error_color,
        _ => logger_style.highlight_color,
    };

    RichText::new(level_str + &target_str)
        .monospace()
        .color(highlight_color)
        .append_to(&mut layout_job, &style, FontSelection::Default, Align::LEFT);

    let mut message = RichText::new(&record.message).monospace();
    match record.level {
        log::Level::Warn => message = message.color(logger_style.warn_color),
        log::Level::Error => message = message.color(logger_style.error_color),
        _ => {}
    }

    message.append_to(&mut layout_job, &style, FontSelection::Default, Align::LEFT);

    layout_job
}
/// Width of the search field.
const SEARCH_W: f32 = 240.0;
/// Width of the max-lines field.
const MAX_LINES_W: f32 = 96.0;

/// Icon of the case-sensitivity toggle.
const CASE_ICON: Icon = Icon::glyph("Aa");
/// Icon of the regex toggle.
const REGEX_ICON: Icon = Icon::glyph(".*");

/// Time formats offered in the Time menu.
const TIME_FORMATS: [(TimeFormat, Icon, &str); 4] = [
    (TimeFormat::Utc, icons::CIRCLE, "UTC"),
    (TimeFormat::LocalTime, icons::CLOCK, "Local Time"),
    (TimeFormat::SinceStart, icons::PLAY, "Since Start"),
    (TimeFormat::Hide, icons::MINUS_CIRCLE, "Hide"),
];

/// Time precisions offered in the Time menu.
const TIME_PRECISIONS: [(TimePrecision, &str); 2] = [
    (TimePrecision::Seconds, "Seconds"),
    (TimePrecision::Milliseconds, "Milliseconds"),
];

/// Icon for a log level.
fn level_icon(level: log::Level) -> Icon {
    match level {
        log::Level::Error => icons::ERROR,
        log::Level::Warn => icons::WARNING,
        log::Level::Info => icons::INFO,
        log::Level::Debug => icons::CIRCLE,
        log::Level::Trace => icons::DOT,
    }
}

const LEVELS: [log::Level; log::Level::Trace as usize] = [
    log::Level::Error,
    log::Level::Warn,
    log::Level::Info,
    log::Level::Debug,
    log::Level::Trace,
];

/// The `log` backend that feeds the viewer; see [`builder`].
pub struct EguiLogger {
    /// The maximum log level that shall be collected.
    max_level: log::LevelFilter,
    /// Whether to show all categories by default (versus only those that are explicitly enabled).
    show_all_categories: bool,

    blacklisted: Vec<String>,
}

impl EguiLogger {
    fn new(
        max_level: log::LevelFilter,
        show_all_categories: bool,
        blacklisted: Vec<String>,
    ) -> Self {
        Self {
            max_level,
            show_all_categories,
            blacklisted,
        }
    }
}

/// Builds an [`EguiLogger`]; see [`builder`].
pub struct Builder {
    max_level: log::LevelFilter,
    show_all_categories: bool,
    /// Targets dropped before they reach the buffer.
    blacklisted: Vec<String>,
}

impl Default for Builder {
    fn default() -> Self {
        Self {
            max_level: log::LevelFilter::Debug,
            show_all_categories: true,
            blacklisted: vec![
                "tracing::span".to_string(),
                "tracing::span::active".to_string(),
            ],
        }
    }
}

impl Builder {
    /// Returns the logger, for adding to a multi-logger.
    pub fn build(self) -> EguiLogger {
        EguiLogger::new(self.max_level, self.show_all_categories, self.blacklisted)
    }

    /// Highest level collected by [`Self::init`]; `Debug` by default.
    pub fn max_level(mut self, max_level: log::LevelFilter) -> Self {
        self.max_level = max_level;
        self
    }

    /// Whether new categories start shown; true by default.
    pub fn show_all_categories(mut self, show_all_categories: bool) -> Self {
        self.show_all_categories = show_all_categories;
        self
    }

    /// Keeps the built-in blacklist when true, clears it when false.
    pub fn default_blacklist(mut self, default_blacklist: bool) -> Self {
        if default_blacklist {
            self
        } else {
            self.blacklisted = vec![];
            self
        }
    }

    /// This adds a `log` target to the blacklist.
    pub fn add_blacklist(mut self, target: impl ToString) -> Self {
        self.blacklisted.push(target.to_string());
        self
    }

    /// Installs the global logger and sets the max level.
    pub fn init(self) -> Result<(), SetLoggerError> {
        log::set_max_level(self.max_level);
        log::set_logger(Box::leak(Box::new(self.build())))
    }
}

impl log::Log for EguiLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        if metadata.level() > self.max_level {
            return false;
        }
        // A blacklist entry matches the exact target or any `target::sub` module.
        let target = metadata.target();
        !self.blacklisted.iter().any(|b| {
            target == b.as_str()
                || target
                    .strip_prefix(b.as_str())
                    .is_some_and(|rest| rest.starts_with("::"))
        })
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata())
            && let Ok(ref mut logger) = LOGGER.lock()
        {
            logger.push(Record {
                level: record.level(),
                message: record.args().to_string(),
                target: record.target().to_string(),
                time: chrono::Local::now(),
            });

            if !logger.categories.contains_key(record.target()) {
                logger
                    .categories
                    .insert(record.target().to_string(), self.show_all_categories);
                logger.max_category_length = logger.max_category_length.max(record.target().len());
            }
        }
    }

    fn flush(&self) {}
}

struct Record {
    level: log::Level,
    message: String,
    target: String,
    time: chrono::DateTime<chrono::Local>,
}

/// Records the ring buffer keeps before the oldest are dropped.
const MAX_RETAINED_LOGS: usize = 20_000;

struct Logger {
    logs: VecDeque<Record>,
    categories: HashMap<String, bool>,
    max_category_length: usize,
    start_time: chrono::DateTime<chrono::Local>,
}

impl Logger {
    /// Appends `record`, dropping the oldest once the buffer is full.
    fn push(&mut self, record: Record) {
        if self.logs.len() >= MAX_RETAINED_LOGS {
            self.logs.pop_front();
        }
        self.logs.push_back(record);
    }
}

static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| {
    Mutex::new(Logger {
        logs: VecDeque::new(),
        categories: HashMap::new(),
        max_category_length: 0,
        start_time: chrono::Local::now(),
    })
});

/// Clears all existing retained logs.
pub fn clear_logs() {
    if let Ok(mut logger) = LOGGER.lock() {
        logger.logs.clear();
    }
}

/// Retained records as text, the newest `max_logs` when given.
pub fn get_logs_as_string(
    max_logs: Option<usize>,
    include_level: bool,
    include_target: bool,
) -> String {
    let Ok(logger) = LOGGER.lock() else {
        return String::from("Could not access logger");
    };

    let mut output = String::new();
    let skip = max_logs.map_or(0, |max| logger.logs.len().saturating_sub(max));
    for record in logger.logs.iter().skip(skip) {
        let time_str = record.time.format("%Y-%m-%d %H:%M:%S%.3f").to_string();

        let level_str = if include_level {
            format!("[{:5}] ", record.level)
        } else {
            String::new()
        };

        let target_str = if include_target {
            format!("{}: ", record.target)
        } else {
            String::new()
        };

        output.push_str(&format!(
            "[{}] {}{}{}\n",
            time_str, level_str, target_str, record.message
        ));
    }

    output
}

/// The newest `max_logs` retained records as text, with level and target.
pub fn recent_logs(max_logs: usize) -> String {
    let logs = get_logs_as_string(Some(max_logs), true, true);
    if logs.is_empty() {
        return String::from("No logs available");
    }
    logs
}

/// A [`Builder`] with default values.
///
/// ```no_run
/// use log::LevelFilter;
///
/// egui_theme_kit::logger::builder()
///     .max_level(LevelFilter::Info)
///     .init()
///     .unwrap();
/// ```
pub fn builder() -> Builder {
    Builder::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts;
    use crate::metrics::{self, Density};
    use crate::presets::{MATCHABLE_PRESETS, apply_preset, bootstrap_startup_theme, set_density};
    use eframe::egui::{CentralPanel, Context, Pos2, RawInput, Rect, vec2};
    use log::Log;

    fn record(level: log::Level, target: &str, message: &str) -> Record {
        Record {
            level,
            message: message.to_owned(),
            target: target.to_owned(),
            time: chrono::Local::now(),
        }
    }

    fn empty_logger() -> Logger {
        Logger {
            logs: VecDeque::new(),
            categories: HashMap::new(),
            max_category_length: 0,
            start_time: chrono::Local::now(),
        }
    }

    #[test]
    fn the_ring_buffer_keeps_the_newest_records() {
        let mut logger = empty_logger();
        for i in 0..MAX_RETAINED_LOGS + 5 {
            logger.push(record(log::Level::Info, "t", &i.to_string()));
        }
        assert_eq!(logger.logs.len(), MAX_RETAINED_LOGS);
        assert_eq!(logger.logs.front().map(|r| r.message.as_str()), Some("5"));
        let newest = (MAX_RETAINED_LOGS + 4).to_string();
        assert_eq!(logger.logs.back().map(|r| r.message.clone()), Some(newest));
    }

    #[test]
    fn a_blacklisted_target_and_its_submodules_are_dropped() {
        let logger = Builder::default().add_blacklist("noisy").build();
        let enabled = |target: &str, level| {
            logger.enabled(&log::Metadata::builder().target(target).level(level).build())
        };
        assert!(!enabled("noisy", log::Level::Info));
        assert!(!enabled("noisy::inner", log::Level::Info));
        assert!(enabled("noisy_other", log::Level::Info));
        assert!(!enabled("tracing::span", log::Level::Info));
        assert!(!enabled("app", log::Level::Trace), "above the max level");
        assert!(enabled("app", log::Level::Debug));
    }

    #[test]
    fn time_formats_render_each_mode() {
        use chrono::TimeZone;
        let start = chrono::Local.with_ymd_and_hms(2024, 5, 6, 7, 8, 9).unwrap();
        let later = start + chrono::Duration::milliseconds(3_723_045);
        let mut style = LoggerStyle {
            time_format: TimeFormat::SinceStart,
            time_precision: TimePrecision::Seconds,
            ..LoggerStyle::default()
        };
        assert_eq!(format_time(later, &style, start), "1h 2m 3s ");
        style.time_precision = TimePrecision::Milliseconds;
        assert_eq!(format_time(later, &style, start), "1h 2m 3s 45ms ");
        style.time_format = TimeFormat::Hide;
        assert_eq!(format_time(later, &style, start), "");
        style.time_format = TimeFormat::Utc;
        style.time_precision = TimePrecision::Seconds;
        assert!(format_time(later, &style, start).ends_with("Z "));
    }

    #[test]
    fn a_record_formats_with_level_and_padded_target() {
        let mut logger = empty_logger();
        logger.max_category_length = 8;
        let style = LoggerStyle {
            time_format: TimeFormat::Hide,
            ..LoggerStyle::default()
        };
        let job = format_record(
            &logger,
            &style,
            &record(log::Level::Warn, "app", "hello"),
            0,
        );
        assert_eq!(job.text, "[WARN ] app     : hello");
    }

    fn context(density: Density) -> Context {
        let ctx = Context::default();
        let mut definitions = eframe::egui::FontDefinitions::default();
        fonts::install(&mut definitions);
        ctx.set_fonts(definitions);
        metrics::set_density(&ctx, density);
        metrics::install_overlaid_style(&ctx, eframe::egui::Style::default());
        ctx
    }

    fn draw(ctx: &Context, viewer: &mut LoggerUi) {
        for _ in 0..2 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                CentralPanel::default().show(ui, |ui| viewer.ui(ui));
            });
            out.textures_delta.clear();
        }
    }

    #[test]
    fn the_viewer_draws_records_at_every_density() {
        if let Ok(mut logger) = LOGGER.lock() {
            for level in [log::Level::Error, log::Level::Warn, log::Level::Info] {
                logger.push(record(level, "viewer_test", "a line to draw"));
            }
        }
        for density in Density::ALL {
            let ctx = context(density);
            draw(&ctx, &mut LoggerUi::default());
        }
    }

    #[test]
    fn an_owned_viewer_keeps_its_own_search_state() {
        let ctx = context(Density::Regular);
        let mut viewer = LoggerUi {
            search_term: "kept".to_owned(),
            ..LoggerUi::default()
        };
        for _ in 0..2 {
            let input = RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 600.0))),
                ..Default::default()
            };
            let mut out = ctx.run_ui(input, |ui| {
                CentralPanel::default().show(ui, |ui| viewer.show_mut(ui));
            });
            out.textures_delta.clear();
        }
        assert_eq!(viewer.search_term, "kept");
    }

    #[test]
    fn the_viewer_draws_under_every_preset() {
        let ctx = Context::default();
        ctx.set_fonts(fonts::definitions(fonts::TextScale::Normal));
        bootstrap_startup_theme(&ctx);
        for preset in MATCHABLE_PRESETS {
            apply_preset(&ctx, preset);
            for density in Density::ALL {
                set_density(&ctx, density);
                draw(&ctx, &mut LoggerUi::default());
            }
        }
    }

    #[test]
    fn the_search_term_filters_by_case_and_regex() {
        let mut viewer = LoggerUi {
            search_term: "Alpha".to_owned(),
            ..LoggerUi::default()
        };
        assert!(viewer.match_string("an alpha line"));
        viewer.search_case_sensitive = true;
        assert!(!viewer.match_string("an alpha line"));
        viewer.search_case_sensitive = false;
        viewer.search_use_regex = true;
        viewer.regex = RegexBuilder::new("^a.pha$")
            .case_insensitive(true)
            .build()
            .ok();
        assert!(viewer.match_string("ALPHA"));
        assert!(!viewer.match_string("beta"));
    }
}
