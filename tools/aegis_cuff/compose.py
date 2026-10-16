import sys, os, json, time, numpy as np, warnings
warnings.filterwarnings('ignore')
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cuff import *
from scipy import ndimage as ndi
from PIL import Image
t0 = time.time()
B, LV, F, SD, HE = L['border2'], L['levels2'], L['field'], L['sides'], L['heroes']
G = L['grid_mm']; SINK = L['sink_mm']
s_ax = np.arange(-94.2, 94.2 + G/2, G); z_ax = np.arange(-25.3, 25.3 + G/2, G)
NS, NZ = len(s_ax), len(z_ax)
FIELD_IN = B['ridge'][1]
PANEL_HALF = 0.5*BAND - FIELD_IN
u = np.arange(0, 94.3, 0.005)
dh = np.gradient(hw(u), u)
tab_t = np.concatenate([[0], np.cumsum(0.5*(np.sqrt(1+dh[1:]**2) + np.sqrt(1+dh[:-1]**2))*np.diff(u))])
def t_of(s): return np.interp(np.abs(s), u, tab_t)
def edge_proj(s, za):
    ss = s.copy()
    for _ in range(4):
        e = 1e-3
        h0 = hw(ss); hp = hw(ss+e); hm = hw(ss-e)
        d1 = (hp - hm)/(2*e); d2 = (hp - 2*h0 + hm)/(e*e)
        f = (ss - s) + (h0 - za)*d1
        fp = 1 + d1*d1 + (h0 - za)*d2
        ss = ss - f/np.where(np.abs(fp) > 1e-6, fp, 1e-6)
    return ss, np.sqrt((ss - s)**2 + (hw(ss) - za)**2)*np.sign(hw(ss) - za)
# greek key: running hook off a continuous baseline, 6 units per period, 7 units across
KR, KP = B['key_rows'], B['period_units']
UK = (B['key'][1] - B['key'][0])/KR
SEGS = [(0.0, 1.5, 6.0, 1.5), (0.5, 1.5, 0.5, 5.5), (0.5, 5.5, 4.5, 5.5), (4.5, 5.5, 4.5, 3.5), (4.5, 3.5, 2.5, 3.5)]
def key_ink(x, y):
    xm = np.mod(x, KP)
    ink = np.zeros(x.shape, bool)
    for off in (-KP, 0.0, KP):
        for (x0, y0, x1, y1) in SEGS:
            ink |= (xm - off >= min(x0, x1) - 0.55) & (xm - off <= max(x0, x1) + 0.55) & (y >= min(y0, y1) - 0.55) & (y <= max(y0, y1) + 0.55)
    return ink
def seg_params(c):
    t_a = t_of(np.array([c]))[0] + B['terminal_units']*UK
    t_b = t_of(np.array([S_END - B['end_plate']]))[0] - B['terminal_units']*UK
    n = max(1, int(round((t_b - t_a)/(KP*UK))))
    return t_a, t_b, (t_b - t_a)/(n*KP)
KEYSEG = {True: seg_params(B['center_top']), False: seg_params(B['center_bottom'])}
T_CEN = {True: float(t_of(np.array([B['center_top']]))[0]), False: float(t_of(np.array([B['center_bottom']]))[0])}
# hero footprints in the target, for the moat
foot = np.zeros((NZ, NS), bool)
for name, c in HE.items():
    m = np.load(f'motif_{name}.npz'); V = m['V']
    up = (LV['panel_floor'] - SINK + c['kh']*V[:, 2]) > LV['panel_floor'] + 0.08
    st = c['dst'][0] + c['k']*(V[up, 0] - c['src'][0]); zt = c['dst'][1] + c['k']*(V[up, 1] - c['src'][1])
    i = np.clip(np.round((st - s_ax[0])/G).astype(int), 0, NS-1); j = np.clip(np.round((zt - z_ax[0])/G).astype(int), 0, NZ-1)
    foot[j, i] = True
foot = ndi.binary_fill_holes(ndi.binary_closing(foot, iterations=4))
moat = ndi.binary_dilation(foot, iterations=int(round(F['margin_mm']/G)))
print(f'figure footprints {foot.sum()*G*G:.0f} mm2, with moat {moat.sum()*G*G:.0f} mm2', flush=True)
# fill textures
def load_h(path, rows=None, gp=12, tp=99.7):
    a = np.array(Image.open(path)).astype(np.float64)
    if a.ndim == 3: a = a[..., 0]
    band = a[rows[0]:rows[1]] if rows else a
    g0, g1 = np.percentile(band, gp), np.percentile(band, tp)
    return np.clip((a - g0)/(g1 - g0), 0, 1.05)
TILE = load_h(F['png'], None, F['ground_pct'], F['top_pct'])
STR = {}
for side in ('left', 'right'):
    cfg = SD[side]; r0, r1 = cfg['rows']
    STR[side] = (load_h(cfg['png'], cfg['rows'], SD['ground_pct'], SD['top_pct']), r0, cfg['centre_px'], 2*PANEL_HALF/(r1 - r0))
