//! The dimension bar: one `TextEdit` per live dimension by the pointer, owning Tab and Escape inside the tool.
use super::session::Dimension;
use egui::text::{CCursor, CCursorRange};
use egui::{Area, Context, Event, EventFilter, FocusDirection, Frame, Id, Key, Order, Pos2, TextEdit, WidgetInfo};

/// What the fields did this frame, for the session to feed.
#[derive(Clone, Debug, PartialEq)]
pub enum DimEvent {
    Typed { key: &'static str, value: f64 },
    Cleared { key: &'static str },
    Confirm,
    Escape,
    Focused { key: &'static str },
}

/// The fields' text, keyed by dimension; the session stays the owner of the values.
pub struct DimensionBar {
    id: Id,
    texts: Vec<(&'static str, String)>,
    /// The widget holding the keys for the tool while no field does; focus there counts as none.
    host: Option<Id>,
}
impl Default for DimensionBar {
    fn default() -> Self {
        Self::new("dimension-bar")
    }
}

/// A number with an optional unit suffix (mm, °, deg, ×, x); a decimal comma reads as a point.
pub fn parse_value(text: &str) -> Option<f64> {
    let t = text.trim().trim_end_matches(|c: char| c.is_alphabetic() || c == '°' || c == '×' || c.is_whitespace());
    let v: f64 = t.replace(',', ".").parse().ok()?;
    v.is_finite().then_some(v)
}
/// The value as field text, without trailing zeros.
fn plain(v: f64) -> String {
    let s = format!("{v:.6}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if matches!(s, "" | "-" | "-0") { "0".into() } else { s.to_string() }
}
fn pressed(e: &Event, key: Key) -> bool {
    matches!(e, Event::Key { key: k, pressed: true, .. } if *k == key)
}
fn plain_enter(e: &Event) -> bool {
    matches!(e, Event::Key { key: Key::Enter, pressed: true, modifiers, .. } if !(modifiers.ctrl || modifiers.command || modifiers.alt))
}
fn starts_a_number(e: &Event) -> bool {
    matches!(e, Event::Text(t) if t.starts_with(|c: char| c.is_ascii_digit() || c == '-' || c == '.'))
}
fn from_keyboard(e: &Event) -> bool {
    matches!(e, Event::Key { .. } | Event::Text(_) | Event::Paste(_))
}
/// Puts the field's cursor after its last character, as typing into it expects.
fn cursor_to_end(ctx: &Context, id: Id, text: &str) {
    let mut state = TextEdit::load_state(ctx, id).unwrap_or_default();
    state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(text.chars().count()))));
    TextEdit::store_state(ctx, id, state);
}

impl DimensionBar {
    pub fn new(id_salt: impl egui::AsId) -> Self {
        Self { id: Id::new(id_salt), texts: Vec::new(), host: None }
    }
    /// Names the widget that holds the keys for the tool, so a number typed while it has focus still starts the first field.
    pub fn set_host(&mut self, host: Option<Id>) {
        self.host = host;
    }
    /// Gives a field the keyboard, its cursor after its text.
    pub fn focus_field(&self, ctx: &Context, key: &str) {
        let id = self.field_id(key);
        ctx.memory_mut(|m| m.request_focus(id));
        cursor_to_end(ctx, id, self.text(key));
    }
    pub fn field_id(&self, key: &str) -> Id {
        self.id.with(key)
    }
    pub fn text(&self, key: &str) -> &str {
        self.texts.iter().find(|(k, _)| *k == key).map_or("", |(_, t)| t.as_str())
    }
    fn text_mut(&mut self, key: &'static str) -> &mut String {
        let i = match self.texts.iter().position(|(k, _)| *k == key) {
            Some(i) => i,
            None => {
                self.texts.push((key, String::new()));
                self.texts.len() - 1
            }
        };
        &mut self.texts[i].1
    }
    pub fn reset(&mut self) {
        self.texts.clear();
    }
    /// Whether one of the fields holds keyboard focus, so the viewport leaves the keys alone.
    pub fn has_focus(&self, ctx: &Context) -> bool {
        ctx.memory(|m| m.focused()).is_some_and(|f| self.texts.iter().any(|(k, _)| self.field_id(k) == f))
    }

    /// Draws one field per dimension, mirrors their text into `dims`, and returns what they did this frame.
    pub fn show(&mut self, ctx: &Context, anchor: Pos2, dims: &mut [Dimension]) -> Vec<DimEvent> {
        self.texts.retain(|(k, _)| dims.iter().any(|d| d.key == *k));
        let n = dims.len();
        if n == 0 {
            return Vec::new();
        }
        let ids: Vec<Id> = dims.iter().map(|d| self.field_id(d.key)).collect();
        let focused = ctx.memory(|m| m.focused()).and_then(|f| ids.iter().position(|id| *id == f));
        let had = ids.iter().position(|id| ctx.memory(|m| m.had_focus_last_frame(*id)));
        // A field not being typed into shows the session's typed value, or nothing.
        for (i, d) in dims.iter().enumerate() {
            if focused == Some(i) {
                continue;
            }
            let t = self.text_mut(d.key);
            if !d.locked {
                t.clear();
            } else if parse_value(t).is_none_or(|v| (v - d.value).abs() > 1e-6) {
                *t = plain(d.value);
            }
        }
        let before: Vec<String> = dims.iter().map(|d| self.text(d.key).to_owned()).collect();
        let input = ctx.input(|i| i.events.clone());
        // Where the bar takes the frame's keys over, the field they start in, and whether they start it afresh.
        let takeover = match (focused, had) {
            (Some(cur), _) => input
                .iter()
                .position(|e| pressed(e, Key::Tab) || pressed(e, Key::Escape))
                .filter(|&at| !input[..at].iter().any(plain_enter))
                .map(|at| (at, cur, false)),
            // egui acted on an Escape before the field's filter landed and dropped its focus.
            (None, Some(cur)) if input.iter().any(|e| pressed(e, Key::Escape)) => Some((0, cur, false)),
            (None, _) if ctx.memory(|m| m.focused()).is_none_or(|f| Some(f) == self.host) => input.iter().position(starts_a_number).map(|at| (at, 0, true)),
            _ => None,
        };
        if focused.is_some() || takeover.is_some() {
            // Undoes any focus move egui planned on a key the field's filter had not yet caught.
            ctx.memory_mut(|m| m.move_focus(FocusDirection::None));
        }
        if let Some((at, ..)) = takeover {
            ctx.input_mut(|i| {
                let mut k = 0;
                i.events.retain(|e| {
                    k += 1;
                    k <= at || !from_keyboard(e)
                });
            });
        }
        let field_enter = focused.is_some() && input[..takeover.map_or(input.len(), |t| t.0)].iter().any(plain_enter);
        self.draw(ctx, anchor, dims, &ids);
        if field_enter {
            ctx.input_mut(|i| i.events.retain(|e| !plain_enter(e)));
        }
        // The taken-over keys, in order, after the focused field has had the ones before them.
        let (mut confirm, mut escape, mut goto) = (field_enter, false, None);
        if let Some((at, start, fresh)) = takeover {
            let (mut cur, mut moved) = (start, fresh);
            if fresh {
                self.text_mut(dims[0].key).clear();
            }
            for e in &input[at..] {
                let text = self.text_mut(dims[cur].key);
                match e {
                    Event::Key { key: Key::Tab, pressed: true, modifiers, .. } => {
                        cur = if modifiers.shift { (cur + n - 1) % n } else { (cur + 1) % n };
                        moved = true;
                    }
                    Event::Key { key: Key::Escape, pressed: true, .. } if text.is_empty() => {
                        escape = true;
                        break;
                    }
                    Event::Key { key: Key::Escape, pressed: true, .. } => {
                        text.clear();
                        moved = true;
                    }
                    e if plain_enter(e) => {
                        confirm = true;
                        break;
                    }
                    Event::Key { key: Key::Backspace, pressed: true, .. } => {
                        text.pop();
                    }
                    Event::Text(t) | Event::Paste(t) => text.extend(t.chars().filter(|c| !c.is_control())),
                    _ => {}
                }
            }
            goto = (moved && !escape && !confirm).then_some(cur);
        }
        let mut events = Vec::new();
        for (i, d) in dims.iter_mut().enumerate() {
            let t = self.text(d.key);
            d.locked = !t.trim().is_empty();
            let parsed = parse_value(t);
            if let Some(v) = parsed {
                d.value = v;
            }
            if t != before[i] {
                match parsed {
                    Some(value) => events.push(DimEvent::Typed { key: d.key, value }),
                    None if !d.locked && !before[i].trim().is_empty() => events.push(DimEvent::Cleared { key: d.key }),
                    None => {}
                }
            }
        }
        if escape || confirm {
            if let Some(f) = ctx.memory(|m| m.focused()).filter(|f| ids.contains(f)) {
                ctx.memory_mut(|m| m.surrender_focus(f));
            }
        } else if let Some(j) = goto {
            ctx.memory_mut(|m| m.request_focus(ids[j]));
            cursor_to_end(ctx, ids[j], self.text(dims[j].key));
            if focused != Some(j) {
                events.push(DimEvent::Focused { key: dims[j].key });
            }
        }
        if takeover.is_some() {
            ctx.request_repaint();
        }
        if confirm {
            events.push(DimEvent::Confirm);
        }
        if escape {
            events.push(DimEvent::Escape);
        }
        events
    }

    fn draw(&mut self, ctx: &Context, anchor: Pos2, dims: &[Dimension], ids: &[Id]) {
        let filter = EventFilter { tab: true, escape: true, horizontal_arrows: true, vertical_arrows: true };
        Area::new(self.id.with("area")).order(Order::Foreground).fixed_pos(anchor + egui::vec2(18.0, 18.0)).show(ctx, |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (d, id) in dims.iter().zip(ids) {
                        let unit = d.unit.suffix();
                        let name = if unit.is_empty() { d.label.to_owned() } else { format!("{} ({unit})", d.label) };
                        let hint = d.unit.format(d.value);
                        ui.label(d.label);
                        let text = self.text_mut(d.key);
                        let was = text.clone();
                        let out = TextEdit::singleline(text).id(*id).desired_width(72.0).hint_text(hint.as_str()).event_filter(filter).show(ui);
                        let now = text.clone();
                        out.response.response.widget_info(|| WidgetInfo {
                            label: Some(name.clone()),
                            ..WidgetInfo::text_edit(true, was.clone(), now.clone(), hint.clone())
                        });
                    }
                });
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{
        commands::{AddPrimitiveCmd, Primitive},
        session::{Effect, Outcome, Session, StepInput},
    };
    use egui::Modifiers;
    use egui_kittest::{Harness, kittest::Queryable};
    use ringdesign_core::cad::Operation;

    struct App {
        session: Session,
        bar: DimensionBar,
        events: Vec<DimEvent>,
        committed: Vec<Effect>,
        host: Option<Id>,
    }
    /// A decoy button either side of the bar, and a real cylinder waiting at its radius step.
    fn harness() -> Harness<'static, App> {
        hosted(None)
    }
    /// The same, with a focusable widget standing in for the viewport that holds the tool's keys.
    fn hosted(host: Option<Id>) -> Harness<'static, App> {
        let mut session = Session::default();
        session.start(Box::new(AddPrimitiveCmd::new(Primitive::Cylinder, 7)));
        session.feed(StepInput::Pointer {
            world: [0.0, 9.5, 0.0],
            normal: [0.0, 1.0, 0.0],
            theta_deg: 90.0,
            across_mm: 0.0,
            height_mm: 0.0,
            snapped: None,
            dragging: false,
        });
        assert!(matches!(session.feed(StepInput::Click), Outcome::NextStep));
        let mut bar = DimensionBar::default();
        bar.set_host(host);
        let app = App { session, bar, events: vec![], committed: vec![], host };
        let mut h = Harness::builder().with_size([640.0, 320.0]).build_ui_state(
            |ui, app: &mut App| {
                let _ = ui.button("Before");
                if let Some(id) = app.host {
                    let rect = egui::Rect::from_min_size(egui::pos2(300.0, 200.0), egui::vec2(80.0, 40.0));
                    ui.interact(rect, id, egui::Sense::click());
                }
                let mut dims = app.session.dimensions();
                let events = app.bar.show(ui.ctx(), egui::pos2(40.0, 60.0), &mut dims);
                for e in &events {
                    let out = match e {
                        DimEvent::Typed { key, value } => app.session.feed(StepInput::Typed { key, value: *value }),
                        DimEvent::Cleared { key } => app.session.feed(StepInput::Cleared { key }),
                        DimEvent::Confirm => app.session.enter(),
                        DimEvent::Escape => app.session.escape(),
                        DimEvent::Focused { .. } => Outcome::Continue,
                    };
                    if let Outcome::Commit(effects) = out {
                        app.committed = effects;
                    }
                }
                app.events.extend(events);
                let _ = ui.button("After");
            },
            app,
        );
        h.run_steps(2);
        h
    }
    const LABELS: [&str; 6] = ["Before", "θ (°)", "Across (mm)", "Radius (mm)", "Height (mm)", "After"];
    fn focused(h: &Harness<'_, App>) -> Vec<&'static str> {
        LABELS.into_iter().filter(|l| h.query_by_label(l).is_some_and(|n| n.is_focused())).collect()
    }
    fn value(h: &Harness<'_, App>, label: &str) -> String {
        h.get_by_label(label).value().unwrap_or_default()
    }
    fn typed(h: &Harness<'_, App>) -> Vec<DimEvent> {
        h.state().events.iter().filter(|e| !matches!(e, DimEvent::Focused { .. })).cloned().collect()
    }
    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key { key, pressed: true, modifiers, repeat: false, physical_key: None }
    }

