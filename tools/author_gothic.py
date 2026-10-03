#!/usr/bin/env python3
"""Tenebrae's Gothic sketch set, written to bundled/sketches/gothic/*.svg.

Every file is in the form `sketch::exchange::import_svg` takes without
complaint: `width` and `height` in millimetres over a viewBox of the same
size (a uniform scale of 1), no `transform`, no smooth shorthand. Geometric
pieces are exact lines and circular arcs; figurative pieces are polygons
built with shapely and simplified to a few microns. Each root carries what
the core's test holds it to: `data-area-mm2` (the area its regions sweep)
or, for a net, `data-lights` at `data-bar-mm` (the lights
`Sketch::tracery` draws from it).

Nine pieces stand in for the 3DM originals the brief names (Under Gallery
Cuts 001-003, Jalis 000/002/010/016, Ornaments 027/028), which live in the
workstation's git-ignored `assets/User/Profiles/`. `harvest_gothic.py`
replaces them from the 3DM files under the same names.

    uv run --no-project --with shapely python tools/author_gothic.py
"""
from __future__ import annotations

import math
from pathlib import Path

from shapely import affinity
from shapely.geometry import LineString, MultiPolygon, Point, Polygon, box
from shapely.ops import polygonize, unary_union

OUT = Path(__file__).resolve().parents[1] / "bundled" / "sketches" / "gothic"
TAU = 2.0 * math.pi
# Most points one sketch holds is 1024; figurative pieces stay well under it.
MAX_POINTS = 900
# Most points one polyline holds is 512.
MAX_RING = 500


def fmt(v: float) -> str:
    s = f"{v:.9f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


class Path2:
    """A closed loop of lines and circular arcs in y-up millimetres, and its exact signed area."""

    def __init__(self, start):
        self.start = start
        self.cur = start
        self.cmds = [f"M {fmt(start[0])} {fmt(-start[1])}"]
        self.area2 = 0.0

    def line(self, p):
        self.area2 += self.cur[0] * p[1] - p[0] * self.cur[1]
        self.cmds.append(f"L {fmt(p[0])} {fmt(-p[1])}")
        self.cur = p
        return self

    def arc(self, c, r, a0, a1):
        """From the current point, at angle `a0` round `c`, to angle `a1`: anticlockwise when `a1 > a0`."""
        pieces = max(1, math.ceil(abs(a1 - a0) / math.radians(150)))
        for k in range(pieces):
            b0 = a0 + (a1 - a0) * k / pieces
            b1 = a0 + (a1 - a0) * (k + 1) / pieces
            end = (c[0] + r * math.cos(b1), c[1] + r * math.sin(b1))
            # y flips on the way out, so an anticlockwise arc here sweeps the negative way in SVG.
            sweep = 0 if b1 > b0 else 1
            self.cmds.append(f"A {fmt(r)} {fmt(r)} 0 0 {sweep} {fmt(end[0])} {fmt(-end[1])}")
            self.area2 += r * r * (b1 - b0) + c[0] * r * (math.sin(b1) - math.sin(b0)) + c[1] * r * (math.cos(b0) - math.cos(b1))
            self.cur = end
        return self

    def close(self):
        if math.dist(self.cur, self.start) > 1e-9:
            self.line(self.start)
        self.cmds.append("Z")
        return self

    def d(self):
        return " ".join(self.cmds)

    def area(self):
        return abs(0.5 * self.area2)


def write(name: str, desc: str, body: list[str], bounds, attrs: dict[str, str]):
    lo, hi = bounds
    m = 0.5
    x, y, w, h = lo[0] - m, -hi[1] - m, hi[0] - lo[0] + 2 * m, hi[1] - lo[1] + 2 * m
    extra = "".join(f' {k}="{v}"' for k, v in attrs.items())
    text = (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{fmt(w)}mm" height="{fmt(h)}mm" '
        f'viewBox="{fmt(x)} {fmt(y)} {fmt(w)} {fmt(h)}"{extra}>\n'
        f"  <title>{name}</title>\n  <desc>{desc}</desc>\n"
        + "".join(f"  {b}\n" for b in body)
        + "</svg>\n"
    )
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / f"{name}.svg").write_text(text)


