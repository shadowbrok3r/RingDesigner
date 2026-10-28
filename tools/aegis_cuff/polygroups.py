"""Labels every triangle of a ZBrush OBJ export of the Aegis trident cuff by ornament and writes an OBJ whose groups ZBrush imports as polygroups.

Usage: polygroups.py WORK_DIR IN.obj OUT.obj [PREVIEW.png]
WORK_DIR is the pipeline's work folder (layout.json, motif_*.npz, centre_split.json). The cuff may have been resized since the
pipeline ran: the bore is measured from the mesh, and arc positions are taken on the resized outer face, as RingDesigner lays them out.
"""
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw
from scipy import ndimage as ndi

GROUPS = ["band", "frame", "greek_key", "background", "florals", "woman", "snake", "helmet", "wings", "crest", "trident", "centre", "loop"]
COLOURS = {
    "band": (90, 90, 100), "frame": (200, 200, 210), "greek_key": (60, 120, 230), "background": (40, 40, 48), "florals": (70, 170, 80),
    "woman": (230, 120, 170), "snake": (150, 210, 60), "helmet": (240, 170, 40), "wings": (250, 230, 120), "crest": (230, 90, 40),
    "trident": (80, 220, 230), "centre": (200, 140, 255), "loop": (255, 255, 255),
}
GRID = 0.05
FLORAL_RISE = 0.15
FLORAL_SMOOTH = 0.2
HERO_RISE = 0.03
HERO_MARGIN = 0.25


def read_obj(path):
    """Returns the raw vertex block, vertices and zero-based triangles of a triangle OBJ."""
    head, vs, fs = [], [], []
    with open(path, "rb") as fh:
        for line in fh:
            if line.startswith(b"v "):
                vs.append(line)
            elif line.startswith(b"f "):
                fs.append(line[2:])
            elif line.startswith(b"#") and not fs:
                head.append(line)
    V = np.array(b" ".join(l[2:] for l in vs).split(), dtype=np.float64).reshape(-1, 3)
    F = np.array([t.split(b"/")[0] for t in b" ".join(fs).split()], dtype=np.int64).reshape(-1, 3) - 1
    return head, vs, V, F


class Cuff:
    """The band outline and thickness laws of cuff.py, on the bore measured from the mesh."""

    def __init__(self, c, r_bore):
        self.c = c
        self.r_bore = r_bore
        self.t0 = c["thick_mm"]
        self.r_ref = r_bore + self.t0
        self.phi_end = np.pi - np.arcsin(c["gap_mm"] / c["bore_mm"])
        self.s_end = self.phi_end * self.r_ref

    def share(self, s):
        phi = np.degrees(np.asarray(s, float) / self.r_ref)
        soft = np.sqrt(phi * phi + self.c["tip_deg"] ** 2) - self.c["tip_deg"]
        return np.clip(1 - soft / self.c["crest_deg"], 0, None) ** self.c["crest_pow"]

    def hw(self, s):
        s = np.asarray(s, float)
        out = 0.5 * (self.c["band_mm"] + (self.c["crest_mm"] - self.c["band_mm"]) * self.share(s))
        rho = self.c["end_corner_mm"]
        x = np.abs(s) - (self.s_end - rho)
        k = x > 0
        return np.where(k, out - rho + np.sqrt(np.maximum(rho * rho - np.where(k, x, 0) ** 2, 0)), out)

    def thick(self, s):
        return self.t0 + self.c["crest_rise_mm"] * self.share(s)

    def edge_proj(self, s, za):
        """Foot of the perpendicular from (s, |z|) onto the band edge, and the inward distance to it."""
        ss = s.copy()
        for _ in range(5):
            e = 1e-3
            h0, hp, hm = self.hw(ss), self.hw(ss + e), self.hw(ss - e)
            d1, d2 = (hp - hm) / (2 * e), (hp - 2 * h0 + hm) / (e * e)
            f = (ss - s) + (h0 - za) * d1
            fp = 1 + d1 * d1 + (h0 - za) * d2
            ss = ss - f / np.where(np.abs(fp) > 1e-6, fp, 1e-6)
        return ss, np.sqrt((ss - s) ** 2 + (self.hw(ss) - za) ** 2) * np.sign(self.hw(ss) - za)

    def edge_length(self):
        u = np.arange(0, self.s_end + 0.01, 0.005)
        dh = np.gradient(self.hw(u), u)
        t = np.concatenate([[0], np.cumsum(0.5 * (np.sqrt(1 + dh[1:] ** 2) + np.sqrt(1 + dh[:-1] ** 2)) * np.diff(u))])
        return lambda s: np.interp(np.abs(s), u, t)


