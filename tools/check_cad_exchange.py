#!/usr/bin/env python3
"""Independent exchange regression; test dependencies only (not app runtime).

python -m venv /tmp/cad-check
/tmp/cad-check/bin/pip install cadquery-ocp ezdxf
cargo run -p ringdesign-core --example cad_exchange_probe -- /tmp/cad-probe
/tmp/cad-check/bin/python tools/check_cad_exchange.py /tmp/cad-probe
"""
import json
from pathlib import Path
import sys

import ezdxf
from ezdxf import bbox
from OCP.STEPControl import STEPControl_Reader
from OCP.IFSelect import IFSelect_RetDone
from OCP.BRepCheck import BRepCheck_Analyzer
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
from OCP.TopAbs import TopAbs_SOLID
from OCP.TopExp import TopExp_Explorer

root = Path(sys.argv[1])
results = []
for name, expected in json.loads((root / "expected.json").read_text()).items():
    reader = STEPControl_Reader()
    assert reader.ReadFile(str(root / f"{name}.step")) == IFSelect_RetDone, name
    assert reader.TransferRoots() > 0, name
    shape = reader.OneShape()
    assert BRepCheck_Analyzer(shape).IsValid(), f"Invalid STEP solid: {name}"
    properties = GProp_GProps()
    BRepGProp.VolumeProperties_s(shape, properties)
    volume = properties.Mass()
    explorer = TopExp_Explorer(shape, TopAbs_SOLID)
    solids = 0
    while explorer.More():
        solids += 1
        explorer.Next()
    assert solids == expected["solid_components"], (name, solids, expected)
    relative_error = abs(volume - expected["mesh_volume_mm3"]) / volume
    assert relative_error < 0.006, (name, volume, expected)
    results.append(dict(name=name, valid=True, solids=solids,
                        step_volume_mm3=volume, mesh_relative_error=relative_error))

for name, width, height in [("rectangle", 8.0, 6.0), ("circle", 6.0, 6.0)]:
    doc = ezdxf.readfile(root / f"{name}.dxf")
    assert not doc.audit().has_errors, f"DXF audit failed: {name}"
    assert doc.units == 4, f"DXF units must be millimeters: {name}"
    bounds = bbox.extents(doc.modelspace())
    assert abs(bounds.size.x - width) < 1e-6, (name, bounds.size)
    assert abs(bounds.size.y - height) < 1e-6, (name, bounds.size)

(root / "independent-reader-results.json").write_text(json.dumps(results, indent=2))
print(f"PASS: {len(results)} STEP models; 2 DXF profiles; valid solids, units, and dimensions")