def arc_points(c, r, a0, a1, step=0.02):
    n = max(8, math.ceil(abs(a1 - a0) * r / step))
    return [(c[0] + r * math.cos(a0 + (a1 - a0) * i / n), c[1] + r * math.sin(a0 + (a1 - a0) * i / n)) for i in range(n + 1)]


def path_bounds(paths_points):
    xs = [p[0] for pts in paths_points for p in pts]
    ys = [p[1] for pts in paths_points for p in pts]
    return (min(xs), min(ys)), (max(xs), max(ys))


# ---------------------------------------------------------------- outlines of lines and arcs


def ogee(w=3.2, l=6.0):
    """An ogee arch standing on its sill, the finial up: two arcs of one radius each side."""
    q, h = 0.5 * w, 1.1 * w
    y0, top = 0.0, l
    ys = top - h
    alpha = 2.0 * math.atan(w / (2.0 * h))
    r = h / (2.0 * math.sin(alpha))
    p = Path2((q, y0)).line((q, ys))
    # Right side up: convex about a centre inside, then concave about one outside, meeting the axis at the top.
    p.arc((q - r, ys), r, 0.0, alpha)
    p.arc((r, top), r, math.pi + alpha, math.pi)
    p.arc((-r, top), r, 0.0, -alpha)
    p.arc((r - q, ys), r, math.pi - alpha, math.pi)
    p.line((-q, y0)).close()
    return [p], ((-q, y0), (q, top))


def foil_loop(k, rho, scale, start_angle, shrink=0.0):
    """A foil of `k` lobes on centres `scale` out, lobe radius `rho * scale - shrink`, the first lobe at `start_angle`."""
    step = TAU / k
    half = 0.5 * step
    r = rho * scale - shrink
    centres = [(scale * math.cos(start_angle + step * i), scale * math.sin(start_angle + step * i)) for i in range(k)]
    reach = scale * math.cos(half) + math.sqrt(r * r - (scale * math.sin(half)) ** 2)
    cusp = lambda a: (reach * math.cos(a), reach * math.sin(a))
    first = cusp(start_angle - half)
    p = Path2(first)
    for i, c in enumerate(centres):
        a, b = start_angle + step * i - half, start_angle + step * i + half
        ca, cb = cusp(a), cusp(b)
        fa = math.atan2(ca[1] - c[1], ca[0] - c[0])
        fb = math.atan2(cb[1] - c[1], cb[0] - c[0])
        while fb <= fa:
            fb += TAU
        p.arc(c, r, fa, fb)
    p.close()
    return p, scale + r


def quatrefoil():
    p, reach = foil_loop(4, 0.8, 1.4, math.pi / 2)
    return [p], ((-reach, -reach), (reach, reach))


def quatrefoil_ring():
    outer, reach = foil_loop(4, 0.95, 1.8, math.pi / 4)
    inner, _ = foil_loop(4, 0.95, 1.8, math.pi / 4, shrink=0.35)
    return [outer, inner], ((-reach, -reach), (reach, reach))


def through(a, b, sag):
    """The arc from `a` to `b` bulging `sag` to the right of the way from a to b (to the left when negative): centre, radius, start and end angles."""
    mx, my = 0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1])
    dx, dy = b[0] - a[0], b[1] - a[1]
    half = 0.5 * math.hypot(dx, dy)
    s = abs(sag)
    r = (half * half + s * s) / (2.0 * s)
    side = 1.0 if sag > 0 else -1.0
    nx, ny = side * dy / (2 * half), -side * dx / (2 * half)
    c = (mx - nx * (r - s), my - ny * (r - s))
    a0 = math.atan2(a[1] - c[1], a[0] - c[0])
    a1 = math.atan2(b[1] - c[1], b[0] - c[0])
    # A bulge to the right keeps the centre on the left: anticlockwise about it; to the left, clockwise.
    if sag > 0:
        while a1 <= a0:
            a1 += TAU
    else:
        while a1 >= a0:
            a1 -= TAU
    return c, r, a0, a1


