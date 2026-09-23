//! A long press's menu drawn for a finger: thumb-high rows, submenus opened in place, what the phone lacks greyed with why.
use egui::{Pos2, Rect};
use egui_mobile::egui;
use ringdesign_workbench::{
    icons::Icon,
    touch,
    viewport::{MenuAction, MenuItem},
};

/// Isolating a part is the desktop CAD pane's.
pub const NO_ISOLATE: &str = "Not on the phone yet: isolating a part is on the desktop's CAD pane";

/// Why the phone cannot serve `action` yet; `None` for one it serves.
pub fn not_here(action: &MenuAction) -> Option<&'static str> {
    Some(match action {
        MenuAction::SketchOnFace { .. } | MenuAction::SketchOnPlane { .. } => "Not on the phone yet: sketch on the desktop, and the design file brings the sketch here",
        MenuAction::IsolateInCad(_) => NO_ISOLATE,
        MenuAction::ToggleGrid => "The phone's view has no ground grid",
        _ => return None,
    })
}

/// The desktop's items as the phone offers them: everything kept in its place, what it cannot do yet disabled and saying why.
pub fn phone_items(items: Vec<MenuItem>) -> Vec<MenuItem> {
    items
        .into_iter()
        .map(|mut item| {
            if let Some(why) = not_here(&item.action) {
                item.enabled = false;
                item.hint = why;
            }
            item
        })
        .collect()
}

/// One row of a page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// An item, by its index.
    Item(usize),
    /// A submenu folded into one row: its name, its first item and how many it holds.
    Sub { name: &'static str, first: usize, count: usize },
    /// The way back from a submenu.
    Back,
}

/// A page's rows: the top page folds each submenu's run into one row, a submenu's page is the way back then its items.
pub fn rows(items: &[MenuItem], page: Option<&str>) -> Vec<Row> {
    if let Some(sub) = page {
        return std::iter::once(Row::Back).chain(items.iter().enumerate().filter(|(_, i)| i.submenu == Some(sub)).map(|(k, _)| Row::Item(k))).collect();
    }
    let mut out = Vec::new();
    let mut k = 0;
    while k < items.len() {
        match items[k].submenu {
            Some(name) => {
                let end = items[k..].iter().position(|x| x.submenu != Some(name)).map_or(items.len(), |n| k + n);
                out.push(Row::Sub { name, first: k, count: end - k });
                k = end;
            }
            None => {
                out.push(Row::Item(k));
                k += 1;
            }
        }
    }
    out
}

/// Why a folded submenu cannot open: its first item's reason, when none of its items is offered.
pub fn closed_because(group: &[MenuItem]) -> Option<&'static str> {
    (!group.is_empty() && group.iter().all(|x| !x.enabled)).then(|| group[0].hint)
}

/// The area the menu is drawn in.
pub fn area() -> egui::Id {
    egui::Id::new("phone-cad-menu")
}

/// A long press's menu, open over the ring.
#[derive(Clone, Debug)]
pub struct Menu {
    /// Where the finger was.
    pub at: Pos2,
    pub heading: Option<String>,
    pub items: Vec<MenuItem>,
    /// The submenu shown in place of the list, when one is open.
    pub page: Option<&'static str>,
    /// The popup as last drawn.
    pub rect: Rect,
    /// Whether the popup hangs below the finger, settled when it is first drawn.
    pub below: Option<bool>,
    /// The point on the ring under the finger, when the menu is for what lies there.
    pub world: Option<[f64; 3]>,
}

impl Menu {
    pub fn new(at: Pos2, heading: Option<String>, items: Vec<MenuItem>) -> Self {
        Self { at, heading, items, page: None, rect: Rect::NOTHING, below: None, world: None }
    }
}

/// What a row did.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Act(MenuAction),
    Close,
}

/// A row's button: its mark and its words, a thumb high across the menu.
fn row(ui: &mut egui::Ui, icon: Icon, label: &str, checked: bool, enabled: bool, width: f32) -> egui::Response {
    let button = egui::Button::selectable(checked, (icon.image(ui, 22.0), label)).min_size(egui::vec2(width, touch::TARGET_PT)).frame_when_inactive(true);
    ui.add_enabled(enabled, button)
}

