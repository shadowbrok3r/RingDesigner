//! Direct viewport tools: shared controls, cached inspection, and source edits.
mod canvas;
pub mod measure;
mod path;
mod transform;
pub use canvas::{Edit, Pointer};
use ringdesign_core::{
    AlphaLibrary, RingDesign,
    interaction::{
        clearance::Envelope,
        mould::Study,
        paint::{Brush, Gesture},
        section::{Cut, Plane},
    },
};
use std::sync::{Arc, mpsc};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum Tool {
    #[default]
    Select,
    Paint,
    Stamp,
    Section,
    Clearance,
    Mould,
    Path,
    Measure,
    Transform,
}
impl Tool {
    pub const ALL: [Self; 9] = [
        Self::Select,
        Self::Paint,
        Self::Stamp,
        Self::Section,
        Self::Clearance,
        Self::Mould,
        Self::Path,
        Self::Measure,
        Self::Transform,
    ];
    pub fn icon(self) -> crate::icons::Icon {
        use crate::icons::Icon;
        match self {
            Self::Transform => Icon::Transform,
            Self::Select => Icon::Select,
            Self::Paint => Icon::Paint,
            Self::Stamp => Icon::Stamp,
            Self::Path => Icon::Path,
            Self::Section => Icon::Section,
            Self::Measure => Icon::Measure,
            Self::Clearance => Icon::Spacing,
            Self::Mould => Icon::Mould,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Transform => "Move ornament",
            Self::Select => "Select",
            Self::Paint => "Paint 3D",
            Self::Stamp => "Stamp",
            Self::Section => "Section",
            Self::Clearance => "Clearance",
            Self::Mould => "Mould opening",
            Self::Path => "Surface path",
            Self::Measure => "Measure",
        }
    }
}

