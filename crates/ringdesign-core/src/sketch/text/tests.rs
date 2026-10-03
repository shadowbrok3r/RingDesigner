use super::*;
use crate::sketch::Region;

/// Every point a piece passes through, densely.
fn samples(p: &Piece, n: usize) -> Vec<P> {
    match p {
        Piece::Line(l) => (0..=n).map(|k| add(l[0], scale(sub(l[1], l[0]), k as f64 / n as f64))).collect(),
        Piece::Cubic(c) => (0..=n).map(|k| bezier(c, k as f64 / n as f64)).collect(),
    }
}

/// How far the furthest sample of `a` lies from the polyline through `b`'s samples.
fn deviation(a: &[Piece], b: &[Piece]) -> f64 {
    let line: Vec<P> = b.iter().flat_map(|p| samples(p, 64)).collect();
    a.iter()
        .flat_map(|p| samples(p, 32))
        .map(|q| {
            line.windows(2)
                .map(|w| {
                    let d = sub(w[1], w[0]);
                    let t = (dot(sub(q, w[0]), d) / dot(d, d).max(1e-300)).clamp(0.0, 1.0);
                    len(sub(q, add(w[0], scale(d, t))))
                })
                .fold(f64::INFINITY, f64::min)
        })
        .fold(0.0, f64::max)
}

/// The glyph for `ch` in `font`'s own outline, every contour as ttf-parser draws it.
fn drawn(font: TextFont, ch: char) -> (Vec<Vec<Piece>>, f64) {
    let face = ttf_parser::Face::parse(font.bytes(), 0).unwrap();
    let mut outline = Outline::default();
    face.outline_glyph(face.glyph_index(ch).unwrap(), &mut outline);
    if !outline.current.is_empty() {
        outline.finish();
    }
    (outline.contours, f64::from(face.units_per_em()))
}

fn regions(s: &Sketch) -> Vec<Region> {
    s.sweep_regions().unwrap()
}

#[test]
fn a_refitted_glyph_stays_within_its_tolerance_and_costs_fewer_points() {
    let (mut before, mut after) = (0, 0);
    for ch in "SIGILLVM CAPITVLI memento mori".chars().filter(|c| !c.is_whitespace()) {
        let (contours, em) = drawn(TextFont::Textura, ch);
        let tolerance = FIT_TOLERANCE_EM * em;
        for c in &contours {
            let r = refitted(c, tolerance);
            // The refit closes on itself, keeps every corner it started from, and stays on the outline both ways.
            assert_eq!(r.first().unwrap().start(), r.last().unwrap().end(), "{ch:?}");
            for w in r.windows(2) {
                assert_eq!(w[0].end(), w[1].start(), "{ch:?}");
            }
            let (there, back) = (deviation(c, &r), deviation(&r, c));
            assert!(there <= 1.05 * tolerance && back <= 1.05 * tolerance, "{ch:?}: {there:.3} and {back:.3} font units against {tolerance}");
            before += c.iter().map(Piece::cost).sum::<usize>();
            after += r.iter().map(Piece::cost).sum::<usize>();
        }
    }
    eprintln!("refitting at {FIT_TOLERANCE_EM} em: {before} points to {after}");
    assert!(after < before * 3 / 4, "{before} to {after}");
}

#[test]
fn a_word_sweeps_as_closed_regions_with_its_counters_as_holes() {
    // An o is a ring: one region with its counter a hole. An i is a stem and a dot: two regions.
    let o = text(TextFont::Textura, "o", 2.0, 0.0, None).unwrap();
    let r = regions(&o);
    assert_eq!((r.len(), r[0].holes.len()), (1, 1), "{:?}", r.iter().map(|r| r.holes.len()).collect::<Vec<_>>());
    let i = text(TextFont::Textura, "i", 2.0, 0.0, None).unwrap();
    let r = regions(&i);
    assert!(r.len() == 2 && r.iter().all(|r| r.holes.is_empty()), "{}", r.len());
    // A capital stands its cap height on the baseline.
    let h = text(TextFont::Textura, "I", 1.2, 0.0, None).unwrap();
    let top = h.points.iter().map(|p| p.xy[1]).fold(f64::MIN, f64::max);
    assert!((top - 1.2).abs() < 0.15, "an I of a 1.2 mm capital stands {top:.3} mm");
    let word = text(TextFont::Textura, "SIGILLVM", 1.2, 0.05, None).unwrap();
    assert!(word.points.len() <= MAX_ITEMS && word.entities.len() <= MAX_ITEMS);
    assert!(word.entities.iter().all(|e| matches!(e.geometry, Geometry::Line { .. } | Geometry::Bezier { .. })));
    let r = regions(&word);
    let area: f64 = r.iter().map(Region::area).sum();
    eprintln!("SIGILLVM at 1.2 mm: {} points, {} entities, {} regions, {area:.3} mm² of ink", word.points.len(), word.entities.len(), r.len());
    assert!(r.len() >= 8 && area > 2.0 && area < 12.0, "{} regions, {area}", r.len());
    assert_eq!(word.name, "SIGILLVM");
}

