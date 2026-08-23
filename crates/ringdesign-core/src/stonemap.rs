//! The stone spacing map: every stone the design sets, printed to scale —
//! plan view and the unrolled chart — with the census's tight gaps drawn
//! between the pairs the setter has to watch.

use crate::field::wrap_delta;
use crate::stones::StonesReport;
use crate::RingDesign;

/// Drawn at this magnification; the title says so.
const K: f64 = 2.0;
const PAD: f64 = 4.0;

/// The map as SVG text, or `None` when the design sets no stones.
pub fn stone_map_svg(design: &RingDesign, report: Option<&StonesReport>) -> Option<String> {
    let frames = crate::stones::stone_frames(design);
    if frames.is_empty() {
        return None;
    }
    let ctx = design.field_context();
    let r_out = frames
        .iter()
        .map(|(_, f)| f.girdle[0].hypot(f.girdle[1]) + f.reach)
        .fold(ctx.crest_radius_mm, f64::max)
        + 1.0;
    let plan = 2.0 * r_out * K;
    let chart_w = ctx.circumference_mm * K;
    let chart_h = ctx.band_v_len_mm * K;
    let legend_h = 14.0;
    let w = plan.max(chart_w) + 2.0 * PAD;
    let h = plan + chart_h + legend_h + 4.0 * PAD;
    let esc = |t: &str| t.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;");
    let mut s = String::with_capacity(frames.len() * 400 + 2048);
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.1}mm\" height=\"{h:.1}mm\" viewBox=\"0 0 {w:.2} {h:.2}\" font-family=\"sans-serif\">\n<title>{} stone map, drawn 2:1 (mm)</title>\n",
        esc(&design.name)
    ));
    // Plan view: the crest and bore circles, every stone drawn face-on at
    // its position — a bench map, not a projection: a crest-set stone seen
    // down the finger is an edge, which tells the setter nothing.
    let (cx, cy) = (w * 0.5, PAD + r_out * K);
    s.push_str(&format!("<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{:.2}\" fill=\"none\" stroke=\"#888\" stroke-width=\"0.2\"/>\n", ctx.crest_radius_mm * K));
    s.push_str(&format!("<circle cx=\"{cx:.2}\" cy=\"{cy:.2}\" r=\"{:.2}\" fill=\"none\" stroke=\"#888\" stroke-width=\"0.15\" stroke-dasharray=\"1,1\"/>\n", design.inner_radius_mm() * K));
    for (st, f) in &frames {
        let (sin_t, cos_t) = st.theta_deg.to_radians().sin_cos();
        let (tangent, radial) = ([-sin_t, cos_t], [cos_t, sin_t]);
        let (rs, rc) = st.rot_deg().to_radians().sin_cos();
        s.push_str("<path class=\"stone\" fill=\"#dfe\" stroke=\"black\" stroke-width=\"0.2\" d=\"");
        for k in 0..=48 {
            let t = k as f64 / 48.0 * std::f64::consts::TAU;
            let (a, b) = superellipse(t, f.semi.0, f.semi.1, f.plan_pow);
            // The long axis lies along the ring at the seat's bearing.
            let (along, across) = (a * rc - b * rs, a * rs + b * rc);
            let p = [
                f.girdle[0] + tangent[0] * along + radial[0] * across,
                f.girdle[1] + tangent[1] * along + radial[1] * across,
            ];
            s.push_str(&format!("{}{:.2},{:.2} ", if k == 0 { 'M' } else { 'L' }, cx + p[0] * K, cy - p[1] * K));
        }
        s.push_str("Z\"/>\n");
    }
    // The chart: u around the ring, v across the band, every stone's plan at its bearing.
    let (x0, y0) = (PAD, PAD + plan + PAD);
    s.push_str(&format!("<rect x=\"{x0:.2}\" y=\"{y0:.2}\" width=\"{chart_w:.2}\" height=\"{chart_h:.2}\" fill=\"none\" stroke=\"#888\" stroke-width=\"0.2\"/>\n"));
    let centre = |st: &crate::setstone::SetStone| -> (f64, f64) {
        (x0 + ctx.u_of_theta(st.theta_deg.rem_euclid(360.0)) * K, y0 + st.v_mm * K)
    };
    // One label per family: a pad of its own, a run, or a group's seats —
    // sixteen identical labels on a row collide into nothing.
    let family = |st: &crate::setstone::SetStone| -> String {
        match st.source {
            crate::setstone::StoneSource::Run { .. } => st.label.clone(),
            crate::setstone::StoneSource::Pad => match st.label.rsplit_once(" / ") {
                Some((group, _)) => group.to_string(),
                None => format!("{}@{:.1}", st.label, st.theta_deg),
            },
        }
    };
    let mut labelled: std::collections::HashSet<String> = std::collections::HashSet::new();
    for (st, f) in &frames {
        let (cxs, cys) = centre(st);
        let (rs, rc) = st.rot_deg().to_radians().sin_cos();
        s.push_str("<path class=\"stone\" fill=\"#dfe\" stroke=\"black\" stroke-width=\"0.2\" d=\"");
        for k in 0..=48 {
            let t = k as f64 / 48.0 * std::f64::consts::TAU;
            let (a, b) = superellipse(t, f.semi.0, f.semi.1, f.plan_pow);
            let (x, y) = (a * rc - b * rs, a * rs + b * rc);
            s.push_str(&format!("{}{:.2},{:.2} ", if k == 0 { 'M' } else { 'L' }, cxs + x * K, cys + y * K));
        }
        s.push_str("Z\"/>\n");
        let key = family(st);
        let text = if labelled.insert(key.clone()) {
            let n = frames.iter().filter(|(o, _)| family(o) == key).count();
            let name = key.split('@').next().unwrap_or(&key);
            Some(if n > 1 { format!("{name} x{n} {:.1}", st.gem.w_mm) } else { format!("{name} {:.1}", st.gem.w_mm) })
        } else {
            None
        };
        if let Some(text) = text {
            // Kept inside the chart: a label's centre is its stone, unless
            // that would run it off an edge.
            let half = 0.45 * 1.6 * text.chars().count() as f64;
            let x = cxs.clamp(x0 + half, (x0 + chart_w - half).max(x0 + half));
            s.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"1.6\" text-anchor=\"middle\">{}</text>\n",
                x,
                cys + f.semi.1 * K + 2.0,
                esc(&text)
            ));
        }
    }
    // The census's tight pairs, drawn between the two stones with the gap that decides.
    if let Some(r) = report {
        for pair in &r.crowding {
            let find = |label: &str, theta: f64| frames.iter().find(|(st, _)| st.label == label && wrap_delta(st.theta_deg - theta, 360.0).abs() < 0.5);
            let (Some((a, _)), Some((b, _))) = (find(&pair.a, pair.a_theta_deg), find(&pair.b, pair.b_theta_deg)) else { continue };
            let (ax, ay) = centre(a);
            let (bx, by) = centre(b);
            s.push_str(&format!("<line class=\"gap\" x1=\"{ax:.2}\" y1=\"{ay:.2}\" x2=\"{bx:.2}\" y2=\"{by:.2}\" stroke=\"#c33\" stroke-width=\"0.3\"/>\n"));
            s.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{:.2}\" font-size=\"1.6\" fill=\"#c33\" text-anchor=\"middle\">gap {:.2} mm</text>\n",
                (ax + bx) * 0.5,
                (ay + by) * 0.5 - 1.0,
                pair.worst_mm()
            ));
        }
    }
    // Legend.
    let carats: f64 = frames.iter().map(|(st, _)| st.carats()).sum();
    let yl = y0 + chart_h + PAD + 4.0;
    s.push_str(&format!(
        "<text x=\"{PAD:.2}\" y=\"{yl:.2}\" font-size=\"2.4\">{} — size {} — {} stones, {:.2} ct — drawn 2:1, mm</text>\n",
        esc(&design.name),
        design.size.display(),
        frames.len(),
        carats
    ));
    s.push_str(&format!(
        "<text x=\"{PAD:.2}\" y=\"{:.2}\" font-size=\"1.8\">plan view above, the band unrolled below (u around the ring, v across); red lines are gaps under the sand's {:.2} mm fill floor</text>\n",
        yl + 3.5,
        design.draft.min_section_mm
    ));
    s.push_str("</svg>\n");
    Some(s)
}

