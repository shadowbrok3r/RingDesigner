import sys, json, numpy as np, warnings
warnings.filterwarnings('ignore')
from scipy import ndimage as ndi
from PIL import Image, ImageDraw
d = np.load('raster.npz'); ds = float(d['ds']); s0 = float(d['s0'])
relief = np.load('relief.npy').astype(np.float64)
nz, ns = relief.shape
H = ndi.grey_closing(np.where(np.isfinite(relief), relief, -3.0), size=3)
polys = json.load(open('polys_full.json'))
def poly_raster(pts):
    im = Image.new('L', (ns, nz), 0)
    ImageDraw.Draw(im).polygon([((x-s0)/ds, zz/ds) for x, zz in pts], fill=1)
    return np.array(im, bool)
def ij(x, zz): return int(round(zz/ds)), int(round((x-s0)/ds))
disk = lambda rad: (np.hypot(*np.mgrid[-rad:rad+1, -rad:rad+1]) <= rad)
sp = poly_raster(polys['snake'])
lab, _ = ndi.label((H > 0.45) & sp)
body = np.isin(lab, list({lab[ij(*p)] for p in [(22, 22), (26, 36), (14, 30)]} - {0}))
lab2, _ = ndi.label((H > -0.35) & sp)
low = np.isin(lab2, list({lab2[ij(*p)] for p in [(22, 22)]} - {0}))
S_, Z_ = np.meshgrid(s0 + np.arange(ns)*ds, np.arange(nz)*ds)
tongue = low & (((S_ > 26.0) & (Z_ > 30.5) & (Z_ < 36.8)) | ((S_ > 12.0) & (S_ < 19.5) & (Z_ > 5.9) & (Z_ < 11.5)))
snake_c = ndi.binary_fill_holes(ndi.binary_dilation(body, disk(17)) | ndi.binary_dilation(tongue, disk(6)))
masks = {}
masks['snake'] = ndi.binary_fill_holes(snake_c & sp)
masks['female'] = ndi.binary_fill_holes(poly_raster(polys['female']))
left = poly_raster(polys['center']) & ~masks['female'] & (S_ <= 0.0)
i_mid = (0.0 - s0)/ds
cols = np.arange(ns)
mir = np.clip(np.round(2*i_mid - cols).astype(int), 0, ns-1)
centre = left | left[:, mir]
masks['center'] = ndi.binary_fill_holes(centre)
masks['snake'] = ndi.binary_fill_holes((masks['snake'] & ~ndi.binary_dilation(centre, disk(2))) | (body & masks['snake']))
for k, m in masks.items():
    np.save(f'mask_{k}.npy', m)
    print(k, f'{m.sum()*ds*ds:.0f} mm2')
