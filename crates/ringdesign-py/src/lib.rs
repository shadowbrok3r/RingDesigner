//! RingDesigner from Python.
//!
//! A thin, numpy-free module over the core and the graph runtime: designs
//! as JSON-backed objects, builds that release the GIL, the field verdict,
//! and graphs that evaluate the way the app does. Built with maturin into a
//! venv (`maturin develop --release -m crates/ringdesign-py/Cargo.toml`);
//! abi3 for Python 3.12+, so one wheel serves every interpreter from there.

use pyo3::prelude::*;

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

#[pymodule]
fn ringdesign(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(version, m)?)?;
    m.add_function(wrap_pyfunction!(templates, m)?)?;
    Ok(())
}
