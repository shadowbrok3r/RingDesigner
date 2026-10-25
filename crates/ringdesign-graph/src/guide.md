# RingDesigner node guide

<!--
Plain-language help for every node in the graph editor. The editor reads this file; anything before the first section heading is ignored.

Format:
- A node section starts with a heading line of two hash marks, a space and the exact registry key (for example design.new), and nothing else on that line.
- Value types follow as sections headed type:<label>, and general ideas as sections headed concept:<name>, in the same heading style.
- Paragraphs are separated by one blank line. A line starting with a dash and a space is a bullet. Nothing else is markdown.
- The first paragraph is the summary: one or two short sentences, at most 160 characters. The add-node list and tooltips show it.
- Backticks around a pin or node name are shown as code.
-->

## bool
A true or false switch for any yes/no pin, such as `mirror`, `invert` or `closed`.

Use one when several nodes should share the same switch, or wire it into `stream.gate` or `flow.if` to turn part of a graph on and off.

## int
A whole number for counts and positions: repeats round the ring, beads, stones or a list index.

Wire it into any count pin, such as `repeats_around` or `beads_around`. A value with decimals is rounded.

## number
A single number, such as a width in millimetres or an angle in degrees, to wire into any number pin.

One `number` can drive several pins, so changing it moves them all together. It can also hold a list of numbers.

## range
A list of evenly spaced numbers from a start to an end, both included.

For example 0.2 to 0.5 over 4 items gives 0.2, 0.3, 0.4 and 0.5. `count` is how many numbers you get. Wire the list into a pin and the node downstream runs once per number.

## series
A list of numbers counted up from a start by a fixed step, such as 0, 30, 60, 90 for stations round the ring.

Wire it into an angle pin like `theta_deg` and the node downstream runs once for each angle. To divide the whole ring evenly without working out the step, use `polar.array`.

## text
A piece of text: a name, an inscription or a choice for a menu pin such as `style`.

Wire it into any text pin. It also works where a pattern name is wanted.

## band.profile
Defines the band's cross-section: its width, its thickness and how its outer surface is shaped, from flat to half-round.

Pick a `style` first (it sets the drop law, crown and edge), then set any measurements you want to override. Pins you leave empty keep the values of the `profile` you start from.

Wire the result into the `profile` pin of `design.new`. Switch on `flatten_sides` to square the side faces for ornament: faces that look along the finger always release from a sand mould, while relief on a steep outer wall can lock.

## band.profile.library
Loads a saved band section by name from your profile library, including the bundled factory sections.

With a `profile` wired in, it applies the saved shape but keeps that band's width and thickness: a profile is a shape, never a size. Wire the result into `design.new`. If the name is wrong, the error lists what the library holds.

## band.size
Converts a US ring size into millimetres: bore diameter, circumference and radius, plus the size's label.

Use it when something must be sized to the finger, for example by wiring `inner_diameter_mm` into a calculation. `design.new` takes the US size directly, so you do not need this node just to set the ring's size.

## band.size.fit
Finds the US ring size that matches a bore diameter in millimetres.

Wire `size` into `design.new`. A finger size is rounded to the nearest quarter; past size 20 the ring is treated as a bangle and the bore is kept exact.

## base.preset
Starts the ring from one of the 20 bundled factory signet bodies (001 to 020) instead of a plain swept band.

These are real signet shapes with crisp edges where the walls meet the face. Choose one with `id`. Turn on `sand` for the drafted sand master, which is made to release from a two-part mould. `face_length_mm`, `face_width_mm` and `bore_mm` resize it.

Feed a `design` in (usually from `design.new`) and the result onward to `design.assemble`, where your layers add relief over the stock. `bare` hides the ornament while you inspect the stock.

## build.settings
Sets how finely the ring is turned into a mesh: samples round the ring and across the section, the minimum wall left, and optional refinement.

Wire `build` into the `build` pin of `design.settings`. Higher sample counts give smoother metal but slower previews. `min_wall_mm` stops relief from cutting into the finger hole. You rarely need this node; the app's resolution presets cover most work.

## design.new
Starts a ring: a name, a US size and, if you wire them in, a band section and a shank.

Wire `band.profile` into `profile` and a `shank` node into `shank`. The `design` output then goes to whatever adds to the ring: generators, `layer.tiling.fit`, `cad.feature` and `design.assemble`.

It makes the bare band only. Layers are built separately, collected in a `stack` and put on with `design.assemble`.

## design.settings
Writes build resolution and casting criteria into the finished design, leaving its band, layers and parts untouched.

Wire `build.settings` into `build` and `draft.settings` into `draft`. `crisp_relief` straightens walls that cross the build grid. Any input you leave out keeps the design's current setting. Place it near the end of the graph, just before `sink.output`.

## draft.settings
Sets the rules the castability verdict judges against: sand or lost wax, minimum draft, thinnest section that fills and smallest detail.

Choose `process` (`SandTwoPart` or `LostWax`) and a named `sand` to load its numbers, then override single values if needed. `DelftClay` is the finer sand (3° draft, 0.8 mm section, 0.3 mm detail); `Petrobond` is a little coarser (2.5°, 0.6 mm, 0.4 mm). Wire `draft` into `design.settings`.

Lost wax is more forgiving: undercut no longer stops a ring, only fill and detail do.

## head
Describes a signet's head: the shape of its face, its length, how high it stands over the shank and how the shoulders fall away.

Pick an `outline`, then set `length_mm` along the ring and `rise_mm` above the shank. `hollow_mm` scoops the underside out to lighten a heavy head. `table_dome_mm` gives the face a slight dome instead of a dead-flat plane; a flat table has no draft, so a dome of around 0.7 mm can help the casting verdict.

Wire it into the `head` pin of `shank`, or into `shank.add_head` for a second head. `fit_to_width_mm` sizes the face to a band of that width.

## outline.custom
Makes a head outline from a closed list of points, such as a shape you have drawn or imported.

The points can be in any units: the shape is scaled to fit and its hollows are faired so the body stays castable. It needs at least 8 points round a closed shape. `symmetric_across` and `symmetric_along` even out a hand-drawn outline. Wire the result into `shank.outline`.

## outline.library
Loads a head outline by name from the outline library, which includes 19 bundled plans such as clover, rosette, star, butterfly and escutcheon.

Wire the result into `shank.outline` to put it on a head. An outline of yours saved under the same name takes the place of the bundled one.

## shank
Controls how the band changes round the ring: tapered, waved, twisted, split, bypass, signet and more.

Choose a `kind`, then `amount` (0 to 1) for how strongly it applies. `waves` sets the number of waves for Wave and Twist; it must be whole so the joint closes. A signet shank carries a `head`. The Keyframes kind takes a list of `shank.key` stations in `keys`.

Wire the result into the `shank` pin of `design.new`. For a signet, `shank.signet` sets up the head for you.

## shank.add_head
Adds a second or third head to a shank, each at its own angle, for toi et moi and multi-stone rings.

Wire in the existing `shank` and the extra `head`, whose `theta_deg` says where it sits. The result goes to `design.new`.

## shank.key
Sets the band's width, thickness and crown at one angle round the ring, for a hand-shaped keyframed shank.

Make one for each station, then wire them all into the `keys` pin of a `shank` whose `kind` is Keyframes. The values are multipliers on the band's own section: 1.0 leaves it alone, 1.3 makes it 30% wider. The shape blends smoothly between stations and closes at the joint.

## shank.outline
Puts an imported or saved outline onto a shank's head.

