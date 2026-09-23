//! RingDesigner — procedural sand-castable ring design.

// A GUI launched from Explorer must not raise a console behind its window.
// Debug keeps one, because that is where env_logger writes.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod alpha_editor;
mod app;
mod camera;
mod cad_edit;
mod command;
mod comfy_texture;
mod cutter_tools;
mod dock;
mod export;
mod gems;
mod mcp_host;
#[cfg(feature = "kernel-occt")]
mod occt;
mod pane;
mod panels;
mod patterns;
mod theme;
mod viewport;
mod updater;
mod ring_snaps;
mod session;
mod sketch_mode;
mod stone_tools;
mod swatch;
#[cfg(test)]
mod ui_tests;
#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod timeline_tests;
#[cfg(test)]
mod command_tests;
#[cfg(test)]
mod gizmo_tests;
#[cfg(test)]
mod sketch_tests;
#[cfg(test)]
mod stone_tests;
#[cfg(test)]
mod snaps_tests;
#[cfg(test)]
mod pattern_tests;
#[cfg(test)]
mod sweep_tests;
#[cfg(test)]
mod cutter_tests;

use app::RingDesignerApp;

impl eframe::App for RingDesignerApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.tick(&ui.ctx().clone());
        self.poll_export();
        self.updater.poll(ui.ctx());
        panels::render(self, ui);
        if std::mem::take(&mut self.install_update) {
            if let Some(storage) = frame.storage_mut() {
                match self.persist_session(storage) {
                    Ok(()) => { storage.flush(); self.updater.install(ui.ctx()); }
                    Err(error) => self.updater.status = format!("Cannot save the session: {error}"),
                }
            } else {
                self.updater.status = "Restart unavailable: session storage is disabled".into();
            }
        }
        if self.updater.restart {
            updater::RESTART.store(true, std::sync::atomic::Ordering::SeqCst);
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.wants_repaint() {
            ui.ctx().request_repaint();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Err(error) = self.persist_session(storage) { log::error!("Cannot save session: {error}"); }
    }

    fn on_exit(&mut self, gl: Option<&eframe::glow::Context>) {
        if let Some(gl) = gl {
            if let Ok(mut r) = self.renderer.lock() {
                r.destroy(gl);
            }
            if let Ok(mut r) = self.mould_renderer.lock() {
                r.destroy(gl);
            }
        }
    }
}

impl RingDesignerApp {
    fn persist_session(&self, storage: &mut dyn eframe::Storage) -> anyhow::Result<()> {
        let mut design = self.design.clone();
        design.embed_alphas(&self.lib);
        // Serialize the whole session successfully before touching any stored part.
        let design = serde_json::to_string(&design)?;
        let dock = serde_json::to_string(&self.dock)?;
        let workspace = serde_json::to_string(&self.workspace())?;
        storage.set_string(app::DESIGN_STORAGE_KEY, design);
        storage.set_string(app::DOCK_STORAGE_KEY, dock);
        storage.set_string(app::WORKSPACE_STORAGE_KEY, workspace);
        Ok(())
    }
}

/// The taskbar and title-bar icon, rasterized from the bundled SVG. Windows
/// also carries it in the `.exe`'s resources (`build.rs`) so Explorer shows it
/// before the process runs.
fn app_icon() -> eframe::egui::IconData {
    let edge = ringdesign_assets::APP_ICON_EDGE;
    eframe::egui::IconData { rgba: ringdesign_assets::app_icon_rgba(), width: edge, height: edge }
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let executable = std::env::current_exe().ok();
    let result = eframe::run_native(
        &format!("RingDesigner v{}", env!("CARGO_PKG_VERSION")),
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_app_id(session::APP_ID)
                .with_inner_size([1600.0, 980.0])
                .with_min_inner_size([1100.0, 700.0])
                .with_icon(app_icon())
                .with_drag_and_drop(true),
            // eframe asks for a 0-bit depth buffer by default, which leaves the
            // window with no depth attachment at all: GL_DEPTH_TEST then does
            // nothing and the far wall of the ring draws over the near one.
            depth_buffer: 24,
            persistence_path: session::persistence_path(),
            ..Default::default()
        },
        Box::new(|cc| {
            theme::install(&cc.egui_ctx);
            let mut app=RingDesignerApp::new(cc);
            let args:Vec<String>=std::env::args().collect();
            if let Some(path)=args.windows(2).find(|pair|pair[0]=="--open").map(|pair|&pair[1]) {
                export::open_design_path(&mut app,std::path::Path::new(path));
                if app.design.shank.kind==ringdesign_core::ShankKind::Signet {
                    for pane in &mut app.panes {
                        pane.camera.yaw=app.design.shank.head.theta_deg.to_radians() as f32-std::f32::consts::FRAC_PI_8;
                        pane.camera.pitch=-0.55;
                    }
                }
            }
            if std::env::args().any(|a|a=="--casting") { app.focus(pane::PaneKind::Casting); }
            if std::env::args().any(|a|a=="--cad") { app.focus(pane::PaneKind::Cad); }
            Ok(Box::new(app))
        }),
    );
    if result.is_ok() && updater::RESTART.load(std::sync::atomic::Ordering::SeqCst) {
        if let Some(executable) = executable {
            // No --open argument: restore the just-saved session, not an older disk file.
            if let Err(error) = std::process::Command::new(executable).spawn() {
                eprintln!("Update installed. Please reopen RingDesigner: {error}");
            }
        }
    }
    result
}
