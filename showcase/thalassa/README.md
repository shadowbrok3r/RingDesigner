# Thalassa — tidal reliquary signet

A marine signet in yellow gold with a shell cartouche, paired tidal scrolls, seven icy blue stones, engraved currents and satin recesses. Its 27 layer entries (24 at the top level) exceed Nocturne’s 22 entries and three stones.

- [Android highlight reel](highlights/thalassa-android.mp4)
- [Desktop highlight reel](highlights/thalassa-desktop.mp4)
- [Editable portable project](design.ring.json)
- [Hero render](hero.png) · [Face render](face.png)
- [Nominal ring STL](nominal-ring.stl) · [Inspection](inspection.json) · [Stone report](stones.json)

| Element | Editable tools used |
|---|---|
| Oval signet body | Head outline, shoulder transition, dome, rounding, hollowing and comfort fit |
| Shell face | Nested layer group, custom SVG relief, oval framing and seed pearls |
| Shoulders | Mirrored scroll alphas, compass accents and flowing curve rails |
| Surface | Warped procedural guilloche, hammered satin, borders, milgrain and flutes |
| Gallery | Recessed openwork with a retained floor; palm-side hallmark |
| Settings | Central oval, four face accents and two shoulder stones with seat stock |
| Handwork | Three engraved current strokes and a Mariner compass placed using the new viewport tools |

The initial composition was authored with the application’s Rust design tools in
`crates/ringdesign-core/examples/thalassa.rs`. An additional engraving stroke and
compass stamp were made in the Android app and retained in this project. Both
reels show real application interaction; the desktop demonstration edits are
undone after inspection. The PNGs are separate high-resolution renders.

The portable project embeds its artwork. It opens in either app without external
alpha files. Rebuild exports from the finished source without discarding the
recorded handwork:

```sh
cargo run --locked --release -p ringdesign-core --example thalassa -- \
  /tmp/thalassa-export --source showcase/thalassa/design.ring.json
```

The exported nominal mesh has 737,280 triangles and is watertight. The stone
report finds zero tight pairs; the closest modelled gap is 2.89 mm. These are
investment-casting designs: the open gallery, delicate ornament and setting
crowns obstruct a two-part sand pull. The mould animation deliberately shows
those obstructions. It is a sampled cavity study, not a machined mould model.
The STL is the nominal body; apply the workshop recipe’s compensation when
preparing a production pattern, and finish settings to the actual stones.

Raw recordings, action timing and chapter captures are retained under
`highlights/android/` and `highlights/desktop/`. Android was recorded in an Android
36 emulator at 1440 × 3120; desktop was captured from its own X11 application
window. Captions occupy added space above the capture. Review details are in
[VIEWPORT-TOOLS-REVIEW.md](../../docs/VIEWPORT-TOOLS-REVIEW.md).

Reassemble the chaptered MP4s from the retained captures:

```sh
python tools/assemble_reels.py showcase/thalassa/highlights/edit.json
```

The edit list removes an idle wait from the desktop stamp demonstration. Each
output includes chapter markers and a JSON manifest with timing, input hashes
and complete video-decoding verification.
