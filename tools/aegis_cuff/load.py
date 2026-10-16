import numpy as np
def load_welded(path, cache):
    import os
    if os.path.exists(cache):
        d = np.load(cache)
        return d['P'], d['F']
    with open(path, 'rb') as f:
        f.read(80); n = np.frombuffer(f.read(4), dtype='<u4')[0]
        dt = np.dtype([('n', '<f4', 3), ('v', '<f4', (3, 3)), ('a', '<u2')])
        d = np.frombuffer(f.read(n * 50), dtype=dt, count=n)
    V = d['v'].reshape(-1, 3).astype(np.float64)
    q = np.round(V / 1e-5).astype(np.int64)
    u, idx, inv = np.unique(q, axis=0, return_index=True, return_inverse=True)
    P = V[idx]; F = inv.reshape(-1, 3).astype(np.int32)
    np.savez(cache, P=P, F=F)
    return P, F
