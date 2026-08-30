//! Existing serde structs as nodes.
//!
//! A [`StructNode`] is a patch over a base: the node starts from the
//! handle on its base pin (or `T::default()`), writes every *set* field pin
//! into the struct's JSON at that field's pointer, reads the struct back,
//! runs an optional finish hook, and wraps the result. An unset pin leaves
//! the base's field alone — which is what makes a modifier node safe to
//! put after another node of the same kind. Pins are written through
//! serde, so an enum pin is a text pin carrying the variant's serde name
//! and a handle pin carries a whole nested struct.
//!
//! [`StructNode::coverage`] compares the struct's serialized keys with the
//! pins and the declared `hidden` list, so a field added to the core
//! cannot go unnoticed by its node.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::graph::set_pointer;
use crate::registry::{EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec};
use crate::value::{Value, ValueKind};

/// Runs after the patch, with the inputs, for what JSON cannot express —
/// `apply_style`, `fit_length_to`, a clamp.
pub type FinishFn<T> = fn(&mut T, &Inputs, &mut EvalCtx<'_>) -> Result<(), NodeError>;

struct FieldPin {
    pin: String,
    path: String,
}

/// A node built over a serde struct `T`.
pub struct StructNode<T> {
    spec: NodeSpec,
    fields: Vec<FieldPin>,
    hidden: Vec<String>,
    base: Option<(String, ValueKind)>,
    out: String,
    wrap: fn(T) -> Value,
    unwrap: fn(&Value) -> Option<T>,
    default_base: fn() -> T,
    prepare: Option<FinishFn<T>>,
    finish: Option<FinishFn<T>>,
}

/// Every coverage failure seen while building node specs this process.
///
/// A `Vec` behind a `Mutex` rather than a counter, because what a test needs
/// is the message: which node, and which field it forgot or invented.
pub fn coverage_failures() -> &'static std::sync::Mutex<Vec<String>> {
    static LOG: std::sync::OnceLock<std::sync::Mutex<Vec<String>>> = std::sync::OnceLock::new();
    LOG.get_or_init(Default::default)
}

