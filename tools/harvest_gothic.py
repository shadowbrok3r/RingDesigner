#!/usr/bin/env python3
"""Harvest the brief's Gothic profiles from the 3DM library into bundled/sketches/gothic.

Reads `assets/User/Profiles/` (git-ignored; on the workstation) and writes
each named source as an `import_svg`-clean SVG under the name its drawn
stand-in carries, so the bundle, `library::list_sketches` and the core's
test pick it up unchanged:

    Under Gallery Cut 001 -> gallery-ogee          Jali 000 -> jali-lozenge
    Under Gallery Cut 002 -> gallery-quatrefoil    Jali 002 -> jali-quatrefoil
    Under Gallery Cut 003 -> gallery-cusped-lozenge Jali 010 -> jali-honeycomb
    Ornament 027 -> ornament-quatrefoil-ring       Jali 016 -> jali-intersecting-arches
    Ornament 028 -> fleur-de-lis (a B-rep: its outline in plan)

Lines stay lines and arcs stay circular arcs; any other curve is sampled
every 0.02 mm. A document without units is fitted to the stand-in's size.
The root records what the test holds it to: the area an outline sweeps, or
for a net the cells it closes (`data-lights`) at `data-bar-mm`. Rerun
`cargo test -p ringdesign-core --lib every_bundled_gothic_sketch` after.

    uv run --no-project --with rhino3dm==8.32.0 --with shapely \
        python tools/harvest_gothic.py assets/User/Profiles [--dry-run]
"""
from __future__ import annotations

import argparse
import math
import re
from pathlib import Path

import rhino3dm
from shapely.geometry import LineString, Polygon
from shapely.ops import polygonize, unary_union

OUT = Path(__file__).resolve().parents[1] / "bundled" / "sketches" / "gothic"
STEP_MM = 0.02

# (folder keyword, number, output name, kind, fitted extent in mm when unitless, bar for a net)
SOURCES = [
    ("gallery", "001", "gallery-ogee", "outline", 6.0, None),
    ("gallery", "002", "gallery-quatrefoil", "outline", 5.0, None),
    ("gallery", "003", "gallery-cusped-lozenge", "outline", 6.0, None),
    ("jali", "000", "jali-lozenge", "net", 7.2, 0.3),
    ("jali", "002", "jali-quatrefoil", "net", 6.0, 0.25),
    ("jali", "010", "jali-honeycomb", "net", 7.5, 0.3),
    ("jali", "016", "jali-intersecting-arches", "net", 7.2, 0.25),
    ("ornament", "027", "ornament-quatrefoil-ring", "outline", 6.0, None),
    ("ornament", "028", "fleur-de-lis", "brep", 6.5, None),
]

UNIT_MM = {"Millimeters": 1.0, "Centimeters": 10.0, "Meters": 1000.0, "Inches": 25.4, "Feet": 304.8, "Microns": 0.001}


def fmt(v: float) -> str:
    s = f"{v:.9f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


def find(root: Path, keyword: str, number: str) -> Path | None:
    for path in sorted(root.rglob("*")):
        if path.suffix.lower() != ".3dm" or keyword not in str(path.parent).lower():
            continue
        if number in re.findall(r"\d+", path.stem) or re.search(rf"(^|\D){int(number)}(\D|$)", path.stem):
            return path
    return None


def segments(curve):
    """The curve's pieces: a polycurve's segments, each exploded once more if it is itself a polycurve."""
    if isinstance(curve, rhino3dm.PolyCurve):
        out = []
        for i in range(curve.SegmentCount):
            out.extend(segments(curve.SegmentCurve(i)))
        return out
    return [curve]


def sampled(curve, step=STEP_MM):
    """Points along the curve about `step` apart, its ends included."""
    lo, hi = curve.Domain.T0, curve.Domain.T1
    rough = [curve.PointAt(lo + (hi - lo) * i / 64) for i in range(65)]
    length = sum(rough[i].DistanceTo(rough[i + 1]) for i in range(64))
    n = max(8, math.ceil(length / step))
    return [curve.PointAt(lo + (hi - lo) * i / n) for i in range(n + 1)]


class Svg:
    """Subpaths in y-up millimetres, written with y flipped."""

    def __init__(self, scale, origin):
        self.scale, self.origin = scale, origin
        self.d = []
        self.lines = []
        self.pts = []

    def xy(self, p):
        q = ((p.X - self.origin[0]) * self.scale, (p.Y - self.origin[1]) * self.scale)
        self.pts.append(q)
        return q

    def curve(self, curve):
        first = True
        for seg in segments(curve):
            a = self.xy(seg.PointAtStart)
            if first:
                self.d.append(f"M {fmt(a[0])} {fmt(-a[1])}")
                first = False
            arc = seg.TryGetArc() if seg.IsArc() else None
            polyline = seg.TryGetPolyline() if isinstance(seg, rhino3dm.PolylineCurve) else None
            if seg.IsLinear():
                b = self.xy(seg.PointAtEnd)
                self.d.append(f"L {fmt(b[0])} {fmt(-b[1])}")
                self.lines.append([a, b])
            elif arc is not None:
                # Split so no piece reaches half a turn: the importer rebuilds each from its ends and radius.
                pieces = max(1, math.ceil(abs(arc.AngleRadians) / math.radians(150)))
                lo, hi = seg.Domain.T0, seg.Domain.T1
                r = arc.Radius * self.scale
                prev = a
                for k in range(1, pieces + 1):
                    p = self.xy(seg.PointAt(lo + (hi - lo) * k / pieces))
                    mid = self.xy(seg.PointAt(lo + (hi - lo) * (k - 0.5) / pieces))
                    # Turning left in y-up millimetres is the negative sweep once y flips: the angle falls in SVG's y-down frame.
                    turn = (mid[0] - prev[0]) * (p[1] - prev[1]) - (mid[1] - prev[1]) * (p[0] - prev[0])
                    sweep = 0 if turn > 0 else 1
                    self.d.append(f"A {fmt(r)} {fmt(r)} 0 0 {sweep} {fmt(p[0])} {fmt(-p[1])}")
                    prev = p
                self.lines.append([self.xy(q) for q in sampled(seg)])
            elif polyline is not None:
                pts = [self.xy(polyline[i]) for i in range(polyline.Count)]
                self.d.append(" ".join(f"L {fmt(x)} {fmt(-y)}" for x, y in pts[1:]))
                self.lines.append(pts)
            else:
                pts = [self.xy(q) for q in sampled(seg)]
                self.d.append(" ".join(f"L {fmt(x)} {fmt(-y)}" for x, y in pts[1:]))
                self.lines.append(pts)
        if curve.IsClosed:
            self.d.append("Z")


