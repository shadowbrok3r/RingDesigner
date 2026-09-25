from pathlib import Path
import json,math
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.collections import LineCollection
from matplotlib.path import Path as MPath
from scipy.signal import find_peaks

OUT=Path(__file__).resolve().parent
data=json.loads((OUT/'measurements-stage1.json').read_text())
npz=np.load(OUT/'sections.npz')
sections={z:npz['section_'+str(z).replace('.','_')] for z in data['method']['plane_slices_mm']}

def intersections(s,anchor,d):
    p=s[:,0]-anchor;q=s[:,1]-anchor;n=np.array([-d[1],d[0]])
    a=p@n;b=q@n;hit=((a<=0)&(b>0))|((b<=0)&(a>0))
    t=-a[hit]/(b[hit]-a[hit]);v=np.sort((p[hit]+(q[hit]-p[hit])*t[:,None])@d)
    return v[np.r_[True,np.diff(v)>1e-6]]

def sand_intervals(vals):
    assert len(vals)%2==0,len(vals)
    a=np.r_[-np.inf,vals,np.inf]
    return [(a[i],a[i+1]) for i in range(0,len(a)-1,2)]

def trace(s,p,d,n,sign,start,step=.002,limit=5):
    previous=np.array(start);out=[]
    for distance in np.arange(step,limit+step*.5,step):
        offset=sign*distance
        vals=intersections(s,p+offset*n,d)
        candidates=sand_intervals(vals)
        overlaps=[max(0,min(b,previous[1])-max(a,previous[0])) for a,b in candidates]
        best=int(np.argmax(overlaps)) if len(overlaps) else -1
        if best<0 or overlaps[best]<=1e-9:
            return out,{'event':'closed','offset_mm':float(offset)}
        lo,hi=candidates[best]
        if not np.isfinite(lo+hi):
            return out,{'event':'open_to_exterior','offset_mm':float(offset),'left_mm':float(lo) if np.isfinite(lo) else None,'right_mm':float(hi) if np.isfinite(hi) else None}
        out.append([float(offset),float(lo),float(hi)])
        previous=np.array([lo,hi])
    return out,{'event':'range_limit','offset_mm':float(sign*limit)}

measure=[]
for w in data['warnings']:
    p=np.array(w['raw']['point'][:2]);s=sections[w['raw']['point'][2]]
    d=np.array([1.,0.]) if w['scan_axis']=='x' else np.array([0.,1.])
    n=np.array([-d[1],d[0]])
    if np.dot(n,p)<0:n=-n
    if w.get('bore_candidate'):n=-n
    start=w['nearest_bounds_mm']
    back,endback=trace(s,p,d,n,-1,start,limit=2)
    forward,endfwd=trace(s,p,d,n,1,start,limit=3 if not w.get('bore_candidate') else 10)
    samples=np.array(list(reversed(back))+[[0.,*start]]+forward)
    floor=endback['offset_mm']
    depth=samples[:,0]-floor;width=samples[:,2]-samples[:,1]
    target_width_depth={}
    for target in [.15,.30,.40,.60]:
        good=np.flatnonzero(width>=target)
        target_width_depth[str(target)]=float(depth[good[0]]) if len(good) else None
    necks={}
    for h in [.02,.05,.10,.20,.30,.50]:
        i=np.searchsorted(depth,h)
        necks[str(h)]=float(np.interp(h,depth,width)) if 0<=i<len(depth) else None
    item={**w,'outward_scan_normal':n.tolist(),'inward_endpoint':endback,'outward_endpoint':endfwd,'tip_offset_from_warning_mm':float(floor),'bay_depth_in_scan_normal_mm':float(endfwd['offset_mm']-floor) if endfwd['event']=='open_to_exterior' else None,'mouth_width_before_outlet_mm':float(width[-1]) if endfwd['event']=='open_to_exterior' else None,'opening_width_at_depth_from_tip_mm':necks,'depth_to_reach_opening_width_mm':target_width_depth,'samples_offset_left_right_mm':samples.tolist()}
    measure.append(item)
    print(item['id'],round(item['theta_deg'],3),'tip',round(floor,3),'end',endfwd['event'],'D',round(item['bay_depth_in_scan_normal_mm'],3) if item['bay_depth_in_scan_normal_mm'] is not None else None,'mouth',round(item['mouth_width_before_outlet_mm'],3) if item['mouth_width_before_outlet_mm'] is not None else None,'d.3',target_width_depth['0.3'],'w@.1',necks['0.1'])

# A serialised exact section chord is retained for every raw warning, without clustering it away.
data['warnings']=measure
data['method']['scan_normal_step_mm']=.002
data['method']['supported_opening_definition']='Exact chord between the two metal sides, tracked parallel to the original raster scan while moving its line toward the exterior. Tip is where the bounded sand interval closes; mouth is where one side disappears and the same interval opens to unbounded exterior. Finite widths are not a sand-strength proof.'
(OUT/'measurements.json').write_text(json.dumps(data,indent=2)+'\n')

plt.rcParams.update({'font.family':'DejaVu Sans','font.size':9,'axes.spines.top':False,'axes.spines.right':False,'savefig.facecolor':'white'})
metal='#b6a06d';line='#453819';warn='#bd2633';scan='#087c9d'

# Extract closed contours for fill and preserve topology as even/odd paths.
def paths_from_segments(s):
    q=np.round(s.reshape(-1,2),7)
    nodes,inv=np.unique(q,axis=0,return_inverse=True)
    edges=inv.reshape(-1,2);adj={}
    for i,(a,b) in enumerate(edges):adj.setdefault(a,[]).append((b,i));adj.setdefault(b,[]).append((a,i))
    unused=set(range(len(edges)));paths=[]
    while unused:
        e=next(iter(unused));a,b=edges[e];chain=[a,b];unused.remove(e)
        while chain[-1]!=chain[0]:
            choices=[(other,idx) for other,idx in adj[chain[-1]] if idx in unused]
            if not choices:break
            other,idx=choices[0];chain.append(other);unused.remove(idx)
        paths.append(nodes[chain])
    return paths
