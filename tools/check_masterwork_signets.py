#!/usr/bin/env python3
"""Read exported STL/3MF/ZIP independently (uses installed numpy/trimesh)."""
import hashlib
import json
from pathlib import Path
import struct
import sys
import zipfile

import numpy as np
import trimesh

def connected_bodies(mesh):
    # Trimesh's component count optionally needs scipy; union the triangle
    # vertices directly so this check only needs numpy and trimesh.
    parent = list(range(len(mesh.vertices)))
    count = len(parent)
    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    for face in mesh.faces:
        a = find(int(face[0]))
        for vertex in face[1:]:
            b = find(int(vertex))
            if a != b:
                parent[b] = a
                count -= 1
    return count

root = Path(sys.argv[1]).resolve()
rows = json.loads((root / "collection.json").read_text())
assert {r["slug"] for r in rows} == {"solstice", "nocturne"}
results = []
for row in rows:
    folder = root / row["slug"]
    report = json.loads((folder / "pattern-package/report.json").read_text())
    item = {"ring": row["slug"], "files": []}
    for name, expected_volume in [("nominal.stl", row["volume_mm3"]), ("pattern-package/pattern.stl", report["geometry"]["volume_mm3"])]:
        file = folder / name
        data = file.read_bytes()
        count = struct.unpack_from("<I", data, 80)[0]
        assert len(data) == 84 + 50 * count, name
        mesh = trimesh.load_mesh(file, file_type="stl", process=True)
        assert np.isfinite(mesh.vertices).all(), name
        assert mesh.is_watertight and mesh.is_winding_consistent, name
        assert connected_bodies(mesh) == 1 and mesh.euler_number == 0, name
        error = abs(mesh.volume - expected_volume) / expected_volume
        assert error < 1e-6, (name, error)
        bore = 2 * np.linalg.norm(mesh.vertices[:, :2], axis=1).min()
        if name == "nominal.stl":
            assert abs(bore - 18.2) < 1e-4, bore
        item["files"].append(dict(file=name, triangles=count, sha256=hashlib.sha256(data).hexdigest(), watertight=True, consistent_winding=True, solids=1, euler_number=0, volume_mm3=float(mesh.volume), relative_volume_error=float(error), measured_bore_mm=float(bore)))
        print(row["slug"], name, count, "closed, one solid", flush=True)
        del mesh, data
    for name in ["nominal.3mf", "pattern-package/pattern.3mf"]:
        with zipfile.ZipFile(folder / name) as archive:
            assert archive.testzip() is None
            model = archive.read("3D/3dmodel.model")
            assert b'unit="millimeter"' in model
            assert model.count(b"<triangle ") == report["geometry"]["validation"]["triangle_count"]
    with zipfile.ZipFile(folder / "pattern-package.zip") as archive:
        assert archive.testzip() is None
        for name in archive.namelist():
            assert archive.read(name) == (folder / "pattern-package" / name).read_bytes(), name
    checks = json.loads((folder / "verification.json").read_text())
    assert len(checks) == 2 and all(c["nominal_identical"] and c["pattern_identical"] and c["empty_library"] for c in checks)
    if row["slug"] == "solstice":
        fine = json.loads((folder / "release-fine.json").read_text())
        for release in [report["release"], fine]:
            assert not release["obstructions"] and release["unresolved_rays"] == 0 and release["fits_flask"]
        assert not report["detail_findings"]
        item["fine_release_cell_mm"] = fine["cell_mm"]
    else:
        stones = json.loads((folder / "stones.json").read_text())
        assert stones["count"] == 3 and stones["tight_pairs"] == 0
        orientations = json.loads((folder / "sand-orientations.json").read_text())
        assert all(o["obstructions"] > 0 for o in orientations)
        item["stone_count"] = stones["count"]
    item.update(threemf_valid=True, delivered_zip_identical=True, empty_library_source_checks=2)
    results.append(item)
(root / "independent-checks.json").write_text(json.dumps(results, indent=2) + "\n")
print("PASS: both signets, four solids, 3MF and ZIP delivery, source checks, withdrawal and stone census")
