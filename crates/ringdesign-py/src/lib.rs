//! RingDesigner from Python.
//!
//! A thin, numpy-free module over the core and the graph runtime: designs
//! as JSON-backed objects, builds that release the GIL, the field verdict,
//! and graphs that evaluate the way the app does. Built with maturin into a
//! venv (`maturin develop --release -m crates/ringdesign-py/Cargo.toml`);
//! abi3 for Python 3.12+, so one wheel serves every interpreter from there.

use std::sync::Arc;

use pyo3::exceptions::{PyIOError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

/// pyo3 0.29 dropped the alias; a Python object is a `Py<PyAny>`.
type PyObject = Py<PyAny>;
use ringdesign_core::alpha::AlphaLibrary;
use ringdesign_core::castability::{self, FieldReport};
use ringdesign_core::mesh::{BuildParams, Report};
use ringdesign_core::refine::RefineParams;
use ringdesign_core::{Mesh, RingDesign, gltf, library, metal, render, stl, stones, threemf};
use ringdesign_graph::eval::{Evaluator, evaluate_design};
use ringdesign_graph::graph::{Graph as CoreGraph, NodeId};
use ringdesign_graph::value::Literal;
use ringdesign_graph::{file, lift, templates as graph_templates};

fn bad(e: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(e.to_string())
}

fn io(e: impl std::fmt::Display) -> PyErr {
    PyIOError::new_err(e.to_string())
}

/// A JSON value as Python objects.
fn json_to_py(py: Python<'_>, v: &serde_json::Value) -> PyResult<PyObject> {
    Ok(match v {
        serde_json::Value::Null => py.None(),
        serde_json::Value::Bool(b) => b.into_pyobject(py)?.to_owned().into_any().unbind(),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py)?.into_any().unbind()
            } else {
                n.as_f64().unwrap_or(f64::NAN).into_pyobject(py)?.into_any().unbind()
            }
        }
        serde_json::Value::String(s) => s.into_pyobject(py)?.into_any().unbind(),
        serde_json::Value::Array(items) => {
            let list = PyList::empty(py);
            for x in items {
                list.append(json_to_py(py, x)?)?;
            }
            list.into_any().unbind()
        }
        serde_json::Value::Object(map) => {
            let dict = PyDict::new(py);
            for (k, x) in map {
                dict.set_item(k, json_to_py(py, x)?)?;
            }
            dict.into_any().unbind()
        }
    })
}

/// Python objects as a JSON value.
fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(serde_json::Value::Bool(b));
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(serde_json::json!(i));
    }
    if let Ok(x) = obj.extract::<f64>() {
        return Ok(serde_json::json!(x));
    }
    if let Ok(s) = obj.extract::<String>() {
        return Ok(serde_json::Value::String(s));
    }
    if let Ok(dict) = obj.cast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            map.insert(k.extract::<String>()?, py_to_json(&v)?);
        }
        return Ok(serde_json::Value::Object(map));
    }
    if let Ok(list) = obj.cast::<PyList>() {
        let mut items = Vec::new();
        for v in list.iter() {
            items.push(py_to_json(&v)?);
        }
        return Ok(serde_json::Value::Array(items));
    }
    if let Ok(items) = obj.extract::<Vec<Bound<'_, PyAny>>>() {
        return Ok(serde_json::Value::Array(items.iter().map(py_to_json).collect::<PyResult<Vec<_>>>()?));
    }
    Err(bad(format!("{} cannot be carried as JSON", obj.get_type().name()?)))
}

fn serialize_py<T: serde::Serialize>(py: Python<'_>, t: &T) -> PyResult<PyObject> {
    json_to_py(py, &serde_json::to_value(t).map_err(bad)?)
}

/// The alpha library: the builtins plus the user's alpha folder.
#[pyclass(name = "Library", from_py_object)]
#[derive(Clone)]
pub struct Library {
    lib: Arc<AlphaLibrary>,
}