#[test]
fn every_letter_of_every_font_sweeps_on_its_own() {
    let printable: String = (0x21u8..0x7f).map(char::from).collect();
    for font in TextFont::ALL {
        let mut refused = Vec::new();
        for ch in printable.chars() {
            if let Err(e) = text(*font, &ch.to_string(), 2.0, 0.0, None) {
                refused.push(format!("{ch:?}: {e:#}"));
            }
        }
        eprintln!("{}: {} of {} letters refused", font.label(), refused.len(), printable.len());
        for r in &refused {
            eprintln!("  {r}");
        }
        if *font == TextFont::Textura {
            assert!(refused.is_empty(), "{refused:#?}");
        }
    }
}

#[test]
fn a_legend_too_long_for_one_sketch_is_set_a_word_at_a_time_where_each_falls() {
    let legend = TextLayout { tracking: 0.3, ..TextLayout::new(TextFont::Textura, "SIGILLVM CAPITVLI", 1.2) };
    let e = legend.sketch().unwrap_err().to_string();
    assert!(e.contains("over the 1024 one sketch takes") && e.contains("2 parts"), "{e}");
    let parts = legend.parts().unwrap();
    assert_eq!(parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["SIGILLVM", "CAPITVLI"]);
    assert_eq!(legend.part_count().unwrap(), 2);
    assert_eq!(legend.part(1).unwrap(), parts[1]);
    assert!(legend.part(2).unwrap_err().to_string().contains("there is no part 2"));
    // Each part stands where it falls in the whole: the second word starts past the first word and the space.
    let first = TextLayout { tracking: 0.3, ..TextLayout::new(TextFont::Textura, "SIGILLVM ", 1.2) }.length_mm().unwrap();
    let left = parts[1].points.iter().map(|p| p.xy[0]).fold(f64::MAX, f64::min);
    assert!(left > first && left < first + 0.6, "CAPITVLI starts at {left:.3} after {first:.3}");
    // A text short enough for one sketch is its parts together, point for point.
    let short = TextLayout { tracking: 0.1, ..TextLayout::new(TextFont::Textura, "AVE MARIA", 1.0) };
    let whole = short.sketch().unwrap();
    let mut together: Vec<[f64; 2]> = short.parts().unwrap().iter().flat_map(|p| p.points.iter().map(|q| q.xy)).collect();
    let mut all: Vec<[f64; 2]> = whole.points.iter().map(|q| q.xy).collect();
    let key = |a: &[f64; 2], b: &[f64; 2]| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]));
    together.sort_by(key);
    all.sort_by(key);
    assert_eq!(together, all);
}

