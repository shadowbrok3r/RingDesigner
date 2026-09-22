//! One metadata-only template library for desktop and Android menus.
//! Ring data is parsed only after a choice; thumbnails are small bundled renders.
use std::sync::LazyLock;
use egui::{TextureHandle, Ui};
use ringdesign_core::{AlphaLibrary, RingDesign};
use ringdesign_graph::{registry::Registry, templates::TemplateGraph};

pub struct Template {
    pub name: &'static str,
    pub slug: &'static str,
    pub description: &'static str,
    source: Source,
}
/// `Design` names a bundled `.ring.json`; the document is decompressed only
/// when that template is chosen, not to list it in a menu.
enum Source { Graph(&'static TemplateGraph), Starter(&'static ringdesign_core::templates::Template), Design(&'static str) }
impl Template {
    pub fn instantiate(&self, reg: &Registry, lib: &AlphaLibrary) -> anyhow::Result<RingDesign> {
        match &self.source {
            Source::Graph(graph) => Ok(graph.instantiate(reg, lib)?),
            Source::Starter(template) => Ok(template.design()),
            Source::Design(slug) => {
                let asset = ringdesign_assets::find(ringdesign_assets::DESIGNS, slug).expect("bundled design");
                Ok(ringdesign_graph::templates::refine_sources(&serde_json::from_str(&asset.text())?))
            }
        }
    }
}
pub struct Collection { pub name: &'static str, pub templates: Vec<Template> }

pub fn collections() -> &'static [Collection] {
    static CATALOG: LazyLock<Vec<Collection>> = LazyLock::new(|| {
        let group = |name, slugs: &[&str], description| Collection {
            name,
            templates: slugs.iter().map(|slug| {
                let t = ringdesign_graph::templates::catalog().find(|t| t.slug == *slug).expect("catalogued graph");
                let source = ringdesign_core::templates::all().iter().find(|starter| starter.name == t.name)
                    .map(Source::Starter).unwrap_or(Source::Graph(t));
                Template { name: t.name, slug: t.slug, description, source }
            }).collect(),
        };
        vec![
            group("Reptilia collection", &["ecdysis-reptilia", "tessera-reptilia", "lorica-reptilia", "ophidian-reptilia", "varanus-reptilia"], "Sculpted reptile skins with editable artwork and geometry."),
            group("Stock masterworks", &["nocturne-imported", "solstice-imported", "aurelia-imported", "vesper-imported", "saurian-imported", "zenith-imported", "caiman-imported"], "Authored ornament on calibrated imported signet stock."),
            Collection { name: "Workshop collection", templates: vec![
                Template { name: "Aster — cushion seal", slug: "aster-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", source: Source::Design("aster-workshop") },
                Template { name: "Tide — twelve reeds", slug: "tide-workshop", description: "Editable workshop design with a nominal 18.2 mm bore.", source: Source::Design("tide-workshop") },
                Template { name: "Lantern — pierced octagonal signet", slug: "lantern-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", source: Source::Design("lantern-workshop") },
                Template { name: "Aureole — half-turn ribbon", slug: "aureole-workshop", description: "Editable CAD assembly; use the CAD workspace for its feature history.", source: Source::Design("aureole-workshop") },
            ] },
            {
                let mut atelier = group("Atelier designs", &["aster-atelier", "thalassa", "oriel"], "Complete authored designs with their artwork and settings.");
                atelier.templates.push(Template { name: "Aster — original botanical signet", slug: "aster-botanical", description: "The original botanical sand signet.", source: Source::Design("aster-botanical") });
                atelier
            },
            group("Original masterwork signets", &["nocturne", "solstice"], "Original sculpted designs, before the stock-based editions."),
            group("Starter bands", &["court-band", "braided-band", "wishbone-wave", "split-shank"], "A simple parametric starting point for a band."),
            group("Starter signets", &["heart-signet", "waved-hexagon-signet", "shouldered-cushion-signet"], "A parametric signet with an editable head and shank."),
            group("Stone settings", &["cathedral-solitaire-stock", "toi-et-moi"], "Starter rings with editable stone settings."),
        ]
    });
    &CATALOG
}

pub fn preview_bytes(slug: &str) -> Option<&'static [u8]> {
    Some(match slug {
        "court-band" => include_bytes!("../assets/templates/court-band.png"),
        "heart-signet" => include_bytes!("../assets/templates/heart-signet.png"),
        "waved-hexagon-signet" => include_bytes!("../assets/templates/waved-hexagon-signet.png"),
        "shouldered-cushion-signet" => include_bytes!("../assets/templates/shouldered-cushion-signet.png"),
        "braided-band" => include_bytes!("../assets/templates/braided-band.png"),
        "cathedral-solitaire-stock" => include_bytes!("../assets/templates/cathedral-solitaire-stock.png"),
        "wishbone-wave" => include_bytes!("../assets/templates/wishbone-wave.png"),
        "split-shank" => include_bytes!("../assets/templates/split-shank.png"),
        "toi-et-moi" => include_bytes!("../assets/templates/toi-et-moi.png"),
        "nocturne" => include_bytes!("../assets/templates/nocturne.png"),
        "solstice" => include_bytes!("../assets/templates/solstice.png"),
        "aster-atelier" => include_bytes!("../assets/templates/aster-atelier.png"),
        "thalassa" => include_bytes!("../assets/templates/thalassa.png"),
        "oriel" => include_bytes!("../assets/templates/oriel.png"),
        "nocturne-imported" => include_bytes!("../assets/templates/nocturne-imported.png"),
        "solstice-imported" => include_bytes!("../assets/templates/solstice-imported.png"),
        "aurelia-imported" => include_bytes!("../assets/templates/aurelia-imported.png"),
        "vesper-imported" => include_bytes!("../assets/templates/vesper-imported.png"),
        "saurian-imported" => include_bytes!("../assets/templates/saurian-imported.png"),
        "zenith-imported" => include_bytes!("../assets/templates/zenith-imported.png"),
        "caiman-imported" => include_bytes!("../assets/templates/caiman-imported.png"),
        "ecdysis-reptilia" => include_bytes!("../assets/templates/ecdysis-reptilia.png"),
        "tessera-reptilia" => include_bytes!("../assets/templates/tessera-reptilia.png"),
        "lorica-reptilia" => include_bytes!("../assets/templates/lorica-reptilia.png"),
        "ophidian-reptilia" => include_bytes!("../assets/templates/ophidian-reptilia.png"),
        "varanus-reptilia" => include_bytes!("../assets/templates/varanus-reptilia.png"),
        "aster-botanical" => include_bytes!("../assets/templates/aster-botanical.png"),
        "aster-workshop" => include_bytes!("../assets/templates/aster-workshop.png"),
        "tide-workshop" => include_bytes!("../assets/templates/tide-workshop.png"),
        "lantern-workshop" => include_bytes!("../assets/templates/lantern-workshop.png"),
        "aureole-workshop" => include_bytes!("../assets/templates/aureole-workshop.png"),
        _ => return None,
    })
}

fn thumbnail(ui: &Ui, template: &Template) -> TextureHandle {
    let id = egui::Id::new(("template-thumbnail", template.slug));
    if let Some(texture) = ui.ctx().data(|data| data.get_temp::<TextureHandle>(id)) { return texture; }
    let image = image::load_from_memory(preview_bytes(template.slug).expect("template preview")).expect("valid bundled preview").into_rgba8();
    let texture = ui.ctx().load_texture(template.slug, egui::ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], &image), egui::TextureOptions::LINEAR);
    ui.ctx().data_mut(|data| data.insert_temp(id, texture.clone()));
    texture
}

