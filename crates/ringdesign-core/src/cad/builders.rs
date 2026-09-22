//! Parts made from a stone by name — the stone itself, claw heads, bezels, baskets, seat burs and
//! halos — each built in the stone's own frame (girdle plane at `z = 0`, table up, `x` along the
//! stone's length) from the solids [`crate::setting`] makes for made seats, then seated by the
//! stone's placement. A builder's value is a mesh: every face names the patch it lies on ("Claw 3",
//! "Bearing"), its creases are its edges, and it meets the band through `csg` like any part.
use super::{Attach, Component, ComponentRole, Feature, Operation, Placement, Stage, SurfaceKind};
use crate::csg::{Op, P3, Solid};
use crate::gem::{Gem, GemCut, GemForm};
use crate::setting::{self, Fit, Named, Rails};
use crate::sketch::Id;
use anyhow::{Result, bail, ensure};
use serde::Serialize;
use serde_json::{Value as Json, json};
use std::collections::{HashMap, HashSet};

/// The stone itself: a reference part, never metal.
pub const STONE: &str = "stone";
/// The claw head the made seats carry.
pub const CLAW: &str = "head.claw";
/// A collet round the girdle.
pub const BEZEL: &str = "head.bezel";
/// Claws tied by gallery rails up to the girdle.
pub const BASKET: &str = "head.basket";
/// What the setter cuts under the stone.
pub const BUR: &str = "seat.bur";
/// A ring of melee round the stone.
pub const HALO: &str = "halo";

/// Dihedral at and above which a builder's edge is a crease, degrees.
pub const CREASE_DEG: f64 = 30.0;
/// How far a head's claws reach past the metal under their feet, mm.
pub const FOOT_SINK_MM: f64 = setting::FOOT_SINK_MM;
/// How far a bezel's base sinks into the metal when the setting seats it, mm.
pub const BEZEL_SINK_MM: f64 = 0.25;
/// How far a claw or basket setting holds the culet clear of the metal, mm.
pub const CULET_CLEAR_MM: f64 = 0.2;

/// One builder: its key, what it is called, and how its part meets the band when it is added.
#[derive(Clone, Copy, Debug)]
pub struct Spec {
    pub key: &'static str,
    pub label: &'static str,
    pub role: ComponentRole,
    pub attach: Attach,
    /// A stone, never metal.
    pub reference: bool,
    /// Stands on a stone feature and reads its gem and frame from it.
    pub on_stone: bool,
    pub hint: &'static str,
}

pub const SPECS: &[Spec] = &[
    Spec { key: STONE, label: "Stone", role: ComponentRole::Stone, attach: Attach::Separate, reference: true, on_stone: false, hint: "The stone itself as the viewport draws it: a reference part the settings are built round, never metal" },
    Spec { key: CLAW, label: "Claw head", role: ComponentRole::Head, attach: Attach::Join, reference: false, on_stone: true, hint: "Claws leaning out from a base rail, bent over the crown and notched by the stone itself: the made seats' own head" },
    Spec { key: BEZEL, label: "Bezel", role: ComponentRole::Setting, attach: Attach::Join, reference: false, on_stone: true, hint: "A collet round the girdle: tapered wall, a bearing ledge at the pavilion's angle, a lip leaning up the crown" },
    Spec { key: BASKET, label: "Basket", role: ComponentRole::Head, attach: Attach::Join, reference: false, on_stone: true, hint: "The claw head's claws tied by gallery rails from the base up to just under the girdle, so the pavilion sits in a cage" },
    Spec { key: BUR, label: "Seat bur", role: ComponentRole::Setting, attach: Attach::Cut, reference: false, on_stone: true, hint: "What the setter cuts under the stone: the whole bur where the girdle sits in the metal, else room for the pavilion and a pilot" },
    Spec { key: HALO, label: "Halo", role: ComponentRole::Setting, attach: Attach::Join, reference: false, on_stone: true, hint: "Melee in small collets or claw heads round the stone at equal arc length, tied by a rail under them" },
];

/// The builder called `key`.
pub fn spec(key: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.key == key)
}

/// What a builder is called; "Builder" for a key no builder answers to.
pub fn label(key: &str) -> &'static str {
    spec(key).map_or("Builder", |s| s.label)
}

/// A part's component as its builder adds it.
pub fn component(key: &str) -> Component {
    let s = spec(key);
    Component {
        role: s.map_or(ComponentRole::Other, |s| s.role),
        attach: s.map_or(Attach::Separate, |s| s.attach),
        reference: s.is_some_and(|s| s.reference),
        ..Component::default()
    }
}

/// What kind of value a builder parameter takes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub enum Kind {
    /// A length or a share, in the parameter's unit.
    Number,
    /// A whole count.
    Whole,
    /// One of these names.
    Choice(&'static [&'static str]),
    /// On or off.
    Flag,
}

/// One parameter of a builder, as an inspector draws it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Param {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: &'static str,
    pub min: f64,
    pub max: f64,
    pub kind: Kind,
    /// What it takes when the params leave it out, read off the stone.
    pub default: Json,
}

/// Every cut by its serde name, in `GemCut::ALL` order.
pub const CUTS: &[&str] = &["Round", "Oval", "Cushion", "Princess", "Emerald", "Baguette", "Pear", "Marquise", "Trillion", "Heart", "Radiant", "Asscher", "Hexagon", "HalfMoon"];
const FORMS: &[&str] = &["Faceted", "Cabochon"];
const STYLES: &[&str] = &["Bezel", "Claw"];

fn number(key: &'static str, label: &'static str, unit: &'static str, min: f64, max: f64, default: f64) -> Param {
    Param { key, label, unit, min, max, kind: Kind::Number, default: json!(default) }
}
fn whole(key: &'static str, label: &'static str, min: f64, max: f64, default: u32) -> Param {
    Param { key, label, unit: "", min, max, kind: Kind::Whole, default: json!(default) }
}

/// The halo's melee for a centre stone: a fifth of its width, 1 to 2 mm.
pub fn melee_mm(gem: Gem) -> f64 {
    (0.2 * gem.w_mm).clamp(1.0, 2.0)
}

/// A builder's parameters with the defaults `gem` gives them; empty for a key no builder answers to.
pub fn schema(key: &str, gem: Gem) -> Vec<Param> {
    let prongs = whole("prongs", "Claws", 3.0, 8.0, setting::claw_count(gem, 0));
    let wire = number("wire_mm", "Wire", "mm", 0.4, 2.0, setting::prong_wire_mm(gem));
    match key {
        STONE => vec![
            Param { key: "cut", label: "Cut", unit: "", min: 0.0, max: 0.0, kind: Kind::Choice(CUTS), default: json!(gem.cut) },
            number("w_mm", "Width", "mm", 0.8, 20.0, gem.w_mm),
            number("l_mm", "Length", "mm", 0.8, 30.0, gem.l_mm),
            Param { key: "form", label: "Make", unit: "", min: 0.0, max: 0.0, kind: Kind::Choice(FORMS), default: json!(gem.form) },
        ],
        CLAW => vec![prongs, wire],
        BASKET => vec![prongs, wire, whole("rails", "Rails", 1.0, 6.0, 3)],
        BEZEL => vec![
            number("wall_mm", "Wall", "mm", 0.25, 1.5, setting::collet_wall_mm(gem)),
            number("lip", "Lip", "of crown", 0.1, 0.8, setting::collet_lip(gem)),
        ],
        BUR => vec![Param { key: "through", label: "Pilot through", unit: "", min: 0.0, max: 1.0, kind: Kind::Flag, default: json!(false) }],
        HALO => vec![
            number("melee_mm", "Melee", "mm", 0.8, 3.0, melee_mm(gem)),
            number("gap_mm", "Gap", "mm", 0.0, 2.0, 0.3),
            number("bridge_mm", "Bridge", "mm", 0.05, 1.5, 0.25),
            whole("count", "Melee count", 0.0, 60.0, 0),
            number("drop_mm", "Drop", "mm", -1.0, 3.0, 0.0),
            Param { key: "style", label: "Melee setting", unit: "", min: 0.0, max: 0.0, kind: Kind::Choice(STYLES), default: json!("Bezel") },
        ],
        _ => Vec::new(),
    }
}