Wire in an `outline` from `outline.library` or `outline.custom`, optionally the `shank` to change and a `band_width_mm` to fit the face to the band. Deeply lobed plans such as a clover are switched to the cut-dome construction so the flank stays smooth. The result goes to `design.new`.

## shank.signet
Builds a signet shank the way the app does: the signet taper, a face fitted to the band and the lofted body.

Set `band_width_mm` to the band's width (it sizes the face) and, if you like, choose a builtin `outline`. Use the same width in `band.profile`. The result goes to `design.new`.

## decal.stamp
Describes one placement of a decal: where it goes round the ring and across the band, how big, how turned and how high.

Make one for each copy and wire them into the `decals` pin of `layer.decals`. Lettering on the High side face needs `flip` on, or it comes out mirrored. Not to be confused with `stamp`, which strikes a true outline.

## entry
Puts a layer into the stack and sets its name, how it blends with what is under it, its strength and where on the ring it applies.

Wire a layer node into `layer`; add a `window` to limit it to an arc, a `mask` to shape where it shows and a `remap` to reshape its relief. Feed the entry into a `stack`.

`blend` says how it meets the relief already there:

- `Max` (the default) keeps the higher of the two, so overlaps merge.
- `Add` piles it on, `Subtract` carves it in and `Replace` overrides what is under it.
- `SmoothMax` and `SmoothMin` round the join over `soft_mm`.

Carving layers such as `layer.openwork` need `Add`: `Max` ignores negative relief.

## gem
Describes a stone: its cut, width, length and whether it is faceted or a cabochon.

A gem is a record, not metal. Seats are sized to it and the stones report counts it. Wire it into `layer.seat`, `layer.seatrun`, `gen.pave`, `gen.halo` or `gen.channel`, or into `layer.seat.fit` to size a seat to it. `w_mm` is the short axis and `l_mm` the long one; `gem.calibrated` fills the length in for you.

## gem.calibrated
Makes a stone from a cut and a width, with the length that cut normally has, and gives its estimated weight.

Outputs `gem`, `l_mm` and `carats`. Choose Faceted or Cabochon: a cabochon has no pavilion, so it needs far less metal under it and is the easiest stone for a cast band to carry.

## layer.border
A rail running all the way round the ring at one position across the band: round wire, flat, knife, stepped or rope.

`v_mm` is the rail's distance from one edge, measured across the surface. With `mirror` on (the default) a second rail sits the same distance from the other edge. `rope_twists` sets the twists round the ring and must be whole.

Wire it into `entry`. Rails sit best on a squared side face or on the middle line; off-centre on a domed crown they can lean back and lock in the sand, so check the castability verdict.

## layer.curve
Sweeps a wire along a curve you draw, repeated round the ring: scrolls, vines and wavy rails.

Each point in `points` is a position along one repeat (0 to 1) and a distance across the band in millimetres. `repeats_around` copies the curve round the ring and must be whole so the pattern closes. `closed` joins the last point to the first for an endless rail; `mirror_v` adds a copy on the other side. `widths`, `heights` and `beads` vary the wire along its length.

Wire it into `entry`. Wires are safest on a side face; across a domed crown the flank can lean back and lock.

## layer.curve.preset
Gives a ready-made wire for a design: a scroll, a vine or a closed wavy rail, sized to its band.

Wire in the `design` and choose the `preset`. To adjust it, wire the result into a `layer.curve` node's `layer` pin. The wire is centred on the middle of the band, not on a side face, so on a domed band check the castability verdict or move it onto a side face.

## layer.decals
Places copies of one pattern at spots you choose, each with its own size, angle and height: scrolls, stars, a crest.

Set the `alpha` and wire one `decal.stamp` per copy into `decals`. Nothing repeats on a grid, so use it for one-off or off-centre motifs; `layer.tiling` is for repeating patterns. `feather_mm` softens each stamp's edge. Wire it into `entry`.

## layer.flutes
Evenly spaced ridges or grooves running across the band or round it: reeding and fluting.

`count` is the number round the ring and must be whole. Choose a `profile`: Round, Vee or Square. In the `entry`, a `Max` blend stands them up as reeds and `Subtract` cuts them as grooves. `along` runs them round the ring instead.

`lean` drifts them diagonally. In sand keep it under about 0.1, or the flanks lean back and lock.

## layer.group
Combines a stack of layers into one layer, so it can be blended, windowed and masked as a single unit.

Wire a `stack` in and the group out to an `entry`. A `Replace` blend inside the group stays inside it instead of wiping the layers beneath. `clamp` holds the group to the sand's draft rule: relief that rises too steeply is cut back, never filled in.

## layer.milgrain
A row of tiny beads running round the ring at one position across the band.

`v_mm` places the row across the band. `bead_diameter_mm` and `height_mm` size the beads, and `beads_around` counts them; it must be whole so the row closes. With `mirror` on, a second row sits the same distance from the other edge.

The middle of the band is half of `band_v_len_mm` from `design.info`. Beads finer than the sand's detail floor (about 0.3 to 0.4 mm) cast soft; `sink.dfm` warns.

## layer.openwork
Carves a pierced-looking pattern down toward a floor kept above the finger hole, so it looks pierced but keeps a wall.

Wire a `layer.tiling` into `tiling`: the ink of its pattern is what is carved. `depth_mm` is the deepest cut and `keep_mm` the metal left over the bore. In the `entry`, set `blend` to `Add`; the default `Max` ignores carving.

Best on a side face. Deep cuts from both faces can leave a thin web between them that the wall check cannot see.

## layer.seat
Casts the metal stock for one stone: a boss, a bezel collar or a tall gypsy mound, shaped to the stone's outline.

Place it with `theta_deg` (round the ring; 90° is the top) and `v_mm` (across the band), and size it with `diameter_mm` and `height_mm`. To size it from a stone, make a `gem`, run it through `layer.seat.fit`, and wire that into this node's `layer` pin.

`solid` adds a made setting (Flush, Bead, Prong or Bezel). A sand pattern leaves it to the bench and keeps only the stock and a drill-start dot, `mark_mm`. Wire the seat into `entry`.

## layer.seat.fit
Sizes a seat to a stone: the stone's plan, stock and set depth are filled in for you.

Wire in the `gem`, and optionally a seat to start from. To place the result, wire it into a `layer.seat` node's `layer` pin and set `theta_deg` and `v_mm`. A fitted seat also makes a good prototype for `layer.seatrun`.

## layer.seatrun
A row of identical seats round the ring, as on an eternity band, optionally graduating in size.

Give it a prototype `seat`, a `gem`, a `count` (whole) and the `bridge_mm` of metal between neighbours. `taper` shrinks the stones toward the far side; the largest sits at `taper_theta_deg`. `layer.seatrun.solve` finds the count that fits.

Put a `window` on its `entry` for a half eternity. With `bare` on, the mounds are cast without setting stones.

## layer.seatrun.solve
Works out how many stones of this size fit round the ring with the bridge you want, measured in metal.

Wire in the `design` and the run. The `layer` output is the run with its count set, ready for `entry`; `count` and `bridge_mm` report the result. Measuring in metal rather than on the flat chart keeps the bridge honest on a curved band.

## layer.signet
A flat-faced pad standing on the band, like a small table. For a real signet head, use a signet shank instead.

A signet's head is better made part of the band itself (`shank.signet` with `head`), which is built to release from the mould. This pad sits on top of the band, so keep it for small flat facets on an ordinary band.

