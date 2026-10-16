import sys, json, numpy as np, warnings
warnings.filterwarnings('ignore')
sys.path.insert(0, __import__('os').path.dirname(__import__('os').path.abspath(__file__)))
from cuff import *
from shade import hillshade
from PIL import Image
meta = json.load(open('target_relief.json')); T = np.load('target_relief.npy').astype(np.float64)
G = meta['ds']; s0, z0 = meta['s0'], meta['z0']
P = T[::2, ::2].copy(); PG = 2*G
HE = L['heroes']; SINK = L['sink_mm']; FL = L['levels']['panel_floor']
for name, c in HE.items():
    m = np.load(f'motif_{name}.npz'); V = m['V']
    st = c['dst'][0] + c['k']*(V[:, 0] - c['src'][0]); zt = c['dst'][1] + c['k']*(V[:, 1] - c['src'][1])
    ht = FL - SINK + c['kh']*V[:, 2]
    i = np.clip(np.round((st - s0)/PG).astype(int), 0, P.shape[1]-1); j = np.clip(np.round((zt - z0)/PG).astype(int), 0, P.shape[0]-1)
    np.maximum.at(P, (j, i), ht)
    print(name, f'target s {st.min():.1f}..{st.max():.1f} z {zt.min():.1f}..{zt.max():.1f} h max {ht.max():.2f}')
S, Z = np.meshgrid(s0 + np.arange(P.shape[1])*PG, z0 + np.arange(P.shape[0])*PG)
outside = np.abs(Z) > hw(S.ravel()).reshape(S.shape)
sh = hillshade(P, PG)
sh[outside] = 0.08
img = (np.clip(sh, 0, 1)*255).astype(np.uint8)[::-1]
im = Image.fromarray(img)
im.save('preview_full.png')
i0 = int((-50 - s0)/PG); i1 = int((50 - s0)/PG)
im.crop((i0, 0, i1, img.shape[0])).save('preview_front.png')
print(im.size)
