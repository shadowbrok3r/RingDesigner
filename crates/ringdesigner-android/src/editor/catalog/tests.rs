use super::*;

struct Browser {
    ctx: egui::Context,
    safe: egui::Rect,
    report: serde_json::Value,
}

impl Browser {
    fn new(width: f32, text_scale: f32) -> Self {
        let ctx = egui::Context::default();
        crate::theme::apply_config(
            &ctx,
            &egui_mobile_theme::ThemeConfig {
                text_scale,
                ..Default::default()
            },
        );
        Self {
            ctx,
            safe: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 700.0)),
            report: serde_json::Value::Null,
        }
    }

    fn frame(&mut self, events: Vec<egui::Event>) -> Option<Choice> {
        let mut choice = None;
        let mut output = self.ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(self.safe),
                events,
                ..Default::default()
            },
            |root| {
                crate::editor::layout::begin(root.ctx());
                egui::CentralPanel::default().show(root, |ui| choice = show(ui));
            },
        );
        output.textures_delta.clear();
        self.report = crate::editor::layout::report(&self.ctx, self.safe);
        assert_eq!(
            self.report["overflow"],
            serde_json::json!([]),
            "{}",
            self.report
        );
        choice
    }

    fn has(&self, label: &str) -> bool {
        self.report["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|c| c["label"] == label)
    }

    fn tap(&mut self, label: &str) -> Option<Choice> {
        let r = &self.report["controls"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["label"] == label)
            .unwrap()["rect"];
        let pos = egui::pos2(
            ((r[0].as_f64().unwrap() + r[2].as_f64().unwrap()) / 2.0) as f32,
            ((r[1].as_f64().unwrap() + r[3].as_f64().unwrap()) / 2.0) as f32,
        );
        let event = |pressed| {
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ]
        };
        self.frame(event(true));
        self.frame(event(false))
    }
}

#[test]
fn search_filters_without_selecting_and_clear_restores_the_library() {
    let mut browser = Browser::new(320.0, 1.0);
    for _ in 0..3 {
        assert!(browser.frame(vec![]).is_none());
    }
    browser.tap("templates/search");
    assert!(
        browser
            .frame(vec![egui::Event::Text("COURT".into())])
            .is_none()
    );
    assert!(browser.has("templates/Court band"));
    assert!(!browser.has("templates/Braided band"));
    assert!(browser.tap("templates/Clear").is_none());
    assert!(browser.has("templates/Braided band"));
    browser.tap("templates/search");
    browser.frame(vec![egui::Event::Text("nothing-matches-this".into())]);
    assert!(!browser.has("templates/Starter bands"));
    assert!(browser.has("templates/Blank band"));
    browser.tap("templates/Clear");
    assert!(
        matches!(browser.tap("templates/Court band"), Some(Choice::Template(t)) if t.name == "Court band")
    );
}

#[test]
fn library_and_collection_search_fit_narrow_screens_with_large_text() {
    for (width, text_scale) in [(280.0, 1.0), (320.0, 1.5), (411.0, 1.0), (720.0, 1.0)] {
        let mut browser = Browser::new(width, text_scale);
        for _ in 0..3 {
            browser.frame(vec![]);
        }
        browser.tap("templates/search");
        browser.frame(vec![egui::Event::Text("starter bands".into())]);
        assert!(browser.has("templates/Court band"));
        assert!(browser.has("templates/Braided band"));
        assert!(!browser.has("templates/Starter signets"));
    }
}
