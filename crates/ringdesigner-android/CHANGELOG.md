# Changelog

## 0.28.0 — 2026-09-19

- CAD Workshop groups creation and modification tools into menus with dedicated colored SVG icons and real example thumbnails. Missing source prerequisites and the unsupported generic twisted sweep are explicitly disabled.
- Workshop previews use the shared navigation cube, with orbit, pan, zoom, fit and view lock. The cube stays out of the fixed app chrome when scrolling properties, and the unused placement magnifier is disabled in CAD. Numeric parameter rows keep labels left and values right.
- Painting shows its brush footprint on the 3D preview and smoothly follows drawing drags. Follow brush can hold the camera still; locked views and manual navigation remain respected.
- Every menu button draws a visible down caret. Geometry hover keeps a stable feature caption without claiming individual mesh polygons are editable.

## 0.27.1 — 2026-09-19

- Reopening the alpha picker after filtering restores full thumbnail and caption space. The dialog follows the phone's safe content area, including keyboard and rotation changes.

## 0.27.0 — 2026-09-19

- Graph PNG sources open a visual, searchable alpha picker instead of editing base64 text. Cached previews keep large images out of text layout; chosen artwork remains embedded and portable.
- New design includes all 31 authored templates in eight collections, with real ring thumbnails and preview images.
- Reptilia bands no longer include dormant signet controls. Authored graphs omit unused and overwritten artwork sources while preserving their effective geometry and images.
- Darker button and selectable backgrounds retain readable text, colored icons and selection/hover outlines.

## 0.26.0 — 2026-09-19

- File → Feature request / bug report opens a form for a prefilled GitHub issue. Add a title and details, review it on GitHub, then submit it there. The form includes app version, platform and workspace; it does not attach design files or logs.
- Menu, icon and compact buttons share a consistent 32-point height. Selectable controls retain their visible frames and colours.
- Node titles no longer select text when dragged. Graph layouts account for output badges appearing after the first evaluation.

## 0.25.0 — 2026-09-19

- Graph opens in its own workspace with contextual navigation and hides unrelated modeling palettes. Graph view can still dock beside the ring; each modeling mode remembers its inspector panel.
- Colored Atelier icons and permanent button frames make selectable labels and dimension controls easier to recognize. Graph nodes show hover and selection borders.
- A viewport-local Clear button releases feature, node and stone selection, exits active tools and releases dimension handles for free orbiting. Empty-space taps clear selection; Escape dismisses menus first, then clears selection. Mouse and hovering pen users see the selectable feature under the pointer.
- The viewport shows an explicit Cutters button while seat cutters are visible, so their overlay can be hidden independently of selection.
- Unavailable history, graph parameter and editing actions are disabled; view controls are scoped to visual workspaces.

## 0.24.0 — 2026-09-19

- Reptilia adds five editable presets in File → New: Ecdysis (snake belly band), Tessera (shield mosaic band), Lorica (crocodile armour band), Ophidian (factory 013 with a purple 7 × 5 mm oval stone), and Varanus (fully patterned factory 017). Each includes its geometry, artwork, recipe graph and separate casting and bench-finishing layers.
- Four reusable patterns join the library: Snake keels, Ventral scutes, Crocodile scutes and Reptile shields.
- Stones have a saved preview colour, editable under Stone & setting. Ophidian opens with its amethyst colour on desktop and Android; stone dimensions and setting geometry stay independent of colour.

## 0.23.0 — 2026-09-19

- A new ring in File → New → Stock masterworks: Caiman — armoured hide, on the factory's octagonal signet (preset 015) with one emerald, held to two-part sand. One crocodile hide runs from back to belly. Rows of dorsal plates cross the face and run down both shoulders, grading from broad plates on the spine to small ones at the rim. A crest of keeled horns is struck along the spine, and the emerald-cut stone is flush set as the central plate of the nuchal shield, with a window drilled through beneath it. The head's walls carry granular flank skin studded with rows of bony knobs, and the palm carries the belly's broad scutes. After the pour the bench punches pits into the plates and cuts the lines that divide the belly into tiles. It reads zero withdrawal obstructions at every resolution checked.
- Each part of the hide is its own layer and the crest is one family of stamps, so the recipe graph and the build reel show the ring assembled in steps: dorsal armour, flank granules, the knobs, the belly scutes, the graver's work, the stone's plate, the horns, then the burs, the cut seat and the stone.
- The build reel strikes a design's stamps as steps of their own, family by family, before the setting burs are ghosted: Caiman's crest of horns, Zenith's moons and star trails. A ring whose only made parts are stamps no longer shows burs and seat cuts it does not have.

## 0.22.1 — 2026-09-18

