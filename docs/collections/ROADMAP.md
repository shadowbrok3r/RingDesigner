# All collections: execution inventory

As of 2026-09-25. Baseline: pushed master `56ccad4` (P7 complete); remaining Batch 16 platform and starter integration in progress; IN-FLIGHT.md is the current checkpoint. This is a planning inventory, not a new validation claim.

**71 active deliverables: 32 master rings + 33 starters + 6 Officina lessons.** Draco remains a cut archive, and Wishbone remains a tracked optional return after C-S6. All later collection scope is preserved.

The Bestiarium package still stops for Logan’s review. The latest request authorizes useful parallel agents; it does not explicitly waive that stop.

## Immediate assignments

1. Finish P5–P8 reviews and validation, integrate compatibility fences, then run every crate and platform check. Arachne is art-approved at 7.5/10; Draco is archived at 5.8.
2. Finish starter part 1 (fixtures → stocks → cameras → settings → split starters → thumbnails/golden), in parallel with disjoint platform work.
3. Continue Phoenix round two and Basiliscus crease-safe relief; finish Harpyia’s active one-wing spike. C-S2/C-S3 and Officina’s first Rivet proof may use a disjoint enabler lane.
4. Continue Corvus/Manticora, then Fenrir/Kraken after C-B1, then Harpyia. Package eight, send the final render ZIP by Taildrop, and stop for Logan.

## Entire master-ring queue

| Collection | Ring | Process | State / main dependency |
|---|---|---|---|
| Bestiarium | Draco | delft | cut_archive; landed APIs |
| Bestiarium | Arachne | lost_wax | art_accepted_pending_integration; landed APIs |
| Bestiarium | Phoenix | delft | round 1 revise 6.0; anatomical revision and sand-slot sections |
| Bestiarium | Basiliscus | lost_wax | authoring; resolve paint folds at native creases |
| Bestiarium | Corvus | delft | queued; P3, P4, C-B3 |
| Bestiarium | Manticora | lost_wax | queued; P3, P4, C-B3 |
| Bestiarium | Fenrir | lost_wax | queued; P6, P3, P4, C-B3 |
| Bestiarium | Kraken | lost_wax | queued; P6, C-B1 |
| Bestiarium | Harpyia | lost_wax | active one-wing spike; loft tessellation failed, sweep fallback under test |
| Cataphracta | Sphenodon | delft | queued; C-R2, C-R7 |
| Cataphracta | Heloderma | delft | queued; C-R1, C-R2, C-R3, P5, C-R7 |
| Cataphracta | Moloch | petrobond | queued; P4, C-R2, C-R7 |
| Cataphracta | Gekko | delft | queued; C-R1, C-R2, C-R7 |
| Cataphracta | Ouroborus | petrobond | queued; P5, C-R2, C-R1, C-R7 |
| Cataphracta | Chelonia | lost_wax | queued; C-R4, C-R2, C-R7, C-R8 |
| Cataphracta | Phrynosoma | lost_wax | queued; C-R4, C-R2, C-R7, P4 |
| Cataphracta | Chamaeleo | delft | queued; C-R1, C-R4, C-R7, P4 |
| Tenebrae | Oculus | delft | queued; C-T2 |
| Tenebrae | Ogiva | delft | queued; C-T2 |
| Tenebrae | Rosa | lost_wax | queued; C-B2, C-T1, C-T3, C-T4 |
| Tenebrae | Sigillum | delft | queued; C-T6, C-T5, C-T3 |
| Tenebrae | Porta | lost_wax | queued; C-V2, C-T3, C-T7 |
| Tenebrae | Arcus | lost_wax | queued; C-T3 |
| Tenebrae | Capsa | lost_wax | queued; C-T3 |
| Tenebrae | Lanterna | lost_wax | queued; P2, C-T1, C-T3, C-T4 |
| Vepres | Rubus | delft | queued; P3, P4 |
| Vepres | Ilex | delft | queued; P3, P4, P2 |
| Vepres | Prunus | delft | queued; C-V1 |
| Vepres | Viscum | lost_wax | queued; P2, P3, P4 |
| Vepres | Datura | lost_wax | queued; C-V1 |
| Vepres | Rosa mortua | lost_wax | queued; P6, C-B2, C-V1 |
| Vepres | Hedera | lost_wax | queued; C-V2, C-V5 |
| Vepres | Sentis | lost_wax | queued; C-V4, P6 |