#[test]
fn a_seal_legend_runs_round_its_circle_and_mirrors() {
    let (r, cap) = (6.2, 1.2);
    let arc = TextArc { radius_mm: r, start_deg: 90.0, clockwise: true };
    let mut legend = TextLayout { arc: Some(arc), ..TextLayout::new(TextFont::Textura, "\u{2720} SIGILLVM CAPITVLI", cap) };
    // Tracking that closes the circle but for a tenth of it.
    let span = 324.0_f64.to_radians() * r;
    legend.tracking = legend.tracking_for(span).unwrap();
    assert!((legend.length_mm().unwrap() - span).abs() < 1e-9);
    let parts = legend.parts().unwrap();
    assert_eq!(parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["\u{2720}", "SIGILLVM", "CAPITVLI"]);
    // Upright and outward: every letter lies in the ring from a little under the baseline to a little over its capitals.
    for p in parts.iter().flat_map(|s| s.points.iter()) {
        let d = p.xy[0].hypot(p.xy[1]);
        assert!(d > r - 0.6 * cap && d < r + 1.8 * cap, "{:?} at {d:.3}", p.xy);
    }
    // The cross stands just past the top, the legend runs clockwise from it, and every point of it within the span.
    let behind = |xy: [f64; 2]| (90.0 - xy[1].atan2(xy[0]).to_degrees()).rem_euclid(360.0);
    let middle = |s: &Sketch| {
        let c = s.points.iter().fold([0.0; 2], |m, p| add(m, p.xy));
        behind(scale(c, 1.0 / s.points.len() as f64))
    };
    let (cross, first, last) = (middle(&parts[0]), middle(&parts[1]), middle(&parts[2]));
    eprintln!("tracking {:.3} em: the cross {cross:.1}°, SIGILLVM {first:.1}°, CAPITVLI {last:.1}° clockwise from the top", legend.tracking);
    assert!(cross > 0.0 && cross < 10.0 && cross < first && first < last && last < 324.0, "{cross} {first} {last}");
    for p in parts.iter().flat_map(|s| s.points.iter()) {
        assert!(behind(p.xy) < 326.0 || behind(p.xy) > 358.0, "{:?} lies {:.1}° round", p.xy, behind(p.xy));
    }
    for p in &parts {
        regions(p);
    }
    // Mirrored for the seal's impression: the same points reflected in the sketch's y axis.
    let mirrored = TextLayout { mirror: true, ..legend.clone() }.parts().unwrap();
    for (a, b) in parts.iter().zip(&mirrored) {
        assert_eq!(a.points.len(), b.points.len());
        for (p, q) in a.points.iter().zip(&b.points) {
            assert_eq!((p.xy[0], p.xy[1]), (-q.xy[0], q.xy[1]));
        }
        regions(b);
    }
    // Counter-clockwise the letters stand inward, under the baseline: their tops close up, so they take some tracking.
    let inward = TextLayout { arc: Some(TextArc { radius_mm: r, start_deg: 270.0, clockwise: false }), align: TextAlign::Centre, ..TextLayout::new(TextFont::Textura, "MORI", cap) };
    assert!(inward.sketch().unwrap_err().to_string().contains("'M' (character 1) and 'O' (character 2) touch"));
    let under = TextLayout { tracking: 0.15, ..inward };
    let s = under.sketch().unwrap();
    for p in &s.points {
        let d = p.xy[0].hypot(p.xy[1]);
        assert!(d < r + 0.6 * cap && d > r - 1.8 * cap, "{:?} at {d:.3}", p.xy);
    }
    let c = scale(s.points.iter().fold([0.0; 2], |m, p| add(m, p.xy)), 1.0 / s.points.len() as f64);
    assert!(c[0].abs() < 0.4 && c[1] < -r + 1.6 * cap, "centred under the origin: {c:?}");
}

#[test]
fn text_is_refused_by_name_where_it_cannot_be_drawn() {
    let e = text(TextFont::Textura, "\u{292}", 1.0, 0.0, None).unwrap_err().to_string();
    assert!(e.contains("Textura (UnifrakturMaguntia) has no letter for") && e.contains("U+0292"), "{e}");
    assert!(text(TextFont::Textura, "  ", 1.0, 0.0, None).unwrap_err().to_string().contains("no text"));
    assert!(text(TextFont::Textura, "A\nB", 1.0, 0.0, None).unwrap_err().to_string().contains("control character"));
    assert!(text(TextFont::Textura, "AB", 0.0, 0.0, None).is_err());
    // Letters pulled through one another touch, and the message names them.
    let e = text(TextFont::Textura, "MM", 1.0, -0.45, None).unwrap_err().to_string();
    assert!(e.contains("'M' (character 1) and 'M' (character 2) touch") && e.contains("open the tracking"), "{e}");
    // More than a turn round a small circle.
    let e = text(TextFont::Textura, "SIGILLVM", 1.2, 0.0, Some(TextArc { radius_mm: 1.0, start_deg: 90.0, clockwise: true })).unwrap_err().to_string();
    assert!(e.contains("one turn at most"), "{e}");
}

