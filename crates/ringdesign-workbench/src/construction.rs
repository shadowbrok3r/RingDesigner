//! Compact construction guide shared by the native desktop and phone editors.
use ringdesign_core::{AlphaLibrary, RingDesign, construction as recipe};

#[derive(Clone, Copy)]
pub enum View { Seal, ThreeQuarter, Cheek, Bore }
#[derive(Default)]
pub struct Event { pub changed: bool, pub view: Option<View> }
pub struct Guide {
    pub open: bool,
    started: bool,
    step: usize,
    strength: f64,
    error: Option<String>,
    preview: Option<(String, egui::TextureHandle)>,
    art: Option<AlphaLibrary>,
}
impl Default for Guide {
    fn default() -> Self {
        Self { open: false, started: false, step: 0, strength: 1., error: None, preview: None, art: None }
    }
}
impl Guide {
    pub fn ui(&mut self, ui: &mut egui::Ui, d: &mut RingDesign) -> Event {
        let mut event = Event::default();
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
        ui.spacing_mut().interact_size.y=24.;
        ui.spacing_mut().item_spacing=egui::vec2(4.,3.);
        ui.spacing_mut().button_padding=egui::vec2(6.,3.);
        ui.label(egui::RichText::new("ASTER ATELIER / SAND SIGNET").strong());
        if !self.started {
            let completed = (0..recipe::STEPS.len()).filter(|&i| recipe::present(d, i)).count();
            if d.name == recipe::source().name && completed > 0 {
                if ui.add_sized([ui.available_width(), 40.], egui::Button::new("Continue this design")).clicked() {
                    self.started = true;
                    self.step = (0..recipe::STEPS.len()).find(|&i| !recipe::present(d, i)).unwrap_or(recipe::STEPS.len()-1);
                    self.strength = 1.;
                }
            }
            ui.label("Build a botanical signet from a blank band. Apply one operation at a time; every layer remains editable.");
            ui.label("Solid metal · broad ornament · two-part mould");
            if ui.add_sized([ui.available_width(), 40.], egui::Button::new("Start from a blank band")).clicked() {
                *d = recipe::blank();
                self.started = true;
                self.step = 0;
                event.changed = true;
                event.view = Some(View::ThreeQuarter);
            }
            return event;
        }
        let count = recipe::STEPS.len();
        let completed = (0..count).filter(|&i| recipe::present(d, i)).count();
        ui.add(egui::ProgressBar::new(completed as f32 / count as f32).desired_height(16.).text(format!("{completed} / {count} operations in this design")));
        ui.horizontal_wrapped(|ui| {
            for (label, view) in [("Seal",View::Seal),("3/4",View::ThreeQuarter),("Cheek",View::Cheek),("Bore",View::Bore)] {
                if ui.add_sized([54.,32.],egui::Button::new(label)).clicked() { event.view=Some(view); }
            }
        });
        // The step list remains usable after undo or an ordinary editor change.
        // Completion is read from the document, never claimed by a counter.
        self.step = self.step.min(count - 1);
        let step = &recipe::STEPS[self.step];
        egui::ComboBox::from_id_salt("construction-step").width(ui.available_width() - 8.)
            .selected_text(format!("{:02}  {}",self.step+1,step.title)).show_ui(ui, |ui| {
                for (i,s) in recipe::STEPS.iter().enumerate() {
                    let mark=if recipe::present(d,i) { "✓" } else { "·" };
                    if ui.selectable_value(&mut self.step,i,format!("{mark} {:02} {}",i+1,s.title)).changed() { self.strength=1.; }
                }
            });
        let step = &recipe::STEPS[self.step];
        ui.label(egui::RichText::new(step.tool).strong().color(ui.visuals().hyperlink_color));
        let alpha = step.layers.first().and_then(|name| recipe::source().layers.layers.iter().find(|e| e.name == *name))
            .and_then(|e| match &e.layer {
                ringdesign_core::field::Layer::Decals(l) => Some(l.alpha.as_str()),
                ringdesign_core::field::Layer::Tiling(l) => Some(l.alpha.as_str()),
                _ => None,
            });
        if let Some(name) = alpha {
            if self.preview.as_ref().is_none_or(|(n,_)| n != name) {
                let art = self.art.get_or_insert_with(|| {
                    let mut lib=AlphaLibrary::default();
                    recipe::source().unpack_embedded(&mut lib);
                    recipe::source().bake_all(&mut lib);
                    lib
                });
                self.preview=art.get(name).map(|a| {
                    let (w,h,pixels)=a.thumbnail_rgba8(100);
                    (name.to_owned(),ui.ctx().load_texture(format!("guide-{name}"),egui::ColorImage::from_rgba_unmultiplied([w,h],&pixels),egui::TextureOptions::LINEAR))
                });
            }
            ui.horizontal(|ui| {
                if let Some((_,tex))=&self.preview { ui.image((tex.id(),egui::vec2(60.,60.))); }
                ui.label(step.detail);
            });
        } else { ui.label(step.detail); }
        if !step.layers.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Relief");
                ui.spacing_mut().slider_width=(ui.available_width()-76.).max(50.);
                ui.add(egui::Slider::new(&mut self.strength,0.1..=1.).custom_formatter(|v,_|format!("{:.0}%",v*100.)));
            });
        }
        let label=if recipe::present(d,self.step) { "Apply settings" } else { "Apply operation" };
        let apply = ui.horizontal(|ui| {
            let width=ui.available_width();
            let apply=ui.add_sized([width*0.62,40.],egui::Button::new(label)).clicked();
            if ui.add_enabled(self.step+1<count,egui::Button::new("Next").min_size(egui::vec2(width*0.32,40.))).clicked() {
                self.step=(self.step+1).min(count-1);
                self.strength=1.;
            }
            apply
        }).inner;
        if apply {
            match recipe::apply(d,self.step,self.strength) {
                Ok(()) => {
                    event.changed=true;
                    self.error=None;
                },
                Err(e) => self.error=Some(e.to_string()),
            }
        }
        if let Some(error)=&self.error { ui.colored_label(egui::Color32::LIGHT_RED,error); }
        if completed==count { ui.label("Construction complete. Inspect the actual pattern with Mould; all layers remain editable."); }
        event
    }
}
