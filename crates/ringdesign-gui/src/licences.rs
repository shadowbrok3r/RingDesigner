//! The licences RingDesigner is under and the notices of the software it is built on, carried in the executable and shown in a window.
use crate::theme;
use ringdesign_occt::client::Locator;

/// RingDesigner's MIT licence.
pub const MIT: &str = include_str!("../../../LICENSE-MIT");
/// RingDesigner's Apache licence.
pub const APACHE: &str = include_str!("../../../LICENSE-APACHE");
/// OpenCascade's, cadrum's and MinGW-w64's notices and licences.
pub const NOTICES: &str = include_str!("../../../THIRD-PARTY-NOTICES.md");

/// Where RingDesigner's source is, the OpenCascade worker's included.
pub const REPOSITORY: &str = "https://github.com/shadowbrok3r/RingDesigner";
/// The commit this build was made from, as its build script found it.
pub const COMMIT: &str = env!("RINGDESIGNER_COMMIT");

/// Code blocks up to this many lines show open; longer ones fold under their heading.
const OPEN_LINES: usize = 6;

/// Whether the window is open, kept per context.
fn open_id() -> egui::Id {
    egui::Id::new("licences-open")
}

/// Opens the window in `ctx` on its next frame.
pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(open_id(), true));
}

/// Where this build's source is: the repository and the commit it was made from.
pub fn source_line() -> String {
    format!("Built from {REPOSITORY} at commit {COMMIT}.")
}

/// A piece of the notices file as the window draws it.
#[derive(Debug, PartialEq)]
pub enum Block<'a> {
    /// A heading and its level, one to three.
    Heading(usize, &'a str),
    /// Lines of prose joined into one paragraph, Markdown marks taken out.
    Text(String),
    /// A fenced block verbatim, under the heading before it.
    Code { under: &'a str, text: String },
}

/// `markdown` as headings, paragraphs and fenced blocks: the subset the notices file is written in.
pub fn blocks(markdown: &str) -> Vec<Block<'_>> {
    fn flush<'a>(paragraph: &mut Vec<&'a str>, out: &mut Vec<Block<'a>>) {
        if !paragraph.is_empty() {
            out.push(Block::Text(plain(&paragraph.join(" "))));
            paragraph.clear();
        }
    }
    let mut out = Vec::new();
    let (mut heading, mut paragraph, mut code): (&str, Vec<&str>, Option<Vec<&str>>) = ("", Vec::new(), None);
    for line in markdown.lines() {
        if let Some(lines) = code.as_mut() {
            if line.starts_with("```") {
                out.push(Block::Code { under: heading, text: lines.join("\n") });
                code = None;
            } else {
                lines.push(line);
            }
        } else if line.starts_with("```") {
            flush(&mut paragraph, &mut out);
            code = Some(Vec::new());
        } else if let Some(rest) = line.strip_prefix('#') {
            flush(&mut paragraph, &mut out);
            let level = 1 + rest.chars().take_while(|c| *c == '#').count();
            heading = rest.trim_start_matches('#').trim();
            out.push(Block::Heading(level, heading));
        } else if line.trim().is_empty() {
            flush(&mut paragraph, &mut out);
        } else if line.trim_start().starts_with("- ") {
            flush(&mut paragraph, &mut out);
            paragraph.push(line.trim());
        } else {
            paragraph.push(line.trim());
        }
    }
    flush(&mut paragraph, &mut out);
    out
}

/// `text` with emphasis, code ticks and the brackets round a link taken out.
fn plain(text: &str) -> String {
    let text = text.replace("**", "").replace('`', "");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("<http") {
        out.push_str(&rest[..at]);
        let link = &rest[at + 1..];
        let end = link.find('>').unwrap_or(link.len());
        out.push_str(&link[..end]);
        rest = link.get(end + 1..).unwrap_or("");
    }
    out.push_str(rest);
    out
}

/// What this build carries of OpenCascade, in a line.
fn opencascade_line(locator: &Locator) -> String {
    let carried = crate::occt_embedded::EMBEDDED;
    match (carried.is_present(), crate::occt_embedded::missing(locator)) {
        (true, _) => format!("This build carries the OpenCascade worker ({:.1} MB, SHA-256 {}…).", carried.bytes as f64 / 1e6, &carried.sha256[..12]),
        (false, None) => "This build carries no OpenCascade worker; it runs the one beside it or the one RINGDESIGN_OCCT_WORKER names.".into(),
        (false, Some(_)) => "This build carries no OpenCascade worker, and none is beside it.".into(),
    }
}

fn licence_text(ui: &mut egui::Ui, title: &str, text: &str) {
    egui::CollapsingHeader::new(title).id_salt(("licence", title)).show(ui, |ui| {
        ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0)).wrap_mode(egui::TextWrapMode::Extend));
    });
}