impl<T> StructNode<T>
where
    T: Serialize + DeserializeOwned + Clone + Send + Sync + 'static,
{
    /// `out` is the output pin's name; `wrap` and `unwrap` move `T` in and
    /// out of a [`Value`]; `base` is what the node starts from when no base
    /// pin is wired (`T::default`, or a new signet's lofted head).
    pub fn new(spec: NodeSpec, out: impl Into<String>, base: fn() -> T, wrap: fn(T) -> Value, unwrap: fn(&Value) -> Option<T>) -> Self {
        Self { spec, fields: Vec::new(), hidden: Vec::new(), base: None, out: out.into(), wrap, unwrap, default_base: base, prepare: None, finish: None }
    }

    /// An optional input of the struct's own kind to start from.
    pub fn base(mut self, pin: impl Into<String>, kind: ValueKind, doc: impl Into<String>) -> Self {
        let pin = pin.into();
        self.spec = self.spec.input(PinSpec::item(pin.clone(), kind).doc(doc).optional());
        self.base = Some((pin, kind));
        self
    }

    /// A pin written at `/<name>`.
    pub fn field(self, pin: PinSpec) -> Self {
        let path = format!("/{}", pin.name);
        self.field_at(pin, path)
    }

    /// A pin written at a JSON pointer inside the struct (`/head/length_mm`).
    pub fn field_at(mut self, pin: PinSpec, path: impl Into<String>) -> Self {
        self.fields.push(FieldPin { pin: pin.name.clone(), path: path.into() });
        self.spec = self.spec.input(pin.optional());
        self
    }

    /// Fields deliberately not exposed as pins.
    pub fn hidden(mut self, names: &[&str]) -> Self {
        self.hidden.extend(names.iter().map(|s| s.to_string()));
        self
    }

    /// Runs on the base before the field pins are written, so a pin the
    /// hook reads (a style preset) cannot clobber pins set explicitly.
    pub fn prepare(mut self, f: FinishFn<T>) -> Self {
        self.prepare = Some(f);
        self
    }

    pub fn finish(mut self, f: FinishFn<T>) -> Self {
        self.finish = Some(f);
        self
    }

    /// An input that is not a field: read by the hooks, ignored by the
    /// patch and the coverage check.
    pub fn extra(mut self, pin: PinSpec) -> Self {
        self.spec = self.spec.input(pin.optional());
        self
    }

    /// Every serialized field of the base is a pin or hidden, and every
    /// pin or hidden name is a field.
    pub fn coverage(&self) -> Result<(), String> {
        let json = serde_json::to_value((self.default_base)()).map_err(|e| format!("{}: {e}", self.spec.key))?;
        let Some(obj) = json.as_object() else { return Err(format!("{}: the struct does not serialize as an object", self.spec.key)) };
        let keys: BTreeSet<&str> = obj.keys().map(String::as_str).collect();
        let mut named: BTreeSet<&str> = self.hidden.iter().map(String::as_str).collect();
        for f in &self.fields {
            let first = f.path.trim_start_matches('/').split('/').next().unwrap_or("");
            named.insert(first);
        }
        let missing: Vec<&str> = keys.difference(&named).copied().collect();
        let extra: Vec<&str> = named.difference(&keys).copied().collect();
        if missing.is_empty() && extra.is_empty() {
            Ok(())
        } else {
            let mut msg = format!("{}:", self.spec.key);
            if !missing.is_empty() {
                msg.push_str(&format!(" fields with no pin and not hidden: {missing:?};"));
            }
            if !extra.is_empty() {
                msg.push_str(&format!(" pins or hidden names that are not fields: {extra:?};"));
            }
            Err(msg)
        }
    }

    /// The node spec.
    ///
    /// The coverage check runs here, and it is the only place it runs — the
    /// registry stores `NodeSpec`s, so by the time anything can walk it the
    /// `StructNode` is gone. It used to be a bare `debug_assert!`, which meant
    /// two things: a release build compiled it out entirely and registered a
    /// node with a missing pin *silently*, leaving the field unreachable from
    /// the graph with nothing said; and the test named for it,
    /// `coverage_names_what_a_node_forgot_or_invented`, only ever exercised
    /// the helper on synthetic nodes, so it passed happily while a real one
    /// was broken. That is how `crest_round_mm` got in.
    ///
    /// Now every failure is recorded where a test can find it, whatever the
    /// build profile, and the debug assertion stays as the fast local signal.
    pub fn build(mut self) -> NodeSpec {
        if let Err(e) = self.coverage() {
            coverage_failures().lock().expect("coverage log").push(e.clone());
            debug_assert!(false, "{e}");
            log::error!("{e}");
        }
        // A modifier must not overwrite its base with pin defaults.
        if self.base.is_some() {
            let fields: BTreeSet<&str> = self.fields.iter().map(|f| f.pin.as_str()).collect();
            for p in &mut self.spec.inputs {
                if fields.contains(p.name.as_str()) {
                    p.default = None;
                }
            }
        }
        let out_kind = self.out_kind();
        self.spec = self.spec.output(PinSpec::item(self.out.clone(), out_kind).doc("The result."));
        let fields: Arc<Vec<FieldPin>> = Arc::new(self.fields);
        let base = self.base.clone();
        let out = self.out.clone();
        let key = self.spec.key.clone();
        let (wrap, unwrap, prepare, finish, default_base) = (self.wrap, self.unwrap, self.prepare, self.finish, self.default_base);
        self.spec.eval(move |ctx, _node, inputs| {
            let mut value: T = match &base {
                Some((pin, kind)) => {
                    let v = inputs.get(pin);
                    if v.is_null() {
                        default_base()
                    } else {
                        unwrap(v).ok_or_else(|| NodeError::input(pin, format!("expected {}, got {}", kind.label(), v.summary())))?
                    }
                }
                None => default_base(),
            };
            if let Some(f) = prepare {
                f(&mut value, inputs, ctx)?;
            }
            let mut json = serde_json::to_value(&value).map_err(|e| NodeError::new(format!("{key}: {e}")))?;
            let mut touched = false;
            for f in fields.iter() {
                let v = inputs.get(&f.pin);
                if v.is_null() {
                    continue;
                }
                let jv = v.to_json_any().ok_or_else(|| NodeError::input(&f.pin, format!("{} cannot be written into a field", v.kind().label())))?;
                set_pointer(&mut json, &f.path, jv).map_err(|m| NodeError::input(&f.pin, m))?;
                touched = true;
            }
            if touched {
                value = serde_json::from_value(json).map_err(|e| NodeError::new(format!("{key}: {e}")))?;
            }
            if let Some(f) = finish {
                f(&mut value, inputs, ctx)?;
            }
            Ok(Outputs::one(out.clone(), wrap(value)))
        })
    }

    fn out_kind(&self) -> ValueKind {
        (self.wrap)((self.default_base)()).kind()
    }
}

