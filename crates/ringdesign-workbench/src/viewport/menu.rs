//! What a right-click on the ring may do, decided by what lies under it — else by the last thing
//! chosen — and said as data, so the desktop and the phone draw the same menu and a test reads it
//! without a window.
use super::selection::{Sel, Selection, feature_name};
use crate::icons::Icon;
use ringdesign_core::{
    RingDesign,
    cad::{Attach, Stage},
    interaction::pick::{Entity, Pick},
    sketch::Id,
};

/// Starters that make sense seated on the ring's surface.
pub const PLACEABLE: [&str; 6] = ["Box", "Cylinder", "Sphere", "Extrude", "Sweep", "Loft"];

#[derive(Clone, Debug, PartialEq)]
pub enum MenuAction {
    AddPartHere { theta_deg: f64, height_mm: f64, label: &'static str },
    EditFeature(Id),
    Attach(Id, Attach),
    Stage(Id, Stage),
    FilletEdge { feature: Id, edge: u32 },
    ChamferEdge { feature: Id, edge: u32 },
    IsolateInCad(Id),
    FitView,
    OpenCad,
    ToggleWire,
    ToggleGrid,
    /// A new sketch on a planar face of a part.
    SketchOnFace { feature: Id, face: u32 },
    /// A new sketch on a plane through a point of the band.
    SketchOnPlane { theta_deg: f64, across_mm: f64 },
    /// A reference stone seated where the click landed; `key` names the stone.
    AddStone { theta_deg: f64, height_mm: f64, key: &'static str },
    /// A made setting round a stone: a reference part, or a height-field stone by its layer path.
    Setting { part: Option<Id>, stone: Option<Vec<usize>>, key: &'static str },
    /// A reference pin dropped on the ring where the click landed.
    PinHere { world: [f64; 3] },
    /// Every pin taken off the ring.
    ClearPins,
    /// A pattern of a part: an array or a mirror, named by `key`.
    Pattern { feature: Id, key: &'static str },
    /// A planar face of a part pushed or pulled along its normal.
    PressPull { feature: Id, face: u32 },
    /// A reference stone seated on a planar face of a part; `key` names the stone.
    AddStoneOnFace { feature: Id, face: u32, key: &'static str },
}

#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub icon: Icon,
    pub action: MenuAction,
    pub enabled: bool,
    pub hint: &'static str,
    /// Consecutive items naming the same submenu fold into it.
    pub submenu: Option<&'static str>,
    /// Drawn ticked: the attachment or stage the part has now.
    pub checked: bool,
}

impl MenuItem {
    pub(crate) fn new(label: impl Into<String>, icon: Icon, action: MenuAction, hint: &'static str) -> Self {
        Self { label: label.into(), icon, action, enabled: true, hint, submenu: None, checked: false }
    }
    pub(crate) fn under(mut self, submenu: &'static str) -> Self {
        self.submenu = Some(submenu);
        self
    }
    pub(crate) fn ticked(mut self, checked: bool) -> Self {
        self.checked = checked;
        self
    }
    pub(crate) fn only_if(mut self, enabled: bool, hint: &'static str) -> Self {
        if !enabled {
            self.enabled = false;
            self.hint = hint;
        }
        self
    }
}

/// What the menu is about.
enum Subject {
    /// A place on the band, when one is known.
    Band(Option<[f64; 3]>),
    Feature { id: Id, edge: Option<u32>, face: Option<u32> },
    /// A height-field stone, by its layer path when one is known.
    Stone(Option<Vec<usize>>),
    Nothing,
}

fn subject(sel: &Selection, under: Option<&Pick>) -> Subject {
    if let Some(p) = under {
        return match &p.entity {
            Entity::Band => Subject::Band(Some(p.world)),
            Entity::Part { feature } | Entity::Vertex { feature, .. } => Subject::Feature { id: *feature, edge: None, face: None },
            Entity::Face { feature, face } => Subject::Feature { id: *feature, edge: None, face: Some(*face) },
            Entity::Edge { feature, edge } => Subject::Feature { id: *feature, edge: Some(*edge), face: None },
            Entity::Stone { path } => Subject::Stone(Some(path.clone())),
        };
    }
    match sel.items.last() {
        Some(Sel::BandPoint { world, .. }) => Subject::Band(Some(*world)),
        Some(Sel::Layer(_)) => Subject::Band(None),
        Some(Sel::Stone(path)) => Subject::Stone(Some(path.clone())),
        Some(Sel::Face { feature, face }) => Subject::Feature { id: *feature, edge: None, face: Some(*face) },
        Some(s) => match (s.feature(), s.edge()) {
            (Some(id), edge) => Subject::Feature { id, edge, face: None },
            _ => Subject::Nothing,
        },
        None => Subject::Nothing,
    }
}

/// The line over the items: what the menu is about, when it is one thing of a part.
pub fn heading(sel: &Selection, under: Option<&Pick>, design: &RingDesign) -> Option<String> {
    let name = |id: Id| feature_name(id, design, None);
    let of = |entity: &Entity| match entity {
        Entity::Face { feature, face } => Some(format!("Face {face} of {}", name(*feature))),
        Entity::Edge { feature, edge } => Some(format!("Edge {edge} of {}", name(*feature))),
        Entity::Vertex { feature, vertex } => Some(format!("Vertex {vertex} of {}", name(*feature))),
        Entity::Part { feature } => Some(name(*feature)),
        Entity::Stone { .. } => Some("Stone".into()),
        Entity::Band => None,
    };
    if let Some(p) = under {
        return of(&p.entity);
    }
    match sel.items.last()? {
        Sel::Face { feature, face } => of(&Entity::Face { feature: *feature, face: *face }),
        Sel::Edge { feature, edge } => of(&Entity::Edge { feature: *feature, edge: *edge }),
        Sel::Vertex { feature, vertex } => of(&Entity::Vertex { feature: *feature, vertex: *vertex }),
        Sel::Part(id) => Some(name(*id)),
        Sel::Stone(_) => Some("Stone".into()),
        Sel::BandPoint { .. } | Sel::Layer(_) => None,
    }
}

/// The items a right-click offers: what lies under it first, else the last thing chosen, then the
/// view. Pure: the caller draws them and routes the actions.
pub fn context_items(sel: &Selection, under: Option<&Pick>, design: &RingDesign) -> Vec<MenuItem> {
    let mut items = Vec::new();
    match subject(sel, under) {
        Subject::Band(Some(world)) => {
            let (x, y) = (world[0], world[1]);
            let radius = design.inner_radius_mm() + design.profile.thickness_mm;
            let (theta_deg, height_mm) = (y.atan2(x).to_degrees(), x.hypot(y) - radius);
            for label in PLACEABLE {
                items.push(MenuItem::new(label, Icon::Add, MenuAction::AddPartHere { theta_deg, height_mm, label }, "A new part seated where the click landed, joined to the band").under("Add CAD part here"));
            }
            items.extend(super::stones::band_items(theta_deg, height_mm));
            items.push(MenuItem::new("Sketch on a plane here", Icon::CadSketch, MenuAction::SketchOnPlane { theta_deg, across_mm: world[2] }, "A new sketch on a plane through this point of the band"));
            items.extend(super::pins::band_items(world));
        }
        Subject::Feature { id, edge, face } => {
            let feature = design.cad.as_ref().and_then(|d| d.features.iter().find(|f| f.id == id));
            let component = feature.map(|f| &f.component);
            let reference = component.is_some_and(|c| c.reference);
            const STONE: &str = "A reference stone is never metal";
            if let Some(face) = face {
                items.push(MenuItem::new("Sketch on this face", Icon::CadSketch, MenuAction::SketchOnFace { feature: id, face }, "A new sketch lying on this face, moving with it"));
                if !reference {
                    items.extend(super::stones::face_items(id, face));
                }
            }
            if reference {
                items.extend(super::stones::setting_items(Some(id), None));
            }
            if let Some(edge) = edge {
                items.push(MenuItem::new("Fillet this edge", Icon::CadFillet, MenuAction::FilletEdge { feature: id, edge }, "Round the edge with a new Fillet feature on this part"));
                items.push(MenuItem::new("Chamfer this edge", Icon::CadChamfer, MenuAction::ChamferEdge { feature: id, edge }, "Bevel the edge with a new Chamfer feature on this part"));
            }
            items.extend(super::patterns::part_items(id, face, reference));
            items.push(MenuItem::new("Edit feature", Icon::Panel, MenuAction::EditFeature(id), "Open the part in the CAD pane with its parameters"));
            let attach = component.map(|c| c.attach).unwrap_or_default();
            for (value, label, icon, hint) in [
                (Attach::Separate, "Separate", Icon::CadPlace, "Kept beside the band as its own solid"),
                (Attach::Join, "Join", Icon::CadUnion, "United into the band; the pattern and the verdict see one solid"),
                (Attach::Cut, "Cut", Icon::CadSubtract, "Subtracted from the band"),
            ] {
                items.push(MenuItem::new(label, icon, MenuAction::Attach(id, value), hint).under("Attach").ticked(attach == value).only_if(!reference, STONE));
            }
            let stage = component.map(|c| c.stage).unwrap_or_default();
            for (value, label, icon, hint) in [
                (Stage::Cast, "Cast", Icon::Casting, "Cast in the pattern"),
                (Stage::Bench, "Bench", Icon::Workshop, "Added at the bench after the pour; never in a sand pattern"),
            ] {
                items.push(MenuItem::new(label, icon, MenuAction::Stage(id, value), hint).under("Stage").ticked(stage == value).only_if(!reference, STONE));
            }
            items.push(MenuItem::new("Isolate in CAD", Icon::Layers, MenuAction::IsolateInCad(id), "Show this part alone in the CAD pane"));
        }
        Subject::Stone(path) => items.extend(super::stones::setting_items(None, path)),
        Subject::Band(None) | Subject::Nothing => {}
    }
    items.push(MenuItem::new("Fit view", Icon::Fit, MenuAction::FitView, "Frame the whole ring"));
    items.push(MenuItem::new("Open CAD workspace", Icon::Workshop, MenuAction::OpenCad, "The feature tree and the parts pane"));
    items.push(MenuItem::new("Wireframe", Icon::Wire, MenuAction::ToggleWire, "Draw the mesh's edges over the metal"));
    items.push(MenuItem::new("Grid", Icon::Grid, MenuAction::ToggleGrid, "The ground grid and axes"));
    items
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::viewport::selection::Mods;
    use ringdesign_core::cad::{Component, Document, Feature, Operation};

    fn design() -> RingDesign {
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        doc.append(Feature { id: 0, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(Feature {
            id: 3,
            name: "Bezel".into(),
            enabled: true,
            operation: Operation::Cylinder { radius_mm: 2.0, height_mm: 2.0 },
            component: Component { attach: Attach::Join, stage: Stage::Cast, ..Default::default() },
        })
        .unwrap();
        doc.append(Feature {
            id: 4,
            name: "Stone".into(),
            enabled: true,
            operation: Operation::Sphere { radius_mm: 1.0 },
            component: Component { reference: true, ..Default::default() },
        })
        .unwrap();
        d.cad = Some(doc);
        d
    }
    fn pick(entity: Entity, world: [f64; 3]) -> Pick {
        Pick { entity, world, normal: [0.0, 1.0, 0.0], depth: 1.0, px: 0.0 }
    }
    fn labels(items: &[MenuItem]) -> Vec<(Option<&'static str>, String)> {
        items.iter().map(|i| (i.submenu, i.label.clone())).collect()
    }

    #[test]
    fn the_band_offers_a_part_here_the_view_and_the_cad_workspace() {
        let d = design();
        let sel = Selection::default();
        let radius = d.inner_radius_mm() + d.profile.thickness_mm;
        let under = pick(Entity::Band, [0.0, radius + 0.5, 0.0]);
        let items = context_items(&sel, Some(&under), &d);
        assert_eq!(heading(&sel, Some(&under), &d), None);
        let adds: Vec<_> = items.iter().filter(|i| i.submenu == Some("Add CAD part here")).collect();
        assert_eq!(adds.len(), PLACEABLE.len());
        match &adds[1].action {
            MenuAction::AddPartHere { theta_deg, height_mm, label } => {
                assert!((theta_deg - 90.0).abs() < 1e-9 && (height_mm - 0.5).abs() < 1e-9 && *label == "Cylinder");
            }
            other => panic!("{other:?}"),
        }
        let stones: Vec<_> = items.iter().filter(|i| i.submenu == Some("Add stone here")).collect();
        assert_eq!(stones.len(), ringdesign_core::cad::builders::STONES.len());
        let tail: Vec<_> = items.iter().skip(adds.len() + stones.len()).map(|i| i.action.clone()).collect();
        let sketch = MenuAction::SketchOnPlane { theta_deg: 90.0, across_mm: 0.0 };
        let pin = MenuAction::PinHere { world: [0.0, radius + 0.5, 0.0] };
        assert_eq!(tail, [sketch, pin, MenuAction::ClearPins, MenuAction::FitView, MenuAction::OpenCad, MenuAction::ToggleWire, MenuAction::ToggleGrid]);
        assert!(items.iter().all(|i| i.enabled));
        // With nothing under and nothing chosen only the view items remain.
        assert_eq!(context_items(&sel, None, &d).len(), 4);
    }

    #[test]
    fn a_part_offers_its_feature_attachment_stage_and_isolation_with_the_current_ones_ticked() {
        let d = design();
        let sel = Selection::default();
        let under = pick(Entity::Part { feature: 3 }, [0.0, 10.0, 0.0]);
        let items = context_items(&sel, Some(&under), &d);
        assert_eq!(heading(&sel, Some(&under), &d).as_deref(), Some("Bezel"));
        assert_eq!(
            labels(&items),
            [
                (Some("Pattern"), "Array round the ring…".to_string()),
                (Some("Pattern"), "Array round its stone…".into()),
                (Some("Pattern"), "Mirror across the band".into()),
                (Some("Pattern"), "Mirror through the head".into()),
                (None, "Edit feature".into()),
                (Some("Attach"), "Separate".into()),
                (Some("Attach"), "Join".into()),
                (Some("Attach"), "Cut".into()),
                (Some("Stage"), "Cast".into()),
                (Some("Stage"), "Bench".into()),
                (None, "Isolate in CAD".into()),
                (None, "Fit view".into()),
                (None, "Open CAD workspace".into()),
                (None, "Wireframe".into()),
                (None, "Grid".into()),
            ]
        );
        assert_eq!(items[4].action, MenuAction::EditFeature(3));
        let ticked: Vec<_> = items.iter().filter(|i| i.checked).map(|i| i.action.clone()).collect();
        assert_eq!(ticked, [MenuAction::Attach(3, Attach::Join), MenuAction::Stage(3, Stage::Cast)]);
        assert_eq!(items[10].action, MenuAction::IsolateInCad(3));
        assert!(items.iter().all(|i| i.enabled));
        // A reference stone cannot be attached or staged.
        let stone = pick(Entity::Part { feature: 4 }, [0.0, 10.0, 0.0]);
        let items = context_items(&sel, Some(&stone), &d);
        let off: Vec<_> = items.iter().filter(|i| !i.enabled).map(|i| i.label.clone()).collect();
        assert_eq!(off, ["Separate", "Join", "Cut", "Cast", "Bench"]);
        assert!(items.iter().filter(|i| !i.enabled).all(|i| i.hint == "A reference stone is never metal"));
    }

    #[test]
    fn an_edge_leads_with_fillet_and_chamfer_and_a_face_carries_a_heading() {
        let d = design();
        let sel = Selection::default();
        let edge = pick(Entity::Edge { feature: 3, edge: 2 }, [0.0, 10.0, 0.0]);
        let items = context_items(&sel, Some(&edge), &d);
        assert_eq!(heading(&sel, Some(&edge), &d).as_deref(), Some("Edge 2 of Bezel"));
        assert_eq!(items[0].action, MenuAction::FilletEdge { feature: 3, edge: 2 });
        assert_eq!(items[0].icon, Icon::CadFillet);
        assert_eq!(items[1].action, MenuAction::ChamferEdge { feature: 3, edge: 2 });
        assert_eq!(items[2].action, MenuAction::Pattern { feature: 3, key: crate::viewport::patterns::RING_ARRAY });
        assert_eq!(items[6].action, MenuAction::EditFeature(3));
        assert_eq!(items.len(), 17);
        let face = pick(Entity::Face { feature: 3, face: 1 }, [0.0, 10.0, 0.0]);
        let items = context_items(&sel, Some(&face), &d);
        assert_eq!(heading(&sel, Some(&face), &d).as_deref(), Some("Face 1 of Bezel"));
        assert_eq!(items[0].action, MenuAction::SketchOnFace { feature: 3, face: 1 });
        assert_eq!(items[1].action, MenuAction::PressPull { feature: 3, face: 1 });
        assert_eq!(items.len(), 17, "a face has the part's items, a sketch on it, a press-pull and the patterns");
        assert!(!items.iter().any(|i| matches!(i.action, MenuAction::FilletEdge { .. })));
    }

    #[test]
    fn a_stone_offers_its_settings_and_the_last_chosen_thing_stands_in_for_an_empty_right_click() {
        let d = design();
        let mut sel = Selection::default();
        let stone = pick(Entity::Stone { path: vec![0] }, [0.0, 10.0, 0.0]);
        let items = context_items(&sel, Some(&stone), &d);
        let settings = ringdesign_core::cad::builders::SETTINGS.len();
        assert_eq!(items.len(), 4 + settings);
        assert!(items[..settings].iter().all(|i| i.submenu == Some("Setting")));
        assert_eq!(heading(&sel, Some(&stone), &d).as_deref(), Some("Stone"));
        sel.click(Some(Sel::Edge { feature: 3, edge: 0 }), Mods::default());
        let items = context_items(&sel, None, &d);
        assert_eq!(items[0].action, MenuAction::FilletEdge { feature: 3, edge: 0 });
        assert_eq!(heading(&sel, None, &d).as_deref(), Some("Edge 0 of Bezel"));
        sel.click(Some(Sel::BandPoint { theta_deg: 90.0, v_mm: 0.0, world: [0.0, 10.0, 0.0] }), Mods::default());
        assert!(context_items(&sel, None, &d).iter().any(|i| matches!(i.action, MenuAction::AddPartHere { .. })));
        assert_eq!(heading(&sel, None, &d), None);
    }
}
