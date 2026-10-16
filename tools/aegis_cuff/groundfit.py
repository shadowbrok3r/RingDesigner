import sys, numpy as np, warnings
warnings.filterwarnings('ignore')
from PIL import Image
d = np.load('raster.npz'); rmax = d['rmax'].astype(np.float64); ds = float(d['ds']); s0 = float(d['s0'])
rmax[~np.isfinite(rmax)] = np.nan
nz, ns = rmax.shape
B = 10
nzb, nsb = nz//B, ns//B
blk = rmax[:nzb*B, :nsb*B].reshape(nzb, B, nsb, B).transpose(0, 2, 1, 3).reshape(nzb, nsb, B*B)
cnt = np.isfinite(blk).sum(-1)
p10, p50, p90 = [np.nanpercentile(np.where(cnt[..., None] > 60, blk, np.nan), q, axis=-1) for q in (10, 50, 90)]
sc = s0 + (np.arange(nsb)+0.5)*B*ds; zc = (np.arange(nzb)+0.5)*B*ds
S, Z = np.meshgrid(sc, zc)
flat = np.isfinite(p50) & ((p90 - p10) < 0.3)
print('flat blocks', flat.sum(), 'of', np.isfinite(p50).sum())
xs, zs, ys = S[flat]/47.0, (Z[flat]-25)/25.0, p50[flat]
def basis(x, z):
    return np.stack([x**i * z**j for i in range(7) for j in range(4)], -1)
A = basis(xs, zs)
w = np.ones_like(ys)
coef = np.linalg.lstsq(A, ys, rcond=None)[0]
for it in range(15):
    res = ys - A@coef
    sig = 1.4826*np.median(np.abs(res[w > 0.5])) if (w > 0.5).any() else 0.2
    w = np.where(np.abs(res) < max(2.5*sig, 0.12), 1.0, 0.0)
    coef = np.linalg.lstsq(A*w[:, None], ys*w, rcond=None)[0]
res = ys - A@coef
print('kept %.2f  |res| p50 %.3f p90 %.3f  sigma %.3f' % ((w == 1).mean(), np.median(np.abs(res[w == 1])), np.percentile(np.abs(res[w == 1]), 90), sig))
np.save('ground_coef.npy', coef)
sf = s0 + np.arange(ns)*ds; zf = np.arange(nz)*ds
G = np.empty((nz, ns))
for j0 in range(0, nz, 100):
    SS, ZZ = np.meshgrid(sf/47.0, (zf[j0:j0+100]-25)/25.0)
    G[j0:j0+100] = basis(SS, ZZ)@coef
np.save('ground.npy', G.astype(np.float32))
h = rmax - G
np.save('relief.npy', h.astype(np.float32))
for (name, s, z) in [('ground near snake', 33, 26), ('ground left of female', -33, 34), ('ground below trident', -5, 38), ('helmet face', 0, 22), ('snake coil', 22, 22), ('female cheek', -22, 30), ('acanthus', -33, 14), ('frieze', 20, 3), ('key', 20, 6.5), ('trident prong', 0, 46), ('rim top', 20, 44.6), ('rim bottom', 20, 0.8)]:
    i, j = int((s-s0)/ds), int(z/ds)
    print(f'{name:22s} h p10 {np.nanpercentile(h[j-6:j+6, i-6:i+6],10):5.2f} p50 {np.nanpercentile(h[j-6:j+6, i-6:i+6],50):5.2f} p90 {np.nanpercentile(h[j-6:j+6, i-6:i+6],90):5.2f}')
hh = np.clip(np.nan_to_num(h, nan=-1), -0.5, 7)
img = ((np.clip(hh+0.5, 0, 7)/7)**0.6*255).astype(np.uint8)[::-1]
Image.fromarray(img).save('relief_gray.png')