- Stamps: an outline extruded off the ring's surface and joined to it, or cut from it, by boolean — relief with true walls and a silhouette as crisp as its polygon, which the height field cannot hold at any resolution. A stamp's top follows the surface it stands on at one height; on a head's leaning wall it can stand along the mould's pull instead, so neither edge tucks under. A stamp can be marked as made at the bench, which keeps it in the finished ring and out of the pattern. They show in the viewport, obey Live cuts, and their cutters appear under Show cutters.
- Zenith's sky is struck, not painted. The moon's phases down each shoulder, the crescent beside Orion and the star trails on the cheeks are stamps now: the true outlines, limb a half circle and terminator the half ellipse it really is. The painted versions had been cut back by the casting rule until they read as arrows, and their edges combed where the rule bit column by column. The full, gibbous and half moons cast as they are; a crescent's inner edge faces back across the parting line wherever it is put, so each crescent is cast as its half moon and cut to the terminator at the bench, and each new moon is cast a disc and milled to its rim. The pattern STL carries the blanks; the ring on screen is the finished sky.
- Shared renders and turntables use the viewport's own studio: reflected softboxes through the alloy's coloured reflectance, and stones as a dielectric over a tinted body, instead of a plain key light.
- Rotating the ring by hand tumbles it all the way round. Dragging the face away used to stop at the back view and turn the ring upside down; the tilt now runs on past the back and over the top, and a sideways drag keeps carrying the surface under your finger even while the ring is upside down.
- Pinching the recipe graph no longer zooms, pans or rolls the ring. A pinch belongs to the pane the first finger landed on; before, egui reported one gesture for the whole screen and both panes took it.
- The graph keeps the zoom you give it. Lifting a pinch was being read as a tap on whatever node sat under the first finger — choosing it and turning the ring to it — and two quick pinches as the double tap that fits the whole graph. A pinch is never a tap now, a double tap no longer re-fits on a touch screen (the Fit button does), choosing a node that is already on screen leaves the view where it is, and re-measuring a laid-out graph no longer re-fits it.
- Zenith's waxing full and half moons now join the ring: their tops were built with a sliver of triangle that belonged outside the outline, which left the stamp not quite closed.
- A saved design reopens exactly. Numbers were read back from a file up to one unit in the last place off, which nothing noticed until a stamp's outline went through the boolean and came back a hair different.
- Exported STLs from made settings and stamps carry no zero-width triangles. Where a cutter lined up with the ring's own mesh the boolean kept two vertices a ten-thousandth of a micron apart, and the face between them had no area; those are merged or turned away now, and the part stays closed.
- The fine-detail findings read stamps. Each poured stamp's outline is measured the way a texture is, so a thin arm is named and a pointed tip is not held against it. Zenith's star trails used to taper to 0.19 mm, under the sand's floor; they now end at 0.36 mm.

## 0.22.0 — 2026-09-18

