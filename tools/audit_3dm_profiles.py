#!/usr/bin/env python3
"""Inventory actual 3DM geometry without converting or modifying source files.

uv run --no-project --with rhino3dm==8.32.0 python tools/audit_3dm_profiles.py \
    assets/User/Profiles --out target/preset-study/3dm-profile-audit.json
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path

import rhino3dm


def inspect(path, root):
    model = rhino3dm.File3dm.Read(str(path))
    row = {
        "path": str(path.relative_to(root)),
        "family": path.parent.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "readable": model is not None,
        "objects": [],
    }
    if model is None:
        return row
    row["units"] = str(model.Settings.ModelUnitSystem)
    for obj in model.Objects:
        geometry = obj.Geometry
        box = geometry.GetBoundingBox()
        record = {
            "type": type(geometry).__name__,
            "name": obj.Attributes.Name,
            # Native document coordinates; unitless profiles are not mm.
            "bounds": [[box.Min.X, box.Min.Y, box.Min.Z], [box.Max.X, box.Max.Y, box.Max.Z]],
        }
        if isinstance(geometry, rhino3dm.Curve):
            record.update(closed=geometry.IsClosed, planar=geometry.IsPlanar(), degree=geometry.Degree)
        if isinstance(geometry, rhino3dm.Brep):
            record.update(solid=geometry.IsSolid, faces=len(geometry.Faces))
        if isinstance(geometry, rhino3dm.Mesh):
            record.update(vertices=len(geometry.Vertices), faces=len(geometry.Faces))
        row["objects"].append(record)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    root, out = args.root.resolve(), args.out.resolve()
    if not root.is_dir():
        parser.error("profile root is not a directory")
    if out == root or root in out.parents:
        parser.error("report must be outside the supplied profile tree")
    files = sorted(p for p in root.rglob("*") if p.is_file() and p.suffix.lower() == ".3dm")
    if not files:
        parser.error("no 3DM files found")
    rows = [inspect(path, root) for path in files]
    families = {}
    for family in sorted({r["family"] for r in rows}):
        subset = [r for r in rows if r["family"] == family]
        objects = [o for r in subset for o in r["objects"]]
        families[family] = {
            "files": len(subset),
            "types": dict(Counter(o["type"] for o in objects)),
            "planar_curves": sum(o.get("planar", False) for o in objects),
            "closed_curves": sum(o.get("closed", False) for o in objects),
            "solid_breps": sum(o.get("solid", False) for o in objects if o["type"] == "Brep"),
            "units": dict(Counter(r.get("units", "unknown") for r in subset)),
        }
    report = {
        "source_root": str(root), "rhino3dm": rhino3dm.__version__,
        "files": len(rows), "families": families, "models": rows,
    }
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(report, indent=2, allow_nan=False) + "\n")
    for family, data in families.items():
        kinds = ", ".join(f"{count} {kind}" for kind, count in data["types"].items())
        print(f"{family}: {data['files']} files; {kinds}")
    print(f"{len(rows)} files inspected; report: {out}")
    if any(not r["readable"] for r in rows):
        parser.exit(1, "Some files could not be read; see report.\n")


if __name__ == "__main__":
    main()