def cusped_lozenge(h=6.0, w=4.0, pull=0.55, sag=0.32):
    """A lozenge whose four sides each bulge out in two arcs meeting at a cusp drawn in toward the middle."""
    v = [(0.0, 0.5 * h), (-0.5 * w, 0.0), (0.0, -0.5 * h), (0.5 * w, 0.0)]
    p = Path2(v[0])
    for i in range(4):
        a, b = v[i], v[(i + 1) % 4]
        m = (0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1]))
        k = math.hypot(*m)
        cusp = (m[0] * (1 - pull / k), m[1] * (1 - pull / k))
        # Walking anticlockwise round the lozenge the outside is on the right.
        for s, e in ((a, cusp), (cusp, b)):
            c, r, a0, a1 = through(s, e, sag)
            p.arc(c, r, a0, a1)
    p.close()
    return [p], ((-0.5 * w - sag, -0.5 * h), (0.5 * w + sag, 0.5 * h))


def lancet_hole(x0, x1, y0, span_top):
    """A lancet opening from x0 to x1, its jambs rising from y0 to its springing, the point `span_top` over it."""
    w = x1 - x0
    ys = span_top - math.sqrt(3.0) / 2.0 * w
    p = Path2((x1, y0)).line((x1, ys))
    p.arc((x0, ys), w, 0.0, math.pi / 3)
    p.arc((x1, ys), w, 2 * math.pi / 3, math.pi)
    p.line((x0, y0)).close()
    return p


def nave_arcade(bays=3, pitch=2.6, col=0.5, height=6.0, sill=0.6, head=0.7):
    """Three bays of lancet arcade in a band: the band, less one opening per bay."""
    width = bays * pitch + col
    outer = Path2((0.0, 0.0)).line((width, 0.0)).line((width, height)).line((0.0, height)).close()
    holes = [lancet_hole(col + i * pitch, (i + 1) * pitch, sill, height - head) for i in range(bays)]
    return [outer] + holes, ((0.0, 0.0), (width, height))


def cross_pattee(arm=3.0, root=0.55, flare=1.3, sag=0.35):
    """Four arms narrow at the middle, flaring to wide straight ends, their sides curved in."""
    p = None
    for i in range(4):
        a = i * math.pi / 2
        rot = lambda x, y: (x * math.cos(a) - y * math.sin(a), x * math.sin(a) + y * math.cos(a))
        s0, e0, e1 = rot(root, -root), rot(arm, -flare), rot(arm, flare)
        if p is None:
            p = Path2(s0)
        c, r, b0, b1 = through(s0, e0, -sag)
        p.arc(c, r, b0, b1)
        p.line(e1)
        s1 = rot(root, root)
        c, r, b0, b1 = through(e1, s1, -sag)
        p.arc(c, r, b0, b1)
    p.close()
    return [p], ((-arm, -arm), (arm, arm))


# ---------------------------------------------------------------- nets for tracery


def net_svg(name, desc, lines, arcs, circles, frame, bar):
    """A net's centre lines: straight lines, arcs and whole circles, one light per cell it closes."""
    body = []
    geoms = []
    # Rounded as written, so ends that meet in the file meet for the count too.
    rnd = lambda p: (round(p[0], 9), round(p[1], 9))
    lines = [(rnd(a), rnd(b)) for a, b in lines]
    for a, b in lines:
        body.append(f'<line x1="{fmt(a[0])}" y1="{fmt(-a[1])}" x2="{fmt(b[0])}" y2="{fmt(-b[1])}"/>')
        geoms.append(LineString([a, b]))
    for c, r, a0, a1 in arcs:
        p = Path2((c[0] + r * math.cos(a0), c[1] + r * math.sin(a0)))
        p.arc(c, r, a0, a1)
        body.append(f'<path d="{p.d()}"/>')
        geoms.append(LineString([rnd(q) for q in arc_points(c, r, a0, a1, 0.002)]))
    for c, r in circles:
        body.append(f'<circle cx="{fmt(c[0])}" cy="{fmt(-c[1])}" r="{fmt(r)}"/>')
        # A quarter degree apart, so every eighth of a turn, where ties meet a circle, is a vertex the count nodes on.
        geoms.append(LineString([rnd((c[0] + r * math.cos(TAU * i / 1440), c[1] + r * math.sin(TAU * i / 1440))) for i in range(1441)]))
    cells = list(polygonize(unary_union(geoms)))
    write(name, desc, body, frame, {"data-lights": str(len(cells)), "data-bar-mm": fmt(bar)})
    return len(cells)