paths={z:paths_from_segments(s) for z,s in sections.items()}

def draw_section(ax,w,range_mm=1.05,labels=True):
    z=w['raw']['point'][2];p=np.array(w['raw']['point'][:2]);s=sections[z]
    d=np.array([1.,0.]) if w['scan_axis']=='x' else np.array([0.,1.]);n=np.array(w['outward_scan_normal'])
    # Axes follow the original raster scan and its measured outward normal.
    trans=lambda x:np.stack([(x-p)@d,(x-p)@n],axis=-1)
    local=trans(s)
    tx=np.linspace(-range_mm,range_mm,220);ty=np.linspace(-range_mm,range_mm,220)
    xx,yy=np.meshgrid(tx,ty);world=p+xx.reshape(-1,1)*d+yy.reshape(-1,1)*n
    occupied=np.zeros(len(world),dtype=bool)
    for poly in paths[z]:occupied^=MPath(poly).contains_points(world)
    ax.contourf(xx,yy,occupied.reshape(xx.shape),levels=[.5,1.5],colors=[metal],alpha=.7)
    ax.add_collection(LineCollection(local,colors=line,linewidths=.8))
    a,b=w['nearest_bounds_mm'];ax.plot([a,b],[0,0],color=scan,lw=2);ax.scatter([0],[0],c=warn,s=14,zorder=5)
    samples=np.array(w['samples_offset_left_right_mm']);show=samples[np.abs(samples[:,0])<range_mm]
    ax.plot(.5*(show[:,1]+show[:,2]),show[:,0],color=scan,linestyle=':',lw=.8)
    tip=w['tip_offset_from_warning_mm'];ax.axhline(tip,color='#666',ls='--',lw=.6)
    for depth in [.1,.3]:
        v=tip+depth
        idx=np.argmin(np.abs(samples[:,0]-v));row=samples[idx]
        if abs(row[0]-v)<.004:ax.plot(row[1:], [row[0]]*2, color='#247337',alpha=.8,lw=1)
    ax.set(xlim=(-range_mm,range_mm),ylim=(-range_mm,range_mm),aspect='equal')
    if labels:ax.set(xlabel=f'{w["scan_axis"]}-scan direction (mm)',ylabel='Toward opening (mm)')
    ax.grid(alpha=.15,lw=.5)
    ax.set_title(f'{w["id"]}  {w["theta_deg"]:.2f}°  z={z:.3f}\nraw {w["raw"]["width_mm"]:.3f}; exact {w["exact_chord_mm"]:.3f} mm',fontsize=8)

for page,start in enumerate(range(0,len(measure),8),1):
    fig,axes=plt.subplots(2,4,figsize=(13,7.2),layout='constrained')
    for ax,w in zip(axes.flat,measure[start:start+8]):draw_section(ax,w)
    for ax in list(axes.flat)[len(measure[start:start+8]):]:ax.axis('off')
    fig.suptitle(f'Draco — exact casting-pattern sections, raw warnings {start+1}–{min(start+8,len(measure))}\nGold = metal; white = sand; red = raster warning; blue = exact chord / tracked centre; green = opening at tip +0.10 and +0.30 mm',fontsize=12)
    fig.savefig(OUT/f'contact-{page}.png',dpi=180);fig.savefig(OUT/f'contact-{page}.pdf');plt.close(fig)

explicit=['f07','f08','f05','f04']
fig,axes=plt.subplots(2,4,figsize=(14,7),layout='constrained')
for col,key in enumerate(explicit):
    w=next(x for x in measure if x['id']==key);draw_section(axes[0,col],w,range_mm=.8)
    samples=np.array(w['samples_offset_left_right_mm']);depth=samples[:,0]-w['tip_offset_from_warning_mm'];width=samples[:,2]-samples[:,1]
    ax=axes[1,col];ax.plot(depth,width,color=scan,lw=1.6);ax.axhline(.3,c=warn,ls='--',lw=.8,label='0.30 mm detail floor');ax.axhline(.6,c='#6f648b',ls=':',lw=.8,label='0.60 mm recipe sand-web flag')
    ax.set(xlabel='Depth from closed tip toward outlet (mm)',ylabel='Exact supported chord (mm)',xlim=(0,.20),ylim=(0,.65));
    ax.axvline(w['bay_depth_in_scan_normal_mm'],color='#888',ls=':',lw=.7);
    ax.annotate('open',xy=(w['bay_depth_in_scan_normal_mm'],.61),xytext=(w['bay_depth_in_scan_normal_mm'],max(.33,width[-1]+.07)),ha='center',fontsize=8,arrowprops={'arrowstyle':'->','color':scan},color=scan);ax.grid(alpha=.2);ax.set_title(f'{key}: opens to exterior after {w["bay_depth_in_scan_normal_mm"]:.3f} mm' if w['bay_depth_in_scan_normal_mm'] else key,fontsize=9)
axes[1,0].legend(fontsize=7)
fig.suptitle('Four disputed crest warnings — same shrink-compensated casting mesh\nWidths at the raster line are real chords; the profiles distinguish a thin terminal tip from a sustained neck.',fontsize=13)
fig.savefig(OUT/'four-correlated-sections.png',dpi=200);fig.savefig(OUT/'four-correlated-sections.pdf');plt.close(fig)
print('Plots saved',OUT)