AMP = (LV['panel_top'] - LV['panel_floor'])*1.1
out = np.zeros((NZ, NS), np.float32); FILL = np.zeros((NZ, NS), np.float32); CF = np.zeros((NZ, NS), np.float32)
SUB = [(-0.33, -0.33), (-0.33, 0.33), (0.33, -0.33), (0.33, 0.33), (0.0, 0.0)]
CH = 120
for j0 in range(0, NZ, CH):
    S, Z = np.meshgrid(s_ax, z_ax[j0:j0+CH]); za = np.abs(Z); top = Z >= 0; sa = np.abs(S)
    inside = za <= hw(S); endp = sa > S_END - B['end_plate']
    cov = {k: np.zeros(S.shape) for k in ('rail', 'ridge', 'keyink', 'keyband', 'field')}
    for (dss, dzz) in SUB:
        Sq, Zq = S + dss*G, za + dzz*G
        sp, d = edge_proj(Sq, Zq)
        tq = t_of(sp)
        cen = np.where(top, tq < T_CEN[True], tq < T_CEN[False])
        rail = d < B['rail']
        band = ~cen & ~rail & (d < B['key'][1])
        ridge = ~cen & (d >= B['ridge'][0]) & (d < B['ridge'][1])
        ta = np.where(top, KEYSEG[True][0], KEYSEG[False][0]); tb = np.where(top, KEYSEG[True][1], KEYSEG[False][1])
        uk = np.where(top, KEYSEG[True][2], KEYSEG[False][2])
        x = (tq - ta)/uk; y = (d - B['key'][0])/UK
        term = band & ((tq < ta) | (tq > tb))
        ink = band & ~term & key_ink(x, y)
        cov['rail'] += rail
        cov['ridge'] += ridge | term
        cov['keyink'] += ink
        cov['keyband'] += band & ~term & ~ink
        cov['field'] += ~rail & ~band & ~ridge
    for k in cov: cov[k] /= len(SUB)
    h = cov['rail']*LV['rail'] + cov['ridge']*LV['ridge'] + cov['keyink']*LV['key_top'] + cov['keyband']*LV['key_floor'] + cov['field']*LV['panel_floor']
    # field fill: tiled acanthus in front, the generated strips on the sides, nothing in the moat
    fill = np.zeros(S.shape)
    front = sa < SD['start']
    fill[front] = ndi.map_coordinates(TILE, [np.mod(Z[front]/F['tile_mm'], 1.0)*(TILE.shape[0]-1), np.mod(np.abs(S[front])/F['tile_mm'], 1.0)*(TILE.shape[1]-1)], order=1, mode='wrap')
    for side, sg in (('left', -1.0), ('right', 1.0)):
        hgt, r0, cx, mmpx = STR[side]
        lo_, hi_ = SD['start'] + SD['bar'], S_END - B['end_plate']
        k = (sg*S >= lo_) & (sg*S <= hi_)
        mid = 0.5*(lo_ + hi_)
        xx = cx + (sg*S[k] - mid)/mmpx; yy = r0 + (PANEL_HALF - Z[k])/mmpx
        fe = np.clip((sg*S[k] - lo_)/1.2, 0, 1)*np.clip((hi_ - sg*S[k])/1.2, 0, 1)
        fill[k] = ndi.map_coordinates(hgt, [yy, np.mod(xx, hgt.shape[1])], order=1, mode='nearest')*fe
    sbar = (sa >= SD['start']) & (sa < SD['start'] + SD['bar'])
    fill[sbar] = 0
    fill *= ~moat[j0:j0+CH]
    FILL[j0:j0+CH] = fill*AMP
    CF[j0:j0+CH] = np.where(endp | ~inside, 0, cov['field']*(~sbar))
    h = np.where(sbar & (cov['field'] > 0), h + cov['field']*(LV['ridge'] - LV['panel_floor']), h)
    out[j0:j0+CH] = np.where(endp | ~inside, LV['rail'], h)
    if j0 % 600 == 0: print(f'row {j0}/{NZ} {time.time()-t0:.0f}s', flush=True)
def slope_limit(A, tan_a, px):
    r = 2
    yy, xx = np.mgrid[-r:r+1, -r:r+1]
    st = (-tan_a*px*np.hypot(yy, xx)).astype(np.float32)
    o = A.astype(np.float32)
    for _ in range(40):
        nx = ndi.grey_dilation(o, structure=st)
        if float((nx - o).max()) < 1e-4: break
        o = np.maximum(o, nx)
    return o
FILL = slope_limit(np.minimum(FILL, AMP*0.95), np.tan(np.radians(68)), G)
FILL = (LV['panel_top'] - LV['panel_floor'])*np.tanh(FILL/(LV['panel_top'] - LV['panel_floor']))
out = out + CF*FILL
rng = np.random.default_rng(7)
noise = ndi.gaussian_filter(rng.standard_normal(out.shape).astype(np.float32), 1.6); noise /= noise.std()
floors = np.zeros(out.shape, np.float32)
for lvl in (LV['panel_floor'], LV['key_floor']):
    floors = np.maximum(floors, np.clip(1.0 - np.abs(out - lvl)/0.03, 0, 1))
floors = ndi.gaussian_filter(floors, 1.0)
out = out + floors*(0.035*noise - 0.03)
np.save('target_relief.npy', out)
json.dump({'s0': float(s_ax[0]), 'z0': float(z_ax[0]), 'ds': G, 'ns': NS, 'nz': NZ}, open('target_relief.json', 'w'))
out.astype('<f4').tofile('target_relief.f32')
print('done', f'{time.time()-t0:.0f}s', 'range', float(out.min()), float(out.max()), 'key periods', [round((v[1]-v[0])/(KP*v[2])) for v in KEYSEG.values()])