- Stone settings are made parts now, not bumps in the surface. A seat's new "Made setting" picker (Stone & setting, in the seat's editor) gives it a claw head, a collet, a flush cut or a bead-set cut, built beforehand to the stone's own millimetres and joined to or cut from the ring by boolean: claws lean out from a gallery rail, bend once and lie along the crown, notched by the stone itself so they bear on it; a collet has a tapered wall, a bearing ledge at the pavilion's angle and a lip up the crown; the setting bur leaves a bright-cut bevel, a girdle wall with a lip over the stone, a bearing cone and a pilot, drilled through to the finger with "Drill through". Bead-set neighbours share the beads between them. Every cut and plan is covered — round, oval, cushion, princess, emerald, marquise, pear, cabochons — and runs, pave and halos carry them too. The result is one watertight mesh at any resolution.
- Seats fit their stones on signet heads. A "True size" seat is now round along the ring as well as across it: the chart measures round the ring at the band's reference radius, and a signet's table stands about 15% further out, so a seat there was drawn that much too long and gaped at both sides of its stone.
- A flat-topped seat no longer has a moat round it. The boss's top rolled off to nothing at its rim and the skirt then started back up at full height, cutting a groove round every skirted seat; the top now meets its skirt.
- The ring can be stood on its head. Twist with two fingers to roll the view — it turns with your fingers, ignores the wobble of a pinch, and settles onto the nearest quarter turn when you let go near one — or tap the new Upside down button on the view cube. The cube and its labels roll with the view, dragging still moves the surface under the finger, and any named view stands the ring upright again. Views → Roll left, Roll right and Upside down do the same in quarter turns.
- Two new rings in File → New → Stock masterworks, each one theme from face to palm, on the factory's own preset signets with their hard-edged faces, and held to two-part sand. Saurian — beaded skin (preset 013, one stone) wears a single reptile form throughout: pointed, keeled scutes the width of the band, graded from the face down both shoulders to a polished palm, with small round scales on the head's walls. Zenith — the hunter's belt (preset 017, three stones) is the night sky: Orion on the face with his belt as the three stones, lying on the parting line; a crescent beside him; the moon's phases down each shoulder; star trails on the cheeks; and the hunter's stars, the lines between them and the Pleiades left to the graver after the pour. Both read zero withdrawal obstructions. Palisade is retired from the templates. Preset 013 joins the imported signet bases (its file was one triangle short of closed).
- Sand rings are shown finished and exported as patterns. The ring on screen has its stones set in cut seats; the STL and 3MF are what the founder is handed — no seats, and a raised dot at each stone's centre where the drill starts. It is raised because a pit's far wall faces back into its own mould half and locks, while a dot on the parting line pulls. The bur takes the dot with the seat. Lost-wax designs export as shown, cuts and heads cast in place.
- View menu → Live cuts resolves made settings into the ring as you edit; off, the ring shows its cast stock alone and builds faster. View menu → Show cutters draws each seat's cutter through the metal as a ghost — bevel, girdle wall, bearing and pilot, where the boolean will take them.
- Tools → Play build reel replays the open design's construction on screen for recording: the bare stock, each layer switching on in turn under its name, the cutters ghosted, the seats cut, the stones set, then a full turn and a flip. Tap the ring to stop; what you had on screen comes back.
- On imported stock, work marked "cut at the bench" is laid on the sand-supported surface instead of under it, so an engraved line is no longer filled in by the withdrawal support.
- Oriel is rebuilt on made settings: its cabochon sits in a collet that fits it, inside a bead-set halo on one plate, with bead-set pave on the shoulders. Choosing a seat's node lights its head as well as its stock.
- The casting verdict says what it did not judge: seats cut with the bur after the pour, and made heads — cast in place under lost wax, soldered on after a sand pour. A manufacturing pattern leaves both out where the process cannot cast them.

## 0.21.0 — 2026-09-18

- The corner navigator is a view cube. It turns with the ring: tap a face to look straight at that side (FACE, PALM, LEFT, RIGHT and the two openings), an edge or a corner to look between them, drag the cube to orbit, tap the arrows to step to the face on that side, double-tap for the home view. Views ease in instead of jumping.
- Mirror now shows the same view from the other shoulder and never turns the face away; the old button looked from the diametrically opposite side, which is still in the Views menu as Opposite side. Left and Right shoulder are also the way round they appear on screen.
- Choosing a graph node always turns the ring to what it does. A layer mirrored onto both shoulders is faced on the nearer shoulder rather than at the head between them; a layer that runs most of the way round is entered from its near end; whole-band nodes frame the whole ring.
- Two new masterworks in File → New → Showcase templates, each with its editable graph. Palisade — deco colonnade is held to two-part sand and reads Castable at 0.0000% undercut: a buff-top oval with a bench-cut sunburst, an emerald cut and graduated rounds on the parting line, gadrooned shoulders that run whole into the head, reeded edges, zigzag wires and crest beads. Oriel — jewelled lantern has no guard rails: a bezel-set cabochon in an 18-stone prong halo laid out in true metal millimetres, pave shoulders, engine turning, rope and milgrain rails, and a pierced lattice gallery, judged for lost wax.
- The casting verdict and the fine-detail findings set aside layers marked "cut at the bench": a graver's line in a signet's table is in the finished ring and not in the pattern, and no longer fails the pour. The verdict says which layers it set aside.

## 0.20.0 — 2026-09-18

- The recipe graph docks under the ring (beside it in landscape): Tools → Recipe graph, or Open graph on a graph-driven design. Drag the grip to share the screen; the expand button gives the graph the whole screen and brings it back.
- Choosing a node lights what it does on the ring, and the ring turns to face it. A layer's reach is measured by building the ring with and without it — the caption gives its share of the surface and how far metal moved — so a window, a mask or a layer that loses its blend shows truthfully, including "no metal moves". Head nodes light the head, section and shank nodes wash the whole band, and build or casting settings say they move nothing.
- Step through the recipe with < and >, jump to any node from the list between them, or follow the chosen node's wires from the In and Out menus. Tap ornament on the ring to open the node that made it.
- Nodes keep their width. The shell's text wrapping reached into the canvas and folded titles to a letter per line, which also threw off Arrange; rows now lay out on one line at any zoom.
- Taps choose the node under the finger at any pan or zoom (they were tested against the wrong coordinates, so a tap elsewhere could change the selection). The title-bar grab grows as you zoom out, so a node can be moved with a fingertip even when the whole graph is in view.
- Arrange places each source beside what it feeds along one straight spine, instead of stacking every source in the first column beside a chain running off screen. A graph that opens with overlapping nodes is arranged once automatically; a tidy one is left alone.
- Rebuilt Nocturne and Solstice on imported stock with ornament fitted to the real face and shoulders, leaving polished rims clear of relief.
- Added Aurelia, a large floral sand-pattern showcase, and Vesper, an unrestricted 15-stone celestial signet. All four stock masterworks include portable artwork, manufacturing setup, and editable graphs in File → New.
- Higher-resolution relief sampling and 16-bit embedded height maps retain fine detail after saving and reopening. Imported stone settings keep their physical dimensions when the face is resized. New saves use format 4; update desktop RingDesigner before opening them there.
- Optional sand withdrawal support fills relief toward the parting plane and keeps a calibrated releasing bore. Casting checks still report low draft and other findings for workshop review.
- Kept the dedicated File button, content-sized half-screen inspector, and menu-opening camera adjustment. Mesh detail labels now account for imported meshes with variable triangle counts.