def jali_lozenge(n=3, pitch=2.4):
    """A square frame crossed by its diagonals' parallels: a lozenge lattice, half-lozenges at the rim."""
    s = n * pitch
    lines = [((0, 0), (s, 0)), ((s, 0), (s, s)), ((s, s), (0, s)), ((0, s), (0, 0))]
    for k in range(1, 2 * n):
        t = k * pitch
        # x + y = t and x - y = t - s, clipped to the square.
        lines.append(((max(0, t - s), min(s, t)), (min(s, t), max(0, t - s))))
        lines.append(((max(0, t - s), max(0, s - t)), (min(s, t), min(s, 2 * s - t))))
    return lines, [], [], ((0, 0), (s, s))


def jali_quatrefoil(n=2, pitch=3.0):
    """Circles on a square lattice, each through its four neighbours' centres' midpoints: quatrefoil cells between lens cells, in a frame."""
    s = n * pitch
    lines = [((0, 0), (s, 0)), ((s, 0), (s, s)), ((s, s), (0, s)), ((0, s), (0, 0))]
    circles = [((pitch * (i + 0.5), pitch * (j + 0.5)), 0.5 * pitch * 0.7) for i in range(n) for j in range(n)]
    # Diagonal ties from each circle to the frame corners of its cell, so every cell closes against the frame.
    for i in range(n):
        for j in range(n):
            cx, cy = pitch * (i + 0.5), pitch * (j + 0.5)
            r = 0.5 * pitch * 0.7
            for dx, dy in ((1, 1), (1, -1), (-1, 1), (-1, -1)):
                k = r * math.cos(math.pi / 4)
                lines.append(((cx + dx * k, cy + dy * k), (cx + dx * 0.5 * pitch, cy + dy * 0.5 * pitch)))
    return lines, [], circles, ((0, 0), (s, s))


def jali_honeycomb(r=1.5):
    """A rosette of seven flat-topped hexagons, one in the middle and six round it, every edge drawn once."""
    h = math.sqrt(3) * r
    centres = [(0.0, 0.0)] + [(h * math.cos(math.pi / 6 + k * math.pi / 3), h * math.sin(math.pi / 6 + k * math.pi / 3)) for k in range(6)]
    edges = {}
    for cx, cy in centres:
        pts = [(cx + r * math.cos(k * math.pi / 3), cy + r * math.sin(k * math.pi / 3)) for k in range(6)]
        for k in range(6):
            a, b = pts[k], pts[(k + 1) % 6]
            key = tuple(sorted((tuple(round(v, 6) for v in a), tuple(round(v, 6) for v in b))))
            edges.setdefault(key, (a, b))
    lines = list(edges.values())
    xs = [p[0] for e in lines for p in e]
    ys = [p[1] for e in lines for p in e]
    return lines, [], [], ((min(xs), min(ys)), (max(xs), max(ys)))


