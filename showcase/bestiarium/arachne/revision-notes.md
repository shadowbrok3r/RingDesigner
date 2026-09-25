Arachne now passes the draft and export geometry gates and the cold graph gate. Art-director approval remains pending.

| Gate | Result |
| --- | --- |
| Draft / export triangles | 666,134 / 1,478,112 |
| Watertight; boundary / non-manifold / degenerate faces | yes; 0 / 0 / 0 |
| Ring / made-part self-crossings | 0 / 0 |
| CAD and setting notes; DFM findings | 0; 0 |
| Lost-wax field verdict | Castable |
| Investment floor; minimum band wall | 0.8 mm; 1.89 mm |
| Minimum leg / claw diameter | 0.8 mm / 0.8 mm |
| Minimum bend radius / tube radius | 1.269 |
| Minimum actual part-vertex clearance outside local bore | 0.392 mm; 0 vertices below the 0.10 mm gate |
| Minimum sampled leg-to-abdomen clearance | 0.422 mm |
| Leg IV length; claw angle | 12.502 mm; 124° |
| Minimum leg cheek share | 0.571 |
| Axial extent | 8.774 mm |
| Stones reported / previewed; metal vertices inside stones | 2 / 2; 0 |
| Empty-library design reload | identical vertices and faces |
| Cold graph evaluation | identical design bytes, vertices, faces and normals |
| Design patches | 4: three draft fields and shank keys |

The eight legs now use one linearly tapered Twist per limb segment, radii 0.58 → 0.52 → 0.47 → 0.43 → 0.40 mm, with knuckles at 1.12 times the local radius. The hind leg is raised and shortened. The onyx is 9 × 7 mm at a 0.4 mm rise. The flared, rolled collets, pedicel and spinnerets preserve the spider anatomy.

The crown has 0.30 × 0.30 mm radials, separately layered 0.21 × 0.20 mm capture silk, a 1.15 mm main pitch, open irregular hubs and edge fades over 0.25 mm. Cheek silk uses upright 1.3 mm-pitch radials and two catenary rows with 0.25 mm sag. It stops 0.30 mm short of the bore, fades over 0.20 mm, and clears leg footprints and the fragments beneath them. Quantized masks resolve relief to about 0.0012 mm.

Remaining limitations:

- The design is 475,459 bytes and the graph is 514,239 bytes. Both exceed the 300 KB procedural budget, which becomes binding after P7. The stored collets remain because `head.bezel` exposes wall and lip but cannot reproduce the 9° flare and rolled equator.
- Segment-to-knuckle fillets are not claimed. `parts::resolve` unions the part cluster before laying `blend_mm` on cluster-to-band seams; it cannot fillet internal part-to-part joins. Setting 0.12 mm on these parts produces missing-seam warnings. The current knuckles have real overlaps and no CAD notes.
- Before export cleaning, exact boolean intersections leave small slivers: the worst leg count is 7, down from the prior review's 70–165. The 0.001 mm export clean remains watertight with no degenerate faces; minimum face angle is 0.145°.

Reproduce geometry:

```sh
cargo build --offline --release -p ringdesign-core --example bestiarium_arachne
systemd-run --user --scope -p MemoryMax=8G --quiet -- target/release/examples/bestiarium_arachne --verify
```

The example records geometry separately and leaves the overall gate false until a fresh graph validation updates `report.json` and `verification.json`. The local template probe follows `template_open_probe` phases against this unregistered graph; phase timings and the size flag are in `verification.json`. No catalog registration, publication, tag or version change is included.