## Starters and Officina

| Group | Entries |
|---|---|
| Bands (4) | Court, Braided, Split shank, Split gallery |
| Parametric signet (1) | Shouldered cushion |
| Settings (8) | Cathedral solitaire, Bezel solitaire, Halo, Trilogy, Toi et moi, Split-shank basket, Half eternity, Gypsy trio |
| Factory stocks (20) | 001 Cushion; 002 Kite; 003 Clover; 004 Shield; 005 Rosette; 006 Square; 007 Quatrefoil; 008 Heart; 009 Drop; 010 Trillion; 011 Badge; 012 Cushion; 013 Round; 014 Heater; 015 Octagon; 016 Star; 017 Tonneau; 018 Butterfly; 019 Jewel; 020 Escutcheon |
| Officina (6), build order | Rivet → Keystone → Torsade → Sigil → Fenestra → Aile |
| Tracked optional return | Wishbone wave after C-S6; old version remains a fixture |

The complete shared menu reaches **89** entries: 33 starters + 6 Officina + 18 retained existing designs + 32 new masters. Wishbone makes 90. Do not hard-code an intermediate 57 before Officina is actually registered.

## Dependencies and ownership that affect parallel work

- P6 owns `setting.rs` claws and `cad/builders.rs`; starter cutters wait for it. P7 owns imported-base serialization, library fences, graph CAD/key/stamp nodes and lift.
- P5 touches field context and station tables plus exhaustive gate consumers. Reserve `graph/nodes/layer.rs` before C-B1 or any Cataphracta layer work.
- P8 owns only `render_collection.py`, `catalog_collection.py`, and `collection_templates.rs`. The starter lane owns the workbench thumbnail macro.
- C-B1 precedes Kraken. Phoenix’s contour stays local until Corvus becomes its second consumer.
- After Logan’s Bestiarium review: C-B2 true stone plans, then C-R enablers because both touch `field.rs`; re-render Manticora/Trilogy/Toi et moi after their pear upgrades.
- Cataphracta order: Sphenodon → Heloderma → Moloch/Gekko → Ouroborus → Chelonia/Phrynosoma → Chamaeleo. Pair core field integration with disjoint SVG generation/plan-mask work, not another `field.rs` owner.
- Tenebrae enablers: C-T1/C-T6 → C-T3/C-V2 → C-T4/C-T5/C-T7; C-T2 clusters before packaging. C-V2 must serve both Porta’s sketch path and Vepres surface/part paths.
- Vepres C-V1 and C-V3 are **not wholly disjoint**: both eventually edit `cad.rs` and `library.rs`. Split ownership explicitly or serialize those edits; afterward C-V4 parts and C-V5 graph paths can pair.
- Lead owns registration, shared scaffolds, manifests, collection READMEs and sheets. Every ring author owns only its example/art/showcase.

## Correct stale brief text before using it

- Faces are allowed. Capsa may use its skull and Arcus may have a gargoyle face if they meet the review bar.
- Chelonia/Phrynosoma use real native 007/016 stock in wax. Cataphracta is **4 Delft + 2 Petrobond + 2 wax**, not all sand. Their sand clamp and base-decision text is obsolete.
- Viscum uses native 003 in wax. Vepres is **3 Delft + 5 wax**. Do not re-ask this decision.
- Bestiarium after Draco is cut is **2 Delft + 6 wax**, with approximately 35 stones; recompute from final stone reports.
- Stock starter symmetry is only a classification. 003/007 have known envelope refusals; 005/016 have rewrite concerns. The brief explicitly allows a failing stock starter to open in wax. Keep its actual-process badge truthful.
- Chamaeleo’s face is 17 × 14.5; 13 mm is outside the stock guard.
- Never publish, tag, or bump versions, regardless of old packaging prose. Master push requires all suites, NDK, wasm, and locked checks green.

## Choices still reserved for Logan

- Bestiarium package review and any resulting count change.
- Capsa: actual bench hinge/Separate lid versus cast shut.
- Tenebrae weight alloys: default Silver 925 for sand and Gold 18k for wax.
- Oculus: roughly 18 g / 24 lights / 4.6 mm, or lighter 16 lights / 4.0 mm variant.

The collection files contain each entry’s base, process, dependencies, source locations and required gates. Keep this scope inventory alongside IN-FLIGHT.md when work changes hands.