def hero_footprints(work, layout, s_lo, z_lo, shape):
    """Rasters, on the GRID, where each figure stands above the panel floor, keyed by name."""
    out = {}
    for name, h in layout["heroes"].items():
        V = np.load(work / f"motif_{name}.npz")["V"]
        up = h["kh"] * V[:, 2] > layout["sink_mm"] + HERO_RISE
        s = h["dst"][0] + h["k"] * (V[up, 0] - h["src"][0])
        z = h["dst"][1] + h["k"] * (V[up, 1] - h["src"][1])
        m = np.zeros(shape, bool)
        i = np.clip(np.round((s - s_lo) / GRID).astype(int), 0, shape[1] - 1)
        j = np.clip(np.round((z - z_lo) / GRID).astype(int), 0, shape[0] - 1)
        m[j, i] = True
        m = ndi.binary_fill_holes(ndi.binary_closing(m, iterations=3))
        out[name] = m
    return out


def centre_split(work, s, z):
    """Sub-labels for the centre figure from polygons in centre_split.json (cuff s, z in mm); later polygons win."""
    path = work / "centre_split.json"
    lab = np.full(len(s), "helmet", dtype=object)
    if not path.exists():
        return lab
    from matplotlib.path import Path as MPath
    for part, polys in json.loads(path.read_text()).items():
        for poly in polys:
            lab[MPath(poly).contains_points(np.c_[s, z])] = part
    return lab


def label_faces(work, V, F):
    layout = json.loads((work / "layout.json").read_text())
    B, LV, SD = layout["border2"], layout["levels2"], layout["sides"]
    r_v = np.hypot(V[:, 0], V[:, 1])
    cuff = Cuff(layout["cuff"], float(np.percentile(r_v, 0.05)))
    phi_v = (np.arctan2(V[:, 1], V[:, 0]) - np.pi / 2 + np.pi) % (2 * np.pi) - np.pi
    s_v = phi_v * cuff.r_ref
    h_v = r_v - (cuff.r_bore + cuff.thick(s_v))

    P = V[F]
    sc, zc = s_v[F].mean(1), P[:, :, 2].mean(1)
    hmax = h_v[F].max(1)
    n = np.cross(P[:, 1] - P[:, 0], P[:, 2] - P[:, 0])
    n /= np.maximum(np.linalg.norm(n, axis=1, keepdims=True), 1e-12)
    cx, cy = P[:, :, 0].mean(1), P[:, :, 1].mean(1)
    rc = np.hypot(cx, cy)
    n_rad = (n[:, 0] * cx + n[:, 1] * cy) / rc
    n_tan = (-n[:, 0] * cy + n[:, 1] * cx) / rc
    hw_c = cuff.hw(sc)
    lab = np.full(len(F), "", dtype=object)

    # The band's inside, side walls and end caps.
    bore = (n_rad < -0.3) & (rc < cuff.r_bore + 0.5 * cuff.thick(sc))
    sides = (np.abs(n[:, 2]) > 0.85) & (np.abs(zc) > hw_c - 0.6)
    caps = (np.abs(sc) > cuff.s_end - 0.8) & (np.abs(n_tan) > 0.7)
    lab[bore | sides | caps] = "band"
    lab[(lab == "") & (zc > hw_c + 0.05) & (np.abs(sc) < 6)] = "loop"

    # Border zones by inward distance from the band edge, as compose.py lays them out.
    t_of = cuff.edge_length()
    sp, d = cuff.edge_proj(sc, np.abs(zc))
    tq = t_of(sp)
    top = zc >= 0
    uk = (B["key"][1] - B["key"][0]) / B["key_rows"]
    t_cen = np.where(top, t_of(np.array([B["center_top"]]))[0], t_of(np.array([B["center_bottom"]]))[0])
    cen = tq < t_cen
    t_a = t_cen + B["terminal_units"] * uk
    t_b = t_of(np.array([cuff.s_end - B["end_plate"]]))[0] - B["terminal_units"] * uk
    rail = d < B["rail"]
    keyband = ~cen & ~rail & (d < B["key"][1])
    terminal = keyband & ((tq < t_a) | (tq > t_b))
    ridge = ~cen & (d >= B["ridge"][0]) & (d < B["ridge"][1])
    endplate = np.abs(sc) > cuff.s_end - B["end_plate"]
    sidebar = (np.abs(sc) >= SD["start"]) & (np.abs(sc) < SD["start"] + SD["bar"]) & ~rail & ~keyband & ~ridge
    base = np.where(rail, LV["rail"], np.where(keyband | ridge | sidebar, LV["ridge"], LV["panel_floor"]))

    # Figures win wherever they stand above the surface beneath them.
    s_lo, z_lo = -cuff.s_end - 1, -26.0
    shape = (int(round(58 / GRID)) + 1, int(round((2 * cuff.s_end + 2) / GRID)) + 1)
    foot = hero_footprints(work, layout, s_lo, z_lo, shape)
    dist = {k: ndi.distance_transform_edt(~m) * GRID for k, m in foot.items()}
    i = np.clip(np.round((sc - s_lo) / GRID).astype(int), 0, shape[1] - 1)
    j = np.clip(np.round((zc - z_lo) / GRID).astype(int), 0, shape[0] - 1)
    names = list(foot)
    dmat = np.stack([dist[k][j, i] for k in names])
    nearest = np.array(names, dtype=object)[dmat.argmin(0)]
    hero = (lab == "") & (dmat.min(0) <= HERO_MARGIN) & (hmax > base + HERO_RISE)
    lab[hero] = nearest[hero]

    rest = lab == ""
    lab[rest & (rail | endplate | ridge | terminal | sidebar)] = "frame"
    lab[(lab == "") & keyband] = "greek_key"
    # Florals stand above the stippled floor once the stipple is smoothed away; only field faces feed the average.
    field = lab == ""
    hmean = h_v[F].mean(1)
    acc, cnt = np.zeros(shape), np.zeros(shape)
    np.add.at(acc, (j[field], i[field]), hmean[field])
    np.add.at(cnt, (j[field], i[field]), 1.0)
    sigma = FLORAL_SMOOTH / GRID
    smooth = ndi.gaussian_filter(acc, sigma) / np.maximum(ndi.gaussian_filter(cnt, sigma), 1e-9)
    floral = smooth > LV["panel_floor"] + FLORAL_RISE
    floral = ndi.binary_closing(ndi.binary_opening(floral, iterations=2), iterations=2)
    lab[field] = np.where(floral[j[field], i[field]], "florals", "background")

    centre = lab == "center"
    lab[centre] = centre_split(work, sc[centre], zc[centre])
    lab[lab == "female"] = "woman"
    return lab, sc, zc, cuff


