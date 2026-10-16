import numpy as np
for name in ['center', 'female', 'snake']:
    d = np.load(f'motif_{name}.npz'); V = d['V'].astype('<f8'); F = d['F'].astype('<u4')
    with open(f'motif_{name}.bin', 'wb') as f:
        f.write(np.array([len(V), len(F)], '<u8').tobytes()); f.write(V.tobytes()); f.write(F.tobytes())
    print(name, len(V), len(F), f'h {V[:,2].min():.2f}..{V[:,2].max():.2f}')
