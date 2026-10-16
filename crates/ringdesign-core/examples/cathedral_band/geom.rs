//! Plane geometry, x round the ring at the rails' radius and y across the band: signed-distance shapes and the Gothic forms drawn with them.
use std::f64::consts::{PI, TAU};

pub type V = [f64; 2];

pub fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1]]
}
pub fn mul(a: V, s: f64) -> V {
    [a[0] * s, a[1] * s]
}
pub fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
pub fn len(a: V) -> f64 {
    a[0].hypot(a[1])
}
pub fn unit(a: V) -> V {
    mul(a, 1.0 / len(a).max(1e-12))
}
pub fn polar(c: V, r: f64, a: f64) -> V {
    [c[0] + r * a.cos(), c[1] + r * a.sin()]
}
pub fn lerp(a: V, b: V, t: f64) -> V {
    add(a, mul(sub(b, a), t))
}

pub fn seg_dist(p: V, a: V, b: V) -> f64 {
    let (ab, ap) = (sub(b, a), sub(p, a));
    let t = (dot(ap, ab) / dot(ab, ab).max(1e-18)).clamp(0.0, 1.0);
    len(sub(ap, mul(ab, t)))
}
/// Distance to the arc of radius `r` about `c` running counter-clockwise from `a0` to `a1`.
fn arc_dist(p: V, c: V, r: f64, a0: f64, a1: f64) -> f64 {
    let d = sub(p, c);
    let span = (a1 - a0).rem_euclid(TAU);
    let rel = (d[1].atan2(d[0]) - a0).rem_euclid(TAU);
    if rel <= span { (len(d) - r).abs() } else { len(sub(p, polar(c, r, a0))).min(len(sub(p, polar(c, r, a1)))) }
}
/// Signed distance to a capsule whose radius runs from `ra` at `a` to `rb` at `b`.
fn taper_dist(p: V, a: V, b: V, ra: f64, rb: f64) -> f64 {
    let h = len(sub(b, a)).max(1e-12);
    let ey = mul(sub(b, a), 1.0 / h);
    let ex = [ey[1], -ey[0]];
    let q = [dot(sub(p, a), ex).abs(), dot(sub(p, a), ey)];
    let s = ((ra - rb) / h).clamp(-0.999, 0.999);
    let c = (1.0 - s * s).sqrt();
    let k = dot(q, [-s, c]);
    if k < 0.0 {
        len(q) - ra
    } else if k > c * h {
        len(sub(q, [0.0, h])) - rb
    } else {
        dot(q, [c, s]) - ra
    }
}
/// Signed distance to a simple polygon.
fn poly_dist(p: V, v: &[V]) -> f64 {
    let n = v.len();
    let mut d = dot(sub(p, v[0]), sub(p, v[0]));
    let mut s = 1.0;
    let mut j = n - 1;
    for i in 0..n {
        let e = sub(v[j], v[i]);
        let w = sub(p, v[i]);
        let b = sub(w, mul(e, (dot(w, e) / dot(e, e).max(1e-18)).clamp(0.0, 1.0)));
        d = d.min(dot(b, b));
        let c = [p[1] >= v[i][1], p[1] < v[j][1], e[0] * w[1] > e[1] * w[0]];
        if (c[0] && c[1] && c[2]) || (!c[0] && !c[1] && !c[2]) {
            s = -s;
        }
        j = i;
    }
    s * d.sqrt()
}

/// How far from a shape its distance is read exactly; anything further reads as this.
pub const CAP: f64 = 0.3;
const FAR: f64 = 1e9;

#[derive(Clone, Debug)]
enum Kind {
    Disc { c: V, r: f64 },
    Ring { c: V, r: f64, w: f64 },
    Seg { a: V, b: V, w: f64 },
    Arc { c: V, r: f64, a0: f64, a1: f64, w: f64 },
    Foil { c: V, n: u32, e: f64, a: f64, phase: f64 },
    Taper { a: V, b: V, ra: f64, rb: f64 },
    Half { p: V, n: V },
    Poly(Vec<V>),
    Union(Vec<Shape>),
    Inter(Vec<Shape>),
    Not(Box<Shape>),
    Offset(Box<Shape>, f64),
}

/// A signed-distance shape, negative inside, with its box.
#[derive(Clone, Debug)]
pub struct Shape {
    kind: Kind,
    lo: V,
    hi: V,
}

