//! Node help: what a node does, what each pin takes and gives, and which nodes connect to it.

use super::*;
use ringdesign_graph::registry::Category;

/// A small dot in a value kind's colour.
pub fn kind_dot(ui: &mut Ui, kind: ValueKind) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, crate::widgets::kind_color(kind));
}

/// "one number", "a list of entries".
pub fn kind_words(kind: ValueKind, access: Access) -> String {
    match access {
        Access::Item => format!("one {}", kind.label()),
        Access::List => format!("a list of {}", kind.label()),
    }
}

/// Text from the guide: paragraphs, `- ` bullets, `code` in a monospace face.
pub fn guide_text(ui: &mut Ui, text: &str) {
    for para in text.split("\n\n") {
        let lines: Vec<&str> = para.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        if lines.is_empty() {
            continue;
        }
        let mut prose: Vec<&str> = Vec::new();
        let flush = |ui: &mut Ui, prose: &mut Vec<&str>| {
            if !prose.is_empty() {
                rich_line(ui, &prose.join(" "), "");
                prose.clear();
            }
        };
        for l in lines {
            match l.strip_prefix("- ") {
                Some(item) => {
                    flush(ui, &mut prose);
                    rich_line(ui, item, "•  ");
                }
                None => prose.push(l),
            }
        }
        flush(ui, &mut prose);
        ui.add_space(4.0);
    }
}

/// One wrapped line, `code` spans set in monospace.
fn rich_line(ui: &mut Ui, text: &str, lead: &str) {
    let mut job = egui::text::LayoutJob::default();
    let body = egui::TextStyle::Body.resolve(ui.style());
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let ink = ui.visuals().text_color();
    let plain = egui::TextFormat { font_id: body.clone(), color: ink, ..Default::default() };
    let code = egui::TextFormat { font_id: mono, color: crate::style::AQUA, ..Default::default() };
    job.append(lead, 0.0, plain.clone());
    for (i, part) in text.split('`').enumerate() {
        job.append(part, 0.0, if i % 2 == 1 { code.clone() } else { plain.clone() });
    }
    job.wrap.max_width = ui.available_width();
    ui.label(job);
}

/// The kinds of node that can feed `pin`, or be fed by it, by label.
fn partners(reg: &Registry, mode: ringdesign_graph::graph::Mode, pin: &PinSpec, input: bool) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for spec in reg.list(mode) {
        let hit = if input {
            spec.outputs.iter().any(|o| o.kind != ValueKind::Any && fits(o.kind, pin))
        } else {
            spec.inputs.iter().any(|i| i.kind != ValueKind::Any && i.kind != ValueKind::List && i.kind != ValueKind::Json && fits(pin.kind, i))
        };
        if hit {
            out.push((spec.key.clone(), spec.label.clone()));
        }
    }
    out
}

/// Everything there is to say about one node kind; a click on another node's name asks for its help through `goto`.
pub fn node_help(ui: &mut Ui, reg: &Registry, mode: ringdesign_graph::graph::Mode, key: &str, goto: &mut Option<String>) {
    let Some(spec) = reg.get(key) else {
        ui.weak(format!("{key} is not a node this build knows."));
        return;
    };
    ui.horizontal(|ui| {
        ui.add(crate::marks::of(spec).image(ui, 22.0));
        ui.label(RichText::new(&spec.label).heading().strong());
    });
    ui.label(RichText::new(format!("{}  ·  {}", spec.category.label(), spec.key)).small().monospace().color(crate::style::INK_DIM));
    ui.add_space(6.0);
    match ringdesign_graph::guide::node(key) {
        Some(g) => {
            ui.label(RichText::new(g.line()).strong());
            ui.add_space(4.0);
            guide_text(ui, g.body);
        }
        None => {
            ui.label(&spec.doc);
        }
    }
    let pin_rows = |ui: &mut Ui, title: &str, pins: &[PinSpec], input: bool, goto: &mut Option<String>| {
        if pins.is_empty() {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new(title).strong());
        for p in pins {
            ui.horizontal_wrapped(|ui| {
                kind_dot(ui, p.kind);
                ui.label(RichText::new(&p.name).strong());
                ui.label(RichText::new(kind_words(p.kind, p.access)).color(crate::style::INK_DIM));
                if let Some(d) = &p.default {
                    ui.label(RichText::new(format!("default {}", literal_words(d))).small().color(crate::style::INK_DIM));
                }
                if p.optional {
                    ui.label(RichText::new("optional").small().color(crate::style::INK_DIM));
                }
            });
            if !p.doc.is_empty() {
                ui.indent(("pin-doc", &p.name), |ui| {
                    ui.label(RichText::new(&p.doc).small());
                });
            }
            let mates = partners(reg, mode, p, input);
            if !mates.is_empty() && !matches!(p.kind, ValueKind::Any | ValueKind::Number | ValueKind::Int | ValueKind::Bool | ValueKind::Text) {
                ui.indent(("pin-mates", &p.name), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(if input { "Wire from:" } else { "Feeds:" }).small().color(crate::style::INK_DIM));
                        for (k, label) in mates.iter().take(8) {
                            if ui.small_button(label).on_hover_text(k).clicked() {
                                *goto = Some(k.clone());
                            }
                        }
                        if mates.len() > 8 {
                            ui.label(RichText::new(format!("+{} more", mates.len() - 8)).small().color(crate::style::INK_DIM));
                        }
                    });
                });
            }
        }
    };
    let (ins, outs) = (spec.inputs.clone(), spec.outputs.clone());
    pin_rows(ui, "Inputs (what it takes)", &ins, true, goto);
    pin_rows(ui, "Outputs (what it gives)", &outs, false, goto);
    let kinds: BTreeSet<&'static str> = ins.iter().chain(&outs).map(|p| p.kind.label()).collect();
    let glossary: Vec<(&str, ringdesign_graph::guide::Entry)> = kinds.iter().filter_map(|k| ringdesign_graph::guide::kind(k).map(|e| (*k, e))).collect();
    if !glossary.is_empty() {
        ui.add_space(6.0);
        egui::CollapsingHeader::new("The types these pins carry").id_salt(("glossary", key)).show(ui, |ui| {
            for (k, e) in glossary {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(k).strong());
                    ui.label(e.line());
                });
            }
        });
    }
}

