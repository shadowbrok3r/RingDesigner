//! Sand-cast verdict, ring dimensions, and metal weight.

use egui_phosphor::regular as icon;

use ringdesign_core::castability::{CastReport, DraftSettings, FaceClass, Verdict};
use ringdesign_core::mesh::Report;

use crate::app::RingDesignerApp;
use crate::pane::PaneKind;
use crate::theme;
use crate::viewport::ShadeMode;

const CLASSES: [FaceClass; 4] = [
    FaceClass::Good,
    FaceClass::Marginal,
    FaceClass::Vertical,
    FaceClass::Undercut,
];

pub fn ui(app: &mut RingDesignerApp, ui: &mut egui::Ui) {
    ui.add_space(6.0);
    heading(ui, icon::SHIELD_CHECK, "Sand cast check");

    let pane = app.active_pane.min(app.panes.len() - 1);
    let already_draft =
        app.panes[pane].shade == ShadeMode::Draft && app.panes[pane].kind == PaneKind::Solid;
    let mut want_draft = false;
    let dfm = ringdesign_core::dfm::findings(&app.design);
    match app.cast.as_ref() {
        Some(cast) => {
            want_draft = castability(
                ui,
                cast,
                app.field.as_ref(),
                &app.design.draft,
                &dfm,
                already_draft,
            );
        }
        None => placeholder(ui, app.is_building(), "No draft analysis yet"),
    }
    if want_draft {
        app.panes[pane].shade = ShadeMode::Draft;
        app.focus(PaneKind::Solid);
    }

    ui.add_space(8.0);

    if let Some(stones) = app.stones.as_ref() {
        let badge = if stones.any_warnings() {
            format!(" {}", icon::WARNING)
        } else {
            String::new()
        };
        egui::CollapsingHeader::new(format!("{} Stones{badge}", icon::DIAMOND))
            .default_open(true)
            .show(ui, |ui| stones_section(ui, stones));
        ui.add_space(8.0);
    }

    match app.build.as_ref() {
        Some(build) => {
            let report = &build.report;
            let size = app.design.size.display();
            egui::CollapsingHeader::new(format!("{} Dimensions", icon::RULER))
                .default_open(true)
                .show(ui, |ui| dimensions(ui, report, &size));
            egui::CollapsingHeader::new(format!("{} Metal weight", icon::SCALES))
                .default_open(true)
                .show(ui, |ui| metals_priced(ui, report, &app.prices));
            if let Some((theta, m)) = app.hot_spot {
                ui.add_space(2.0);
                ui.label(
                    egui::RichText::new(format!(
                        "{} Freezes last at {theta:.0}° (section modulus {m:.2} mm)",
                        icon::THERMOMETER_HOT
                    ))
                    .small()
                    .color(theme::TEXT_DIM),
                )
                .on_hover_text(
                    "Chvorinov: the slice with the most area per perimeter cools slowest —                      feed it with the gate or a riser, or shrinkage porosity collects there.",
                );
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} {} ms • {} tris",
                    icon::TIMER,
                    report.build_ms,
                    report.validation.triangle_count
                ))
                .small()
                .color(theme::TEXT_DIM),
            );
        }
        None => placeholder(ui, app.is_building(), "No mesh yet"),
    }
}

// --- Castability -----------------------------------------------------------

