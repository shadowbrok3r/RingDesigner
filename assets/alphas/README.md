# Alphas

Grayscale height maps tiled across the band surface. Black leaves the base
surface alone, white displaces by the owning layer's full `height_mm`.

Drop `.png`, `.jpg`, or `.bmp` files here and they load into the tile library at
startup, named after the file stem. Import at runtime with the Tile Library
panel's **Import images** button.

Guidelines:

- Square images tile most predictably; the layer's cell aspect stretches
  anything else.
- The source should tile seamlessly in both axes. If it does not, either turn on
  `mirror_alternate_u` / `mirror_alternate_v` on the tiling layer (checkerboard
  mirroring butts the edges cleanly) or run the image through
  `Alpha::make_seamless`.
- Use the full 0..255 range. Flat, low-contrast sources produce mush; the
  layer's `contrast` and `bias` can shape the response but cannot invent detail.
- 256px is plenty. The mesh samples far below that resolution.

The 16 built-in patterns are generated procedurally and need no files.