fn boxed(c: V, r: f64) -> (V, V) {
    ([c[0] - r, c[1] - r], [c[0] + r, c[1] + r])
}

impl Shape {
    fn new(kind: Kind) -> Shape {
        let (lo, hi) = match &kind {
            Kind::Disc { c, r } => boxed(*c, *r),
            Kind::Ring { c, r, w } | Kind::Arc { c, r, w, .. } => boxed(*c, r + 0.5 * w),
            Kind::Seg { a, b, w } => ([a[0].min(b[0]) - 0.5 * w, a[1].min(b[1]) - 0.5 * w], [a[0].max(b[0]) + 0.5 * w, a[1].max(b[1]) + 0.5 * w]),
            Kind::Foil { c, e, a, .. } => boxed(*c, e + a),
            Kind::Taper { a, b, ra, rb } => {
                let r = ra.max(*rb);
                ([a[0].min(b[0]) - r, a[1].min(b[1]) - r], [a[0].max(b[0]) + r, a[1].max(b[1]) + r])
            }
            Kind::Half { .. } | Kind::Not(_) => ([-FAR; 2], [FAR; 2]),
            Kind::Offset(s, d) => ([s.lo[0] - d.max(0.0), s.lo[1] - d.max(0.0)], [s.hi[0] + d.max(0.0), s.hi[1] + d.max(0.0)]),
            Kind::Poly(v) => v.iter().fold(([FAR; 2], [-FAR; 2]), |(lo, hi), q| ([lo[0].min(q[0]), lo[1].min(q[1])], [hi[0].max(q[0]), hi[1].max(q[1])])),
            Kind::Union(s) => s.iter().fold(([FAR; 2], [-FAR; 2]), |(lo, hi), q| ([lo[0].min(q.lo[0]), lo[1].min(q.lo[1])], [hi[0].max(q.hi[0]), hi[1].max(q.hi[1])])),
            Kind::Inter(s) => s.iter().fold(([-FAR; 2], [FAR; 2]), |(lo, hi), q| ([lo[0].max(q.lo[0]), lo[1].max(q.lo[1])], [hi[0].min(q.hi[0]), hi[1].min(q.hi[1])])),
        };
        Shape { kind, lo, hi }
    }

    pub fn sd(&self, p: V) -> f64 {
        if p[0] < self.lo[0] - CAP || p[0] > self.hi[0] + CAP || p[1] < self.lo[1] - CAP || p[1] > self.hi[1] + CAP {
            return CAP;
        }
        match &self.kind {
            Kind::Disc { c, r } => len(sub(p, *c)) - r,
            Kind::Ring { c, r, w } => (len(sub(p, *c)) - r).abs() - 0.5 * w,
            Kind::Seg { a, b, w } => seg_dist(p, *a, *b) - 0.5 * w,
            Kind::Arc { c, r, a0, a1, w } => arc_dist(p, *c, *r, *a0, *a1) - 0.5 * w,
            Kind::Foil { c, n, e, a, phase } => {
                (0..*n).map(|k| len(sub(p, polar(*c, *e, phase + TAU * f64::from(k) / f64::from(*n)))) - a).fold(f64::MAX, f64::min)
            }
            Kind::Taper { a, b, ra, rb } => taper_dist(p, *a, *b, *ra, *rb),
            Kind::Half { p: o, n } => dot(sub(p, *o), *n) / len(*n),
            Kind::Poly(v) => poly_dist(p, v),
            Kind::Union(s) => s.iter().map(|s| s.sd(p)).fold(f64::MAX, f64::min),
            Kind::Inter(s) => {
                let mut m = f64::MIN;
                for s in s {
                    m = m.max(s.sd(p));
                    if m >= CAP {
                        return CAP;
                    }
                }
                m
            }
            Kind::Not(s) => -s.sd(p),
            Kind::Offset(s, d) => s.sd(p) - d,
        }
    }

    pub fn x_span(&self) -> (f64, f64) {
        (self.lo[0], self.hi[0])
    }