def preview(path, lab, sc, zc, cuff, width=2400):
    height = int(width * 58 / (2 * cuff.s_end + 2))
    img = np.zeros((height, width, 3), np.uint8)
    x = np.clip(((sc + cuff.s_end + 1) / (2 * cuff.s_end + 2) * (width - 1)).astype(int), 0, width - 1)
    y = np.clip(((32 - zc) / 58 * (height - 1)).astype(int), 0, height - 1)
    for name in GROUPS:
        k = (lab == name) & (name != "band")
        img[y[k], x[k]] = COLOURS[name]
    im = Image.fromarray(img)
    draw = ImageDraw.Draw(im)
    for s_mark in range(-80, 81, 10):
        xx = int((s_mark + cuff.s_end + 1) / (2 * cuff.s_end + 2) * (width - 1))
        draw.line([(xx, height - 12), (xx, height - 1)], fill=(255, 255, 255))
        draw.text((xx + 2, height - 12), str(s_mark), fill=(255, 255, 255))
    for z_mark in range(-25, 31, 5):
        yy = int((32 - z_mark) / 58 * (height - 1))
        draw.line([(0, yy), (10, yy)], fill=(255, 255, 255))
        draw.text((12, yy - 6), str(z_mark), fill=(255, 255, 255))
    im.save(path)


def write_obj(path, head, vlines, F, lab):
    with open(path, "wb") as fh:
        fh.writelines(head)
        fh.writelines(vlines)
        for name in GROUPS:
            k = np.flatnonzero(lab == name)
            if not len(k):
                continue
            fh.write(f"g {name}\n".encode())
            np.savetxt(fh, F[k] + 1, fmt="f %d %d %d")


def main():
    work, src, dst = Path(sys.argv[1]), Path(sys.argv[2]), Path(sys.argv[3])
    head, vlines, V, F = read_obj(src)
    lab, sc, zc, cuff = label_faces(work, V, F)
    print(f"bore Ø{2 * cuff.r_bore:.2f} mm, arc ±{cuff.s_end:.2f} mm")
    for name in GROUPS:
        print(f"  {name:<11} {int((lab == name).sum()):>9} faces")
    if len(sys.argv) > 4:
        preview(sys.argv[4], lab, sc, zc, cuff)
    if str(dst) != "-":
        dst.parent.mkdir(parents=True, exist_ok=True)
        write_obj(dst, head, vlines, F, lab)
        print(f"wrote {dst}")


if __name__ == "__main__":
    main()
