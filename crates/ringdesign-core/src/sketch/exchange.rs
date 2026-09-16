//! Vector profile interchange: SVG uses +Y down, DXF uses +Y up. Millimeter
//! model coordinates are preserved; unsupported entities fail explicitly.
use super::{Geometry, Sketch};
use crate::manufacturing::package::escape;
use anyhow::{Context, Result, bail, ensure};
use std::fmt::Write;

pub fn svg(s: &Sketch) -> Result<String> {
    s.validate()?;
    let mut lo = [f64::INFINITY; 2];
    let mut hi = [f64::NEG_INFINITY; 2];
    for e in &s.entities {
        for c in s.curves_of(e)? {
            for p in c.tessellate_within(0.005) {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    ensure!(lo[0].is_finite(), "Sketch is empty");
    let w = (hi[0] - lo[0] + 2.0).max(2.0);
    let h = (hi[1] - lo[1] + 2.0).max(2.0);
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}mm\" height=\"{h}mm\" viewBox=\"{} {} {w} {h}\"><title>{}</title>",
        lo[0] - 1.0,
        -hi[1] - 1.0,
        escape(&s.name)
    );
    for e in &s.entities {
        let style = if e.construction {
            "fill=\"none\" stroke=\"#888\" stroke-width=\"0.1\" stroke-dasharray=\"0.5 0.5\" data-construction=\"true\""
        } else {
            "fill=\"none\" stroke=\"black\" stroke-width=\"0.1\""
        };
        match &e.geometry {
            Geometry::Circle { center, rim } => {
                let c = s.at(*center)?;
                let r = super::distance(c, s.at(*rim)?);
                write!(
                    out,
                    "<circle cx=\"{}\" cy=\"{}\" r=\"{r}\" {style}/>",
                    c[0], -c[1]
                )?;
            }
            _ => {
                let mut path = String::new();
                match &e.geometry {
                    Geometry::Line { a, b } => {
                        let a = s.at(*a)?;
                        let b = s.at(*b)?;
                        write!(path, "M {} {} L {} {}", a[0], -a[1], b[0], -b[1])?;
                    }
                    Geometry::Polyline { points, closed } => {
                        for (i, id) in points.iter().enumerate() {
                            let p = s.at(*id)?;
                            write!(
                                path,
                                "{} {} {} ",
                                if i == 0 { "M" } else { "L" },
                                p[0],
                                -p[1]
                            )?;
                        }
                        if *closed {
                            path.push('Z');
                        }
                    }
                    Geometry::Arc { center, start, end } => {
                        let c = s.at(*center)?;
                        let a = s.at(*start)?;
                        let b = s.at(*end)?;
                        let r = super::distance(c, a);
                        let span = ((b[1] - c[1]).atan2(b[0] - c[0])
                            - (a[1] - c[1]).atan2(a[0] - c[0]))
                        .rem_euclid(std::f64::consts::TAU);
                        write!(
                            path,
                            "M {} {} A {r} {r} 0 {} 0 {} {}",
                            a[0],
                            -a[1],
                            u8::from(span > std::f64::consts::PI),
                            b[0],
                            -b[1]
                        )?;
                    }
                    Geometry::Bezier { points } => {
                        let p = points
                            .iter()
                            .map(|id| s.at(*id))
                            .collect::<Result<Vec<_>>>()?;
                        write!(
                            path,
                            "M {} {} C {} {} {} {} {} {}",
                            p[0][0],
                            -p[0][1],
                            p[1][0],
                            -p[1][1],
                            p[2][0],
                            -p[2][1],
                            p[3][0],
                            -p[3][1]
                        )?;
                    }
                    _ => {}
                }
                write!(out, "<path d=\"{path}\" {style}/>")?;
            }
        }
    }
    out.push_str("</svg>");
    Ok(out)
}
fn length(v: &str) -> Result<f64> {
    let v = v.trim();
    for (unit, scale) in [
        ("mm", 1.0),
        ("cm", 10.0),
        ("in", 25.4),
        ("pt", 25.4 / 72.0),
        ("px", 25.4 / 96.0),
    ] {
        if let Some(n) = v.strip_suffix(unit) {
            return Ok(n.trim().parse::<f64>()? * scale);
        }
    }
    bail!("SVG width needs a physical unit (mm, cm, in, pt, or px)")
}
fn point(s: &mut Sketch, xy: [f64; 2]) -> u64 {
    s.points
        .iter()
        .find(|p| super::distance(p.xy, xy) < 1e-8)
        .map(|p| p.id)
        .unwrap_or_else(|| s.point(xy))
}
fn add(s: &mut Sketch, geometry: Geometry, construction: bool) {
    let id = s.entity(geometry);
    s.entities
        .iter_mut()
        .find(|e| e.id == id)
        .unwrap()
        .construction = construction;
}

pub fn import_svg(text: &str) -> Result<Sketch> {
    ensure!(text.len() < 5_000_000, "SVG is too large");
    let doc = roxmltree::Document::parse(text)?;
    let root = doc.root_element();
    ensure!(root.tag_name().name() == "svg", "Expected SVG");
    let vb = root
        .attribute("viewBox")
        .map(|v| {
            v.split(|c: char| c.is_whitespace() || c == ',')
                .filter(|v| !v.is_empty())
                .map(str::parse::<f64>)
                .collect::<std::result::Result<Vec<_>, _>>()
        })
        .transpose()?;
    let scale = if let Some(v) = &vb {
        ensure!(
            v.len() == 4 && v[2] > 0.0 && v[3] > 0.0,
            "Invalid SVG viewBox"
        );
        let width = length(
            root.attribute("width")
                .context("SVG needs width with units")?,
        )?;
        let height = length(
            root.attribute("height")
                .context("SVG needs height with units")?,
        )?;
        ensure!(
            (width / v[2] - height / v[3]).abs() < 1e-6,
            "Nonuniform SVG scale is unsupported; export a uniform profile"
        );
        width / v[2]
    } else {
        1.0
    };
    ensure!(scale.is_finite() && scale > 0.0, "Invalid SVG scale");
    let mut s = Sketch::default();
    s.name = "Imported SVG profile".into();
    for n in root.descendants().filter(|n| n.is_element()) {
        ensure!(
            n.attribute("transform").is_none(),
            "Apply SVG transforms before importing the profile"
        );
        let val = |key| -> Result<f64> {
            Ok(n.attribute(key)
                .context(format!("SVG element needs {key}"))?
                .parse::<f64>()?
                * scale)
        };
        let construction = n.attribute("data-construction") == Some("true");
        match n.tag_name().name() {
            "svg" | "g" | "title" | "desc" | "metadata" => {}
            "circle" => {
                let c = [val("cx")?, -val("cy")?];
                let r = val("r")?;
                let center = point(&mut s, c);
                let rim = point(&mut s, [c[0] + r, c[1]]);
                add(&mut s, Geometry::Circle { center, rim }, construction);
            }
            "line" => {
                let a = point(&mut s, [val("x1")?, -val("y1")?]);
                let b = point(&mut s, [val("x2")?, -val("y2")?]);
                add(&mut s, Geometry::Line { a, b }, construction);
            }
            "polyline" | "polygon" => {
                let values = n
                    .attribute("points")
                    .context("Missing points")?
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|s| !s.is_empty())
                    .map(str::parse::<f64>)
                    .collect::<std::result::Result<Vec<_>, _>>()?;
                ensure!(values.len() % 2 == 0, "Unpaired polygon coordinates");
                let points = values
                    .chunks_exact(2)
                    .map(|p| point(&mut s, [p[0] * scale, -p[1] * scale]))
                    .collect();
                add(
                    &mut s,
                    Geometry::Polyline {
                        points,
                        closed: n.tag_name().name() == "polygon",
                    },
                    construction,
                );
            }
            "path" => {
                use svgtypes::PathSegment as P;
                let mut cursor = [0.0; 2];
                let mut start = cursor;
                for segment in
                    svgtypes::PathParser::from(n.attribute("d").context("Missing path data")?)
                {
                    let to = |x: f64, y: f64, abs: bool| {
                        if abs {
                            [x * scale, -y * scale]
                        } else {
                            [cursor[0] + x * scale, cursor[1] - y * scale]
                        }
                    };
                    match segment? {
                        P::MoveTo { abs, x, y } => {
                            cursor = to(x, y, abs);
                            start = cursor;
                        }
                        P::LineTo { abs, x, y } => {
                            let end = to(x, y, abs);
                            let a = point(&mut s, cursor);
                            let b = point(&mut s, end);
                            add(&mut s, Geometry::Line { a, b }, construction);
                            cursor = end;
                        }
                        P::HorizontalLineTo { abs, x } => {
                            let end = [
                                if abs {
                                    x * scale
                                } else {
                                    cursor[0] + x * scale
                                },
                                cursor[1],
                            ];
                            let a = point(&mut s, cursor);
                            let b = point(&mut s, end);
                            add(&mut s, Geometry::Line { a, b }, construction);
                            cursor = end;
                        }
                        P::VerticalLineTo { abs, y } => {
                            let end = [
                                cursor[0],
                                if abs {
                                    -y * scale
                                } else {
                                    cursor[1] - y * scale
                                },
                            ];
                            let a = point(&mut s, cursor);
                            let b = point(&mut s, end);
                            add(&mut s, Geometry::Line { a, b }, construction);
                            cursor = end;
                        }
                        P::CurveTo {
                            abs,
                            x1,
                            y1,
                            x2,
                            y2,
                            x,
                            y,
                        } => {
                            let p = [cursor, to(x1, y1, abs), to(x2, y2, abs), to(x, y, abs)];
                            let points = p.map(|p| point(&mut s, p));
                            add(&mut s, Geometry::Bezier { points }, construction);
                            cursor = p[3];
                        }
                        P::Quadratic { abs, x1, y1, x, y } => {
                            let c = to(x1, y1, abs);
                            let end = to(x, y, abs);
                            let p = [
                                cursor,
                                [
                                    (cursor[0] + 2.0 * c[0]) / 3.0,
                                    (cursor[1] + 2.0 * c[1]) / 3.0,
                                ],
                                [(end[0] + 2.0 * c[0]) / 3.0, (end[1] + 2.0 * c[1]) / 3.0],
                                end,
                            ];
                            let points = p.map(|p| point(&mut s, p));
                            add(&mut s, Geometry::Bezier { points }, construction);
                            cursor = end;
                        }
                        P::EllipticalArc {
                            abs,
                            rx,
                            ry,
                            x_axis_rotation: _,
                            large_arc,
                            sweep,
                            x,
                            y,
                        } => {
                            ensure!(
                                (rx - ry).abs() < 1e-8 && rx > 0.0,
                                "Only circular SVG arcs are supported; convert ellipses to cubic paths"
                            );
                            let end = to(x, y, abs);
                            let r = rx * scale;
                            let half = super::distance(cursor, end) * 0.5;
                            ensure!(half > 1e-9 && r + 1e-8 >= half, "Invalid circular arc");
                            let d = [end[0] - cursor[0], end[1] - cursor[1]];
                            let h = (r * r - half * half).max(0.0).sqrt();
                            let sign = if large_arc == sweep { 1.0 } else { -1.0 };
                            let c = [
                                (cursor[0] + end[0]) / 2.0 - sign * d[1] / (2.0 * half) * h,
                                (cursor[1] + end[1]) / 2.0 + sign * d[0] / (2.0 * half) * h,
                            ];
                            let center = point(&mut s, c);
                            let a = point(&mut s, cursor);
                            let b = point(&mut s, end);
                            add(
                                &mut s,
                                Geometry::Arc {
                                    center,
                                    start: if sweep { b } else { a },
                                    end: if sweep { a } else { b },
                                },
                                construction,
                            );
                            cursor = end;
                        }
                        P::ClosePath { .. } => {
                            if super::distance(cursor, start) > 1e-8 {
                                let a = point(&mut s, cursor);
                                let b = point(&mut s, start);
                                add(&mut s, Geometry::Line { a, b }, construction);
                            }
                            cursor = start;
                        }
                        _ => bail!("Expand shorthand smooth SVG commands before importing"),
                    }
                }
            }
            other => bail!("SVG element {other} is not a supported manufacturing profile"),
        }
    }
    s.validate()?;
    ensure!(!s.entities.is_empty(), "No supported profile geometry");
    Ok(s)
}

