//! A long press's menu drawn for a finger: thumb-high rows, submenus opened in place, the phone's own rows beside the desktop's, and what the phone lacks greyed with why.
use egui::{Pos2, Rect};
use egui_mobile::egui;
use ringdesign_core::{
    cad::{Evaluated, SurfaceKind},
    interaction::pick::{Entity, Pick},
    sketch::Id,
};
use ringdesign_workbench::{
    icons::Icon,
    touch,
    viewport::{MenuAction, MenuItem, Sel},
};

/// Why the phone cannot serve `action` yet; `None` for one it serves.
pub fn not_here(action: &MenuAction) -> Option<&'static str> {
    Some(match action {
        MenuAction::ToggleGrid => "The phone's view has no ground grid",
        _ => return None,
    })
}

/// What a row only the phone's menu carries does.
#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    /// A work plane on planar face `face` of part `feature`, offset by a drag or a typed distance.
    PlaneOnFace { feature: Id, face: u32 },
    /// A work plane through the finger's axis at a typed angle, `theta_deg` where the band was pressed.
    PlaneAtAngle { theta_deg: f64 },
    /// The whole ring shown again after parts were shown alone.
    ShowAll,
    /// Part `id` taken out of the parts shown alone, the others staying.
    TakeOut(Id),
}

/// One of the phone's own rows: its mark, its words, what it does, and why not when it is not offered.
#[derive(Clone, Debug, PartialEq)]
pub struct ExtraItem {
    pub icon: Icon,
    pub label: &'static str,
    pub act: Extra,
    pub enabled: bool,
    pub hint: String,
}

/// The rows the phone adds for what lies under the finger, else the last thing chosen: a work plane on a flat face or through the axis where the band was pressed,
/// and while parts are shown alone, taking one of several out and the whole ring again.
pub fn extras(under: Option<&Pick>, chosen: Option<&Sel>, evaluated: Option<&Evaluated>, isolated: &[Id]) -> Vec<ExtraItem> {
    let mut out = Vec::new();
    let subject = match (under, chosen) {
        (Some(p), _) => Some((p.entity.clone(), p.world)),
        (None, Some(Sel::Face { feature, face })) => Some((Entity::Face { feature: *feature, face: *face }, [0.0; 3])),
        (None, Some(Sel::BandPoint { world, .. })) => Some((Entity::Band, *world)),
        _ => None,
    };
    match subject.as_ref().map(|(e, w)| (e, *w)) {
        Some((Entity::Face { feature, face }, _)) => {
            let part = evaluated.and_then(|e| e.components.iter().find(|c| c.id == *feature));
            let flat = match part {
                None => Err(format!("Part #{feature} has not built yet")),
                Some(c) if c.settings.reference => Err("A stone carries no work plane; put one on its setting's face".to_string()),
                Some(c) if c.made.is_some() => Err(format!("{} is built as a mesh; a work plane lies on a kernel part's flat face", c.name)),
                Some(c) => match c.trace.face_kind.get(*face as usize) {
                    Some(SurfaceKind::Plane) => Ok(()),
                    Some(kind) => Err(format!("Face {face} of {} is a {}; a work plane lies on a flat face", c.name, format!("{kind:?}").to_lowercase())),
                    None => Err(format!("{} has no face {face}", c.name)),
                },
            };
            out.push(ExtraItem {
                icon: Icon::Section,
                label: "Work plane here…",
                act: Extra::PlaneOnFace { feature: *feature, face: *face },
                enabled: flat.is_ok(),
                hint: flat.err().unwrap_or_else(|| "A plane on this face, moved off it by a drag or a typed distance".into()),
            });
        }
        Some((Entity::Band, world)) => out.push(ExtraItem {
            icon: Icon::Section,
            label: "Work plane at angle…",
            act: Extra::PlaneAtAngle { theta_deg: world[1].atan2(world[0]).to_degrees().round().rem_euclid(360.0) },
            enabled: true,
            hint: "A plane through the finger's axis, at the angle you type".into(),
        }),
        _ => {}
    }
    // A part among several shown alone can be taken out of the view.
    let part = match (under.map(|p| &p.entity), chosen) {
        (Some(Entity::Part { feature } | Entity::Face { feature, .. } | Entity::Edge { feature, .. } | Entity::Vertex { feature, .. }), _) => Some(*feature),
        (Some(_), _) => None,
        (None, Some(s)) => s.feature(),
        (None, None) => None,
    };
    if let Some(id) = part.filter(|id| isolated.len() > 1 && isolated.contains(id)) {
        let n = isolated.len() - 1;
        let hint = if n == 1 { "1 other part stays shown alone".to_string() } else { format!("{n} other parts stay shown alone") };
        out.push(ExtraItem { icon: Icon::Close, label: "Take it out of the view", act: Extra::TakeOut(id), enabled: true, hint });
    }
    if !isolated.is_empty() {
        out.push(ExtraItem { icon: Icon::Layers, label: "Show the whole ring", act: Extra::ShowAll, enabled: true, hint: "Every part and the band again".into() });
    }
    out
}