/// Nested collection menus keep all authored templates reachable without parsing designs per frame.
pub fn menu(ui: &mut Ui) -> Option<&'static Template> {
    let mut chosen = None;
    for collection in collections() {
        let thumb = thumbnail(ui, &collection.templates[0]);
        let size = (ui.spacing().interact_size.y - ui.spacing().button_padding.y * 2.).min(24.);
        ui.menu_button((egui::Image::new((thumb.id(), egui::vec2(size, size))), collection.name), |ui| {
            egui::ScrollArea::vertical().max_height((ui.ctx().content_rect().height() * 0.7).min(520.)).show(ui, |ui| {
                for template in &collection.templates {
                    let thumb = thumbnail(ui, template);
                    let response = ui.add(egui::Button::new((egui::Image::new((thumb.id(), egui::vec2(size, size))), template.name)).image_tint_follows_text_color(false));
                    if response.clicked() { chosen = Some(template); ui.close(); }
                    response.on_hover_ui(|ui| {
                        ui.image((thumb.id(), egui::vec2(160., 160.)));
                        ui.label(template.description);
                    });
                }
            });
        });
    }
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_authored_graph_and_collection_has_a_real_preview() {
        let entries: Vec<_> = collections().iter().flat_map(|c| &c.templates).collect();
        let slugs: std::collections::HashSet<_> = entries.iter().map(|t| t.slug).collect();
        assert_eq!(entries.len(), slugs.len(), "templates appear once");
        for graph in ringdesign_graph::templates::catalog() { assert!(slugs.contains(graph.slug), "{} absent from menu", graph.slug); }
        let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../graphs/templates");
        for file in std::fs::read_dir(directory).unwrap() {
            let file = file.unwrap().file_name().to_string_lossy().into_owned();
            if let Some(slug) = file.strip_suffix(".graph.json") { assert!(slugs.contains(slug), "{slug} is authored but unavailable"); }
        }
        for entry in entries {
            let png = image::load_from_memory(preview_bytes(entry.slug).expect(entry.slug)).unwrap().to_rgb8();
            assert_eq!((png.width(), png.height()), (160, 160));
            assert!(png.pixels().any(|p| p.0.iter().copied().max().unwrap() > 100), "{} preview is blank", entry.slug);
        }
        assert_eq!(slugs.len(), 31);
    }
}