/// Draws the menu by the finger, kept inside `view`; the choice a row made.
pub fn show(ctx: &egui::Context, menu: &mut Menu, view: Rect) -> Option<Choice> {
    let width = (view.width() - 32.0).clamp(180.0, 300.0);
    let page = rows(&menu.items, menu.page);
    let tall = menu.heading.is_some() as usize as f32 * 22.0 + page.len() as f32 * (touch::TARGET_PT + 4.0) + 20.0;
    // Opens below the finger when its first page fits, else above it, and stays there as its pages change.
    let below = *menu.below.get_or_insert(menu.at.y + tall <= view.bottom());
    let (pivot, at) = if below { (egui::Align2::LEFT_TOP, menu.at + egui::vec2(-width * 0.25, 12.0)) } else { (egui::Align2::LEFT_BOTTOM, menu.at + egui::vec2(-width * 0.25, -12.0)) };
    let mut chosen = None;
    let area = egui::Area::new(area())
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(at)
        .constrain_to(view.shrink(4.0))
        .show(ctx, |ui| {
            // Lets a page grow past the size the area last drew at.
            ui.set_max_height(view.height() - 8.0);
            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgba_unmultiplied(16, 15, 24, 244)).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.spacing_mut().interact_size.y = touch::TARGET_PT;
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if let Some(h) = &menu.heading {
                    ui.label(egui::RichText::new(h).size(12.0).color(crate::theme::INK_DIM));
                }
                egui::ScrollArea::vertical().max_height((view.height() - 48.0).max(touch::TARGET_PT * 2.0)).show(ui, |ui| {
                    for r in &page {
                        match *r {
                            Row::Back => {
                                if row(ui, Icon::Collapse, "Back", false, true, width).clicked() {
                                    menu.page = None;
                                }
                            }
                            Row::Sub { name, first, count } => {
                                let group = &menu.items[first..first + count];
                                let icon = group.iter().find(|x| x.checked).map_or(group[0].icon, |x| x.icon);
                                let label = format!("{name}…");
                                let closed = closed_because(group);
                                if row(ui, icon, &label, false, closed.is_none(), width).clicked() {
                                    menu.page = Some(name);
                                }
                                if let Some(why) = closed {
                                    ui.label(egui::RichText::new(why).size(11.0).color(crate::theme::INK_DIM));
                                }
                            }
                            Row::Item(k) => {
                                let item = &menu.items[k];
                                if row(ui, item.icon, &item.label, item.checked, item.enabled, width).clicked() {
                                    chosen = Some(Choice::Act(item.action.clone()));
                                }
                                if !item.enabled {
                                    ui.label(egui::RichText::new(item.hint).size(11.0).color(crate::theme::INK_DIM));
                                }
                            }
                        }
                    }
                    if row(ui, Icon::Close, "Close", false, true, width).clicked() {
                        chosen = Some(Choice::Close);
                    }
                });
            });
        });
    menu.rect = area.response.rect;
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::{
        RingDesign,
        cad::{Attach, Component, Document, Feature, Operation},
        interaction::pick::{Entity, Pick},
    };
    use ringdesign_workbench::viewport::{Selection, context_items};

    fn design() -> RingDesign {
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature { id: 3, name: "Post".into(), enabled: true, operation: Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 }, component: Component { attach: Attach::Join, ..Component::default() } }).unwrap();
        d.cad = Some(doc);
        d
    }
    fn pick(entity: Entity, world: [f64; 3]) -> Pick {
        Pick { entity, world, normal: [0.0, 1.0, 0.0], depth: 1.0, px: 0.0 }
    }

    #[test]
    fn the_band_menu_offers_parts_and_stones_and_says_sketching_is_not_here_yet() {
        let d = design();
        let under = pick(Entity::Band, [0.0, 9.5, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d));
        let off: Vec<(&str, &str)> = items.iter().filter(|i| !i.enabled).map(|i| (i.label.as_str(), i.hint)).collect();
        assert_eq!(off.len(), 2);
        assert_eq!(off[0].0, "Sketch on a plane here");
        assert!(off[0].1.starts_with("Not on the phone yet"), "{}", off[0].1);
        assert_eq!(off[1], ("Grid", "The phone's view has no ground grid"));
        let top = rows(&items, None);
        let subs: Vec<(&str, usize)> = top.iter().filter_map(|r| match r {
            Row::Sub { name, count, .. } => Some((*name, *count)),
            _ => None,
        }).collect();
        assert_eq!(subs, [("Add CAD part here", 6), ("Add stone here", ringdesign_core::cad::builders::STONES.len())]);
        assert_eq!(top.len(), 2 + 7, "two folded submenus, then the sketch, two pin rows and four view rows");
        // The stones' page: the way back, then every preset, each served here.
        let stones = rows(&items, Some("Add stone here"));
        assert_eq!(stones[0], Row::Back);
        assert_eq!(stones.len(), 1 + ringdesign_core::cad::builders::STONES.len());
        assert!(stones[1..].iter().all(|r| matches!(r, Row::Item(k) if items[*k].enabled && matches!(items[*k].action, MenuAction::AddStone { .. }))));
    }

    #[test]
    fn a_face_menu_keeps_press_pull_and_patterns_and_greys_only_what_the_phone_lacks() {
        let d = design();
        let under = pick(Entity::Face { feature: 3, face: 1 }, [0.0, 10.0, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d));
        let off: Vec<&str> = items.iter().filter(|i| !i.enabled).map(|i| i.label.as_str()).collect();
        assert_eq!(off, ["Sketch on this face", "Isolate in CAD", "Grid"]);
        assert!(items.iter().any(|i| i.enabled && i.action == MenuAction::PressPull { feature: 3, face: 1 }));
        let top = rows(&items, None);
        let stones = ringdesign_core::cad::builders::STONES.len();
        assert!(top.contains(&Row::Sub { name: "Add stone here", first: 1, count: stones }), "{top:?}");
        assert!(items[1..=stones].iter().all(|i| i.enabled && matches!(i.action, MenuAction::AddStoneOnFace { feature: 3, face: 1, .. })));
        assert!(top.contains(&Row::Sub { name: "Pattern", first: 2 + stones, count: 4 }), "{top:?}");
        let attach = rows(&items, Some("Attach"));
        let ticked: Vec<&str> = attach.iter().filter_map(|r| match r {
            Row::Item(k) if items[*k].checked => Some(items[*k].label.as_str()),
            _ => None,
        }).collect();
        assert_eq!(ticked, ["Join"]);
        assert_eq!(not_here(&MenuAction::AddStoneOnFace { feature: 3, face: 1, key: "round-5" }), None, "a stone on a part's face is set here too");
        assert_eq!(not_here(&MenuAction::Attach(3, Attach::Cut)), None);
        let Some(Row::Sub { first, count, .. }) = top.iter().find(|r| matches!(r, Row::Sub { name: "Attach", .. })).copied() else { panic!("{top:?}") };
        assert_eq!(closed_because(&items[first..first + count]), None, "a metal part's attachments open");
    }

    #[test]
    fn a_menu_keeps_the_side_it_opened_on_when_a_shorter_page_replaces_its_list() {
        let d = design();
        let under = pick(Entity::Band, [0.0, 9.5, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d));
        let view = Rect::from_min_size(Pos2::ZERO, egui::vec2(420.0, 1000.0));
        let mut menu = Menu::new(egui::pos2(200.0, 600.0), Some("Band at 90°".into()), items);
        let ctx = egui::Context::default();
        // An area is sized from its last pass, so a few passes let a new page settle.
        let draw = |menu: &mut Menu| {
            for _ in 0..4 {
                let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(view), ..Default::default() }, |ui| {
                    show(ui.ctx(), menu, view);
                });
                out.textures_delta.clear();
            }
            menu.rect
        };
        // Nine rows do not fit under a finger 600 points down a 1000-point view, so the list hangs above it.
        let list = draw(&mut menu);
        assert_eq!(menu.below, Some(false));
        assert!((list.bottom() - 588.0).abs() < 1.0, "{list:?}");
        // All nine rows and Close stand at once, not scrolled inside egui's 400-point first guess at an area.
        assert!(list.height() > 9.0 * (touch::TARGET_PT + 4.0), "{list:?}");
        // The parts' page would fit beneath; it keeps the list's side instead of jumping under the finger.
        menu.page = Some("Add CAD part here");
        let page = draw(&mut menu);
        assert!(page.height() < list.height() && (page.bottom() - 588.0).abs() < 1.0, "{list:?} then {page:?}");
    }

    #[test]
    fn a_stones_attach_and_stage_rows_stay_shut_and_say_why() {
        let mut d = design();
        let mut stone = Feature { id: 5, name: "Round 6.5 mm".into(), enabled: true, operation: Operation::Sphere { radius_mm: 3.0 }, component: Component::default() };
        stone.component.reference = true;
        d.cad.as_mut().unwrap().append(stone).unwrap();
        let under = pick(Entity::Part { feature: 5 }, [0.0, 10.0, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d));
        let shut: Vec<(&str, Option<&str>)> = rows(&items, None)
            .into_iter()
            .filter_map(|r| match r {
                Row::Sub { name, first, count } => Some((name, closed_because(&items[first..first + count]))),
                _ => None,
            })
            .filter(|(_, why)| why.is_some())
            .collect();
        assert_eq!(shut, [("Attach", Some("A reference stone is never metal")), ("Stage", Some("A reference stone is never metal"))]);
    }
}