def jali_intersecting(bays=4, pitch=1.8, spring=3.0, drop=0.6):
    """Intersecting tracery: an arch from every mullion to the next but one, crossing its neighbours into pointed lancets.

    Each arch is struck from `drop` pitches under its springing, so neighbouring arches leave a shared
    springing at an angle; semicircles would touch there along one tangent and close a cell no bar fits.
    """
    width = bays * pitch
    d = drop * pitch
    radius = math.hypot(pitch, d)
    top = spring - d + radius + 0.35 * pitch
    lines = [((0, 0), (width, 0)), ((width, 0), (width, top)), ((width, top), (0, top)), ((0, top), (0, 0))]
    for i in range(1, bays):
        lines.append(((i * pitch, 0), (i * pitch, spring)))
    arcs = []
    rise = math.atan2(d, pitch)
    for i in range(-1, bays):
        c = ((i + 1) * pitch, spring - d)
        a0, a1 = rise, math.pi - rise
        # Clipped where they meet the frame's sides.
        if c[0] + pitch > width + 1e-9:
            a0 = math.acos((width - c[0]) / radius)
        if c[0] - pitch < -1e-9:
            a1 = math.acos(-c[0] / radius)
        if a1 > a0 + 1e-6:
            arcs.append((c, radius, a0, a1))
    return lines, arcs, [], ((0, 0), (width, top))


# ---------------------------------------------------------------- figurative pieces


def polygon_svg(name, desc, shape, attrs=None):
    """A shapely shape as `polygon` rings: exteriors and holes, simplified to a few microns."""
    shape = shape.buffer(0)
    if isinstance(shape, Polygon):
        shape = MultiPolygon([shape])
    body = []
    points = 0
    area = 0.0
    tol = 0.002
    while True:
        simple = [p.simplify(tol, preserve_topology=True) for p in shape.geoms]
        points = sum(len(p.exterior.coords) - 1 + sum(len(i.coords) - 1 for i in p.interiors) for p in simple)
        longest = max(len(r.coords) - 1 for p in simple for r in [p.exterior, *p.interiors])
        if points <= MAX_POINTS and longest <= MAX_RING:
            break
        tol *= 1.5
    for p in simple:
        assert p.is_valid, name
        area += p.area
        for ring in [p.exterior] + list(p.interiors):
            pts = list(ring.coords)[:-1]
            body.append('<polygon points="' + " ".join(f"{fmt(x)},{fmt(-y)}" for x, y in pts) + '"/>')
    lo = (shape.bounds[0], shape.bounds[1])
    hi = (shape.bounds[2], shape.bounds[3])
    a = {"data-area-mm2": fmt(area)}
    a.update(attrs or {})
    write(name, desc, body, (lo, hi), a)
    return points


def poly(pts):
    return Polygon(pts)


def ring_of(f, n=160):
    return [f(TAU * i / n) for i in range(n)]


def petal(height, half_width, base_y, bias=0.7, point=0.35):
    """A pointed leaf rising from `base_y`: widest a third of the way up, closing to a point."""
    pts = []
    n = 120
    for i in range(n + 1):
        t = i / n
        hw = half_width * math.sin(math.pi * t) ** bias * (1 - t) ** point
        pts.append((hw, base_y + height * t))
    right = pts
    left = [(-x, y) for x, y in reversed(pts[1:-1])]
    return Polygon(right + left)


def bezier(p0, p1, p2, p3, n=120):
    return [
        (
            (1 - t) ** 3 * p0[0] + 3 * (1 - t) ** 2 * t * p1[0] + 3 * (1 - t) * t * t * p2[0] + t ** 3 * p3[0],
            (1 - t) ** 3 * p0[1] + 3 * (1 - t) ** 2 * t * p1[1] + 3 * (1 - t) * t * t * p2[1] + t ** 3 * p3[1],
        )
        for t in (i / n for i in range(n + 1))
    ]


def stroke(points, r0, r1):
    """A stroke along `points` whose half-width runs from `r0` to `r1`: a tapering petal or stalk."""
    n = len(points) - 1
    return unary_union([Point(p).buffer(r0 + (r1 - r0) * i / n, 24) for i, p in enumerate(points)])