/// Returns true when the jeweller asked for the draft-coloured view.
fn castability(
    ui: &mut egui::Ui,
    cast: &CastReport,
    field: Option<&ringdesign_core::castability::FieldReport>,
    draft: &DraftSettings,
    dfm: &[ringdesign_core::dfm::DfmFinding],
    already_draft: bool,
) -> bool {
    match field {
        Some(f) => field_banner(ui, f),
        None => verdict_banner(ui, cast),
    }
    ui.add_space(6.0);

    if let Some(f) = field {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Thinnest wall").color(theme::TEXT_DIM));
            let color = if f.thinnest_wall_mm < draft.min_section_mm {
                theme::WARN
            } else {
                theme::GOOD
            };
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{:.2} mm", f.thinnest_wall_mm))
                        .strong()
                        .color(color),
                )
                .selectable(true),
            );
            ui.label(
                egui::RichText::new(format!("at {:.0}° • min {:.1}", f.thinnest_wall_theta_deg, draft.min_section_mm))
                    .small()
                    .color(theme::TEXT_DIM),
            );
        })
        .response
        .on_hover_text(
            "Outer surface to bore over the middle of the finger hole — the metal a pour must fill.",
        );
        ui.add_space(4.0);
    }

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Parting plane").color(theme::TEXT_DIM));
        ui.add(
            egui::Label::new(egui::RichText::new(format!("{:+.2} mm", cast.parting_z_mm)).strong())
                .selectable(true),
        );
        let tag = if draft.auto_parting {
            "auto"
        } else {
            "set by hand"
        };
        ui.label(egui::RichText::new(tag).small().color(theme::ACCENT_DIM));
    })
    .response
    .on_hover_text(
        "Height of the split between cope and drag; the mould pulls +Z above it and -Z below it.",
    );

    let areas = class_areas(cast);
    ui.add_space(4.0);
    class_bar(ui, cast, &areas);
    ui.add_space(3.0);

    egui::Grid::new("draft_classes")
        .num_columns(3)
        .striped(true)
        .min_col_width(48.0)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            let counts = [cast.good, cast.marginal, cast.vertical, cast.undercut];
            for (k, class) in CLASSES.iter().enumerate() {
                ui.horizontal(|ui| {
                    swatch(ui, theme::class_color(*class));
                    ui.label(class.label());
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", counts[k]))
                            .monospace()
                            .color(theme::TEXT_DIM),
                    );
                });
                let detail = matches!(class, FaceClass::Marginal | FaceClass::Undercut);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if detail {
                        ui.label(
                            egui::RichText::new(format!(
                                "{:.1} mm2 • {:.1}%",
                                areas[k],
                                fraction(areas[k], cast.total_area_mm2) * 100.0
                            ))
                            .monospace()
                            .color(if areas[k] > 0.0 {
                                theme::class_color(*class)
                            } else {
                                theme::TEXT_DIM
                            }),
                        );
                    }
                });
                ui.end_row();
            }
        });

    ui.add_space(5.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Worst draft").color(theme::TEXT_DIM));
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!("{:+.1}°", cast.worst_draft_deg))
                    .strong()
                    .color(draft_color(cast.worst_draft_deg, draft.min_draft_deg)),
            )
            .selectable(true),
        );
        ui.label(
            egui::RichText::new(format!("min {:.1}°", draft.min_draft_deg))
                .small()
                .color(theme::TEXT_DIM),
        );
    })
    .response
    .on_hover_text("Most negative draft on the mesh. Below zero the face leans back under itself and locks in the sand.");

    notes(ui, &cast.notes);
    if let Some(f) = field {
        // The field's own findings: the noise-band line and any located,
        // blamed undercut arcs.
        for n in f
            .notes
            .iter()
            .filter(|n| !n.starts_with("Field-sampled: the surface itself"))
        {
            ui.horizontal_top(|ui| {
                ui.label(egui::RichText::new("•").color(theme::ACCENT));
                ui.add(egui::Label::new(egui::RichText::new(n)).wrap());
            });
        }
    }
    for f in dfm {
        ui.horizontal_top(|ui| {
            ui.label(egui::RichText::new(icon::WARNING).color(theme::WARN));
            ui.add(
                egui::Label::new(
                    egui::RichText::new(format!("{}: {}", f.label, f.message))
                        .small()
                        .color(theme::WARN),
                )
                .wrap(),
            );
        });
    }

    ui.add_space(6.0);
    let button = egui::Button::new(format!("{} Show draft colours", icon::PALETTE));
    let clicked = ui
        .add_enabled(!already_draft, button)
        .on_hover_text("Colour the ring by face class in the 3D view")
        .clicked();
    if already_draft {
        ui.label(
            egui::RichText::new("Draft colours are on")
                .small()
                .color(theme::TEXT_DIM),
        );
    }
    clicked
}

