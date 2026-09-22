//! User-authored reports. GitHub owns authentication and the final submission.
use egui::{Context, Id};

const ISSUES: &str = "https://github.com/shadowbrok3r/RingDesigner/issues/new";
fn id() -> Id {
    Id::new("ringdesigner-feedback")
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    Bug,
    Feature,
}

#[derive(Clone, Default)]
struct Draft {
    open: bool,
    kind: Kind,
    title: String,
    details: String,
    steps: String,
    expected: String,
}

pub fn open(ctx: &Context) {
    ctx.data_mut(|data| {
        let draft = data.get_temp_mut_or_default::<Draft>(id());
        draft.open = true;
    });
}
pub fn is_open(ctx: &Context) -> bool {
    ctx.data(|data| data.get_temp::<Draft>(id()).is_some_and(|draft| draft.open))
}

fn encode(text: &str) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(byte as char);
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
    }
    out
}
impl Draft {
    fn body(&self, version: &str, workspace: &str) -> String {
        let mut body = self.details.trim().to_owned();
        if self.kind == Kind::Bug {
            if !self.steps.trim().is_empty() {
                body += &format!("\n\n### Steps to reproduce\n{}", self.steps.trim());
            }
            if !self.expected.trim().is_empty() {
                body += &format!("\n\n### Expected behavior\n{}", self.expected.trim());
            }
        }
        body.push_str(&format!(
            "\n\n### App\nRingDesigner {version}\nPlatform: {}\nWorkspace: {workspace}",
            std::env::consts::OS
        ));
        body
    }
    fn url(&self, version: &str, workspace: &str) -> Result<String, &'static str> {
        if self.title.trim().is_empty() || self.details.trim().is_empty() {
            return Err("Add a title and details.");
        }
        let kind = if self.kind == Kind::Bug {
            "Bug"
        } else {
            "Feature request"
        };
        let url = format!(
            "{ISSUES}?title={}&body={}",
            encode(&format!("[{kind}] {}", self.title.trim())),
            encode(&self.body(version, workspace))
        );
        if url.len() > 7500 {
            return Err(
                "Shorten the report to open it on GitHub, or copy it and paste into a new issue.",
            );
        }
        Ok(url)
    }
}

/// Returns a URL only when the user explicitly chooses a GitHub action.
pub fn show(ctx: &Context, version: &str, workspace: &str) -> Option<String> {
    let mut draft = ctx
        .data(|data| data.get_temp::<Draft>(id()))
        .unwrap_or_default();
    if !draft.open {
        return None;
    }
    let mut url = None;
    let screen = ctx.content_rect();
    let response = egui::Modal::new(id().with("dialog")).show(ctx, |ui| {
        ui.set_width((screen.width() - 36.).clamp(220., 540.));
        ui.horizontal(|ui| {
            ui.strong("Feature request / bug report");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Close").clicked() { draft.open = false; }
            });
        });
        egui::ScrollArea::vertical().max_height((screen.height() - 170.).max(180.)).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut draft.kind, Kind::Bug, "Bug report");
                ui.selectable_value(&mut draft.kind, Kind::Feature, "Feature request");
            });
            let title = ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut draft.title).hint_text("Briefly describe the problem or idea").char_limit(120).desired_width(f32::INFINITY)).labelled_by(title.id);
            let details = ui.label(if draft.kind == Kind::Bug { "What happened?" } else { "What would you like to do, and why?" });
            ui.add(egui::TextEdit::multiline(&mut draft.details).desired_rows(5).desired_width(f32::INFINITY)).labelled_by(details.id);
            if draft.kind == Kind::Bug {
                let steps = ui.label("Steps to reproduce (optional)");
                ui.add(egui::TextEdit::multiline(&mut draft.steps).desired_rows(3).desired_width(f32::INFINITY)).labelled_by(steps.id);
                let expected = ui.label("Expected behavior (optional)");
                ui.add(egui::TextEdit::multiline(&mut draft.expected).desired_rows(2).desired_width(f32::INFINITY)).labelled_by(expected.id);
            }
            ui.weak(format!("Includes app version {version}, {} and {workspace} workspace. No design files or logs are attached.", std::env::consts::OS));
            ui.label("Review the filled-in issue on GitHub, sign in, and submit it there.");
            let ready = draft.url(version, workspace);
            ui.horizontal_wrapped(|ui| {
                if ui.add_enabled(ready.is_ok(), egui::Button::new("Review on GitHub")).clicked() { url = ready.clone().ok(); }
                if ui.add_enabled(!draft.details.trim().is_empty(), egui::Button::new("Copy report")).clicked() { ctx.copy_text(draft.body(version, workspace)); }
                if ready.is_err() && !draft.details.trim().is_empty() && ui.button("Open blank issue").clicked() { url = Some(ISSUES.into()); }
            });
            if let Err(message) = ready { ui.weak(message); }
        });
    });
    if response.should_close() {
        draft.open = false;
    }
    ctx.data_mut(|data| data.insert_temp(id(), draft));
    url
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_encode_user_text_and_require_content() {
        let mut draft = Draft::default();
        assert!(draft.url("0.3.0", "Graph").is_err());
        draft.title = "Width & shape? #1".into();
        draft.details = "First line\nCafé = 1+2".into();
        let url = draft.url("0.3.0", "Graph").unwrap();
        assert!(url.starts_with(ISSUES));
        assert!(url.contains("Width%20%26%20shape%3F%20%231"));
        assert!(url.contains("Caf%C3%A9%20%3D%201%2B2"));
        assert!(url.contains("Workspace%3A%20Graph"));
        draft.details = "x".repeat(8000);
        assert!(draft.url("0.3.0", "Graph").is_err());
    }
}