## 0.19.0 — 2026-09-18

- Dedicated File button on the top bar: New, Open, Save, copy to Downloads, and export access from every workspace.
- Bottom inspector fits its content and scrolls at half the available screen height. Opening menus and tool palettes shifts the ring toward unobstructed space once; manual pan, zoom and orientation remain under your control.
- New design menu includes 19 bundled imported signet bases, Nocturne and Solstice with imported shoulders, and four showcase graph templates. No external asset download is needed.
- Smooth resizing of imported faces and shoulders preserves the bore and source topology. Base-only inspection, dimension reset, deformation validation, and section comparison are available in Shape.
- Imported geometry, artwork, stones, and editable graphs survive save/reload and STL/3MF export. Projects use format version 3; update desktop RingDesigner before opening them there.

## 0.18.1 — 2026-09-17

- The placement magnifier holds the whole stamp: the lens grows to the artwork's own footprint and prints its magnification, easing off 2.5× only when a placement is too large to fit. Its edges are no longer cropped by the circle.

## 0.18.0 — 2026-09-16

- Live 2.5× placement magnifier above the finger for Stamp, Path, Paint and Move; samples the actual mesh and surface overlay. Toggle it in the ring navigator.
- Tap the small ring for face, shoulder, underside and opening views. Quarter-turn arrows, opposite-side flip and 90° tilts keep placement accessible from every side.
- Optional view lock: empty-space dragging pans without rotating; two fingers still pan/zoom and deliberate view buttons still work.
- Empty-space drags orbit with editing tools selected. Each contact keeps its initial editing or navigation role until release, including when crossing the mesh.
- Narrow, vertical 180-point editing palettes; Path Apply stays above repeat options and exact point positions expand when needed.
- Held Path contacts retain their preview and place one draft point on release.

## 0.17.0 — 2026-09-16

- Shared Atelier SVG icons, compact Undo/Redo and tool controls, with finger-hold and 0.7-second hover hints.
- Real alpha thumbnails, searchable pattern grid and surface-conforming artwork preview before placement.
- Move ornament: pick, move, rotate, resize, duplicate and mirror existing stamps, including manual nested groups; preserve masks and Undo.
- Float palettes across the full app; restore a minimized inspector from its Expand button or the always-visible Panel icon.
- Five-stage jewelry Guide: fit, shape, decorate, set stones, inspect and export.
- Pen hover reaches the entire UI; steady hover samples no longer restart the tooltip timer.
- Stationary stamp previews survive long presses and place once on release; immediate Undo includes edits still waiting for the history debounce.

## 0.16.0 — 2026-09-16

- Rounded signet cap/shoulder transitions now change the actual surface, so the
  smoother shape also reaches exported meshes. Continuous fillet sampling avoids
  stepped highlights at profile joins.
- Cut-dome shoulders use a continuous taper with a fixed side-wall join, removing
  the inflated cheek shelf while preserving the face perimeter.
- Surface → Path draws raised wires or engraved curves directly on the ring.
  Drag control points, enter exact positions, repeat and mirror the path, and
  reopen its editable layer. Changes apply as one undoable operation.
- Stamp adds circular/partial-arc copies and mirrored sides in one editable layer.
- Shape → Measure reads point-to-point distance and X/Y/Z spans on the viewport mesh.
- Includes the studio preview, mesh detail and material controls from 0.15.0.

## 0.15.0 — Studio preview candidate

- **See fine ornament.** Detailed preview now settles at 655k triangles. View →
  Mesh detail adds a 1.38M-triangle Showcase option and a Fast option. Edits use
  a lightweight mesh until the gesture ends; the selected quality is remembered.
