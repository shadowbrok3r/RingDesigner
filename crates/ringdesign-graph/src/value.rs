//! The closed set of things a wire can carry.
//!
//! Scalars and lists are plain; everything the core owns travels as an
//! `Arc` handle (or by value where the core type is `Copy`), so cloning a
//! value for every item of an implicit list costs a pointer, and a design
//! with a baked alpha library behind it is never deep-copied by the
//! evaluator. [`Literal`] is the serde subset an unwired pin can hold in a
//! graph file. [`ValueKind`] is the type a pin declares, and
//! [`ValueKind::accepts`] / [`ValueKind::coerce`] are the one coercion
//! table, pinned row by row in the tests.

use std::fmt;
use std::sync::Arc;

use ringdesign_core::alpha::ProcRecipe;
use ringdesign_core::castability::FieldReport;
use ringdesign_core::drawn::DrawnAlpha;
use ringdesign_core::field::Remap;
use ringdesign_core::gem::Gem;
use ringdesign_core::pave::GenRecipe;
use ringdesign_core::profile::SignetHead;
use ringdesign_core::stones::StonesReport;
use ringdesign_core::svg::SvgAlpha;
use ringdesign_core::text::TextAlpha;
use ringdesign_core::{
    BandProfile, BuildParams, CustomOutline, EmbeddedAlpha, Layer, LayerEntry, LayerStack, Mesh,
    RingDesign, ShankStyle, Window,
};
use serde::{Deserialize, Serialize};

/// A solid from the free-mode kernel. The kernel crate implements it; this
/// crate only carries it, so `Value::Solid` exists in every build.
pub trait SolidHandle: Send + Sync {
    /// One line for badges and logs.
    fn describe(&self) -> String;
    /// The solid's surface as a core mesh, if the kernel can produce one.
    fn to_mesh(&self) -> Option<Mesh>;
}

/// An alpha as source data: what the design file carries and the library
/// bakes on load. `AlphaRef` names the baked result.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AlphaSource {
    Procedural(ProcRecipe),
    Text(TextAlpha),
    Svg(SvgAlpha),
    Drawn(DrawnAlpha),
    Embedded(EmbeddedAlpha),
}

impl AlphaSource {
    /// The library name the baked alpha answers to.
    pub fn name(&self) -> &str {
        match self {
            AlphaSource::Procedural(p) => &p.name,
            AlphaSource::Text(t) => &t.name,
            AlphaSource::Svg(s) => &s.name,
            AlphaSource::Drawn(d) => &d.name,
            AlphaSource::Embedded(e) => &e.name,
        }
    }
}

/// What a wire carries.
#[derive(Clone)]
pub enum Value {
    Null,
    Number(f64),
    Int(i64),
    Bool(bool),
    Text(String),
    List(Vec<Value>),
    Design(Arc<RingDesign>),
    Profile(BandProfile),
    Shank(Arc<ShankStyle>),
    Head(SignetHead),
    Outline(Arc<CustomOutline>),
    Gem(Gem),
    Window(Window),
    Remap(Remap),
    Layer(Arc<Layer>),
    Entry(Arc<LayerEntry>),
    Stack(Arc<LayerStack>),
    Recipe(Arc<GenRecipe>),
    AlphaSource(Arc<AlphaSource>),
    AlphaRef(String),
    Build(BuildParams),
    Field(Arc<FieldReport>),
    Stones(Arc<StonesReport>),
    Mesh(Arc<Mesh>),
    Solid(Arc<dyn SolidHandle>),
    /// A point list, `[x, y]` pairs in whichever chart the consuming node
    /// documents.
    Path(Arc<Vec<[f64; 2]>>),
    Json(Arc<serde_json::Value>),
}

/// The type a pin declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ValueKind {
    /// Accepts every value unchanged; for list utilities and sinks that
    /// only describe.
    Any,
    Null,
    Number,
    Int,
    Bool,
    Text,
    List,
    Design,
    Profile,
    Shank,
    Head,
    Outline,
    Gem,
    Window,
    Remap,
    Layer,
    Entry,
    Stack,
    Recipe,
    AlphaSource,
    AlphaRef,
    Build,
    Field,
    Stones,
    Mesh,
    Solid,
    Path,
    Json,
}