#[pymethods]
impl Library {
    /// Builtins and every alpha in the user's folder.
    #[new]
    fn new() -> Self {
        let mut lib = AlphaLibrary::builtin();
        let _ = lib.load_dir(library::user_alpha_dir());
        Self { lib: Arc::new(lib) }
    }

    /// The builtins alone.
    #[staticmethod]
    fn builtin() -> Self {
        Self { lib: Arc::new(AlphaLibrary::builtin()) }
    }

    fn names(&self) -> Vec<String> {
        let mut n = self.lib.names();
        n.sort();
        n
    }

    fn __len__(&self) -> usize {
        self.lib.len()
    }

    fn __repr__(&self) -> String {
        format!("Library({} alphas)", self.lib.len())
    }
}

/// The library a design builds with: the given one (or the builtins), with
/// the design's own sources baked in.
fn lib_for(design: &RingDesign, lib: Option<&Library>) -> AlphaLibrary {
    let mut l = match lib {
        Some(l) => (*l.lib).clone(),
        None => AlphaLibrary::builtin(),
    };
    design.unpack_embedded(&mut l);
    design.bake_all(&mut l);
    l
}

/// A ring design.
#[pyclass(name = "Design", from_py_object)]
#[derive(Clone)]
pub struct Design {
    inner: RingDesign,
}

#[pymethods]
impl Design {
    /// The default band.
    #[new]
    fn new() -> Self {
        Self { inner: RingDesign::default() }
    }

