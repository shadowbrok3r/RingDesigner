//! Shared stock editor and section comparison for desktop and touch.
use egui::{Color32, Stroke, Vec2};
use ringdesign_core::{
    RingDesign,
    imported_base::{ImportedBase, PRESETS, Source},
};

/// One stock in the picker: its table plan drawn to scale beside its name.
fn stock_row(ui: &mut egui::Ui, preset: &ringdesign_core::imported_base::Preset, selected: bool) -> egui::Response {
    let height = 30.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(190.0), height), egui::Sense::click());
    let painter = ui.painter_at(rect);
    let accent = ui.visuals().selection.stroke.color;
    if selected || response.hovered() {
        painter.rect_filled(rect, 3.0, accent.gamma_multiply(if selected { 0.28 } else { 0.12 }));
    }
    // The plan is the table's own radii at even bearings in mm, scaled so the
    // longest is 1 — the aspect is already in it, so a kite reads long.
    let thumb = egui::Rect::from_min_size(rect.min + Vec2::new(4.0, 3.0), Vec2::splat(height - 6.0));
    let n = preset.plan.len().max(1);
    let unit = thumb.width().min(thumb.height()) * 0.5;
    let points: Vec<egui::Pos2> = (0..n)
        .map(|i| {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            let r = preset.plan[i] * unit;
            thumb.center() + Vec2::new(r * a.cos(), -r * a.sin())
        })
        .collect();
    painter.add(egui::Shape::closed_line(points, Stroke::new(1.2, accent)));
    let text = ui.visuals().text_color();
    painter.text(
        egui::pos2(thumb.right() + 8.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        preset.label(),
        egui::TextStyle::Button.resolve(ui.style()),
        text,
    );
    response
}

