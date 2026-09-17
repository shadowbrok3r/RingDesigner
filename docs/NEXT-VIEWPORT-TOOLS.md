# Jewelry viewport work remaining

Reviewed against the working source on 2026-09-16. Older roadmap checkboxes do
not account for the CAD, manufacturing and shared mobile workbench additions.

## Available in 0.18

Desktop and Android share paint, stamps, editable surface paths, circular and
mirrored stamp arrays, mesh distance measurement, section cuts, clearance
envelopes and mould-opening studies. Path points can be dragged or entered as
exact ring angles and across-band millimetres; Add/Apply makes one ordinary,
undoable Curve layer. The Path selector reopens existing top-level curves.

The CAD workspace already has sketches, constraints, sweeps, lofts, booleans,
supported edge fillets/chamfers, component placement and dimension grips. The
main opportunity is making these operations easier to perform on the ring.

0.17 adds individual ornament handles: pick an existing stamp (including manual
nested groups), move it on the surface, rotate or resize it, then Apply once.
Copy and Mirror create another instance in the same masked layer. Generated
groups remain owned by their recipes. A thumbnail browser and surface-conforming
alpha overlay make placement visible before release. The shared SVG controls,
hold/hover help and five-stage Guide connect these tools to a practical workflow.

0.18 adds the live placement loupe, ring-relative orientation navigator, quarter
turns and flips, optional angle lock, and empty-space navigation within drawing
tools. Compact vertical palettes keep Apply visible while advanced settings
expand on demand. See [placement and navigation review](PLACEMENT-NAVIGATION-REVIEW.md).

## Next useful increments

| Priority | Tool | Concrete completion criteria |
| --- | --- | --- |
| 1 | Settings along a drawn path | Place calibrated stones at measured gaps along a surface curve; show clearance and taper before committing an editable generator. |
| 2 | Direct sweep/loft construction | Pick profiles and rails in the viewport, see a solid preview, edit sections in place, and expose the same flow in touch controls. |
| 3 | Surface quality inspection | Zebra reflections plus crease/curvature overlays to distinguish intended arrises, tight blends and sampling defects; connect a selected join to its radius control. |
| 4 | Sketch and component manipulation | Multi-selection, box selection, constrained move/rotate/scale, object snaps and symmetry planes; avoid opening JSON for advanced features. |
| 5 | Bore and file workflows | Incoming Android design-file intents; inside engraving/bench placement and axial-web thickness checks with clear manufacturing stages. |

The new Measure tool reports a straight chord on the displayed mesh, not an
exact geodesic or global minimum wall. Curves are still surface relief, not
freestanding NURBS wires. Path editing supports top-level Curve layers; nested
and graph-generated paths retain their existing editors. Physical S-Pen and
Samsung GPU testing remains separate from emulator checks.

## UI direction

Keep the existing charcoal `#121214`, text `#E9E9EF`, aqua `#2BE2D6`, pink
`#FF3D8B`, amber `#EFB368` and failure red. Retain the installed sans, 13-point
body and 11-point hints. Aqua points are movable; pink lines show the path;
amber marks selection or measurement. Controls remain left-aligned in the
existing scrollable inspector beside/over the ring:

```text
ring + numbered control points       tool inspector
                                     path source / width / depth
                                     copies / mirror / exact point
                                     clear draft / apply
```

The viewport owns the composition. Additional tools use the current contextual
rail and inspector instead of adding permanent panels.