    /// One of the File-menu templates by name.
    #[staticmethod]
    fn template(name: &str) -> PyResult<Self> {
        ringdesign_core::templates::all()
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(name))
            .map(|t| Self { inner: t.design() })
            .ok_or_else(|| bad(format!("no template {name:?}; one of {:?}", templates())))
    }

    /// A `.ring.json` file.
    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        Ok(Self { inner: library::load_design(path).map_err(io)? })
    }

    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        Ok(Self { inner: library::load_design_str(text).map_err(bad)? })
    }

    /// Write the design file; with a library, referenced alphas are embedded.
    #[pyo3(signature = (path, lib = None))]
    fn save(&self, path: &str, lib: Option<&Library>) -> PyResult<()> {
        match lib {
            Some(l) => library::save_design_embedded(path, &self.inner, &l.lib).map_err(io),
            None => library::save_design(path, &self.inner).map_err(io),
        }
    }

    fn to_json(&self) -> PyResult<String> {
        serde_json::to_string_pretty(&self.inner).map_err(bad)
    }

    /// Any field by RFC 6901 pointer, as Python objects.
    fn get(&self, py: Python<'_>, pointer: &str) -> PyResult<PyObject> {
        let json = serde_json::to_value(&self.inner).map_err(bad)?;
        let v = if pointer.is_empty() { Some(&json) } else { json.pointer(pointer) };
        json_to_py(py, v.ok_or_else(|| bad(format!("nothing at {pointer:?}")))?)
    }

    /// Set any existing field by RFC 6901 pointer.
    fn set(&mut self, pointer: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let mut json = serde_json::to_value(&self.inner).map_err(bad)?;
        if pointer.is_empty() || json.pointer(pointer).is_none() {
            return Err(bad(format!("a design has nothing at {pointer:?}")));
        }
        ringdesign_graph::graph::set_pointer(&mut json, pointer, py_to_json(value)?).map_err(bad)?;
        self.inner = serde_json::from_value(json).map_err(|e| bad(format!("the design would not read back: {e}")))?;
        Ok(())
    }

    #[getter]
    fn name(&self) -> String {
        self.inner.name.clone()
    }

    #[setter]
    fn set_name(&mut self, name: String) {
        self.inner.name = name;
    }

    #[getter]
    fn size(&self) -> f64 {
        self.inner.size.0
    }

    #[setter]
    fn set_size(&mut self, size: f64) -> PyResult<()> {
        if !(1.0..=20.0).contains(&size) {
            return Err(bad(format!("{size} is not a US ring size between 1 and 20")));
        }
        self.inner.size = ringdesign_core::sizing::RingSize(size);
        Ok(())
    }

    #[getter]
    fn width_mm(&self) -> f64 {
        self.inner.profile.width_mm
    }

    #[getter]
    fn thickness_mm(&self) -> f64 {
        self.inner.profile.thickness_mm
    }

    #[getter]
    fn inner_diameter_mm(&self) -> f64 {
        self.inner.size.inner_diameter_mm()
    }

    #[getter]
    fn layers(&self) -> Vec<String> {
        self.inner.layers.layers.iter().map(|e| e.name.clone()).collect()
    }

    /// Build the mesh, releasing the GIL. A preset name (Draft, Preview,
    /// Fine, Export, Maximum), explicit steps, or a refinement tolerance.
    #[pyo3(signature = (lib = None, preset = None, theta_steps = None, profile_steps = None, tolerance_mm = None))]
    fn build(&self, py: Python<'_>, lib: Option<&Library>, preset: Option<&str>, theta_steps: Option<usize>, profile_steps: Option<usize>, tolerance_mm: Option<f64>) -> PyResult<Build> {
        let design = self.inner.clone();
        let lib = lib_for(&design, lib);
        let mut params = design.build;
        if let Some(p) = preset {
            let (_, t, s) = BuildParams::PRESETS.iter().find(|(n, _, _)| n.eq_ignore_ascii_case(p)).ok_or_else(|| bad(format!("{p:?} is not a build preset")))?;
            params.theta_steps = *t;
            params.profile_steps = *s;
        }
        if let Some(t) = theta_steps {
            params.theta_steps = t.clamp(16, 8192);
        }
        if let Some(s) = profile_steps {
            params.profile_steps = s.clamp(8, 4096);
        }
        params.refine = None;
        Ok(py.detach(move || match tolerance_mm {
            Some(tol) => {
                let rp = RefineParams { tolerance_mm: tol.clamp(0.002, 1.0), ..RefineParams::preset("Draft").unwrap_or_default() };
                let out = ringdesign_core::refine::build(&design, &lib, rp, design.build.min_wall_mm);
                Build { mesh: out.mesh, report: None }
            }
            None => {
                let out = ringdesign_core::mesh::build(&design, &lib, params);
                Build { mesh: out.mesh, report: Some(out.report) }
            }
        }))
    }

    /// The castability verdict from the true surface, as a dict.
    #[pyo3(signature = (lib = None, theta_steps = 192, profile_steps = 128))]
    fn field_report(&self, py: Python<'_>, lib: Option<&Library>, theta_steps: usize, profile_steps: usize) -> PyResult<PyObject> {
        let design = self.inner.clone();
        let lib = lib_for(&design, lib);
        let f: FieldReport = py.detach(move || castability::attributed_field_report(&design, &lib, &design.draft, theta_steps.clamp(16, 4096), profile_steps.clamp(8, 2048)));
        serialize_py(py, &f)
    }

    /// "Castable", "Castable with care" or "Will not release".
    #[pyo3(signature = (lib = None))]
    fn verdict(&self, py: Python<'_>, lib: Option<&Library>) -> String {
        let design = self.inner.clone();
        let lib = lib_for(&design, lib);
        py.detach(move || castability::attributed_field_report(&design, &lib, &design.draft, 192, 128).verdict.label().to_string())
    }

    /// The cross-section at an angle (90° is the top), as a dict.
    #[pyo3(signature = (theta_deg = 90.0, steps = 128, lib = None))]
    fn section(&self, py: Python<'_>, theta_deg: f64, steps: usize, lib: Option<&Library>) -> PyResult<PyObject> {
        let lib = lib_for(&self.inner, lib);
        let s = castability::section_at(&self.inner, &lib, theta_deg, steps.clamp(8, 4096));
        serialize_py(py, &s)
    }

    /// The bench check for every seat.
    #[pyo3(signature = (lib = None))]
    fn stones(&self, py: Python<'_>, lib: Option<&Library>) -> PyResult<PyObject> {
        let lib = lib_for(&self.inner, lib);
        let parting = castability::attributed_field_report(&self.inner, &lib, &self.inner.draft, 96, 64).parting_z_mm;
        let dict = PyDict::new(py);
        match stones::report(&self.inner, parting) {
            Some(r) => {
                dict.set_item("count", r.stone_count)?;
                dict.set_item("carats", r.total_carats)?;
                dict.set_item("tight_pairs", r.tight_pairs)?;
                dict.set_item("crowding_note", r.crowding_note())?;
                let warnings: Vec<String> = r.seats.iter().flat_map(|s| s.warnings.iter().cloned()).collect();
                dict.set_item("warnings", warnings)?;
            }
            None => {
                dict.set_item("count", 0)?;
                dict.set_item("carats", 0.0)?;
                dict.set_item("tight_pairs", 0)?;
                dict.set_item("crowding_note", py.None())?;
                dict.set_item("warnings", Vec::<String>::new())?;
            }
        }
        Ok(dict.into_any().unbind())
    }

    /// Chvorinov modulus per slice: `(theta_deg, modulus_mm)`, the highest freezes last.
    #[pyo3(signature = (bins = 64, lib = None))]
    fn modulus_scan(&self, py: Python<'_>, bins: usize, lib: Option<&Library>) -> Vec<(f64, f64)> {
        let design = self.inner.clone();
        let lib = lib_for(&design, lib);
        py.detach(move || castability::modulus_scan(&design, &lib, bins))
    }

    /// A software-rendered PNG of a preview build.
    #[pyo3(signature = (path, lib = None, yaw = 0.6, pitch = 1.12, edge = 800, tint = (1.0, 0.78, 0.36)))]
    fn render_png(&self, py: Python<'_>, path: &str, lib: Option<&Library>, yaw: f64, pitch: f64, edge: usize, tint: (f32, f32, f32)) -> PyResult<()> {
        let design = self.inner.clone();
        let lib = lib_for(&design, lib);
        let path = path.to_string();
        py.detach(move || {
            let mut params = design.build;
            params.theta_steps = 384;
            params.profile_steps = 144;
            params.refine = None;
            let out = ringdesign_core::mesh::build(&design, &lib, params);
            render::write_png(&path, &out.mesh, yaw, pitch, edge.clamp(32, 4096), [tint.0, tint.1, tint.2])
        })
        .map_err(io)
    }

    /// The graph behind this design, if it carries one.
    fn graph(&self) -> PyResult<Option<Graph>> {
        match &self.inner.graph {
            Some(j) => Ok(Some(Graph { g: serde_json::from_value(j.clone()).map_err(bad)? })),
            None => Ok(None),
        }
    }

    fn __repr__(&self) -> String {
        format!("Design({:?}, size {}, {:.2} × {:.2} mm, {} layers)", self.inner.name, self.inner.size.0, self.inner.profile.width_mm, self.inner.profile.thickness_mm, self.inner.layers.layers.len())
    }
}