/// The serde names of an enum's variants, for a select pin.
pub fn enum_names<E: Serialize>(all: &[E]) -> Vec<String> {
    all.iter().filter_map(|e| serde_json::to_value(e).ok()).filter_map(|v| v.as_str().map(str::to_string)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{Evaluator, Targets};
    use crate::graph::Graph;
    use crate::registry::{Category, Registry, Widget};
    use crate::value::Literal;
    use ringdesign_core::profile::ShankKind;
    use ringdesign_core::{AlphaLibrary, BandProfile, ShankStyle};

    fn profile_node() -> StructNode<BandProfile> {
        StructNode::new(
            NodeSpec::new("test.profile", "Profile", Category::Band).doc("A band section."),
            "profile",
            BandProfile::default,
            Value::Profile,
            |v| match v {
                Value::Profile(p) => Some(*p),
                _ => None,
            },
        )
        .base("profile", ValueKind::Profile, "Start from this section.")
        .field(PinSpec::item("width_mm", ValueKind::Number).doc("Width.").widget(Widget::Mm { min: 1.0, max: 20.0 }))
        .field(PinSpec::item("thickness_mm", ValueKind::Number).doc("Thickness."))
        .field(PinSpec::item("crown_mm", ValueKind::Number).doc("Crown."))
        .field(PinSpec::item("shape_a", ValueKind::Number).doc("a."))
        .field(PinSpec::item("shape_b", ValueKind::Number).doc("b."))
        .field(PinSpec::item("crest_bias", ValueKind::Number).doc("Bias."))
        .field(PinSpec::item("edge_round_mm", ValueKind::Number).doc("Edge."))
        .field(PinSpec::item("comfort_fit_mm", ValueKind::Number).doc("Comfort."))
        .field(PinSpec::item("side_draft_deg", ValueKind::Number).doc("Draft."))
        .field(PinSpec::select("style", vec!["HalfRound".into(), "Flat".into()]).doc("Family."))
        .hidden(&["flange", "drop_curve", "morph"])
    }

    fn shank_node() -> StructNode<ShankStyle> {
        let keys: Vec<String> = serde_json::to_value(ShankStyle::default())
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .filter(|k| *k != "kind" && *k != "amount")
            .cloned()
            .collect();
        let hidden: Vec<&str> = keys.iter().map(String::as_str).collect();
        StructNode::new(
            NodeSpec::new("test.shank", "Shank", Category::Shank).doc("A shank."),
            "shank",
            ShankStyle::default,
            |s| Value::Shank(Arc::new(s)),
            |v| match v {
                Value::Shank(s) => Some((**s).clone()),
                _ => None,
            },
        )
        .field(PinSpec::select("kind", enum_names(ShankKind::ALL)).doc("Kind."))
        .field_at(PinSpec::item("amount", ValueKind::Number).doc("Amount."), "/amount")
        .hidden(&hidden)
    }

    fn reg() -> Registry {
        let mut reg = Registry::empty();
        reg.register(profile_node().build()).unwrap();
        reg.register(shank_node().build()).unwrap();
        reg
    }

    fn run(g: &Graph, reg: &Registry) -> crate::eval::EvalReport {
        Evaluator::new().evaluate(g, reg, &AlphaLibrary::default(), 0, Targets::AllPure)
    }

    #[test]
    fn unset_pins_leave_the_base_alone_and_set_pins_patch_it() {
        let reg = reg();
        let mut g = Graph::default();
        let a = g.add("test.profile").unwrap();
        let r = run(&g, &reg);
        let Some(Value::Profile(p)) = r.value(a, "profile") else { panic!("{:?}", r.value(a, "profile")) };
        let d = BandProfile::default();
        assert_eq!((p.width_mm, p.thickness_mm), (d.width_mm, d.thickness_mm), "nothing set: the default");

        g.set_input(a, "width_mm", Literal::Number(8.0)).unwrap();
        let r = run(&g, &reg);
        let Some(Value::Profile(p)) = r.value(a, "profile") else { panic!() };
        assert_eq!(p.width_mm, 8.0);
        assert_eq!(p.thickness_mm, d.thickness_mm, "an unset pin leaves its field");

        // A second node patches the first's result, keeping the width.
        let b = g.add("test.profile").unwrap();
        g.connect(a, "profile", b, "profile").unwrap();
        g.set_input(b, "thickness_mm", Literal::Number(2.5)).unwrap();
        g.set_input(b, "style", Literal::Text("Flat".into())).unwrap();
        let r = run(&g, &reg);
        let Some(Value::Profile(p)) = r.value(b, "profile") else { panic!() };
        assert_eq!((p.width_mm, p.thickness_mm), (8.0, 2.5));
        assert_eq!(p.style, ringdesign_core::ProfileStyle::Flat, "an enum pin is its serde name");

        // Lists patch per item.
        g.set_input(a, "width_mm", Literal::List(vec![Literal::Number(5.0), Literal::Number(7.0)])).unwrap();
        let r = run(&g, &reg);
        match r.value(b, "profile") {
            Some(Value::List(items)) => {
                let widths: Vec<f64> = items.iter().map(|v| if let Value::Profile(p) = v { p.width_mm } else { f64::NAN }).collect();
                assert_eq!(widths, vec![5.0, 7.0]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn bad_values_fail_by_name() {
        let reg = reg();
        let mut g = Graph::default();
        let a = g.add("test.profile").unwrap();
        g.set_input(a, "style", Literal::Text("Octagon".into())).unwrap();
        let r = run(&g, &reg);
        let msg = &r.status[&a].errors[0].1;
        assert!(msg.contains("unknown variant") && msg.contains("Octagon"), "{msg}");
        // The base pin refuses a value of another kind.
        let s = g.add("test.shank").unwrap();
        let b = g.add("test.profile").unwrap();
        g.wires.push(crate::graph::Wire { from: s, out: "shank".into(), to: b, input: "profile".into() });
        let errs = g.validate(Some(&reg));
        assert!(errs.iter().any(|e| e.message.contains("takes profile, but")), "{errs:?}");
    }

    #[test]
    fn enum_pins_read_serde_names_and_ints_write_whole_numbers() {
        let reg = reg();
        let names = enum_names(ShankKind::ALL);
        assert!(names.contains(&"Signet".to_string()) && names.contains(&"Uniform".to_string()), "{names:?}");
        let mut g = Graph::default();
        let s = g.add("test.shank").unwrap();
        g.set_input(s, "kind", Literal::Text("Signet".into())).unwrap();
        g.set_input(s, "amount", Literal::Int(1)).unwrap();
        let r = run(&g, &reg);
        let Some(Value::Shank(sh)) = r.value(s, "shank") else { panic!("{:?}", r.status[&s]) };
        assert_eq!(sh.kind, ShankKind::Signet);
        assert_eq!(sh.amount, 1.0);
        let spec = reg.get("test.shank").unwrap();
        assert!(matches!(&spec.inputs[0].widget, Widget::Select(v) if v.len() == ShankKind::ALL.len()));
        assert!(spec.inputs.iter().all(|p| p.optional), "struct pins are optional");
        assert_eq!(spec.outputs[0].kind, ValueKind::Shank);
    }

    #[test]
    fn coverage_names_what_a_node_forgot_or_invented() {
        assert!(profile_node().coverage().is_ok());
        let missing = StructNode::<BandProfile>::new(
            NodeSpec::new("test.thin", "Thin", Category::Band),
            "profile",
            BandProfile::default,
            Value::Profile,
            |_| None,
        )
        .field(PinSpec::item("width_mm", ValueKind::Number))
        .hidden(&["flange", "drop_curve", "morph", "bogus"])
        .coverage()
        .unwrap_err();
        assert!(missing.contains("\"thickness_mm\"") && missing.contains("no pin"), "{missing}");
        assert!(missing.contains("\"bogus\"") && missing.contains("not fields"), "{missing}");
        // A pin on a nested path counts its first segment.
        let nested = StructNode::<ShankStyle>::new(NodeSpec::new("test.n", "N", Category::Shank), "shank", ShankStyle::default, |s| Value::Shank(Arc::new(s)), |_| None)
            .field_at(PinSpec::item("head_length", ValueKind::Number), "/head/length_mm");
        let err = nested.coverage().unwrap_err();
        assert!(!err.contains("\"head\""), "head is covered by the nested pin: {err}");
    }
}
