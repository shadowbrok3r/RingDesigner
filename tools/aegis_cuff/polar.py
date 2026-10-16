import sys, numpy as np
sys.path.insert(0, sys.argv[1])
from load import load_welded
P, F = load_welded(sys.argv[2], sys.argv[3])
x, y, z = P.T
m = (z > 15) & (z < 35)
# coarse centre from bbox, then refine with innermost points per degree
cx, cy = 0.5*(x.min()+x.max()), 0.5*(y.min()+y.max())
for it in range(3):
    th = np.degrees(np.arctan2(y[m]-cy, x[m]-cx)) % 360; r = np.hypot(x[m]-cx, y[m]-cy)
    pts = []
    for a in range(360):
        k = (th >= a) & (th < a+1)
        if k.sum() < 20: continue
        i = np.argmin(r[k]); pts.append((x[m][k][i], y[m][k][i]))
    Q = np.array(pts)
    A = np.c_[2*Q[:,0], 2*Q[:,1], np.ones(len(Q))]; b = (Q**2).sum(1)
    cx, cy, cc = np.linalg.lstsq(A, b, rcond=None)[0]; R = np.sqrt(cc+cx*cx+cy*cy)
    res = np.hypot(Q[:,0]-cx, Q[:,1]-cy) - R
    print(f'iter {it}: centre ({cx:.3f},{cy:.3f}) R_in {R:.3f} resid {res.min():.3f}..{res.max():.3f} n {len(Q)}')
th = np.degrees(np.arctan2(y-cy, x-cx)) % 360; r = np.hypot(x-cx, y-cy)
h, _ = np.histogram(th, bins=72, range=(0, 360))
print('vertex count per 5 deg:'); print(' '.join(f'{int(a*5)}:{c}' for a, c in enumerate(h)))
for a in range(0, 360, 10):
    k = (th >= a) & (th < a+10) & (z > 20) & (z < 30)
    if k.sum() == 0: print(f'{a:3d}: empty'); continue
    print(f'{a:3d}: r in {np.percentile(r[k],1):.2f} med {np.median(r[k]):.2f} out p99 {np.percentile(r[k],99):.2f} max {r[k].max():.2f}  z {z[(th>=a)&(th<a+10)].min():.2f}..{z[(th>=a)&(th<a+10)].max():.2f}')
np.save('centre.npy', np.array([cx, cy, R]))