/// A built mesh with its report.
#[pyclass(name = "Build")]
pub struct Build {
    mesh: Mesh,
    report: Option<Report>,
}

#[pymethods]
impl Build {
    fn vertices(&self) -> Vec<(f32, f32, f32)> {
        self.mesh.vertices.iter().map(|v| (v.0, v.1, v.2)).collect()
    }

    fn normals(&self) -> Vec<(f32, f32, f32)> {
        self.mesh.normals.iter().map(|v| (v.0, v.1, v.2)).collect()
    }

    fn faces(&self) -> Vec<(u32, u32, u32)> {
        self.mesh.faces.iter().map(|f| (f[0], f[1], f[2])).collect()
    }

    #[getter]
    fn triangles(&self) -> usize {
        self.mesh.faces.len()
    }

    #[getter]
    fn watertight(&self) -> bool {
        self.mesh.validate().watertight
    }

    #[getter]
    fn volume_mm3(&self) -> f64 {
        self.mesh.volume_mm3()
    }

    #[getter]
    fn surface_area_mm2(&self) -> f64 {
        self.mesh.surface_area_mm2()
    }

    /// The build report as a dict (None for a refined build).
    fn report(&self, py: Python<'_>) -> PyResult<PyObject> {
        match &self.report {
            Some(r) => serialize_py(py, r),
            None => Ok(py.None()),
        }
    }

