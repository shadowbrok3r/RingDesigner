import numpy as np
from PIL import Image
def fill_nan(h, it=3):
    h = h.copy()
    for _ in range(it):
        bad = ~np.isfinite(h)
        if not bad.any(): break
        p = np.pad(h, 1, constant_values=np.nan)
        nb = np.stack([p[:-2,1:-1], p[2:,1:-1], p[1:-1,:-2], p[1:-1,2:]])
        with np.errstate(all='ignore'):
            m = np.nanmax(np.where(np.isfinite(nb), nb, -np.inf), 0)
        h[bad & np.isfinite(m) & (m > -np.inf)] = m[bad & np.isfinite(m) & (m > -np.inf)]
    return h
def hillshade(h, ds, light=(-0.45, 0.55, 0.7), exag=1.0):
    hh = np.where(np.isfinite(h), h, np.nan)
    base = np.nanmin(hh)
    g = np.nan_to_num(hh, nan=base)
    gy, gx = np.gradient(g*exag, ds)
    l = np.array(light); l = l/np.linalg.norm(l)
    n = np.stack([-gx, -gy, np.ones_like(gx)], -1); n /= np.linalg.norm(n, axis=-1, keepdims=True)
    s = np.clip(n@l, 0, 1)
    s[~np.isfinite(hh)] = 0
    return s
def save(s, path, flip=True):
    img = (np.clip(s, 0, 1)*255).astype(np.uint8)
    if flip: img = img[::-1]
    Image.fromarray(img).save(path)