/// The banner from the field-sampled report: the verdict of the surface
/// itself, so no build kind or resolution can put a phantom in it.
fn field_banner(ui: &mut egui::Ui, f: &ringdesign_core::castability::FieldReport) {
    let color = theme::verdict_color(f.verdict);
    let glyph = match f.verdict {
        Verdict::Castable => icon::CHECK_CIRCLE,
        Verdict::Marginal => icon::WARNING,
        Verdict::NotCastable => icon::X_CIRCLE,
    };
    let detail = match f.verdict {
        Verdict::Castable => "The surface itself clears a two-part pull.".to_string(),
        Verdict::Marginal => f
            .notes
            .iter()
            .find(|n| n.contains("Thinnest wall"))
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "{:.1}% undercuts or drags on the surface itself.",
                    (f.undercut_fraction()
                        + (f.marginal_area_mm2 + f.vertical_area_mm2) / f.total_area_mm2.max(1e-9))
                        * 100.0
                )
            }),
        Verdict::NotCastable => format!(
            "{:.1}% of the surface locks in, worst {:.1}°.",
            f.undercut_fraction() * 100.0,
            -f.worst_draft_deg
        ),
    };

    egui::Frame::NONE
        .fill(color.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.60)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(glyph).size(22.0).color(color));
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(f.verdict.label())
                                .size(15.0)
                                .strong()
                                .color(color),
                        );
                        ui.label(
                            egui::RichText::new("field-sampled")
                                .small()
                                .color(theme::ACCENT_DIM),
                        )
                        .on_hover_text(format!(
                            "Sampled off the surface itself at {}x{} — independent of the preview mesh, so facet noise cannot fake an undercut.",
                            f.theta_samples, f.profile_samples
                        ));
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(detail).small().color(theme::TEXT_DIM),
                        )
                        .wrap(),
                    );
                });
            });
        });
}

fn verdict_banner(ui: &mut egui::Ui, cast: &CastReport) {
    let color = theme::verdict_color(cast.verdict);
    let glyph = match cast.verdict {
        Verdict::Castable => icon::CHECK_CIRCLE,
        Verdict::Marginal => icon::WARNING,
        Verdict::NotCastable => icon::X_CIRCLE,
    };
    let undercut_pct = fraction(cast.undercut_area_mm2, cast.total_area_mm2) * 100.0;
    let detail = match cast.verdict {
        Verdict::Castable => "Every face clears a two-part pull.".to_string(),
        Verdict::Marginal => format!(
            "{} faces drag on the sand, {} undercut.",
            cast.marginal, cast.undercut
        ),
        Verdict::NotCastable => format!(
            "{} undercut faces • {:.1}% of the surface locks in.",
            cast.undercut, undercut_pct
        ),
    };

    egui::Frame::NONE
        .fill(color.gamma_multiply(0.16))
        .stroke(egui::Stroke::new(1.0, color.gamma_multiply(0.60)))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(glyph).size(22.0).color(color));
                ui.add_space(2.0);
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(cast.verdict.label())
                            .size(15.0)
                            .strong()
                            .color(color),
                    );
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(detail).small().color(theme::TEXT_DIM),
                        )
                        .wrap(),
                    );
                });
            });
        });
}

/// Per-class area in mm2; good and vertical share the released remainder by face count.
fn class_areas(cast: &CastReport) -> [f64; 4] {
    let rest = (cast.total_area_mm2 - cast.marginal_area_mm2 - cast.undercut_area_mm2).max(0.0);
    let n = (cast.good + cast.vertical) as f64;
    let (good, vertical) = if n > 0.0 {
        (rest * cast.good as f64 / n, rest * cast.vertical as f64 / n)
    } else {
        (0.0, 0.0)
    };
    [
        good,
        cast.marginal_area_mm2,
        vertical,
        cast.undercut_area_mm2,
    ]
}