/// Every parameter of a builder at its default for `gem`, as one object.
pub fn defaults(key: &str, gem: Gem) -> Json {
    Json::Object(schema(key, gem).into_iter().map(|p| (p.key.to_string(), p.default)).collect())
}

/// The value `params` gives parameter `p`, else its default; refused when it is not of its kind or out of range.
fn read(params: &Json, p: &Param, who: &str) -> Result<Json> {
    let Some(v) = params.get(p.key).filter(|v| !v.is_null()) else { return Ok(p.default.clone()) };
    match p.kind {
        Kind::Number | Kind::Whole => {
            let x = v.as_f64().filter(|x| x.is_finite()).ok_or_else(|| anyhow::anyhow!("{who}: {} must be a number", p.label))?;
            ensure!(x >= p.min && x <= p.max, "{who}: {} must be between {} and {}{}", p.label, p.min, p.max, if p.unit.is_empty() { String::new() } else { format!(" {}", p.unit) });
            ensure!(p.kind != Kind::Whole || x.fract() == 0.0, "{who}: {} must be a whole number", p.label);
            Ok(json!(x))
        }
        Kind::Choice(names) => {
            let s = v.as_str().ok_or_else(|| anyhow::anyhow!("{who}: {} must be one of {}", p.label, names.join(", ")))?;
            ensure!(names.contains(&s), "{who}: {} must be one of {}", p.label, names.join(", "));
            Ok(v.clone())
        }
        Kind::Flag => Ok(json!(v.as_bool().ok_or_else(|| anyhow::anyhow!("{who}: {} must be true or false", p.label))?)),
    }
}

/// A builder's parameters resolved: every schema key with its value, read and checked.
struct Values(HashMap<&'static str, Json>);
impl Values {
    fn of(key: &str, gem: Gem, params: &Json) -> Result<Self> {
        ensure!(params.is_null() || params.is_object(), "{}: parameters must be an object", label(key));
        schema(key, gem).iter().map(|p| Ok((p.key, read(params, p, label(key))?))).collect::<Result<HashMap<_, _>>>().map(Self)
    }
    fn f(&self, key: &str) -> f64 {
        self.0.get(key).and_then(Json::as_f64).unwrap_or(0.0)
    }
    fn n(&self, key: &str) -> u32 {
        self.f(key).max(0.0) as u32
    }
    fn s(&self, key: &str) -> &str {
        self.0.get(key).and_then(Json::as_str).unwrap_or("")
    }
    fn b(&self, key: &str) -> bool {
        self.0.get(key).and_then(Json::as_bool).unwrap_or(false)
    }
}

/// Whether `params` read for builder `key`: every value of its kind and in its range, else what is wrong.
pub fn check_params(key: &str, gem: Gem, params: &Json) -> std::result::Result<(), String> {
    Values::of(key, gem, params).map(|_| ()).map_err(|e| format!("{e:#}"))
}

/// The stone a stone builder's parameters describe; a 6.5 mm round brilliant where they are silent.
pub fn gem_of(params: &Json) -> Result<Gem> {
    let base = Gem::calibrated(GemCut::Round, 6.5);
    let cut: GemCut = match params.get("cut").filter(|v| !v.is_null()) {
        Some(v) => serde_json::from_value(v.clone()).map_err(|_| anyhow::anyhow!("Stone: Cut must be one of {}", CUTS.join(", ")))?,
        None => base.cut,
    };
    let form: GemForm = match params.get("form").filter(|v| !v.is_null()) {
        Some(v) => serde_json::from_value(v.clone()).map_err(|_| anyhow::anyhow!("Stone: Make must be one of {}", FORMS.join(", ")))?,
        None => GemForm::Faceted,
    };
    let w = params.get("w_mm").and_then(Json::as_f64).unwrap_or(base.w_mm);
    let shaped = match form {
        GemForm::Faceted => Gem::calibrated(cut, w),
        GemForm::Cabochon => Gem::cabochon(cut, w),
    };
    let gem = Gem { l_mm: params.get("l_mm").and_then(Json::as_f64).unwrap_or(shaped.l_mm), ..shaped };
    let v = Values::of(STONE, gem, params)?;
    Ok(Gem { w_mm: v.f("w_mm"), l_mm: v.f("l_mm"), ..gem })
}

/// A stone builder's parameters for `gem`.
pub fn stone_params(gem: Gem) -> Json {
    json!({ "cut": gem.cut, "w_mm": gem.w_mm, "l_mm": gem.l_mm, "form": gem.form })
}

/// Where a stone meets the metal under it, in the stone's own frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Seat {
    /// Height of the metal surface under the girdle's centre, mm: negative where the stone stands proud of it.
    pub surface_z: f64,
    /// From the girdle down to open air past the bore, when a pilot can run there.
    pub through_mm: Option<f64>,
}

/// What a builder made: its named solid, the kind of surface each patch reads as, its creases, the stone
/// it was made for and, for a stone, where it meets the metal. In the stone's own frame until placed.
#[derive(Clone, Debug)]
pub struct Made {
    pub key: String,
    pub named: Named,
    /// The surface each patch reads as, by patch index.
    pub kinds: Vec<SurfaceKind>,
    /// Polylines along every edge whose faces turn at least [`CREASE_DEG`].
    pub creases: Vec<Vec<P3>>,
    pub gem: Option<Gem>,
    pub seat: Option<Seat>,
    /// A halo's melee: where each girdle's centre stands.
    pub stations: Vec<P3>,
}

impl Made {
    pub fn solid(&self) -> &Solid {
        &self.named.solid
    }

    /// Patches whose name starts with `prefix` that own at least one face.
    pub fn count(&self, prefix: &str) -> usize {
        let census = self.named.census();
        self.named.names.iter().zip(census).filter(|(n, c)| n.starts_with(prefix) && *c > 0).count()
    }

    /// Carats of the stone the part was made for.
    pub fn carats(&self) -> f64 {
        self.gem.map_or(0.0, |g| g.carats())
    }

    /// The part moved rigidly by `frame`.
    pub fn placed(&self, frame: &cadkernel::brep::Placement) -> Made {
        let f = frame_of(frame);
        Made {
            named: self.named.placed(&f),
            creases: self.creases.iter().map(|l| l.iter().map(|p| f.point(*p)).collect()).collect(),
            stations: self.stations.iter().map(|p| f.point(*p)).collect(),
            ..self.clone()
        }
    }