    /// `(metal, grams, dwt)` for every metal in the table.
    fn weights(&self) -> Vec<(String, f64, f64)> {
        match &self.report {
            Some(r) => r.metals.iter().map(|w| (w.metal.to_string(), w.grams, w.dwt)).collect(),
            None => {
                let v = self.mesh.volume_mm3();
                metal::METALS.iter().map(|m| {
                    let grams = v / 1000.0 * m.density;
                    (m.name.to_string(), grams, grams / 1.555_173_84)
                }).collect()
            }
        }
    }

    /// The mesh scaled up for this metal's shrink: a pattern, not a ring.
    fn pattern_for(&self, metal_name: &str) -> PyResult<Build> {
        let m = metal::find(metal_name).ok_or_else(|| bad(format!("{metal_name:?} is not a metal in the table")))?;
        Ok(Build { mesh: self.mesh.scaled(metal::pattern_scale(m.shrink_pct)), report: None })
    }

    #[pyo3(signature = (path, name = "ring"))]
    fn export_stl(&self, path: &str, name: &str) -> PyResult<usize> {
        stl::write_stl(path, &self.mesh, name).map_err(io)
    }

    #[pyo3(signature = (path, name = "ring"))]
    fn export_obj(&self, path: &str, name: &str) -> PyResult<usize> {
        stl::write_obj(path, &self.mesh, name).map_err(io)
    }

    #[pyo3(signature = (path, name = "ring"))]
    fn export_ply(&self, path: &str, name: &str) -> PyResult<usize> {
        stl::write_ply(path, &self.mesh, name).map_err(io)
    }

    #[pyo3(signature = (path, name = "ring", size_label = ""))]
    fn export_3mf(&self, path: &str, name: &str, size_label: &str) -> PyResult<usize> {
        threemf::write_3mf(path, &self.mesh, name, size_label).map_err(io)
    }

    #[pyo3(signature = (path, name = "ring", tint = (1.0, 0.78, 0.36)))]
    fn export_glb(&self, path: &str, name: &str, tint: (f32, f32, f32)) -> PyResult<usize> {
        gltf::write_glb(path, &self.mesh, name, [tint.0, tint.1, tint.2]).map_err(io)
    }

    fn __repr__(&self) -> String {
        format!("Build({} triangles, {:.2} mm³)", self.mesh.faces.len(), self.mesh.volume_mm3())
    }
}

fn registry() -> &'static ringdesign_graph::registry::Registry {
    static REG: std::sync::OnceLock<ringdesign_graph::registry::Registry> = std::sync::OnceLock::new();
    REG.get_or_init(ringdesign_script::registry)
}

/// A dataflow graph that evaluates to a design.
#[pyclass(name = "Graph", from_py_object)]
#[derive(Clone)]
pub struct Graph {
    g: CoreGraph,
}

#[pymethods]
impl Graph {
    #[new]
    #[pyo3(signature = (name = "Untitled"))]
    fn new(name: &str) -> Self {
        Self { g: CoreGraph::new(name, ringdesign_graph::graph::Mode::SandRing) }
    }

