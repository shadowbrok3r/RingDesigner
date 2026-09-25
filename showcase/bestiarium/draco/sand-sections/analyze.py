from pathlib import Path
import json, hashlib, math, shutil
import numpy as np
from scipy.signal import find_peaks

OUT=Path(__file__).resolve().parent
SRC=OUT.parent
OUT.mkdir(exist_ok=True)
report=json.loads((SRC/'report.json').read_text())
coarse=report['manufacturing']['release']
fine=json.loads((SRC/'release-fine.json').read_text())
for fn in ['report.json','release-fine.json','verification.json']:
    shutil.copy2(SRC/fn,OUT/('raw-'+fn))
mesh=SRC/'casting-pattern.stl'
dtype=np.dtype([('normal','<f4',(3,)),('vertices','<f4',(3,3)),('attribute','<u2')])
records=np.memmap(mesh,dtype=dtype,offset=84,mode='r')
tri=records['vertices']
meta={'source':str(mesh),'sha256':hashlib.sha256(mesh.read_bytes()).hexdigest(),'triangles':len(tri),'pattern_scale':report['manufacturing']['pattern_scale'],'bounds_mm':[tri.min(axis=(0,1)).astype(float).tolist(),tri.max(axis=(0,1)).astype(float).tolist()]}

def section(z):
    relevant=(tri[:,:,2].min(axis=1)<=z)&(tri[:,:,2].max(axis=1)>z)
    ts=np.asarray(tri[relevant],dtype=float)
    ids=[]; points=[]
    for a,b in [(0,1),(1,2),(2,0)]:
        za=ts[:,a,2];zb=ts[:,b,2]
        hit=((za<=z)&(zb>z))|((zb<=z)&(za>z))
        i=np.flatnonzero(hit)
        t=(z-za[i])/(zb[i]-za[i])
        p=ts[i,a,:2]+(ts[i,b,:2]-ts[i,a,:2])*t[:,None]
        ids.extend(i);points.extend(p)
    order=np.argsort(ids,kind='stable');ids=np.array(ids)[order];points=np.array(points)[order]
    counts=np.unique(ids,return_counts=True)[1]
    assert np.all(counts==2),np.unique(counts,return_counts=True)
    s=points.reshape(-1,2,2)
    return s[np.linalg.norm(s[:,1]-s[:,0],axis=1)>1e-8]

def line_intersections(s,anchor,direction):
    p=s[:,0]-anchor; q=s[:,1]-anchor
    d=np.asarray(direction,dtype=float); n=np.array([-d[1],d[0]])
    a=p@n;b=q@n
    hit=((a<=0)&(b>0))|((b<=0)&(a>0))
    t=-a[hit]/(b[hit]-a[hit]); pts=p[hit]+(q[hit]-p[hit])*t[:,None]
    values=np.sort(pts@d)
    return values[np.r_[True,np.diff(values)>1e-6]]

def chord(s,p,axis):
    direction=np.array([1.,0.]) if axis==0 else np.array([0.,1.])
    vals=line_intersections(s,np.asarray(p[:2]),direction)
    lo=vals[vals<0];hi=vals[vals>=0]
    return {'intersections_relative_mm':vals.tolist(),'nearest_bounds_mm':[float(lo[-1]) if len(lo) else None,float(hi[0]) if len(hi) else None],'exact_chord_mm':float(hi[0]-lo[-1]) if len(lo) and len(hi) else None,'point_is_metal':bool(np.sum(vals>0)%2)}

warnings=[]
sections={}
for scan,r in [('coarse',coarse),('fine',fine)]:
    for i,f in enumerate(r['sand_findings']):
        if f['width_mm']>=.3:continue
        p=f['point'];z=p[2]
        if z not in sections:sections[z]=section(z)
        axis=int(np.argmin([abs(f['width_mm']/h-round(f['width_mm']/h)) for h in r['cell_mm']]))
        w={'id':f'{scan[0]}{i:02d}','scan':scan,'raw':f,'theta_deg':float(np.degrees(np.arctan2(p[1],p[0]))%360),'radius_mm':float(np.hypot(p[0],p[1])),'scan_axis':'x' if axis==0 else 'y','scan_cell_mm':r['cell_mm'],**chord(sections[z],p,axis)}
        warnings.append(w)