/// Uses ordinary design history in the host. Rejected numeric edits leave the
/// source untouched and explain which calibrated limit was crossed.
pub fn ui(ui: &mut egui::Ui, d: &mut RingDesign) -> bool {
    let mut changed = false;
    ui.collapsing("Imported signet base",|ui|{
        ui.label("Use finished shoulders as the master, then resize and decorate them.");
        let mut chosen=None;
        let at=d.imported_base.as_ref().map(|b|b.source.name.clone());
        let current=PRESETS.iter().position(|p|at.as_deref()==Some(p.stock_name().as_str()));
        egui::ComboBox::from_id_salt("imported-stock-picker")
            .selected_text(current.map(|i|PRESETS[i].label()).or(at).unwrap_or_else(||"Choose stock…".into()))
            .width(ui.available_width().min(260.0))
            .show_ui(ui,|ui|{
                // Every head is drawn from its own table plan: a number says
                // nothing about the shape it is about to become.
                for (i,p) in PRESETS.iter().enumerate(){
                    if stock_row(ui,p,current==Some(i)).clicked(){chosen=Some(i);ui.close();}
                }
            });
        let key=ui.id().with("base-error");
        let mut error=ui.data(|m|m.get_temp::<String>(key).unwrap_or_default());
        if let Some(i)=chosen {
            match PRESETS[i].load().and_then(|s|ImportedBase::attach(d,s)) {Ok(())=>{changed=true;error.clear();},Err(e)=>error=e.to_string()}
        }
        ui.collapsing("Import a base file",|ui|{
            ui.label("Paste a .ringbase.json master. Units are mm; finger axis Z, face +Y. The CLI can package calibrated OBJ stock.");
            let text_key=ui.id().with("base-text");let mut text=ui.data(|m|m.get_temp::<String>(text_key).unwrap_or_default());
            egui::ScrollArea::vertical().id_salt("imported-base-text").max_height(110.0).show(ui,|ui| { ui.add(egui::TextEdit::multiline(&mut text).desired_rows(3).desired_width(f32::INFINITY)); });
            if ui.button("Import master").clicked(){match Source::from_json(&text).and_then(|s|ImportedBase::attach(d,s)){Ok(())=>{changed=true;text.clear();error.clear();},Err(e)=>error=e.to_string()}}
            ui.data_mut(|m|m.insert_temp(text_key,text));
        });
        if let Some(base)=d.imported_base.clone(){
            let c=&base.source.calibration;
            ui.small(format!("{} triangles · saved master in mm",base.source.faces.len()));
            let mut candidate=d.clone();let mut edited=false;
            let mut bore=d.inner_radius_mm()*2.0;
            for (label,value,range) in [
                ("Bore diameter",&mut bore,(c.bore_radius_mm*2.0-3.0)..=(c.bore_radius_mm*2.0+3.0)),
                ("Face length",&mut candidate.shank.head.length_mm,(c.face_length_mm*0.7)..=(c.face_length_mm*1.3)),
                ("Face width",&mut candidate.profile.width_mm,(c.face_width_mm*0.7)..=(c.face_width_mm*1.3)),
                ("Palm thickness",&mut candidate.profile.thickness_mm,(c.palm_thickness_mm-0.5).max(0.8)..=(c.palm_thickness_mm+0.5)),
            ] {edited|=ui.add(egui::Slider::new(value,range).text(label).suffix(" mm").max_decimals(2)).changed();}
            candidate.size=ringdesign_core::RingSize((bore*std::f64::consts::PI-36.5)/2.55);
            let mut height=candidate.profile.thickness_mm+candidate.shank.head.rise_mm;
            edited|=ui.add(egui::Slider::new(&mut height,(c.head_height_mm-1.5)..=(c.head_height_mm+1.5)).text("Head height").suffix(" mm")).changed();
            candidate.shank.head.rise_mm=height-candidate.profile.thickness_mm;
            edited|=ui.checkbox(&mut candidate.imported_base.as_mut().unwrap().bare,"Base only (preview and export)").changed();
            edited|=ui.checkbox(&mut candidate.imported_base.as_mut().unwrap().sand_envelope,"Support relief for sand withdrawal")
                .on_hover_text("Adds metal toward the Z=0 parting line to support relief. Layers remain editable. Check Casting after changing the stock or ornament.").changed();
            if edited {match base.validate_design(&candidate){Ok(())=>{candidate.graph=None;*d=candidate;changed=true;error.clear();},Err(e)=>error=e.to_string()}}
            ui.horizontal_wrapped(|ui|{
                if ui.button("Reset master dimensions").clicked(){ImportedBase::reset(d);d.graph=None;changed=true;error.clear();}
                if ui.button("Validate deformation").clicked(){error=match base.validate_shape(d){Ok(())=>"Master topology and sampled deformation checks passed. Use casting checks for wall thickness and undercuts.".into(),Err(e)=>e.to_string()};}
            });
            ui.collapsing("Compare sections",|ui|{
                let angle_key=ui.id().with("section-angle");let mut angle=ui.data(|m|m.get_temp::<f64>(angle_key).unwrap_or(65.0));
                ui.add(egui::Slider::new(&mut angle,0.0..=360.0).text("Angle").suffix("°"));ui.data_mut(|m|m.insert_temp(angle_key,angle));
                let mut master=d.clone();ImportedBase::reset(&mut master);
                let a=master.section_at(angle,256,None,None);let b=d.section_at(angle,256,None,None);
                let (rect,_)=ui.allocate_exact_size(Vec2::new(ui.available_width(),180.0),egui::Sense::hover());
                let all=a.pts.iter().chain(&b.pts);let (mut min,mut max)=([f64::INFINITY;2],[f64::NEG_INFINITY;2]);for p in all {min[0]=min[0].min(p.z);min[1]=min[1].min(p.r);max[0]=max[0].max(p.z);max[1]=max[1].max(p.r);}
                let scale=((rect.width()-12.0) as f64/(max[0]-min[0]).max(1.0)).min((rect.height()-12.0) as f64/(max[1]-min[1]).max(1.0));
                for (section,color) in [(&a,Color32::GRAY),(&b,Color32::from_rgb(65,205,181))]{let mut points:Vec<_>=section.pts.iter().map(|p|egui::pos2(rect.center().x+((p.z-(min[0]+max[0])*0.5)*scale) as f32,rect.center().y-((p.r-(min[1]+max[1])*0.5)*scale) as f32)).collect();if let Some(first)=points.first().copied(){points.push(first);}ui.painter().add(egui::Shape::line(points,Stroke::new(1.5,color)));}
                ui.small("Grey: original · teal: resized. Bore at the bottom; shoulder and face above.");
            });
            if ui.button("Return to procedural base").clicked(){d.imported_base=None;d.graph=None;changed=true;}
        }
        if !error.is_empty(){ui.label(&error);}
        ui.data_mut(|m|m.insert_temp(key,error));
    });
    changed
}