fn class_bar(ui: &mut egui::Ui, cast: &CastReport, areas: &[f64; 4]) {
    let (response, painter) =
        ui.allocate_painter(egui::vec2(ui.available_width(), 15.0), egui::Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 3.0, theme::BG);

    let mut x = rect.left();
    for (k, class) in CLASSES.iter().enumerate() {
        let frac = fraction(areas[k], cast.total_area_mm2);
        if frac <= 0.0 {
            continue;
        }
        let w = ((frac * rect.width() as f64) as f32)
            .max(2.0)
            .min(rect.right() - x);
        if w <= 0.0 {
            break;
        }
        painter.rect_filled(
            egui::Rect::from_min_size(egui::pos2(x, rect.top()), egui::vec2(w, rect.height())),
            0.0,
            theme::class_color(*class),
        );
        x += w;
    }

    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, theme::GRID),
        egui::StrokeKind::Inside,
    );

    let tip = CLASSES
        .iter()
        .enumerate()
        .map(|(k, c)| {
            format!(
                "{}: {:.1} mm2 ({:.1}%)",
                c.label(),
                areas[k],
                fraction(areas[k], cast.total_area_mm2) * 100.0
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    response.on_hover_text(format!("{tip}\nTotal {:.1} mm2", cast.total_area_mm2));
}

fn notes(ui: &mut egui::Ui, notes: &[String]) {
    ui.add_space(5.0);
    if notes.is_empty() {
        ui.label(
            egui::RichText::new("• Nothing flagged.")
                .small()
                .color(theme::TEXT_DIM),
        );
        return;
    }
    for note in notes {
        ui.horizontal_top(|ui| {
            ui.label(egui::RichText::new("•").color(theme::ACCENT));
            ui.add(egui::Label::new(egui::RichText::new(note)).wrap());
        });
    }
}

fn draft_color(worst_deg: f64, min_deg: f64) -> egui::Color32 {
    if worst_deg < 0.0 {
        theme::BAD
    } else if worst_deg < min_deg {
        theme::WARN
    } else {
        theme::GOOD
    }
}

// --- Dimensions ------------------------------------------------------------

fn dimensions(ui: &mut egui::Ui, report: &Report, size: &str) {
    egui::Grid::new("dimensions")
        .num_columns(2)
        .striped(true)
        .min_col_width(96.0)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            row(ui, "Ring size", size.to_string());
            row(
                ui,
                "Inside dia",
                format!("{:.2} mm", report.inner_diameter_mm),
            );
            row(
                ui,
                "Outside dia",
                format!("{:.2} mm", report.outer_diameter_mm),
            );
            row(ui, "Band width", format!("{:.2} mm", report.band_width_mm));
            row(
                ui,
                "Overall",
                format!(
                    "{:.2} x {:.2} x {:.2} mm",
                    report.bounds_mm[0], report.bounds_mm[1], report.bounds_mm[2]
                ),
            );
            row(
                ui,
                "Highest relief",
                format!("{:+.3} mm", report.max_relief_mm),
            );
            row(
                ui,
                "Deepest cut",
                format!("{:+.3} mm", report.min_relief_mm),
            );
            row(ui, "Surface", format!("{:.1} mm2", report.surface_area_mm2));
            row(ui, "Volume", format!("{:.2} mm3", report.volume_mm3));
        });

    ui.add_space(5.0);
    let v = &report.validation;
    ui.horizontal(|ui| {
        let (glyph, color, text) = if v.watertight {
            (icon::CHECK_CIRCLE, theme::GOOD, "Watertight".to_string())
        } else {
            (
                icon::WARNING,
                theme::BAD,
                format!(
                    "Not watertight • {} boundary, {} non-manifold edges",
                    v.boundary_edges, v.non_manifold_edges
                ),
            )
        };
        ui.label(egui::RichText::new(glyph).color(color));
        ui.add(egui::Label::new(egui::RichText::new(text).color(color)).wrap());
    });
    let q = &report.quality;
    ui.label(
        egui::RichText::new(format!(
            "{} tris • {} verts • min angle {:.1}{} aspect {:.0}{}",
            v.triangle_count,
            v.vertex_count,
            q.min_angle_deg,
            "\u{00b0} •",
            q.worst_aspect,
            if q.degenerate_faces > 0 {
                format!(" • {} degenerate", q.degenerate_faces)
            } else {
                String::new()
            }
        ))
        .small()
        .color(theme::TEXT_DIM),
    )
    .on_hover_text("Worst triangle anywhere: corner angle (60 is equilateral) and longest-edge to height ratio. Slivers shade and slice badly even when watertight.");
}

// --- Metal weight ----------------------------------------------------------

fn metals_priced(
    ui: &mut egui::Ui,
    report: &Report,
    prices: &std::collections::HashMap<String, f64>,
) {
    let cols = if prices.is_empty() { 3 } else { 4 };
    egui::Grid::new("metal_weights")
        .num_columns(cols)
        .striped(true)
        .min_col_width(56.0)
        .spacing([10.0, 3.0])
        .show(ui, |ui| {
            ui.label(egui::RichText::new("Metal").small().color(theme::TEXT_DIM));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("grams").small().color(theme::TEXT_DIM));
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("dwt").small().color(theme::TEXT_DIM));
            });
            if !prices.is_empty() {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new("metal $").small().color(theme::TEXT_DIM));
                });
            }
            ui.end_row();

            for m in &report.metals {
                ui.label(m.metal);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{:.2}", m.grams)).monospace(),
                        )
                        .selectable(true),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(format!("{:.2}", m.dwt))
                                .monospace()
                                .color(theme::TEXT_DIM),
                        )
                        .selectable(true),
                    );
                });
                if !prices.is_empty() {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match prices.get(m.metal) {
                            Some(p) => ui.add(
                                egui::Label::new(
                                    egui::RichText::new(format!("{:.0}", p * m.grams))
                                        .monospace(),
                                )
                                .selectable(true),
                            ),
                            None => ui.label(egui::RichText::new("-").color(theme::TEXT_DIM)),
                        };
                    });
                }
                ui.end_row();
            }
        });
    ui.add_space(3.0);
    ui.label(
        egui::RichText::new("Casting weight only — no sprue, button, or finishing loss.")
            .small()
            .color(theme::TEXT_DIM),
    );
}