    #[staticmethod]
    fn load(path: &str) -> PyResult<Self> {
        Ok(Self { g: file::load_graph(path, Some(registry())).map_err(io)? })
    }

    #[staticmethod]
    fn from_json(text: &str) -> PyResult<Self> {
        Ok(Self { g: file::load_graph_str(text, Some(registry())).map_err(bad)? })
    }

    /// A bundled template graph by name, or "Simple".
    #[staticmethod]
    fn template(name: &str) -> PyResult<Self> {
        if name.eq_ignore_ascii_case("simple") {
            return Ok(Self { g: graph_templates::simple() });
        }
        graph_templates::graph(name).map(|g| Self { g }).ok_or_else(|| bad(format!("no template graph {name:?}")))
    }

    /// Lift a design into a graph that evaluates back to it exactly.
    #[staticmethod]
    #[pyo3(signature = (design, lib = None))]
    fn from_design(design: &Design, lib: Option<&Library>) -> PyResult<Self> {
        let lib = lib_for(&design.inner, lib);
        Ok(Self { g: lift::from_design(&design.inner, registry(), &lib).map_err(bad)? })
    }

    fn to_json(&self) -> PyResult<String> {
        file::graph_to_string(&self.g).map_err(bad)
    }

    fn save(&self, path: &str) -> PyResult<()> {
        file::save_graph(path, &self.g).map_err(io)
    }

    #[getter]
    fn name(&self) -> String {
        self.g.name.clone()
    }

    /// `(id, kind, label)` for every node.
    fn nodes(&self) -> Vec<(u64, String, Option<String>)> {
        self.g.nodes.iter().map(|n| (n.id.0, n.kind.clone(), n.label.clone())).collect()
    }

    /// `(from, out, to, input)` for every wire.
    fn wires(&self) -> Vec<(u64, String, u64, String)> {
        self.g.wires.iter().map(|w| (w.from.0, w.out.clone(), w.to.0, w.input.clone())).collect()
    }

    /// The exposed parameter names.
    fn exposed(&self) -> Vec<String> {
        self.g.exposed.iter().map(|e| e.name.clone()).collect()
    }

    /// Set an exposed parameter by its name.
    fn set(&mut self, name: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let e = self.g.exposed.iter().find(|e| e.name == name).cloned().ok_or_else(|| bad(format!("{name:?} is not exposed; the graph exposes {:?}", self.exposed())))?;
        let lit: Literal = serde_json::from_value(py_to_json(value)?).map_err(bad)?;
        self.g.set_input(e.node, e.input, lit).map_err(bad)
    }

    /// Set a literal on any node's input; a dict `{"expr": "..."}` is an expression.
    fn set_input(&mut self, id: u64, input: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        let lit: Literal = serde_json::from_value(py_to_json(value)?).map_err(bad)?;
        self.g.set_input(NodeId(id), input, lit).map_err(bad)
    }

    fn set_param(&mut self, id: u64, pointer: &str, value: &Bound<'_, PyAny>) -> PyResult<()> {
        self.g.set_param(NodeId(id), pointer, py_to_json(value)?).map_err(bad)
    }

    /// Add a node of a kind, optionally with literals; returns its id.
    #[pyo3(signature = (kind, inputs = None))]
    fn add_node(&mut self, kind: &str, inputs: Option<&Bound<'_, PyDict>>) -> PyResult<u64> {
        if registry().get(kind).is_none() {
            return Err(bad(format!("no node kind {kind:?}; see node_specs()")));
        }
        let id = self.g.add(kind).map_err(bad)?;
        if let Some(d) = inputs {
            for (k, v) in d.iter() {
                let lit: Literal = serde_json::from_value(py_to_json(&v)?).map_err(bad)?;
                self.g.set_input(id, k.extract::<String>()?, lit).map_err(bad)?;
            }
        }
        Ok(id.0)
    }