    // A key press is a down frame and an up frame; the extra step lets a newly focused field's filter land.
    #[test]
    fn tab_cycles_inside_the_bar_never_reaches_the_decoys_and_the_typed_cylinder_commits() {
        let mut h = harness();
        assert_eq!(focused(&h), Vec::<&str>::new());
        h.get_by_label("Radius (mm)").type_text("2.5");
        h.run_steps(2);
        assert_eq!(focused(&h), ["Radius (mm)"], "a number typed with nothing focused starts the first field");
        assert_eq!(value(&h, "Radius (mm)"), "2.5");
        h.key_press(Key::Tab);
        h.run_steps(2);
        assert_eq!(focused(&h), ["Height (mm)"]);
        h.key_press(Key::Tab);
        h.run_steps(2);
        assert_eq!(focused(&h), ["Radius (mm)"], "tab on the last field wraps to the first");
        h.key_press_modifiers(Modifiers::SHIFT, Key::Tab);
        h.run_steps(2);
        assert_eq!(focused(&h), ["Height (mm)"], "shift-tab goes back");
        h.get_by_label("Height (mm)").type_text("4");
        h.run_steps(2);
        h.key_press(Key::Enter);
        h.run_steps(2);
        assert_eq!(typed(&h), [DimEvent::Typed { key: "radius", value: 2.5 }, DimEvent::Typed { key: "height", value: 4.0 }, DimEvent::Confirm]);
        let focus: Vec<_> = h.state().events.iter().filter_map(|e| if let DimEvent::Focused { key } = e { Some(*key) } else { None }).collect();
        assert_eq!(focus, ["radius", "height", "radius", "height"]);
        assert!(!h.state().session.is_live());
        let [Effect::Add { feature }] = h.state().committed.as_slice() else { panic!("{:?}", h.state().committed) };
        assert!(matches!(feature.operation, Operation::Cylinder { radius_mm: 2.5, height_mm: 4.0 }));
        assert_eq!(feature.component.placement.theta_deg(), Some(90.0));
        assert_eq!(focused(&h), Vec::<&str>::new(), "enter leaves nothing focused, the decoys included");
    }

