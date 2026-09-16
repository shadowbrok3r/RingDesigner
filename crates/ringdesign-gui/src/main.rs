//! RingDesigner — procedural sand-castable ring design.

mod alpha_editor;
mod app;
mod camera;
mod dock;
mod export;
mod gems;
mod mcp_host;
mod pane;
mod panels;
mod theme;
mod viewport;

use app::RingDesignerApp;

impl eframe::App for RingDesignerApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.tick(&ui.ctx().clone());
        self.poll_export();
        panels::render(self, ui);
        if self.wants_repaint() {
            ui.ctx().request_repaint();
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(&self.design) {
            storage.set_string(app::DESIGN_STORAGE_KEY, json);
        }
        if let Ok(json) = serde_json::to_string(&self.dock) {
            storage.set_string(app::DOCK_STORAGE_KEY, json);
        }
        if let Ok(json) = serde_json::to_string(&self.workspace()) {
            storage.set_string(app::WORKSPACE_STORAGE_KEY, json);
        }
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

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    eframe::run_native(
        &format!("RingDesigner v{}", env!("CARGO_PKG_VERSION")),
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([1600.0, 980.0])
                .with_min_inner_size([1100.0, 700.0])
                .with_drag_and_drop(true),
            // eframe asks for a 0-bit depth buffer by default, which leaves the
            // window with no depth attachment at all: GL_DEPTH_TEST then does
            // nothing and the far wall of the ring draws over the near one.
            depth_buffer: 24,
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
    )
}