/// The Licences window, while it is open; `locator` is where the app looks for OpenCascade.
pub fn window(ctx: &egui::Context, locator: &Locator) {
    let id = open_id();
    let mut open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    if !open {
        return;
    }
    // Over the viewports' own foreground plates, such as the navigator cube.
    egui::Window::new("Licences")
        .order(egui::Order::Foreground)
        .frame(egui::Frame::window(&ctx.global_style()).fill(theme::FLOAT))
        .default_size([620.0, 560.0])
        .default_pos(egui::pos2(360.0, 90.0))
        .resizable(true)
        .collapsible(false)
        .open(&mut open)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.strong(format!("RingDesigner v{}", env!("CARGO_PKG_VERSION")));
                ui.label("Licensed under the MIT licence or the Apache License 2.0, at your option.");
                ui.label(source_line());
                licence_text(ui, "MIT licence", MIT);
                licence_text(ui, "Apache License 2.0", APACHE);
                ui.weak(opencascade_line(locator));
                ui.separator();
                for (k, block) in blocks(NOTICES).into_iter().enumerate() {
                    match block {
                        Block::Heading(1, _) => {}
                        Block::Heading(level, text) => {
                            ui.add_space(if level == 2 { 8.0 } else { 4.0 });
                            let text = egui::RichText::new(text).strong();
                            ui.label(if level == 2 { text.size(16.0).color(theme::ACCENT) } else { text });
                        }
                        Block::Text(text) => {
                            ui.label(text);
                        }
                        Block::Code { text, .. } if text.lines().count() <= OPEN_LINES => {
                            ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0)).wrap_mode(egui::TextWrapMode::Wrap));
                        }
                        Block::Code { under, text } => {
                            egui::CollapsingHeader::new(format!("{under}: full text")).id_salt(("notice", k)).show(ui, |ui| {
                                ui.add(egui::Label::new(egui::RichText::new(text).monospace().size(11.0)).wrap_mode(egui::TextWrapMode::Extend));
                            });
                        }
                    }
                }
            });
        });
    ctx.data_mut(|d| d.insert_temp(id, open));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction_tests::harness;
    use egui_kittest::kittest::Queryable;

    #[test]
    fn the_notices_carry_opencascades_licence_its_exception_and_where_its_source_is() {
        // A Windows checkout's CRLF read as LF.
        let notices = NOTICES.replace("\r\n", "\n");
        let blocks = blocks(&notices);
        let headings: Vec<&str> = blocks.iter().filter_map(|b| if let Block::Heading(2, h) = b { Some(*h) } else { None }).collect();
        assert_eq!(headings, ["Open CASCADE Technology 8.0.1", "cadrum 0.8.20", "MinGW-w64 runtime (the Windows worker)", "Everything else"]);
        let code = |under: &str| blocks.iter().find_map(|b| if let Block::Code { under: u, text } = b { (*u == under).then_some(text.as_str()) } else { None }).unwrap();
        assert!(code("GNU Lesser General Public License, version 2.1").starts_with("                  GNU LESSER GENERAL PUBLIC LICENSE"));
        assert!(code("Open CASCADE exception, version 1.0").starts_with("Open CASCADE exception (version 1.0) to GNU LGPL version 2.1."));
        assert!(code("cadrum 0.8.20").contains("Copyright (c) 2026 cadrum Contributors"));
        assert!(notices.contains("https://github.com/Open-Cascade-SAS/OCCT/archive/refs/tags/V8_0_1.tar.gz"));
        assert!(notices.contains("makes use of, and is in part based on,\nfacilities provided by the Open CASCADE Technology software"));
        assert!(MIT.contains("Copyright (c) 2026 Logan and the RingDesigner authors") && APACHE.contains("Version 2.0, January 2004"));
        // RingDesigner's own source, the worker's included, and which releases carry the worker.
        assert!(notices.contains(&format!("<{REPOSITORY}>")), "the repository");
        let carried = blocks.iter().find_map(|b| if let Block::Text(t) = b { t.contains("package.sh --occt").then_some(t.as_str()) } else { None }).unwrap();
        assert!(carried.contains("The releases published on GitHub, which the in-app updater installs, do not"), "{carried}");
        let prose = blocks.iter().find_map(|b| if let Block::Text(t) = b { t.contains("RingDesigner's desktop app makes use of").then_some(t.as_str()) } else { None }).unwrap();
        assert!(!prose.contains("**"), "{prose}");
        assert!(blocks.iter().any(|b| matches!(b, Block::Text(t) if t.contains("under Tools > Licences."))), "the menu path keeps its >");
        assert_eq!(plain("see <https://example.org/a> and `x` > **y**"), "see https://example.org/a and x > y");
    }

    #[test]
    fn opening_the_window_in_one_context_leaves_another_alone() {
        let (a, b) = (egui::Context::default(), egui::Context::default());
        open(&a);
        let shown = |ctx: &egui::Context| ctx.data(|d| d.get_temp::<bool>(open_id())).unwrap_or(false);
        assert!(shown(&a) && !shown(&b));
    }

    #[test]
    fn the_palette_opens_the_licences_window_over_the_navigator() {
        let mut h = harness();
        h.state_mut().palette_open = true;
        h.state_mut().palette_query = "licences".into();
        h.run_steps(3);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
        assert!(h.query_by_label("Licensed under the MIT licence or the Apache License 2.0, at your option.").is_some());
        // The commit the build was made from, as git names one, or said to be unknown.
        assert!(h.query_by_label(&source_line()).is_some());
        let sha = COMMIT.trim_end_matches("-dirty");
        assert!(COMMIT == "unknown" || (sha.len() == 40 && sha.bytes().all(|b| b.is_ascii_hexdigit())), "{COMMIT}");
        assert!(h.query_by_label("Open CASCADE Technology 8.0.1").is_some());
        assert!(h.query_by_label("GNU Lesser General Public License, version 2.1: full text").is_some());
        assert!(h.query_by_label_contains("Copyright (c) 2026 Logan").is_none(), "folded until asked for");
        // The window's header lies over the Ring viewport's navigator cube in this small window.
        h.get_by_label("MIT licence").click();
        h.run_steps(3);
        assert!(h.query_by_label_contains("Copyright (c) 2026 Logan and the RingDesigner authors").is_some());
    }
}