    fn connect(&mut self, from: u64, out: &str, to: u64, input: &str) -> PyResult<()> {
        self.g.connect(NodeId(from), out, NodeId(to), input).map(|_| ()).map_err(bad)
    }

    fn remove(&mut self, id: u64) -> PyResult<()> {
        self.g.remove(NodeId(id)).map(|_| ()).map_err(bad)
    }

    fn expose(&mut self, id: u64, input: &str, name: &str) -> PyResult<()> {
        self.g.expose(NodeId(id), input, name).map_err(bad)
    }

    /// Validation errors; empty means evaluable.
    fn errors(&self) -> Vec<String> {
        self.g.validate(Some(registry())).iter().map(ToString::to_string).collect()
    }

    /// Evaluate to `(design, field)` — the design with its verdict dict — releasing the GIL.
    #[pyo3(signature = (lib = None))]
    fn evaluate(&self, py: Python<'_>, lib: Option<&Library>) -> PyResult<(Design, PyObject)> {
        let g = self.g.clone();
        let lib = match lib {
            Some(l) => (*l.lib).clone(),
            None => AlphaLibrary::builtin(),
        };
        let out = py.detach(move || {
            let mut ev = Evaluator::with_exprs(ringdesign_script::engine());
            evaluate_design(&mut ev, &g, registry(), &lib, 0).map(|o| ((*o.design).clone(), o.field, o.notes))
        });
        let (mut design, field, notes) = out.map_err(bad)?;
        if !notes.is_empty() {
            return Err(bad(notes.join("; ")));
        }
        design.graph = Some(serde_json::to_value(&self.g).map_err(bad)?);
        Ok((Design { inner: design }, serialize_py(py, &field)?))
    }

    fn __repr__(&self) -> String {
        format!("Graph({:?}, {} nodes, {} wires)", self.g.name, self.g.nodes.len(), self.g.wires.len())
    }
}

/// The crate version.
#[pyfunction]
fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// The File-menu template names.
#[pyfunction]
fn templates() -> Vec<String> {
    ringdesign_core::templates::all().iter().map(|t| t.name.to_string()).collect()
}

/// Every node kind the graph runtime knows, as dicts.
#[pyfunction]
fn node_specs(py: Python<'_>) -> PyResult<PyObject> {
    let reg = registry();
    let list = PyList::empty(py);
    for spec in reg.list(ringdesign_graph::graph::Mode::Free) {
        let d = PyDict::new(py);
        d.set_item("key", &spec.key)?;
        d.set_item("label", &spec.label)?;
        d.set_item("category", spec.category.label())?;
        d.set_item("doc", &spec.doc)?;
        d.set_item("side_effect", spec.side_effect)?;
        let pins = |pins: &[ringdesign_graph::registry::PinSpec]| -> PyResult<PyObject> {
            let l = PyList::empty(py);
            for p in pins {
                let pd = PyDict::new(py);
                pd.set_item("name", &p.name)?;
                pd.set_item("kind", p.kind.label())?;
                pd.set_item("list", p.access == ringdesign_graph::graph::Access::List)?;
                pd.set_item("doc", &p.doc)?;
                pd.set_item("default", match &p.default { Some(l) => json_to_py(py, &serde_json::to_value(l).map_err(bad)?)?, None => py.None() })?;
                l.append(pd)?;
            }
            Ok(l.into_any().unbind())
        };
        d.set_item("inputs", pins(&spec.inputs)?)?;
        d.set_item("outputs", pins(&spec.outputs)?)?;
        list.append(d)?;
    }
    Ok(list.into_any().unbind())
}

#[pymodule]
fn ringdesign(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(templates, m)?)?;
    m.add_function(wrap_pyfunction!(node_specs, m)?)?;
    m.add_class::<Library>()?;
    m.add_class::<Design>()?;
    m.add_class::<Build>()?;
    m.add_class::<Graph>()?;
    Ok(())
}