fn literal_words(l: &Literal) -> String {
    match l {
        Literal::Number(x) => format!("{x}"),
        Literal::Int(i) => format!("{i}"),
        Literal::Bool(b) => format!("{b}"),
        Literal::Text(t) if t.chars().count() <= 24 => format!("\"{t}\""),
        _ => "set".into(),
    }
}

/// The browse-all list in the help window.
#[derive(Clone, Debug, Default)]
pub struct Browse {
    query: String,
}

/// The list of every node, searchable; a click opens that node's help.
fn browse(ui: &mut Ui, reg: &Registry, mode: ringdesign_graph::graph::Mode, b: &mut Browse, goto: &mut Option<String>) {
    ui.add(egui::TextEdit::singleline(&mut b.query).hint_text("Find a node").desired_width(f32::INFINITY));
    let needle = b.query.trim().to_lowercase();
    egui::ScrollArea::vertical().id_salt("help-browse").max_height(260.0).show(ui, |ui| {
        for cat in Category::ALL {
            let specs: Vec<&NodeSpec> = reg.list(mode).into_iter().filter(|s| s.category == *cat).filter(|s| needle.is_empty() || s.label.to_lowercase().contains(&needle) || s.key.contains(&needle)).collect();
            if specs.is_empty() {
                continue;
            }
            ui.label(RichText::new(cat.label()).strong());
            for s in specs {
                let about = ringdesign_graph::guide::node(&s.key).map(|g| g.line()).unwrap_or_else(|| s.doc.clone());
                if ui.selectable_label(false, &s.label).on_hover_text(about).clicked() {
                    *goto = Some(s.key.clone());
                }
            }
        }
    });
}

impl Editor {
    /// Open the help on a node kind.
    pub fn show_help(&mut self, key: &str) {
        self.help = Some(key.to_string());
    }

    /// The help window, while one is open.
    pub(super) fn help_ui(&mut self, ctx: &egui::Context, reg: &Registry) {
        let Some(key) = self.help.clone() else { return };
        let mut open = true;
        let mut goto = None;
        let mode = self.graph.mode;
        let browse_state = &mut self.help_browse;
        egui::Window::new("Node help")
            .id(egui::Id::unique("ringdesigner-node-help"))
            .open(&mut open)
            .default_width(440.0)
            .default_height(520.0)
            .resizable(true)
            .collapsible(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().id_salt("help-body").max_height(ui.available_height() - 40.0).show(ui, |ui| {
                    node_help(ui, reg, mode, &key, &mut goto);
                    ui.add_space(8.0);
                    egui::CollapsingHeader::new("How a ring graph fits together").id_salt("help-overview").show(ui, |ui| {
                        for name in ["overview", "layers", "groups", "wiring"] {
                            if let Some(e) = ringdesign_graph::guide::concept(name) {
                                ui.label(RichText::new(e.line()).strong());
                                guide_text(ui, e.body);
                            }
                        }
                    });
                    egui::CollapsingHeader::new("Browse every node").id_salt("help-browse-all").show(ui, |ui| {
                        browse(ui, reg, mode, browse_state, &mut goto);
                    });
                });
            });
        if let Some(k) = goto {
            self.help = Some(k);
        } else if !open {
            self.help = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_help_draws_for_every_node() {
        let reg = Registry::builtin();
        let mut goto = None;
        let mut harness = egui_kittest::Harness::new_ui(|ui| {
            for spec in reg.list(ringdesign_graph::graph::Mode::Free) {
                node_help(ui, &reg, ringdesign_graph::graph::Mode::Free, &spec.key, &mut goto);
            }
        });
        harness.set_size(egui::vec2(480.0, 800.0));
        harness.run();
        drop(harness);
        assert!(goto.is_none());
    }
}