    /// Distinct ends of the crease polylines, for snapping.
    pub fn corners(&self) -> Vec<P3> {
        let mut out: Vec<P3> = Vec::new();
        for l in &self.creases {
            for p in [l.first(), l.last()].into_iter().flatten() {
                if !out.iter().any(|q| (0..3).all(|k| (q[k] - p[k]).abs() < 1e-9)) {
                    out.push(*p);
                }
            }
        }
        out
    }

    /// A closed named solid as what `key` made, its patches' kinds and creases read off it.
    pub fn of(key: &str, named: Named, gem: Option<Gem>) -> Result<Made> {
        let (open, repeated) = named.solid.open_edges();
        ensure!(open == 0 && repeated == 0 && !named.solid.is_empty(), "{} did not close ({open} open edges, {repeated} repeated)", label(key));
        let kinds = named.names.iter().map(|n| kind_of(n)).collect();
        let creases = creases(&named.solid, CREASE_DEG);
        Ok(Made { key: key.to_string(), named, kinds, creases, gem, seat: None, stations: Vec::new() })
    }
}

/// A kernel placement as a `csg` frame.
pub fn frame_of(p: &cadkernel::brep::Placement) -> crate::csg::Frame {
    crate::csg::Frame { origin: p.origin, x: p.x_axis, y: p.y_axis, z: p.z_axis }
}

/// The surface a patch reads as, by its name.
fn kind_of(name: &str) -> SurfaceKind {
    match name {
        "Table" | "Base" | "Back" | "Bed" => SurfaceKind::Plane,
        "Girdle" | "Girdle wall" | "Girdle seat" | "Wall" | "Inner wall" | "Pilot" | "Clearance" => SurfaceKind::Cylinder,
        "Pavilion" | "Crown" | "Bearing" | "Bevel" | "Lip" | "Relief" | "Rim" => SurfaceKind::Cone,
        "Dome" => SurfaceKind::Sphere,
        n if n.ends_with("rail") || n.starts_with("Gallery rail") => SurfaceKind::Torus,
        _ => SurfaceKind::Freeform,
    }
}

/// Polylines along every edge of a closed solid whose two faces turn at least `min_deg`; none past 20 000 such edges.
pub fn creases(solid: &Solid, min_deg: f64) -> Vec<Vec<P3>> {
    let unit = |f: &[u32; 3]| {
        let [a, b, c] = f.map(|i| solid.v[i as usize]);
        let (e, g) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
        let n = [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if l > 1e-300 { n.map(|v| v / l) } else { [0.0; 3] }
    };
    let normals: Vec<P3> = solid.f.iter().map(unit).collect();
    let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (i, f) in solid.f.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (f[k], f[(k + 1) % 3]);
            by_edge.entry((a.min(b), a.max(b))).or_default().push(i);
        }
    }
    let cos = min_deg.to_radians().cos();
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let mut edges: Vec<(u32, u32)> = by_edge
        .iter()
        .filter(|(_, fs)| fs.len() == 2 && dot(normals[fs[0]], normals[fs[1]]) < cos)
        .map(|(e, _)| *e)
        .collect();
    if edges.len() > 20_000 {
        return Vec::new();
    }
    edges.sort_unstable();
    let mut adj: HashMap<u32, Vec<u32>> = HashMap::new();
    for (a, b) in &edges {
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }
    for n in adj.values_mut() {
        n.sort_unstable();
    }
    let mut ends: Vec<u32> = adj.iter().filter(|(_, n)| n.len() != 2).map(|(v, _)| *v).collect();
    let mut all: Vec<u32> = adj.keys().copied().collect();
    ends.sort_unstable();
    all.sort_unstable();
    let key = |a: u32, b: u32| (a.min(b), a.max(b));
    let mut used: HashSet<(u32, u32)> = HashSet::new();
    let mut lines = Vec::new();
    for start in ends.into_iter().chain(all) {
        for first in adj[&start].clone() {
            if !used.insert(key(start, first)) {
                continue;
            }
            let mut line = vec![start, first];
            let mut at = first;
            while at != start && adj[&at].len() == 2 {
                let Some(&next) = adj[&at].iter().find(|&&t| !used.contains(&key(at, t))) else { break };
                used.insert(key(at, next));
                line.push(next);
                at = next;
            }
            lines.push(line.into_iter().map(|v| solid.v[v as usize]).collect());
        }
    }
    lines.truncate(512);
    lines
}

/// Where the girdle stands over the metal for the setting preset or builder `key`: claws, baskets and halos
/// hold the culet [`CULET_CLEAR_MM`] clear, a bezel sinks its base [`BEZEL_SINK_MM`] into the metal.
pub fn stand_off_mm(key: &str, gem: Gem) -> f64 {
    match key {
        BEZEL | "bezel" => setting::collet_depth_mm(gem) - BEZEL_SINK_MM,
        _ => gem.pavilion_mm() + CULET_CLEAR_MM,
    }
}

/// Where a flat-bottomed bezel meets the metal under its wall, read round its plan: wholly over metal, its lowest
/// point, so the whole base sinks and the seam closes round it; overhanging the band, its highest, where it sits.
fn under_wall(gem: Gem, wall: f64, floor: setting::Floor) -> Option<f64> {
    let plan = setting::Plan::of(gem);
    let metal: Vec<Option<f64>> = (0..32)
        .map(|k| {
            let phi = std::f64::consts::TAU * k as f64 / 32.0;
            let (p, n) = (plan.point(phi), plan.normal(phi));
            floor([p[0] + n[0] * 0.8 * wall, p[1] + n[1] * 0.8 * wall])
        })
        .collect();
    let found = metal.iter().flatten().copied();
    if metal.iter().all(Option::is_some) { found.reduce(f64::min) } else { found.reduce(f64::max) }
}

/// What builder `key` makes for `gem` with `params`, in the stone's own frame: `seat` says where the metal is under
/// the girdle's centre, and `floor`, when there is a surface, where it is under any point of the girdle plane, so
/// claws and a bezel's wall reach it wherever they stand.
pub fn build(key: &str, gem: Gem, params: &Json, seat: Seat, floor: Option<setting::Floor>) -> Result<Made> {
    let who = label(key);
    ensure!(spec(key).is_some(), "No builder called {key}; choose {}", SPECS.iter().map(|s| s.key).collect::<Vec<_>>().join(", "));
    let v = Values::of(key, gem, params)?;
    let snag = |e: crate::csg::Snag| anyhow::anyhow!("{who} would not resolve: {e}");
    let named = match key {
        STONE => setting::envelope_named(gem, 0.0),
        CLAW => setting::claw_head_named(gem, v.n("prongs"), v.f("wire_mm"), Rails::Seat, floor).map_err(snag)?,
        BASKET => setting::claw_head_named(gem, v.n("prongs"), v.f("wire_mm"), Rails::Basket(v.n("rails")), floor).map_err(snag)?,
        BEZEL => {
            let metal = floor.and_then(|f| under_wall(gem, v.f("wall_mm"), f)).unwrap_or(seat.surface_z).min(seat.surface_z);
            setting::collet_named(gem, v.f("wall_mm"), v.f("lip"), metal - BEZEL_SINK_MM)
        }
        BUR => {
            let fit = Fit { surface_z: seat.surface_z, through_mm: seat.through_mm.filter(|_| v.b("through")), prongs: 0 };
            // A girdle in the metal takes the whole bur; over it, a bur that deep would cut the claws holding the stone.
            if seat.surface_z >= -setting::girdle_half_mm(gem) {
                setting::bur_named(gem, &fit)
            } else {
                match setting::relief_named(gem, fit.surface_z.min(-0.1) + 0.3, (0.18 * gem.w_mm).max(0.3), &fit) {
                    Some(n) => n,
                    None => bail!("{who}: a cabochon standing proud of the metal sits on its head and needs no seat cut"),
                }
            }
        }
        HALO => return halo(gem, &v),
        _ => unreachable!("checked above"),
    };
    let mut made = Made::of(key, named, Some(gem))?;
    if key == STONE {
        made.seat = Some(seat);
    }
    Ok(made)
}

