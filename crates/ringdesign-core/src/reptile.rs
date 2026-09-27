//! Sculpted reptile relief, shared by the tile library and collection author.
//! Coordinates are cell units; heights are 0..1. There is no baked lighting.

use crate::field::smoothstep;
use std::f64::consts::TAU;

/// Imbricated, keeled lanceolate scales. Every second transverse row is offset
/// half a scale. Integer along periods and even across periods close exactly.
pub fn snake(along: f64, across: f64) -> f64 {
    let mut h: f64 = 0.0;
    let row = across.round() as i64;
    for j in row - 1..=row + 1 {
        let stagger = 0.5 * j.rem_euclid(2) as f64;
        let col = (along - stagger).round();
        for i in [-1.0, 0.0, 1.0] {
            let x = along - col - stagger - i;
            let y = across - j as f64;
            // A broad heel tapering to a pointed free edge, with a soft bevel.
            let q = (x.abs() / 0.70).powf(1.25) + (y.abs() / 0.55).powf(1.40);
            let edge = 1.0 - smoothstep(0.79, 1.0, q);
            let plate = 0.51 + 0.24 * smoothstep(-0.55, 0.48, x);
            let keel = 0.22 * (-((y / 0.14).powi(2))).exp()
                * (1.0 - smoothstep(0.23, 0.65, x.abs()));
            h = h.max(edge * (plate + keel));
        }
    }
    h.clamp(0.0, 1.0)
}

/// Broad snake belly plates. `across` is -1..1 across a ventral plate;
/// the bowed joints continue into the small flank scales in the collection.
pub fn ventral(along: f64, across: f64) -> f64 {
    let t = (along + 0.13 * across * across).rem_euclid(1.0);
    let joint = smoothstep(0.045, 0.17, t) * (1.0 - smoothstep(0.86, 0.985, t));
    let bow = 0.78 + 0.16 * (std::f64::consts::PI * t).sin();
    joint * bow * (1.0 - 0.10 * across.abs().min(1.0).powi(4))
}

/// Rectangular crocodilian osteoderms with a central keel and shallow grain.
/// The common joint grid joins cleanly when adjacent columns change size.
pub fn crocodile(along: f64, across: f64) -> f64 {
    let x = along.rem_euclid(1.0) - 0.5;
    let y = across.rem_euclid(1.0) - 0.5;
    let q = ((x / 0.48).abs().powi(6) + (y / 0.47).abs().powi(6)).powf(1.0 / 6.0);
    let edge = 1.0 - smoothstep(0.76, 1.0, q);
    let crown = 0.57 + 0.17 * (1.0 - (x / 0.5).powi(2)).max(0.0);
    let keel = 0.23 * (1.0 - smoothstep(0.035, 0.22, y.abs()))
        * (1.0 - smoothstep(0.20, 0.45, x.abs()));
    let grain = 0.055 * (TAU * (y * 5.0 + 0.11 * (TAU * x).sin())).cos()
        * (1.0 - smoothstep(0.55, 0.85, q));
    (edge * (crown + keel + grain)).clamp(0.0, 1.0)
}

/// Six-sided shield scales with broad centres, drafted rims and recessed joints.
pub fn shields(along: f64, across: f64) -> f64 {
    let row = across.round() as i64;
    let mut h: f64 = 0.0;
    for j in row - 1..=row + 1 {
        let stagger = 0.5 * j.rem_euclid(2) as f64;
        let x = (along - stagger + 0.5).rem_euclid(1.0) - 0.5;
        let y = (across - j as f64).abs();
        let q = (x.abs() * 2.0).max(x.abs() + 1.5 * y);
        let edge = 1.0 - smoothstep(0.84, 0.99, q);
        let rim = smoothstep(0.57, 0.73, q) * (1.0 - smoothstep(0.79, 0.94, q));
        h = h.max(edge * (0.58 + 0.14 * (1.0 - q).max(0.0)) + 0.22 * rim);
    }
    h.clamp(0.0, 1.0)
}

/// Unit-square library tiles. Each includes enough cells to show the stagger.
pub fn snake_tile(x: f64, y: f64) -> f64 { snake(x * 4.0, y * 4.0) }
pub fn ventral_tile(x: f64, y: f64) -> f64 {
    ventral(x * 4.0, (y * TAU).cos())
}
pub fn crocodile_tile(x: f64, y: f64) -> f64 { crocodile(x * 3.0, y * 3.0) }
pub fn shield_tile(x: f64, y: f64) -> f64 { shields(x * 4.0, y * 4.0) }

/// The collection's reptile skins as SVG, one pattern period per tile (C-R7).
///
/// A tile is drawn in millimetres, `pitch_mm` round the ring by `height_mm`
/// across the band, so the tiling that lays it one cell per tile lays every
/// feature at the size it was drawn. A builtin tile carries several periods
/// and falls under the detail floor at a ring's tightest station; these carry
/// one, and each is tested there with `Alpha::min_feature_px`, ink **and**
/// gaps, against 0.40 mm. Features reaching over an edge are drawn again on
/// the far side, so every tile is seamless. Ink is black: a dome is a radial
/// opacity gradient whose 0.5 iso-line, where the detail floor thresholds,
/// is the size the generator solved for.
pub mod svg {
    use serde::{Deserialize, Serialize};
    use std::fmt::Write as _;

