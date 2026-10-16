import sys, json, time, numpy as np, warnings
warnings.filterwarnings('ignore')
sys.path.insert(0, sys.argv[1])
from load import load_welded
from scipy import ndimage as ndi
from scipy.sparse import coo_matrix
from scipy.sparse.csgraph import connected_components
from skimage.measure import marching_cubes
name = sys.argv[4]; DV = float(sys.argv[5]); TM = float(sys.argv[6])
t0 = time.time()
P, F = load_welded(sys.argv[2], sys.argv[3])
cx, cy, _ = np.load('centre.npy')
d = np.load('raster.npz'); ds = float(d['ds']); s0 = float(d['s0']); RREF = float(d['rref'])
coef = np.load('ground_coef.npy')
def basis(x, z): return np.stack([x**i * z**j for i in range(7) for j in range(4)], -1)
th = (np.degrees(np.arctan2(P[:,1]-cy, P[:,0]-cx)) - 175.0) % 360 + 175.0
r = np.hypot(P[:,0]-cx, P[:,1]-cy); z = P[:,2]; s = np.radians(th-270.0)*RREF
pmask = np.load(f'mask_{name}.npy')
jj, ii = np.nonzero(pmask)
M = 1.2
s_lo, s_hi = s0 + ii.min()*ds - M, s0 + ii.max()*ds + M
z_lo, z_hi = jj.min()*ds - M, jj.max()*ds + M
near = (s > s_lo-0.3) & (s < s_hi+0.3) & (z > z_lo-0.3) & (z < z_hi+0.3)
g = np.zeros(len(P)); g[near] = basis(s[near]/47.0, (z[near]-25)/25.0)@coef
h = r - g
T = F[near[F].all(1)]
H_LO = -3.0
H_HI = float(h[np.unique(T)].max()) + 0.4
nsc, nzc, nh = int((s_hi-s_lo)/DV), int((z_hi-z_lo)/DV), int((H_HI-H_LO)/DV)
JS, JZ = 0.3183, 0.2718
SYM = name == 'center'
if SYM:
    half = int(np.ceil(max(-s_lo, s_hi)/DV)) + 2
    nsc = 2*half
    s_lo = -(half + 0.5 + JS)*DV + 0.5*DV
print(f'{name}: grid {nsc} x {nzc} x {nh} = {nsc*nzc*nh/1e6:.0f} M voxels, {len(T)} triangles')
S3, Z3, R3 = s[T], z[T], h[T]
fs = (S3 - s_lo)/DV - 0.5 - JS; fz = (Z3 - z_lo)/DV - 0.5 - JZ
i0 = np.ceil(fs.min(1)).astype(int); i1 = np.floor(fs.max(1)).astype(int)
j0 = np.ceil(fz.min(1)).astype(int); j1 = np.floor(fz.max(1)).astype(int)
wi, wj = (i1-i0).max()+1, (j1-j0).max()+1
cols, hv = [], []
ax, ay = fs[:,1]-fs[:,0], fz[:,1]-fz[:,0]; bx, by = fs[:,2]-fs[:,0], fz[:,2]-fz[:,0]
det = ax*by - ay*bx
good = np.abs(det) > 1e-14
dd = np.where(good, det, 1)
for di in range(wi):
    for dj in range(wj):
        ci, cj = i0+di, j0+dj
        k = good & (ci <= i1) & (cj <= j1) & (ci >= 0) & (cj >= 0) & (ci < nsc) & (cj < nzc)
        qx, qy = ci - fs[:,0], cj - fz[:,0]
        u = (qx*by - qy*bx)/dd; v = (ax*qy - ay*qx)/dd
        k &= (u >= 0) & (v >= 0) & (u+v <= 1)
        cols.append(cj[k]*nsc + ci[k]); hv.append(R3[k,0] + u[k]*(R3[k,1]-R3[k,0]) + v[k]*(R3[k,2]-R3[k,0]))
