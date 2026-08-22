# ringdesign for Python

RingDesigner's core and graph runtime as a Python module: designs, builds
that release the GIL, the sand-casting verdict, and graphs that evaluate
exactly as the desktop app does. No numpy; everything crosses as plain
Python values.

## Install

```bash
# once, into the repo's venv (Python 3.12+; abi3 wheels)
VIRTUAL_ENV=$PWD/tools/venv tools/venv/bin/maturin develop --release -m crates/ringdesign-py/Cargo.toml
tools/venv/bin/python -m pytest crates/ringdesign-py/tests -q
```

## A size run

```python
import ringdesign as rd

base = rd.Design.template("Braided band")
for size in (5.0, 5.5, 6.0, 6.5, 7.0):
    d = rd.Design.from_json(base.to_json())
    d.size = size
    build = d.build(preset="Export")
    print(size, d.verdict(), f"{build.volume_mm3:.1f} mm³", build.weights()[0])
    build.pattern_for("Silver 925").export_stl(f"braided-{size}.stl")
```

## A graph

```python
g = rd.Graph.template("Braided band")      # or rd.Graph.from_design(d) to lift any design
g.set("Beads around", 96)                   # an exposed parameter
design, field = g.evaluate()                # the design with its verdict dict
print(field["verdict"], design.layers)
```

`node_specs()` lists every node kind with its pins; `Graph.add_node`,
`connect`, `set_input` (a dict `{"expr": "width_mm / 3"}` is an
expression) and `expose` build one from scratch.

## Probes

The harvest tools read meshes the module writes: `Design.build().export_obj`
feeds `tools/harvest/deviation.py` and the dihedral census, so a rebuilt
preset can be measured against a reference without the Rust examples
(`tools/harvest/py_build.py`).
