//! Parts built round a stone by name — stone, claw head, bezel, basket, seat bur, halo — as named meshes in its frame.
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

/// What a builder made: named solid, patch kinds, creases, the stone's gem and, for a stone, its seat.
#[derive(Clone, Debug)]
pub struct Made {
    pub key: String,
    pub named: Named,
    /// The surface each patch reads as, by patch index.
    pub kinds: Vec<SurfaceKind>,
    /// Polylines along every edge whose faces turn at least [`CREASE_DEG`], a sampled round's own facets aside ([`creases`]).
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
        let creases = creases(&named, CREASE_DEG);
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

/// Largest turn a facet of a sampled round makes: a round sampled with eight sides or more.
pub const FACET_MAX_DEG: f64 = 45.0;

/// Polylines along every edge of a closed named solid whose two faces turn at least `min_deg`;
/// none past 20 000 such edges. A patch is a sampled round when most of its own turns under
/// [`FACET_MAX_DEG`] repeat, within a tenth, on the next line across a face — a rail's ten
/// sides each turn 36° — and inside it a turn like those is a facet, not a crease, down to the
/// stubs a join retriangulates. Measured on the heads (`the_dihedral_census_of_the_heads`): every
/// rail facet goes and nothing else does — the stone's notch in a claw turns 44.7° and up beside
/// flat faces, a bezel's rim corner 37.6° between corners of 45.0° and 56.6°, and every edge
/// between two patches keeps the plain rule.
pub fn creases(named: &Named, min_deg: f64) -> Vec<Vec<P3>> {
    let solid = &named.solid;
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
    let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let turn: HashMap<(u32, u32), f64> = by_edge
        .iter()
        .filter(|(_, fs)| fs.len() == 2)
        .map(|(e, fs)| (*e, dot(normals[fs[0]], normals[fs[1]]).clamp(-1.0, 1.0).acos().to_degrees()))
        .collect();
    let mut around: HashMap<u32, Vec<u32>> = HashMap::new();
    for &(a, b) in by_edge.keys() {
        around.entry(a).or_default().push(b);
        around.entry(b).or_default().push(a);
    }
    let direction = |a: u32, b: u32| {
        let (p, q) = (solid.v[a as usize], solid.v[b as usize]);
        let d = [q[0] - p[0], q[1] - p[1], q[2] - p[2]];
        let l = dot(d, d).sqrt().max(1e-300);
        d.map(|v| v / l)
    };
    // Whether the turn `t` of edge `e` comes again on the line through face `f`'s third corner, along `e`.
    let repeats = |e: (u32, u32), f: usize, t: f64| {
        let Some(&c) = solid.f[f].iter().find(|v| **v != e.0 && **v != e.1) else { return false };
        let along = direction(e.0, e.1);
        let next = around.get(&c).into_iter().flatten().filter(|x| **x != e.0 && **x != e.1).map(|x| (dot(direction(c, *x), along).abs(), *x)).max_by(|a, b| a.0.total_cmp(&b.0));
        next.is_some_and(|(cos, x)| cos >= 0.9 && turn.get(&(c.min(x), c.max(x))).is_some_and(|u| (u - t).abs() <= 0.1 * t))
    };
    // Each patch's own turns in the facet range, and those of them that repeat across a face.
    let inside = |fs: &[usize]| named.patch.get(fs[0]).filter(|p| named.patch.get(fs[1]) == Some(*p)).copied();
    let mut own: HashMap<u32, (usize, Vec<f64>)> = HashMap::new();
    for (e, fs) in by_edge.iter().filter(|(_, fs)| fs.len() == 2) {
        let (Some(p), Some(&t)) = (inside(fs), turn.get(e)) else { continue };
        if t >= min_deg && t < FACET_MAX_DEG {
            let entry = own.entry(p).or_default();
            entry.0 += 1;
            if repeats(*e, fs[0], t) || repeats(*e, fs[1], t) {
                entry.1.push(t);
            }
        }
    }
    // A sampled round's facet turn: the middle repeating turn, where most of the patch's turns repeat.
    let facet: HashMap<u32, f64> = own
        .into_iter()
        .filter(|(_, (all, rep))| 2 * rep.len() > *all)
        .map(|(p, (_, mut rep))| {
            rep.sort_by(f64::total_cmp);
            (p, rep[rep.len() / 2])
        })
        .collect();
    let mut edges: Vec<(u32, u32)> = by_edge
        .iter()
        .filter(|(e, fs)| {
            let Some(&t) = turn.get(e) else { return false };
            let sampled = inside(fs).and_then(|p| facet.get(&p)).is_some_and(|f| (t - f).abs() <= 0.1 * f);
            t >= min_deg && !sampled
        })
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

/// Where setting `key` stands the girdle over the metal: the culet clear under claws, a bezel's base sunk.
pub fn stand_off_mm(key: &str, gem: Gem) -> f64 {
    match key {
        BEZEL | "bezel" => setting::collet_depth_mm(gem) - BEZEL_SINK_MM,
        _ => gem.pavilion_mm() + CULET_CLEAR_MM,
    }
}

/// The metal under a bezel's wall: its lowest point when the wall is wholly over metal, else its highest.
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

/// What builder `key` makes for `gem` in the stone's frame, over metal at `seat` and wherever `floor` finds it.
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

/// Melee in collets or claw heads round the grown outline at equal arc length, counted by `pave::halo`'s rule, tied by a rail.
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

/// The features setting preset `key` adds round stone `stone`: head (and halo) joined, seat bur cut and Bench under sand.
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
    // Under sand a made setting is soldered on and its seat drilled after the pour; lost wax casts both in place.
    let stage = if sand { Stage::Bench } else { Stage::Cast };
    let mut head = feature_on(next(), name, head_key, stone, params);
    head.component.stage = stage;
    out.push(head);
    if key == "halo" {
        let mut halo = feature_on(next(), "Halo", HALO, stone, json!({}));
        halo.component.stage = stage;
        out.push(halo);
    }
    if !(gem.form == GemForm::Cabochon && key != "bezel") {
        let mut bur = feature_on(next(), "Seat bur", BUR, stone, json!({ "through": key != "bezel" }));
        bur.component.stage = stage;
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

    /// The edge between two vertices as the bits of its ends, lower first.
    fn segment(named: &Named, a: u32, b: u32) -> [[u64; 3]; 2] {
        let (p, q) = (named.solid.v[a as usize].map(f64::to_bits), named.solid.v[b as usize].map(f64::to_bits));
        if p <= q { [p, q] } else { [q, p] }
    }
    /// Every edge the crease rule keeps, as [`segment`]s.
    fn crease_segments(named: &Named) -> HashSet<[[u64; 3]; 2]> {
        creases(named, CREASE_DEG)
            .iter()
            .flat_map(|l| l.windows(2).map(|w| {
                let (p, q) = (w[0].map(f64::to_bits), w[1].map(f64::to_bits));
                if p <= q { [p, q] } else { [q, p] }
            }))
            .collect()
    }
    /// Every edge two faces share, with the angle between their normals in degrees and the two faces.
    fn dihedrals(s: &Solid) -> Vec<((u32, u32), f64, [usize; 2])> {
        let normal = |f: &[u32; 3]| {
            let [a, b, c] = f.map(|i| s.v[i as usize]);
            let (e, g) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]);
            let n = [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-300);
            n.map(|v| v / l)
        };
        let mut by_edge: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
        for (i, f) in s.f.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (f[k], f[(k + 1) % 3]);
                by_edge.entry((a.min(b), a.max(b))).or_default().push(i);
            }
        }
        by_edge
            .into_iter()
            .filter(|(_, fs)| fs.len() == 2)
            .map(|(e, fs)| {
                let (n, m) = (normal(&s.f[fs[0]]), normal(&s.f[fs[1]]));
                let dot = (n[0] * m[0] + n[1] * m[1] + n[2] * m[2]).clamp(-1.0, 1.0);
                (e, dot.acos().to_degrees(), [fs[0], fs[1]])
            })
            .collect()
    }

    /// The dihedrals of the heads' edges by kind — tube facets, joins between parts, the stone's notch; run `--ignored --nocapture`.
    #[test]
    #[ignore]
    fn the_dihedral_census_of_the_heads() {
        let bins = [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 60.0, 90.0, 180.1];
        let census = |label: &str, plain: Option<&Named>, named: &Named| {
            let known: HashSet<[u64; 3]> = plain.map(|p| p.solid.v.iter().map(|q| q.map(f64::to_bits)).collect()).unwrap_or_default();
            let mut classes: Vec<(&str, Vec<usize>, f64, f64)> = ["facet", "join", "notch"].iter().map(|c| (*c, vec![0; bins.len() - 1], f64::INFINITY, 0.0)).collect();
            for ((a, b), deg, [f, g]) in dihedrals(&named.solid) {
                let fresh = plain.is_some() && [a, b].iter().any(|v| !known.contains(&named.solid.v[*v as usize].map(f64::to_bits)));
                let class = if fresh { 2 } else if named.patch[f] == named.patch[g] { 0 } else { 1 };
                let bin = bins.windows(2).position(|w| deg >= w[0] && deg < w[1]).unwrap_or(bins.len() - 2);
                let c = &mut classes[class];
                c.1[bin] += 1;
                if deg >= 5.0 {
                    c.2 = c.2.min(deg);
                }
                c.3 = c.3.max(deg);
            }
            for (class, counts, lo, hi) in classes.iter().filter(|c| c.1.iter().sum::<usize>() > 0) {
                let row: Vec<String> = counts.iter().zip(bins.windows(2)).filter(|(n, _)| **n > 0).map(|(n, w)| format!("{:.0}-{:.0}°: {n}", w[0], w[1])).collect();
                eprintln!("{label:<22} {class:<6} {:>5} edges, turning {lo:.1}..{hi:.1}°  [{}]", counts.iter().sum::<usize>(), row.join(", "));
            }
            // Distinct turns at and past 30° between each pair of patches, to the tenth of a degree, and how many the rule keeps.
            let kept = crease_segments(named);
            let mut turns: std::collections::BTreeMap<(String, i64), (usize, usize)> = std::collections::BTreeMap::new();
            for ((a, b), deg, [f, g]) in dihedrals(&named.solid).into_iter().filter(|e| e.1 >= 30.0) {
                let (p, q) = (named.face_name(f).unwrap_or("?"), named.face_name(g).unwrap_or("?"));
                let pair = if p <= q { format!("{p}|{q}") } else { format!("{q}|{p}") };
                let e = turns.entry((pair, (deg * 10.0).round() as i64)).or_default();
                e.0 += 1;
                e.1 += usize::from(kept.contains(&segment(named, a, b)));
            }
            let mut by_pair: std::collections::BTreeMap<String, (Vec<String>, usize, usize)> = std::collections::BTreeMap::new();
            for ((pair, tenth), (n, k)) in turns {
                let e = by_pair.entry(pair).or_default();
                e.0.push(format!("{:.1}°x{n}", tenth as f64 / 10.0));
                e.1 += n;
                e.2 += k;
            }
            for (pair, (list, n, k)) in by_pair {
                let shown: Vec<String> = list.iter().take(8).cloned().collect();
                eprintln!("    {pair:<28} creases {k:>4} of {n:>4}; {} kinds: {}", list.len(), shown.join(" "));
            }
        };
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let wire = setting::prong_wire_mm(gem);
        for (label, prongs, rails) in [("claw head, 4", 4, Rails::Seat), ("claw head, 6", 6, Rails::Seat), ("basket, 3 rails", 4, Rails::Basket(3))] {
            let plain = Named::union_all(setting::claw_parts_named(gem, prongs, wire, rails, None)).unwrap();
            let head = setting::claw_head_named(gem, prongs, wire, rails, None).unwrap();
            census(label, Some(&plain), &head);
        }
        census("bezel", None, &setting::collet_named(gem, setting::collet_wall_mm(gem), setting::collet_lip(gem), -setting::collet_depth_mm(gem)));
        census("stone", None, &setting::envelope_named(gem, 0.0));
        for (label, melee_mm) in [("halo melee head 1.3", 1.3), ("halo melee head 2.0", 2.0)] {
            let melee = Gem::calibrated(GemCut::Round, melee_mm);
            let wire = setting::prong_wire_mm(melee);
            let plain = Named::union_all(setting::claw_parts_named(melee, 4, wire, Rails::Seat, None)).unwrap();
            census(label, Some(&plain), &setting::claw_head_named(melee, 4, wire, Rails::Seat, None).unwrap());
        }
        let seat = Seat { surface_z: -1.0, through_mm: None };
        for style in ["Claw", "Bezel"] {
            let made = build(HALO, gem, &json!({ "style": style }), seat, None).unwrap();
            census(&format!("halo, {style}"), None, &made.named);
        }
    }

    #[test]
    fn a_rails_facets_are_no_creases_and_a_hover_on_a_claws_side_lands_on_the_claw() {
        use crate::interaction::pick::{Entity, Filter, PickScene, Ray, ViewScale};
        let lib = AlphaLibrary::builtin();
        let d = set("claw4");
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let head = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap().clone();
        let made = head.made.clone().unwrap();
        let patch = |name: &str| made.named.names.iter().position(|n| n == name).unwrap() as u32;
        // Not one of a rail's own facets is a crease; every turn of 30° and more elsewhere is.
        let kept = crease_segments(&made.named);
        let (mut rails, mut others) = ((0, 0), (0, 0));
        for ((a, b), t, [f, g]) in dihedrals(&made.named.solid).into_iter().filter(|e| e.1 >= CREASE_DEG) {
            let rail = made.named.patch[f] == made.named.patch[g] && made.named.face_name(f).is_some_and(|n| n.ends_with("rail"));
            let count = if rail { &mut rails } else { &mut others };
            count.0 += 1;
            count.1 += usize::from(kept.contains(&segment(&made.named, a, b)));
            assert!(!rail || (t - 36.0).abs() < 0.2, "a rail facet turns 36°: {t}");
        }
        assert!(rails.0 > 1000 && rails.1 == 0, "rail facets {rails:?}");
        assert!(others.0 > 700 && others.1 == others.0, "every other turn stays a crease: {others:?}");
        assert_eq!(head.edges, made.creases);
        // The same build with every turn of 30° a crease, as the rule stood.
        let mut plain = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let old: Vec<Vec<[f64; 3]>> = dihedrals(&made.named.solid).into_iter().filter(|e| e.1 >= CREASE_DEG).map(|((a, b), _, _)| vec![made.named.solid.v[a as usize], made.named.solid.v[b as usize]]).collect();
        plain.parts.evaluated.as_mut().unwrap().components.iter_mut().find(|c| c.id == 3).unwrap().edges = old;
        let (now, then) = (PickScene::build(&built, &d), PickScene::build(&plain, &d));
        // Claw 1's outer side, looked at square on at the Ring viewport's 8 px aperture and 20 px/mm.
        let claw = patch("Claw 1");
        let v = &made.named.solid.v;
        let faces: Vec<usize> = (0..made.named.solid.f.len()).filter(|f| made.named.patch[*f] == claw).collect();
        let n = faces.len() as f64;
        let centre: P3 = std::array::from_fn(|k| faces.iter().map(|f| made.named.solid.f[*f].iter().map(|i| v[*i as usize][k]).sum::<f64>() / 3.0).sum::<f64>() / n);
        let (o, up) = (head.frame.origin, head.frame.z_axis);
        let dot = |a: P3, b: P3| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let out: P3 = {
            let d: P3 = std::array::from_fn(|k| centre[k] - o[k]);
            let flat: P3 = std::array::from_fn(|k| d[k] - up[k] * dot(d, up));
            let l = dot(flat, flat).sqrt();
            flat.map(|x| x / l)
        };
        let right: P3 = [up[1] * out[2] - up[2] * out[1], up[2] * out[0] - up[0] * out[2], up[0] * out[1] - up[1] * out[0]];
        let view = ViewScale { right, up, px_per_mm: 20.0 };
        let facing: Vec<P3> = faces
            .iter()
            .filter_map(|f| {
                let [a, b, c] = made.named.solid.f[*f].map(|i| v[i as usize]);
                let n = { let (e, g) = ([b[0] - a[0], b[1] - a[1], b[2] - a[2]], [c[0] - a[0], c[1] - a[1], c[2] - a[2]]); [e[1] * g[2] - e[2] * g[1], e[2] * g[0] - e[0] * g[2], e[0] * g[1] - e[1] * g[0]] };
                let l = dot(n, n).sqrt();
                (l > 1e-12 && dot(n, out) / l > 0.6).then(|| std::array::from_fn(|k| (a[k] + b[k] + c[k]) / 3.0))
            })
            .collect();
        let hovered = |scene: &PickScene, at: P3| {
            let ray = Ray { origin: std::array::from_fn(|k| at[k] + out[k] * 25.0), direction: out.map(|x| -x) };
            scene.pick(ray, &view, 8.0, Filter::default()).into_iter().next().map(|p| p.entity)
        };
        let on_claw = |scene: &PickScene| facing.iter().filter(|p| hovered(scene, **p) == Some(Entity::Face { feature: 3, face: claw })).count();
        // Measured: 137 of 170 hovers land on the claw against 118; the rest are within reach of its foot's rim and the rails' joins.
        let (now_n, then_n) = (on_claw(&now), on_claw(&then));
        assert!(now_n >= then_n + 15 && 5 * now_n >= 4 * facing.len(), "{now_n} and {then_n} of {} hovers land on the claw", facing.len());
        // A quarter millimetre over the gallery rail, where the rail's top facet used to answer first.
        let height = |p: &P3| dot(std::array::from_fn(|k| p[k] - o[k]), up);
        let rail = patch("Gallery rail");
        let top = (0..made.named.solid.f.len()).filter(|f| made.named.patch[*f] == rail).flat_map(|f| made.named.solid.f[f]).map(|i| height(&v[i as usize])).fold(f64::MIN, f64::max);
        let over = facing.iter().min_by(|a, b| (height(a) - top - 0.25).abs().total_cmp(&(height(b) - top - 0.25).abs())).unwrap();
        assert_eq!(hovered(&now, *over), Some(Entity::Face { feature: 3, face: claw }));
        assert!(matches!(hovered(&then, *over), Some(Entity::Edge { feature: 3, .. })));
    }

    #[test]
    fn under_sand_a_setting_is_soldered_on_after_the_pour_and_its_pattern_marks_the_spot() {
        use crate::castability::{Verdict, judged_field_report};
        let lib = AlphaLibrary::builtin();
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let mut n = 10;
        for (sand, stage) in [(true, Stage::Bench), (false, Stage::Cast)] {
            for key in ["claw4", "claw6", "bezel", "basket", "halo"] {
                let made = setting_features(key, 2, gem, sand, &mut || { n += 1; n }).unwrap();
                assert!(made.iter().all(|f| f.component.stage == stage), "{key} under sand {sand}");
            }
        }
        // Staged Bench the head and its seat leave the pour and the pattern marks where each goes.
        let d = set("claw4");
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let f = judged_field_report(&d, &lib, &d.draft, 192, 128, Some(&built));
        assert_ne!(f.verdict, Verdict::NotCastable, "{:?}", f.notes);
        assert!(f.notes.iter().any(|n| n.contains("is soldered on after the pour")), "{:?}", f.notes);
        assert!(f.notes.iter().any(|n| n.contains("is drilled at the bench")), "{:?}", f.notes);
        // Cast in the pattern, the same head's claws and rails lock the mould, and the verdict names it.
        let mut cast = d.clone();
        for feat in &mut cast.cad.as_mut().unwrap().features {
            if feat.id == 3 {
                feat.component.stage = Stage::Cast;
            }
        }
        let built = crate::mesh::try_build(&cast, &lib, params()).unwrap();
        let f = judged_field_report(&cast, &lib, &cast.draft, 192, 128, Some(&built));
        let head = f.parts.iter().find(|p| p.feature == 3).expect("the head is judged");
        assert!(head.judged && head.undercut_area_mm2 - head.silhouette_mm2 > 1.0, "{head:?}");
        assert!(head.note.contains("stage it Bench"), "{}", head.note);
        assert_ne!(f.verdict, Verdict::Castable);
    }

    #[test]
    fn a_stone_set_as_a_part_is_picked_as_one_and_a_heads_faces_answer_by_patch() {
        use crate::interaction::pick::{Entity, Filter, PickScene, Ray, ViewScale};
        let lib = AlphaLibrary::builtin();
        let d = set("claw4");
        let built = crate::mesh::try_build(&d, &lib, params()).unwrap();
        let scene = PickScene::build(&built, &d);
        let stone = d.cad.as_ref().unwrap().feature(2).unwrap().component.placement.frame_on(&d, Some(&built.mesh)).unwrap();
        let above: [f64; 3] = std::array::from_fn(|k| stone.origin[k] + 20.0 * stone.z_axis[k]);
        let view = ViewScale { right: stone.x_axis, up: stone.y_axis, px_per_mm: 40.0 };
        let down = Ray { origin: above, direction: stone.z_axis.map(|v| -v) };
        let picks = scene.pick(down, &view, 6.0, Filter::default());
        assert_eq!(picks.first().map(|p| &p.entity), Some(&Entity::Part { feature: 2 }), "{picks:?}");
        // Every face of the head names the patch it belongs to.
        let head = built.parts.evaluated.as_ref().unwrap().components.iter().find(|c| c.id == 3).unwrap();
        let names: HashSet<&str> = (0..head.mesh.faces.len() as u32).filter_map(|f| head.trace.patch(head.trace.face_of(f as usize)?)).collect();
        assert!((1..=4).all(|k| names.iter().any(|n| *n == format!("Claw {k}"))), "{names:?}");
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
        // The seat leaves the claws standing on the band: one piece of metal.
        assert_eq!(pieces(&ring), 1);
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
    fn a_stone_moved_by_a_transform_carries_the_head_built_round_it() {
        let lib = AlphaLibrary::builtin();
        let d = solitaire();
        let mut lifted = d.clone();
        let doc = lifted.cad.as_mut().unwrap();
        let lift = Operation::Transform { source: 2, translation: [0.0, 1.0, 0.0], rotation_deg: [0.0; 3] };
        doc.apply(&CadEdit::Add { feature: Feature { id: 9, name: "Lift".into(), enabled: true, operation: lift, component: Component::default() }, after: Some(2) }).unwrap();
        doc.apply(&CadEdit::Operation { id: 3, operation: Operation::Builder { key: CLAW.into(), on: Some(9), params: json!({ "prongs": 4 }) } }).unwrap();
        let head = |d: &RingDesign| cad::evaluate(d, &lib, params()).unwrap().components.into_iter().find(|c| c.id == 3).and_then(|c| c.made).unwrap();
        let (a, b) = (centroid(head(&d).solid()), centroid(head(&lifted).solid()));
        assert!((b[0] - a[0]).abs() < 1e-9 && (b[1] - a[1] - 1.0).abs() < 1e-9 && (b[2] - a[2]).abs() < 1e-9, "{a:?} -> {b:?}");
    }

    #[test]
    fn the_cache_keys_a_setting_on_its_params_and_its_stone_and_a_sand_pattern_leaves_the_seat_to_the_bench() {
        let lib = AlphaLibrary::builtin();
        let d = solitaire();
        let surface = crate::mesh::try_build(&court(), &lib, params()).unwrap().mesh;
        let never = AtomicBool::new(false);
        let cache = std::sync::Mutex::new(cad::Cache::default());
        let memo = cad::Memo::new(&cache).with_epoch(cad::surface_epoch(&surface));
        let ctx = BuildCtx::new(&never).with_surface(&surface);
        let tally = || { let c = cache.lock().unwrap(); (c.hits(), c.misses()) };
        // Cold: stone, head and bur built, and each tessellated (a builder's mesh is its own at every chord).
        cad::evaluate_memo(&d, &lib, params(), &ctx, memo).unwrap();
        assert_eq!(tally(), (0, 6));
        cad::evaluate_memo(&d, &lib, params(), &ctx, memo).unwrap();
        assert_eq!(tally(), (6, 6), "an unchanged document builds nothing");
        // Six claws rebuild the head alone; the stone and the bur answer.
        let mut six = d.clone();
        six.cad.as_mut().unwrap().apply(&CadEdit::Operation { id: 3, operation: Operation::Builder { key: CLAW.into(), on: Some(2), params: json!({ "prongs": 6 }) } }).unwrap();
        cad::evaluate_memo(&six, &lib, params(), &ctx, memo).unwrap();
        assert_eq!(tally(), (10, 8));
        // A moved stone rebuilds everything built round it.
        let mut moved = d.clone();
        moved.cad.as_mut().unwrap().apply(&CadEdit::Placement { id: 2, placement: Placement::ring(60.0, 2.8) }).unwrap();
        cad::evaluate_memo(&moved, &lib, params(), &ctx, memo).unwrap();
        assert_eq!(tally(), (10, 14));
        // Under sand the seat is cut after the pour, so the pattern carries the head alone; lost wax cuts it in place.
        let pattern = crate::mesh::try_build_pattern(&d, &lib, params()).unwrap();
        assert_eq!((pattern.parts.joined, pattern.parts.cut, pattern.parts.references), (1, 0, 1));
        let mut wax = d.clone();
        wax.draft.process = crate::castability::CastProcess::LostWax;
        let wax = crate::mesh::try_build_pattern(&wax, &lib, params()).unwrap();
        assert_eq!((wax.parts.joined, wax.parts.cut), (1, 1));
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
            assert_eq!(pieces(&as_solid(&built.mesh)), 1, "{key}: the setting and the band are one piece of metal");
            eprintln!("{key}: {} faces, {:.3} mm³, into the band {overlap:.3} mm³, ring {} faces", head.named.solid.f.len(), head.solid().volume(), built.mesh.faces.len());
        }
    }

    #[test]
    fn a_halo_seats_its_melee_at_equal_arc_length_by_the_pave_rule() {
        let gem = Gem::calibrated(GemCut::Round, 6.5);
        let made = build(HALO, gem, &json!({}), Seat::default(), None).unwrap();
        // 1.3 mm melee on a 4.55 mm ring (3.25 + 0.3 gap + 1.0) at a 2.25 mm pitch: floor(2π 4.55 / 2.25) = 12.
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
