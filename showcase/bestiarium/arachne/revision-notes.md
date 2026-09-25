Arachne's final continuation candidate passes draft, export and cold template gates. Independent art-director approval is pending.

| Gate | Result |
| --- | --- |
| Draft / export triangles | 510,048 / 1,323,510 |
| Watertight; boundary / non-manifold / degenerate faces | yes; 0 / 0 / 0 |
| Ring / made-part self-crossings | 0 / 0 |
| CAD and setting notes; DFM findings | 0; 0 |
| Lost-wax field verdict | Castable |
| Investment floor; minimum band wall | 0.8 mm; 1.881 mm |
| Minimum leg / claw diameter | 0.8 mm / 0.8 mm |
| Minimum segment / joint-loft bend ratio | 1.267 / 1.278 |
| Minimum actual part-vertex bore clearance | 0.404 mm; 0 vertices below 0.10 mm |
| Minimum actual limb-vertex abdomen clearance | 0.413 mm |
| Minimum sampled centre-line abdomen clearance | 0.519 mm |
| Leg IV length; claw angle | 12.759 mm; 124° |
| Axial extent | 8.719 mm |
| Bare crown run; largest bare cell | 3.15 mm; 1.75 mm |
| Minimum free-hub radial-root gap | 0.201 mm |
| Stones reported / previewed; metal vertices inside stones | 2 / 2; 0 |
| Empty-library design reload | identical vertices and faces |
| Cold graph evaluation | identical design bytes, vertices, faces and normals |
| Design patches | 4: three draft fields and shank keys |

Each leg is now one continuous local loft. Tangent-matched transitions replace 0.35 mm of each adjoining segment and swell smoothly through the joint; no sphere-to-segment collars or unsupported blend fields remain. Radii stay 0.58 → 0.52 → 0.47 → 0.43 → 0.40 mm. The femur solver checks full circular sections against the garnet and gates the new knee transition's curvature. Root and knee positions remain fixed except the hind knee moves from 82.973° to 82.158° to preserve real abdomen clearance. The 9 × 7 mm onyx, both body positions, 0.4 mm rises, collets, pedicel and spinnerets are retained.

The loft carries exact curve tangents at 24-sided adaptive sections. Retained chords approximate the dense source stations' centres and radii within 0.002 mm. Packed limb payload fell from 527,560 to 126,548 bytes; each final mesh passes closure, crossing and abdomen-clearance checks before joining the band.

Free radial roots start 1.6 mm from the palm hub and 1.3 mm from the other free hubs. Five differently draped inner threads connect six roots, with one diagonal break. The main hub remains concealed by the garnet. Crown radials are 0.30 × 0.22 mm, and capture silk is 0.21 × 0.15 mm. Nested SVG contours form a rounded profile over the full stroke width. Frame anchors end 0.20 mm before the fold; relief fades over the preceding 0.40 mm. Cheek catenaries and bore clearance remain in place.

Source and remaining limits:

- The 644,207-byte design and 670,316-byte graph mix SVG silk, two painted PNG16 clearance atlases, and stored collet/limb solids. Compact component sizes are 138,189 bytes of atlases, 184,873 of SVG, 105,230 of collets and 126,548 of limbs. They exceed the procedural-only 300 KB allowance and remain below the painted-atlas 3 MB flag. Both are recorded; P7 will not shrink stored meshes automatically.
- Stored limbs retain their generating centre-line recipes, but changing those recipe values alone does not regenerate a stored mesh. The example regenerates it. Collets remain stored because the current bezel builder does not reproduce the flare and rolled equator.
- Exact boolean intersections still leave slivers. The 0.001 mm export clean removes 2,598 faces while remaining watertight with zero degenerate faces; minimum angle is 0.0245° and worst aspect is 2360. No threshold was relaxed.

Reproduce geometry:

```sh
cargo build --offline --release -p ringdesign-core --example bestiarium_arachne
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_arachne showcase/bestiarium/arachne --verify
```

The example leaves the overall gate false until a fresh graph validation updates `report.json` and `verification.json`. The recorded local template probe follows `template_open_probe` phases against this unregistered graph. Catalog registration, publication, tags and version changes are excluded.