cols = np.concatenate(cols); hv = np.concatenate(hv)
o = np.lexsort((hv, cols)); cols, hv = cols[o], hv[o]
kk = np.clip(np.floor((hv - H_LO)/DV - 0.5).astype(int) + 1, 0, nh)
A = np.zeros((nzc*nsc, nh+1), np.uint8)
np.add.at(A, (cols, kk), 1)
mat = (np.cumsum(A[:, ::-1], axis=1)[:, ::-1][:, 1:] & 1).astype(bool).reshape(nzc, nsc, nh)
del A
print(f'hits {len(hv)}, {time.time()-t0:.0f}s')
seal = ndi.binary_dilation(mat, iterations=2)
visible = np.zeros(seal.shape, bool)
sc = s_lo + (np.arange(nsc)+0.5+JS)*DV; zc = z_lo + (np.arange(nzc)+0.5+JZ)*DV
S2, Z2 = np.meshgrid(sc, zc)
helm = (np.abs(S2) < 6.8) & (Z2 > 8.0) & (Z2 < 31.0) if name == 'center' else np.zeros(S2.shape, bool)
for (dsx, dzx) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)]:
    cur = np.zeros((nzc, nsc), bool)
    for m in range(nh-1, -1, -1):
        if dsx or dzx:
            sh = np.ones_like(cur)
            ys = slice(max(dzx, 0), nzc + min(dzx, 0)); yd = slice(max(-dzx, 0), nzc + min(-dzx, 0))
            xs = slice(max(dsx, 0), nsc + min(dsx, 0)); xd = slice(max(-dsx, 0), nsc + min(-dsx, 0))
            sh[yd, xd] = cur[ys, xs]; cur = sh
        cur = cur | seal[:, :, m]
        visible[:, :, m] |= ~cur if not (dsx or dzx) else (~cur & ~helm)
del seal
solid = ~(ndi.binary_dilation(visible, iterations=2) & ~mat)
del visible
hc = H_LO + (np.arange(nh)+0.5)*DV
anys = solid.any(2)
topm = nh - 1 - np.argmax(solid[:, :, ::-1], axis=2)
topz = np.where(anys, hc[topm] + 0.5*DV, H_LO)
sc = s_lo + (np.arange(nsc)+0.5+JS)*DV; zc = z_lo + (np.arange(nzc)+0.5+JZ)*DV
mi = np.clip(np.round((sc - s0)/ds).astype(int), 0, pmask.shape[1]-1); mj = np.clip(np.round(zc/ds).astype(int), 0, pmask.shape[0]-1)
poly = pmask[np.ix_(mj, mi)]
def ground_from(mask):
    dist = ndi.distance_transform_edt(mask)*DV
    rim = mask & (dist < 0.35)
    tz = ndi.median_filter(topz, size=int(1.0/DV) | 1)
    w = rim.astype(float)
    sig = 2.5/DV
    gnd = ndi.gaussian_filter(tz*w, sig) / np.maximum(ndi.gaussian_filter(w, sig), 1e-9)
    return gnd, dist
gnd, _ = ground_from(poly)
rel = topz - gnd
lab, _ = ndi.label(poly & (rel > TM)) if TM > 0 else (poly.astype(int), 1)
sizes = np.bincount(lab.ravel()); sizes[0] = 0
keep_lab = np.flatnonzero(sizes > 0.02*sizes.max())
tight = ndi.binary_fill_holes(np.isin(lab, keep_lab))
tight = ndi.binary_dilation(tight, iterations=int(0.8/DV)) & poly
tight = ndi.binary_fill_holes(tight)
tight = ndi.binary_fill_holes(ndi.binary_opening(ndi.binary_closing(tight, iterations=3), iterations=4))
gnd, dist = ground_from(tight)
lift = None
if name == 'center':
    f4 = 4
    Tc = topz[::f4, ::f4]
    kr = int(round(1.6/(DV*f4)))
    fp = np.hypot(*np.mgrid[-kr:kr+1, -kr:kr+1]) <= kr
    Tcl = ndi.grey_closing(Tc, footprint=fp)
    jj_, ii_ = np.mgrid[0:nzc, 0:nsc]
    Tcl = ndi.map_coordinates(Tcl, [jj_/f4, ii_/f4], order=1, mode='nearest')
    deep = helm & ((Tcl - topz) > 0.9)
    deep = ndi.binary_opening(deep, iterations=2)
    lift = (deep, np.where(deep, Tcl - 1.1, topz))
    print(f'helmet openings filled to recesses: {deep.sum()*DV*DV:.1f} mm2')
if name == 'female':
    S2, Z2 = np.meshgrid(sc, zc)
    def win(x, a, b, c, d):
        f = lambda t: t*t*(3 - 2*t)
        return f(np.clip((x - a)/(b - a), 0, 1))*f(np.clip((d - x)/(d - c), 0, 1))
    Lf = 1.7*win(S2, -25.6, -23.3, -18.5, -16.2)*win(Z2, 11.6, 13.9, 17.0, 22.6)
    region = (Lf > 0.02) & poly
    lift = (region, topz + Lf)
    tight = ndi.binary_fill_holes(tight | region)
    dist = ndi.distance_transform_edt(tight)*DV
