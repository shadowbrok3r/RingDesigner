import sys, numpy as np
sys.path.insert(0, sys.argv[1])
from load import load_welded
P, F = load_welded(sys.argv[2], sys.argv[3])
cx, cy, R = np.load('centre.npy')
# polar per vertex; theta unwrapped so the front 175..365 is contiguous
th = (np.degrees(np.arctan2(P[:,1]-cy, P[:,0]-cx)) - 175.0) % 360 + 175.0
r = np.hypot(P[:,0]-cx, P[:,1]-cy); z = P[:,2]
T = np.stack([th, z, r], -1)[F]          # (n,3,3)
keep = (T[:,:,0].min(1) > 176) & (T[:,:,0].max(1) < 364)
T = T[keep]
bary = np.array([[1,0,0],[0,1,0],[0,0,1],[.5,.5,0],[0,.5,.5],[.5,0,.5],[1/3,1/3,1/3],
                 [2/3,1/6,1/6],[1/6,2/3,1/6],[1/6,1/6,2/3]])
S = np.einsum('kb,nbc->nkc', bary, T).reshape(-1, 3)
RREF = 30.0                               # arc-length reference radius, mm
DS = 0.05
s = np.radians(S[:,0]-270.0)*RREF         # arc mm from the front centre
ns, nz = int(2*np.radians(94)*RREF/DS)+1, int(50.5/DS)+1
s0 = -np.radians(94)*RREF
i = np.clip(((s - s0)/DS).round().astype(np.int64), 0, ns-1)
j = np.clip((S[:,1]/DS).round().astype(np.int64), 0, nz-1)
flat = j*ns + i
rmax = np.full(ns*nz, -np.inf); np.maximum.at(rmax, flat, S[:,2])
rmin = np.full(ns*nz, np.inf); np.minimum.at(rmin, flat, S[:,2])
np.savez_compressed('raster.npz', rmax=rmax.reshape(nz, ns).astype(np.float32), rmin=rmin.reshape(nz, ns).astype(np.float32), s0=s0, ds=DS, rref=RREF)
print('grid', ns, nz, 'filled', np.isfinite(rmax).mean())