impl ValueKind {
    pub const ALL: &'static [ValueKind] = &[
        ValueKind::Any,
        ValueKind::Null,
        ValueKind::Number,
        ValueKind::Int,
        ValueKind::Bool,
        ValueKind::Text,
        ValueKind::List,
        ValueKind::Design,
        ValueKind::Profile,
        ValueKind::Shank,
        ValueKind::Head,
        ValueKind::Outline,
        ValueKind::Gem,
        ValueKind::Window,
        ValueKind::Remap,
        ValueKind::Layer,
        ValueKind::Entry,
        ValueKind::Stack,
        ValueKind::Recipe,
        ValueKind::AlphaSource,
        ValueKind::AlphaRef,
        ValueKind::Build,
        ValueKind::Field,
        ValueKind::Stones,
        ValueKind::Mesh,
        ValueKind::Solid,
        ValueKind::Path,
        ValueKind::Json,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ValueKind::Any => "any",
            ValueKind::Null => "null",
            ValueKind::Number => "number",
            ValueKind::Int => "integer",
            ValueKind::Bool => "boolean",
            ValueKind::Text => "text",
            ValueKind::List => "list",
            ValueKind::Design => "design",
            ValueKind::Profile => "profile",
            ValueKind::Shank => "shank",
            ValueKind::Head => "head",
            ValueKind::Outline => "outline",
            ValueKind::Gem => "gem",
            ValueKind::Window => "window",
            ValueKind::Remap => "remap",
            ValueKind::Layer => "layer",
            ValueKind::Entry => "entry",
            ValueKind::Stack => "stack",
            ValueKind::Recipe => "recipe",
            ValueKind::AlphaSource => "alpha source",
            ValueKind::AlphaRef => "alpha",
            ValueKind::Build => "build",
            ValueKind::Field => "field report",
            ValueKind::Stones => "stones report",
            ValueKind::Mesh => "mesh",
            ValueKind::Solid => "solid",
            ValueKind::Path => "path",
            ValueKind::Json => "json",
        }
    }

    /// Whether a value of kind `from` may be wired into a pin of this kind,
    /// directly or through [`ValueKind::coerce`]. `Null` is accepted
    /// everywhere: it is the per-item failure marker and must flow.
    pub fn accepts(self, from: ValueKind) -> bool {
        use ValueKind::*;
        if self == Any || from == Null || self == from {
            return true;
        }
        matches!(
            (self, from),
            (Number, Int | Bool)
                | (Int, Number | Bool)
                | (Text, Number | Int | Bool | AlphaRef)
                | (AlphaRef, Text)
                | (Entry, Layer)
                | (Stack, Entry | Layer)
                | (Path, Json)
                | (Json, Number | Int | Bool | Text | List | Path)
        )
    }

    /// Convert `v` for a pin of this kind, or say why it cannot be.
    pub fn coerce(self, v: Value) -> Result<Value, CoerceError> {
        use ValueKind::*;
        let from = v.kind();
        if self == Any || from == Null || self == from {
            return Ok(v);
        }
        let refuse = |v: Value| Err(CoerceError { from: v.kind(), to: self, detail: None });
        match (self, v) {
            (Number, Value::Int(i)) => Ok(Value::Number(i as f64)),
            (Number, Value::Bool(b)) => Ok(Value::Number(if b { 1.0 } else { 0.0 })),
            (Int, Value::Number(x)) => {
                if x.is_finite() {
                    Ok(Value::Int(x.round() as i64))
                } else {
                    Err(CoerceError { from, to: self, detail: Some("not finite".into()) })
                }
            }
            (Int, Value::Bool(b)) => Ok(Value::Int(i64::from(b))),
            (Text, Value::Number(x)) => Ok(Value::Text(fmt_number(x))),
            (Text, Value::Int(i)) => Ok(Value::Text(i.to_string())),
            (Text, Value::Bool(b)) => Ok(Value::Text(b.to_string())),
            (Text, Value::AlphaRef(s)) => Ok(Value::Text(s)),
            (AlphaRef, Value::Text(s)) => Ok(Value::AlphaRef(s)),
            (Entry, Value::Layer(l)) => Ok(Value::Entry(Arc::new(LayerEntry::new(layer_label(&l), (*l).clone())))),
            (Stack, Value::Entry(e)) => Ok(Value::Stack(Arc::new(LayerStack { layers: vec![(*e).clone()] }))),
            (Stack, Value::Layer(l)) => {
                let entry = LayerEntry::new(layer_label(&l), (*l).clone());
                Ok(Value::Stack(Arc::new(LayerStack { layers: vec![entry] })))
            }
            (Path, Value::Json(j)) => match serde_json::from_value::<Vec<[f64; 2]>>((*j).clone()) {
                Ok(pts) => Ok(Value::Path(Arc::new(pts))),
                Err(e) => Err(CoerceError { from, to: self, detail: Some(format!("not a list of [x, y] pairs: {e}")) }),
            },
            (Json, v @ (Value::Number(_) | Value::Int(_) | Value::Bool(_) | Value::Text(_) | Value::List(_) | Value::Path(_))) => {
                match v.to_json() {
                    Some(j) => Ok(Value::Json(Arc::new(j))),
                    None => Err(CoerceError { from, to: self, detail: Some("a list item is not representable as JSON".into()) }),
                }
            }
            (_, v) => refuse(v),
        }
    }
}