    /// What a generator draws: the tile and the metal it leaves.
    #[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
    pub struct Params {
        /// The tile round the ring, mm: one pattern period.
        pub pitch_mm: f64,
        /// The tile across the band, mm.
        pub height_mm: f64,
        /// Metal left between features at the 0.5 iso-line, mm: a land, a
        /// joint or a trailing drop, whichever the pattern has.
        pub land_mm: f64,
        /// How a feature's top stands, 0 a plateau with a hard edge to 1 a
        /// full dome.
        pub dome: f64,
        /// Seeds the scattered patterns; the others ignore it.
        #[serde(default)]
        pub seed: u64,
        /// A dome's gradient focus, off its centre as a share of its radius
        /// round the ring and across the band: a bead ramps gently away from
        /// it and drops steeply toward it.
        #[serde(default)]
        pub focus: [f64; 2],
    }

    impl Params {
        pub const fn new(pitch_mm: f64, height_mm: f64, land_mm: f64, dome: f64) -> Self {
            Self { pitch_mm, height_mm, land_mm, dome, seed: 0, focus: [0.0, 0.0] }
        }

        fn wh(&self) -> (f64, f64) {
            (fin(self.pitch_mm, 0.1), fin(self.height_mm, 0.1))
        }

        fn land(&self) -> f64 {
            if self.land_mm.is_finite() { self.land_mm.max(0.0) } else { 0.4 }
        }

        fn dome(&self) -> f64 {
            if self.dome.is_finite() { self.dome.clamp(0.0, 1.0) } else { 1.0 }
        }
    }

    fn fin(x: f64, floor: f64) -> f64 {
        if x.is_finite() { x.max(floor) } else { floor }
    }

    /// A generator: its name, the SVG it draws, and the tile it is tested at,
    /// its ring's tightest station.
    pub struct Generator {
        pub name: &'static str,
        pub draw: fn(&Params) -> String,
        pub tightest: Params,
        /// Where `tightest` comes from.
        pub station: &'static str,
    }

    /// Every generator, in the order the collection doc lists them.
    pub const GENERATORS: &[Generator] = &[
        Generator { name: "sail", draw: sail, tightest: Params::new(1.41, 0.95, 0.4, 1.0), station: "Sphenodon's palm: 44 teeth round a 79.8 mm crest graded 0.4, 1.81 x sqrt(0.6), over a 0.95 mm sail" },
        Generator { name: "tubercle_rows", draw: tubercle_rows, tightest: Params::new(1.84, 2.2, 0.4, 1.0), station: "Sphenodon's palm: a 2.2 mm side face in square cells graded 0.3" },
        Generator { name: "paver", draw: paver, tightest: Params::new(0.98, 0.98, 0.45, 0.35), station: "Heloderma's palm: the 0.98 mm pitch, lands 0.45" },
        Generator { name: "granules", draw: granules, tightest: Params::new(1.8, 1.559, 0.4, 1.0), station: "Moloch's palm granules: 0.9 mm pitch, 0.5 across, in a two-by-two hex cell" },
        Generator { name: "bead_lattice", draw: bead_lattice, tightest: Params { focus: [0.0, 0.35], ..Params::new(1.96, 1.697, 0.412, 1.0) }, station: "Heloderma's palm: 0.98 mm bead pitch, beads 0.58 and lands 0.42 of it" },
        Generator { name: "reticulation", draw: reticulation, tightest: Params::new(12.0, 8.0, 1.5, 1.0), station: "Heloderma's mask: forking bands 1.5 mm across under a 1.2 mm blur" },
        Generator { name: "rosette_thorn", draw: rosette_thorn, tightest: Params::new(3.0, 3.0, 0.45, 1.0), station: "Moloch's side faces: the smallest rosette whose ring keeps 0.45 lands and 0.4 granules" },
        Generator { name: "lamella", draw: lamella, tightest: Params::new(0.9, 4.0, 0.42, 1.0), station: "Gekko's palm: 0.9 mm plates with a 0.4 mm drop" },
        Generator { name: "granule_spots", draw: granule_spots, tightest: Params::new(9.0, 9.0, 0.6, 0.2), station: "Gekko's crown: spots 1.5-3 mm across" },
        Generator { name: "whorl", draw: whorl, tightest: Params::new(1.0, 4.0, 0.42, 1.0), station: "Ouroborus's tail tip: 1.0 mm girdles, a 0.6 loaf and a 0.4 drop" },
        Generator { name: "whorl_spine", draw: whorl_spine, tightest: Params::new(1.0, 0.94, 0.4, 1.0), station: "Ouroborus's tail tip: a 1.0 mm girdle on a 0.94 mm side face" },
        Generator { name: "plate_voronoi", draw: plate_voronoi, tightest: Params::new(6.0, 6.0, 0.42, 0.3), station: "Phrynosoma's table: plates of 1.2 mm and more; the ring's 0.35 mm joints sit under the collection's 0.40 floor" },
        Generator { name: "scute", draw: scute, tightest: Params::new(3.1, 4.4, 0.42, 0.4), station: "Chelonia's vertebrals: along pitch 3.1, across 2.2 either side" },
        Generator { name: "shingle", draw: shingle, tightest: Params::new(1.4, 6.0, 0.42, 1.0), station: "Chelonia's hawksbill plates at their 1.4 mm end" },
        Generator { name: "plastron", draw: plastron, tightest: Params::new(3.0, 6.0, 0.4, 0.3), station: "Chelonia's palm plates" },
        Generator { name: "fringe", draw: fringe, tightest: Params::new(1.2, 1.5, 0.4, 0.5), station: "Phrynosoma's walls at the fringe's graded end" },
        Generator { name: "rosette_tubercle", draw: rosette_tubercle, tightest: Params::new(3.0, 3.0, 0.45, 1.0), station: "Phrynosoma's cheeks: lands 0.45" },
        Generator { name: "granule_voronoi", draw: granule_voronoi, tightest: Params::new(5.0, 5.0, 0.4, 1.0), station: "Chamaeleo's walls: three radii, lands 0.4" },
        Generator { name: "flat_tubercle", draw: flat_tubercle, tightest: Params::new(2.4, 2.0, 0.4, 0.25), station: "Chamaeleo's lateral stripe" },
    ];