def fleur_de_lis(scale=1.0):
    """The lily of the French arms: a tall pointed centre petal, two petals rising out and curling down, a band, three short petals under it."""
    centre = petal(5.0, 0.92, 0.9, bias=0.6, point=0.5)
    side = stroke(bezier((0.3, 1.1), (0.75, 3.5), (2.85, 3.95), (2.6, 1.65)), 0.46, 0.07)
    lower = stroke(bezier((0.2, 0.7), (0.45, 0.05), (1.15, -0.25), (1.45, 0.25)), 0.26, 0.07)
    foot = affinity.scale(petal(1.1, 0.34, 0.0, bias=0.7, point=0.5), 1.0, -1.0, origin=(0, 0))
    foot = affinity.translate(foot, 0.0, 0.8)
    band = box(-1.45, 0.72, 1.45, 1.2).buffer(0.1, 16)
    mirror = lambda g: affinity.scale(g, xfact=-1, origin=(0, 0))
    shape = unary_union([centre, side, mirror(side), lower, mirror(lower), foot, band])
    return affinity.scale(shape, scale, scale, origin=(0, 0))


def fleur_cresting(units=4, pitch=3.0):
    """A cresting strip: a rail under a row of small fleurs, one piece."""
    unit = fleur_de_lis(0.42)
    parts = [box(-0.5 * pitch, -0.9, (units - 0.5) * pitch, -0.3).buffer(0.08, 16)]
    for i in range(units):
        parts.append(affinity.translate(unit, i * pitch, 0.0))
        parts.append(box(i * pitch - 0.14, -0.35, i * pitch + 0.14, 0.05))
    return unary_union(parts)


def crocket_leaf():
    """A crocket: a stalk rising from its seat, arching over and curling down into a bud, lobed leaves standing out from its back."""
    c = (1.25, 2.3)
    stem = [(-0.62, 0.0 + 1.95 * i / 30) for i in range(30)]
    spiral = []
    for i in range(241):
        t = i / 240
        a = math.radians(190.0 - 360.0 * t)
        r = 1.9 - 1.35 * t
        spiral.append((c[0] + r * math.cos(a), c[1] + r * math.sin(a)))
    spine = stem + spiral
    stalk = stroke(spine, 0.32, 0.17)
    bud = Point(spiral[-1]).buffer(0.42, 48)
    leaves = []
    for k, i in enumerate((20, 70, 120, 165)):
        x, y = spiral[i]
        out = math.degrees(math.atan2(y - c[1], x - c[0]))
        size = 1.0 - 0.14 * k
        leaf = petal(0.95 * size, 0.36 * size, 0.0, bias=0.8, point=0.6)
        lobes = unary_union([leaf, affinity.rotate(leaf, 38, origin=(0, 0)), affinity.rotate(leaf, -38, origin=(0, 0))])
        leaves.append(affinity.translate(affinity.rotate(lobes, out - 90.0, origin=(0, 0)), x, y))
    seat = box(-1.5, -0.45, 0.3, 0.05).buffer(0.05, 8)
    return unary_union([stalk, bud, *leaves, seat])


