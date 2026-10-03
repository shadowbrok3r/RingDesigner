//! Text set as sketch outlines: every glyph's closed contours as lines and cubic Béziers, laid on a
//! straight baseline or round a circle, so a legend is a sketch an Extrude, a cut or a region pick
//! takes like any other. A counter is a hole by the region builder's even-odd nesting.
//!
//! A TrueType glyph is drawn in quadratics, and one quadratic as one cubic costs three of the
//! sketch's [`MAX_ITEMS`] points, so a Textura capital would cost a hundred. Runs of quadratics
//! that join smoothly are refitted as fewer cubics whose ends keep the run's own tangents, held to
//! [`FIT_TOLERANCE_EM`] of the em; a corner and a straight line are kept exactly. A word that still
//! holds more points than one sketch takes is set as [`TextLayout::parts`], each laid where it
//! falls in the whole text.
use super::{Geometry, Id, MAX_ITEMS, Sketch, Workplane};
use crate::text::TextFont;
use anyhow::{Context, Result, bail, ensure};
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::{FRAC_PI_2, TAU};

/// How far a refitted run may stray from the glyph's own outline, as a share of the em: under two
/// microns at a 1.2 mm capital.
pub const FIT_TOLERANCE_EM: f64 = 0.001;
/// Longest text one layout sets.
pub const MAX_CHARS: usize = 256;
/// Characters a font lacks that are drawn from the bundled sketch library instead.
pub const SYMBOLS: &[(char, &str)] = &[('\u{2720}', "gothic/cross-pattee")];
/// Space either side of a symbol drawn from the library, as a share of the em.
const SYMBOL_BEARING_EM: f64 = 0.08;
/// Samples a run is read at per piece while it is refitted.
const FIT_SAMPLES: usize = 12;
/// Turn between two pieces' tangents under which they join smoothly and may share a refit, degrees.
const SMOOTH_DEG: f64 = 2.0;

/// Text round a circle about the sketch's origin.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextArc {
    /// Radius of the baseline, mm.
    pub radius_mm: f64,
    /// Where the text is anchored round the circle, degrees counter-clockwise from the sketch's x:
    /// its start, or its middle or end as [`TextAlign`] says.
    pub start_deg: f64,
    /// Reads clockwise with the letters standing outward, as a seal's legend does; counter-clockwise
    /// with them standing inward, as a coin's legend does under its foot.
    pub clockwise: bool,
}

/// Which point of the text its anchor names: the start of the first character, the middle of the
/// run, or the end of the last character.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    #[default]
    Start,
    Centre,
    End,
}

impl TextAlign {
    pub const ALL: &'static [TextAlign] = &[TextAlign::Start, TextAlign::Centre, TextAlign::End];
    pub fn name(self) -> &'static str {
        match self {
            TextAlign::Start => "Start",
            TextAlign::Centre => "Centre",
            TextAlign::End => "End",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|a| a.name() == name)
    }
}

/// A line of text laid out as sketch outlines.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    pub font: TextFont,
    pub text: String,
    /// Height of a capital, mm.
    pub cap_mm: f64,
    /// Space added after every character but the last, as a share of the em.
    pub tracking: f64,
    /// Round a circle about the origin, or along the sketch's x through its origin when `None`.
    pub arc: Option<TextArc>,
    pub align: TextAlign,
    /// Reflected in the sketch's y axis once laid out, as a seal is cut so its impression reads true.
    pub mirror: bool,
}

/// Glyph outlines as closed cubic loops; counters become holes by even-odd nesting. The whole text
/// is one sketch, refused when it holds more points than a sketch takes; see [`TextLayout::parts`].
pub fn text(font: TextFont, text: &str, cap_mm: f64, tracking: f64, arc: Option<TextArc>) -> Result<Sketch> {
    TextLayout { tracking, arc, ..TextLayout::new(font, text, cap_mm) }.sketch()
}

type P = [f64; 2];

/// One piece of a contour: a line, or a cubic by its four control points.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Piece {
    Line([P; 2]),
    Cubic([P; 4]),
}