/// A superellipse point at parameter `t`, semi-axes `a` (long) and `b`.
fn superellipse(t: f64, a: f64, b: f64, n: f64) -> (f64, f64) {
    let (st, ct) = t.sin_cos();
    let e = 2.0 / n.max(1.0);
    (a * ct.signum() * ct.abs().powf(e), b * st.signum() * st.abs().powf(e))
}

/// Writes the map; an error when the design sets no stones.
pub fn write_stone_map_svg(path: impl AsRef<std::path::Path>, design: &RingDesign, report: Option<&StonesReport>) -> anyhow::Result<usize> {
    let svg = stone_map_svg(design, report).ok_or_else(|| anyhow::anyhow!("the design sets no stones"))?;
    std::fs::write(path, svg.as_bytes())?;
    Ok(svg.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::{Layer, LayerEntry, SeatPadLayer, SeatRunLayer};
    use crate::gem::{Gem, GemCut};

    #[test]
    fn the_map_draws_every_stone_and_the_tight_pairs() {
        let mut d = RingDesign::default();
        let ctx = d.field_context();
        let v = ctx.crest_v_mm;
        let pad = |theta: f64, name: &str| {
            let mut s = SeatPadLayer { theta_deg: theta, v_mm: v, ..Default::default() };
            s.fit_stone(Gem::calibrated(GemCut::Round, 2.5));
            LayerEntry::new(name, Layer::SeatPad(s))
        };
        d.layers.layers.push(pad(90.0, "A"));
        d.layers.layers.push(pad(96.0, "B"));
        let mut run = SeatRunLayer::default();
        run.gem = Gem::calibrated(GemCut::Princess, 1.5);
        run.seat.v_mm = v;
        run.count = 12;
        run.solve_spacing(&ctx);
        d.layers.layers.push(LayerEntry::new("Row", Layer::SeatRun(run)));
        let report = crate::stones::report(&d, 0.0).unwrap();
        let stones = crate::setstone::set_stones(&d).len();
        let svg = stone_map_svg(&d, Some(&report)).unwrap();
        assert_eq!(svg.matches("class=\"stone\"").count(), 2 * stones, "plan and chart, one path each per stone");
        assert!(svg.contains(">A 2.5<") && svg.contains(">B 2.5<"), "labels with sizes");
        let in_row = crate::setstone::set_stones(&d).iter().filter(|s| s.label == "Row").count();
        assert_eq!(svg.matches(&format!("Row x{in_row} ")).count(), 1, "a run is labelled once, with its count");
        assert!(svg.contains("class=\"gap\"") && svg.contains("gap "), "A and B are 6 degrees apart: a tight pair");
        assert!(svg.contains("drawn 2:1"));
        let dir = std::env::temp_dir().join(format!("rd-map-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let n = write_stone_map_svg(dir.join("m.svg"), &d, Some(&report)).unwrap();
        assert!(n > 1000);
        let _ = std::fs::remove_dir_all(dir);
        assert!(stone_map_svg(&RingDesign::default(), None).is_none(), "no stones, no map");
    }
}