- **Reflective jewelry materials.** Shared desktop/Android studio reflections,
  colored metal reflectance, highlight compression and separate stone shading.
  View → Metal & polish offers seven alloys and Polished, Satin and Rough finishes.
- **Edit wide borders.** Border controls cover the width of the actual band,
  including the broad cushion used in the sand-signet construction guide.
- **Frame close-ups with a tap.** Zoom in/out buttons are available in View →
  Zoom and the floating Camera & display tools, alongside the existing gestures.

## 0.14.0 — 2026-09-12

- **Choose your viewport space.** Drag the inspector's grip to resize it, or
  hide it while working on the ring. Portrait and landscape sizes are remembered.
- **Move tools beside the model.** A floating tool rail starts on the left.
  Its contextual palettes follow Shape, Surface, Stones and Casting, with layer
  selection, paint/stamps, sections, spacing and mould tools. Drag their headers
  to move them; More groups the remaining commands into submenus.
- **More room for jewelry.** Compact buttons and navigation reduce chrome; the
  old viewport action strip moves into the floating rail. Reset workspace layout
  restores the default positions.
- **Scroll without changing numbers.** Numeric fields now open typed entry on
  tap and leave swipes to scrolling, including slider values, Workshop and graph
  controls. Slider bars and dimensions on the ring remain draggable.

## 0.13.0 — 2026-09-10

- **Paint and stamp on the ring.** Surface mode adds pressure-aware engraving,
  raised strokes and library alpha placement, with millimetre cursors and Undo.
  Artwork stays editable in the existing drawing and layer tools.
- **Drag sections.** Shape mode adds a movable cut plane, filled sections that
  retain the finger opening, and local wall-chord measurements.
- **See stone spacing.** Stones mode shows girdle and pavilion envelopes that
  respond to the requested gap and existing stone edits.
- **Watch mould withdrawal.** Casting mode builds the prepared pattern and
  animates sampled cavity surfaces along its configured pull. Obstruction markers
  and the investment-pattern label explain the study.
- Fine-detail analysis runs in a separate worker, keeping large painted
  designs responsive while their checks finish.
- Shared controls fit narrow inspectors and keep focused fields above the
  keyboard. The same four tools are available in the desktop viewport.

## 0.12.0 — 2026-09-10

- **The ring stays visible.** One compact inspector replaces stacked sheets,
  sits beside the viewport in landscape, and scrolls within the phone's bounds.
  Navigation and menus fit narrow screens and respect Android's safe area.
- **Select what you are editing.** Shape, Surface, Stones and Casting modes
  give taps a clear purpose. Pick an ornament or stone on the model, inspect
  overlapping layers, or temporarily isolate an ornament without changing the design.
- **Visual dimensions.** Drag handles for the opening, width, thickness and
  signet dimensions. Primary controls explain their effect and accept exact values;
  focused fields remain visible when the keyboard opens.
- **See the change.** Continuous parameter edits rebuild previews while the camera
  stays steady. Hold Before to compare, or use the always-accessible Undo and Redo.
  Signets open facing the camera, with direct face and angled views available.
- **Casting feedback.** Pull/parting guides, draft and radial-wall colours, legends
  and pending-check labels connect the preview to the existing mould Workshop.
- **Existing tools stay close.** Pattern browsing and band/tile drawing share space
  with a live ring preview. CAD, recipe graphs, exports and history remain in Tools.

## 0.11.0 — 2026-09-10

- **Workshop on the phone.** Edit casting recipes, mould pull and parting,
  pattern stock, components and manufacturing stages from the new Workshop tab.
- **CAD design tools.** Build and edit parametric rings and signets, constrain
  sketches, inspect sections and measurements, and resize designs with a preview,
  Apply/Cancel and undo.
- **Casting checks and repairs.** Inspect release obstructions, draft and detail
  findings for sand and investment casting; preview supported repairs before
  applying them. Production exports enforce the casting checks, with a separate
  diagnostic export for designs that need work.
- **Portable workshop files.** Import designs and export CAD and manufacturing
  packages with source artwork and reports. Longer workshop jobs run in the
  background so editing stays responsive.
- **Accurate gemstone previews.** Relief and textures no longer move or tilt a
  stone away from the seat used by the setting report.

## 0.10.0

- **On-device models, behind a flag.** Built with `--features local-npu` and a
  model pack on the phone, two things become possible. **Describe a pattern** and
  the NPU makes a tileable one — the latent is rolled between denoising steps so
  the model never sees the wrap as an edge — and the app then measures it against
  the sand's detail floor before offering it, which is the part nothing else
  does. And **index the library** to ask "what else is like this" by long-pressing
  a tile, with near-duplicates flagged rather than quietly accumulating.

  A default build links none of it, and Files says plainly which of the three
  things is missing when it is: the feature, the runtime, or a pack.