impl Piece {
    fn start(&self) -> P {
        match self {
            Piece::Line(p) => p[0],
            Piece::Cubic(p) => p[0],
        }
    }
    fn end(&self) -> P {
        match self {
            Piece::Line(p) => p[1],
            Piece::Cubic(p) => p[3],
        }
    }
    /// The sketch points it adds after its start.
    fn cost(&self) -> usize {
        match self {
            Piece::Line(_) => 1,
            Piece::Cubic(_) => 3,
        }
    }
    fn mapped(&self, f: &impl Fn(P) -> P) -> Piece {
        match self {
            Piece::Line(p) => Piece::Line(p.map(f)),
            Piece::Cubic(p) => Piece::Cubic(p.map(f)),
        }
    }
    /// The unit tangent leaving its start.
    fn start_tangent(&self) -> Option<P> {
        match self {
            Piece::Line(p) => unit(sub(p[1], p[0])),
            Piece::Cubic(p) => [p[1], p[2], p[3]].iter().find_map(|q| unit(sub(*q, p[0]))),
        }
    }
    /// The unit tangent arriving at its end.
    fn end_tangent(&self) -> Option<P> {
        match self {
            Piece::Line(p) => unit(sub(p[1], p[0])),
            Piece::Cubic(p) => [p[2], p[1], p[0]].iter().find_map(|q| unit(sub(p[3], *q))),
        }
    }
}

fn sub(a: P, b: P) -> P {
    [a[0] - b[0], a[1] - b[1]]
}
fn add(a: P, b: P) -> P {
    [a[0] + b[0], a[1] + b[1]]
}
fn scale(a: P, k: f64) -> P {
    [a[0] * k, a[1] * k]
}
fn dot(a: P, b: P) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn len(a: P) -> f64 {
    a[0].hypot(a[1])
}
fn unit(a: P) -> Option<P> {
    let l = len(a);
    (l.is_finite() && l > 1e-12).then(|| scale(a, 1.0 / l))
}

fn bezier(p: &[P; 4], t: f64) -> P {
    let m = 1.0 - t;
    let (b0, b1, b2, b3) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
    [b0 * p[0][0] + b1 * p[1][0] + b2 * p[2][0] + b3 * p[3][0], b0 * p[0][1] + b1 * p[1][1] + b2 * p[2][1] + b3 * p[3][1]]
}
fn bezier_d1(p: &[P; 4], t: f64) -> P {
    let m = 1.0 - t;
    let d = |k: usize| 3.0 * m * m * (p[1][k] - p[0][k]) + 6.0 * m * t * (p[2][k] - p[1][k]) + 3.0 * t * t * (p[3][k] - p[2][k]);
    [d(0), d(1)]
}
fn bezier_d2(p: &[P; 4], t: f64) -> P {
    let m = 1.0 - t;
    let d = |k: usize| 6.0 * m * (p[2][k] - 2.0 * p[1][k] + p[0][k]) + 6.0 * t * (p[3][k] - 2.0 * p[2][k] + p[1][k]);
    [d(0), d(1)]
}

/// A glyph's closed contours in font units, the y axis up, and how far it advances the pen.
#[derive(Clone, Debug)]
struct Glyph {
    contours: Vec<Vec<Piece>>,
    advance: f64,
}

impl Glyph {
    fn cost(&self) -> usize {
        self.contours.iter().map(|c| c.iter().map(Piece::cost).sum::<usize>()).sum()
    }
}

/// Contours as ttf-parser hands them over: every quadratic raised to its exact cubic.
#[derive(Default)]
struct Outline {
    contours: Vec<Vec<Piece>>,
    current: Vec<Piece>,
    start: P,
    at: P,
}

impl Outline {
    fn push(&mut self, piece: Piece) {
        if piece.start() != piece.end() || matches!(piece, Piece::Cubic(p) if p[1] != p[0] || p[2] != p[0]) {
            self.current.push(piece);
        }
        self.at = piece.end();
    }
    fn finish(&mut self) {
        if self.at != self.start {
            let (at, start) = (self.at, self.start);
            self.push(Piece::Line([at, start]));
        }
        let contour = std::mem::take(&mut self.current);
        // A contour of one piece encloses nothing a sketch can sweep.
        if contour.len() >= 2 {
            self.contours.push(contour);
        }
    }
}