/// The halo outline sampled so stations land at equal arc length: `(points, cumulative arc)`.
fn halo_ring(a: f64, b: f64) -> (Vec<[f64; 2]>, Vec<f64>) {
    const STEPS: usize = 256;
    let pts: Vec<[f64; 2]> = (0..=STEPS).map(|i| {
        let t = i as f64 / STEPS as f64 * std::f64::consts::TAU;
        [a * t.cos(), b * t.sin()]
    }).collect();
    let mut arc = vec![0.0];
    for w in pts.windows(2) {
        arc.push(arc.last().unwrap_or(&0.0) + (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]));
    }
    (pts, arc)
}

/// The point `frac` of the way round a sampled outline by arc length.
fn at_arc(pts: &[[f64; 2]], arc: &[f64], frac: f64) -> [f64; 2] {
    let total = *arc.last().unwrap_or(&0.0);
    let target = frac.rem_euclid(1.0) * total;
    let i = arc.partition_point(|&s| s < target).clamp(1, pts.len() - 1);
    let (a0, a1) = (arc[i - 1], arc[i]);
    let t = if a1 > a0 { (target - a0) / (a1 - a0) } else { 0.0 };
    [pts[i - 1][0] + (pts[i][0] - pts[i - 1][0]) * t, pts[i - 1][1] + (pts[i][1] - pts[i - 1][1]) * t]
}

/// Melee in small collets (or claw heads) round the stone's own outline grown by the gap, at equal arc length,
/// counted as `pave::halo` counts accents — the perimeter over the melee's footprint and bridge, at least six —
/// and tied by a rail under their bases.
fn halo(gem: Gem, v: &Values) -> Result<Made> {
    let melee = Gem::calibrated(GemCut::Round, v.f("melee_mm"));
    let footprint = melee.w_mm + 0.7;
    let grow = v.f("gap_mm") + footprint * 0.5;
    let (a, b) = (gem.l_mm * 0.5 + grow, gem.w_mm * 0.5 + grow);
    let (pts, arc) = halo_ring(a, b);
    let perimeter = *arc.last().unwrap_or(&0.0);
    let pitch = footprint + v.f("bridge_mm");
    let n = if v.n("count") >= 3 { v.n("count") } else { ((perimeter / pitch).floor() as u32).max(6) };
    let z = -v.f("drop_mm");
    let claws = v.s("style") == "Claw";
    let unit = if claws {
        setting::claw_head_named(melee, 4, setting::prong_wire_mm(melee), Rails::Seat, None).map_err(|e| anyhow::anyhow!("Halo: a melee head would not resolve: {e}"))?
    } else {
        setting::collet_named(melee, setting::collet_wall_mm(melee), setting::collet_lip(melee), -setting::collet_depth_mm(melee))
    };
    let (lo, hi) = unit.solid.bounds().unwrap_or(([0.0; 3], [0.0; 3]));
    let reach = hi[0].max(hi[1]).max(-lo[0]).max(-lo[1]);
    let mut parts = Vec::with_capacity(n as usize + 1);
    let mut stations = Vec::with_capacity(n as usize);
    for k in 0..n {
        let c = at_arc(&pts, &arc, k as f64 / n as f64);
        let station = [c[0], c[1], z];
        let frame = crate::csg::Frame { origin: station, ..crate::csg::Frame::IDENTITY };
        let placed = unit.solid.placed(&frame);
        stations.push(station);
        parts.push(Named::whole(placed, format!("{} {}", if claws { "Melee head" } else { "Collet" }, k + 1)));
    }
    // The rail runs round the outside of the melee's bases, clear of their pavilions, so the ring is one piece.
    let r = (0.3 * melee.w_mm).clamp(0.2, 0.45);
    let plan = setting::Plan { a, b, pow: 2.0 };
    let (offset, rail_z) = (reach - 0.5 * r, lo[2] + z + 0.6 * r);
    let section: Vec<setting::Station> = (0..16).map(|k| {
        let t = std::f64::consts::TAU * (k as f64 + 0.31) / 16.0;
        setting::Station { s: 1.0, o: offset + r * t.sin(), z: rail_z + r * t.cos() }
    }).collect();
    parts.push(Named::whole(setting::sweep(&plan, &section, 128), "Halo rail"));
    let named = Named::union_all(parts).map_err(|e| anyhow::anyhow!("Halo would not resolve: {e}"))?;
    let mut made = Made::of(HALO, named, Some(gem))?;
    made.stations = stations;
    Ok(made)
}

/// A stone the right-click menu offers.
#[derive(Clone, Copy, Debug)]
pub struct StonePreset {
    pub key: &'static str,
    pub label: &'static str,
    pub cut: GemCut,
    pub w_mm: f64,
    pub l_mm: f64,
}

impl StonePreset {
    pub fn gem(&self) -> Gem {
        Gem { l_mm: self.l_mm, ..Gem::calibrated(self.cut, self.w_mm) }
    }
}

pub const STONES: &[StonePreset] = &[
    StonePreset { key: "round-5", label: "Round 5 mm", cut: GemCut::Round, w_mm: 5.0, l_mm: 5.0 },
    StonePreset { key: "round-6.5", label: "Round 6.5 mm", cut: GemCut::Round, w_mm: 6.5, l_mm: 6.5 },
    StonePreset { key: "oval-7x5", label: "Oval 7 × 5", cut: GemCut::Oval, w_mm: 5.0, l_mm: 7.0 },
    StonePreset { key: "princess-5", label: "Princess 5 mm", cut: GemCut::Princess, w_mm: 5.0, l_mm: 5.0 },
    StonePreset { key: "cushion-6", label: "Cushion 6 mm", cut: GemCut::Cushion, w_mm: 6.0, l_mm: 6.0 },
    StonePreset { key: "emerald-7x5", label: "Emerald 7 × 5", cut: GemCut::Emerald, w_mm: 5.0, l_mm: 7.0 },
    StonePreset { key: "pear-7x5", label: "Pear 7 × 5", cut: GemCut::Pear, w_mm: 5.0, l_mm: 7.0 },
    StonePreset { key: "marquise-8x4", label: "Marquise 8 × 4", cut: GemCut::Marquise, w_mm: 4.0, l_mm: 8.0 },
];