- **The pen cuts metal.** Tilt and azimuth now shape the stamp: a pen held
  upright cuts a round bead, a pen laid over cuts a long flat facet along the
  way it leans — because a graver's cut section is set by how the tool is held.
  Pressure still means millimetres of depth; tilt shapes the tool, it does not
  change how deep it goes. Designs written before this open unchanged, and a
  phone-drawn tilted stroke opens on the desktop with the same footprint.
- **Every sample, and only the pen's.** The S-Pen reports 120–240 times a second
  against a 60 fps screen, and the app was keeping one sample a frame, so a fast
  arc landed as a polygon. It now takes every one. And the depth of a stroke came
  from the last pressure seen anywhere — so a hand resting on the glass was
  changing how deep the pen cut. Each sample now carries its own.
- **Hover is a pre-flight.** Before the pen touches, the brush circle shows two
  rings: the depth this spot will take and the part it will refuse. Cross from a
  side face onto the crest and the inner ring collapses. A brush finer than the
  sand can hold is ringed amber. The preview fades in as the tip approaches.
- **The barrel button stopped being a second eraser.** The flipped tip still
  erases, and so does Carve — the button now pans and zooms while held, so the
  band can be moved without putting the pen down, taps to pick up the depth under
  the tip as the new brush depth, and on the second button, steps undo.
- **Haptics that mean something.** A tick the moment the ceiling starts refusing
  the depth being asked for, a lighter one crossing between castability zones, a
  buzz when a build makes the verdict worse, and detents on the quarter sizes.
- **Draw the face, draw the section.** Sketch a closed plan with the pen and it
  becomes the signet head's outline — a real `CustomOutline` carried in the
  design, so it opens on the desktop unchanged and is saved to the outline
  library the desktop reads. Sketch half a cross-section and it becomes a Custom
  profile. Monotone by default, which *is* the no-undercut guarantee; there is an
  explicit switch to draw an undercut and the line turns red while it is on.
- **Pick the stone.** Fourteen cuts, faceted or cabochon, the sizes each cut is
  actually sold in, and three setting styles — with the carat weight and depth
  read back as you choose. Auto pavé and Channel set use what you picked instead
  of the 1.5 mm round they were hardcoded to, and you set the arc: centre, span
  and whether the rows stagger. A fill can also come off the side face onto a
  strip of the band, which is the honest answer to "no side face to fill" on a
  domed profile — and it says what it costs.
- **Add a layer.** Border, milgrain, a gem seat pad, an eternity row, three
  curve presets and a halo. A curve lands on the wider side face when the
  profile has one, retargeted and gated there, because a rail across the crown
  leans back on its crest-side flank while the same wire on a face square to the
  pull measures 0.000%.
- **A Report sheet.** Dimensions, weight in every alloy in grams *and*
  pennyweight, and a per-seat stone table — footing, seat diameter, edge
  clearance, pavilion room, bridges, every bench warning, and the tightest
  neighbours with the gap that decides. All of it was already computed on every
  build and thrown into a tooltip.
- **Files that behave.** Newest first instead of alphabetical, recents at the
  top, rename, and delete behind one confirmation — app storage is unreachable
  from any file manager, so a file the app cannot delete is permanent until
  uninstall. Saving over an existing name warns once instead of silently
  replacing it: two designs called "untitled" used to be one file.
- **It remembers.** The desktop's address and token, the brush, the shading, the
  stone, the shrink alloy — all kept between launches. The sync fields were
  documented as remembered and never were.
- **Renders file under RingDesigner.** They were going into another app's
  gallery album; the folder now comes from the app's own name.
- **The app hears the OS.** `onPause` reaches the app, so an edit made a moment
  before backgrounding is flushed rather than lost to a reaped process.
- **The stack, on the phone.** A Layers sheet lists every layer with its kind,
  and a tap opens it: rename, opacity, blend, and the window that decides where
  round the ring and where across the band it acts — including **snap to side
  faces**, which is how ornament gets onto the two faces square to the mould
  pull, the ground that measures 0.000% undercut at any relief the band holds.
  Mute, solo, copy, delete, and move up, down, to top or to bottom. A layer the
  sand cannot hold is amber, with the finding underneath it.

  Until now the phone could make layers and the only thing it could do to them
  was Clear layers, which deleted all of them — so Auto pavé was one-way, and a
  design pulled from the desktop arrived with a stack it could not open.

  No drag-to-reorder, deliberately: a drag source inside a scrolling touch list
  steals the scroll.