/// A value a pin of one kind would not take.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoerceError {
    pub from: ValueKind,
    pub to: ValueKind,
    pub detail: Option<String>,
}

impl fmt::Display for CoerceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot take {} as {}", self.from.label(), self.to.label())?;
        if let Some(d) = &self.detail {
            write!(f, ": {d}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CoerceError {}

fn fmt_number(x: f64) -> String {
    if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{x:.0}")
    } else {
        let s = format!("{x:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A short name for a layer, used when one is wrapped into an entry.
fn layer_label(l: &Layer) -> &'static str {
    match l {
        Layer::Tiling(_) => "Tiling",
        Layer::Border(_) => "Border",
        Layer::Milgrain(_) => "Milgrain",
        Layer::SeatPad(_) => "Seat",
        Layer::SeatRun(_) => "Seat run",
        Layer::Signet(_) => "Signet",
        Layer::Curve(_) => "Wire",
        Layer::Flutes(_) => "Flutes",
        Layer::Decals(_) => "Decals",
        Layer::Group(_) => "Group",
        Layer::Openwork(_) => "Openwork",
    }
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::Null => ValueKind::Null,
            Value::Number(_) => ValueKind::Number,
            Value::Int(_) => ValueKind::Int,
            Value::Bool(_) => ValueKind::Bool,
            Value::Text(_) => ValueKind::Text,
            Value::List(_) => ValueKind::List,
            Value::Design(_) => ValueKind::Design,
            Value::Profile(_) => ValueKind::Profile,
            Value::Shank(_) => ValueKind::Shank,
            Value::Head(_) => ValueKind::Head,
            Value::Outline(_) => ValueKind::Outline,
            Value::Gem(_) => ValueKind::Gem,
            Value::Window(_) => ValueKind::Window,
            Value::Remap(_) => ValueKind::Remap,
            Value::Layer(_) => ValueKind::Layer,
            Value::Entry(_) => ValueKind::Entry,
            Value::Stack(_) => ValueKind::Stack,
            Value::Recipe(_) => ValueKind::Recipe,
            Value::AlphaSource(_) => ValueKind::AlphaSource,
            Value::AlphaRef(_) => ValueKind::AlphaRef,
            Value::Build(_) => ValueKind::Build,
            Value::Field(_) => ValueKind::Field,
            Value::Stones(_) => ValueKind::Stones,
            Value::Mesh(_) => ValueKind::Mesh,
            Value::Solid(_) => ValueKind::Solid,
            Value::Path(_) => ValueKind::Path,
            Value::Json(_) => ValueKind::Json,
        }
    }

    pub fn is_null(&self) -> bool {
        matches!(self, Value::Null)
    }

    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(x) => Some(*x),
            Value::Int(i) => Some(*i as f64),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(i) => Some(*i),
            Value::Number(x) if x.is_finite() => Some(x.round() as i64),
            Value::Bool(b) => Some(i64::from(*b)),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            Value::Int(i) => Some(*i != 0),
            Value::Number(x) => Some(*x != 0.0),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Value::Text(s) | Value::AlphaRef(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_list(&self) -> Option<&[Value]> {
        match self {
            Value::List(v) => Some(v),
            _ => None,
        }
    }

    /// The JSON form of a literal-shaped value; `None` for handles.
    pub fn to_json(&self) -> Option<serde_json::Value> {
        Some(match self {
            Value::Null => serde_json::Value::Null,
            Value::Number(x) => serde_json::json!(x),
            Value::Int(i) => serde_json::json!(i),
            Value::Bool(b) => serde_json::json!(b),
            Value::Text(s) | Value::AlphaRef(s) => serde_json::json!(s),
            Value::List(items) => serde_json::Value::Array(items.iter().map(Value::to_json).collect::<Option<Vec<_>>>()?),
            Value::Path(p) => serde_json::json!(p.as_slice()),
            Value::Json(j) => (**j).clone(),
            _ => return None,
        })
    }

    /// The JSON form of any serializable value, handles included; `None`
    /// for meshes, reports and solids.
    pub fn to_json_any(&self) -> Option<serde_json::Value> {
        if let Some(j) = self.to_json() {
            return Some(j);
        }
        match self {
            Value::Design(d) => serde_json::to_value(&**d).ok(),
            Value::Profile(p) => serde_json::to_value(p).ok(),
            Value::Shank(s) => serde_json::to_value(&**s).ok(),
            Value::Head(h) => serde_json::to_value(h).ok(),
            Value::Outline(o) => serde_json::to_value(&**o).ok(),
            Value::Gem(g) => serde_json::to_value(g).ok(),
            Value::Window(w) => serde_json::to_value(w).ok(),
            Value::Remap(r) => serde_json::to_value(r).ok(),
            Value::Layer(l) => serde_json::to_value(&**l).ok(),
            Value::Entry(e) => serde_json::to_value(&**e).ok(),
            Value::Stack(s) => serde_json::to_value(&**s).ok(),
            Value::Recipe(r) => serde_json::to_value(&**r).ok(),
            Value::AlphaSource(a) => serde_json::to_value(&**a).ok(),
            Value::Build(b) => serde_json::to_value(b).ok(),
            _ => None,
        }
    }

    /// One line for badges, logs and error messages.
    pub fn summary(&self) -> String {
        match self {
            Value::Null => "null".into(),
            Value::Number(x) => fmt_number(*x),
            Value::Int(i) => i.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Text(s) => format!("{s:?}"),
            Value::List(v) => format!("list ×{}", v.len()),
            Value::Design(d) => format!("design {:?}, {} layers", d.name, d.layers.layers.len()),
            Value::Profile(p) => format!("profile {:.2} × {:.2} mm", p.width_mm, p.thickness_mm),
            Value::Shank(s) => format!("shank {:?}", s.kind),
            Value::Head(h) => format!("head {:?} {:.1} mm", h.outline, h.length_mm),
            Value::Outline(o) => format!("outline {:?}", o.name),
            Value::Gem(g) => format!("gem {:?} {:.2} mm", g.cut, g.w_mm),
            Value::Window(w) => format!("window {:.0}° over {:.0}°", w.theta_deg, w.span_deg),
            Value::Remap(_) => "remap".into(),
            Value::Layer(l) => format!("layer {}", layer_label(l)),
            Value::Entry(e) => format!("entry {:?}", e.name),
            Value::Stack(s) => format!("stack ×{}", s.layers.len()),
            Value::Recipe(_) => "recipe".into(),
            Value::AlphaSource(a) => format!("alpha source {:?}", a.name()),
            Value::AlphaRef(s) => format!("alpha {s:?}"),
            Value::Build(b) => format!("build {}×{}", b.theta_steps, b.profile_steps),
            Value::Field(f) => format!("field {:?} {:.3}%", f.verdict, f.undercut_fraction() * 100.0),
            Value::Stones(s) => format!("stones ×{}", s.seats.len()),
            Value::Mesh(m) => format!("mesh {} tris", m.faces.len()),
            Value::Solid(s) => s.describe(),
            Value::Path(p) => format!("path ×{}", p.len()),
            Value::Json(j) => {
                let s = j.to_string();
                if s.len() > 40 { format!("{}…", &s[..40]) } else { s }
            }
        }
    }
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::List(v) => f.debug_list().entries(v).finish(),
            other => write!(f, "{}", other.summary()),
        }
    }
}

