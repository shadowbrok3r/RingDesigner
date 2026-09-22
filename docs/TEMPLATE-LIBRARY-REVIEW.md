# Visual alpha sources and template library — 2026-09-19

Desktop 0.4.0 and Android 0.27.1 replace graph PNG text editing with a visual alpha picker. Search covers built-ins, imported images and the active design's artwork. PNG inputs keep their existing logical name and embed the selected image, preserving portability and layer references. Alpha-reference inputs use the same picker. Cancel leaves inputs untouched. Previews are cached; scrolling creates thumbnails only for visible rows. The desktop inspector no longer clones the entire graph every frame.

The shared New from template library includes 31 designs in eight collections. Both collection rows and template rows show actual render thumbnails, with a larger hover preview. Menus read metadata and small thumbnails; only choosing a template parses its design. Starter designs retain their direct property editing.

| Collection | Templates |
| --- | ---: |
| Reptilia | 5 |
| Stock masterworks | 7 |
| Workshop | 4 |
| Atelier designs | 4 |
| Original masterwork signets | 2 |
| Starter bands | 4 |
| Starter signets | 3 |
| Stone settings | 2 |

The desktop top bar centers the active design/template name, or the filename after Open/Save As. Long names truncate within the space between controls and keep their full tooltip. File identity persists across restarts and clears for a new design or template. Button and selectable fills are darker on both platforms; text, colored icons and selection/hover outlines retain their contrast.

## Template audit

The lift previously created a signet head for every ring. Ecdysis, Tessera and Lorica are uniform bands, so that head had no geometry effect. Each now omits the dormant head and default shank nodes. Ophidian and Varanus retain their heads: face length and head rise deform the imported stock. Those nodes are now named **Stock face dimensions**.

Across the 26 authored graph templates, cleanup removed 105 nodes and 75 artwork sources. The removed artwork either had no layer/mask reference or was overwritten by a later source of the same name during baking. Original source designs remain intact. Curated graph files and editable graph copies use the refined sources; their generators apply the same cleanup. Every remaining node reaches the output, and no whole-layer/profile/shank patch overrides the authored branches.

Verification found identical geometry settings and referenced alpha pixels across every graph template. All five Reptilia designs additionally produced identical full-resolution vertices, faces and normals before and after cleanup. Evidence: `target/template-library-review/template-audit.json`.

To reproduce the audit and regenerate curated graphs:

```sh
cargo run -p ringdesign-graph --example refine_templates -- --write --meshes
```

Thumbnail provenance is recorded in `crates/ringdesign-workbench/assets/templates/sources.json`; `tools/template_thumbnails.py` regenerates the small assets from the original renders. A contact sheet is in `target/template-library-review/thumbnails.png`.

## Verification

300 Rust regression tests passed: graph 74, graph UI 24, desktop 39, workbench 31 and Android 132. The added picker checks cover cached PNG previews, cancel preservation, image replacement, same-name library refresh and card bounds at 360, 600 and 900 points. A reproduced reopening failure now passes: filtering to no results, cancelling and reopening restores visible captions and the full gallery height; keyboard safe bounds also resize and restore correctly. Logs are in `target/template-library-review/`.

Native inspection verified all eight collection menus, Ecdysis loading with 18 nodes, source replacement from both the Node inspector and inline graph fields, cancellation, search and live geometry updates. The title remains centered and truncates without overlapping controls at 1100 and 1600 points. Visual review found and corrected inherited horizontal layout in the picker cards; captions now sit below their thumbnails. Screenshots include `native-alpha-picker-final.png`, `native-collections-final.png`, `native-title-1100.png` and `native-inline-applied.png`.

Desktop 0.4.0 and both Android 0.27.1 ABI release builds succeeded. The ARM64 APK verifies with APK signature schemes v2/v3 and carries version code 16784129. Release artifacts are in `target/releases/ringdesigner-desktop-0.4.0/` and `target/releases/ringdesigner-android-0.27.1/`.

ARTEMIS verified Android 0.27.0 on the Android 16 emulator with 8/8 checks passing (trace `8e00d5f5-ffd9-4903-b9e2-a4744a47f239`). It checked both collection levels, Ecdysis's 18-node graph, the image-source control, no-match search, cancel preservation, portrait/landscape fit, alpha replacement and live geometry updates. Source names remain intact after choosing another image. The report and machine-readable result are `android-review.md` and `android-test-summary.json` in the evidence directory. Screenshots were also captured directly over ADB; ARTEMIS's optional video recorder reported encoding/audio errors, so the review does not rely on a complete video.

Final manual screenshot review caught a missed issue after that first automated pass: a filtered search could leave the remembered modal height too short on reopening, clipping the first row's captions. Android 0.27.1 restores the scroll viewport height explicitly and uses the host's safe content bounds. The targeted ARTEMIS follow-up passed 11/11 checks (trace `b0b3bd59-50dd-4b90-9ffe-07a6943dc2dc`), including zero-result and single-result reopening, keyboard resizing, rotation and image replacement. Independent final screenshot review confirmed three complete thumbnail/caption rows in portrait. Evidence: `android-patch-review.md`, `android-patch-test-summary.json` and `android-alpha-picker-final.png`.

Android 0.27.1 is published to the app store. The authenticated downloaded APK matches the local ARM64 release byte for byte and matches the store's SHA-256 metadata; version and changelog were verified. SHA-256: `c0b382e302b8f3dc055ea5d1940c45619dc28589032b6a519326d92a7b2ba5ad`. Evidence: `store-verification.json`. Desktop 0.4.0 is built locally; no GitHub desktop release was published.