- **Undo, at last.** Every edit on the phone is undoable now, not just the last
  paint stroke: a size change, a shank change, an Auto pavé, a cleared stack, a
  loaded template, a pasted design. Undo and Redo sit in the Ring toolbar and
  name the step they would take back — "Half Round", "Added a layer" — because
  the name is read out of the difference between two designs rather than typed
  at the call site. Long-press Undo for the whole timeline and tap to jump.
  Clear layers used to be one tap with no confirmation and an autosave 90 ms
  behind it; it is now one tap and one tap back.
- **The findings open.** The DFM chip is a button now: tap it and every finding
  reads as text — which layer, which texture, what it measures, and the floor it
  misses. A tiling gets **Fit to the floor**, which sets the repeats to the most
  that pattern can carry and still cast. When no repeat count clears it, the
  layer is left alone and the sheet says how tall the cell has to be instead,
  which is the number worth designing the band around.
- **The sand answers before you commit.** Every tile in Alphas carries its
  finest stroke or gap in millimetres, measured on the cell the current repeat
  count would actually lay down. Anything under the detail floor is rimmed
  amber. The refusal used to arrive after the pattern was on the band.
- **Sand or lost wax, on the phone.** The Design sheet has the process picker
  the desktop has had, with the Delft clay and Petrobond presets under it and
  the three floors printed underneath. Returning to sand restores a sand floor —
  the core writes floors only on the way into lost wax, so coming back would
  otherwise leave a Delft-clay ring judged at investment numbers and reading
  Castable on walls it will not fill.
- The verdict chip names the process. Under lost wax the undercut percentage is
  measured and reported but never gates, so "Castable · 3.10%" is true and
  unreadable without it.

### Logged late

- **Stone map** in Share — every stone to scale in plan and on the unrolled
  band, with the census's tight gaps drawn in red and the gap that decides
  written on the line. Shipped after the 0.9.0 entry was written.
- **The DFM reads the textures.** Findings stopped trusting a tile's cell pitch
  and started measuring the mask itself by granulometry, so a fine-lined alpha
  on coarse cells is caught.

## 0.9.0

- **Graph tab.** The design's recipe graph, edited on the phone: convert the
  current design to nodes, or start from the simple or a template graph; drag
  to pan, pinch to zoom, long-press for the node and background menus, drag
  pins to wire; Arrange, Fit, Lock and Bake. A graph arriving with a pulled
  design shows up the same way. The build worker evaluates the graph before
  every build, and the editing tabs say "driven by the graph" with a Bake
  button while one is in charge.
- **Exports off the UI thread.** STL, 3MF, GLB, the casting sheet, the render
  and the turntable each build on their own thread and open the share sheet
  when the file lands; several can run at once and none is dropped behind a
  preview build.
- **Save a copy to Downloads.** The design file into shared storage through
  MediaStore, where it survives uninstalling the app.

## 0.8.2

- Cut dome slider on the signet head: the face cut from a swollen dome
  instead of the prism construction — no pinched corners, no prism walls.
- Diamond and Cross face outlines.

## 0.8.1

- **Cope and drag.** A fifth shade mode splits the ring into its two mould halves — cope in blue,
  drag in sand — with the parting line glowing bright between them, so where the sand splits is
  visible at a glance.

## 0.8.0

- **The galactic glass look.** The app now wears the same theme as the rest of the family: an
  AMOLED-black page lit by soft colour pools, violet-glass surfaces with real backdrop blur
  behind the bottom chrome, hot pink for what's chosen and aqua for what's ready.
- **A bottom bar that fits.** Ring, Band and Tile stay labelled, the rarer tabs become icon
  squares, and nothing runs off the edge of the screen any more.
- **Design slides up over the ring.** The design controls are a collapsible sheet now — tap
  Design and the sliders rise over the live 3D view, so every change shows on the mesh as you
  drag instead of after a tab-switch. Tap again to tuck it away.
- **Menus instead of button walls.** Files now offers File / New / Export / Share menus that
  open upward (never under the gesture bar), with the templates tucked into New.

## 0.7.0

- **Keyframed shanks.** The new Keyframes shape in Design hands you the band itself: set width,
  thickness and crown at your own stations round the ring and they blend smoothly, closing on
  themselves. Taper, pinch, bombé — or something nobody named yet.
- **The engine underneath grew too.** Crisp mm-true pattern edges, openwork carving with a floor
  over the finger hole, pattern rows that flow along a drawn wave, and export meshes that spend a
  third fewer triangles for the same accuracy — designs made on the desktop with these open here
  and preview correctly.

## 0.6.0

- **A real design editor.** The new Design tab edits the whole ring on the phone: US size,
  profile style and dimensions, every shank shape — including Wave, Twist, Split and Signet with
  its head controls — a second head for a toi et moi, and one-tap stock generators for pavé and
  channel setting. Templates in Files start a design worth editing.
