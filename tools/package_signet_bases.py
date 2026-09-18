#!/usr/bin/env python3
"""Package audited, welded OBJ stock as portable RingDesigner masters.
Run audit_preset_bases.py --clean-signets first. Originals are never edited.
"""
import argparse, hashlib, json
from pathlib import Path
p=argparse.ArgumentParser();p.add_argument('--ids',nargs='+',default=['001','006']);a=p.parse_args()
root=Path(__file__).resolve().parents[1]
for name in a.ids:
    original=root/'assets/decoded/presets/Signet Ring'/name
    clean=root/'target/preset-study/cleaned-signets'/f'{name}.obj'
    vertices=[];faces=[]
    for line in clean.read_text().splitlines():
        row=line.split()
        if row and row[0]=='v':
            x,y,z=map(float,row[1:4]);vertices.append([x,z,-y])
        elif row and row[0]=='f':faces.append([int(x.split('/')[0])-1 for x in row[1:4]])
    params=json.loads((original/'params.json').read_text())
    bore=min((x*x+y*y)**.5 for x,y,z in vertices)
    head=max(v[1] for v in vertices)-bore
    c=dict(bore_radius_mm=bore,face_length_mm=params['Table Width'],face_width_mm=params['Table Length'],palm_thickness_mm=params.get('Bottom Thickness',params['Side Thickness']),head_height_mm=head,shoulder_start_mm=bore*.35,shoulder_end_mm=bore+head*.8)
    out=root/'bases/signets'/f'{name}.ringbase.json';out.parent.mkdir(parents=True,exist_ok=True)
    doc=dict(version=1,name=f'Signet {name}',source_sha256=hashlib.sha256((original/'Rings-0.obj').read_bytes()).hexdigest(),calibration=c,vertices=vertices,faces=faces)
    out.write_text(json.dumps(doc,separators=(',',':'))+'\n')
    print(name,len(vertices),len(faces),out.stat().st_size)