s0=sections[0.]
step=.002
angles=np.arange(round(360/step))*step
profile=np.zeros(len(angles))
inner=np.full(len(angles),np.inf)
for a,b in s0:
    ra,rb=np.linalg.norm(a),np.linalg.norm(b)
    aa,bb=np.degrees(np.arctan2([a[1],b[1]],[a[0],b[0]]))
    if bb-aa>180:bb-=360
    if aa-bb>180:bb+=360
    lo,hi=sorted([aa,bb])
    bins=np.arange(math.ceil(lo/step),math.floor(hi/step)+1,dtype=int)
    if not len(bins):continue
    th=bins*step*np.pi/180
    unit=np.column_stack([np.cos(th),np.sin(th)])
    delta=b-a
    denom=unit[:,0]*delta[1]-unit[:,1]*delta[0]
    radius=(a[0]*delta[1]-a[1]*delta[0])/denom
    if min(ra,rb)>10.5:np.maximum.at(profile,bins%len(angles),radius)
    elif max(ra,rb)<10.5:np.minimum.at(inner,bins%len(angles),radius)
assert np.min(profile)>10.5
assert np.all(np.isfinite(inner))
np.savez_compressed(OUT/'sections.npz',**{f'section_{str(z).replace(".","_")}':s for z,s in sections.items()},angles=angles,outer=profile,inner=inner)

# Each radial minimum is kept down to 0.001 mm prominence; no 0.05 mm notch filter is used.
n=len(profile)
extended=np.tile(profile,3)
mins,props=find_peaks(-extended,prominence=.001,wlen=round(14/step)|1)
valleys=[]
for k,idx in enumerate(mins):
    if not n<=idx<2*n:continue
    valley={'index':int(idx-n),'theta_deg':float(angles[idx-n]),'floor_radius_mm':float(extended[idx]),'prominence_mm':float(props['prominences'][k]),'left_peak_theta_deg':float((props['left_bases'][k]-n)*step),'right_peak_theta_deg':float((props['right_bases'][k]-n)*step),'left_peak_radius_mm':float(extended[props['left_bases'][k]]),'right_peak_radius_mm':float(extended[props['right_bases'][k]])}
    valleys.append(valley)
for w in warnings:
    if abs(w['raw']['point'][2])>1e-8:continue
    w['radial_distance_outside_metal_mm']=float(w['radius_mm']-profile[round(w['theta_deg']/step)%n])
    w['bore_candidate']=w['radius_mm']<10.5
    if w['bore_candidate']:continue
    vv=sorted(valleys,key=lambda v:abs((v['theta_deg']-w['theta_deg']+180)%360-180))[:3]
    w['nearest_radial_valleys']=vv

result={'metadata':meta,'method':{'angular_step_deg':step,'plane_slices_mm':list(sections),'section_segments':{str(z):len(s) for z,s in sections.items()},'radial_minimum_prominence_floor_mm':.001,'radial_minimum_search_window_deg':14},'raw_warning_count':len(warnings),'warnings':warnings,'radial_valleys':valleys}
(OUT/'measurements-stage1.json').write_text(json.dumps(result,indent=2)+'\n')
for w in warnings:
    print(w['id'],round(w['theta_deg'],3),'raw',round(w['raw']['width_mm'],4),'exact',round(w['exact_chord_mm'],4),'metal',w['point_is_metal'],'outside',round(w.get('radial_distance_outside_metal_mm',0),4),'nearest',[(round(v['theta_deg'],3),round(v['prominence_mm'],3)) for v in w.get('nearest_radial_valleys',[])])
print('section counts',result['method']['section_segments'],'valleys',len(valleys))
