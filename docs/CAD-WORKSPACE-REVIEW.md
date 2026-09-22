# CAD workspace

CAD edits a history of solid-modeling operations. The procedural Model workspace edits a ring's size, profile and ornament; CAD builds and combines explicit solids and sketch-derived bodies. CAD edits remain a candidate until Apply, so Cancel discards the edit and Undo restores the previous source graph.

## Starting a design

1. Open **Workspace → CAD**.
2. Choose **Create → Example projects** for a rendered starter, or **Create → Procedural shank** to turn the current procedural ring into a CAD feature.
3. Select a feature in the left history. Edit its right-aligned values; a preview starts after input settles. Preview also evaluates explicitly.
4. Choose **Sketch → Edit selected sketch** for extrusion, revolve, sweep or loft profiles. Sketch dimensions and constraints are solved before creating a solid. Rectangle/circle starters keep dimensional constraints; expand Workplane and point dimensions and edit the distance fields to resize them. Point drags alone do not override these dimensions.
5. Click **Apply** after a successful preview. Apply stays disabled while evaluating, on an error, or while viewing a history rollback.

**Modify** contains booleans, fillet, chamfer, shell and component placement. Booleans need two different earlier solids. Click a body to select its feature, or an edge to use its index for fillet/chamfer. Source references remain editable in the inspector. Fillet/chamfer support depends on the kernel's topology; unsupported intersections produce a specific preview error.

The generic **Twisted sweep** creation entry is disabled: the tested default leaves open tessellation edges. **Twisted ring** is supported, including whole and half turns. The unavailable entry explains this distinction instead of creating an invalid candidate.

## Navigation and inspection

- The resizable feature inspector leaves the remainder for geometry. Inspect groups feature properties, components/assembly, section, sizes, manufacturing stages and advanced source.
- The shared cube supports faces, edges, corners, dragging, quarter turns, opposite/mirrored views and orbit lock. Drag orbits; Shift/middle drag pans; scroll zooms. Fit frames visible components.
- The local footer groups view, wire/grid, component isolation/explosion and history rollback controls. Preview on the main toolbar changes CAD's metal, polish and lighting too.
- Previewing a change retains the camera. A new example and explicit Fit reframe it.
- Apply commits the CAD source for regular exports. STEP export carries analytic geometry; assembly packages are available from Components after applying a current design.

The mobile Workshop shares creation/modification icons, real example thumbnails, source prerequisites, aligned fields and cube navigation. Its compact candidate preview remains the existing mesh preview; the main Android ring viewport is GPU-rendered.

## Interaction changes

Command search uses full-width rows, a right-side scrollbar, Up/Down selection, Enter activation and Escape dismissal. Menus draw a vector down caret, independent of font coverage.

Paint mode displays its brush footprint in 3D, follows a drawing drag with smooth orbit/pan, and offers Follow brush to hold the camera. Desktop paint opens a 3D/paint split when started from a single view. Camera following respects manual navigation and orbit lock. Brush controls are grouped in a menu. Hover selection retains the editable feature between raycasts, uses a fixed caption, and no longer outlines or advertises individual mesh triangles.

## Verification

- Existing core CAD tests: 10 passed, including primitives, revolve/sweep/loft, booleans, edge treatments, samples and analytic STEP output.
- Every enabled Create default and all seven Modify defaults build through the real solid kernel with positive volume and closed tessellation. Modifier checks use distinct intersecting boxes and confirm the original document is unchanged.
- Regression suites: desktop 43 passed; shared workbench 33 passed; Android host 132 passed.
- Native inspection verified loading/applying a twisted-band example, sketch/extrusion editing (changing width from 8 to 10 mm changes evaluated volume from 144 to 180 mm³), inspector resizing, full-width search and arrow selection, paint tracking and an unchanged camera with Follow brush off. Moving across different band triangles retains the same feature caption and no triangle outline. Escape closes a CAD menu first; a second Escape discards the candidate. The release binary and a regression test verify both steps.
- ARTEMIS on Android 16 emulator-5554 passed 14/14 initial checks and 7/7 final checks. It verified menu carets, five example thumbnails, CAD Preview/Apply/Cancel, cylinder dimensions, cube navigation, aligned fields, paint camera tracking on/off, and portrait/landscape layouts. Final screenshots confirm the cube disappears when scrolling would cover fixed app chrome, returns with the preview, and the live pink brush outline appears on the 3D ring during a stroke. No new mobile UI test scripts were authored.
- Desktop release 0.5.0 is in `target/releases/ringdesigner-desktop-0.5.0/`. Android 0.28.0 ARM64/x86_64 artifacts are in `target/releases/ringdesigner-android-0.28.0/`. The ARM64 build is published to the app store (code 16784384); the downloaded APK, store metadata and local artifact match SHA-256 `30027ea2759f634a8ffe9fe0da8d7eab83b7fc06974474d1067445cf41d8e0c3`.
- Screenshots, device reports and regression logs are in `target/cad-workspace-review/`. Device traces: initial `112d4b6e-1632-45f2-aa3b-cd1190bed6cc`, final `bbc32176-979d-4c57-8876-32901ba0f190`. Screenshots provide the visual evidence; the optional device video recorder reported a capture/audio error. The post-run ARTEMIS diagnosis passed all five required checks.