    pub fn shifted(&self, dx: f64) -> Shape {
        let m = |c: &V| [c[0] + dx, c[1]];
        Shape::new(match &self.kind {
            Kind::Disc { c, r } => Kind::Disc { c: m(c), r: *r },
            Kind::Ring { c, r, w } => Kind::Ring { c: m(c), r: *r, w: *w },
            Kind::Seg { a, b, w } => Kind::Seg { a: m(a), b: m(b), w: *w },
            Kind::Arc { c, r, a0, a1, w } => Kind::Arc { c: m(c), r: *r, a0: *a0, a1: *a1, w: *w },
            Kind::Foil { c, n, e, a, phase } => Kind::Foil { c: m(c), n: *n, e: *e, a: *a, phase: *phase },
            Kind::Taper { a, b, ra, rb } => Kind::Taper { a: m(a), b: m(b), ra: *ra, rb: *rb },
            Kind::Half { p, n } => Kind::Half { p: m(p), n: *n },
            Kind::Poly(v) => Kind::Poly(v.iter().map(m).collect()),
            Kind::Union(s) => Kind::Union(s.iter().map(|s| s.shifted(dx)).collect()),
            Kind::Inter(s) => Kind::Inter(s.iter().map(|s| s.shifted(dx)).collect()),
            Kind::Not(s) => Kind::Not(Box::new(s.shifted(dx))),
            Kind::Offset(s, d) => Kind::Offset(Box::new(s.shifted(dx)), *d),
        })
    }
}