## layer.tiling
Repeats a pattern image (an alpha) round the ring and across the band as raised relief: scales, rope, Greek key, your own art.

Pick the `alpha` (from `alpha.library` or one of the other alpha nodes), then `repeats_around` (whole, so the pattern closes at the joint) and `rows`. `v_center_mm` and `v_span_mm` choose which stretch of the band it covers, and `height_mm` how high the relief stands. `mirror_v` repeats it on the other side face.

`layer.tiling.fit` is an easier start. Wire the result into `entry`. Detail finer than about 0.3 mm casts soft in sand.

## layer.tiling.fit
Makes a tiling sized to your band the way the app's add menu does: on the side faces, with square cells.

Wire in the `design` and choose the `alpha`. If the band has no side face (a plain dome) the tiling stays where it started and a warning says so. Wire the output into `entry`, or into a `layer.tiling` node's `layer` pin to adjust height and other settings.

## remap.curve
Reshapes how a layer's relief rises, through a curve, for cushioned tops or chamfered take-offs.

`points` are [x, d] pairs from (0, 0) to (1, 1): x is the relief height as a share of `span_mm`, d is the height it becomes. Points above the straight diagonal round the top off. Wire the result into the `remap` pin of `entry`. Only raised relief is reshaped.

## remap.terrace
Steps a layer's relief into flat terraces, like contour layers, with sharp or soft risers.

`steps` (1 to 64) is the number of terraces over `span_mm` of relief; `riser` runs from near-vertical (0) to sloping (1). Wire the result into the `remap` pin of `entry`. Steep risers are walls: fine on a side face, risky on a dome.

## stamp
Strikes a true closed outline into or onto the ring, as a crisp shape with its own top rather than a pixel pattern.

Give it an `outline` (from the `stamp.outline` nodes), a place (`theta_deg` round the ring, `v_mm` across the band) and a size: `height_mm` stands it over the surface and `sink_mm` reaches under it (for a cut, the depth). A `top` from `stamp.top` shapes it. `cut` takes it out of the metal; `bench` adds it after the pour and leaves it out of the sand pattern; `along_pull` keeps its walls parallel to the mould pull on a side face.

Stamps are not layers and do not go in a stack. Wire `stamp` into `design.stamps`, or into `stamp.row` to repeat it along a line.

## stamp.outline.blossom
A flower outline with the number of petals you choose.

`diameter` is measured across the petal tips. Wire `outline` into a `stamp`.

## stamp.outline.comb_lobe
A six-sided lobe with rounded corners, like a comb tooth.

Set `length`, `width` and the corner `round`. Wire `outline` into a `stamp`.

## stamp.outline.fork
A forked shape: a stem that splits into two arms at a spread angle.

Set the overall `length`, the `spread_deg` between the arms and the `stem_width` and `arm_width`. Wire `outline` into a `stamp`.

## stamp.outline.jaw
A curved jaw: an arc with a row of teeth pointing inward.

`radius` and `width` size the arc, `sweep_deg` says how far it curves, `teeth` how many, and `tooth_mm` how long each is. Wire `outline` into a `stamp`.

## stamp.outline.keel
A long, narrow hexagon pointed at both ends, like a boat's keel.

Set `length` and `width`; `tip` is the width of the blunted ends. Wire `outline` into a `stamp`.

## stamp.outline.lanceolate
A lance-head shape, widest a third of the way up from a rounded base to a pointed tip.

Set `length`, `width` and the blunting `tip`. Wire `outline` into a `stamp`.

## stamp.outline.leaf
A leaf outline with a smooth, serrated, lobed or holly-spined edge.

Choose the `margin`. For the toothed edges, `count` is the number of teeth, lobes or spines and `depth` how deep they cut; `lean_deg` tilts serrations and spines. Wire `outline` into a `stamp`.

## stamp.outline.moon
A moon-phase outline: `lit` runs from 0 (new) to 1 (full), so 0.5 is a half moon.

`horn` squares off a crescent's points so the metal can hold them. In sand a raised crescent fills in to a half moon, so keep crescents for lost wax or cut them at the bench. Wire `outline` into a `stamp`.

## stamp.outline.quill
A quill or feather-blade shape, widest a fifth of the way up from a rounded base to a pointed tip.

Set `length`, `width` and the blunting `tip`. Wire `outline` into a `stamp`.

## stamp.outline.rounded_triangle
A triangle pointing along the stamp, with rounded corners.

Set `width`, `height` and the corner `radius`. Wire `outline` into a `stamp`.

## stamp.outline.spiral
A tapering spiral strip with rounded ends.

`turns`, `start_radius` and `end_radius` shape the winding, and `start_width` and `end_width` the strip. If the strip is wider than the gap between turns it crosses itself and the node reports an error. Wire `outline` into a `stamp`.

## stamp.row
Strikes a stamp again and again along a line: the parting line, a fixed line across the band, or a side face.

Wire in the `design` and the `stamp` to repeat. Choose `path` (`PartingLine`, `ChartV` or `SideFace`), the `count` of stations (up to 400) and the arc from `from_deg` to `to_deg`. `taper` makes the last stamp smaller than the first; `mirror_shoulders` repeats the row on the other side of the head.

The `stamps` list goes into `design.stamps`.

## stamp.top
Shapes the top of a stamp, or the floor of a cut: flat, gabled, ridged, conical, domed, tapered or pillow.

`shape` picks the form, and each form reads only its own pins: `rise_mm` and `axis_deg` for a gable, `apex_mm` for a cone, `crown_mm` for a dome or pillow, and so on. Wire `top` into the `top` pin of `stamp`.

## window
Limits a layer to an arc of the ring, such as the shoulders either side of a stone.

Think of it as a stencil laid round the ring. `theta_deg` is the centre of the arc (90° is the top) and `span_deg` the stretch where relief shows at full strength. It fades out over `fade_deg` beyond each end, a fifth of the span if left empty. `invert` keeps everything except the arc.

`v_gate` can also limit the layer across the band:

- `band`: a strip `band_span_mm` wide, centred `band_center_mm` across.
- `side_faces`: only the flat side faces (`side_pick` chooses which).
- `draft`: only where the surface faces along the finger.

Wire it into the `window` pin of `entry`. Leave some fade: a hard end raises a wall the mould has to clear.

## gen.channel
A channel setting: two rails with a recessed groove between them for a row of stones, on the wider side face.

