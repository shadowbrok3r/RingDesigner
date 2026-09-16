#!/usr/bin/env python3
"""Independently read the delivered Aster meshes and native-app source snapshots."""
import hashlib
import argparse
import json
from pathlib import Path
import struct
import zipfile
import numpy as np
import trimesh

repo = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--root', type=Path, default=repo / 'showcase/aster')
parser.add_argument('--recipe', type=Path, default=repo / 'crates/ringdesign-core/assets/aster.ring.json')
args = parser.parse_args()
root = args.root.resolve()
report = json.loads((root / 'report.json').read_text())
fine = json.loads((root / 'release-fine.json').read_text())
for release in (report['release'], fine):
    assert not release['obstructions'] and release['unresolved_rays'] == 0
    assert release['fits_flask']
assert not report['detail_findings']

results = {'release_pitches_mm': [report['release']['cell_mm'], fine['cell_mm']], 'meshes': []}
for name, scale in [('nominal.stl', 1.), ('pattern-package/pattern.stl', report['pattern_scale'])]:
    path = root / name
    with path.open('rb') as f:
        count = struct.unpack_from('<I', f.read(84), 80)[0]
    assert path.stat().st_size == 84 + 50 * count
    mesh = trimesh.load_mesh(path, file_type='stl', process=True)
    assert np.isfinite(mesh.vertices).all()
    assert mesh.is_watertight and mesh.is_winding_consistent and mesh.euler_number == 0
    # Union the indexed triangles to count solids independently of the CAD kernel.
    parent = np.arange(len(mesh.vertices)).tolist()
    def find(i):
        while parent[i] != i:
            parent[i] = parent[parent[i]]
            i = parent[i]
        return i
    bodies = len(parent)
    for face in mesh.faces:
        a = find(int(face[0]))
        for vertex in face[1:]:
            b = find(int(vertex))
            if a != b:
                parent[b] = a
                bodies -= 1
    assert bodies == 1
    bore = float(2 * np.linalg.norm(mesh.vertices[:, :2], axis=1).min())
    assert abs(bore - 18.2 * scale) < 0.0001
    expected = report['geometry']['volume_mm3'] * (scale / report['pattern_scale']) ** 3
    assert abs(mesh.volume - expected) / expected < 0.000001
    results['meshes'].append(dict(file=name, triangles=count, solids=bodies, watertight=True,
        consistent_winding=True, bore_mm=bore, volume_mm3=float(mesh.volume),
        sha256=hashlib.file_digest(path.open('rb'), 'sha256').hexdigest()))
    print(name, 'closed, consistently wound, one ring body', flush=True)
    del mesh, parent

with zipfile.ZipFile(root / 'pattern-package/pattern.3mf') as archive:
    assert archive.testzip() is None
    model = archive.read('3D/3dmodel.model')
    assert b'unit="millimeter"' in model and model.count(b'<triangle ') == count
results['threemf_valid'] = True

baseline = json.loads(args.recipe.read_text())
results['app_sources'] = []
for platform in ['desktop', 'android']:
    path = root / 'reels' / (platform + '-created.ring.json')
    d = json.loads(path.read_text())
    # Embedded copies can be repacked on Save; source artwork and every modelling
    # parameter must match the reviewed construction recipe exactly.
    for key in ('name','size','profile','shank','layers','build','draft','manufacturing','svgs','drawn','texts','recipes'):
        assert d[key] == baseline[key], (platform, key)
    assert not any(e.get('bench_only',False) for e in d['layers']['layers'])
    results['app_sources'].append(dict(platform=platform, parameters_identical=True,
        layer_count=len(d['layers']['layers']), sha256=hashlib.file_digest(path.open('rb'),'sha256').hexdigest()))

(root / 'independent-checks.json').write_text(json.dumps(results, indent=2) + '\n')
print('PASS: casting geometry, exchange archive, and both actual native-app designs')