impl ttf_parser::OutlineBuilder for Outline {
    fn move_to(&mut self, x: f32, y: f32) {
        if !self.current.is_empty() {
            self.finish();
        }
        self.start = [f64::from(x), f64::from(y)];
        self.at = self.start;
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let to = [f64::from(x), f64::from(y)];
        self.push(Piece::Line([self.at, to]));
    }
    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (a, c, b) = (self.at, [f64::from(x1), f64::from(y1)], [f64::from(x), f64::from(y)]);
        // A control point on the chord between its ends draws a straight line.
        let chord = sub(b, a);
        let off = chord[0] * (c[1] - a[1]) - chord[1] * (c[0] - a[0]);
        let t = dot(sub(c, a), chord) / dot(chord, chord).max(1e-300);
        if off.abs() <= 1e-9 * dot(chord, chord).max(1e-300) && (0.0..=1.0).contains(&t) {
            self.push(Piece::Line([a, b]));
        } else {
            self.push(Piece::Cubic([a, add(a, scale(sub(c, a), 2.0 / 3.0)), add(b, scale(sub(c, b), 2.0 / 3.0)), b]));
        }
    }
    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let p = [self.at, [f64::from(x1), f64::from(y1)], [f64::from(x2), f64::from(y2)], [f64::from(x), f64::from(y)]];
        self.push(Piece::Cubic(p));
    }
    fn close(&mut self) {
        self.finish();
    }
}

/// Whether `a` runs into `b` without a corner.
fn smooth(a: &Piece, b: &Piece) -> bool {
    match (a.end_tangent(), b.start_tangent()) {
        (Some(u), Some(v)) => dot(u, v) > SMOOTH_DEG.to_radians().cos(),
        _ => false,
    }
}

/// `contour` with every smooth run of cubics refitted as few cubics as stay within `tolerance` of it;
/// lines and corners are kept as drawn.
fn refitted(contour: &[Piece], tolerance: f64) -> Vec<Piece> {
    let n = contour.len();
    // Start at a corner, so no smooth run wraps past the contour's first point.
    let first = (0..n).find(|&i| !smooth(&contour[(i + n - 1) % n], &contour[i])).unwrap_or(0);
    let pieces: Vec<Piece> = (0..n).map(|k| contour[(first + k) % n]).collect();
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    while i < n {
        let Piece::Cubic(_) = pieces[i] else {
            out.push(pieces[i]);
            i += 1;
            continue;
        };
        let (mut best, mut end) = (pieces[i], i);
        while end + 1 < n && matches!(pieces[end + 1], Piece::Cubic(_)) && smooth(&pieces[end], &pieces[end + 1]) {
            match fit(&pieces[i..=end + 1], tolerance) {
                Some(c) => {
                    best = Piece::Cubic(c);
                    end += 1;
                }
                None => break,
            }
        }
        out.push(best);
        i = end + 1;
    }
    out
}