pub fn disc(c: V, r: f64) -> Shape {
    Shape::new(Kind::Disc { c, r })
}
pub fn ring(c: V, r: f64, w: f64) -> Shape {
    Shape::new(Kind::Ring { c, r, w })
}
pub fn seg(a: V, b: V, w: f64) -> Shape {
    Shape::new(Kind::Seg { a, b, w })
}
/// A band `w` wide along the arc of radius `r` about `c`, counter-clockwise from `a0` to `a1`.
pub fn arc(c: V, r: f64, a0: f64, a1: f64, w: f64) -> Shape {
    Shape::new(Kind::Arc { c, r, a0, a1, w })
}
pub fn foil(c: V, n: u32, e: f64, a: f64, phase: f64) -> Shape {
    Shape::new(Kind::Foil { c, n, e, a, phase })
}
pub fn taper(a: V, b: V, ra: f64, rb: f64) -> Shape {
    Shape::new(Kind::Taper { a, b, ra, rb })
}
/// The side of the line through `p` that `n` points away from.
pub fn half(p: V, n: V) -> Shape {
    Shape::new(Kind::Half { p, n })
}
pub fn poly(v: Vec<V>) -> Shape {
    Shape::new(Kind::Poly(v))
}
pub fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Shape {
    poly(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
}
pub fn union(s: Vec<Shape>) -> Shape {
    Shape::new(Kind::Union(s))
}
pub fn inter(s: Vec<Shape>) -> Shape {
    Shape::new(Kind::Inter(s))
}
pub fn not(s: Shape) -> Shape {
    Shape::new(Kind::Not(Box::new(s)))
}
/// The shape grown by `d`, or shrunk where `d` is negative.
pub fn offset(s: Shape, d: f64) -> Shape {
    Shape::new(Kind::Offset(Box::new(s), d))
}
pub fn below(y: f64) -> Shape {
    half([0.0, y], [0.0, 1.0])
}
pub fn above(y: f64) -> Shape {
    half([0.0, y], [0.0, -1.0])
}
pub fn left_of(x: f64) -> Shape {
    half([x, 0.0], [1.0, 0.0])
}
pub fn right_of(x: f64) -> Shape {
    half([x, 0.0], [-1.0, 0.0])
}

/// Metal standing at one level: a solid shape, or a plate pierced by lights.
#[derive(Clone, Debug)]
pub enum Metal {
    Solid(Shape),
    Pierced(Shape, Shape),
}

impl Metal {
    pub fn sd(&self, p: V) -> f64 {
        match self {
            Metal::Solid(s) => s.sd(p),
            Metal::Pierced(body, lights) => {
                let b = body.sd(p);
                if b >= CAP { CAP } else { b.max(-lights.sd(p)) }
            }
        }
    }
    pub fn x_span(&self) -> (f64, f64) {
        match self {
            Metal::Solid(s) | Metal::Pierced(s, _) => s.x_span(),
        }
    }
    pub fn shifted(&self, dx: f64) -> Metal {
        match self {
            Metal::Solid(s) => Metal::Solid(s.shifted(dx)),
            Metal::Pierced(b, l) => Metal::Pierced(b.shifted(dx), l.shifted(dx)),
        }
    }
}

/// Metal whose top stands at `level`, 1 just under the rails and 4 one step above the deepest floor.
#[derive(Clone, Debug)]
pub struct Piece {
    pub level: usize,
    pub metal: Metal,
}

/// A raised feature struck onto the floor at level `floor` after the pockets round it are cut, its top at level `top`.
#[derive(Clone, Debug)]
pub struct Ornament {
    pub name: &'static str,
    pub shape: Shape,
    pub top: usize,
    pub floor: usize,
}

/// What a section draws, level by level.
#[derive(Default)]
pub struct Draw {
    pub pieces: Vec<Piece>,
    pub ornaments: Vec<Ornament>,
}

impl Draw {
    pub fn ornament(&mut self, name: &'static str, top: usize, floor: usize, shape: Shape) {
        self.ornaments.push(Ornament { name, shape, top, floor });
    }
    pub fn solid(&mut self, level: usize, s: Shape) {
        self.pieces.push(Piece { level, metal: Metal::Solid(s) });
    }
    pub fn pierced(&mut self, level: usize, body: Shape, lights: Shape) {
        self.pieces.push(Piece { level, metal: Metal::Pierced(body, lights) });
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Gothic forms.

/// Height of a two-centred arch of half-width `a` struck at radius `rho`.
pub fn rise(a: f64, rho: f64) -> f64 {
    (rho * rho - (rho - a) * (rho - a)).max(0.0).sqrt()
}

/// The inside of a two-centred pointed arch over the springing line `ys`.
pub fn pointed(xc: f64, ys: f64, a: f64, rho: f64) -> Shape {
    inter(vec![disc([xc - a + rho, ys], rho), disc([xc + a - rho, ys], rho), above(ys)])
}

/// A band `w` wide along a pointed arch whose centre line has half-width `a`, mitred at the apex.
pub fn arch_band(xc: f64, ys: f64, a: f64, rho: f64, w: f64) -> Shape {
    let h = rise(a, rho);
    let (cl, cr) = ([xc - a + rho, ys], [xc + a - rho, ys]);
    let (al, ar) = (h.atan2(a - rho), h.atan2(rho - a));
    union(vec![
        inter(vec![arc(cl, rho, al - 0.25, PI, w), left_of(xc)]),
        inter(vec![arc(cr, rho, 0.0, ar + 0.25, w), right_of(xc)]),
    ])
}

/// A light from `sill` to a pointed head springing at `spring`, struck at `k` times its width.
pub fn lancet(x0: f64, x1: f64, sill: f64, spring: f64, k: f64) -> Shape {
    let (xc, a) = (0.5 * (x0 + x1), 0.5 * (x1 - x0));
    union(vec![rect(x0, sill, x1, spring + 0.01), pointed(xc, spring, a, k * 2.0 * a)])
}

/// An upright light from `sill` to its head: a straight body, then three lobes round a core, cusped where they meet, under a point.
pub fn trefoil_lancet(x0: f64, x1: f64, sill: f64, spring: f64) -> Shape {
    let (xc, w) = (0.5 * (x0 + x1), 0.5 * (x1 - x0));
    let a = 0.5 * w;
    let top = [xc, spring + a];
    let k = 1.25 * a;
    union(vec![
        rect(x0, sill, x1, spring),
        disc([xc, spring], 0.55 * w),
        disc([xc - a, spring], a),
        disc([xc + a, spring], a),
        disc(top, a),
        inter(vec![disc([xc + k - a, top[1]], k), disc([xc - k + a, top[1]], k), above(top[1])]),
    ])
}

/// A light radiating along `phi` between spokes `bar` wide at `phi ± alpha`, from `t0` to a pointed head on `t1` struck at `k` times its width.
pub fn radial_lancet(c: V, phi: f64, alpha: f64, bar: f64, t0: f64, t1: f64, k: f64) -> Shape {
    let dir = [phi.cos(), phi.sin()];
    let side = [-dir[1], dir[0]];
    let n1 = [-(phi - alpha).sin(), (phi - alpha).cos()];
    let n2 = [(phi + alpha).sin(), -(phi + alpha).cos()];
    let w = t1 * alpha.sin() - 0.5 * bar;
    let rho = k * 2.0 * w;
    let b = add(c, mul(dir, t1));
    union(vec![
        inter(vec![half(add(c, mul(n1, 0.5 * bar)), mul(n1, -1.0)), half(add(c, mul(n2, 0.5 * bar)), mul(n2, -1.0)), half(add(c, mul(dir, t0)), mul(dir, -1.0)), half(add(b, mul(dir, 0.01)), dir)]),
        inter(vec![disc(add(b, mul(side, rho - w)), rho), disc(add(b, mul(side, w - rho)), rho), half(b, mul(dir, -1.0))]),
    ])
}

/// A foiled light: `n` lobes reaching `reach`, cusped where neighbours meet, the middle filled.
pub fn foiled(c: V, n: u32, reach: f64, phase: f64) -> Shape {
    let s = (PI / f64::from(n)).sin();
    let lobe = reach * s / (1.0 + s) * 1.18;
    let e = reach - lobe;
    union(vec![disc(c, e.min(reach * 0.62)), foil(c, n, e, lobe, phase)])
}

/// A curved dagger: a round head at `head` tapering along an arc bowed by `bend` to a point at `tail`.
pub fn mouchette(head: V, r: f64, tail: V, bend: f64) -> Shape {
    let n = 10;
    let d = sub(tail, head);
    let side = unit([-d[1], d[0]]);
    let at = |t: f64| add(lerp(head, tail, t), mul(side, bend * 4.0 * t * (1.0 - t)));
    let radius = |t: f64| r * (1.0 - t).powf(0.85) + 0.02 * t;
    let mut parts = vec![disc(head, r)];
    for k in 0..n {
        let (t0, t1) = (k as f64 / n as f64, (k + 1) as f64 / n as f64);
        parts.push(taper(at(t0), at(t1), radius(t0), radius(t1)));
    }
    union(parts)
}

/// A small leaf knob on an edge at `base`, reaching out along `dir` and curling up.
pub fn crocket(base: V, dir: V) -> Shape {
    let d = unit(add(unit(dir), [0.0, 0.45]));
    taper(sub(base, mul(d, 0.04)), add(base, mul(d, 0.17)), 0.085, 0.11)
}

/// A fleuron finial on a stem from `base` up to `top`.
pub fn finial(base: V, top: f64) -> Shape {
    let x = base[0];
    let yf = base[1] + 0.5 * (top - base[1]);
    union(vec![
        seg(base, [x, top], 0.2),
        seg([x - 0.17, yf], [x + 0.17, yf], 0.17),
        disc([x - 0.18, yf + 0.03], 0.1),
        disc([x + 0.18, yf + 0.03], 0.1),
        disc([x, top - 0.06], 0.11),
    ])
}

/// A pinnacle: a shaft `w` wide from `foot` to `cap`, a gablet, and a crocketed spirelet to a knob at `tip`.
pub fn pinnacle(x: f64, foot: f64, cap: f64, tip: f64, w: f64) -> Shape {
    let hw = 0.5 * w;
    let mut parts = vec![
        rect(x - hw, foot, x + hw, cap),
        poly(vec![[x - hw - 0.07, cap - 0.02], [x + hw + 0.07, cap - 0.02], [x, cap + 0.36]]),
        poly(vec![[x - 0.62 * hw, cap], [x + 0.62 * hw, cap], [x + 0.09, tip], [x - 0.09, tip]]),
        disc([x, tip], 0.11),
    ];
    let n = ((tip - cap - 0.45) / 0.36).floor().max(0.0) as usize;
    for k in 0..n {
        let t = (0.45 + 0.36 * k as f64) / (tip - cap);
        let y = cap + t * (tip - cap);
        let half_w = 0.09 + (0.62 * hw - 0.09) * (1.0 - t);
        let s = if k % 2 == 0 { -1.0 } else { 1.0 };
        parts.push(crocket([x + s * half_w, y], [s, 0.0]));
    }
    union(parts)
}

/// An ogee arch: from springing half-width `a` at `ys`, convex to its inflection, then reversed to a point `h` above.
pub struct Ogee {
    pub xc: f64,
    pub ys: f64,
    pub a: f64,
    pub rho1: f64,
    pub rho2: f64,
    pub phi: f64,
    /// Lower and upper centres of the left half.
    pub c1: V,
    pub c2: V,
    pub apex: V,
    pub inflection: V,
}

pub fn ogee(xc: f64, ys: f64, a: f64, h: f64, rho1: f64) -> Ogee {
    let phi = 2.0 * (h / a).atan();
    let sum = a / (1.0 + phi.cos());
    let c1 = [xc - a + rho1, ys];
    let c2 = polar(c1, sum, phi);
    Ogee { xc, ys, a, rho1, rho2: sum - rho1, phi, c1, c2, apex: [xc, c2[1]], inflection: polar(c1, rho1, phi) }
}

impl Ogee {
    fn mirror(&self, c: V) -> V {
        [2.0 * self.xc - c[0], c[1]]
    }
    /// The curve as a band `w` wide, with its jambs down to `foot`.
    pub fn band(&self, w: f64, foot: f64) -> Shape {
        let (c1r, c2r) = (self.mirror(self.c1), self.mirror(self.c2));
        union(vec![
            arc(self.c1, self.rho1, self.phi, PI, w),
            arc(self.c2, self.rho2, self.phi + PI, TAU, w),
            arc(c1r, self.rho1, 0.0, PI - self.phi, w),
            arc(c2r, self.rho2, PI, TAU - self.phi, w),
            seg([self.xc - self.a, foot], [self.xc - self.a, self.ys], w),
            seg([self.xc + self.a, foot], [self.xc + self.a, self.ys], w),
        ])
    }
    /// The inside of the arch above its springing, grown by `grow`.
    pub fn inside(&self, grow: f64) -> Shape {
        let (c1r, c2r) = (self.mirror(self.c1), self.mirror(self.c2));
        let yi = self.inflection[1];
        let side = |c1: V, c2: V, keep: Shape, toward: Shape| {
            union(vec![
                inter(vec![disc(c1, self.rho1 + grow), above(self.ys), below(yi + 0.02), keep.clone()]),
                inter(vec![not(disc(c2, self.rho2 - grow)), toward, above(yi - 0.02), below(self.apex[1] + grow), keep]),
            ])
        };
        union(vec![
            side(self.c1, self.c2, left_of(self.xc + 0.02), right_of(self.c2[0])),
            side(c1r, c2r, right_of(self.xc - 0.02), left_of(c2r[0])),
        ])
    }
    /// Points along the outer edge of a band `w` wide, every `pitch` from the apex down to height `down_to`, with the outward direction there.
    pub fn outer_edge(&self, w: f64, pitch: f64, down_to: f64) -> Vec<(V, V)> {
        let mut out = Vec::new();
        let upper = self.rho2 * (TAU - (self.phi + PI));
        let mut s = 0.32;
        while s < upper {
            let psi = TAU - s / self.rho2;
            let p = polar(self.c2, self.rho2 - 0.5 * w, psi);
            out.push((p, unit(sub(self.c2, p))));
            s += pitch;
        }
        let mut s2 = s - upper;
        loop {
            let t = self.phi + s2 / self.rho1;
            let p = polar(self.c1, self.rho1 + 0.5 * w, t);
            if t > PI || p[1] < down_to {
                break;
            }
            out.push((p, unit(sub(p, self.c1))));
            s2 += pitch;
        }
        let mirrored: Vec<(V, V)> = out.iter().map(|(p, d)| (self.mirror(*p), [-d[0], d[1]])).collect();
        out.extend(mirrored);
        out
    }
}

/// Lead lines `w` wide, `pitch` apart, crossing at `±angle` from the vertical through `through`, inside `region` within the box `lo..hi`.
pub fn quarries(region: Shape, lo: V, hi: V, pitch: f64, angle: f64, w: f64, through: V) -> Shape {
    let mut lines = Vec::new();
    for s in [-1.0, 1.0] {
        let d = [s * angle.sin(), angle.cos()];
        let n = [-d[1], d[0]];
        let phase = dot(through, n);
        let corners = [lo, [hi[0], lo[1]], hi, [lo[0], hi[1]]];
        let (mut n0, mut n1, mut t0, mut t1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for c in corners {
            n0 = n0.min(dot(c, n));
            n1 = n1.max(dot(c, n));
            t0 = t0.min(dot(c, d));
            t1 = t1.max(dot(c, d));
        }
        let mut k = ((n0 - phase) / pitch).floor();
        while phase + k * pitch <= n1 {
            let o = phase + k * pitch;
            lines.push(seg(add(mul(n, o), mul(d, t0 - 0.2)), add(mul(n, o), mul(d, t1 + 0.2)), w));
            k += 1.0;
        }
    }
    inter(vec![union(lines), region])
}