print(f'tight mask {tight.sum()*DV*DV:.0f} mm2 of polygon {poly.sum()*DV*DV:.0f}; ground {np.percentile(gnd[tight],5):.2f}..{np.percentile(gnd[tight],95):.2f}')
D = 1.0; FE = 0.8
cap = gnd - 0.05 + np.tan(np.radians(68))*dist
solid &= tight[:, :, None] & (hc[None, None, :] > gnd[:, :, None] - D) & (hc[None, None, :] < cap[:, :, None])
if lift is not None:
    region, newtop = lift
    raise_cols = region & (newtop > topz)
    jr, ir = np.nonzero(raise_cols)
    col_fill = (hc[None, :] > (gnd[jr, ir] - D)[:, None]) & (hc[None, :] < newtop[jr, ir][:, None])
    solid[jr, ir] |= col_fill
    print(f'sculpted neck and chest: {raise_cols.sum()*DV*DV:.1f} mm2 raised')
occ = solid.astype(np.float32)
if lift is not None:
    nt = newtop[jr, ir]
    mtop = np.clip(np.floor((nt - H_LO)/DV - 0.5).astype(int), 0, nh-2)
    occ[jr, ir, mtop] = np.clip((nt - (hc[mtop] - 0.5*DV))/DV, 0, 1)
    occ[jr, ir, mtop+1] = np.clip((nt - (hc[mtop+1] - 0.5*DV))/DV, 0, 1)
# sub-voxel heights at vertical transitions, from the nearest exact hit
up = solid[:, :, :-1] & ~solid[:, :, 1:]
dn = ~solid[:, :, :-1] & solid[:, :, 1:]
for trans, top in [(up, True), (dn, False)]:
    jq, iq, mq = np.nonzero(trans)
    col = jq*nsc + iq
    hb = hc[mq] + 0.5*DV
    lo = np.searchsorted(cols, col, 'left'); hi_ = np.searchsorted(cols, col, 'right')
    keyc = cols.astype(np.float64)*1e3 + hv
    pos = np.searchsorted(keyc, col*1e3 + hb)
    best = np.full(len(col), np.nan)
    for p in (pos-1, pos):
        pp = np.clip(p, 0, len(hv)-1)
        ok = (pp >= lo) & (pp < hi_)
        cand = np.where(ok, hv[pp], np.nan)
        better = np.isnan(best) | (np.abs(cand - hb) < np.abs(best - hb))
        best = np.where(ok & better, cand, best)
    ok = np.isfinite(best) & (np.abs(best - hb) < DV) & (best > gnd[jq, iq] - D + DV) & (best < cap[jq, iq] - DV)
    jq, iq, mq, best = jq[ok], iq[ok], mq[ok], best[ok]
    if top:
        occ[jq, iq, mq] = np.clip((best - (hc[mq] - 0.5*DV))/DV, 0, 1)
        occ[jq, iq, mq+1] = np.clip((best - (hc[mq+1] - 0.5*DV))/DV, 0, 1)
    else:
        occ[jq, iq, mq+1] = np.clip(((hc[mq+1] + 0.5*DV) - best)/DV, 0, 1)
        occ[jq, iq, mq] = np.clip(((hc[mq] + 0.5*DV) - best)/DV, 0, 1)
del solid, up, dn
if SYM:
    occ[:, nsc//2:, :] = occ[:, :nsc//2, :][:, ::-1, :]
    gnd[:, nsc//2:] = gnd[:, :nsc//2][:, ::-1]
    tight[:, nsc//2:] = tight[:, :nsc//2][:, ::-1]
occ = ndi.gaussian_filter(occ, (0.6, 0.6, 0.35))
occ = np.pad(occ, 1)
vt, ft, _, _ = marching_cubes(occ, 0.5, spacing=(DV, DV, DV), allow_degenerate=False)
vt -= DV
nv = len(vt)
e = np.concatenate([ft[:, [0, 1]], ft[:, [1, 2]], ft[:, [2, 0]]])
nc, labv = connected_components(coo_matrix((np.ones(len(e)), (e[:, 0], e[:, 1])), shape=(nv, nv)), directed=False)
cnt = np.bincount(labv[ft[:, 0]])
keepf = cnt[labv[ft[:, 0]]] > 0.03*cnt.max()
used = np.unique(ft[keepf]); remap = -np.ones(nv, int); remap[used] = np.arange(len(used))
ft = remap[ft[keepf]]; vt = vt[used]
vs = s_lo + vt[:,1] + (0.5+JS)*DV; vz = z_lo + vt[:,0] + (0.5+JZ)*DV; vh = H_LO + vt[:,2] + 0.5*DV
gi = ndi.map_coordinates(gnd, [vt[:,0]/DV, vt[:,1]/DV], order=1, mode='nearest')
V = np.stack([vs, vz, vh - gi], -1)
np.savez(f'motif_{name}.npz', V=V, F=ft.astype(np.int32), tight=tight, gnd=gnd, box=np.array([s_lo, z_lo, DV, JS, JZ]))
print(f'components {nc}; kept {keepf.sum()} faces; relief top {V[:,2].max():.2f} mm; {time.time()-t0:.0f}s')