Wire in the `design` and the `gem` the channel is for; `recess_mm` is the groove depth. It needs a thick, squared band (the stone's width plus about 1.4 mm of side face) and reports an error if the face is too narrow. The `entry` goes into a `stack`.

It stays live: change the band and it re-solves. Stones are set at the bench.

## gen.halo
A centre stone on a domed plate, ringed by small accent stones at equal spacing.

Give it the `design`, the `center` stone and the `accent` stone. `gap_mm` is the space between them, `bridge_mm` the metal between accents, and a `count` of 0 lets the ring decide. The plate casts as one clean dome with the accents marked on it for the setter to drill and bead. `accents` reports how many were placed.

The `entry` goes into a `stack`.

## gen.pave
Packs a field of tiny stones (pavé) over part of the ring in staggered rows that fit exactly.

Give it the `design` and the melee `gem`. `region` picks a side face or a strip across the band; `theta_deg` and `span_deg` pick how much of the ring (360 is all of it). `bridge_mm` is the metal between stones. `seats`, `rows` and `note` report what it placed and what it had to leave out; there is a cap of 240 seats.

The `entry` goes into a `stack`. It re-packs when the band or the pins change.

## alpha.drawn
A hand-painted pattern: brush strokes on a canvas, the same format the apps use for painting directly on the band.

Normally made by painting in the app. `strokes` takes the stroke list; `wrap_x` joins the left and right edges so the pattern closes round the ring. Wire `source` into the `alphas` pin of `design.assemble` and `alpha` into a layer.

## alpha.library
A pattern that already exists in the library, picked by name: a bundled pattern or an image in your alpha folder.

The name is checked, and a near miss gets a list of close matches. Wire `alpha` into a tiling or decal layer. Library patterns need no entry in `design.assemble`; only patterns you make in the graph do.

## alpha.png
Embeds a greyscale image as a pattern: lighter areas become higher relief. The image is saved with the design.

Choose the image in the node and give it a `name`. Wire `source` into the `alphas` pin of `design.assemble` so the design carries it, and `alpha` into the layer that uses it.

## alpha.proc
Tunes one of the built-in seamless patterns (rope, braid, scales, Greek key and others) and gives it a name of your own.

Choose a `kind`. `repeats` repeats the pattern within the tile (1 to 6 take effect), `quarter_turns` rotates it and `gamma` changes its depth: above 1 deepens, below 1 flattens. Wire `source` into `design.assemble` and `alpha` into the layer.

## alpha.svg
Turns vector art (an SVG drawing) into relief: inked areas rise, or sink if you invert. Text inside the SVG is not drawn.

Paste the drawing into `svg`. For lettering use `alpha.text`. Wire `source` into `design.assemble` and `alpha` into the layer. Lines finer than the sand's detail floor cast soft.

## alpha.text
An inscription set in a bundled font (serif, script or blackletter), ready to emboss on the band.

Type the words in `text`, choose the `font` and adjust `tracking` (letter spacing). Use `layer.decals` to place the words where you want; a tiling would stretch one copy round the whole ring. Wire `source` into `design.assemble` and `alpha` into the layer. Fine script strokes may be finer than the sand holds; check `sink.dfm`.

## cad.feature
Adds one solid part to the ring's build history: a stone, a claw head, a cutter, a sweep or any other shape, placed on the band.

Chain these nodes: each takes the design so far and returns it with one more part, so the order of the chain is the order of the build. Wire an operation into `operation` (from the `cad.op` nodes, `path.sweep` or the `sketch` nodes). The node's own settings say how the part meets the band (separate, join or cut) and whether it is cast or added at the bench.

The placement pins move it: `theta_deg` round the ring, `height_mm` off the surface, `across_mm` across the band.

## cad.features
Adds a whole list of parts in one go, one per item, for example a prickle at every point along the crest.

Wire lists of operations and placements (for instance the `placements` from `path.crest`); the shorter list repeats its last item. `attach` sets how all of them meet the band. The `ids` output lists the new parts' feature ids, for later builders to point at.

## cad.op.cutter.azure
Cuts round or teardrop windows up through the band under a stone, letting light in from below.

Choose the `shape`, `count` and `size_mm`; `radius_mm` and `turn_deg` place them round the stone. Wire `operation` into a `cad.feature`, and set `on` to the id of the stone's feature. Pins left empty take defaults worked out from the stone.

## cad.op.cutter.pierce
Cuts a shaped hole through the band along its surface normal: round, oval, marquise, heart or drop, with a bright-cut chamfer at the rim.

Choose the `shape` and size it with `width_mm`, `length_mm` and `turn_deg`. `through` opens it to the bore; otherwise it stops at `depth_mm`. `chamfer_mm` sets the bright-cut bevel. Wire `operation` into a `cad.feature`; pins left empty take defaults.

## cad.op.cutter.split
Cuts a slot from crest to bore that parts the band into two rails, as in a split shank. Judged for lost wax.

Set the centre `theta_deg`, the half arc `spread_deg`, the `gap_mm`, `rail_round_mm` and `tip`. Wire `operation` into a `cad.feature`. For a sand-castable look-alike use the Split kind of `shank`, which carves the groove into the side faces instead.

## cad.op.cutter.window
Cuts a gallery window through the band along the pull, with drafted walls and metal rails left inside and outside.

`from_deg` and `to_deg` give the arc; `rail_in_mm` and `rail_out_mm` the metal kept; `draft_deg` the wall draft and `tip_round_mm` the rounding at the ends. Wire `operation` into a `cad.feature`.

## cad.op.halo
Small stones set in collets or claw heads round a stone at equal spacing, tied by a rail underneath.

Set `melee_mm`, `gap_mm`, `bridge_mm`, `count`, `drop_mm` (how far below the centre stone they sit) and the melee `style`. This builds real made settings in the CAD history; the `gen.halo` generator makes cast stock instead. Wire `operation` into a `cad.feature` and set `on` to the centre stone's feature.

## cad.op.head.basket
Claws tied together with gallery rails from the base up to just under the girdle, so the pavilion sits in a cage.

Settings: `prongs`, `wire_mm`, `rails` (0 to 6), and the claw `style`, `grouping`, `tip` and `rise`. Wire `operation` into a `cad.feature` and set `on` to the stone's feature. Pins left empty take defaults worked out from the stone.

## cad.op.head.bezel
A bezel (collet) round a stone: a tapered wall, a bearing ledge at the pavilion's angle and a lip leaning up the crown.

`wall_mm` sets the wall and `lip` how far up the crown the lip reaches. Wire `operation` into a `cad.feature` and set `on` to the stone's feature.

## cad.op.head.claw
A claw (prong) head: claws leaning out from a base rail, bent over the crown and notched by the stone itself.

Choose `prongs`, `wire_mm`, `rails`, `grouping`, `tip` and the claw `style` (Wire, Talon, Fang, Tentacle, Thorn or Sepal). Because the stone itself notches the claws, they fit it exactly. Wire `operation` into a `cad.feature` and set `on` to the stone's feature.

## cad.op.seat.bur
What the setter cuts under a stone: the bur that seats the girdle, with room for the pavilion and a pilot hole.

`through` drills the pilot out to the finger. Wire `operation` into a `cad.feature` that cuts into the band, and set `on` to the stone's feature. Stage it as a bench cut so the sand pattern leaves it out.

## cad.op.shank.cathedral
Two wire arches rising from the band's shoulders either side of the stone up to the underside of its head, joined to both.

`spread_deg` sets how far apart the arches start, `rise` how high they go and `wire_mm` their thickness. Wire `operation` into a `cad.feature` and set `on` to the stone's feature.

## cad.op.stone
The stone itself as a reference part that the settings are built round. It is never metal.

Choose the `cut`, `w_mm`, `l_mm` and `form`. Wire `operation` into a `cad.feature`, then point the other builders' `on` pins at that feature.

## cad.source
Starts a CAD build history from a whole ring's nominal parameters, for graphs made of `cad.feature` steps.

It holds the ring in its own settings and outputs it with an empty feature list. Wire `design` into the first `cad.feature`. Most graphs start from `design.new` instead; this node is mostly what converting a CAD design to a graph creates.

## cluster
A saved group of nodes used as one node. Its pins are whatever the saved graph chose to expose.

Bundled clusters include the signet builder and the Gothic tracery pieces (wheel window, pointed-arch section, rose tracery, lancet arcade). The graph travels inside the node, so a design opens anywhere. The editor's Collapse folds selected nodes into a cluster.

## design.assemble
Puts a layer stack and the pattern images it needs onto a design: the step that actually adds your ornament to the ring.

Wire the design from `design.new` into `design`, your last `stack` into `stack`, and the `source` outputs of any `alpha.proc`, `alpha.text`, `alpha.svg`, `alpha.drawn` or `alpha.png` nodes into `alphas`. It replaces the design's layers with the stack you give it; leave `stack` empty to keep the design's own.

Continue to `design.settings` and `sink.output`.

## design.get
Reads any value from a design by its JSON path, such as /profile/width_mm. An advanced escape hatch.

The result is JSON; use `json.get` or the math nodes on it. For the common values, `design.info` is easier.

## design.info
Reports a design's vital statistics: name, size, bore, band width and thickness, the unrolled band's two lengths and its layer stack.

`band_v_len_mm` is the distance across the band surface from edge to edge. Half of it is the middle line, so `band_v_len_mm` times 0.5 into a layer's `v_mm` puts a rail or bead row on the centre (the Braided band template does this). `circumference_mm` is the length round the ring at the crest.

## design.resize
Resizes the whole ring to an exact bore diameter in millimetres, keeping the stones and, by default, the head.

With `preserve_head` the signet head keeps its size. With `preserve_pitch` the repeat counts of tilings change so the pattern spacing stays the same at the new size, rounded to whole repeats. Wire the `design` in and out.

## design.set
Writes a value into any existing field of a design by its JSON path. An advanced escape hatch for what has no node yet.

`pointer` names the field, for example /profile/width_mm, and `value` is what to write. The field must already exist and the result must still be a valid design, or the node reports an error. Prefer the dedicated nodes where they exist.

## design.stamps
Adds a list of struck stamps to a design. Stamps are separate from the layer stack.

Wire the `design` and the `stamps` from `stamp` or `stamp.row`. With `replace` on, they take the place of the stamps already there instead of being added.

## path.along
Copies a part along a path, such as a row of leaves round the band, turning and scaling each copy as you ask.

The first part stands at the path's start and each copy follows. `sources` are the feature ids of the parts to copy and `path` comes from `path.crest` (its `along` output) or another path node. `count` includes the original. `alternate_deg` of 180 alternates sides; `roll_deg` of 137.5 gives a sunflower spiral; `scale_start` and `scale_end` grade the sizes. Wire `operation` into a `cad.feature`.

## path.arc
A circular path round the finger's axis, full or partial, as points in world millimetres.

A whole turn ends on its start, which closes the path for an array. Wire `path` into `path.sweep` or `path.along`.

## path.climb
A stem that winds from edge to edge across the band as it travels round the ring, like a climbing vine.

`turns` is the number of swings across the band per turn of the ring, `amplitude_mm` how far it swings either side of the crest and `lift_mm` how far it floats off the band. `path` is in world millimetres; `chart` gives the same route as ring angle and position across the band, which follows the band when the ring is resized.

## path.crest
Points evenly spaced along the band's crest, from one ring angle to another, as a path, as placements or as an array route.

`placements` gives one ring placement per point, turned to run along the crest: wire it into `cad.features` to put a part at each point. `along` is the same crest as a route for `path.along`.

## path.helix
A helical path winding round the finger and climbing `pitch_mm` along the finger per turn.

Negative `turns` wind the other way. Wire `path` into `path.sweep` or `path.along`.

## path.sweep
Turns a path into a sweep: a round wire, or any section, carried along it.

`radius_mm` sets a round wire; `section` takes a sketch or profile instead, square to the path at its start. A path has 2 to 128 stations. Wire `operation` into a `cad.feature`.

## path.wreath
The paths for a woven wreath round the finger: several strands that wave in and out and cross each other.

Each strand's path is a separate item in the `paths` list; sweep them with `path.sweep` and a `cad.feature`, using the same value for `cane_mm` and the sweep's radius. Strands cross with a real overlap (`overlap_mm`), never just touching.

## sketch.gothic.arcade
A row of pointed (lancet) arched bays standing on a sill, as niches to cut or as one stamp outline.

Set `bays`, `width_mm`, `height_mm`, `bar_mm` (the post between bays), `sill_mm` and the arch `head`. Wire `operation` into a `cad.feature` to cut the niches, or `outline` into a `stamp`.

## sketch.gothic.arch
A ring cross-section shaped as a blunt pointed arch standing on the bore, drawn to be revolved about the finger.

Set the bore radius, width, thickness, `keel` and corner `fillet_mm`. Every section is a single crest falling to vertical feet, so the ring it makes releases from two-part sand. Wire `operation` into a `cad.feature` and revolve it about the finger.

## sketch.gothic.lights
Gothic window openings (lights) radiating from a centre: a wheel window round the finger, or a rose on a table.

Each light is a closed loop with a bar of metal between neighbours. Set `count`, the `sill_r_mm` and `apex_r_mm` radii, `bar_mm` and the head shape (Pointed, Round, Trefoil or Mouchette). Wire `operation` into a `cad.feature`, which an extrusion can then cut with.

## sketch.library
A drawn sketch from the library by name: the bundled Gothic outlines, tracery nets and artwork, or your own.

`name` is the sketch's path without .svg, for example gothic/fleur-de-lis, and `scale` enlarges it about its origin. Wire `operation` into a `cad.feature`, or through `sketch.tracery` first if it is a net.

## sketch.text
Text set in a bundled font as drawn outlines you can cut or raise: straight or round a circle, mirrored for a seal.

The holes inside letters are kept. A `radius_mm` above 0 sets the text round a circle; `span_deg` or `width_mm` solve the letter spacing to fill a given arc or length. Wire `operation` into a `cad.feature` and cut it with an extrusion. A very long text is set a part at a time with `part`.

## sketch.tracery
Turns a net of crossing lines into tracery: each closed cell is redrawn half a bar inside itself as a light, leaving metal between.

Wire a sketch operation carrying the net into `operation` (from `sketch.library`, for example). `bar_mm` is the metal left between neighbouring lights. `lights` counts them and `skipped` says which cells could not be drawn and why. Wire `operation` onward into a `cad.feature`.

## stack
Collects entries, or bare layers, in order into a layer stack, bottom first, ready for `design.assemble`.

Wire entries into `entries`. Each pin takes one wire, so to add several at once join them with `list.merge`, or chain another `stack` whose `stack` pin takes this one: each stack appends to the one before. Later entries are applied after earlier ones, which matters for `Subtract` and `Replace` blends.

A layer wired straight in becomes an entry named after its kind with default settings. Send the last stack into `design.assemble`.

## gate.castable
Passes the design on only if it releases from the mould; otherwise it fails with the verdict's reasons and nothing downstream runs.

Put it between the finished design and a file-writing sink. "Castable with care" passes unless you turn `allow_marginal` off. Wire a `field` report in to save recomputing it.

## sink.build
Turns the design into a watertight mesh at a resolution preset (Draft, Preview, Fine, Export or Maximum), with its volume and weights.

`mesh` goes to `sink.export` or `sink.render`. `weights` gives grams and pennyweight for each metal. Leave `soften_mm` at 0 for anything you will export; above 0 it softens fine detail to preview how it will pour.

## sink.dfm
Checks each layer's finest feature against the smallest detail the sand can reproduce.

DFM means design for manufacture. `findings` lists the layers that are too fine, `count` how many, and `summary` gives one line each. A finding means the detail will cast soft; it does not stop the ring pouring.

## sink.export
Writes the mesh to a file (STL, OBJ, PLY, 3MF or GLB), and can scale it up to allow for a metal's casting shrinkage.

Set `path` and `format`. `shrink_metal` scales the mesh to a pattern for that metal and renames the file as a pattern. In SandRing mode the ring is judged first, so wire the `design` (or its `field` report) as well; a ring that will not release is refused.

The editor skips file-writing nodes; they write only when the graph is run from the command line with `--run-sinks`.

## sink.field_verdict
Judges whether the ring will release from a two-part sand mould, and which layer is to blame for any undercut.

`verdict` is Castable, Castable with care, or Will not release. Also read `undercut_pct`, `worst_draft_deg`, `thinnest_wall_mm`, `parting_z_mm` and `notes`. Wire `field` into `gate.castable`, `sink.export` or `sink.sheet` so it is not recomputed.

The wall figure measures outward from the finger hole; it cannot see a thin web between deep carves on opposite faces.

## sink.mesh_verdict
Judges a mesh by the directions of its faces, for solids the normal verdict cannot read. Free mode only.

The finger hole counts as a vertical wall, never an undercut. Outputs `verdict`, `undercut_pct`, `worst_draft_deg`, `parting_z_mm` and a count of `vertical_faces`. Use it on the mesh from `solid.mesh`.

## sink.output
Marks the finished ring: the design wired in here is the one the editor builds, shows and judges.

Put one at the end of the main chain, usually after `design.settings`. The editor evaluates only the nodes that lead into it, so a node that is not connected to the output does nothing there. Without an output node, the editor falls back to the last design any node produced, which may not be the one you meant.

## sink.refine
Builds a mesh to a tolerance instead of a step count, putting triangles only where the surface needs them.

Choose a `preset` (Coarse, Draft, Fine or Export) or override `tolerance_mm` and `normal_tolerance_deg`. Use it for export; `sink.build` is quicker for previews. It does not work on imported factory stock; use `sink.build` there.

## sink.render
Draws a picture of a mesh: a PNG still, or a looping turntable GIF.

Set `path`, the camera `yaw` and `pitch` in radians, the image size in `edge` and a metal `tint`. Turn on `turntable` for a spinning GIF of `frames` images. It writes a file, so it runs only from the command line with `--run-sinks`.

## sink.save_design
Saves the design as a .ring.json file, with its pattern images embedded so it opens on any machine.

In SandRing mode a ring that will not release is refused. It writes a file, so it runs only from the command line with `--run-sinks`.

## sink.sheet
Makes the printable casting sheet as a web page: dimensions, weights, the verdict, detail findings, the stones table and provenance.

The `html` output goes to `sink.write_text` to save it. Wire in `field` and `stones` reports if you already have them; otherwise they are computed.

## sink.stones
The setter's bench check for every seat: footing, edge clearance, pavilion room, bridges, carats and which stones sit too close.

`count` and `carats` total the stones; `tight_pairs` counts neighbours with too little metal between them and `crowding_note` names the tightest pair. `warnings` lists per-seat problems. Wire in the `field` report if you have it, for the parting plane.

## sink.write_text
Writes text to a file, such as the casting sheet, a manifest or a note.

Set `path` and `text`. Like the other file writers it runs only from the command line with `--run-sinks`.

## cad.inspect
Measures the parts of a CAD design and checks them for interference: sizes, volumes and findings.

Wire the finished design in and read the `report`; nothing is changed.

## flow.if
Picks one of two values depending on a true or false condition.

Wire a boolean into `cond`, and the values into `a` (used when true) and `b` (used when false). It works per item, so a list of conditions gives a list of choices.

## json.get
Reads one field out of structured data by its JSON path, such as /width_mm. For advanced graphs.

Leave `pointer` empty to get the whole value. Any serialisable value works, including a design.

## json.set
Returns a copy of structured data with one field changed at a JSON path.

Objects are created on the way if the path does not exist yet. For designs, `design.set` does the same job on the design itself.

## list.cull_index
Removes the items at the positions you list. Positions wrap round, and negative ones count from the end.

Turn `wrap` off to make an out-of-range position an error instead. Wire a list into the square `items` pin.

## list.cull_nth
Drops every n-th item (the n-th, 2n-th and so on), for example every other station.

With `n` of 2 the second, fourth and sixth items go. Wire a list into the square `items` pin.

## list.cull_pattern
Keeps or drops items following a repeating true and false pattern.

A pattern of true, false, false keeps one item in three. The pattern repeats along the list.

## list.dispatch
Splits a list in two by a repeating true and false pattern: true items go to `a`, false items to `b`.

Use it to treat alternate stations differently, for example large and small stones round a ring.

## list.entwine
Gathers up to four lists as the branches of one list of lists.

An advanced list tool. To simply join lists end to end, use `list.merge`.

## list.flatten
Opens every nested list into one flat list.

Use it when a node hands back a list of lists that the next node wants as a single list.

## list.graft
Wraps each item in its own one-item list, so the next node treats every item as a separate branch.

An advanced list tool; most graphs never need it.

## list.item
Picks one item out of a list by its position; 0 is the first and negative positions count from the end.

With `wrap` on, a position past the end wraps round to the start; with it off, that is an error.

## list.length
Counts the items in a list.

Wire a list into the square `items` pin; `n` is the count.

## list.merge
Joins up to four lists, or single values, end to end into one list.

This is how you hand several entries to one `stack`: a pin takes only one wire, so merge them first.

## list.partition
Cuts a list into chunks of the sizes you give, repeating the size pattern; the last chunk keeps whatever is left.

Sizes of 2, 3 make chunks of 2, 3, 2, 3 and so on. The result is a list of lists.

## list.repeat
Repeats a list's items until the list is the length you want.

Handy for a pattern such as true, false repeated across 24 stations.

## list.reverse
Puts a list in the opposite order.

Wire a list into the square `items` pin.

## list.shift
Rotates a list: with a positive offset the first items move to the end.

With `wrap` off the shifted items are dropped instead of moved round.

## list.slice
Takes a stretch of a list from a start position up to, but not including, an end position.

Negative positions count from the end, so an end of -1 stops before the last item.

## list.sort
Sorts numbers by value or words alphabetically; a mixed list is refused.

Turn on `descending` for largest first.

## list.sort_keys
Sorts a list of keys and carries a second list of values along in the same new order.

Use it to order stones by size, say, with the stone list following.

## list.split
Divides a list in two at a position: `a` is everything before it, `b` everything from it on.

A negative position counts from the end.

## list.sum
Adds up a list of numbers and gives the total and the mean.

An empty list has a mean of 0.

## list.weave
Interleaves up to four lists following a pattern of list numbers, such as 0, 1, 1.

That pattern takes one item from list 0, then two from list 1, and repeats. Every item comes out exactly once.

## logic.and
True only when both a and b are true.

Combine two switches, for example two conditions that must both hold.

## logic.not
The opposite of a true or false value.

True becomes false and false becomes true.

## logic.or
True when either a or b (or both) is true.

Combine two switches where either one is enough.

## manufacturing.inspect
Inspects the casting pattern as it would go into the mould: release from the flask, stock and wall thickness.

Wire the design in and read the `report`. The design's own manufacturing setup is used if it has one.

## math.abs
The size of a number with any minus sign dropped.

The result is never negative.

## math.add
Adds two numbers: a plus b.

Like every math node it works on lists too: give a pin a list and the sum is done for each item.

## math.atan2
Gives the angle in degrees of a direction: `a` is the distance up (y) and `b` the distance across (x).

Useful for turning a position into a ring angle.

## math.ceil
Rounds a number up to the next whole number.

For example 4.1 becomes 5.

## math.clamp
Holds a number between a minimum and a maximum.

Anything below `min` becomes `min`; anything above `max` becomes `max`.

## math.compare
Compares two numbers with `<`, `<=`, `>`, `>=`, `=` or `!=` and gives true or false.

Wire the result into `flow.if`, `stream.gate` or a checkbox pin.

## math.cos
The cosine of an angle given in degrees.

The result runs from -1 to 1.

## math.deg
Converts an angle from radians to degrees.

Use it with camera pins that are in radians.

## math.div
Divides a by b. B must not be zero.

A list item that divides by zero fails with a message and the rest carry on.

## math.floor
Rounds a number down to the whole number below.

For example 4.9 becomes 4.

## math.lerp
Blends between two numbers: a `t` of 0 gives a, 1 gives b and 0.5 the point halfway.

A `t` outside 0 to 1 carries on past the ends.

## math.max
The larger of two numbers.

Wire in a and b.

## math.min
The smaller of two numbers.

Wire in a and b.

## math.mod
The remainder after dividing a by b, taking the sign of b, so -30 mod 360 is 330.

Good for wrapping an angle back onto the ring.

## math.mul
Multiplies two numbers: a times b.

Works on lists item by item.

## math.neg
Flips the sign of a number.

Positive becomes negative and the other way round.

## math.pow
Raises a to the power b.

For example 2 to the power 3 is 8.

## math.rad
Converts an angle from degrees to radians.

Use it where a pin or script wants radians.

## math.remap
Carries a number from one range to another in proportion, without clamping.

For example 0.5 in the range 0 to 1 becomes 180 in the range 90 to 270. Follow it with `math.clamp` to stay within the new range.

## math.round
Rounds a number to the nearest whole number.

For example 4.5 becomes 5.

## math.sin
The sine of an angle given in degrees.

The result runs from -1 to 1; sin of 90 is 1.

## math.sqrt
The square root of a number. The number must not be negative.

A negative item fails with a message.

## math.sub
Subtracts b from a.

Works on lists item by item.

## math.tan
The tangent of an angle given in degrees.

Beware of angles near 90 and 270, where it grows without limit.

## polar.array
Divides the ring into evenly spaced angles: a full turn gives `count` stations with none doubled at the joint, a partial arc includes both ends.

`start_deg` is the first station (90° is the top) and `span_deg` the arc covered (360 is the whole ring). Wire `angles` into a `theta_deg` pin and the node downstream runs once per angle: six angles into a `layer.seat` make six seats. `step_deg` is the angle between stations.

## stream.gate
Switches a value on or off: when closed it sends nothing on, so everything below it runs zero times.

Wire a boolean into `open` to include or leave out an optional feature, such as a row of beads, without deleting its nodes.

## text.concat
Joins two pieces of text with an optional separator between them.

For a list of parts use `text.join`.

## text.format
Fills a template with values: `{0}` to `{3}` (or `{a}` to `{d}`) are replaced by the values on `a` to `d`, written out as text.

For example the template "Ring size {0}" with a size of 7 gives "Ring size 7".

## text.join
Joins a list into one piece of text with a separator between the parts; numbers are written out.

The default separator is a comma and a space.

## script
Runs a short script for anything the other nodes cannot do, with pins you declare in its first lines.

Declare pins in header comments, for example `// in: a: Number = 1.0, b: List<Number>` and `// out: total: Number`. Each input arrives as a variable and every output must be set by the script. `i` and `n` say which item of a list this is and how many there are. The language is rhai, and a script can only calculate.

## solid.cylinder
A cylinder along Z, or a cone if you give a different top radius.

Free mode only. It stands on z = 0 unless `center` is on. `segments` sets how many flat facets make up the circle.

## solid.sphere
A sphere centred on the origin.

Free mode only. `segments` sets how many facets make up the surface. Move it with `solid.translate` or `solid.place`.

## solid.box
A box with one corner at the origin, or centred on it.

Free mode only. Set the sizes along x, y and z in millimetres.

## solid.extrude
Extrudes a closed flat outline in the XY plane up along Z into a solid.

Free mode only. `points` is a path of [x, y] pairs, for example from a `stamp.outline` node; `height` is how tall.

## solid.revolve
Revolves a flat section about the Y axis into a ring-shaped solid.

Free mode only. The section is [radius, height] pairs at radius 0 or more, `degrees` says how far round to turn, and 360 is a complete ring.

## solid.from_design
Takes the design's own band into the kernel as a solid, so settings and vines can be added to a band the castability verdict has already judged.

Free mode only. Choose the sweep `preset` for its resolution. The result goes to `solid.union` and friends.

## solid.union
Joins two solids into one.

Free mode only. Use `solid.union_all` for a longer list.

## solid.difference
Cuts the second solid out of the first.

Free mode only. `a` is kept and `b` is cut away.

## solid.intersect
Keeps only the volume the two solids share.

Free mode only.

## solid.union_all
Joins a whole list of solids into one in a single step.

Free mode only. Wire a list of solids into the square `solids` pin. This is faster than chaining `solid.union`.

## solid.translate
Moves a solid by x, y and z millimetres.

Free mode only. To put a solid on the ring instead, use `frame.on_ring` and `solid.place`.

## solid.rotate
Turns a solid about the X, then the Y, then the Z axis, in degrees.

Free mode only.

## solid.scale
Scales a solid by a factor along each axis.

Free mode only. Factors must be above zero; 1 leaves that axis alone.

## frame.on_ring
Makes a coordinate frame on the band at a ring angle, with Z pointing out of the metal: the spot where a setting or a leaf goes.

Free mode only. Wire in a `design` (for the crest radius) or give a `radius_mm`. `theta_deg` is the angle (90° is the top), `axial_mm` moves along the finger and `extra_mm` outward from the crest; `tilt_deg` and `roll_deg` turn it. Wire `frame` into `solid.place`.

## solid.place
Moves a solid, built at the origin with Z up, into a frame such as one from `frame.on_ring`.

Free mode only. Build the part at the origin first, then place it.

## solid.tube
A round wire following a list of [x, y, z] points, with spheres at the corners: vines, stems and tendrils.

Free mode only. `radius` is the wire's radius and `segments` how smooth its round section is.

## solid.setting
A ready-made four-prong setting, round or marquise, standing on z = 0 with the girdle rail at the seat height.

Free mode only. Set the stone's `width` (and `length` for a marquise) and `seat_h`; `cross_brace` adds bars under a marquise.

## solid.leaf
A domed leaf with a midrib and veins cut in, lying flat on the XY plane.

Free mode only. Set `length`, `width` and `thickness` in millimetres.

## solid.import
Brings in an OBJ or STL file as a solid; the mesh must be watertight.

Free mode only. Give the file's `path`. A mesh with holes is refused.

## solid.vine_semimount
The finished vine semi-mount: a round band wire, a vine over the top, a marquise centre with flanking stones, buds and leaves, thirteen stones in all.

Free mode only, and made for lost wax, not sand. Set the bore and the wire radius; it returns the solid, the stone count, the total carats and a list of the stones. Wire `solid` into `solid.mesh`.

## solid.mesh
Turns a solid into a mesh, with its volume and surface area, so it can be checked, exported or rendered.

Free mode only. Wire `mesh` into `sink.mesh_verdict`, `sink.export` or `sink.render`.

## type:any
A pin of this type takes any kind of value; each value is checked when it arrives.

List tools, `flow.if` and the `json` nodes use it.

## type:null
Nothing. It marks an item that failed upstream and flows on instead of stopping the whole graph.

The node that failed shows the reason.

## type:number
A number that may have decimals: millimetres, degrees or a fraction.

`number`, `series`, `range` and the `math` nodes make them, and most size and angle pins take them. Whole numbers and true or false convert automatically.

## type:integer
A whole number for counts and positions, such as repeats, beads or a list index.

`int` makes one. A number with decimals is rounded when it is wired into an integer pin.

## type:boolean
True or false.

`bool`, `logic.and`, `logic.or`, `logic.not` and `math.compare` make them; checkbox pins, `flow.if` and `stream.gate` take them.

## type:text
Words: names, inscriptions and menu choices such as `style` or `profile`.

`text`, `text.concat`, `text.format` and `text.join` make them. Numbers and pattern names convert to text.

## type:list
Several values in order.

Square pins take a whole list; a list wired into a round pin makes the node run once per item. `series`, `range` and `polar.array` make lists, and `list.merge`, `list.slice` and the other list nodes reshape them.

## type:design
A whole ring in progress: band, shank, layers, stamps and parts.

`design.new`, `base.preset` and `design.assemble` make or change one; generators, sinks and `design.info` take one.

## type:profile
The band's cross-section: its width, thickness and surface shape.

`band.profile` and `band.profile.library` make one for the `profile` pin of `design.new`.

## type:shank
How the band changes round the ring, with any signet heads it carries.

`shank`, `shank.signet`, `shank.add_head` and `shank.outline` make one for `design.new`.

## type:head
A signet head: its outline, size and construction.

`head` makes one for `shank` or `shank.add_head`.

## type:outline
A head's plan shape, with its hollows faired for casting.

`outline.library` and `outline.custom` make one for `shank.outline`. Stamp outlines are paths, not this type.

## type:gem
A stone record: cut, width, length and form.

`gem` and `gem.calibrated` make one; seats, runs and generators take it.

## type:window
An arc of the ring, with an optional gate across the band, that limits where a layer applies.

`window` makes one for the `window` pin of `entry`.

## type:remap
A curve or stepping that reshapes a layer's relief.

`remap.curve` and `remap.terrace` make one for the `remap` pin of `entry`.

## type:layer
One kind of relief (a tiling, border, seat and so on) without its name or blend.

The layer nodes make them; `entry` and `stack` take them, and a layer node's own `layer` pin starts from one.

## type:entry
A layer together with its name, blend, opacity and window.

`entry` and the generators (`gen.pave`, `gen.halo`, `gen.channel`) make them; `stack` takes them.

## type:stack
An ordered list of entries, bottom first.

`stack` makes one; `design.assemble` and `layer.group` take one; `design.info` reads one back.

## type:recipe
The record a generator keeps so it can re-solve its layout when the band changes.

It rides inside the groups that `gen.pave`, `gen.halo` and `gen.channel` make; no node has a pin of this type.

## type:alpha source
The definition of a pattern created inside the graph: procedural, text, SVG, drawn or PNG.

The alpha nodes output it, and `design.assemble` takes it so the design carries the pattern.

## type:alpha
A pattern's name. An alpha is a greyscale height image: lighter means higher relief.

`alpha.library` and the `alpha` outputs of the other alpha nodes give one; `layer.tiling`, `layer.decals` and the `mask` pin of `entry` take it.

## type:build
Settings for turning the ring into a mesh.

The built-in nodes carry them as json, so no pin shows this type today.

## type:field report
The castability report for a design: undercut, draft, wall, parting plane and notes.

`sink.field_verdict` and `gate.castable` make it; `sink.export`, `sink.save_design`, `sink.sheet` and `sink.stones` can take it.

## type:stones report
The bench check of every stone and seat.

`sink.stones` makes it and `sink.sheet` can take it.

## type:mesh
The triangle mesh of a ring or a solid.

`sink.build`, `sink.refine` and `solid.mesh` make one; `sink.export`, `sink.render` and `sink.mesh_verdict` take it.

## type:solid
A closed 3D solid from the free-mode kernel.

The solid nodes make and combine them, and `solid.mesh` turns one into a mesh. Free mode only, in a build that includes the kernel.

## type:path
A list of [x, y] points tracing a shape.

The stamp outline nodes make closed outlines; `stamp`, `outline.custom`, `layer.curve`, `remap.curve` and `solid.extrude` take them.

## type:json
Structured data in JSON form: stamps, tops, operations, frames, settings and reports.

Most of it passes untouched between matching nodes, and `json.get` reads a piece out. The path nodes make 3D paths as json.

## concept:overview
A ring graph is a recipe read from left to right: shape the metal, add ornament, assemble, then read the result.

First comes the shape of the metal. `band.profile` sets the cross-section and `shank` (with a `head` for a signet) sets how the band changes round the ring. Both feed `design.new`, which makes the bare ring.

Then the ornament. Each layer node (`layer.tiling`, `layer.seat`, `layer.border` and so on) makes one piece of relief. An `entry` gives a layer its name, its blend and, if you want, a `window`. `stack` nodes collect the entries in order, and `design.assemble` puts the finished stack onto the design.

Last, the settings and the result. `design.settings` (with `build.settings` and `draft.settings`) says how to build and judge the ring, and `sink.output` marks the design the editor shows. Report sinks such as `sink.field_verdict` check the finished design.

An unwired pin shows a value you can type. Wire it to another node to set it from there.

## concept:layers
The ornament on a ring is a stack of layers, laid one on another from the bottom up.

- A layer node (`layer.tiling`, `layer.seat`, `layer.border`, `layer.milgrain` and the rest) makes one kind of relief: raised or carved metal.
- An `entry` wraps a layer with its settings: a name, how it blends with the layers under it, its strength, and optionally a `window`, a `mask` and a `remap`.
- A `window` limits an entry to an arc of the ring (90° is the top) and, if you like, to part of the band's width, with a fade at each edge.
- A `stack` lists entries in order, and `design.assemble` puts the stack on the design.

A tiny example, wired left to right:

- `design.new` makes the bare band.
- `layer.tiling.fit` with the Scales alpha makes a tiling sized to the side faces.
- `entry` takes that layer. A `window` with centre 90°, span 100° and `invert` on, wired into the entry, keeps the scales off the top.
- `stack` takes the entry, and `design.assemble` takes the stack and the design.
- `design.settings` and then `sink.output` finish the graph.

Positions work the same for every layer: `theta_deg` runs round the ring (90° is the top) and `v_mm` runs across the band, measured over the surface from one edge. Half of `band_v_len_mm` from `design.info` is the middle line.

## concept:wiring
Pins are coloured by type: round pins take one value, square pins take a list, and a wire only joins compatible types.

Pins are the dots on a node's edges: inputs on the left edge, outputs on the right.

A wire joins an output to an input of a compatible type. A number can go into a whole-number pin (it is rounded) or a text pin (it is written out), and text and pattern names are interchangeable. A layer can go straight into an `entry` pin or a `stack` pin, and an entry into a `stack` pin. Anything else is refused with a message naming both types.

An input takes only one wire, and a node cannot feed itself. Feed a list into a round pin and the node runs once for each item, giving a list of results; shorter lists repeat their last item. Six angles into a seat's `theta_deg` make six seats.

A wire overrides the value typed on the pin. Optional pins left empty keep the base's value.

Drag a wire into empty space and let go: the editor offers nodes that fit.