    #[test]
    fn escape_clears_a_typed_field_and_keeps_focus_then_a_second_escape_leaves_the_step() {
        let mut h = harness();
        h.get_by_label("Radius (mm)").type_text("2.5");
        h.run_steps(2);
        assert!(h.state().session.dimensions()[0].locked);
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(focused(&h), ["Radius (mm)"], "a cleared field keeps focus");
        assert_eq!(value(&h, "Radius (mm)"), "");
        assert!(!h.state().session.dimensions()[0].locked);
        assert_eq!(h.state().events.last(), Some(&DimEvent::Cleared { key: "radius" }));
        h.key_press(Key::Escape);
        h.run_steps(2);
        assert_eq!(h.state().events.last(), Some(&DimEvent::Escape));
        assert_eq!(focused(&h), Vec::<&str>::new());
        assert_eq!(h.state().session.command().unwrap().step(), 0, "the session backed a step");
        assert!(h.state().session.is_live());
        assert_eq!(value(&h, "θ (°)"), "", "the step's own fields are fresh");
    }

    #[test]
    fn a_key_in_the_frame_a_field_gains_focus_still_stays_in_the_bar() {
        // Shift-Tab the frame after a typed number focused the radius, before its filter could land.
        let mut h = harness();
        h.event(Event::Text("2.5".into()));
        h.event(key(Key::Tab, Modifiers::SHIFT));
        h.run_steps(2);
        assert_eq!(focused(&h), ["Height (mm)"], "egui would have sent it to the decoy before the radius");
        // Escape the frame after Shift-Tab focused the radius: the text clears and the field keeps focus.
        h.get_by_label("Height (mm)").type_text("4");
        h.run_steps(2);
        h.event(key(Key::Tab, Modifiers::SHIFT));
        h.event(key(Key::Escape, Modifiers::NONE));
        h.run_steps(2);
        assert_eq!(focused(&h), ["Radius (mm)"]);
        assert_eq!((value(&h, "Radius (mm)"), value(&h, "Height (mm)")), (String::new(), "4".to_owned()));
        assert!(h.state().events.contains(&DimEvent::Cleared { key: "radius" }) && !h.state().events.contains(&DimEvent::Escape));
        let locked: Vec<_> = h.state().session.dimensions().iter().map(|d| d.locked).collect();
        assert_eq!(locked, [false, true]);
    }