def gargoyle(face: bool):
    """A crouching gargoyle in profile, facing right: haunches, a folded wing, a horned head thrust forward, a curled tail."""
    body = affinity.scale(Point(0.0, 1.4).buffer(1.0, 128), 1.7, 1.0)
    chest = affinity.scale(Point(1.35, 2.0).buffer(1.0, 128), 0.85, 0.95)
    neck = Polygon([(1.4, 2.4), (2.6, 3.3), (3.0, 2.7), (1.9, 1.7)])
    head = affinity.scale(Point(3.05, 3.1).buffer(0.75, 128), 1.1, 0.9)
    snout = Polygon([(3.3, 3.55), (4.55, 3.2), (4.65, 2.95), (4.2, 2.8), (3.3, 2.55)])
    jaw = Polygon([(3.2, 2.7), (4.25, 2.55), (4.3, 2.35), (3.3, 2.25)])
    horn = Polygon([(2.6, 3.6), (2.2, 4.6), (1.75, 5.05), (2.05, 4.45), (2.25, 3.55)])
    horn2 = Polygon([(3.0, 3.75), (2.95, 4.5), (2.7, 4.95), (2.72, 4.35), (2.75, 3.7)])
    ear = Polygon([(2.55, 3.5), (2.0, 3.95), (2.35, 3.3)])
    wing = Polygon([(-0.6, 2.0), (-1.0, 3.9), (-0.4, 3.35), (0.0, 4.35), (0.35, 3.45), (0.95, 4.05), (1.0, 2.7), (0.4, 2.2)])
    haunch = affinity.scale(Point(-0.9, 1.1).buffer(0.8, 128), 1.0, 1.1)
    hind = Polygon([(-1.4, 0.7), (-0.4, 0.5), (-0.2, 0.0), (-1.3, 0.0), (-1.6, 0.3)])
    fore = Polygon([(1.3, 1.4), (1.9, 1.3), (2.1, 0.0), (1.55, 0.0), (1.45, 0.6)])
    claws = [Polygon([(x, 0.0), (x + 0.3, 0.0), (x + 0.45, -0.25), (x + 0.15, -0.12)]) for x in (1.55, 1.8, -1.3, -1.0)]
    tail = LineString([(-1.6, 1.0), (-2.4, 0.9), (-2.9, 1.4), (-2.7, 2.0), (-2.25, 1.9), (-2.35, 1.55)]).buffer(0.17, 16)
    tip = Polygon([(-2.3, 1.35), (-2.0, 1.75), (-2.55, 1.65)])
    base = box(-3.2, -0.55, 4.9, -0.1)
    shape = unary_union([body, chest, neck, head, snout, jaw, horn, horn2, ear, wing, haunch, hind, fore, *claws, tail, tip, base])
    # The mouth's line runs in from the snout's tip as a notch in the silhouette.
    shape = shape.difference(Polygon([(4.7, 2.75), (3.55, 2.62), (4.7, 2.52)]))
    if face:
        eye = Point(3.3, 3.35).buffer(0.17, 48)
        brow = Polygon([(2.95, 3.62), (3.75, 3.62), (3.62, 3.52), (3.0, 3.5)])
        nostril = Point(4.35, 3.18).buffer(0.08, 32)
        fang = Polygon([(3.85, 2.63), (3.98, 2.63), (3.92, 2.45)])
        shape = shape.difference(unary_union([eye, nostril])).difference(brow).union(fang)
        # Teeth along the notch, which the fang joins back to the jaw.
    return shape


def memento_mori():
    """An hourglass on two crossed bones: its frame, its glass as two openings, the bones in a saltire under it."""
    def bone(length=6.2, shaft=0.34, knob=0.36):
        half = 0.5 * length
        parts = [box(-half + knob, -0.5 * shaft, half - knob, 0.5 * shaft)]
        for s in (-1, 1):
            parts.append(Point(s * (half - knob), 0.36).buffer(knob, 48))
            parts.append(Point(s * (half - knob), -0.36).buffer(knob, 48))
        return unary_union(parts)
    bones = unary_union([affinity.rotate(bone(), a, origin=(0, 0)) for a in (32, -32)])
    frame = unary_union([
        box(-1.35, 1.3, 1.35, 1.7).buffer(0.06, 8),
        box(-1.35, 5.3, 1.35, 5.7).buffer(0.06, 8),
        box(-1.15, 1.5, -0.95, 5.5),
        box(0.95, 1.5, 1.15, 5.5),
    ])
    glass_outline = []
    for i in range(121):
        t = i / 120
        y = 1.75 + 3.5 * t
        hw = 0.12 + 0.62 * abs(math.sin(math.pi * (t - 0.5))) ** 0.8
        glass_outline.append((hw, y))
    glass = Polygon(glass_outline + [(-x, y) for x, y in reversed(glass_outline)]).buffer(0.1, 16)
    body = unary_union([frame, glass, bones.intersection(box(-4, -4, 4, 1.35)).union(bones.difference(box(-1.5, -4, 1.5, 6)))])
    bulbs = [
        Polygon([(x, y) for x, y in glass_outline if y <= 3.4] + [(-x, y) for x, y in reversed(glass_outline) if y <= 3.4]).buffer(-0.12, 16),
        Polygon([(x, y) for x, y in glass_outline if y >= 3.6] + [(-x, y) for x, y in reversed(glass_outline) if y >= 3.6]).buffer(-0.12, 16),
    ]
    return body.difference(unary_union(bulbs))