#[test]
fn a_text_sketch_extrudes_cuts_and_picks_like_any_other() {
    use crate::cad::{Component, Document, Feature, Operation, Profile, evaluate};
    use crate::sketch::RegionRef;
    use crate::{AlphaLibrary, BuildParams, RingDesign};
    let s = text(TextFont::Textura, "MORI", 1.6, 0.08, None).unwrap();
    let all = regions(&s);
    let ink: f64 = all.iter().map(Region::area).sum();
    let feature = |id, name: &str, operation| Feature { id, name: name.into(), enabled: true, operation, component: Component::default() };
    let design = |operation: Operation| {
        let mut doc = Document::default();
        doc.append(feature(1, "Legend", Operation::Sketch { sketch: s.clone() })).unwrap();
        doc.append(feature(2, "Cut the legend", operation)).unwrap();
        RingDesign { cad: Some(doc), ..RingDesign::default() }
    };
    let (lib, params) = (AlphaLibrary::builtin(), BuildParams::default());
    // The whole sketch: one lump per letter's region, counters through, its ink times its depth.
    let started = std::time::Instant::now();
    let e = evaluate(&design(Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: -0.4, draft_deg: 0.0 }), &lib, params).unwrap();
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let c = &e.components[0];
    eprintln!("MORI cut 0.4 deep: {} lumps, {:.4} mm³ against {:.4}, {} faces, in {:.0} ms", c.body.roots.len(), c.mesh.volume_mm3(), ink * 0.4, c.mesh.faces.len(), started.elapsed().as_secs_f64() * 1e3);
    assert!(c.mesh.validate().watertight && c.body.roots.len() == all.len());
    assert!((c.mesh.volume_mm3() / (ink * 0.4) - 1.0).abs() < 0.01, "{} against {}", c.mesh.volume_mm3(), ink * 0.4);
    // Two of its regions picked by name.
    let picks: Vec<RegionRef> = (0..2).map(|i| RegionRef::among(&all, i, all[i].inside().unwrap()).unwrap()).collect();
    let e = evaluate(&design(Operation::Extrude { sketch: Profile::Regions { feature: 1, regions: picks }, height_mm: 0.3, draft_deg: 0.0 }), &lib, params).unwrap();
    assert!(e.failures().is_empty(), "{:?}", e.failures());
    let v = e.components[0].mesh.volume_mm3();
    let want = (all[0].area() + all[1].area()) * 0.3;
    assert!((v / want - 1.0).abs() < 0.01, "{v} against {want}");
}

/// Every Textura letter cuts as a closed solid. The kernel samples a Bézier wall's edges more finely than the flat
/// cap beside it; `V`, `A` and `P` left cracks that the cap's own triangles are now split along.
#[test]
fn every_letter_cuts_as_a_closed_solid() {
    use crate::cad::{Component, Document, Feature, Operation, Profile, evaluate};
    use crate::{AlphaLibrary, BuildParams, RingDesign};
    let lib = AlphaLibrary::builtin();
    let started = std::time::Instant::now();
    let mut failed = Vec::new();
    for ch in ('A'..='Z').chain('a'..='z') {
        let s = text(TextFont::Textura, &ch.to_string(), 1.2, 0.0, None).unwrap();
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Letter".into(), enabled: true, operation: Operation::Sketch { sketch: s }, component: Component::default() }).unwrap();
        let cut = Operation::Extrude { sketch: Profile::Feature { feature: 1 }, height_mm: -0.4, draft_deg: 0.0 };
        doc.append(Feature { id: 2, name: "Cut".into(), enabled: true, operation: cut, component: Component::default() }).unwrap();
        let e = evaluate(&RingDesign { cad: Some(doc), ..RingDesign::default() }, &lib, BuildParams::default()).unwrap();
        match e.failures().first() {
            Some(f) => failed.push(format!("{ch:?}: {f:?}")),
            None => assert!(e.components[0].mesh.validate().watertight, "{ch:?}"),
        }
    }
    eprintln!("52 letters cut in {:.1} s", started.elapsed().as_secs_f64());
    assert!(failed.is_empty(), "{failed:#?}");
}

#[test]
fn every_font_is_named_and_reads() {
    for font in TextFont::ALL {
        let face = ttf_parser::Face::parse(font.bytes(), 0).unwrap();
        assert!(face.units_per_em() >= 1000 && face.glyph_index('A').is_some(), "{}", font.label());
    }
    assert_eq!(TextFont::ALL.len(), 3);
    assert_eq!(serde_json::to_string(&TextFont::Textura).unwrap(), "\"Textura\"");
}
