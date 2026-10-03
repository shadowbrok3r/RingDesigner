# Heloderma — the beaded one: revival report (lost wax, rounds 4 and 5)

**Outcome: cut at round 5 with a score of 6.9.** The ring scored 5.8, 6.4 and 6.3 in rounds 1 to 3 (cut 2026-10-02), then 6.8 in round 4 and 6.9 in round 5 under Logan's extension of 2026-10-03. The ship bar is 7.5. Every gate was green before both new reviews, at draft and at export, and the template gate passed on the final design. The reviewer's verdict stands.

- **Rounds used:** 5 of 5 (3, plus the 2 extra rounds Logan granted on 2026-10-03, recorded in the ring's section of `docs/collections/cataphracta.md` before the first new review).
- **Branch:** `claude/cataphracta-heloderma-revival`, from `claude/cataphracta-heloderma`, with master `2e11632` merged. At the start of round 5 `origin/master` was still `2e11632`, so that merge had nothing to bring in. The only conflict was in this file, and it was resolved in the ring's favour. Nothing under `src/` was touched.
- **Files:**
  - Author file: `crates/ringdesign-core/examples/cataphracta_heloderma.rs`.
  - Sector SVGs: `crates/ringdesign-core/examples/cataphracta/art/heloderma/`.
  - Outputs: `showcase/cataphracta/heloderma/`. That holds the renders, `report.json`, `template-verification.json`, `review-round{1..5}.json` and `read-test-{1..3}.json`. Earlier lanes are archived in `sand/` and `lost-wax-pattern/`.
- **Process decision (Logan, 2026-10-03), recorded in the doc:** judged as lost wax, with a 0.8 mm minimum section and no pull rule.
  - **Sand bonus:** none. The ring would not pull from sand. The two-part undercut is 4.14% of the surface (worst −29.8°), with a "Will not release" verdict if poured in sand. The stored Gila locks over about 79 mm².

## Verdicts

| Stage | Verdict | Score | The reviewer's core finding |
|---|---|---|---|
| Read tests 1–3 (27 Sep) | reads: false | — | "A lizard … not Gila"; "some small reptile" |
| Round 1 | revise | 5.8 | Gates unrecorded; bald face, bulb head, stepped edges, tile seams |
| Round 2 | revise | 6.4 | Gates green; still a lizard, not a Gila; blocky ground with a bald halo; shank seams |
| Round 3 | cut | 6.3 | Bulb or croc head, cracked-mud hide, stepped blocks on the face, sector walls, tail off the crest |
| **Round 4 (revival)** | **revise** | **6.8** | One continuous ground, no sector walls, round collet-lip domes, a flat beaded head; reads at once as a lizard in side view. It fell short because it reads as "a lizard", not "a Gila": the head was as big as the trunk, the bands read as ridges, the sculpt smeared at the legs and tail, a polished halo stayed round the stone, the palm beads were stair-stepped, and two feet ran onto the rims |
| **Round 5 (revival)** | **cut** | **6.9** | Head now a blunt lizard's head, feet inside the rims, tail on the crown, halo gone. It fell short because at 300 px it is still "a lizard, or a ribbed grub with a fish-like head": the salmon bands stand as hoops and the eye sits forward. The tail rings and legs still tear, needle slivers stand at the band forks, and the feet read as webbed paddles. The collet top is plain, and the palm beads merge in pairs |

The full JSON is in `showcase/cataphracta/heloderma/review-round4.json` and `review-round5.json`.

## What the final ring is

| Layer or part | What it is | Why |
|---|---|---|
| Base | Procedural `HalfRound` 8.0 × 3.2, edge round 0.3, comfort fit 0.2, bore 18.6 (size 8.6). Keys (width / thickness): 1.42 / 1.00 at 90°, 1.20 / 1.00 at 35°, 1.32 / 1.00 at 145°, 1.26 / 0.98 at 200°, 0.92 / 0.90 at 270°, 1.05 / 0.95 at 330° | An 11.4 mm crown to carry the animal |
| `crisp_relief` | **Off** | The template gate needs the graph lift, which cannot carry it yet. With it on, the gate failed at the design node; with it off, it passes. The ground is round domes with no steep height-field wall |
| Spessartite (top-level `SeatPad`, `Max`) | 3.0 mm round at θ 37.5°, `GypsyMound` 0.45 mm, `Bezel` collet sunk 0.35 mm, through, 0.5 mm drill dot | The stone the Gila noses, in a made setting just ahead of the snout |
| Gila beadwork (`Group`, `Add`) | Eight sector `DecalLayer`s, each overlapping its neighbours by 0.6 mm and composited by plain `Max` inside the group. The group adds over the band and the mound | One continuous bead ground with no walls. A bead in an overlap is the same bead in both sectors, and adding over the pad runs the beads up the mound with no bald skirt |
| — its beads | Fine 0.41 mm beads (0.10 mm domes) everywhere at the face. Round the shank the tail's rings carry on by bead size: 0.75 mm salmon beads (0.22 mm domes) in the salmon bands, fine beads in the black ones. The domes are soft-skirted and grade to nothing over the last 0.45 mm before the rims. Salmon beads stand whole or not at all, so the DFM ink measure sees only whole crowns | The hide face to palm; no plateaus, no ledges |
| Gila (stored part, `Attach::Join`) | One sculpted distance field, 190,122 triangles. Its parts are listed below the table | The figure that names the animal |
| Collet lip | 22 round domes 0.4 mm across, dropped onto the mound's own surface at 2.24 mm from the stone's axis, part of the stored mesh | Replaces round 3's radial teeth |

The Gila part holds:
- **The head:** a flat wedge 4.6 × 4.2 × 2.35 mm with a square snout and rounded jowls over a tucked jaw. Its eyes are polished and sunk under lids and brows, and it carries a mouth line and 0.78 mm domed beads in rows across it.
- **The trunk:** keyed half-widths up to 2.8 mm. It drifts across the crown so the tail can sweep back.
- **The tail:** an arc of 2.9 mm radius through 115°, at least 70% of the trunk's width at mid-length, with a blunt end. It stays on the crown.
- **The hide:** explicit hex bead rows in each part's own (along, around) coordinates, never noise. Salmon bead crowns lie on the envelope, with their seams cut in. The black bands sink 0.48 mm on 0.38 mm beads: three forked crossbands on the trunk and four rings round the tail.
- **The legs:** four, in a walking pose. Each leaves the flank inside a black band. The feet are smooth, with five tapering toes and 0.52 mm claw ends, and every toe stays at least 0.8 mm inside the rims.

## Gates, final design (round 5), from `report.json`

| Gate | Draft (768 × 320) | Export (1536 × 448) |
|---|---|---|
| Watertight, 0 degenerate faces | pass (647,538 triangles) | pass (1,505,566) |
| 0 self-crossings: ring, figure, pattern | pass | pass |
| Figure joined; solids and parts notes empty; stamps 0 | pass | pass |
| Nothing in the finger hole (closest 9.29997 / 9.29999 against 9.30) | pass, 0 inside | pass, 0 inside |
| Lost-wax field verdict Castable, thinnest wall | pass, 2.10 mm | pass, 2.10 mm |
| Land widths per feature | pass | pass |
| Ray release at 0.100 and 0.075 mm (NotApplicable under lost wax; reported) | 1,034 and 1,505 obstructions, 0 and 0 unresolved | 1,020 and 1,591, 0 and 0 unresolved |
| Draft clamp | none, bite 0 | none, bite 0 |
| 0 DFM findings | pass | pass |
| Stones reported = previewed | 1 = 1 | 1 = 1 |
| Casting pattern closed | pass | pass |
| Within 2 million triangles | — | pass |
| `--verify` cold reload, empty library | — | identical vertices, faces and normals |

**Land widths, per feature:**
- limbs 1.36 mm;
- toes 0.82 mm;
- neck 1.85 mm;
- tail's end 1.12 mm;
- claw ends 0.52 mm (rounded ends, named against 0.5 mm);
- collet-lip domes 0.40 mm (relief beads, judged at the 0.15 mm detail floor);
- finest ground bead 0.27 mm.

Two things are named under the floor rather than removed: the figure's whole-part ray read (50 mm² of chords through bead seams), and the standard collet (0.18 mm at its lip, burnished at the bench).

There are no stamps, so the 384 × 192 rerun does not apply.

## Template gate (run on the final design, after round 5)

```
collection_templates cataphracta target/tpl-src --output-dir target/tpl --only heloderma --verify-export   (template_class painted)
```
- `design.set` patches: **1** (`/manufacturing`), against a limit of 4.
- Graph: **2,487,420 bytes**, within the painted budget of **3,000,000**. 71 nodes, 0 detail findings. The editable design is 3,033,504 bytes; no size review is required.
- Cold source parity: **passed** (`source_identical`, plus the cold design and graph reloads).
- Mesh parity: **passed**. Vertices, faces and normals are identical at export (1,505,566 triangles).
- With `crisp_relief` on, the lift failed ("#72: the design failed upstream"). That is the known lift gap, so the ring ships the flag off.

## What each new round changed

- **Round 4,** from round 3's punch list, plus the crisp pass:
  - The figure was rebuilt from scratch: a flat square-snouted head, explicit bead rows in place of Voronoi, countable sunk bands, and a tail swept back onto the crown.
  - The ground became bead-only decals met by `Max`. This removed the plateaus that read as sector walls, the frilled rims, and the footprint mask that cut the bald blocks.
  - The collet lip became round domes on the mound.
  - Close-ups are now framed with `write_png_framed`: `stones`, plus the new `close-head`, `close-body`, `close-tail` and `close-palm`.
  - Gate fixes along the way:
    - The DFM ink sliver: the full-ink height was halved, so salmon crowns read whole.
    - The collet-lip feature is judged at the detail floor.
    - The claw ends went from 0.50 to 0.52 mm.
- **Round 5,** from round 4's punch list:
  - The head shrank to 4.6 × 4.2 mm, with tucked jowls, sunk eyes and the nostril pits removed.
  - Salmon crowns now lie on the envelope, and the black bands sink 0.48 mm on 0.38 mm beads, forked on one flank.
  - The feet were pulled 0.8 mm inside the rims, with smooth toes along the ring.
  - The ground now rides over the mound, and its domes are soft-skirted.

## What I could not do

- **Make it read as a Gila monster, not a generic lizard, at 300 px.** Five rounds carried it from "cracked bulb on a lizard" to "a lizard with a blunt head" (side view). In gold, the pale and dark bands are relief; they read as hoops or ridges, never as colour, and at 300 px the trunk reads as a ribbed grub. The head reads as fish-like or toad-like from above.
- **Clean the sculpt's surface.** Decimating about 1.5 M raw triangles to about 190 k (the 3 MB template budget) tears facets on the tail rings and legs, and leaves slivers at the band forks. A finer mesh would break the template budget.
- **Make the feet read as clawed digits.** The toes are 0.82 mm capsules, the section floor's minimum, and they fuse into paddles at the pad.
- **Bead the collet's top.** `SolidKind::Bezel` is a fixed core part. Its plain top ring cannot be bevelled or beaded from the example.

## Recommendation, if Heloderma is restarted

1. **Put the colour back.** Blackened silver in the sunk bands is the literal Gila; relief alone has failed to name it across five rounds and four lanes. That needs the core change below.
2. **Model the head from a reference plan:** eyes 40% back, a thick neck, a broad blunt snout. Get its 300 px read first, before any hide work.
3. **Carry the figure's beads as a separate fine stored mesh,** or as stamps, rather than as displacement on a decimated sculpt, so the bead seams never tear.

## Core changes wanted

- **The graph lift should carry `crisp_relief`.** One line in the lift's design patches: emit `{"kind": "design.set", "inputs": {"pointer": "/crisp_relief", "value": true}}` when `design.crisp_relief` is true. The fence in `library.rs` already recognises that patch.
- **A second metal finish in `render::finished`,** so an oxidised-band version can be reviewed. For example, a `Part::metal_masked(&mesh, GOLD, OXIDE, mask)` that reads a per-vertex flag.
- **A beaded or bevelled lip option on `SolidKind::Bezel`,** for example `bezel_milgrain: Option<(usize, f64)>`, so a collet can carry a beaded top edge.
- **A relief-aware `dfm::part_sections`,** carried over from the earlier report. This lane measured per feature on the sculpt's own field instead.