/// The desktop's items as the phone offers them: everything kept in its place, what it cannot do yet disabled and saying why;
/// Isolate adds a part to the parts shown alone, and is not offered for one already there.
pub fn phone_items(items: Vec<MenuItem>, isolated: &[Id]) -> Vec<MenuItem> {
    items
        .into_iter()
        .filter(|item| !matches!(item.action, MenuAction::IsolateInCad(id) if isolated.contains(&id)))
        .map(|mut item| {
            if let Some(why) = not_here(&item.action) {
                item.enabled = false;
                item.hint = why;
            }
            if matches!(item.action, MenuAction::IsolateInCad(_)) {
                (item.label, item.hint) = if isolated.is_empty() { ("Isolate".into(), "Show this part alone on the ring") } else { ("Isolate with the others".into(), "Show this part beside the parts already shown alone") };
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
    /// The point on the ring under the finger and the surface's normal there, when the menu is for what lies there.
    pub under: Option<([f64; 3], [f64; 3])>,
    /// The phone's own rows, after the desktop's sketch row on the first page.
    pub extras: Vec<ExtraItem>,
}

impl Menu {
    pub fn new(at: Pos2, heading: Option<String>, items: Vec<MenuItem>) -> Self {
        Self { at, heading, items, page: None, rect: Rect::NOTHING, below: None, under: None, extras: Vec::new() }
    }
}

/// What a row did.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    Act(MenuAction),
    Extra(Extra),
    Close,
}

/// A row's button: its mark and its words, a thumb high across the menu.
pub(super) fn row(ui: &mut egui::Ui, icon: Icon, label: &str, checked: bool, enabled: bool, width: f32) -> egui::Response {
    let button = egui::Button::selectable(checked, (icon.image(ui, 22.0), label)).min_size(egui::vec2(width, touch::TARGET_PT)).frame_when_inactive(true);
    ui.add_enabled(enabled, button)
}

/// A popup of `rows` thumb-high rows under `heading` by the finger at `at`, kept inside `view`, its rows laid out by `body` at the width it passes; the popup's rect.
pub(super) fn popup(ctx: &egui::Context, at: Pos2, below: &mut Option<bool>, heading: Option<&str>, rows: usize, view: Rect, body: impl FnOnce(&mut egui::Ui, f32)) -> Rect {
    let width = (view.width() - 32.0).clamp(180.0, 300.0);
    let tall = heading.is_some() as usize as f32 * 22.0 + rows as f32 * (touch::TARGET_PT + 4.0) + 20.0;
    // Opens below the finger when its first page fits, else above it, and stays there as its pages change.
    let below = *below.get_or_insert(at.y + tall <= view.bottom());
    let (pivot, pos) = if below { (egui::Align2::LEFT_TOP, at + egui::vec2(-width * 0.25, 12.0)) } else { (egui::Align2::LEFT_BOTTOM, at + egui::vec2(-width * 0.25, -12.0)) };
    let area = egui::Area::new(area())
        .order(egui::Order::Foreground)
        .pivot(pivot)
        .fixed_pos(pos)
        .constrain_to(view.shrink(4.0))
        .show(ctx, |ui| {
            // Lets a page grow past the size the area last drew at.
            ui.set_max_height(view.height() - 8.0);
            egui::Frame::popup(ui.style()).fill(egui::Color32::from_rgba_unmultiplied(16, 15, 24, 244)).show(ui, |ui| {
                ui.set_width(width);
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.spacing_mut().interact_size.y = touch::TARGET_PT;
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
                if let Some(h) = heading {
                    ui.label(egui::RichText::new(h).size(12.0).color(crate::theme::INK_DIM));
                }
                egui::ScrollArea::vertical().max_height((view.height() - 48.0).max(touch::TARGET_PT * 2.0)).show(ui, |ui| body(ui, width));
            });
        });
    area.response.rect
}

/// Draws the menu by the finger, kept inside `view`; the choice a row made.
pub fn show(ctx: &egui::Context, menu: &mut Menu, view: Rect) -> Option<Choice> {
    let page = rows(&menu.items, menu.page);
    let mut chosen = None;
    let mut page_to = menu.page;
    let items = &menu.items;
    let extras: &[ExtraItem] = if menu.page.is_none() { &menu.extras } else { &[] };
    // The phone's rows stand after the sketch row, else last.
    let after = page.iter().position(|r| matches!(r, Row::Item(k) if matches!(items[*k].action, MenuAction::SketchOnFace { .. } | MenuAction::SketchOnPlane { .. })));
    menu.rect = popup(ctx, menu.at, &mut menu.below, menu.heading.as_deref(), page.len() + extras.len(), view, |ui, width| {
        let own = |ui: &mut egui::Ui, chosen: &mut Option<Choice>| {
            for x in extras {
                if row(ui, x.icon, x.label, false, x.enabled, width).clicked() {
                    *chosen = Some(Choice::Extra(x.act.clone()));
                }
                if !x.enabled {
                    ui.label(egui::RichText::new(&x.hint).size(11.0).color(crate::theme::INK_DIM));
                }
            }
        };
        if after.is_none() {
            own(ui, &mut chosen);
        }
        for (i, r) in page.iter().enumerate() {
            match *r {
                Row::Back => {
                    if row(ui, Icon::Collapse, "Back", false, true, width).clicked() {
                        page_to = None;
                    }
                }
                Row::Sub { name, first, count } => {
                    let group = &items[first..first + count];
                    let icon = group.iter().find(|x| x.checked).map_or(group[0].icon, |x| x.icon);
                    let label = format!("{name}…");
                    let closed = closed_because(group);
                    if row(ui, icon, &label, false, closed.is_none(), width).clicked() {
                        page_to = Some(name);
                    }
                    if let Some(why) = closed {
                        ui.label(egui::RichText::new(why).size(11.0).color(crate::theme::INK_DIM));
                    }
                }
                Row::Item(k) => {
                    let item = &items[k];
                    if row(ui, item.icon, &item.label, item.checked, item.enabled, width).clicked() {
                        chosen = Some(Choice::Act(item.action.clone()));
                    }
                    if !item.enabled {
                        ui.label(egui::RichText::new(item.hint).size(11.0).color(crate::theme::INK_DIM));
                    }
                }
            }
            if after == Some(i) {
                own(ui, &mut chosen);
            }
        }
        if row(ui, Icon::Close, "Close", false, true, width).clicked() {
            chosen = Some(Choice::Close);
        }
    });
    menu.page = page_to;
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
    fn the_band_menu_offers_parts_stones_a_sketch_and_a_work_plane_and_greys_only_the_grid() {
        let d = design();
        let under = pick(Entity::Band, [0.0, 9.5, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d), &[]);
        let off: Vec<(&str, &str)> = items.iter().filter(|i| !i.enabled).map(|i| (i.label.as_str(), i.hint)).collect();
        assert_eq!(off, [("Grid", "The phone's view has no ground grid")]);
        assert!(items.iter().any(|i| i.enabled && i.action == MenuAction::SketchOnPlane { theta_deg: 90.0, across_mm: 0.0 }));
        // The phone's own row: a plane through the axis at the angle pressed, and the whole ring back while a part stands alone.
        let own = extras(Some(&under), None, None, &[]);
        assert_eq!(own.iter().map(|x| (x.label, x.act.clone(), x.enabled)).collect::<Vec<_>>(), [("Work plane at angle…", Extra::PlaneAtAngle { theta_deg: 90.0 }, true)]);
        // Pressed below the axis, the angle reads on 0–360° as the band's readout does.
        let low = extras(Some(&pick(Entity::Band, [2.25, -9.74, 0.0])), None, None, &[]);
        assert_eq!(low[0].act, Extra::PlaneAtAngle { theta_deg: 283.0 });
        let alone = extras(Some(&under), None, None, &[3]);
        assert_eq!(alone.last().map(|x| x.act.clone()), Some(Extra::ShowAll));
        // A face that has not built cannot carry one, and says so.
        let face = extras(Some(&pick(Entity::Face { feature: 3, face: 1 }, [0.0, 10.0, 0.0])), None, None, &[]);
        assert_eq!((face[0].label, face[0].enabled, face[0].hint.as_str()), ("Work plane here…", false, "Part #3 has not built yet"));
        let top = rows(&items, None);
        let subs: Vec<(&str, usize)> = top.iter().filter_map(|r| match r {
            Row::Sub { name, count, .. } => Some((*name, *count)),
            _ => None,
        }).collect();
        let cuts = ringdesign_workbench::viewport::cutters::PIERCE_KEYS.len();
        assert_eq!(subs, [("Add CAD part here", 6), ("Add stone here", ringdesign_core::cad::builders::STONES.len()), ("Cut here", cuts)]);
        assert_eq!(top.len(), 3 + 7, "three folded submenus, then the sketch, two pin rows and four view rows");
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
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d), &[]);
        let off: Vec<&str> = items.iter().filter(|i| !i.enabled).map(|i| i.label.as_str()).collect();
        assert_eq!(off, ["Grid"], "sketching and isolating are the phone's now");
        assert!(items.iter().any(|i| i.enabled && i.action == MenuAction::IsolateInCad(3)));
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
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d), &[]);
        let view = Rect::from_min_size(Pos2::ZERO, egui::vec2(420.0, 1000.0));
        let mut menu = Menu::new(egui::pos2(200.0, 700.0), Some("Band at 90°".into()), items);
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
        // Ten rows do not fit under a finger 700 points down a 1000-point view, so the list hangs above it.
        let list = draw(&mut menu);
        assert_eq!(menu.below, Some(false));
        assert!((list.bottom() - 688.0).abs() < 1.0, "{list:?}");
        // All ten rows and Close stand at once, not scrolled inside egui's 400-point first guess at an area.
        assert!(list.height() > 10.0 * (touch::TARGET_PT + 4.0), "{list:?}");
        // The parts' page would fit beneath; it keeps the list's side instead of jumping under the finger.
        menu.page = Some("Add CAD part here");
        let page = draw(&mut menu);
        assert!(page.height() < list.height() && (page.bottom() - 688.0).abs() < 1.0, "{list:?} then {page:?}");
    }

    #[test]
    fn a_stones_attach_and_stage_rows_stay_shut_and_say_why() {
        let mut d = design();
        let mut stone = Feature { id: 5, name: "Round 6.5 mm".into(), enabled: true, operation: Operation::Sphere { radius_mm: 3.0 }, component: Component::default() };
        stone.component.reference = true;
        d.cad.as_mut().unwrap().append(stone).unwrap();
        let under = pick(Entity::Part { feature: 5 }, [0.0, 10.0, 0.0]);
        let items = phone_items(context_items(&Selection::default(), Some(&under), &d), &[]);
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

    #[test]
    fn isolate_adds_a_part_to_those_alone_and_a_part_already_alone_is_offered_out_instead() {
        let mut d = design();
        d.cad.as_mut().unwrap().append(Feature { id: 4, name: "Block".into(), enabled: true, operation: Operation::Box { size: [1.0; 3] }, component: Component::default() }).unwrap();
        let on = |id: Id| pick(Entity::Face { feature: id, face: 0 }, [0.0, 10.0, 0.0]);
        let isolate = |items: &[MenuItem]| items.iter().find(|i| matches!(i.action, MenuAction::IsolateInCad(_))).map(|i| (i.label.clone(), i.action.clone()));
        // Nothing alone: the post's menu isolates it, and there is nothing to take out or bring back.
        let items = phone_items(context_items(&Selection::default(), Some(&on(3)), &d), &[]);
        assert_eq!(isolate(&items), Some(("Isolate".into(), MenuAction::IsolateInCad(3))));
        assert!(extras(Some(&on(3)), None, None, &[]).iter().all(|x| !matches!(x.act, Extra::TakeOut(_) | Extra::ShowAll)));
        // The post alone: its own menu no longer isolates it, the block's adds the block beside it.
        let items = phone_items(context_items(&Selection::default(), Some(&on(3)), &d), &[3]);
        assert_eq!(isolate(&items), None);
        let block = phone_items(context_items(&Selection::default(), Some(&on(4)), &d), &[3]);
        assert_eq!(isolate(&block), Some(("Isolate with the others".into(), MenuAction::IsolateInCad(4))));
        // One part alone is taken out by showing the whole ring; of two, either comes out alone.
        let acts = |under: &Pick, alone: &[Id]| extras(Some(under), None, None, alone).into_iter().map(|x| x.act).filter(|a| matches!(a, Extra::TakeOut(_) | Extra::ShowAll)).collect::<Vec<_>>();
        assert_eq!(acts(&on(3), &[3]), [Extra::ShowAll]);
        assert_eq!(acts(&on(3), &[3, 4]), [Extra::TakeOut(3), Extra::ShowAll]);
        assert_eq!(acts(&on(4), &[3, 4]), [Extra::TakeOut(4), Extra::ShowAll]);
        // With nothing under the finger, the last part chosen is the one taken out.
        let chosen = extras(None, Some(&Sel::Part(4)), None, &[3, 4]);
        assert_eq!(chosen.iter().find(|x| matches!(x.act, Extra::TakeOut(_))).map(|x| (x.act.clone(), x.hint.as_str())), Some((Extra::TakeOut(4), "1 other part stays shown alone")));
    }
}