- **Wall heatmap.** A fourth shade mode colours the ring by metal under the surface: red where
  the sand will not fill, through amber and green to blue-grey where it is heavy, with the legend
  in the toolbar. Same ramp as the desktop.
- **Touch the ring, get answers.** Long-press the 3D view and a chip reads back where you landed
  (angle and position across the band), the relief height, the wall thickness, the draft class,
  and which layer put metal there.
- **Stones on their seats.** The viewer now shows faceted preview stones sitting on seat pads and
  eternity rows — display only, never in the mesh, exactly as the desktop draws them. The stones
  chip counts them and carries every bench warning; a DFM chip flags detail the sand cannot hold.
- **Share it moving.** Three new shares: a polished PNG render, a looping turntable GIF, and a
  GLB — glTF in real metres, which AR viewers and web viewers open as a ring-sized ring.

## 0.5.0

- **The honest verdict.** Castability now comes off the surface itself rather than the preview
  mesh, so facet noise can never fake an undercut: the chip shows the field verdict, the undercut
  share, and the thinnest wall over the finger hole — and when something does undercut, the note
  names the arc and the layer to blame ("caused by \"Flat boss\"; muting it clears it").
- **As-cast.** A toggle softens the preview at the sand's own detail radius, so beads merge and
  fine cells mush on screen the way they will in the pour. Display only — exports stay exact.
- **3MF export and pattern shrink.** Share as 3MF (the file states its units, so nothing downstream
  guesses mm vs inches), and cut either export oversize for a chosen alloy's shrink — the file is
  renamed as a pattern so it cannot be poured as nominal by mistake.
- **One brush, both devices.** The pressure-to-millimetres math and the band-layer convention now
  live in the shared core, and the desktop's unrolled editor paints with them too: a band painted
  here opens on the desktop as the same layer, and vice versa.

## 0.4.0

- **Sync with the desktop.** Files gained a host and token; Pull takes whatever the desktop app is
  showing, Push replaces it. Over Tailscale that works from anywhere, not just your own network —
  the desktop binds its `100.x` tailnet address specifically, never `0.0.0.0`, and refuses to serve
  off-loopback without a token. An open port here would let anyone who can reach it rewrite the
  design you are looking at.

## 0.3.0

- **Photograph a surface and cast it.** Pick a photo from the gallery and it becomes an alpha on
  the band. Shoot flat, even light — a photo records the lighting as much as the surface, and
  raking light turns into bumps that are not there, which the picker now says on screen. Imports
  are kept on disk, so they survive a restart instead of coming back as a blank layer.
- **1:1.** The ring renders at true physical size, from the panel's real `DisplayMetrics.xdpi`
  rather than the density bucket Android rounds to — hold the phone against a finger or a mandrel
  and compare. The toggle only appears when the panel reports a DPI worth trusting.

## 0.2.0

The alpha library, and somewhere to put your work.

- **Alphas** — all 16 procedural patterns as a thumbnail grid. Tap one to put it on the band; the
  layer snaps to the side faces where the profile has any, because that is where relief casts
  clean. Repeats and depth adjust live, and the whole set regenerates at 128, 256 or 512.
- **Files** — name, save and reopen designs; export an STL at 1024x320 straight to the share sheet
  with a real `model/stl` type; copy or paste a whole design as JSON, which needs no network and
  works when nothing else does.
- `Alpha::from_bytes` in the core, so an image that arrives as bytes rather than a path can become
  an alpha — the shape every Android file handoff takes.

## 0.1.0

First release. RingDesigner on the phone, for designs that have to come out of a two-part sand
mould.

- **Ring** — the band in 3D, ported to OpenGL ES. One finger orbits, two pinch and pan. Metal,
  draft and normal shading, a barycentric wireframe, and the castability verdict with its undercut
  percentage.
- **Band** — the unrolled `(u, v)` surface with the composited height field drawn underneath, so
  you can see where the metal actually lands. Castability zones are tinted by what they can hold,
  with the ring-angle ruler and the seam marked.
- **Pen** — pressure is millimetres of metal, not opacity, and the ceiling comes from the local
  draft angle: about 1.6 mm on a squared side face, about 0.05 mm on the crest of a half-round.
  Press past what the surface allows and the stroke says so. Hover reads out the limit before you
  commit. The barrel button and the flipped tip carve; palm and finger contacts are rejected while
  Pen only is on.
- **Tile** — draw one motif that wraps in both axes and marches round the ring an integer number of
  times, so it meets itself at the joint by construction.
- **Bench** — per-stage timings of the geometry core on this device.

Drawings are stored as strokes inside the design, not as baked images, so a `.ring.json` stays
self-contained, re-rasterizes at any resolution, and opens on the desktop unchanged.
