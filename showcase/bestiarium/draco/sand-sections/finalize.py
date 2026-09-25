from pathlib import Path
import json,csv,math,hashlib,shutil
import numpy as np
OUT=Path(__file__).resolve().parent
code=(OUT/'measure.py').read_text().split('measure=[]')[0]
ns={'__file__': str(OUT/'measure.py')};exec(code,ns)
data=json.loads((OUT/'measurements.json').read_text())
refined=[]
for key in ['f07','f08','f05','f04','f13']:
 w=next(w for w in data['warnings'] if w['id']==key)
 p=np.array(w['raw']['point'][:2]);d=np.array([1.,0.]) if w['scan_axis']=='x' else np.array([0.,1.]);n=np.array(w['outward_scan_normal']);s=ns['sections'][w['raw']['point'][2]]
 back,bend=ns['trace'](s,p,d,n,-1,w['nearest_bounds_mm'],step=.0002,limit=.5)
 forward,fend=ns['trace'](s,p,d,n,1,w['nearest_bounds_mm'],step=.0002,limit=.6)
 samples=np.array(list(reversed(back))+[[0.,*w['nearest_bounds_mm']]]+forward)
 depths=samples[:,0]-bend['offset_mm'];widths=samples[:,2]-samples[:,1]
 item={'id':key,'step_mm':.0002,'bay_depth_mm':fend['offset_mm']-bend['offset_mm'],'tip_offset_mm':bend['offset_mm'],'mouth_width_mm':float(widths[-1]),'width_at_0p10_mm':float(np.interp(.1,depths,widths)) if depths[-1]>=.1 else None,'depth_to_0p30_mm':float(depths[np.flatnonzero(widths>=.3)[0]]) if np.any(widths>=.3) else None,'change_from_0p002_step_mm':fend['offset_mm']-bend['offset_mm']-w['bay_depth_in_scan_normal_mm']}
 refined.append(item)
print('refinement',json.dumps(refined,indent=2))

# Coarse/fine observations belong to one local bay only when their tracked tips coincide.
groups=[]
for w in data['warnings']:
 p=np.array(w['raw']['point'][:2]);d=np.array([1.,0.]) if w['scan_axis']=='x' else np.array([0.,1.]);n=np.array(w['outward_scan_normal'])
 row=np.array(w['samples_offset_left_right_mm'][0]);tip=p+.5*(row[1]+row[2])*d+row[0]*n
 group=next((g for g in groups if g['scan_axis']==w['scan_axis'] and g['z_mm']==w['raw']['point'][2] and np.linalg.norm(tip-np.array(g['tip_point_xy_mm']))<.01),None)
 if group is None:
  group={'id':f'G{len(groups)+1:02d}','warning_ids':[],'scan_axis':w['scan_axis'],'z_mm':w['raw']['point'][2],'tip_point_xy_mm':tip.tolist(),'classification':'bore tangency' if w.get('bore_candidate') else ('shallow external cusp' if w['bay_depth_in_scan_normal_mm']<=.10+1e-8 else 'open bay with short terminal narrowing')};groups.append(group)
 group['warning_ids'].append(w['id']);w['distinct_feature']=group['id'];w['classification']=group['classification']