// --- Stones ----------------------------------------------------------------

fn stones_section(ui: &mut egui::Ui, stones: &ringdesign_core::stones::StonesReport) {
    use ringdesign_core::stones::SeatFooting;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} stones", stones.stone_count)).strong());
        ui.label(
            egui::RichText::new(format!("{:.2} ct total", stones.total_carats))
                .strong()
                .color(theme::ACCENT),
        );
        ui.label(
            egui::RichText::new("set at the bench — the ring casts the stock")
                .small()
                .color(theme::TEXT_DIM),
        );
    });
    ui.add_space(4.0);

    for seat in &stones.seats {
        let stone = seat
            .gem
            .map(|g| g.display())
            .unwrap_or_else(|| "no stone assigned".into());
        let count = if seat.count > 1 {
            format!(" ×{}", seat.count)
        } else {
            String::new()
        };
        ui.label(egui::RichText::new(format!("{}{count} — {stone}", seat.label)).strong());
        egui::Grid::new(format!("stone_{}", seat.label))
            .num_columns(2)
            .min_col_width(96.0)
            .spacing([8.0, 2.0])
            .show(ui, |ui| {
                let footing = match seat.footing {
                    SeatFooting::SideFace => "side face — castable by construction".to_string(),
                    SeatFooting::Crown(d) => format!("crown, {d:+.1}° base draft"),
                };
                row(ui, "Sits on", footing);
                row(
                    ui,
                    "Seat",
                    format!("{:.2} mm {}", seat.seat_diameter_mm, seat.style.label()),
                );
                row(
                    ui,
                    "Edge clearance",
                    format!("{:.2} mm", seat.edge_clearance_mm),
                );
                row(
                    ui,
                    "Depth for pavilion",
                    format!("{:.2} mm", seat.depth_available_mm),
                );
                if let Some(b) = seat.bridge_mm {
                    row(ui, "Bridge", format!("{b:.2} mm"));
                }
                if let Some((pairs, dia, proud)) = seat.shared_prongs {
                    row(
                        ui,
                        "Shared prongs",
                        format!("{pairs} pairs, {dia:.2} mm posts, {proud:.2} mm proud"),
                    );
                }
            });
        for w in &seat.warnings {
            ui.horizontal_top(|ui| {
                ui.label(egui::RichText::new(icon::WARNING).color(theme::WARN));
                ui.add(egui::Label::new(egui::RichText::new(w).small().color(theme::WARN)).wrap());
            });
        }
        ui.add_space(4.0);
    }
}

// --- Shared bits -----------------------------------------------------------

fn heading(ui: &mut egui::Ui, glyph: &str, text: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(glyph).color(theme::ACCENT));
        ui.label(egui::RichText::new(text).size(14.0).strong());
    });
    ui.add_space(3.0);
}

fn row(ui: &mut egui::Ui, label: &str, value: String) {
    ui.label(egui::RichText::new(label).color(theme::TEXT_DIM));
    ui.add(egui::Label::new(value).selectable(true));
    ui.end_row();
}

/// Fixed allocation so the swatch sits on the text centre line.
fn swatch(ui: &mut egui::Ui, color: egui::Color32) {
    let (response, painter) = ui.allocate_painter(egui::Vec2::splat(9.0), egui::Sense::hover());
    painter.rect_filled(response.rect, 2.0, color);
}

fn placeholder(ui: &mut egui::Ui, building: bool, what: &str) {
    ui.horizontal(|ui| {
        if building {
            ui.add(egui::Spinner::new().size(12.0));
            ui.label(egui::RichText::new("Building…").color(theme::TEXT_DIM));
        } else {
            ui.label(egui::RichText::new(icon::CIRCLE_DASHED).color(theme::TEXT_DIM));
            ui.label(egui::RichText::new(what).color(theme::TEXT_DIM));
        }
    });
}

fn fraction(part: f64, total: f64) -> f64 {
    if total > 0.0 {
        (part / total).clamp(0.0, 1.0)
    } else {
        0.0
    }
}