def brep_outline(brep):
    """A B-rep's outline in plan: its edges laid flat, the faces they close, merged."""
    lines = []
    for i in range(len(brep.Edges)):
        edge = brep.Edges[i]
        pts = sampled(edge)
        lines.append(LineString([(p.X, p.Y) for p in pts]))
    cells = list(polygonize(unary_union(lines)))
    return unary_union(cells).buffer(0)


def harvest(root: Path, dry: bool):
    for keyword, number, name, kind, extent, bar in SOURCES:
        path = find(root, keyword, number)
        if path is None:
            print(f"{name}: no {keyword} {number} under {root}; the stand-in stays")
            continue
        model = rhino3dm.File3dm.Read(str(path))
        if model is None:
            print(f"{name}: {path} does not read; the stand-in stays")
            continue
        geoms = [o.Geometry for o in model.Objects]
        box = None
        for g in geoms:
            b = g.GetBoundingBox()
            box = b if box is None else rhino3dm.BoundingBox.Union(box, b)
        unit = UNIT_MM.get(str(model.Settings.ModelUnitSystem).split(".")[-1])
        span = max(box.Max.X - box.Min.X, box.Max.Y - box.Min.Y)
        scale = unit if unit else extent / span
        origin = (0.5 * (box.Min.X + box.Max.X), 0.5 * (box.Min.Y + box.Max.Y))
        svg = Svg(scale, origin)
        attrs = {}
        body = []
        if kind == "brep":
            breps = [g for g in geoms if isinstance(g, rhino3dm.Brep)]
            outline = unary_union([brep_outline(b) for b in breps])
            polys = list(getattr(outline, "geoms", [outline]))
            area = 0.0
            for p in polys:
                p = Polygon([((x - origin[0]) * scale, (y - origin[1]) * scale) for x, y in p.exterior.coords],
                            [[((x - origin[0]) * scale, (y - origin[1]) * scale) for x, y in r.coords] for r in p.interiors]).simplify(0.003)
                area += p.area
                for ring in [p.exterior] + list(p.interiors):
                    pts = list(ring.coords)[:-1]
                    svg.pts.extend(pts)
                    body.append('<polygon points="' + " ".join(f"{fmt(x)},{fmt(-y)}" for x, y in pts) + '"/>')
            attrs["data-area-mm2"] = fmt(area)
        else:
            for g in geoms:
                if isinstance(g, rhino3dm.Curve):
                    svg.curve(g)
            body.append('<path d="' + " ".join(svg.d) + '"/>')
            flat = unary_union([LineString(l) for l in svg.lines])
            if kind == "net":
                attrs["data-lights"] = str(len(list(polygonize(flat))))
                attrs["data-bar-mm"] = fmt(bar)
            else:
                cells = list(polygonize(flat))
                # Even-odd: a loop inside another is its hole.
                area = sum(c.area * (-1) ** sum(1 for o in cells if o is not c and o.contains(c.representative_point()) and o.area > c.area) for c in cells)
                attrs["data-area-mm2"] = fmt(abs(area))
        xs = [p[0] for p in svg.pts]
        ys = [p[1] for p in svg.pts]
        m = 0.5
        x, y, w, h = min(xs) - m, -max(ys) - m, max(xs) - min(xs) + 2 * m, max(ys) - min(ys) + 2 * m
        extra = "".join(f' {k}="{v}"' for k, v in attrs.items())
        text = (
            f'<svg xmlns="http://www.w3.org/2000/svg" width="{fmt(w)}mm" height="{fmt(h)}mm" viewBox="{fmt(x)} {fmt(y)} {fmt(w)} {fmt(h)}"{extra}>\n'
            f"  <title>gothic/{name}</title>\n  <desc>Harvested from {path.relative_to(root)} by tools/harvest_gothic.py.</desc>\n"
            + "".join(f"  {b}\n" for b in body)
            + "</svg>\n"
        )
        print(f"{name}: {path.relative_to(root)} -> {'(dry run)' if dry else OUT / (name + '.svg')} {attrs}")
        if not dry:
            (OUT / f"{name}.svg").write_text(text)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("root", type=Path)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    if not args.root.is_dir():
        parser.error(f"{args.root} is not a directory; the 3DM library lives on the workstation")
    harvest(args.root.resolve(), args.dry_run)


if __name__ == "__main__":
    main()