/// Handles compare by identity, literals by value.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Text(a), Value::Text(b)) | (Value::AlphaRef(a), Value::AlphaRef(b)) => a == b,
            (Value::List(a), Value::List(b)) => a == b,
            (Value::Design(a), Value::Design(b)) => Arc::ptr_eq(a, b),
            (Value::Profile(a), Value::Profile(b)) => {
                serde_json::to_string(a).ok() == serde_json::to_string(b).ok()
            }
            (Value::Shank(a), Value::Shank(b)) => Arc::ptr_eq(a, b),
            (Value::Head(a), Value::Head(b)) => serde_json::to_string(a).ok() == serde_json::to_string(b).ok(),
            (Value::Outline(a), Value::Outline(b)) => Arc::ptr_eq(a, b),
            (Value::Gem(a), Value::Gem(b)) => a == b,
            (Value::Window(a), Value::Window(b)) => a == b,
            (Value::Remap(a), Value::Remap(b)) => a == b,
            (Value::Layer(a), Value::Layer(b)) => Arc::ptr_eq(a, b),
            (Value::Entry(a), Value::Entry(b)) => Arc::ptr_eq(a, b),
            (Value::Stack(a), Value::Stack(b)) => Arc::ptr_eq(a, b),
            (Value::Recipe(a), Value::Recipe(b)) => Arc::ptr_eq(a, b),
            (Value::AlphaSource(a), Value::AlphaSource(b)) => Arc::ptr_eq(a, b),
            (Value::Build(a), Value::Build(b)) => serde_json::to_string(a).ok() == serde_json::to_string(b).ok(),
            (Value::Field(a), Value::Field(b)) => Arc::ptr_eq(a, b),
            (Value::Stones(a), Value::Stones(b)) => Arc::ptr_eq(a, b),
            (Value::Mesh(a), Value::Mesh(b)) => Arc::ptr_eq(a, b),
            (Value::Solid(a), Value::Solid(b)) => Arc::ptr_eq(a, b),
            (Value::Path(a), Value::Path(b)) => a == b,
            (Value::Json(a), Value::Json(b)) => a == b,
            _ => false,
        }
    }
}

