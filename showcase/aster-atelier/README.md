# Aster Atelier — botanical sand signet

These September 12 native Android and desktop recordings build the same editable
signet from a blank band, then demonstrate layer controls and opposed-pull mould
opening. Their measurements and images below predate the 0.16 shoulder revision;
see the [current surface review](../../docs/SURFACE-TOOLS-REVIEW.md) for its
corrected renders, measurements and release verification.

| Delivery | File | Duration / dimensions |
| --- | --- | --- |
| Android creation | [MP4](reels/aster-atelier-creation-android.mp4) | 4:45 · 1440 × 3200 · 30 fps |
| Desktop creation | [MP4](reels/aster-atelier-creation-desktop.mp4) | 3:55 · 1600 × 1060 · 30 fps |
| Native viewport images | [Hero](renders/Aster-Atelier-hero.png), [seal](renders/Aster-Atelier-seal.png), [cheek](renders/Aster-Atelier-cheek.png) | Cursor-free crops; full window captures retained |
| Editable ring | [Portable source](design.ring.json) | Twelve casting layers; embedded alphas and masks |
| Manufacturing package | [Pattern STL](pattern-package/pattern.stl), [3MF](pattern-package/pattern.3mf), [moulding sheet](pattern-package/molding-sheet.html) | Compensated pattern, recipe and report |

Both creation MP4s and all three final PNGs were sent successfully by Taildrop to
Logan's S26 Ultra on 2026-09-12. The [transfer receipt](review/taildrop-receipt.json)
records file names, sizes, SHA-256 hashes and successful command results. The
earlier Android/desktop studio preview clips were sent before these creation reels.

The flower uses eight broad sculpted petals with 0.10 mm relief and a centred
3.20 mm solid-metal heart. Paired palmettes, polished cheek rails, masked sunseed
texture, swept vines, pearl edging and shallow shoulder/palm reeds complete the
composition. The seal's monotone curve rounds the crest while retaining its
withdrawal slope. There are no stones, pockets, prongs or bench-only ornament.

| Geometry and process | Reviewed value |
| --- | --- |
| Nominal finger opening | 18.20 mm · US 8.11 |
| Minimum sampled radial wall | 2.844 mm |
| Estimated ring mass | 28.43 g in 14k gold |
| Pattern compensation | 1.5% linear shrinkage · scale 1.0152284 |
| Manufacturing mesh | 2,457,600 triangles · 1920 × 640 grid |
| Final viewport mesh, both apps | 1,376,256 triangles |
| Mould arrangement | Opposed Z pull; centred parting; Delft clay recipe |

## Casting review

The **actual compensated pattern** has zero detected withdrawal obstructions
and zero unresolved rays at both requested sample pitches, 0.10 and 0.075 mm.
Obstruction tolerance remains 0.020 mm. The nominal source has no detail findings.
Independent mesh checks confirm one connected ring body, watertight surfaces,
consistent winding, the nominal bore, compensation volume and millimetre 3MF units.

The manufacturing status remains **Review**: 461.83 mm² has less than 3° draft,
including bore walls, and the coarse check reports ten narrow sand-slot findings.
Inspect their support, pattern finish and actual release in the shop. These
sampled checks do not prove sand strength, metal flow or a successful physical cast.

Evidence: [manufacturing report](report.json), [finer release check](release-fine.json)
and [independent checks](independent-checks.json). Reproduce the independent check:

```sh
python tools/check_aster.py --root showcase/aster-atelier --recipe crates/ringdesign-core/assets/aster-atelier.ring.json
```

## What the recordings demonstrate

- The shared Construction guide applies fourteen ordinary modelling operations:
  sizing, cushion head, drafted seal, frame, petals, heart, palmettes, paired rails,
  texture, vines, milgrain, shoulder reeds, palm reeds and manufacturing recipe.
  It builds editable parameters/layers; no finished mesh is substituted.
- The [desktop save](reels/desktop-created.ring.json) and
  [Android save](reels/android-created.ring.json) match every reviewed modelling
  parameter. Desktop JSON was extracted from its native autosave and given the
  portable format marker. Android's save is already portable JSON.
- Desktop demonstrates palmette visibility and numeric relief opacity
  1 → 0.5 → 1. ARTEMIS directly operates Android layer visibility, contextual tools
  and the mould study. Both return to the complete twelve-layer source.
- Android runs on the S26 display-profile emulator, Android 36, 1440 × 3120 at
  560 dpi. Its native VP9 capture avoids guest encoder downscaling. Desktop records
  the actual 1600 × 980 window. An 80-pixel caption strip is added above each;
  neither image is upscaled. Both finish with a 45-second detailed reveal.
- ARTEMIS Pro explored blank construction, inspector resizing and four camera
  views before capture scripting: `9d07a51f-9cb9-4cc1-979e-2a728ecc7fdb`, five
  checkpoints passed. Flash's recorded layer/mould session is
  `f0722be1-5a50-4d5a-85cd-61477313f652`. Notes, statuses and actions are in
  [review](review/); final mesh telemetry is [retained](review/android-showcase-mesh.json).

The guide applies prepared operations; these are guided construction sessions,
not recordings of drawing each original SVG freehand. Idle intervals are trimmed
according to [the edit list](reels/edit-list.json). Raw captures, action receipts
and operation screenshots remain beside each chapter. Desktop chapter 04 ends
before a failed OCR lookup; chapter 05 records the remaining operations. The
failed first layer-control take is excluded and its successful retake is used.

Construction checkpoints, chapter boundaries and final framing were visually
reviewed. Both complete MP4s passed frame-by-frame decoding. Details and hashes:
[final review](review/final-review.json). Still images come from the native
OpenGL viewport and retain its analytic studio lighting; no synthetic ring images
were inserted. The root-level software-rendered PNGs are earlier geometry previews.

The construction regression and native builds passed. These recordings retain
their [local review builds](review/build-artifacts.json). Android 0.16.0,
including the Atelier recipe, studio rendering, revised shoulder geometry and
new viewport tools, was published to the [app store](https://appstore.shadowbroker.app)
on 2026-09-16.