pub struct Visual {
    pub tool: Tool,
    pub path: path::PathTool,
    pub transform: transform::TransformTool,
    pub arrangement: ringdesign_core::interaction::surface::Arrangement,
    pub(super) measure: Vec<[f64; 3]>,
    /// The Ring viewport's measurement between picks.
    pub measurement: measure::Measure,
    pub brush: Brush,
    pub plane: Plane,
    pub gap_mm: f64,
    pub stylus_only: bool,
    pub gesture: Gesture,
    stamp_contact: canvas::StampContact,
    contact: canvas::Contact,
    pub cursor: Option<ringdesign_core::interaction::picking::Hit>,
    pub selected_stone: Option<usize>,
    pub study: Option<Arc<Study>>,
    pub study_serial: u64,
    pub opening_mm: f64,
    pub playing: bool,
    pub show_upper: bool,
    pub show_lower: bool,
    pub message: String,
    pub busy: bool,
    receiver: Option<mpsc::Receiver<Result<Study, String>>>,
    pub(super) cut: Cut,
    pub(super) cap: Vec<[[f64; 3]; 3]>,
    pub(super) cut_key: Option<(usize, u64)>,
    pub(super) wall: Option<[[f64; 3]; 2]>,
    pub(super) envelopes: Vec<Envelope>,
    pub(super) crowding: Option<ringdesign_core::stones::StonesReport>,
    pub(super) clearance_gap: Option<u64>,
    pub(super) phase: f64,
    pub(super) section_dragging: bool,
    pub(super) section_grab: Option<(egui::Pos2, f64)>,
    pub(super) placement_extent: egui::Rect,
}
impl Default for Visual {
    fn default() -> Self {
        Self {
            tool: Tool::Select,
            path: Default::default(),
            transform: Default::default(),
            arrangement: Default::default(),
            measure: Vec::new(),
            measurement: Default::default(),
            brush: Brush::default(),
            plane: Plane::default(),
            gap_mm: 0.4,
            stylus_only: false,
            gesture: Gesture::default(),
            stamp_contact: Default::default(),
            contact: Default::default(),
            cursor: None,
            selected_stone: None,
            study: None,
            study_serial: 0,
            opening_mm: 7.0,
            playing: false,
            show_upper: true,
            show_lower: true,
            message: String::new(),
            busy: false,
            receiver: None,
            cut: Cut::default(),
            cap: Vec::new(),
            cut_key: None,
            wall: None,
            envelopes: Vec::new(),
            crowding: None,
            clearance_gap: None,
            phase: 0.0,
            section_dragging: false,
            section_grab: None,
            placement_extent: egui::Rect::NOTHING,
        }
    }
}
impl Visual {
    pub fn select(&mut self, tool: Tool) {
        if tool != self.tool {
            self.path.stop_drag();
            self.transform.stop_drag();
            self.section_dragging = false;
            self.section_grab = None;
            self.gesture = Gesture::default();
            self.stamp_contact = Default::default();
            self.contact = Default::default();
            self.cursor = None;
            if tool == Tool::Stamp && self.brush.diameter_mm < 1.5 {
                self.brush.diameter_mm = 2.8;
            }
            if tool == Tool::Paint && self.brush.diameter_mm > 1.5 {
                self.brush.diameter_mm = 0.65;
            }
            if tool != Tool::Mould {
                self.playing = false;
            }
        }
        self.tool = tool;
    }
    /// A real source edit invalidates inspection; a camera or tool change does not.
    pub fn invalidate(&mut self) {
        self.stamp_contact = Default::default();
        self.path.invalidate();
        self.measure.clear();
        self.measurement.clear();
        self.study = None;
        self.receiver = None;
        self.busy = false;
        self.playing = false;
        self.cut_key = None;
        self.wall = None;
        self.clearance_gap = None;
        self.cursor = None;
    }
    pub fn mesh_changed(&mut self) {
        self.cut_key = None;
        self.wall = None;
    }
    pub fn clip(&self) -> [f32; 4] {
        if self.tool == Tool::Section {
            self.plane.uniform()
        } else {
            [0.0; 4]
        }
    }
    pub fn is_painting(&self) -> bool {
        matches!(
            self.tool,
            Tool::Paint | Tool::Stamp | Tool::Path | Tool::Transform
        )
    }
    pub fn wants_repaint(&self) -> bool {
        self.busy || self.playing
    }
    pub fn poll(&mut self) -> bool {
        let Some(rx) = &self.receiver else {
            return false;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.receiver = None;
                self.busy = false;
                match result {
                    Ok(study) => {
                        self.message =
                            format!("{} obstruction regions", study.report.obstructions.len());
                        self.study = Some(Arc::new(study));
                        self.study_serial += 1;
                        true
                    }
                    Err(error) => {
                        self.message = error;
                        false
                    }
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => {
                self.receiver = None;
                self.busy = false;
                self.message = "The mould study stopped; build it again.".into();
                false
            }
            Err(mpsc::TryRecvError::Empty) => false,
        }
    }
    fn build_mould(&mut self, d: &RingDesign, lib: &AlphaLibrary) {
        if self.busy {
            return;
        }
        self.message = "Preparing the pattern and sampling its cavities…".into();
        self.busy = true;
        self.playing = false;
        let d = d.clone();
        let lib = lib.clone();
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result =
                ringdesign_core::interaction::mould::build(&d, &lib).map_err(|e| e.to_string());
            let _ = tx.send(result);
        });
        #[cfg(target_arch = "wasm32")]
        {
            let _ = tx.send(
                ringdesign_core::interaction::mould::build(&d, &lib).map_err(|e| e.to_string()),
            );
        }
    }
    pub fn chooser(&mut self, ui: &mut egui::Ui, choices: &[Tool]) {
        ui.horizontal_wrapped(|ui| {
            for &tool in choices {
                if crate::icons::button(
                    ui,
                    tool.icon(),
                    tool.label(),
                    self.tool == tool,
                    egui::vec2(0.0, 26.0),
                )
                .clicked()
                {
                    self.select(tool);
                }
            }
        });
    }
    pub fn controls(&mut self, ui: &mut egui::Ui, d: &RingDesign, lib: &AlphaLibrary) {
        match self.tool {
            Tool::Transform => self.transform.controls(ui, d),
            Tool::Path => self.path.controls(ui, d),
            Tool::Measure if !self.measurement.picks.is_empty() => {
                ui.label("A vertex, an edge, a face, a stone or a pin measures as itself; a band point snaps to the ring's features, Ctrl frees it. Shift adds a third pick and its angle. Esc clears.");
                for p in &self.measurement.picks {
                    ui.small(p.label());
                }
                for r in self.measurement.readings() {
                    ui.colored_label(egui::Color32::from_rgb(43, 226, 214), r.line());
                }
                if ui.button("Clear measurement").clicked() {
                    self.measurement.clear();
                }
            }
            Tool::Measure => {
                ui.label("Tap two points on the ring to measure their straight-line distance. Tap again to start a new measurement.");
                if let [a, b] = self.measure.as_slice() {
                    ui.colored_label(
                        egui::Color32::from_rgb(43, 226, 214),
                        format!(
                            "Distance {:.3} mm",
                            ringdesign_core::interaction::section::distance(*a, *b)
                        ),
                    );
                    ui.small(format!(
                        "X {:.3}   Y {:.3}   Z {:.3} mm",
                        (b[0] - a[0]).abs(),
                        (b[1] - a[1]).abs(),
                        (b[2] - a[2]).abs()
                    ));
                }
                if ui.button("Clear measurement").clicked() {
                    self.measure.clear();
                }
                ui.small("Measures the displayed mesh; not surface arc length or minimum wall thickness.");
            }
            Tool::Select => {}
            Tool::Paint | Tool::Stamp => {
                if d.graph.is_some() {
                    ui.label("Use an editable procedural ring for surface artwork.");
                    return;
                }
                if crate::cad_tools::replaces_band(d) {
                    ui.label(crate::cad_tools::PARTS_ONLY);
                    return;
                }
                if self.tool == Tool::Stamp {
                    if self.brush.stamp.is_empty() {
                        self.brush.stamp = lib.names().into_iter().next().unwrap_or_default();
                    }
                    crate::artwork::picker(ui, "surface-stamp-alpha", &mut self.brush.stamp, lib);
                }
                relief_direction(ui, &mut self.brush.engrave);
                value(ui, "Size", &mut self.brush.diameter_mm, 0.1..=12.0, " mm");
                value(ui, "Depth", &mut self.brush.depth_mm, 0.01..=1.6, " mm");
                if self.tool == Tool::Stamp {
                    value(
                        ui,
                        "Rotation",
                        &mut self.brush.rotation_deg,
                        -180.0..=180.0,
                        "°",
                    );
                    ui.collapsing("Repeat & input", |ui| {
                        ui.horizontal(|ui| {
                            ui.label("Copies");
                            ui.add(egui::DragValue::new(&mut self.arrangement.count).range(
                                1..=ringdesign_core::interaction::surface::MAX_STAMP_COPIES,
                            ));
                        });
                        ui.checkbox(&mut self.arrangement.mirror, "Mirror sides");
                        if self.arrangement.count > 1 {
                            value(
                                ui,
                                "Spread",
                                &mut self.arrangement.span_deg,
                                1.0..=360.0,
                                "°",
                            );
                        }
                        ui.checkbox(&mut self.stylus_only, "Pen only");
                    });
                    ui.small("Slide on the ring; lift to stamp. Empty space turns the view.");
                } else {
                    ui.checkbox(&mut self.stylus_only, "Pen only");
                    ui.small("Draw on the ring; lift to build. Empty space turns the view.");
                }
                if d.draft.process == ringdesign_core::castability::CastProcess::SandTwoPart {
                    ui.small("Depth follows the local sand-casting limit.");
                }
            }
            Tool::Section => {
                ui.horizontal_wrapped(|ui| {
                    for (axis, label) in ["X", "Y", "Z"].into_iter().enumerate() {
                        if ui
                            .selectable_label(self.plane.axis == axis, label)
                            .clicked()
                        {
                            self.plane.axis = axis;
                            self.plane.offset = 0.0;
                            self.cut_key = None;
                            self.wall = None;
                        }
                    }
                    ui.checkbox(&mut self.plane.flip, "Other half");
                });
                value(
                    ui,
                    "Cut offset",
                    &mut self.plane.offset,
                    -60.0..=60.0,
                    " mm",
                );
                ui.small("Drag the aqua handle. Tap a cut edge to measure a local wall chord.");
                if let Some([a, b]) = self.wall {
                    ui.colored_label(
                        egui::Color32::from_rgb(43, 226, 214),
                        format!(
                            "Section wall {:.2} mm",
                            ringdesign_core::interaction::section::distance(a, b)
                        ),
                    );
                }
                ui.small(format!(
                    "Cut boundary {:.2} mm · mesh measurement",
                    self.cut.length_mm
                ));
            }
            Tool::Clearance => {
                value(ui, "Requested gap", &mut self.gap_mm, 0.0..=3.0, " mm");
                self.refresh_clearance(d);
                if let Some(r) = &self.crowding {
                    if let Some(pair) = &r.closest {
                        ui.label(format!(
                            "Closest: {:.2} mm girdle / {:.2} mm at depth",
                            pair.gap_mm, pair.gap_deep_mm
                        ));
                    }
                    ui.small(format!(
                        "{} stones · {} pairs below the recipe's bench threshold",
                        r.stone_count, r.tight_pairs
                    ));
                } else {
                    ui.label("Add a stone setting to show its envelope.");
                }
                ui.small("Each envelope reserves half the gap. Pavilion outlines are conservative; report values use the stone census.");
            }
            Tool::Mould => {
                if ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new(if self.study.is_some() {
                            "Rebuild mould study"
                        } else {
                            "Build mould study"
                        }),
                    )
                    .clicked()
                {
                    self.build_mould(d, lib);
                }
                if !self.message.is_empty() {
                    ui.label(&self.message);
                }
                if let Some(study) = self.study.clone() {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(if self.playing {
                                "Pause"
                            } else {
                                "Play opening"
                            })
                            .clicked()
                        {
                            self.playing = !self.playing;
                        }
                        ui.checkbox(&mut self.show_upper, "Upper");
                        ui.checkbox(&mut self.show_lower, "Lower");
                    });
                    ui.label(format!("Opening {:.2} mm per half", self.opening_mm));
                    ui.spacing_mut().slider_width = (ui.available_width() - 8.0).max(60.0);
                    if ui
                        .add(egui::Slider::new(&mut self.opening_mm, 0.0..=16.0).show_value(false))
                        .changed()
                    {
                        self.playing = false;
                    }
                    let r = &study.report;
                    ui.small(format!(
                        "Prepared pattern ×{:.4} · pull [{:.2}, {:.2}, {:.2}]",
                        study.scale, r.frame.z[0], r.frame.z[1], r.frame.z[2]
                    ));
                    ui.small(format!(
                        "Parting {:.2} mm · {} × {} samples",
                        r.parting_mm, r.grid[0], r.grid[1]
                    ));
                    if study.investment {
                        ui.colored_label(egui::Color32::from_rgb(239,179,104),"Investment pattern: this pull study illustrates geometry; the mould is expendable.");
                    }
                    ui.small("Translucent sampled cavity surfaces. Red markers locate trapped regions; review repairs in Workshop.");
                }
            }
        }
    }
    pub fn apply_controls(&mut self, d: &mut RingDesign) -> Option<usize> {
        match self.tool {
            Tool::Path => self.path.apply_pending(d),
            Tool::Transform => self.transform.apply_pending(d),
            _ => None,
        }
    }
    fn refresh_clearance(&mut self, d: &RingDesign) {
        if self.clearance_gap != Some(self.gap_mm.to_bits()) {
            self.envelopes = ringdesign_core::interaction::clearance::envelopes(d, self.gap_mm);
            self.crowding = ringdesign_core::stones::report(d, d.draft.parting_z_mm);
            self.clearance_gap = Some(self.gap_mm.to_bits());
        }
    }
    pub fn stone_controls(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) -> bool {
        use ringdesign_core::{field::Layer, interaction::picking};
        let Some(index) = self.selected_stone else {
            ui.small("Tap an envelope to edit its stone.");
            return false;
        };
        let paths = picking::stone_paths(d);
        let Some(path) = paths.get(index) else {
            return false;
        };
        if d.graph.is_some() {
            ui.small("Bake the recipe graph before individual stone edits.");
            return false;
        }
        if picking::live_ancestor(&d.layers, path).is_some() {
            ui.small("This stone belongs to a live group; edit that group's recipe in Layers.");
            return false;
        }
        let max_v = d.field_context().band_v_len_mm;
        let Some(entry) = picking::entry_mut(&mut d.layers, path) else {
            return false;
        };
        ui.separator();
        ui.label(&entry.name);
        let Layer::SeatPad(seat) = &mut entry.layer else {
            ui.small("This is a repeated setting. Edit its shared run in Layers.");
            return false;
        };
        let Some(mut gem) = seat.gem else {
            return false;
        };
        let mut width = gem.w_mm;
        let mut angle = seat.theta_deg;
        let mut across = seat.v_mm;
        value(ui, "Stone width", &mut width, 0.5..=18.0, " mm");
        value(ui, "Around ring", &mut angle, 0.0..=360.0, "°");
        value(ui, "Across band", &mut across, 0.0..=max_v, " mm");
        let mut tint = gem.preview_tint.unwrap_or(ringdesign_core::gems::GEM_TINT);
        let tint_changed = ui.horizontal(|ui| {
            ui.label("Stone colour");
            let picked = ui.color_edit_button_rgb(&mut tint).changed();
            let reset = ui.small_button("Neutral").clicked();
            if reset { gem.preview_tint = None; }
            else if picked { gem.preview_tint = Some(tint); }
            picked || reset
        }).inner;
        let geometry_changed = width != gem.w_mm || angle != seat.theta_deg || across != seat.v_mm;
        if geometry_changed {
            gem.l_mm *= width / gem.w_mm.max(0.01);
            gem.w_mm = width;
            seat.fit_stone(gem);
            seat.theta_deg = angle;
            seat.v_mm = across;
        } else if tint_changed {
            // A colour edit must preserve hand-adjusted stock and bearing sizes.
            seat.gem = Some(gem);
        }
        geometry_changed || tint_changed
    }
}
fn relief_direction(ui: &mut egui::Ui, engrave: &mut bool) {
    ui.horizontal(|ui| {
        for (icon, label, value) in [
            (crate::icons::Icon::Raise, "Raise", false),
            (crate::icons::Icon::Engrave, "Cut", true),
        ] {
            if crate::icons::button(ui, icon, label, *engrave == value, egui::vec2(0.0, 28.0))
                .clicked()
            {
                *engrave = value;
            }
        }
    });
}
fn value(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
) {
    ui.scope_builder(
        egui::UiBuilder::new().id(ui.id().with(("viewport-value", label))),
        |ui| {
            ui.horizontal(|ui| {
                let width = ui.available_width();
                ui.add_sized(
                    [(width - 88.0).max(44.0), 28.0],
                    egui::Label::new(label).truncate(),
                )
                .on_hover_text(label);
                let response = ui.add_sized(
                    [80.0, 28.0],
                    egui::DragValue::new(v)
                        .range(range)
                        .clamp_existing_to_range(false)
                        .speed(0.02)
                        .fixed_decimals(2)
                        .suffix(suffix),
                );
                if response.has_focus() {
                    response.scroll_to_me(Some(egui::Align::Center));
                }
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_viewport_inspector_fits_a_phone_and_preserves_source() {
        let mut d = RingDesign::default();
        d.profile.width_mm = 25.0;
        let mut lib = AlphaLibrary::builtin();
        lib.insert(ringdesign_core::Alpha::new(
            "A very long imported ornament name for a portable workshop alpha",
            8,
            8,
            vec![1.0; 64],
        ));
        let study = Arc::new(ringdesign_core::interaction::mould::build(&d, &lib).unwrap());
        let source = serde_json::to_vec(&d).unwrap();
        for width in [168.0, 180.0, 248.0, 280.0, 320.0, 411.0] {
            for tool in Tool::ALL {
                if width < 248.0 && !matches!(tool, Tool::Paint | Tool::Stamp | Tool::Path | Tool::Transform) { continue; }
                let ctx = egui::Context::default();
                ctx.style_mut_of(egui::Theme::Dark, |s| {
                    s.spacing.interact_size = egui::vec2(40.0, 40.0)
                });
                ctx.style_mut_of(egui::Theme::Light, |s| {
                    s.spacing.interact_size = egui::vec2(40.0, 40.0)
                });
                let mut v = Visual::default();
                v.tool = tool;
                v.study = Some(study.clone());
                v.brush.stamp =
                    "A very long imported ornament name for a portable workshop alpha".into();
                let mut right = 0.0;
                for _ in 0..2 {
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 800.0),
                            )),
                            ..Default::default()
                        },
                        |root| {
                            egui::CentralPanel::default().show(root, |ui| {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    v.controls(ui, &d, &lib);
                                    right = ui.min_rect().right();
                                });
                            });
                        },
                    );
                    output.textures_delta.clear();
                }
                assert!(
                    right <= width + 0.1,
                    "{tool:?} overflows {width}: right={right}"
                );
                assert_eq!(serde_json::to_vec(&d).unwrap(), source);
            }
        }
    }
}

#[cfg(test)]
mod compact_tests {
    use super::*;
    #[test]
    fn labeled_icon_toolbar_wraps_buttons_instead_of_individual_letters() {
        let ctx = egui::Context::default();
        let mut visual = Visual::default();
        let mut height = 0.0;
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(280.0, 640.0),
                )),
                ..Default::default()
            },
            |root| {
                egui::CentralPanel::default().show(root, |ui| {
                    let top = ui.cursor().top();
                    visual.chooser(
                        ui,
                        &[
                            Tool::Select,
                            Tool::Paint,
                            Tool::Stamp,
                            Tool::Path,
                            Tool::Transform,
                        ],
                    );
                    height = ui.cursor().top() - top;
                });
            },
        );
        output.textures_delta.clear();
        assert!(
            height < 125.0,
            "tool buttons wrapped into vertical text: {height}"
        );
    }
}