macro_rules! from_scalar {
    ($($t:ty => $v:ident),* $(,)?) => {$(
        impl From<$t> for Value {
            fn from(x: $t) -> Self { Value::$v(x.into()) }
        }
    )*};
}
from_scalar!(f64 => Number, f32 => Number, i64 => Int, i32 => Int, u32 => Int, bool => Bool, String => Text, &str => Text);

impl From<RingDesign> for Value {
    fn from(d: RingDesign) -> Self {
        Value::Design(Arc::new(d))
    }
}
impl From<BandProfile> for Value {
    fn from(p: BandProfile) -> Self {
        Value::Profile(p)
    }
}
impl From<ShankStyle> for Value {
    fn from(s: ShankStyle) -> Self {
        Value::Shank(Arc::new(s))
    }
}
impl From<SignetHead> for Value {
    fn from(h: SignetHead) -> Self {
        Value::Head(h)
    }
}
impl From<Gem> for Value {
    fn from(g: Gem) -> Self {
        Value::Gem(g)
    }
}
impl From<Window> for Value {
    fn from(w: Window) -> Self {
        Value::Window(w)
    }
}
impl From<Remap> for Value {
    fn from(r: Remap) -> Self {
        Value::Remap(r)
    }
}
impl From<Layer> for Value {
    fn from(l: Layer) -> Self {
        Value::Layer(Arc::new(l))
    }
}
impl From<LayerEntry> for Value {
    fn from(e: LayerEntry) -> Self {
        Value::Entry(Arc::new(e))
    }
}
impl From<LayerStack> for Value {
    fn from(s: LayerStack) -> Self {
        Value::Stack(Arc::new(s))
    }
}
impl From<GenRecipe> for Value {
    fn from(r: GenRecipe) -> Self {
        Value::Recipe(Arc::new(r))
    }
}
impl From<AlphaSource> for Value {
    fn from(a: AlphaSource) -> Self {
        Value::AlphaSource(Arc::new(a))
    }
}
impl From<BuildParams> for Value {
    fn from(b: BuildParams) -> Self {
        Value::Build(b)
    }
}
impl From<FieldReport> for Value {
    fn from(f: FieldReport) -> Self {
        Value::Field(Arc::new(f))
    }
}
impl From<StonesReport> for Value {
    fn from(s: StonesReport) -> Self {
        Value::Stones(Arc::new(s))
    }
}
impl From<Mesh> for Value {
    fn from(m: Mesh) -> Self {
        Value::Mesh(Arc::new(m))
    }
}
impl From<Vec<[f64; 2]>> for Value {
    fn from(p: Vec<[f64; 2]>) -> Self {
        Value::Path(Arc::new(p))
    }
}
impl From<serde_json::Value> for Value {
    fn from(j: serde_json::Value) -> Self {
        Value::Json(Arc::new(j))
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}

/// What an unwired pin holds in a graph file: the literal subset of
/// [`Value`]. Untagged, so a file reads `6.0`, `12`, `"Oval"`, `true`,
/// `[1, 2, 3]` or an object as what they look like.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Null,
    Bool(bool),
    Int(i64),
    Number(f64),
    Text(String),
    List(Vec<Literal>),
    Json(serde_json::Value),
}