    #[test]
    fn keys_batched_into_one_frame_land_in_the_fields_they_were_typed_into() {
        let mut h = harness();
        h.get_by_label("Radius (mm)").type_text("2.");
        h.run_steps(2);
        // A slow frame: the rest of the radius, Tab, the height and Enter all arrive together.
        h.input_mut().events.extend([
            Event::Text("5".into()),
            key(Key::Tab, Modifiers::NONE),
            Event::Text("4".into()),
            key(Key::Enter, Modifiers::NONE),
        ]);
        h.run_steps(2);
        assert_eq!(typed(&h), [DimEvent::Typed { key: "radius", value: 2.0 }, DimEvent::Typed { key: "radius", value: 2.5 }, DimEvent::Typed { key: "height", value: 4.0 }, DimEvent::Confirm]);
        let [Effect::Add { feature }] = h.state().committed.as_slice() else { panic!("{:?}", h.state().committed) };
        assert!(matches!(feature.operation, Operation::Cylinder { radius_mm: 2.5, height_mm: 4.0 }));
    }

    #[test]
    fn a_letter_starts_nothing_a_session_lock_shows_as_text_and_backspacing_to_empty_unlocks() {
        let mut h = harness();
        h.get_by_label("Radius (mm)").type_text("g");
        h.run_steps(2);
        assert!(h.state().events.is_empty());
        assert_eq!(focused(&h), Vec::<&str>::new());
        h.state_mut().session.feed(StepInput::Typed { key: "height", value: 3.25 });
        h.run_steps(2);
        assert_eq!(value(&h, "Height (mm)"), "3.25");
        h.state_mut().session.feed(StepInput::Typed { key: "height", value: 0.0125 });
        h.run_steps(2);
        assert_eq!(value(&h, "Height (mm)"), "0.0125", "a value set elsewhere replaces stale text");
        h.state_mut().session.feed(StepInput::Cleared { key: "height" });
        h.run_steps(2);
        assert_eq!(value(&h, "Height (mm)"), "");
        h.get_by_label("Radius (mm)").type_text("7");
        h.run_steps(2);
        h.key_press(Key::Backspace);
        h.run_steps(2);
        assert_eq!(h.state().events.last(), Some(&DimEvent::Cleared { key: "radius" }));
        assert!(!h.state().session.dimensions()[0].locked);
        assert_eq!(focused(&h), ["Radius (mm)"]);
        assert!(h.state().bar.has_focus(&h.ctx));
    }