external=[w for w in data['warnings'] if not w.get('bore_candidate')]
reach=[min(x for x in [w['depth_to_reach_opening_width_mm']['0.3'],w['bay_depth_in_scan_normal_mm']] if x is not None) for w in external]
summary={'source_commit':'9081d76','artistic_disposition':'CUT under the three-score review cap. The existing 5.8/10 score is unchanged; this diagnostic is not a new art review.','geometry_disposition':'No measured sub-0.30 mm warning is a sustained narrow neck. Each external warning is the terminal portion of an open bay or shallow surface cusp; one warning is a tangent to the broad bore. This supersedes the unresolved evidence item for this stable export, without overriding the raw release Review status or proving sand strength.','raw_sub_0p30_warning_count':len(data['warnings']),'distinct_feature_count':len(groups),'external_warning_count':len(external),'largest_depth_before_0p30_opening_or_unbounded_outlet_mm':max(reach),'largest_external_scan_bay_depth_mm':max(w['bay_depth_in_scan_normal_mm'] for w in external),'refinement':refined,'required_geometry_change':'None established by these sections for avoiding a sustained thin sand web. Do not shorten all thorns merely to zero the raster-warning count.','optional_finish_change':{'where':'G04 / c05,f07 near 47.34 degrees in the z=0 section','measured':'The longest terminal narrowing reaches 0.30 mm after about 0.114 mm; at 0.10 mm its supported x chord is about 0.254 mm, with a 0.454 mm mouth.','change':'If retaining the candidate for a future casting trial, blend this sharp cusp with a 0.15 mm root radius over roughly 0.35 mm of the contour, or add 0.045 mm of local pattern metal at the root to remove its deepest tip. Preserve the already open side outlet; remeasure rather than applying a universal crest-gap claim. This is optional finish refinement, not a demonstrated missing gate.'},'measurement_limits':['Exact piecewise-linear STL sections, not a voxel approximation.','Depth and outlet tracing at 0.002 mm; four disputed sections and the deepest bay rechecked at 0.0002 mm.','The width is a chord perpendicular to the tracked outlet direction, not a constant width over the whole pocket. Every rounded or V-shaped terminal tip tends to zero width.','No material-strength, compaction, pouring or bench-casting claim follows from these geometric measurements.','The fine release grid was capped: actual cells 0.074832 by 0.081112 mm, not square 0.075 mm cells.']}
data['distinct_features']=groups;data['convergence']=refined;data['summary']=summary
(OUT/'measurements.json').write_text(json.dumps(data,indent=2)+'\n')
(OUT/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
fields=['id','distinct_feature','theta_deg','z_mm','scan_axis','raw_width_mm','exact_chord_mm','bay_depth_mm','mouth_width_mm','opening_at_tip_plus_0p10_mm','depth_to_0p30_opening_mm','classification']
rows=[]
for w in data['warnings']:
 rows.append({'id':w['id'],'distinct_feature':w['distinct_feature'],'theta_deg':w['theta_deg'],'z_mm':w['raw']['point'][2],'scan_axis':w['scan_axis'],'raw_width_mm':w['raw']['width_mm'],'exact_chord_mm':w['exact_chord_mm'],'bay_depth_mm':w['bay_depth_in_scan_normal_mm'],'mouth_width_mm':w['mouth_width_before_outlet_mm'],'opening_at_tip_plus_0p10_mm':w['opening_width_at_depth_from_tip_mm']['0.1'],'depth_to_0p30_opening_mm':w['depth_to_reach_opening_width_mm']['0.3'],'classification':w['classification']})
with (OUT/'all-warning-sections.csv').open('w',newline='') as f:
 writer=csv.DictWriter(f,fieldnames=fields);writer.writeheader();writer.writerows(rows)

lines=['# Draco: casting-pattern sand-slot sections','','**The stable 9081d76 candidate remains cut at 5.8/10 under the three-round art cap.** This investigation resolves the thin-slot evidence; it does not change the score.','','All 31 raw sub-0.30 mm warnings are accounted for in 23 distinct section features. Thirty external observations reach a 0.30 mm chord or an unbounded side outlet within 0.116 mm of their terminal tip. The remaining observation is a tangent to the broad bore. No sustained thin neck is demonstrated by these sections.','','The raw warnings are valid narrow chords; calling every one a raster error would be wrong. Their depth and side support make them shallow cusps or the tips of broader openings. `release.status` remains `Review`, and geometry cannot establish sand strength.','','## Four explicit correlations','','Measurements below use the same shrink-compensated casting-pattern STL as the release report. Depth is measured along the outward normal to the original scan line; an outlet begins when one flanking metal side ends. It is not the previous angular width 0.10 mm above a radial floor.','','| Raw ID / angle | Exact scan chord | Local bay depth | Mouth just before outlet | 0.30 mm reached at tip depth |','|---|---:|---:|---:|---:|']
for m in refined[:4]:
 w=next(w for w in data['warnings'] if w['id']==m['id']);d='opens sooner' if m['depth_to_0p30_mm'] is None else f'{m["depth_to_0p30_mm"]:.4f} mm'
 lines.append(f'| {m["id"]} / {w["theta_deg"]:.2f}° | {w["exact_chord_mm"]:.4f} mm | {m["bay_depth_mm"]:.4f} mm | {m["mouth_width_mm"]:.4f} mm | {d} |')
lines+=['','![Four correlated sections](four-correlated-sections.png)','','The 47.32° warning is the most persistent of the four: the actual gap is 0.133 mm at the raster line and widens to 0.30 mm after about 0.114 mm from its tip. The 149.89° and 319.20° findings are small kinks only about 0.022 and 0.057 mm deep in the scan direction, not slots between parallel thorn walls.','','## Every distinct feature','','The full data retain both passes, exact chords, tip and outlet coordinates, sampled opening profiles, and links back to every raw warning. `open` below means one side has already ended before the +0.10 mm section; it does not mean a zero-width neck.','','| Feature | Raw IDs | Angle(s) | Bay depth | Width at tip +0.10 mm | Disposition |','|---|---|---|---:|---:|---|']
for g in groups:
 ww=[w for w in data['warnings'] if w['distinct_feature']==g['id']];w=ww[-1];D=w['bay_depth_in_scan_normal_mm'];W=w['opening_width_at_depth_from_tip_mm']['0.1'];ds='bore cavity' if D is None else f'{D:.3f} mm';ws=f'{W:.3f} mm' if W is not None else 'open';ang=', '.join(f'{a["theta_deg"]:.2f}°' for a in ww)
 lines.append(f'| {g["id"]} | {", ".join(g["warning_ids"])} | {ang} | {ds} | {ws} | {g["classification"]} |')
lines+=['','## Geometry recommendation','','No broad thorn-shortening change is justified by this evidence. If the archived candidate is revisited, optional finishing of the sharp 47.34° cusp can use a 0.15 mm root radius across roughly 0.35 mm of contour, or fill its deepest 0.045 mm with pattern metal; retain the open side outlet and remeasure. This is a local finish refinement, not evidence that the current thorns will fuse.','','## Files and method','','- `all-warning-sections.csv`: one row for every raw warning.','- `measurements.json`: exact section data, 23-feature grouping, profiles and convergence.','- `contact-1.png` through `contact-4.png`: every warning section, with corresponding vector PDFs.','- `four-correlated-sections.png` and `.pdf`: common-scale diagrams for the four disputed points.','- `input/`: immutable mesh and report copies; source SHA-256 is recorded in `measurements.json`.','- `analyze.py`, `measure.py`, `finalize.py`: diagnostic reproduction scripts.','','Triangle-plane intersections are computed analytically on the 1,342,302-triangle casting pattern at z=0 and z=5.690816402 mm. Sand chords are traced toward the exterior with a 0.002 mm step. Five sections were repeated at 0.0002 mm; bay depths changed by at most 0.0024 mm. The mesh carries scale 1.019367991845056. The fine release grid is capped at 384 cells along Y, giving actual pitch 0.074832 × 0.081112 mm.','','Reproduce from the repository root:','','```sh','python3 showcase/bestiarium/draco/sand-sections/analyze.py','python3 showcase/bestiarium/draco/sand-sections/measure.py','python3 showcase/bestiarium/draco/sand-sections/finalize.py','```','']
(OUT/'README.md').write_text('\n'.join(lines))

