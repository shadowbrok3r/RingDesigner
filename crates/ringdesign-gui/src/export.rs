//! File dialogs for mesh export, design persistence, and alpha import.

use std::path::PathBuf;

use ringdesign_core::{library, stl, threemf};

use crate::app::RingDesignerApp;

/// Directory a dialog opens in, created on demand so it is always there to
/// browse. Everything the app writes lands under one predictable tree.
fn dir(kind: &str) -> PathBuf {
    let path = match kind {
        "designs" => library::default_design_dir(),
        _ => library::default_design_dir().with_file_name(kind),
    };
    let _ = std::fs::create_dir_all(&path);
    path
}

pub fn export_stl(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Binary STL", &["stl"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.stl", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    app.set_status("Building at export resolution…");
    let out = app.build_for_export();
    match stl::write_stl(&path, &out.mesh, &app.design.name) {
        Ok(bytes) => {
            let v = out.report.validation;
            let warn = if v.watertight { "" } else { " • NOT watertight" };
            app.set_status(format!(
                "Wrote {} • {} tris • {:.1} KB{warn}",
                path.display(),
                v.triangle_count,
                bytes as f64 / 1024.0
            ));
        }
        Err(e) => app.set_status(format!("STL export failed: {e}")),
    }
}

pub fn export_obj(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Wavefront OBJ", &["obj"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.obj", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    app.set_status("Building at export resolution…");
    let out = app.build_for_export();
    let name = app.design.name.clone();
    match stl::write_obj(&path, &out.mesh, &name) {
        Ok(bytes) => app.set_status(format!(
            "Wrote {} • {} tris • {:.1} KB",
            path.display(),
            out.report.validation.triangle_count,
            bytes as f64 / 1024.0
        )),
        Err(e) => app.set_status(format!("OBJ export failed: {e}")),
    }
}

pub fn export_3mf(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("3MF model", &["3mf"])
        .set_directory(dir("exports"))
        .set_file_name(format!("{}.3mf", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    app.set_status("Building at export resolution…");
    let out = app.build_for_export();
    let size = app.design.size.display();
    match threemf::write_3mf(&path, &out.mesh, &app.design.name, &size) {
        Ok(bytes) => app.set_status(format!(
            "Wrote {} • {} tris • {:.1} KB • units mm stated",
            path.display(),
            out.report.validation.triangle_count,
            bytes as f64 / 1024.0
        )),
        Err(e) => app.set_status(format!("3MF export failed: {e}")),
    }
}

pub fn save_design(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Ring design", &["json"])
        .set_directory(dir("designs"))
        .set_file_name(format!("{}.json", slug(&app.design.name)))
        .save_file()
    else {
        return;
    };
    match library::save_design_embedded(&path, &app.design, &app.lib) {
        Ok(()) => app.set_status(format!("Saved {}", path.display())),
        Err(e) => app.set_status(format!("Save failed: {e}")),
    }
}

pub fn open_design(app: &mut RingDesignerApp) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Ring design", &["json"])
        .set_directory(dir("designs"))
        .pick_file()
    else {
        return;
    };
    match library::load_design(&path) {
        Ok(d) => {
            d.unpack_embedded(app.library_mut());
            d.bake_drawn(app.library_mut());
            d.bake_texts(app.library_mut());
            app.design = d;
            // A different file is a different session; the old timeline does
            // not describe it.
            app.history.reset(&app.design.clone());
            app.selected_layer = None;
            app.fit_pending = true;
            app.mark_dirty();
            app.set_status(format!("Opened {}", path.display()));
        }
        Err(e) => app.set_status(format!("Open failed: {e}")),
    }
}

/// Import image files into the alpha library.
pub fn import_alphas(app: &mut RingDesignerApp) {
    let Some(paths) = rfd::FileDialog::new()
        .add_filter("Images", &["png", "jpg", "jpeg", "bmp"])
        .set_directory(library::default_alpha_dir())
        .pick_files()
    else {
        return;
    };
    let mut added = 0usize;
    let mut failed = 0usize;
    for p in paths {
        match ringdesign_core::alpha::Alpha::load(&p) {
            Ok(a) => {
                app.forget_thumbnail(&a.name);
                app.library_mut().insert(a);
                added += 1;
            }
            Err(e) => {
                log::warn!("import {}: {e}", p.display());
                failed += 1;
            }
        }
    }
    app.set_status(if failed == 0 {
        format!("Imported {added} alpha(s)")
    } else {
        format!("Imported {added}, {failed} failed")
    });
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '_' })
        .collect();
    let s = s.trim_matches('_').to_string();
    if s.is_empty() { "ring".into() } else { s }
}