    /// The generator named `name`.
    pub fn generator(name: &str) -> Option<&'static Generator> {
        GENERATORS.iter().find(|g| g.name == name)
    }

    /// `name`'s SVG for `p`, or `None` for an unknown name: what a script
    /// node calls so a template can expose pitch, land and dome.
    pub fn draw(name: &str, p: &Params) -> Option<String> {
        generator(name).map(|g| (g.draw)(p))
    }

    // --- The page ------------------------------------------------------------

    /// How a feature's height falls from its middle to its rim.
    #[derive(Clone, Copy, PartialEq)]
    enum Fall {
        /// A plateau, then a quarter circle over the last `bevel` of the radius.
        Dome { bevel: f64 },
        /// A flat tip `tip` of the radius across, then straight down.
        Cone { tip: f64 },
    }

    impl Fall {
        fn of(p: &Params) -> Self {
            Fall::Dome { bevel: p.dome().max(0.04) }
        }

        /// Where the height falls through one half, as a share of the radius.
        fn iso(self) -> f64 {
            match self {
                Fall::Dome { bevel } => 1.0 - bevel + bevel * 0.75f64.sqrt(),
                Fall::Cone { tip } => 0.5 * (1.0 + tip),
            }
        }

        fn stops(self) -> Vec<(f64, f64)> {
            match self {
                Fall::Dome { bevel } => {
                    let mut s = vec![(0.0, 1.0), (1.0 - bevel, 1.0)];
                    s.extend((1..=10).map(|k| {
                        let t = k as f64 / 10.0;
                        (1.0 - bevel + bevel * t, (1.0 - t * t).max(0.0).sqrt())
                    }));
                    s
                }
                Fall::Cone { tip } => vec![(0.0, 1.0), (tip, 1.0), (1.0, 0.0)],
            }
        }
    }

    struct Page {
        w: f64,
        h: f64,
        defs: String,
        body: String,
        ids: Vec<String>,
    }

    impl Page {
        fn new(p: &Params) -> Self {
            let (w, h) = p.wh();
            Self { w, h, defs: String::new(), body: String::new(), ids: Vec::new() }
        }

        /// A radial gradient for `fall` focused off centre by `focus`; shared by every feature drawn with it.
        fn radial(&mut self, fall: Fall, focus: [f64; 2]) -> String {
            let key = format!("{:?}{:?}", fall.stops(), focus);
            if let Some(i) = self.ids.iter().position(|k| *k == key) {
                return format!("r{i}");
            }
            let id = format!("r{}", self.ids.len());
            let f = |x: f64| 0.5 + 0.5 * x.clamp(-0.9, 0.9);
            let _ = write!(self.defs, r##"<radialGradient id="{id}" cx="0.5" cy="0.5" r="0.5" fx="{:.4}" fy="{:.4}">"##, f(focus[0]), f(focus[1]));
            for (at, ink) in fall.stops() {
                let _ = write!(self.defs, r##"<stop offset="{at:.4}" stop-color="#000" stop-opacity="{ink:.4}"/>"##);
            }
            self.defs.push_str("</radialGradient>");
            self.ids.push(key);
            id
        }

        /// Every copy of a feature at `(x, y)` reaching `r` that shows in the tile.
        fn copies(&self, x: f64, y: f64, r: f64) -> Vec<(f64, f64)> {
            let mut out = Vec::new();
            for dx in [-self.w, 0.0, self.w] {
                for dy in [-self.h, 0.0, self.h] {
                    let (cx, cy) = (x + dx, y + dy);
                    if cx + r > 0.0 && cx - r < self.w && cy + r > 0.0 && cy - r < self.h {
                        out.push((cx, cy));
                    }
                }
            }
            out
        }

        /// A round feature whose half-height line is `d` across.
        fn bead(&mut self, x: f64, y: f64, d: f64, fall: Fall, focus: [f64; 2]) {
            if !(d > 0.0) {
                return;
            }
            let r = 0.5 * d / fall.iso();
            let id = self.radial(fall, focus);
            for (cx, cy) in self.copies(x, y, r) {
                let _ = write!(self.body, r##"<circle cx="{cx:.4}" cy="{cy:.4}" r="{r:.4}" fill="url(#{id})"/>"##);
            }
        }

        /// A polygon in ink `ink`, wrapped.
        fn polygon(&mut self, pts: &[[f64; 2]], fill: &str) {
            if pts.len() < 3 {
                return;
            }
            let (cx, cy) = (pts.iter().map(|p| p[0]).sum::<f64>() / pts.len() as f64, pts.iter().map(|p| p[1]).sum::<f64>() / pts.len() as f64);
            let r = pts.iter().map(|p| (p[0] - cx).hypot(p[1] - cy)).fold(0.0, f64::max);
            for (x, y) in self.copies(cx, cy, r) {
                let (dx, dy) = (x - cx, y - cy);
                self.body.push_str(r##"<polygon points=""##);
                for p in pts {
                    let _ = write!(self.body, "{:.4},{:.4} ", p[0] + dx, p[1] + dy);
                }
                let _ = write!(self.body, r##"" fill="{fill}"/>"##);
            }
        }

        /// A horizontal or vertical ramp of ink across the tile, `stops` as (share, ink).
        fn linear(&mut self, vertical: bool, stops: &[(f64, f64)]) -> String {
            self.ramp(vertical, stops, "#000")
        }

        /// [`linear`](Self::linear) in `color`: white for a luminance mask.
        fn ramp(&mut self, vertical: bool, stops: &[(f64, f64)], color: &str) -> String {
            let id = format!("l{}", self.ids.len());
            self.ids.push(id.clone());
            let (x2, y2) = if vertical { (0, 1) } else { (1, 0) };
            let _ = write!(self.defs, r##"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="{:.4}" y2="{:.4}">"##, x2 as f64 * self.w, y2 as f64 * self.h);
            for (at, ink) in stops {
                let _ = write!(self.defs, r##"<stop offset="{:.4}" stop-color="{color}" stop-opacity="{:.4}"/>"##, at.clamp(0.0, 1.0), ink.clamp(0.0, 1.0));
            }
            self.defs.push_str("</linearGradient>");
            id
        }

        fn finish(self) -> String {
            let (w, h) = (self.w, self.h);
            format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{w:.4}" height="{h:.4}" viewBox="0 0 {w:.4} {h:.4}"><defs>{}</defs>{}</svg>"##, self.defs, self.body)
        }
    }

    /// Points `n` round a hexagon-staggered cell of `cols` by `rows`: every other row shifted half a column.
    fn staggered(w: f64, h: f64, cols: usize, rows: usize) -> Vec<[f64; 2]> {
        let (cw, rh) = (w / cols as f64, h / rows as f64);
        (0..rows).flat_map(|j| (0..cols).map(move |i| [(i as f64 + 0.25 + 0.5 * (j % 2) as f64) * cw, (j as f64 + 0.5) * rh])).collect()
    }

    /// Nearest distance between two points of a periodic set.
    fn nearest(pts: &[[f64; 2]], w: f64, h: f64) -> f64 {
        let mut best = f64::MAX;
        for (i, a) in pts.iter().enumerate() {
            for (j, b) in pts.iter().enumerate() {
                for dx in [-w, 0.0, w] {
                    for dy in [-h, 0.0, h] {
                        if i == j && dx == 0.0 && dy == 0.0 {
                            continue;
                        }
                        best = best.min((a[0] - b[0] - dx).hypot(a[1] - b[1] - dy));
                    }
                }
            }
        }
        best
    }

    /// A deterministic stream on 0..1.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> f64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    /// Discs of the given diameters dropped at random, each kept only where it leaves `land` to every disc already down, round the torus.
    fn scatter(w: f64, h: f64, land: f64, sizes: &[f64], seed: u64, tries: usize) -> Vec<([f64; 2], f64)> {
        let mut rng = Rng(seed ^ 0x5EED);
        let mut out: Vec<([f64; 2], f64)> = Vec::new();
        for k in 0..tries {
            let d = sizes[k % sizes.len()];
            let p = [rng.next() * w, rng.next() * h];
            let clear = out.iter().all(|(q, e)| {
                let dx = (p[0] - q[0]).abs().min(w - (p[0] - q[0]).abs());
                let dy = (p[1] - q[1]).abs().min(h - (p[1] - q[1]).abs());
                dx.hypot(dy) >= 0.5 * (d + e) + land
            });
            if clear {
                out.push((p, d));
            }
        }
        out
    }

    // --- The generators --------------------------------------------------------

    /// Sphenodon's sail: one tooth per tile. Round the ring a triangular
    /// tooth with a 0.35 mm blunted tip and a 0.4 mm rounded valley; across,
    /// a gable peaked on the tile's middle row, so the fin straddles the
    /// parting line and its flanks face the pull (G2).
    pub fn sail(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h) = (page.w, page.h);
        let tip = (0.5_f64 * 0.35).min(0.25 * w);
        let valley = (0.5_f64 * 0.4).min(0.25 * w);
        let slope = 1.0 / (0.5 * w - tip).max(1e-6);
        let tooth = |x: f64| {
            let s = (x - 0.5 * w).abs();
            if s <= tip {
                return 1.0;
            }
            let d = 0.5 * w - s;
            if d >= valley { d * slope } else { slope * (d * d + valley * valley) / (2.0 * valley) }
        };
        let across = page.linear(true, &[(0.0, 0.0), (0.5, 1.0), (1.0, 0.0)]);
        let round: Vec<(f64, f64)> = (0..=40).map(|k| {
            let t = k as f64 / 40.0;
            (t, tooth(t * w))
        }).collect();
        // A luminance mask keeps what is white: the tooth in white over black, times the gable.
        let along = page.ramp(false, &round, "#fff");
        let _ = write!(page.defs, r##"<mask id="tooth" maskUnits="userSpaceOnUse" x="0" y="0" width="{w:.4}" height="{h:.4}"><rect width="{w:.4}" height="{h:.4}" fill="#000"/><rect width="{w:.4}" height="{h:.4}" fill="url(#{along})"/></mask>"##);
        let _ = write!(page.body, r##"<rect width="{w:.4}" height="{h:.4}" fill="url(#{across})" mask="url(#tooth)"/>"##);
        page.finish()
    }

    /// Three rows of small granules and one row of domed tubercles per tile.
    pub fn tubercle_rows(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let row = h / 5.0;
        // The tubercles take two fifths of the height, the granules a fifth each.
        let big = (2.0 * row).min(w) - land;
        page.bead(0.5 * w, 1.0 * row, big, Fall::of(p), p.focus);
        let small = row - land;
        let n = ((w / (small + land)).floor() as usize).max(1);
        for j in 0..3 {
            let y = (2.5 + j as f64) * row;
            for i in 0..n {
                let x = (i as f64 + 0.5 * (j % 2) as f64 + 0.25) * w / n as f64;
                page.bead(x, y, small.min(w / n as f64 - land), Fall::of(p), [0.0, 0.0]);
            }
        }
        page.finish()
    }

    /// A rounded square paver per tile, lands all round.
    pub fn paver(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let (sw, sh) = ((w - land).max(0.05), (h - land).max(0.05));
        let r = 0.25 * sw.min(sh);
        let bevel = 0.5 * p.dome() * sw.min(sh);
        // The half-height line sits on the drawn square: the bevel straddles it.
        let _ = write!(page.defs, r##"<filter id="bevel" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="{:.4}"/></filter>"##, 0.25 * bevel);
        let _ = write!(page.body, r##"<g filter="url(#bevel)"><rect x="{:.4}" y="{:.4}" width="{sw:.4}" height="{sh:.4}" rx="{r:.4}" fill="#000"/></g>"##, 0.5 * land, 0.5 * land);
        page.finish()
    }

    /// Round granules in a two-by-two hexagon-staggered cell.
    pub fn granules(p: &Params) -> String {
        let mut page = Page::new(p);
        let pts = staggered(page.w, page.h, 2, 2);
        let d = nearest(&pts, page.w, page.h) - p.land();
        for q in pts {
            page.bead(q[0], q[1], d, Fall::of(p), p.focus);
        }
        page.finish()
    }

    /// Heloderma's beads: a two-by-two hexagon-staggered cell of domes 0.58
    /// of the pitch across with the focus toward the band's edge, so each
    /// bead ramps gently on its crest side and drops steeply on its edge side.
    pub fn bead_lattice(p: &Params) -> String {
        let mut page = Page::new(p);
        let pts = staggered(page.w, page.h, 2, 2);
        let d = nearest(&pts, page.w, page.h) - p.land();
        for q in pts {
            page.bead(q[0], q[1], d, Fall::of(p), p.focus);
        }
        page.finish()
    }

    /// Heloderma's reticulation: transverse bands `land_mm` wide that fork
    /// and rejoin, a net of Y-junctions periodic both ways, under a blur a
    /// tenth of the pitch wide at full `dome`.
    pub fn reticulation(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, band) = (page.w, page.h, p.land());
        let blur = 0.1 * w * p.dome();
        let _ = write!(page.defs, r##"<filter id="soft" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="{:.4}"/></filter>"##, 0.5 * blur);
        let _ = write!(page.body, r##"<g filter="url(#soft)" fill="none" stroke="#000" stroke-linecap="round" stroke-width="{band:.4}">"##);
        let (q, t, m, f) = (0.25 * w, h / 3.0, 0.5 * h, 5.0 * h / 6.0);
        let edges = [
            [[0.0, 0.0], [0.0, t]], [[2.0 * q, 0.0], [2.0 * q, t]],
            [[0.0, t], [q, m]], [[0.0, t], [-q, m]], [[2.0 * q, t], [q, m]], [[2.0 * q, t], [3.0 * q, m]],
            [[q, m], [q, f]], [[3.0 * q, m], [3.0 * q, f]],
            [[q, f], [0.0, h]], [[q, f], [2.0 * q, h]], [[3.0 * q, f], [2.0 * q, h]], [[3.0 * q, f], [w, h]],
        ];
        for dx in [-w, 0.0, w] {
            for dy in [-h, 0.0, h] {
                for [a, b] in edges {
                    let _ = write!(page.body, r##"<path d="M{:.4},{:.4} L{:.4},{:.4}"/>"##, a[0] + dx, a[1] + dy, b[0] + dx, b[1] + dy);
                }
            }
        }
        page.body.push_str("</g>");
        page.finish()
    }

    /// A central feature ringed by six granules, the ring solved so every land is `land_mm`.
    fn rosette(p: &Params, centre: Fall) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let reach = 0.5 * w.min(h);
        // Centre `c`, granules half its size: reach = c/2 + land + g, g = c/2, less half a land to the next tile.
        let c = ((reach - 1.5 * land) / 1.0).max(0.05);
        let g = 0.5 * c;
        let ring = 0.5 * c + land + 0.5 * g;
        page.bead(0.5 * w, 0.5 * h, c, centre, p.focus);
        for k in 0..6 {
            let a = k as f64 * std::f64::consts::TAU / 6.0 + std::f64::consts::FRAC_PI_6;
            page.bead(0.5 * w + ring * a.cos(), 0.5 * h + ring * a.sin(), g, Fall::of(p), [0.0, 0.0]);
        }
        page.finish()
    }

    /// Moloch's thorn rosette: a cone to a 0.3 mm flat tip ringed by six granule domes.
    pub fn rosette_thorn(p: &Params) -> String {
        let reach = 0.5 * p.wh().0.min(p.wh().1);
        let c = (reach - 1.5 * p.land()).max(0.05);
        rosette(p, Fall::Cone { tip: (0.3 / (c / Fall::Cone { tip: 0.0 }.iso())).clamp(0.0, 0.8) })
    }

    /// Phrynosoma's cheek tubercle: a dome ringed by six granules.
    pub fn rosette_tubercle(p: &Params) -> String {
        rosette(p, Fall::of(p))
    }

    /// A u-sawtooth: a plate rising gently from its leading joint to its
    /// trailing edge, then a `land_mm` trough at the foot of its drop. It
    /// varies only round the ring, so it is legal on a crown (G4).
    fn sawtooth(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h) = (page.w, page.h);
        let drop = p.land().min(0.8 * w);
        let loaf = (w - drop) / w;
        let steep = 0.02_f64.min(0.25 * drop / w);
        // The loaf starts at 0.55 so all of it stands over the half-height
        // line, and the drop crosses that line exactly `drop` before the joint.
        // The tile's edge sits mid-trough, so the joint's own step is inside it.
        let start = 0.5 * drop / w;
        let top = start + loaf - 0.5 * steep;
        let mut stops = vec![(0.0, 0.0), (start - 0.002, 0.0)];
        stops.extend((0..=12).map(|k| {
            let t = k as f64 / 12.0;
            (start + t * (top - start), 0.55 + 0.45 * (t * std::f64::consts::FRAC_PI_2).sin().powf(1.0 + p.dome()))
        }));
        stops.push((start + loaf + 0.5 * steep, 0.0));
        stops.push((1.0, 0.0));
        let id = page.linear(false, &stops);
        let _ = write!(page.body, r##"<rect width="{w:.4}" height="{h:.4}" fill="url(#{id})"/>"##);
        page.finish()
    }

    /// Gekko's lamella: one plate per tile, a loaf rising to a 0.4 mm trailing drop.
    pub fn lamella(p: &Params) -> String {
        sawtooth(p)
    }

    /// Ouroborus's whorl: one girdle per tile, the lamella's sawtooth at its own pitch.
    pub fn whorl(p: &Params) -> String {
        sawtooth(p)
    }

    /// Gekko's spots: irregular round plateaus 1.5 to 3 mm across, scattered and seeded, a low-frequency mask.
    pub fn granule_spots(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h) = (page.w, page.h);
        for (q, d) in scatter(w, h, p.land(), &[3.0, 2.2, 1.5], p.seed, 60) {
            page.bead(q[0], q[1], d, Fall::of(p), p.focus);
        }
        page.finish()
    }

    /// A blunt spine pointing toward +u: a trapezoid `len` long from a base
    /// `base` across to a flat tip, filled to the half-height line with a
    /// blur for its flanks.
    fn spine(p: &Params, len: f64, base: f64, x_tip: f64, y: f64) -> String {
        let mut page = Page::new(p);
        // Blunt, 0.8 of the base: a feather tip will not fill, and at 0.6 the tail tip's spine reads 0.399 mm of ink.
        let tip = (0.8 * base).max(0.3_f64.min(0.9 * base));
        let pts = [[x_tip - len, y - 0.5 * base], [x_tip, y - 0.5 * tip], [x_tip, y + 0.5 * tip], [x_tip - len, y + 0.5 * base]];
        let blur = 0.15 * p.dome() * base;
        let _ = write!(page.defs, r##"<filter id="flank" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="{blur:.4}"/></filter>"##);
        page.body.push_str(r##"<g filter="url(#flank)">"##);
        page.polygon(&pts, "#000");
        page.body.push_str("</g>");
        page.finish()
    }

    /// Ouroborus's spine: a blunt pyramid on the girdle's trailing edge pointing toward +u, lands all round.
    pub fn whorl_spine(p: &Params) -> String {
        let (w, h) = p.wh();
        let land = p.land();
        spine(p, (w - land).max(0.1), (h - land).max(0.1), w - 0.5 * land, 0.5 * h)
    }

    /// Phrynosoma's fringe: pointed scales tipped palmward (+u), one per tile.
    pub fn fringe(p: &Params) -> String {
        let (w, h) = p.wh();
        let land = p.land();
        spine(p, (w - land).max(0.1), (h - land).max(0.1), w - 0.5 * land, 0.5 * h)
    }

    /// The convex cell of seed `i` among `seeds` round the torus, pulled in by half a joint on every side.
    fn cell(seeds: &[[f64; 2]], i: usize, w: f64, h: f64, joint: f64) -> Vec<[f64; 2]> {
        let a = seeds[i];
        let reach = 2.0 * w.max(h);
        let mut poly = vec![[a[0] - reach, a[1] - reach], [a[0] + reach, a[1] - reach], [a[0] + reach, a[1] + reach], [a[0] - reach, a[1] + reach]];
        for (j, b) in seeds.iter().enumerate() {
            for dx in [-w, 0.0, w] {
                for dy in [-h, 0.0, h] {
                    if j == i && dx == 0.0 && dy == 0.0 {
                        continue;
                    }
                    let b = [b[0] + dx, b[1] + dy];
                    let n = [b[0] - a[0], b[1] - a[1]];
                    let l = n[0].hypot(n[1]);
                    if l < 1e-9 {
                        continue;
                    }
                    let n = [n[0] / l, n[1] / l];
                    let limit = 0.5 * l - 0.5 * joint;
                    let side = |q: &[f64; 2]| (q[0] - a[0]) * n[0] + (q[1] - a[1]) * n[1] - limit;
                    let mut out = Vec::with_capacity(poly.len() + 1);
                    for k in 0..poly.len() {
                        let (p0, p1) = (poly[k], poly[(k + 1) % poly.len()]);
                        let (s0, s1) = (side(&p0), side(&p1));
                        if s0 <= 0.0 {
                            out.push(p0);
                        }
                        if (s0 <= 0.0) != (s1 <= 0.0) {
                            let t = s0 / (s0 - s1);
                            out.push([p0[0] + (p1[0] - p0[0]) * t, p0[1] + (p1[1] - p0[1]) * t]);
                        }
                    }
                    poly = out;
                    if poly.len() < 3 {
                        return poly;
                    }
                }
            }
        }
        poly
    }

    /// Polygonal plates from seeds mirrored across the tile's middle row (the
    /// parting line), `land_mm` joints softened to a V by a blur, every plate
    /// full height. A tier is the layer's remap or a second layer's work.
    pub fn plate_voronoi(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h) = (page.w, page.h);
        let mut rng = Rng(p.seed ^ 0x9A7E);
        let half: Vec<[f64; 2]> = staggered(w, 0.5 * h, 3, 2).into_iter().map(|q| {
            let j = 0.18 * w / 3.0;
            [q[0] + j * (rng.next() - 0.5), (q[1] + j * (rng.next() - 0.5)).clamp(0.1 * h, 0.45 * h)]
        }).collect();
        let seeds: Vec<[f64; 2]> = half.iter().copied().chain(half.iter().map(|q| [q[0], h - q[1]])).collect();
        let blur = 0.25 * p.dome() * p.land();
        let _ = write!(page.defs, r##"<filter id="joint" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="{blur:.4}"/></filter>"##);
        page.body.push_str(r##"<g filter="url(#joint)">"##);
        for i in 0..seeds.len() {
            let c = cell(&seeds, i, w, h, p.land());
            page.polygon(&c, "#000");
        }
        page.body.push_str("</g>");
        page.finish()
    }

    /// Chelonia's scute: a hexagon a joint in from the tile's edge, its areola a radial gradient focused by `focus`
    /// (`[0, 0]` centred, toward the line for a costal), standing over the half-height line to its rim.
    pub fn scute(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, j) = (page.w, page.h, p.land());
        let (a, b) = (0.5 * w - 0.5 * j, 0.5 * h - 0.5 * j);
        let (cx, cy) = (0.5 * w, 0.5 * h);
        let pts = [[cx - a, cy - 0.5 * b], [cx, cy - b], [cx + a, cy - 0.5 * b], [cx + a, cy + 0.5 * b], [cx, cy + b], [cx - a, cy + 0.5 * b]];
        let f = |x: f64| 0.5 + 0.5 * x.clamp(-0.9, 0.9);
        let _ = write!(page.defs, r##"<radialGradient id="areola" cx="0.5" cy="0.5" r="0.6" fx="{:.4}" fy="{:.4}"><stop offset="0" stop-color="#000"/><stop offset="{:.4}" stop-color="#000" stop-opacity="0.6"/><stop offset="1" stop-color="#000" stop-opacity="0.6"/></radialGradient>"##, f(p.focus[0]), f(p.focus[1]), 1.0 - 0.5 * p.dome());
        page.polygon(&pts, "url(#areola)");
        page.finish()
    }

    /// Chelonia's hawksbill shingle: a plate per tile whose free edge is a U
    /// leading toward +u with its apex on the middle row, rising toward that
    /// edge, a `land_mm` trough at its foot.
    pub fn shingle(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let bow = (0.35 * w).min(w - land - 0.4);
        let edge = |y: f64| w - 0.5 * land - bow * ((y - 0.5 * h) / (0.5 * h)).powi(2);
        let n = 24;
        let mut pts: Vec<[f64; 2]> = (0..=n).map(|k| {
            let y = k as f64 / n as f64 * h;
            [edge(y), y]
        }).collect();
        pts.extend((0..=n).rev().map(|k| {
            let y = k as f64 / n as f64 * h;
            [edge(y) - w + land, y]
        }));
        let rise = page.linear(false, &[(0.0, 0.55), (1.0 - bow / w, 0.8), (1.0, 1.0)]);
        // Drawn once and once a tile to the left, so the plate running in from the previous tile shows.
        for dx in [-w, 0.0, w] {
            page.body.push_str(r##"<polygon points=""##);
            for q in &pts {
                let _ = write!(page.body, "{:.4},{:.4} ", q[0] + dx, q[1]);
            }
            let _ = write!(page.body, r##"" fill="url(#{rise})"/>"##);
        }
        page.finish()
    }

    /// Chelonia's plastron: broad plates with transverse seams, `land_mm` wide, a gentle dome round the ring.
    pub fn plastron(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let s = land / w;
        let lift = 0.55 + 0.45 * (1.0 - p.dome());
        let id = page.linear(false, &[(0.0, 0.0), (0.5 * s, 0.0), (0.5 * s + 0.001, lift), (0.5, 1.0), (1.0 - 0.5 * s - 0.001, lift), (1.0 - 0.5 * s, 0.0), (1.0, 0.0)]);
        let _ = write!(page.body, r##"<rect width="{w:.4}" height="{h:.4}" fill="url(#{id})"/>"##);
        page.finish()
    }

    /// Chamaeleo's granules: three radii scattered round the torus, every land at least `land_mm`.
    pub fn granule_voronoi(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h) = (page.w, page.h);
        for (q, d) in scatter(w, h, p.land(), &[1.0, 0.7, 0.5], p.seed, 400) {
            page.bead(q[0], q[1], d, Fall::of(p), p.focus);
        }
        page.finish()
    }

    /// Chamaeleo's flat tubercles: flat-topped hexagons, two to a tile, staggered, `land_mm` apart.
    pub fn flat_tubercle(p: &Params) -> String {
        let mut page = Page::new(p);
        let (w, h, land) = (page.w, page.h, p.land());
        let blur = 0.1 * p.dome() * w.min(h);
        let _ = write!(page.defs, r##"<filter id="cushion" x="-0.5" y="-0.5" width="2" height="2"><feGaussianBlur stdDeviation="{blur:.4}"/></filter>"##);
        page.body.push_str(r##"<g filter="url(#cushion)">"##);
        for q in staggered(w, h, 1, 2) {
            let (a, b) = (0.5 * w - 0.5 * land, 0.25 * h - 0.5 * land);
            let pts = [[q[0] - a, q[1]], [q[0] - 0.5 * a, q[1] - b], [q[0] + 0.5 * a, q[1] - b], [q[0] + a, q[1]], [q[0] + 0.5 * a, q[1] + b], [q[0] - 0.5 * a, q[1] + b]];
            page.polygon(&pts, "#000");
        }
        page.body.push_str("</g>");
        page.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::svg::{GENERATORS, Params};
    use crate::svg::SvgAlpha;

    /// `f` on a pool of one thread. The floor test measures nineteen rasters at the bake's 1024 px, about 110 s of
    /// CPU; on every core it held three of CI's four for 40 s and starved the suite's wall-clock tests beside it.
    fn on_one_thread<R: Send>(f: impl FnOnce() -> R + Send) -> R {
        #[cfg(feature = "parallel")]
        {
            rayon::ThreadPoolBuilder::new().num_threads(1).build().expect("a one-thread pool").install(f)
        }
        #[cfg(not(feature = "parallel"))]
        {
            f()
        }
    }

    /// Every generator at its ring's tightest station keeps ink and gaps at
    /// or over 0.40 mm where the detail floor reads them, rasterized the way
    /// a design bakes it, and closes on itself at the tile's edges.
    #[test]
    fn every_reptile_skin_holds_the_detail_floor_at_its_tightest_station() {
        on_one_thread(every_skin_holds_the_floor);
    }

    fn every_skin_holds_the_floor() {
        let mut failed = Vec::new();
        for g in GENERATORS {
            let svg = (g.draw)(&g.tightest);
            let a = SvgAlpha { name: g.name.into(), svg, invert: false }.rasterize();
            assert!(a.width > 0 && a.height > 0, "{} does not parse", g.name);
            let mm = g.tightest.pitch_mm / a.width as f64;
            assert!((g.tightest.height_mm / a.height as f64 / mm - 1.0).abs() < 0.02, "{}: square texels", g.name);
            let (ink, gap) = a.min_feature_px().unwrap_or_else(|| panic!("{} has one phase", g.name));
            let (ink, gap) = (ink * mm, gap * mm);
            eprintln!("{:16} ink {ink:.3} mm, gaps {gap:.3} mm on {:.2} x {:.2} ({})", g.name, g.tightest.pitch_mm, g.tightest.height_mm, g.station);
            if ink < 0.40 || gap < 0.40 {
                failed.push(format!("{}: ink {ink:.3}, gaps {gap:.3}", g.name));
            }
            // One period: the seam steps no more than the pattern does inside.
            let (w, h) = (a.width, a.height);
            let at = |x: usize, y: usize| a.data[y * w + x];
            let interior = (0..h).flat_map(|y| (0..w - 1).map(move |x| (x, y))).map(|(x, y)| (at(x + 1, y) - at(x, y)).abs()).fold(0.0f32, f32::max);
            let seam_u = (0..h).map(|y| (at(0, y) - at(w - 1, y)).abs()).fold(0.0f32, f32::max);
            let seam_v = (0..w).map(|x| (at(x, 0) - at(x, h - 1)).abs()).fold(0.0f32, f32::max);
            let interior_v = (0..h - 1).flat_map(|y| (0..w).map(move |x| (x, y))).map(|(x, y)| (at(x, y + 1) - at(x, y)).abs()).fold(0.0f32, f32::max);
            assert!(seam_u <= interior * 1.5 + 0.02, "{} seams round the ring: {seam_u} against {interior}", g.name);
            assert!(seam_v <= interior_v * 1.5 + 0.02, "{} seams across: {seam_v} against {interior_v}", g.name);
        }
        assert!(failed.is_empty(), "under the 0.40 mm floor: {failed:#?}");
    }

    /// A generator's pitch, land and dome are what a template exposes: a wider land widens the gaps.
    #[test]
    fn a_skin_answers_its_land() {
        on_one_thread(a_skin_answers_its_land_on_this_thread);
    }

    fn a_skin_answers_its_land_on_this_thread() {
        let g = super::svg::generator("granules").unwrap();
        let gaps = |land: f64| {
            let p = Params { land_mm: land, ..g.tightest };
            let a = SvgAlpha { name: "g".into(), svg: (g.draw)(&p), invert: false }.rasterize();
            a.min_feature_px().unwrap().1 * p.pitch_mm / a.width as f64
        };
        let (narrow, wide) = (gaps(0.4), gaps(0.55));
        assert!(wide > narrow + 0.1, "{narrow} -> {wide}");
        assert!(super::svg::draw("no such skin", &g.tightest).is_none());
    }
}
