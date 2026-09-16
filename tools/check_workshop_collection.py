#!/usr/bin/env python3
"""Independent file checks for the authored collection (cadquery-ocp required)."""
import json
from pathlib import Path
import struct
import sys
import zipfile
from OCP.STEPControl import STEPControl_Reader
from OCP.IFSelect import IFSelect_RetDone
from OCP.BRepCheck import BRepCheck_Analyzer
from OCP.BRepGProp import BRepGProp
from OCP.GProp import GProp_GProps
from OCP.TopAbs import TopAbs_SOLID
from OCP.TopExp import TopExp_Explorer
from OCP.BRepAlgoAPI import BRepAlgoAPI_Common

root = Path(sys.argv[1])
rows = json.loads((root / "collection.json").read_text())
results = []
for row in rows:
    name = row["slug"]
    folder = root / name
    package = folder / "pattern-package"
    report = json.loads((package / "report.json").read_text())
    data = (package / "pattern.stl").read_bytes()
    count = struct.unpack_from("<I", data, 80)[0]
    assert len(data) == 84 + 50 * count
    assert count == report["geometry"]["validation"]["triangle_count"]
    value = 0xCBF29CE484222325
    for byte in data[80:]:
        value = ((value ^ byte) * 0x100000001B3) & ((1 << 64) - 1)
    assert f"{value:016x}" == report["pattern_fingerprint"]
    with zipfile.ZipFile(package / "pattern.3mf") as archive:
        assert archive.testzip() is None
        assert b'unit="millimeter"' in archive.read("3D/3dmodel.model")
    result = dict(name=name, stl_triangles=count, pattern_fingerprint=report["pattern_fingerprint"], threemf_valid=True)
    step = folder / "cad-assembly" / "assembly-nominal.step"
    if step.exists():
        reader = STEPControl_Reader()
        assert reader.ReadFile(str(step)) == IFSelect_RetDone
        assert reader.TransferRoots() > 0
        shape = reader.OneShape()
        assert BRepCheck_Analyzer(shape).IsValid(), name
        prop = GProp_GProps()
        BRepGProp.VolumeProperties_s(shape, prop)
        volume = prop.Mass()
        explorer = TopExp_Explorer(shape, TopAbs_SOLID)
        solids = []
        while explorer.More():
            solids.append(explorer.Current())
            explorer.Next()
        assert len(solids) == (2 if name == "lantern" else 1)
        error = abs(volume - row["volume_mm3"]) / volume
        assert error < 0.006, (name, volume, error)
        result.update(step_valid=True, solids=len(solids), step_volume_mm3=volume, mesh_relative_error=error)
        if len(solids) == 2:
            common = BRepAlgoAPI_Common(solids[0], solids[1])
            assert common.IsDone()
            common_prop = GProp_GProps()
            BRepGProp.VolumeProperties_s(common.Shape(), common_prop)
            result["fitting_stock_overlap_mm3"] = common_prop.Mass()
            # Prove contact without claiming that a stock overlap is a finished joint.
            assert 0 < common_prop.Mass() < volume
            result["fitting_stock_grams"] = common_prop.Mass() * 13.07 / 1000
    results.append(result)
(root / "independent-checks.json").write_text(json.dumps(results, indent=2) + "\n")
print(json.dumps(results, indent=2))
