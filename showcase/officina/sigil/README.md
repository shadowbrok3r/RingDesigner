# Sigil — a quartered seal on the round

Factory stock 013 (Round, 10 × 10 mm face) at its native size, bore 18.2 mm, poured bare in Delft clay from its drafted sand master. The seal is cut at the bench: every cut is `Stage::Bench`, so the casting pattern is the bare stock. Drag the timeline's rollback marker from feature 1 to feature 7.

1. **Stock 013** (`Band`). The factory stock is the band; this feature is the anchor the rest stands on. *Learn:* CAD parts stand on imported stock exactly as on a procedural band.
2. **Table plane** (work plane, tangent at 90°). Dropped onto the built stock, it seats on the table to 0.000 mm and 0.003° off radial. *Learn:* a work plane reads the surface as built, so it sits on factory stock.
3. **Seal** (sketch on 2). A round bordure (two circles: a region with a hole) and a heater shield. The shield's outline is one loop; inside it a second loop runs round the raised cross and the two bright quarters. *Learn:* loops nest even-odd: a loop inside a loop is a hole. The sunk field (the outline groove, opening into the first and fourth quarters) is one region with the cross as its hole.
4. **Seal cutter** (extrude of two picked regions, 2 mm down). *Learn:* pick regions by `RegionRef` and extrude several at once; the cutter stands as a tool body, not yet cut.
5. **Crown** (sketch on its own plane, square to the ring). The table's crown across the finger, read off the stock and let down 0.42 mm. The table falls 0.29 mm at 2 mm and 0.68 mm at 4 mm, so a flat cut would sink the seal 0.9 mm at the middle and barely touch its ends. *Learn:* a plane through the finger's axis shows the section a flat cut cannot follow.
6. **Crown let down** (a sweep of 5 round the ring). A slab whose underside follows the table's crown. *Learn:* a sweep carries a section along a path; ours is a mesh, so the next boolean runs through `csg`.
7. **Seal sunk** (intersect 4 with 6; cut, bench). The seal sinks 0.36–0.47 mm everywhere, square into the crown. Its walls stay crisp for a clean impression, and its floors are left oxidised satin. *Learn:* intersecting a planar cutter with a surface-following slab gives a cut of even depth on a curved table; the two-stage model keeps it out of the pour.

**Parting seam dressed.** Bare, the 013 sand master fields "Castable with care": a mirror seam about 1 micron deep runs on the crest line between 0° and 55°. A 0.5 × 0.004 mm round bead on the crest line (the layer "Parting seam dressed", outside the feature timeline) fills it, and the field reads Castable. No render can see it.

**Render shading.** The stock's source mesh, and so `finished-metal.stl`, carries 0.55 mm facets that stand within microns of the true surface. On a polished shank they read as stepped highlights, so the renders diffuse the shank's shading normals over 0.6 mm (never across an edge sharper than 20°). The geometry, the STL and every gate are untouched by this; it is the render's only smoothing.