    #[test]
    fn a_number_typed_while_the_host_holds_the_keys_starts_the_first_field() {
        let host = Id::new("viewport");
        let mut h = hosted(Some(host));
        h.ctx.memory_mut(|m| m.request_focus(host));
        h.run_steps(2);
        h.event(Event::Text("2.5".into()));
        h.run_steps(2);
        assert_eq!(focused(&h), ["Radius (mm)"], "focus on the host counts as none");
        assert_eq!(value(&h, "Radius (mm)"), "2.5");
        assert_eq!(typed(&h), [DimEvent::Typed { key: "radius", value: 2.5 }]);
        // Without the host named, a focused widget keeps its keys.
        let mut h = hosted(Some(host));
        h.state_mut().bar.set_host(None);
        h.ctx.memory_mut(|m| m.request_focus(host));
        h.run_steps(2);
        h.event(Event::Text("2.5".into()));
        h.run_steps(2);
        assert_eq!(focused(&h), Vec::<&str>::new());
        assert!(typed(&h).is_empty());
        // The host's Tab hands the keys to a field by name.
        let mut h = hosted(Some(host));
        h.state().bar.focus_field(&h.ctx, "height");
        h.run_steps(2);
        assert_eq!(focused(&h), ["Height (mm)"]);
    }

    #[test]
    fn values_parse_with_their_units_and_print_without_trailing_zeros() {
        assert_eq!(parse_value(" 12.5 mm"), Some(12.5));
        assert_eq!(parse_value("-3°"), Some(-3.0));
        assert_eq!(parse_value("2deg"), Some(2.0));
        assert_eq!(parse_value("1.5×"), Some(1.5));
        assert_eq!(parse_value("2,5"), Some(2.5));
        assert_eq!(parse_value("2."), Some(2.0));
        assert_eq!(parse_value("-"), None);
        assert_eq!(parse_value(""), None);
        assert_eq!(parse_value("inf"), None);
        assert_eq!(plain(2.5), "2.5");
        assert_eq!(plain(3.0), "3");
        assert_eq!(plain(0.0125), "0.0125");
        assert_eq!(plain(-0.0000004), "0");
    }
}
