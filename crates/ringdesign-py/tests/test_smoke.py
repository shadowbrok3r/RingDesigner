"""The module end to end: templates, builds, files, verdicts, graphs.

Run with the venv the module was developed into:
    VIRTUAL_ENV=$PWD/tools/venv tools/venv/bin/maturin develop --release -m crates/ringdesign-py/Cargo.toml
    tools/venv/bin/python -m pytest crates/ringdesign-py/tests -q
"""

import json
import os
import tempfile

import pytest

import ringdesign as rd


def strip_graph(design):
    d = json.loads(design.to_json())
    d.pop("graph", None)
    return json.dumps(d, sort_keys=True)


def test_templates_and_version():
    names = rd.templates()
    assert "Court band" in names and "Heart signet" in names and len(names) == 9
    assert rd.version()


def test_default_build_is_watertight_and_exports():
    d = rd.Design()
    b = d.build(preset="Draft")
    assert b.watertight
    assert b.triangles == 192 * 96 * 2
    assert b.volume_mm3 > 50
    assert len(b.vertices()) == len(b.normals())
    assert all(len(f) == 3 for f in b.faces()[:5])
    metals = dict((m, g) for m, g, _ in b.weights())
    assert metals["Silver 925"] > 0
    with tempfile.TemporaryDirectory() as t:
        p = os.path.join(t, "ring.stl")
        assert b.export_stl(p) > 84 and os.path.getsize(p) > 84
        for ext, fn in [("obj", b.export_obj), ("ply", b.export_ply), ("3mf", b.export_3mf), ("glb", b.export_glb)]:
            q = os.path.join(t, f"ring.{ext}")
            assert fn(q) > 0 and os.path.getsize(q) > 0
    pattern = b.pattern_for("Silver 925")
    assert pattern.volume_mm3 > b.volume_mm3


def test_refined_build_is_watertight():
    b = rd.Design.template("Court band").build(tolerance_mm=0.08)
    assert b.watertight and b.triangles > 1000
    assert b.report() is None


def test_json_round_trip_and_pointers():
    d = rd.Design.template("Braided band")
    text = d.to_json()
    again = rd.Design.from_json(text)
    assert again.to_json() == text
    assert d.get("/profile/width_mm") == 7.5
    d.set("/profile/width_mm", 8.0)
    assert d.width_mm == 8.0
    with pytest.raises(ValueError):
        d.set("/profile/nope", 1)
    assert d.layers == ["Braid", "Milgrain"]
    with tempfile.TemporaryDirectory() as t:
        p = os.path.join(t, "b.ring.json")
        d.save(p, rd.Library.builtin())
        assert rd.Design.load(p).width_mm == 8.0


def test_verdict_and_reports():
    d = rd.Design.template("Court band")
    assert d.verdict() == "Castable"
    f = d.field_report()
    assert f["verdict"] == "Castable" and f["thinnest_wall_mm"] > 1.0
    s = d.section(90.0, 64)
    assert len(s["points"]) == 64
    scan = d.modulus_scan(16)
    assert len(scan) == 16
    stones = rd.Design.template("Cathedral solitaire stock").stones()
    assert stones["count"] == 1 and stones["carats"] > 0


def test_graph_evaluate_of_the_lift_equals_the_design():
    for name in rd.templates():
        d = rd.Design.template(name)
        g = rd.Graph.from_design(d)
        assert g.errors() == []
        out, field = g.evaluate()
        assert strip_graph(out) == strip_graph(d), name
        assert field["verdict"] in ("Castable", "Marginal")
        assert out.graph() is not None


def test_a_graph_built_from_scratch_equals_the_template():
    g = rd.Graph("Court band")
    p = g.add_node("band.profile", {"style": "LowDome", "width_mm": 4.0, "thickness_mm": 2.0})
    d = g.add_node("design.new", {"name": "Court band"})
    o = g.add_node("sink.output")
    g.connect(p, "profile", d, "profile")
    g.connect(d, "design", o, "design")
    g.expose(p, "width_mm", "Width")
    assert g.exposed() == ["Width"]
    out, _ = g.evaluate()
    assert strip_graph(out) == strip_graph(rd.Design.template("Court band"))
    g.set("Width", 6.0)
    out, _ = g.evaluate()
    assert out.width_mm == 6.0
    g.set_input(p, "thickness_mm", {"expr": "width_mm / 3.0"})
    out, _ = g.evaluate()
    assert abs(out.thickness_mm - 2.0) < 1e-9
    with pytest.raises(ValueError):
        g.set("Nope", 1)
    with pytest.raises(ValueError):
        g.add_node("no.such")


def test_template_graphs_and_node_specs():
    g = rd.Graph.template("Braided band")
    assert len(g.nodes()) == 13 and g.exposed() == ["Beads around"]
    g.set("Beads around", 96)
    out, _ = g.evaluate()
    assert json.loads(out.to_json())["layers"]["layers"][1]["layer"]["Milgrain"]["beads_around"] == 96
    specs = rd.node_specs()
    keys = {s["key"] for s in specs}
    assert {"band.profile", "sink.output", "gen.pave", "script", "cluster"} <= keys
    assert any(p["name"] == "width_mm" for s in specs if s["key"] == "band.profile" for p in s["inputs"])