/// One cubic through a smooth run of cubics, leaving and arriving along the run's own tangents, or
/// `None` when no such cubic stays within `tolerance` of it: Schneider's least squares for the two
/// handle lengths, with the samples' parameters refined by Newton's method.
fn fit(run: &[Piece], tolerance: f64) -> Option<[P; 4]> {
    let cubics: Vec<[P; 4]> = run.iter().map(|p| if let Piece::Cubic(c) = p { Some(*c) } else { None }).collect::<Option<_>>()?;
    let mut pts = Vec::with_capacity(cubics.len() * FIT_SAMPLES + 1);
    for c in &cubics {
        pts.extend((0..FIT_SAMPLES).map(|k| bezier(c, k as f64 / FIT_SAMPLES as f64)));
    }
    pts.push(cubics.last()?[3]);
    let mut along = vec![0.0];
    for w in pts.windows(2) {
        along.push(along.last()? + len(sub(w[1], w[0])));
    }
    let total = *along.last()?;
    if !(total > 1e-9) {
        return None;
    }
    let mut u: Vec<f64> = along.iter().map(|d| d / total).collect();
    let (p0, p3) = (pts[0], *pts.last()?);
    let (t0, t3) = (run.first()?.start_tangent()?, run.last()?.end_tangent()?);
    let chord = len(sub(p3, p0));
    let mut cubic = None;
    for _ in 0..4 {
        let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (&t, p) in u.iter().zip(&pts) {
            let m = 1.0 - t;
            let (b0, b1, b2, b3) = (m * m * m, 3.0 * m * m * t, 3.0 * m * t * t, t * t * t);
            let a1 = scale(t0, b1);
            let a2 = scale(t3, -b2);
            c00 += dot(a1, a1);
            c01 += dot(a1, a2);
            c11 += dot(a2, a2);
            let rest = sub(*p, add(scale(p0, b0 + b1), scale(p3, b2 + b3)));
            x0 += dot(a1, rest);
            x1 += dot(a2, rest);
        }
        let det = c00 * c11 - c01 * c01;
        if !(det.abs() > 1e-12 * (c00 * c11).max(1e-300)) {
            return None;
        }
        let (alpha, beta) = ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det);
        // A handle that turns back or runs past the run's own length draws a loop, not the run.
        if !(alpha > 1e-3 * chord && beta > 1e-3 * chord && alpha < total && beta < total) {
            return None;
        }
        let c = [p0, add(p0, scale(t0, alpha)), sub(p3, scale(t3, beta)), p3];
        for (t, p) in u.iter_mut().zip(&pts) {
            let (q, d1, d2) = (sub(bezier(&c, *t), *p), bezier_d1(&c, *t), bezier_d2(&c, *t));
            let den = dot(d1, d1) + dot(q, d2);
            if den.abs() > 1e-12 {
                *t = (*t - dot(q, d1) / den).clamp(0.0, 1.0);
            }
        }
        cubic = Some(c);
    }
    let c = cubic?;
    let near = u.iter().zip(&pts).map(|(t, p)| len(sub(bezier(&c, *t), *p))).fold(0.0, f64::max);
    // And the other way: nothing of the new cubic may bulge away from the run's own outline.
    let to_run = |q: P| {
        pts.windows(2)
            .map(|w| {
                let d = sub(w[1], w[0]);
                let t = (dot(sub(q, w[0]), d) / dot(d, d).max(1e-300)).clamp(0.0, 1.0);
                len(sub(q, add(w[0], scale(d, t))))
            })
            .fold(f64::INFINITY, f64::min)
    };
    let far = (0..=48).map(|k| to_run(bezier(&c, k as f64 / 48.0))).fold(0.0, f64::max);
    (near.max(far) <= tolerance).then_some(c)
}

/// A circular arc from angle `a0` turning `sweep` radians as cubics of a quarter turn at most.
fn arc_cubics(centre: P, radius: f64, a0: f64, sweep: f64) -> Vec<Piece> {
    let n = (sweep.abs() / FRAC_PI_2).ceil().max(1.0) as usize;
    let step = sweep / n as f64;
    (0..n)
        .map(|k| {
            let (f0, f1) = (a0 + step * k as f64, a0 + step * (k + 1) as f64);
            let h = 4.0 / 3.0 * (step / 4.0).tan() * radius;
            let at = |f: f64| add(centre, [radius * f.cos(), radius * f.sin()]);
            let tangent = |f: f64| [-f.sin(), f.cos()];
            let (q0, q3) = (at(f0), at(f1));
            Piece::Cubic([q0, add(q0, scale(tangent(f0), h)), sub(q3, scale(tangent(f1), h)), q3])
        })
        .collect()
}