pub fn dxf(s: &Sketch) -> Result<String> {
    s.validate()?;
    let mut out="0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n".to_string();
    for e in &s.entities {
        let layer = if e.construction {
            "CONSTRUCTION"
        } else {
            "PROFILE"
        };
        let mut emit = |g: &Geometry| -> Result<()> {
            match g {
                Geometry::Line { a, b } => {
                    let a = s.at(*a)?;
                    let b = s.at(*b)?;
                    write!(
                        out,
                        "0\nLINE\n100\nAcDbEntity\n8\n{layer}\n100\nAcDbLine\n10\n{}\n20\n{}\n11\n{}\n21\n{}\n",
                        a[0], a[1], b[0], b[1]
                    )?;
                }
                Geometry::Polyline { points, closed } => {
                    write!(
                        out,
                        "0\nLWPOLYLINE\n100\nAcDbEntity\n8\n{layer}\n100\nAcDbPolyline\n90\n{}\n70\n{}\n",
                        points.len(),
                        u8::from(*closed)
                    )?;
                    for id in points {
                        let p = s.at(*id)?;
                        write!(out, "10\n{}\n20\n{}\n", p[0], p[1])?;
                    }
                }
                Geometry::Circle { center, rim } => {
                    let c = s.at(*center)?;
                    write!(
                        out,
                        "0\nCIRCLE\n100\nAcDbEntity\n8\n{layer}\n100\nAcDbCircle\n10\n{}\n20\n{}\n40\n{}\n",
                        c[0],
                        c[1],
                        super::distance(c, s.at(*rim)?)
                    )?;
                }
                Geometry::Arc { center, start, end } => {
                    let c = s.at(*center)?;
                    let a = s.at(*start)?;
                    let b = s.at(*end)?;
                    write!(
                        out,
                        "0\nARC\n100\nAcDbEntity\n8\n{layer}\n100\nAcDbCircle\n10\n{}\n20\n{}\n40\n{}\n100\nAcDbArc\n50\n{}\n51\n{}\n",
                        c[0],
                        c[1],
                        super::distance(c, a),
                        (a[1] - c[1]).atan2(a[0] - c[0]).to_degrees(),
                        (b[1] - c[1]).atan2(b[0] - c[0]).to_degrees()
                    )?;
                }
                Geometry::Bezier { points } => {
                    write!(
                        out,
                        "0\nSPLINE\n100\nAcDbEntity\n8\n{layer}\n100\nAcDbSpline\n70\n8\n71\n3\n72\n8\n73\n4\n"
                    )?;
                    for knot in [0, 0, 0, 0, 1, 1, 1, 1] {
                        write!(out, "40\n{knot}\n")?;
                    }
                    for id in points {
                        let p = s.at(*id)?;
                        write!(out, "10\n{}\n20\n{}\n30\n0\n", p[0], p[1])?;
                    }
                }
            }
            Ok(())
        };
        emit(&e.geometry)?;
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    Ok(out)
}
pub fn import_dxf(text: &str) -> Result<Sketch> {
    ensure!(text.len() < 5_000_000, "DXF is too large");
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    ensure!(lines.len() % 2 == 0, "Incomplete DXF group pair");
    let pairs = lines
        .chunks_exact(2)
        .map(|p| Ok((p[0].parse::<i32>()?, p[1])))
        .collect::<Result<Vec<_>>>()?;
    let unit = pairs
        .windows(2)
        .find(|p| p[0] == (9, "$INSUNITS"))
        .map(|p| p[1].1)
        .context("DXF needs $INSUNITS; save the profile with millimeter units")?;
    let scale = match unit {
        "4" => 1.0,
        "1" => 25.4,
        "5" => 10.0,
        _ => bail!("Unsupported DXF units {unit}"),
    };
    let mut s = Sketch::default();
    s.name = "Imported DXF profile".into();
    let mut in_entities = false;
    let mut i = 0;
    while i < pairs.len() {
        if pairs[i] == (2, "ENTITIES") {
            in_entities = true;
            i += 1;
            continue;
        }
        if !in_entities || pairs[i].0 != 0 {
            i += 1;
            continue;
        }
        let kind = pairs[i].1;
        if kind == "ENDSEC" {
            break;
        }
        let start = i + 1;
        i = start;
        while i < pairs.len() && pairs[i].0 != 0 {
            i += 1;
        }
        let values = &pairs[start..i];
        let value = |code: i32| -> Result<f64> {
            Ok(values
                .iter()
                .find(|(c, _)| *c == code)
                .with_context(|| format!("DXF {kind} lacks group {code}"))?
                .1
                .parse::<f64>()?)
        };
        let xy = |x, y| -> Result<[f64; 2]> { Ok([value(x)? * scale, value(y)? * scale]) };
        let construction = values.contains(&(8, "CONSTRUCTION"));
        let geometry = match kind {
            "LINE" => {
                let a = point(&mut s, xy(10, 20)?);
                let b = point(&mut s, xy(11, 21)?);
                Geometry::Line { a, b }
            }
            "CIRCLE" | "ARC" => {
                let c = xy(10, 20)?;
                let radius = value(40)? * scale;
                let center = point(&mut s, c);
                if kind == "CIRCLE" {
                    let rim = point(&mut s, [c[0] + radius, c[1]]);
                    Geometry::Circle { center, rim }
                } else {
                    let a = value(50)?.to_radians();
                    let b = value(51)?.to_radians();
                    let start = point(&mut s, [c[0] + radius * a.cos(), c[1] + radius * a.sin()]);
                    let end = point(&mut s, [c[0] + radius * b.cos(), c[1] + radius * b.sin()]);
                    Geometry::Arc { center, start, end }
                }
            }
            "LWPOLYLINE" | "SPLINE" => {
                ensure!(
                    !values
                        .iter()
                        .any(|(c, v)| *c == 42 && v.parse::<f64>().unwrap_or(0.0) != 0.0),
                    "Expand bulged DXF polylines into arcs"
                );
                let xs: Vec<_> = values
                    .iter()
                    .filter(|(c, _)| *c == 10)
                    .map(|(_, v)| v.parse::<f64>())
                    .collect::<std::result::Result<_, _>>()?;
                let ys: Vec<_> = values
                    .iter()
                    .filter(|(c, _)| *c == 20)
                    .map(|(_, v)| v.parse::<f64>())
                    .collect::<std::result::Result<_, _>>()?;
                ensure!(xs.len() == ys.len(), "Unpaired DXF coordinates");
                let points: Vec<_> = xs
                    .into_iter()
                    .zip(ys)
                    .map(|(x, y)| point(&mut s, [x * scale, y * scale]))
                    .collect();
                if kind == "SPLINE" {
                    ensure!(
                        value(71)? == 3.0
                            && points.len() == 4
                            && !values.iter().any(|(c, _)| *c == 41),
                        "Only nonrational cubic Bezier DXF splines are supported"
                    );
                    let knots: Vec<_> = values
                        .iter()
                        .filter(|(c, _)| *c == 40)
                        .map(|(_, v)| v.parse::<f64>())
                        .collect::<std::result::Result<_, _>>()?;
                    ensure!(
                        knots == vec![0., 0., 0., 0., 1., 1., 1., 1.],
                        "Unsupported spline knot vector"
                    );
                    Geometry::Bezier {
                        points: points.try_into().unwrap(),
                    }
                } else {
                    Geometry::Polyline {
                        points,
                        closed: value(70).unwrap_or(0.0) as i32 & 1 != 0,
                    }
                }
            }
            _ => bail!("DXF entity {kind} is unsupported for profile import"),
        };
        add(&mut s, geometry, construction);
    }
    s.validate()?;
    ensure!(!s.entities.is_empty(), "No supported DXF profile geometry");
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangle_and_circle_vector_roundtrips_preserve_mm() {
        for source in [Sketch::rectangle(8.0, 6.0), Sketch::circle(3.2)] {
            for parsed in [
                import_svg(&svg(&source).unwrap()).unwrap(),
                import_dxf(&dxf(&source).unwrap()).unwrap(),
            ] {
                let area = |s: &Sketch| {
                    s.curves()
                        .unwrap()
                        .iter()
                        .map(|c| c.enclosed_area())
                        .sum::<f64>()
                };
                assert!((area(&source).abs() - area(&parsed).abs()).abs() < 1e-6);
            }
        }
    }
    #[test]
    fn unsupported_transforms_and_missing_units_are_refused() {
        assert!(import_svg("<svg><g transform=\"scale(2)\"/></svg>").is_err());
        assert!(import_dxf("0\nSECTION\n2\nENTITIES\n0\nENDSEC\n").is_err());
    }
}
