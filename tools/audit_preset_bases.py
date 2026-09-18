#!/usr/bin/env python3
"""Measure decoded preset stock without modifying the supplied assets.

python tools/audit_preset_bases.py assets/decoded/presets --out target/preset-study --clean-signets
Requires numpy and trimesh. Clean copies retain the source coordinate frame
(finger axis Y, head +Z); they are not procedural RingDesigner designs.
"""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import statistics

import numpy as np
import trimesh


def topology(mesh):
    counts = np.bincount(mesh.edges_unique_inverse)
    return {
        "vertices": len(mesh.vertices),
        "triangles": len(mesh.faces),
        "boundary_edges": int(np.sum(counts == 1)),
        "nonmanifold_edges": int(np.sum(counts > 2)),
        "watertight": bool(mesh.is_watertight),
        "consistent_winding": bool(mesh.is_winding_consistent),
        "components": len(mesh.split(only_watertight=False)),
    }


def signet(folder, root, out, clean):
    path = folder / "Rings-0.obj"
    mesh = trimesh.load_mesh(path, process=False)
    raw = topology(mesh)
    mesh.merge_vertices(digits_vertex=8)
    welded = topology(mesh)
    before = len(mesh.faces)
    # Drop collapsed/duplicate faces only. No smoothing, hole filling,
    # subdivision, remeshing, or dimensional scaling.
    mesh.update_faces(mesh.nondegenerate_faces(height=1e-8))
    mesh.update_faces(mesh.unique_faces())
    mesh.remove_unreferenced_vertices()
    cleaned = topology(mesh)
    usable = cleaned["watertight"] and cleaned["consistent_winding"] and cleaned["components"] == 1
    entry = {
        "id": folder.name,
        "source": str(path.relative_to(root)),
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "params": json.loads((folder / "params.json").read_text()),
        "raw": raw,
        "welded": welded,
        "cleaned": cleaned,
        "removed_triangles": before - len(mesh.faces),
        "candidate_for_solid_import": usable,
        "bounds_mm": mesh.bounds.tolist(),
        "minimum_vertex_radius_from_finger_axis_mm": float(np.hypot(mesh.vertices[:, 0], mesh.vertices[:, 2]).min()),
    }
    if usable:
        entry["volume_mm3"] = float(abs(mesh.volume))
    if clean and usable:
        target = out / "cleaned-signets" / (folder.name + ".obj")
        target.parent.mkdir(parents=True, exist_ok=True)
        mesh.export(target, digits=12)
        entry["cleaned_copy"] = str(target)
    return entry


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--clean-signets", action="store_true")
    args = parser.parse_args()
    root, out = args.root.resolve(), args.out.resolve()
    if not root.is_dir():
        parser.error("preset root is not a directory")
    if out == root or root in out.parents:
        parser.error("output must be outside the supplied preset tree")
    families = {}
    for family in sorted(p for p in root.iterdir() if p.is_dir()):
        files = sorted(family.glob("*/params.json"))
        if not files:
            continue
        values = collections.defaultdict(list)
        mesh_count = 0
        for path in files:
            params = json.loads(path.read_text())
            for key, value in params.items():
                if isinstance(value, (int, float)) and not isinstance(value, bool):
                    values[key].append(value)
            mesh_count += len(list(path.parent.glob("*.obj")))
        families[family.name] = {
            "presets": len(files), "mesh_files": mesh_count,
            "parameters": {key: {"min": min(v), "median": statistics.median(v), "max": max(v)} for key, v in sorted(values.items())},
        }
    out.mkdir(parents=True, exist_ok=True)
    signets = [signet(p.parent, root, out, args.clean_signets) for p in sorted((root / "Signet Ring").glob("*/params.json"))]
    report = {
        "source_root": str(root),
        "preset_count": sum(v["presets"] for v in families.values()),
        "family_count": len(families),
        "method": "Weld to eight decimal places in mm; remove triangles with minimum height below 1e-8 mm and duplicate faces. No hole filling or vertex smoothing. Topology is not a casting or wall-thickness guarantee.",
        "families": families, "signets": signets,
    }
    (out / "preset-audit.json").write_text(json.dumps(report, indent=2) + "\n")
    ready = sum(s["candidate_for_solid_import"] for s in signets)
    lines = [
        f"# Supplied preset audit\n\n{report['preset_count']} presets in {len(families)} families; {len(signets)} signet bases measured.",
        f"\n{ready}/{len(signets)} signets are closed, consistently wound single components after seam cleanup. Original files are untouched.",
        "\n" + report["method"],
        "\n| Signet | Raw boundary edges | Welded boundary edges | Removed faces | Clean boundary edges | Solid candidate |",
        "|---|---:|---:|---:|---:|---|",
    ]
    for s in signets:
        lines.append(f"| {s['id']} | {s['raw']['boundary_edges']} | {s['welded']['boundary_edges']} | {s['removed_triangles']} | {s['cleaned']['boundary_edges']} | {'Yes' if s['candidate_for_solid_import'] else 'Needs repair'} |")
    lines += ["\n| Family | Presets | Cached meshes |", "|---|---:|---:|"]
    for family, f in families.items():
        lines.append(f"| {family} | {f['presets']} | {f['mesh_files']} |")
    (out / "README.md").write_text("\n".join(lines) + "\n")
    print(f"{report['preset_count']} presets / {len(families)} families; {ready}/{len(signets)} signets ready for solid-import evaluation; report: {out / 'README.md'}")


if __name__ == "__main__":
    main()