# ---------------------------------------------------------------- the set


STAND_IN = "Stand-in drawn in place of {src} (assets/User/Profiles, git-ignored, not in this checkout); tools/harvest_gothic.py replaces it from the 3DM under this name."


def main():
    made = []

    def outline(name, desc, loops_bounds):
        loops, bounds = loops_bounds
        body = ['<path d="' + " ".join(p.d() for p in loops) + '"/>']
        area = loops[0].area() - sum(p.area() for p in loops[1:])
        write(name, desc, body, bounds, {"data-area-mm2": fmt(area)})
        made.append((name, f"{area:.4f} mm2"))

    outline("gallery-ogee", STAND_IN.format(src="Under Gallery Cut 001 (ogee)") + " An ogee arch, finial up.", ogee())
    outline("gallery-quatrefoil", STAND_IN.format(src="Under Gallery Cut 002 (quatrefoil)") + " Four lobes on the axes.", quatrefoil())
    outline("gallery-cusped-lozenge", STAND_IN.format(src="Under Gallery Cut 003 (cusped lozenge)") + " A lozenge of eight arcs, cusped at each side.", cusped_lozenge())
    outline("ornament-quatrefoil-ring", STAND_IN.format(src="Ornament 027 (quatrefoil ring)") + " A quatrefoil less a quatrefoil 0.35 mm inside it.", quatrefoil_ring())
    outline("nave-arcade", "Nave arcade tile: three lancet bays in a band, for wall arcades on stock and Capsa's bays.", nave_arcade())
    outline("cross-pattee", "Cross pattee: four arms flaring from a narrow middle to straight ends, their sides curved in. Sigillum's legend cross.", cross_pattee())

    nets = [
        ("jali-lozenge", STAND_IN.format(src="Jali 000") + " A lozenge lattice in a square frame: centre lines for Sketch::tracery.", jali_lozenge(), 0.3),
        ("jali-quatrefoil", STAND_IN.format(src="Jali 002") + " Circles on a square lattice tied to the frame: centre lines for Sketch::tracery.", jali_quatrefoil(), 0.25),
        ("jali-honeycomb", STAND_IN.format(src="Jali 010") + " Hexagons in a frame: centre lines for Sketch::tracery.", jali_honeycomb(), 0.3),
        ("jali-intersecting-arches", STAND_IN.format(src="Jali 016") + " Intersecting tracery: segmental arches from every other mullion crossing into lancets: centre lines for Sketch::tracery.", jali_intersecting(), 0.25),
    ]
    for name, desc, (lines, arcs, circles, frame), bar in nets:
        made.append((name, f"{net_svg(name, desc, lines, arcs, circles, frame, bar)} cells"))

    figures = [
        ("fleur-de-lis", STAND_IN.format(src="Ornament 028 (fleur-de-lis)") + " A fleur-de-lis: centre petal, two petals curling out, band and feet.", fleur_de_lis()),
        ("fleur-cresting", "Fleur cresting: a rail under four small fleurs, one piece, for a crown's or a reliquary's ridge.", fleur_cresting()),
        ("crocket-leaf", "Crocket: a stalk curling over into a bud, three lobed leaves on its back, on a seat. Porta's crockets.", crocket_leaf()),
        ("gargoyle-silhouette", "Gargoyle in profile, silhouette only: haunches, folded wing, horned head, curled tail.", gargoyle(False)),
        ("gargoyle-face", "Gargoyle in profile with its face read: an eye, a brow, a nostril and a fang at the open mouth. Cut it if it does not read at size.", gargoyle(True)),
        ("memento-mori", "Crossed bones under an hourglass, the glass open: Capsa's memento mori.", memento_mori()),
    ]
    for name, desc, shape in figures:
        made.append((name, f"{polygon_svg(name, desc, shape)} points"))
    for name, what in made:
        print(f"{name:28} {what}")


if __name__ == "__main__":
    main()