impl Literal {
    /// The literal form of a value, or `None` for a handle.
    pub fn of(v: &Value) -> Option<Literal> {
        Some(match v {
            Value::Null => Literal::Null,
            Value::Number(x) => Literal::Number(*x),
            Value::Int(i) => Literal::Int(*i),
            Value::Bool(b) => Literal::Bool(*b),
            Value::Text(s) | Value::AlphaRef(s) => Literal::Text(s.clone()),
            Value::List(items) => Literal::List(items.iter().map(Literal::of).collect::<Option<Vec<_>>>()?),
            Value::Json(j) => Literal::Json((**j).clone()),
            Value::Path(p) => Literal::Json(serde_json::json!(p.as_slice())),
            _ => return None,
        })
    }

    pub fn kind(&self) -> ValueKind {
        match self {
            Literal::Null => ValueKind::Null,
            Literal::Bool(_) => ValueKind::Bool,
            Literal::Int(_) => ValueKind::Int,
            Literal::Number(_) => ValueKind::Number,
            Literal::Text(_) => ValueKind::Text,
            Literal::List(_) => ValueKind::List,
            Literal::Json(_) => ValueKind::Json,
        }
    }
}

impl From<Literal> for Value {
    fn from(l: Literal) -> Self {
        match l {
            Literal::Null => Value::Null,
            Literal::Bool(b) => Value::Bool(b),
            Literal::Int(i) => Value::Int(i),
            Literal::Number(x) => Value::Number(x),
            Literal::Text(s) => Value::Text(s),
            Literal::List(items) => Value::List(items.into_iter().map(Value::from).collect()),
            Literal::Json(j) => Value::Json(Arc::new(j)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringdesign_core::field::MilgrainLayer;

    fn milgrain() -> Layer {
        Layer::Milgrain(MilgrainLayer::default())
    }

    /// One row per arrow in the table, and the arrows that are not there.
    #[test]
    fn the_coercion_table_row_by_row() {
        use ValueKind as K;
        let yes: &[(Value, K, Value)] = &[
            (Value::Int(3), K::Number, Value::Number(3.0)),
            (Value::Bool(true), K::Number, Value::Number(1.0)),
            (Value::Number(2.5), K::Int, Value::Int(3)),
            (Value::Number(2.4), K::Int, Value::Int(2)),
            (Value::Bool(false), K::Int, Value::Int(0)),
            (Value::Number(6.0), K::Text, Value::Text("6".into())),
            (Value::Number(0.125), K::Text, Value::Text("0.125".into())),
            (Value::Int(12), K::Text, Value::Text("12".into())),
            (Value::Bool(true), K::Text, Value::Text("true".into())),
            (Value::AlphaRef("Scales".into()), K::Text, Value::Text("Scales".into())),
            (Value::Text("Scales".into()), K::AlphaRef, Value::AlphaRef("Scales".into())),
            (Value::Json(Arc::new(serde_json::json!([[0.0, 1.0], [2.0, 3.0]]))), K::Path, Value::from(vec![[0.0, 1.0], [2.0, 3.0]])),
            (Value::Int(4), K::Json, Value::Json(Arc::new(serde_json::json!(4)))),
            (Value::from(vec![1.0, 2.0]), K::Json, Value::Json(Arc::new(serde_json::json!([1.0, 2.0])))),
            (Value::Null, K::Design, Value::Null),
        ];
        for (from, to, want) in yes {
            assert!(to.accepts(from.kind()), "{:?} should be wirable into {}", from.kind(), to.label());
            let got = to.coerce(from.clone()).unwrap_or_else(|e| panic!("{from:?} -> {}: {e}", to.label()));
            assert_eq!(&got, want, "{from:?} -> {}", to.label());
        }

        // Layers wrap into entries and stacks, with the kind as the name.
        let layer = Value::from(milgrain());
        match K::Entry.coerce(layer.clone()).unwrap() {
            Value::Entry(e) => assert_eq!(e.name, "Milgrain"),
            other => panic!("{other:?}"),
        }
        match K::Stack.coerce(layer.clone()).unwrap() {
            Value::Stack(s) => assert_eq!(s.layers.len(), 1),
            other => panic!("{other:?}"),
        }
        let entry = K::Entry.coerce(layer).unwrap();
        match K::Stack.coerce(entry).unwrap() {
            Value::Stack(s) => assert_eq!(s.layers[0].name, "Milgrain"),
            other => panic!("{other:?}"),
        }

        // Any takes everything unchanged; same kind is identity.
        let d = Value::from(RingDesign::default());
        assert_eq!(K::Any.coerce(d.clone()).unwrap(), d);
        assert_eq!(K::Design.coerce(d.clone()).unwrap(), d);

        // What is not in the table is refused, at wiring and at runtime.
        let no: &[(Value, K)] = &[
            (Value::Text("6".into()), K::Number),
            (Value::Text("x".into()), K::Int),
            (Value::Number(1.0), K::Bool),
            (d.clone(), K::Layer),
            (Value::from(milgrain()), K::Design),
            (Value::from(vec![1.0]), K::Number),
            (Value::Json(Arc::new(serde_json::json!({"a": 1}))), K::Path),
        ];
        for (from, to) in no {
            if from.kind() != K::Json {
                assert!(!to.accepts(from.kind()), "{:?} must not wire into {}", from.kind(), to.label());
            }
            assert!(to.coerce(from.clone()).is_err(), "{from:?} -> {} must fail", to.label());
        }
        let e = K::Number.coerce(Value::Text("6".into())).unwrap_err();
        assert_eq!(e.to_string(), "cannot take text as number");
    }

    #[test]
    fn literals_round_trip_through_files_and_values() {
        let cases = [
            ("null", Literal::Null),
            ("true", Literal::Bool(true)),
            ("12", Literal::Int(12)),
            ("6.0", Literal::Number(6.0)),
            ("\"Oval\"", Literal::Text("Oval".into())),
            ("[1,2.5,\"x\"]", Literal::List(vec![Literal::Int(1), Literal::Number(2.5), Literal::Text("x".into())])),
            ("{\"a\":1}", Literal::Json(serde_json::json!({"a": 1}))),
        ];
        for (text, want) in cases {
            let got: Literal = serde_json::from_str(text).unwrap();
            assert_eq!(got, want, "{text}");
            let back = serde_json::to_string(&got).unwrap();
            let again: Literal = serde_json::from_str(&back).unwrap();
            assert_eq!(again, want, "{text} via {back}");
            let v = Value::from(got.clone());
            assert_eq!(Literal::of(&v), Some(want), "{text} through Value");
        }
        // Handles have no literal form.
        assert_eq!(Literal::of(&Value::from(RingDesign::default())), None);
        // A path is a literal as its JSON pairs.
        let p = Value::from(vec![[1.0, 2.0]]);
        assert_eq!(Literal::of(&p), Some(Literal::Json(serde_json::json!([[1.0, 2.0]]))));
    }

    #[test]
    fn handles_are_cheap_and_compare_by_identity() {
        let d = Arc::new(RingDesign::default());
        let a = Value::Design(d.clone());
        let b = a.clone();
        assert_eq!(a, b);
        assert_eq!(Arc::strong_count(&d), 3);
        let c = Value::from(RingDesign::default());
        assert_ne!(a, c, "two designs with equal contents are different handles");
        assert_eq!(Value::Number(1.0), Value::Number(1.0));
        assert_ne!(Value::Number(1.0), Value::Int(1), "no cross-kind equality");
        assert_eq!(Value::from(vec![1.0, 2.0]).summary(), "list ×2");
        assert_eq!(format!("{:?}", Value::from(vec![Value::Int(1), Value::Text("a".into())])), "[1, \"a\"]");
    }

    #[test]
    fn every_kind_labels_and_accepts_itself_and_null() {
        for &k in ValueKind::ALL {
            assert!(!k.label().is_empty());
            assert!(k.accepts(k));
            assert!(k.accepts(ValueKind::Null), "{} must let a failed item through", k.label());
            assert!(ValueKind::Any.accepts(k));
        }
    }
}