/// The stone preset called `key`.
pub fn stone_preset(key: &str) -> Option<&'static StonePreset> {
    STONES.iter().find(|s| s.key == key)
}

/// A made setting the right-click menu offers round a stone.
#[derive(Clone, Copy, Debug)]
pub struct SettingPreset {
    pub key: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
}

pub const SETTINGS: &[SettingPreset] = &[
    SettingPreset { key: "claw4", label: "Four claws", hint: "A four-claw head joined to the band, and the seat bur under it" },
    SettingPreset { key: "claw6", label: "Six claws", hint: "A six-claw head joined to the band, and the seat bur under it" },
    SettingPreset { key: "bezel", label: "Bezel", hint: "A collet joined to the band round the girdle, and the seat bur under it" },
    SettingPreset { key: "basket", label: "Basket", hint: "Claws tied by gallery rails up to the girdle, joined to the band, and the seat bur under it" },
    SettingPreset { key: "halo", label: "Halo", hint: "A four-claw head, a ring of melee round it, and the seat bur under it" },
];

/// The setting preset called `key`.
pub fn setting_preset(key: &str) -> Option<&'static SettingPreset> {
    SETTINGS.iter().find(|s| s.key == key)
}

/// A stone's name in millimetres: "Round 6.5 mm", "Oval 7 × 5", "Oval cabochon 7 × 5".
pub fn gem_label(gem: Gem) -> String {
    let mm = |v: f64| {
        let s = format!("{v:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    let cut = if gem.cut == GemCut::Round { "Round" } else { gem.cut.label() };
    let make = if gem.form == GemForm::Cabochon { " cabochon" } else { "" };
    if (gem.l_mm - gem.w_mm).abs() < 0.05 {
        format!("{cut}{make} {} mm", mm(gem.w_mm))
    } else {
        format!("{cut}{make} {} × {}", mm(gem.l_mm), mm(gem.w_mm))
    }
}

/// A reference stone feature for `gem` standing at `placement`.
pub fn stone_feature(id: Id, gem: Gem, placement: Placement) -> Feature {
    let name = gem_label(gem);
    Feature {
        id,
        name: name.clone(),
        enabled: true,
        operation: Operation::Builder { key: STONE.into(), on: None, params: stone_params(gem) },
        component: Component { placement, stone_id: Some(name), ..component(STONE) },
    }
}

/// A builder feature standing on stone `on`.
pub fn feature_on(id: Id, name: &str, key: &str, on: Id, params: Json) -> Feature {
    Feature { id, name: name.into(), enabled: true, operation: Operation::Builder { key: key.into(), on: Some(on), params }, component: component(key) }
}

/// What setting preset `key` adds round stone `stone`: its head or bezel (and a halo) joined to the band, then the
/// seat bur cut from it — staged Bench under sand, because a seat is cut after the pour. Each takes an id from `next`.
pub fn setting_features(key: &str, stone: Id, gem: Gem, sand: bool, next: &mut dyn FnMut() -> Id) -> Result<Vec<Feature>> {
    let mut out = Vec::new();
    let (head_key, name, params) = match key {
        "claw4" => (CLAW, "Four-claw head", json!({ "prongs": 4 })),
        "claw6" => (CLAW, "Six-claw head", json!({ "prongs": 6 })),
        "bezel" => (BEZEL, "Bezel", json!({})),
        "basket" => (BASKET, "Basket", json!({})),
        "halo" => (CLAW, "Four-claw head", json!({ "prongs": 4 })),
        _ => bail!("No setting called {key}; choose {}", SETTINGS.iter().map(|s| s.key).collect::<Vec<_>>().join(", ")),
    };
    out.push(feature_on(next(), name, head_key, stone, params));
    if key == "halo" {
        out.push(feature_on(next(), "Halo", HALO, stone, json!({})));
    }
    if !(gem.form == GemForm::Cabochon && key != "bezel") {
        let mut bur = feature_on(next(), "Seat bur", BUR, stone, json!({ "through": key != "bezel" }));
        bur.component.stage = if sand { Stage::Bench } else { Stage::Cast };
        out.push(bur);
    }
    Ok(out)
}

/// Whether a design is judged for a two-part sand mould, where a seat is cut after the pour.
pub fn sand(design: &crate::RingDesign) -> bool {
    design.draft.process == crate::castability::CastProcess::SandTwoPart
}

/// The boolean a stone's value takes with another's, each face keeping its patch.
pub fn combined(a: &Named, b: &Named, op: Op) -> Result<Named> {
    a.clone().combine(b, op).map_err(|e| anyhow::anyhow!("the meshes would not resolve: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{self, BuildCtx, Document, EdgeRef, FaceRef, FeatureStatus, edit::CadEdit};
    use crate::{AlphaLibrary, BuildParams, RingDesign, csg};
    use std::sync::atomic::AtomicBool;

    fn params() -> BuildParams {
        BuildParams { theta_steps: 256, profile_steps: 128, ..BuildParams::default() }
    }
    fn court() -> RingDesign {
        crate::templates::all().iter().find(|t| t.name == "Court band").unwrap().design()
    }
    /// The Court band carrying the claw solitaire's document: band, stone #2, head #3, bur #4.
    fn solitaire() -> RingDesign {
        RingDesign { cad: cad::examples::design("claw-solitaire").unwrap().cad, ..court() }
    }
    /// The Court band with a 6.5 mm round seated for setting preset `key`, and that setting round it.
    fn set(key: &str) -> RingDesign {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let mut doc = Document::default();
        doc.append(Feature { id: 1, name: "Procedural shank".into(), enabled: true, operation: Operation::Band, component: Component::default() }).unwrap();
        doc.append(stone_feature(2, gem, Placement::ring(90.0, stand_off_mm(key, gem)))).unwrap();
        let mut next = 2;
        for f in setting_features(key, 2, gem, true, &mut || { next += 1; next }).unwrap() {
            doc.append(f).unwrap();
        }
        RingDesign { cad: Some(doc), ..court() }
    }
    fn as_solid(mesh: &crate::Mesh) -> Solid {
        Solid { v: mesh.vertices.iter().map(|p| [p.0 as f64, p.1 as f64, p.2 as f64]).collect(), f: mesh.faces.clone() }
    }
    /// The furthest any vertex of `made` stands from the girdle centre of stone #2 of `d`.
    fn reach(made: &Made, d: &RingDesign, surface: &crate::Mesh) -> f64 {
        let stone = d.cad.as_ref().unwrap().feature(2).unwrap();
        let o = stone.component.placement.frame_on(d, Some(surface)).unwrap().origin;
        made.solid().v.iter().map(|p| (0..3).map(|k| (p[k] - o[k]).powi(2)).sum::<f64>().sqrt()).fold(0.0, f64::max)
    }
    fn centroid(s: &Solid) -> P3 {
        let n = s.v.len().max(1) as f64;
        std::array::from_fn(|k| s.v.iter().map(|p| p[k]).sum::<f64>() / n)
    }
    /// Faces joined by shared edges into separate pieces: one for a part that holds together.
    fn pieces(s: &Solid) -> usize {
        let mut root: Vec<usize> = (0..s.v.len()).collect();
        fn find(root: &mut [usize], mut i: usize) -> usize {
            while root[i] != i {
                root[i] = root[root[i]];
                i = root[i];
            }
            i
        }
        for f in &s.f {
            for k in 1..3 {
                let (a, b) = (find(&mut root, f[0] as usize), find(&mut root, f[k] as usize));
                root[a.max(b)] = a.min(b);
            }
        }
        let used: HashSet<usize> = s.f.iter().flatten().map(|v| find(&mut root, *v as usize)).collect();
        used.len()
    }

    #[test]
    fn a_claw_solitaire_builds_watertight_with_its_head_joined_its_seat_cut_and_its_stone_clear() {
        let lib = AlphaLibrary::builtin();
        let d = solitaire();
        let bare = crate::mesh::try_build(&court(), &lib, params()).unwrap();
        let full = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let v = &full.report.validation;
        assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{v:?}");
        assert!(full.parts.notes.is_empty(), "{:?}", full.parts.notes);
        assert_eq!((full.parts.joined, full.parts.cut, full.parts.separate, full.parts.references), (1, 1, 0, 1));
        let e = full.parts.evaluated.as_ref().unwrap();
        assert!(e.features.iter().all(|r| r.status.is_ok()), "{:?}", e.features);
        let part = |id| e.components.iter().find(|c| c.id == id).unwrap();
        let (stone, head, bur) = (part(2), part(3), part(4));
        // The stone is a reference part and a carat, gem.rs's anchor: 6.5 x 6.5 x 4.03 x 0.0061.
        let stone_made = stone.made.as_ref().unwrap();
        assert!(stone.settings.reference && stone.brep().is_none() && stone.body.faces.is_empty());
        assert!((stone_made.carats() - 1.0).abs() < 0.08 && (stone_made.carats() - 1.0386).abs() < 1e-3, "{}", stone_made.carats());
        assert_eq!(stone_made.named.names, ["Crown", "Girdle", "Table", "Pavilion"]);
        assert_eq!(stone_made.creases.len(), 3, "the table's edge and the girdle's two");
        // Four claws counted by patch, their notches their own, and the trace naming every face.
        let made = head.made.as_ref().unwrap();
        assert_eq!(made.count("Claw "), 4);
        assert_eq!(made.named.names, ["Claw 1", "Claw 2", "Claw 3", "Claw 4", "Base rail", "Gallery rail"]);
        assert_eq!(head.trace.patch(2), Some("Claw 3"));
        assert_eq!(head.trace.tri_face.len(), head.mesh.faces.len());
        assert!(head.trace.tri_face.iter().all(|p| (*p as usize) < head.trace.patches.len()));
        assert_eq!(head.edges.len(), made.creases.len());
        assert_eq!((head.attach, bur.attach, bur.stage), (Attach::Join, Attach::Cut, Stage::Bench));
        // Joined: the ring gains the head's own metal, less the feet it sinks into the band.
        let mut joined = d.clone();
        joined.cad.as_mut().unwrap().apply(&CadEdit::Enable { id: 4, enabled: false }).unwrap();
        let j = crate::mesh::try_build(&joined, &lib, params()).unwrap();
        let own = made.solid().volume();
        let overlap = csg::combine(made.solid(), &as_solid(&bare.mesh), Op::Intersect).unwrap().volume();
        let rise = j.report.volume_mm3 - bare.report.volume_mm3;
        assert!((rise - (own - overlap)).abs() < 1e-3 * own, "rise {rise:.3} against {own:.3} less {overlap:.3}");
        assert!((rise / own - 1.0).abs() < 0.02, "rise {rise:.3} against the head's {own:.3}");
        assert!(overlap > 0.1, "the claws reach the band: {overlap:.3}");
        // ...down the claws' own lines to the band beside the stone, never across the finger hole.
        let far = reach(made, &d, &bare.mesh);
        assert!(far < 6.5, "the head stands by its stone: {far:.2} mm out");
        // Cut: the pilot comes out of the band under the stone.
        let removed = j.report.volume_mm3 - full.report.volume_mm3;
        let pilot = csg::combine(bur.made.as_ref().unwrap().solid(), &as_solid(&j.mesh), Op::Intersect).unwrap().volume();
        assert!(removed > 1.0 && (removed - pilot).abs() < 1e-3 * removed, "removed {removed:.3} against {pilot:.3}");
        // The stone sits in its claws and never in the metal.
        let ring = as_solid(&full.mesh);
        let buried = stone_made.solid().v.iter().filter(|p| csg::inside(&ring, **p) == Some(true)).count();
        assert_eq!(buried, 0);
        // Every part vertex names its feature; the stone, never metal, names none.
        let named: HashSet<Id> = full.mesh.origin.iter().filter_map(|o| full.parts.feature_of(*o)).collect();
        assert_eq!(named, [3, 4].into_iter().collect());
        eprintln!("claw solitaire on the Court band: rise {rise:.3} of the head's {own:.3} mm³ ({:.2}%), overlap {overlap:.3}, seat cut {removed:.3} mm³, {} faces", 100.0 * (1.0 - rise / own), full.mesh.faces.len());
    }

    #[test]
    fn moving_the_stone_moves_its_head_and_bur_and_suppressing_it_skips_both_by_name() {
        let lib = AlphaLibrary::builtin();
        let d = solitaire();
        let surface = crate::mesh::try_build(&court(), &lib, params()).unwrap().mesh;
        let never = AtomicBool::new(false);
        let eval = |d: &RingDesign| cad::evaluate_with(d, &lib, params(), &BuildCtx::new(&never).with_surface(&surface)).unwrap();
        let solid_of = |e: &cad::Evaluated, id: Id| e.components.iter().find(|c| c.id == id).and_then(|c| c.made.clone()).unwrap();
        let before = eval(&d);
        let mut moved = d.clone();
        let height = stand_off_mm("claw4", Gem::calibrated(GemCut::Round, 6.5));
        moved.cad.as_mut().unwrap().apply(&CadEdit::Placement { id: 2, placement: Placement::ring(60.0, height) }).unwrap();
        let after = eval(&moved);
        let (s, c) = (-30f64).to_radians().sin_cos();
        for id in [2, 3, 4] {
            let (a, b) = (centroid(solid_of(&before, id).solid()), centroid(solid_of(&after, id).solid()));
            let turned = [a[0] * c - a[1] * s, a[0] * s + a[1] * c, a[2]];
            let off = (0..3).map(|k| (turned[k] - b[k]).powi(2)).sum::<f64>().sqrt();
            assert!(off < 0.05, "#{id} did not follow its stone 30° round the ring: {off:.4} mm off");
        }
        let mut off = d.clone();
        off.cad.as_mut().unwrap().apply(&CadEdit::Enable { id: 2, enabled: false }).unwrap();
        let e = eval(&off);
        for id in [3, 4] {
            assert_eq!(e.status_of(id), Some(&FeatureStatus::Skipped("source #2 Round 6.5 mm was suppressed".into())));
        }
        assert!(e.components.is_empty() && e.band == Some(1) && e.failures().is_empty());
    }

    #[test]
    fn six_claws_a_bezel_a_basket_and_a_halo_build_round_the_stone_and_join_the_band() {
        let lib = AlphaLibrary::builtin();
        for (key, joins, patches) in [
            ("claw6", 1, vec!["Claw 1", "Claw 2", "Claw 3", "Claw 4", "Claw 5", "Claw 6", "Base rail", "Gallery rail"]),
            ("bezel", 1, vec!["Rim", "Wall", "Base", "Inner wall", "Bearing", "Girdle seat", "Lip"]),
            ("basket", 1, vec!["Claw 1", "Claw 2", "Claw 3", "Claw 4", "Base rail", "Gallery rail 1", "Gallery rail 2"]),
            ("halo", 2, vec!["Claw 1", "Claw 2", "Claw 3", "Claw 4", "Base rail", "Gallery rail"]),
        ] {
            let d = set(key);
            let built = crate::mesh::try_build(&d, &lib, params()).unwrap_or_else(|e| panic!("{key}: {e:#}"));
            let v = &built.report.validation;
            assert!(v.watertight && v.boundary_edges == 0 && v.non_manifold_edges == 0, "{key}: {v:?}");
            assert!(built.parts.notes.is_empty(), "{key}: {:?}", built.parts.notes);
            assert_eq!((built.parts.joined, built.parts.cut, built.parts.references), (joins, 1, 1), "{key}");
            let e = built.parts.evaluated.as_ref().unwrap();
            let head = e.components.iter().find(|c| c.id == 3).and_then(|c| c.made.clone()).unwrap();
            assert_eq!(head.named.names, patches, "{key}");
            assert_eq!(pieces(head.solid()), 1, "{key}: the head holds together");
            let bare = crate::mesh::try_build(&court(), &lib, params()).unwrap();
            let overlap = csg::combine(head.solid(), &as_solid(&bare.mesh), Op::Intersect).unwrap().volume();
            assert!(overlap > 0.05, "{key}: reaches the band: {overlap:.3}");
            let far = reach(&head, &d, &bare.mesh);
            assert!(far < 6.5, "{key}: stands by its stone: {far:.2} mm out");
            eprintln!("{key}: {} faces, {:.3} mm³, into the band {overlap:.3} mm³, ring {} faces", head.named.solid.f.len(), head.solid().volume(), built.mesh.faces.len());
        }
    }

    #[test]
    fn a_halo_seats_its_melee_at_equal_arc_length_by_the_pave_rule() {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let made = build(HALO, gem, &json!({}), Seat::default(), None).unwrap();
        // 1.3 mm melee, a fifth of the centre; footprint 2.0; ring 3.25 + 0.3 + 1.0 = 4.55 round; pitch 2.25:
        // floor(2π 4.55 / 2.25) = floor(12.71) = 12.
        assert_eq!(made.stations.len(), 12);
        assert_eq!(made.count("Collet "), 12);
        assert!(made.named.names.iter().any(|n| n == "Halo rail"));
        assert_eq!(pieces(made.solid()), 1, "the rail ties the collets into one piece");
        let chord = 2.0 * 4.55 * (std::f64::consts::PI / 12.0).sin();
        for k in 0..12 {
            let (a, b) = (made.stations[k], made.stations[(k + 1) % 12]);
            let d = (a[0] - b[0]).hypot(a[1] - b[1]);
            assert!((d - chord).abs() < 1e-3, "station {k}: {d:.4} against {chord:.4}");
            assert!((a[0].hypot(a[1]) - 4.55).abs() < 1e-3);
        }
        // An oval centre gets an oval halo, its stations still equal steps round it.
        let oval = Gem { l_mm: 7.0, ..Gem::calibrated(GemCut::Oval, 5.0) };
        let made = build(HALO, oval, &json!({ "melee_mm": 1.2 }), Seat::default(), None).unwrap();
        let (pts, arc) = halo_ring(3.5 + 0.3 + 0.95, 2.5 + 0.3 + 0.95);
        assert_eq!(made.stations.len(), ((arc[arc.len() - 1] / (1.9 + 0.25)).floor() as usize).max(6));
        let n = made.stations.len();
        for (k, s) in made.stations.iter().enumerate() {
            let want = at_arc(&pts, &arc, k as f64 / n as f64);
            assert!((s[0] - want[0]).abs() < 1e-9 && (s[1] - want[1]).abs() < 1e-9);
        }
        // A given count wins over the rule.
        assert_eq!(build(HALO, gem, &json!({ "count": 16 }), Seat::default(), None).unwrap().stations.len(), 16);
    }

    #[test]
    fn a_kernel_operation_on_a_builders_mesh_is_refused_by_name_and_a_boolean_goes_through_csg() {
        let lib = AlphaLibrary::builtin();
        let surface = crate::mesh::try_build(&court(), &lib, params()).unwrap().mesh;
        let never = AtomicBool::new(false);
        let mut d = solitaire();
        let doc = d.cad.as_mut().unwrap();
        doc.append(Feature { id: 5, name: "Fillet".into(), enabled: true, operation: Operation::Fillet { source: 3, edges: vec![EdgeRef::bare(0)], radius_mm: 0.2 }, component: Component::default() }).unwrap();
        let mut sketch = crate::sketch::Sketch::circle(0.5);
        sketch.plane.on_face = Some(crate::sketch::FaceAnchor { feature: 3, face: FaceRef::bare(0) });
        doc.append(Feature { id: 6, name: "Sketch".into(), enabled: true, operation: Operation::Sketch { sketch }, component: Component::default() }).unwrap();
        let mut boss = Feature { id: 7, name: "Boss".into(), enabled: true, operation: Operation::Box { size: [2.0, 2.0, 2.0] }, component: Component::default() };
        boss.component.placement = Placement::ring(90.0, 1.0);
        doc.append(boss).unwrap();
        doc.append(Feature { id: 8, name: "Head and boss".into(), enabled: true, operation: Operation::Boolean { a: 3, b: 7, kind: cad::Boolean::Union }, component: Component::default() }).unwrap();
        let e = cad::evaluate_with(&d, &lib, params(), &BuildCtx::new(&never).with_surface(&surface)).unwrap();
        assert_eq!(e.status_of(5), Some(&FeatureStatus::Failed("Fillet works on kernel bodies, and #3 Four-claw head is a mesh a builder made".into())));
        let Some(FeatureStatus::Failed(why)) = e.status_of(6) else { panic!("{:?}", e.status_of(6)) };
        assert!(why.starts_with("Sketch face: feature #3 is a claw head a builder made, a mesh"), "{why}");
        // Head ∪ box through csg: one mesh, every face still named, the box's by its kernel face.
        assert_eq!(e.status_of(8), Some(&FeatureStatus::Ok));
        let union = e.components.iter().find(|c| c.id == 8).and_then(|c| c.made.clone()).unwrap();
        assert_eq!(union.solid().open_edges(), (0, 0));
        assert!(union.named.names.iter().any(|n| n == "Claw 1") && union.named.names.iter().any(|n| n.starts_with("Face ")));
        assert!(!e.components.iter().any(|c| c.id == 3 || c.id == 7), "the union consumed both");
        // A builder standing on something other than a stone, or on nothing, says so.
        let mut d = solitaire();
        d.cad.as_mut().unwrap().apply(&CadEdit::Operation { id: 4, operation: Operation::Builder { key: BUR.into(), on: Some(3), params: json!({}) } }).unwrap();
        let e = cad::evaluate(&d, &lib, params()).unwrap();
        assert_eq!(e.status_of(4), Some(&FeatureStatus::Failed("#3 Four-claw head is not a stone; a seat bur is built round a stone part".into())));
        d.cad.as_mut().unwrap().apply(&CadEdit::Operation { id: 4, operation: Operation::Builder { key: "head.crown".into(), on: Some(2), params: json!({}) } }).unwrap();
        let e = cad::evaluate(&d, &lib, params()).unwrap();
        assert!(matches!(e.status_of(4), Some(FeatureStatus::Failed(m)) if m.starts_with("No builder called head.crown")));
    }

    #[test]
    fn every_builder_names_its_parameters_with_units_ranges_and_defaults_read_off_the_gem() {
        let round = Gem::calibrated(GemCut::Round, 6.5);
        for spec in SPECS {
            let schema = schema(spec.key, round);
            assert!(!schema.is_empty(), "{}", spec.key);
            for p in &schema {
                assert!(!p.label.is_empty() && p.min <= p.max, "{} {}", spec.key, p.key);
                if let Some(x) = p.default.as_f64() {
                    assert!(x >= p.min && x <= p.max, "{} {}: {x} out of {}..{}", spec.key, p.key, p.min, p.max);
                }
            }
            assert_eq!(check_params(spec.key, round, &defaults(spec.key, round)), Ok(()));
            assert_eq!(component(spec.key).attach, spec.attach);
        }
        // Defaults come off the stone: the seats' own wire and claw count, the collet's own wall.
        assert_eq!(defaults(CLAW, round), json!({ "prongs": 4, "wire_mm": setting::prong_wire_mm(round) }));
        assert_eq!(defaults(CLAW, Gem::calibrated(GemCut::Marquise, 4.0))["prongs"], 6);
        assert_eq!(defaults(BEZEL, round)["wall_mm"], setting::collet_wall_mm(round));
        assert!((defaults(HALO, round)["melee_mm"].as_f64().unwrap() - 1.3).abs() < 1e-12);
        // Out of range, of the wrong kind, or not an object: refused by name before anything is built.
        let e = build(CLAW, round, &json!({ "wire_mm": 5.0 }), Seat::default(), None).unwrap_err().to_string();
        assert_eq!(e, "Claw head: Wire must be between 0.4 and 2 mm");
        let e = build(CLAW, round, &json!({ "prongs": 4.5 }), Seat::default(), None).unwrap_err().to_string();
        assert_eq!(e, "Claw head: Claws must be a whole number");
        let e = build(HALO, round, &json!({ "style": "Pavé" }), Seat::default(), None).unwrap_err().to_string();
        assert_eq!(e, "Halo: Melee setting must be one of Bezel, Claw");
        assert!(build(BEZEL, round, &json!([1, 2]), Seat::default(), None).is_err());
        // A stone reads its gem off its parameters, and names itself.
        let oval = gem_of(&json!({ "cut": "Oval", "w_mm": 5.0, "l_mm": 7.0 })).unwrap();
        assert_eq!((oval.cut, oval.w_mm, oval.l_mm, oval.form), (GemCut::Oval, 5.0, 7.0, GemForm::Faceted));
        assert_eq!(gem_label(oval), "Oval 7 × 5");
        assert_eq!(gem_label(round), "Round 6.5 mm");
        assert_eq!(gem_of(&Json::Null).unwrap(), round);
        assert!(gem_of(&json!({ "cut": "Rose" })).unwrap_err().to_string().contains("Cut must be one of Round, Oval"));
        // Every preset the menus offer builds its stone and its settings.
        for s in STONES {
            let made = build(STONE, s.gem(), &stone_params(s.gem()), Seat::default(), None).unwrap();
            assert_eq!(made.gem, Some(s.gem()), "{}", s.key);
            assert_eq!(gem_label(s.gem()), s.label);
        }
        for s in SETTINGS {
            let mut n = 10;
            let features = setting_features(s.key, 2, round, true, &mut || { n += 1; n }).unwrap();
            assert!(features.iter().all(|f| f.operation.sources() == vec![2] && f.operation.consumes().is_empty()), "{}", s.key);
            let bur = features.iter().find(|f| matches!(&f.operation, Operation::Builder { key, .. } if key == BUR)).unwrap();
            assert_eq!((bur.component.attach, bur.component.stage), (Attach::Cut, Stage::Bench), "{}", s.key);
        }
    }

    /// Build costs for the report: `cargo test -p ringdesign-core measured_builders -- --ignored --nocapture`.
    #[test]
    #[ignore = "timings only"]
    fn measured_builders() {
        let lib = AlphaLibrary::builtin();
        let sizes = [("preview 256x128", params()), ("export 1024x384", BuildParams { theta_steps: 1024, profile_steps: 384, ..BuildParams::default() })];
        for (label, p) in &sizes {
            let surface = crate::mesh::try_build(&court(), &lib, *p).unwrap().mesh;
            let never = AtomicBool::new(false);
            // Best of three evaluations, so a builder's cost is its document's less the stone's alone.
            let best = |d: &RingDesign| {
                (0..3)
                    .map(|_| {
                        let started = std::time::Instant::now();
                        let e = cad::evaluate_with(d, &lib, *p, &BuildCtx::new(&never).with_surface(&surface)).unwrap();
                        (started.elapsed().as_secs_f64() * 1e3, e)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .unwrap()
            };
            let mut stone_only = solitaire();
            stone_only.cad.as_mut().unwrap().remove_with_dependents(3).unwrap();
            stone_only.cad.as_mut().unwrap().remove_with_dependents(4).unwrap();
            let (base_ms, _) = best(&stone_only);
            for (key, from) in [(STONE, "claw4"), (CLAW, "claw4"), (BASKET, "basket"), (BEZEL, "bezel"), (BUR, "claw4"), (HALO, "halo")] {
                let mut d = set(from);
                let doc = d.cad.as_mut().unwrap();
                let id = doc.features.iter().find(|f| matches!(&f.operation, Operation::Builder { key: k, .. } if k == key)).unwrap().id;
                let others: Vec<Id> = doc.features.iter().filter(|f| f.id > 2 && f.id != id).map(|f| f.id).collect();
                for o in others {
                    doc.apply(&CadEdit::Remove { id: o }).unwrap();
                }
                let (ms, e) = best(&d);
                let c = e.components.iter().find(|c| c.id == id).unwrap();
                let own = if key == STONE { ms } else { ms - base_ms };
                eprintln!("{label:<16} {key:<12} {own:>7.1} ms, {} faces, {} patches, {} creases", c.mesh.faces.len(), c.trace.patches.len(), c.edges.len());
            }
            for key in ["claw4", "claw6", "bezel", "basket", "halo"] {
                let d = set(key);
                let started = std::time::Instant::now();
                let built = crate::mesh::try_build(&d, &lib, *p).unwrap();
                let ms = started.elapsed().as_secs_f64() * 1e3;
                eprintln!("{label:<16} {key:<12} ring {ms:>7.1} ms, parts {:>5} ms, {} faces, open {} non-manifold {}", built.parts.ms, built.mesh.faces.len(), built.report.validation.boundary_edges, built.report.validation.non_manifold_edges);
            }
        }
    }
}