/// The bundled library sketch drawn for `symbol` as a glyph a capital high, in font units, sitting
/// on the baseline with [`SYMBOL_BEARING_EM`] either side.
fn symbol_glyph(name: &str, cap_units: f64, em_units: f64) -> Result<Glyph> {
    use cadkernel::geom2d::Curve;
    let asset = ringdesign_assets::find(ringdesign_assets::SKETCHES, name).with_context(|| format!("the bundled sketch {name} is missing"))?;
    let sketch = super::exchange::import_svg(&asset.text())?;
    let loops = super::region::loops(&sketch.solve()?.sketch)?;
    let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
    for l in &loops {
        for c in &l.curves {
            for p in c.tessellate_within(1e-4) {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
        }
    }
    let height = hi[1] - lo[1];
    ensure!(height > 1e-9, "the bundled sketch {name} has no height");
    let k = cap_units / height;
    let bearing = SYMBOL_BEARING_EM * em_units;
    let to = |p: P| [(p[0] - lo[0]) * k + bearing, (p[1] - lo[1]) * k];
    let mut contours = Vec::new();
    for l in &loops {
        let mut pieces = Vec::new();
        for (c, forward) in l.curves.iter().zip(&l.forward) {
            match c {
                Curve::Line(line) => {
                    let (a, b) = if *forward { (line.start, line.end) } else { (line.end, line.start) };
                    pieces.push(Piece::Line([to(a), to(b)]));
                }
                Curve::Arc(arc) => {
                    let sweep = arc.sweep();
                    let (a0, s) = if *forward { (arc.start_angle, sweep) } else { (arc.start_angle + sweep, -sweep) };
                    pieces.extend(arc_cubics(arc.centre, arc.radius, a0, s).iter().map(|p| p.mapped(&to)));
                }
                other => bail!("the bundled sketch {name} draws a {other:?}; a symbol is lines and arcs"),
            }
        }
        contours.push(pieces);
    }
    Ok(Glyph { contours, advance: (hi[0] - lo[0]) * k + 2.0 * bearing })
}

/// One character laid out: what it is, which word it belongs to, where it starts along the baseline
/// and how far it advances there, in mm, and its outline in font units.
#[derive(Clone, Debug)]
struct Set {
    index: usize,
    ch: char,
    word: usize,
    x_mm: f64,
    advance_mm: f64,
    glyph: Option<std::sync::Arc<Glyph>>,
}

/// The whole text laid along its baseline.
struct Laid {
    sets: Vec<Set>,
    /// Font units to millimetres.
    scale: f64,
    /// From the first character's start to the last character's advance, mm.
    length_mm: f64,
}

impl TextLayout {
    /// `text` in `font` with capitals `cap_mm` high, straight along the sketch's x from its origin.
    pub fn new(font: TextFont, text: &str, cap_mm: f64) -> Self {
        Self { font, text: text.to_string(), cap_mm, tracking: 0.0, arc: None, align: TextAlign::Start, mirror: false }
    }

    fn check(&self) -> Result<()> {
        let n = self.text.chars().count();
        ensure!(!self.text.trim().is_empty(), "There is no text to set");
        ensure!(n <= MAX_CHARS, "Text of {n} characters is longer than the {MAX_CHARS} one layout sets");
        if let Some(c) = self.text.chars().find(|c| c.is_control()) {
            bail!("Text sets one line; {:?} (U+{:04X}) is a control character", c, c as u32);
        }
        ensure!(self.cap_mm.is_finite() && (0.05..=100.0).contains(&self.cap_mm), "A capital height must be 0.05 to 100 mm; {} is not", self.cap_mm);
        ensure!(self.tracking.is_finite() && (-0.5..=50.0).contains(&self.tracking), "Tracking must be -0.5 to 50 em; {} is not", self.tracking);
        if let Some(arc) = &self.arc {
            ensure!(arc.radius_mm.is_finite() && arc.radius_mm > 0.0 && arc.radius_mm <= 1000.0, "An arc's radius must be above 0 and at most 1000 mm; {} is not", arc.radius_mm);
            ensure!(arc.start_deg.is_finite(), "An arc needs an angle to start at");
            ensure!(arc.clockwise || arc.radius_mm > self.cap_mm, "Letters standing inward on a {} mm arc need it wider than their {} mm capitals", arc.radius_mm, self.cap_mm);
        }
        Ok(())
    }

    /// Every character in order, laid along the baseline from its anchor.
    fn laid(&self) -> Result<Laid> {
        self.check()?;
        let face = ttf_parser::Face::parse(self.font.bytes(), 0).map_err(|e| anyhow::anyhow!("{}: {e}", self.font.label()))?;
        let em = f64::from(face.units_per_em());
        let cap_units = face
            .capital_height()
            .map(f64::from)
            .filter(|h| *h > 0.0)
            .or_else(|| face.glyph_index('H').and_then(|g| face.glyph_bounding_box(g)).map(|b| f64::from(b.y_max)).filter(|h| *h > 0.0))
            .unwrap_or(0.7 * f64::from(face.ascender()).max(1.0));
        let scale = self.cap_mm / cap_units;
        let tolerance = FIT_TOLERANCE_EM * em;
        let kern = |a: ttf_parser::GlyphId, b: ttf_parser::GlyphId| -> f64 {
            let Some(table) = face.tables().kern else { return 0.0 };
            table.subtables.into_iter().filter(|s| s.horizontal && !s.variable && !s.has_cross_stream).filter_map(|s| s.glyphs_kerning(a, b)).map(f64::from).sum()
        };
        let mut glyphs: HashMap<ttf_parser::GlyphId, std::sync::Arc<Glyph>> = HashMap::new();
        let mut symbols: HashMap<char, std::sync::Arc<Glyph>> = HashMap::new();
        let space = face.glyph_index(' ').and_then(|g| face.glyph_hor_advance(g)).map_or(0.25 * em, f64::from);
        let (mut sets, mut pen, mut word, mut gap) = (Vec::new(), 0.0, 0, true);
        let mut previous: Option<ttf_parser::GlyphId> = None;
        for (index, ch) in self.text.chars().enumerate() {
            if ch.is_whitespace() {
                sets.push(Set { index, ch, word, x_mm: pen, advance_mm: space * scale, glyph: None });
                pen += space * scale + self.tracking * em * scale;
                if !gap {
                    word += 1;
                }
                gap = true;
                previous = None;
                continue;
            }
            gap = false;
            let gid = face.glyph_index(ch);
            let glyph = match gid {
                Some(gid) => {
                    if let Some(p) = previous {
                        pen += kern(p, gid) * scale;
                    }
                    match glyphs.get(&gid) {
                        Some(g) => g.clone(),
                        None => {
                            let mut outline = Outline::default();
                            face.outline_glyph(gid, &mut outline);
                            if !outline.current.is_empty() {
                                outline.finish();
                            }
                            let contours = outline.contours.iter().map(|c| refitted(c, tolerance)).collect();
                            let advance = face.glyph_hor_advance(gid).map_or(0.0, f64::from);
                            let g = std::sync::Arc::new(Glyph { contours, advance });
                            glyphs.insert(gid, g.clone());
                            g
                        }
                    }
                }
                None => match SYMBOLS.iter().find(|(c, _)| *c == ch) {
                    Some((_, name)) => match symbols.get(&ch) {
                        Some(g) => g.clone(),
                        None => {
                            let mut g = symbol_glyph(name, cap_units, em)?;
                            g.contours = g.contours.iter().map(|c| refitted(c, tolerance)).collect();
                            let g = std::sync::Arc::new(g);
                            symbols.insert(ch, g.clone());
                            g
                        }
                    },
                    None => bail!("{} has no letter for {:?} (U+{:04X})", self.font.label(), ch, ch as u32),
                },
            };
            previous = gid;
            let advance_mm = glyph.advance * scale;
            sets.push(Set { index, ch, word, x_mm: pen, advance_mm, glyph: Some(glyph) });
            pen += advance_mm + self.tracking * em * scale;
        }
        // The run ends at the last character's advance; the tracking after it is no part of it.
        let length_mm = pen - self.tracking * em * scale;
        let offset = match self.align {
            TextAlign::Start => 0.0,
            TextAlign::Centre => -0.5 * length_mm,
            TextAlign::End => -length_mm,
        };
        for s in &mut sets {
            s.x_mm += offset;
        }
        if let Some(arc) = &self.arc {
            let turn = length_mm / arc.radius_mm;
            ensure!(turn <= TAU, "The text runs {:.0}° round its {} mm arc; it may run one turn at most", turn.to_degrees(), arc.radius_mm);
        }
        Ok(Laid { sets, scale, length_mm })
    }

    /// How long the text runs along its baseline, from the first character's start to the last
    /// character's advance, mm.
    pub fn length_mm(&self) -> Result<f64> {
        Ok(self.laid()?.length_mm)
    }

    /// The tracking that runs the text `length_mm` along its baseline: round an arc, the arc's own
    /// length at the baseline's radius.
    pub fn tracking_for(&self, length_mm: f64) -> Result<f64> {
        ensure!(length_mm.is_finite() && length_mm > 0.0, "A length to fit must be above 0 mm");
        let bare = Self { tracking: 0.0, arc: None, ..self.clone() };
        let laid = bare.laid()?;
        let gaps = self.text.chars().count().saturating_sub(1);
        ensure!(gaps > 0, "One character has no spacing to fit");
        let face = ttf_parser::Face::parse(self.font.bytes(), 0).map_err(|e| anyhow::anyhow!("{}: {e}", self.font.label()))?;
        let em_mm = f64::from(face.units_per_em()) * laid.scale;
        let tracking = (length_mm - laid.length_mm) / (gaps as f64 * em_mm);
        ensure!(
            (-0.5..=50.0).contains(&tracking),
            "Running {:?} {length_mm:.2} mm takes a tracking of {tracking:.2} em, outside -0.5 to 50",
            self.text
        );
        Ok(tracking)
    }

    /// Where a point of a glyph set at `s`, `p` in font units from its pen position, lies in the sketch.
    fn place(&self, s: &Set, scale: f64, p: P) -> P {
        let (gx, gy) = (p[0] * scale, p[1] * scale);
        let mut q = match &self.arc {
            None => [s.x_mm + gx, gy],
            Some(arc) => {
                let (r, centre) = (arc.radius_mm, s.x_mm + 0.5 * s.advance_mm);
                let dx = gx - 0.5 * s.advance_mm;
                let sign = if arc.clockwise { -1.0 } else { 1.0 };
                let theta = arc.start_deg.to_radians() + sign * centre / r;
                let (sin, cos) = theta.sin_cos();
                if arc.clockwise {
                    [(r + gy) * cos + dx * sin, (r + gy) * sin - dx * cos]
                } else {
                    [(r - gy) * cos - dx * sin, (r - gy) * sin + dx * cos]
                }
            }
        };
        if self.mirror {
            q[0] = -q[0];
        }
        q
    }

    /// The sketch of `sets`, named `name`, checked to sweep: each set's outline as closed loops of
    /// lines and Béziers.
    fn draw(&self, laid: &Laid, sets: &[&Set], name: &str) -> Result<Sketch> {
        let cost: usize = sets.iter().filter_map(|s| s.glyph.as_ref()).map(|g| g.cost()).sum();
        ensure!(cost > 0, "{name:?} draws nothing a sketch can hold");
        ensure!(
            cost <= MAX_ITEMS,
            "{name:?} holds {cost} points, over the {MAX_ITEMS} one sketch takes; set it a part at a time ({} parts)",
            self.parts_of(laid).map_or(0, |p| p.len())
        );
        let mut s = Sketch { name: name.to_string(), plane: Workplane::default(), ..Sketch::default() };
        let mut owner: BTreeMap<Id, usize> = BTreeMap::new();
        for (k, set) in sets.iter().enumerate() {
            let Some(glyph) = &set.glyph else { continue };
            let at = |p: P| self.place(set, laid.scale, p);
            for contour in &glyph.contours {
                let first = s.point(at(contour[0].start()));
                let mut from = first;
                for (i, piece) in contour.iter().enumerate() {
                    // The last piece closes the loop on the contour's first point.
                    let last = i + 1 == contour.len();
                    let (geometry, end) = match piece {
                        Piece::Line(p) => {
                            let b = if last { first } else { s.point(at(p[1])) };
                            (Geometry::Line { a: from, b }, b)
                        }
                        Piece::Cubic(p) => {
                            let (c1, c2) = (s.point(at(p[1])), s.point(at(p[2])));
                            let b = if last { first } else { s.point(at(p[3])) };
                            (Geometry::Bezier { points: [from, c1, c2, b] }, b)
                        }
                    };
                    owner.insert(s.entity(geometry), k);
                    from = end;
                }
            }
        }
        // Where two outlines touch the sketch cannot sweep: say which letters, in the text's terms.
        if let Err(e) = s.sweep_regions() {
            let message = format!("{e:#}");
            let mut named: Vec<usize> = Vec::new();
            for part in message.split('#').skip(1) {
                let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
                if let Some(k) = digits.parse::<Id>().ok().and_then(|id| owner.get(&id)) {
                    if !named.contains(k) {
                        named.push(*k);
                    }
                }
            }
            let who = |k: usize| format!("{:?} (character {})", sets[k].ch, sets[k].index + 1);
            match named.as_slice() {
                [a, b, ..] => bail!("{} and {} touch, which a sketch cannot sweep; open the tracking ({message})", who(*a), who(*b)),
                [a] => bail!("{} is drawn with strokes that cross or touch, which a sketch cannot sweep ({message})", who(*a)),
                [] => return Err(e),
            }
        }
        Ok(s)
    }

    /// Words, and runs of a word's characters where the whole word holds more points than one sketch takes.
    fn parts_of<'a>(&self, laid: &'a Laid) -> Result<Vec<Vec<&'a Set>>> {
        let mut parts: Vec<Vec<&Set>> = Vec::new();
        let words = laid.sets.iter().filter(|s| s.glyph.is_some()).map(|s| s.word).max().map_or(0, |w| w + 1);
        for word in 0..words {
            let mut part: Vec<&Set> = Vec::new();
            let mut cost = 0;
            for s in laid.sets.iter().filter(|s| s.word == word) {
                let Some(g) = &s.glyph else { continue };
                let c = g.cost();
                ensure!(c <= MAX_ITEMS, "{:?} alone holds {c} points, over the {MAX_ITEMS} one sketch takes", s.ch);
                if cost + c > MAX_ITEMS && !part.is_empty() {
                    parts.push(std::mem::take(&mut part));
                    cost = 0;
                }
                part.push(s);
                cost += c;
            }
            if !part.is_empty() {
                parts.push(part);
            }
        }
        Ok(parts)
    }

    /// The whole text as one sketch, named after it; refused when it holds more points than one
    /// sketch takes, which [`Self::parts`] sets a part at a time.
    pub fn sketch(&self) -> Result<Sketch> {
        let laid = self.laid()?;
        let sets: Vec<&Set> = laid.sets.iter().collect();
        self.draw(&laid, &sets, self.text.trim())
    }

    /// The text a part at a time, each laid where it falls in the whole: one sketch per word, and a
    /// word that holds more points than one sketch takes split between its letters. Each is named
    /// after the characters it sets.
    pub fn parts(&self) -> Result<Vec<Sketch>> {
        let laid = self.laid()?;
        self.parts_of(&laid)?
            .iter()
            .map(|part| {
                let name: String = part.iter().map(|s| s.ch).collect();
                self.draw(&laid, part, &name)
            })
            .collect()
    }

    /// How many parts [`Self::parts`] sets the text in.
    pub fn part_count(&self) -> Result<usize> {
        Ok(self.parts_of(&self.laid()?)?.len())
    }

    /// Part `k` of [`Self::parts`] alone.
    pub fn part(&self, k: usize) -> Result<Sketch> {
        let laid = self.laid()?;
        let parts = self.parts_of(&laid)?;
        let part = parts.get(k).with_context(|| format!("{:?} sets in {} part{}; there is no part {k}", self.text, parts.len(), if parts.len() == 1 { "" } else { "s" }))?;
        let name: String = part.iter().map(|s| s.ch).collect();
        self.draw(&laid, part, &name)
    }
}

#[cfg(test)]
mod tests;
