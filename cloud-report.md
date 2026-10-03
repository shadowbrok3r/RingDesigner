# Tenebrae — Rosa: final report

**Outcome: cut at 5.7 after three reviewed rounds.** The rethink Logan approved passed its first read test (read test 4). The rounds then scored 5.4, 5.8 and 5.7 (revise, revise, cut). Every gate and the template gate were green in rounds 2 and 3; the ring lost on art alone.

- **Rounds used:** 3 of 3, after 4 read tests.
- **Branch:** `claude/tenebrae-rosa`, with master `2e11632` merged in.
- **Example:** `crates/ringdesign-core/examples/tenebrae_rosa.rs`.
- **Outputs:** `showcase/tenebrae/rosa/`.
- **Collection doc:** three lines in the Rosa section of `docs/collections/tenebrae.md` record the base and process, the rethink, and this outcome. No extra rounds were granted, so none is recorded.

## Read tests and reviews

| Step | Build | Verdict | What the reviewer said |
|---|---|---|---|
| Read test 1 | 8 oval sapphires in collets round a ruby; sunk petals | does not read | "a sapphire and ruby flower cluster" |
| Read test 2 | Raised tracery wheel, pear lights | does not read | "still a flower cluster; tracery too thin" |
| Read test 3 | Tracery 0.9 mm proud, cheek arcade | does not read | "a rose window on a second look; the almond collets read as petals" |
| Read test 4 (rethink) | Ruby oculus only; dark lancets; 16-cusp rim; two-lobed spandrels; corner trefoils | **reads** | "rose window… removing the sapphires was right"; weak off the face |
| Round 1 | + raised spokes, cheek gallery panel, shoulder discs | **revise, 5.4** | the ruby collet's land 0.70 mm failed; spokes are floating sticks, the panel a glued plate, the discs rivets, the table a stacked slab |
| Round 2 | Land 0.90; stock enveloped; shoulder discs removed | **revise, 5.8** | all gates green; spokes, slab, plate and missing density each about half a point |
| Round 3 | Rose cut into the face, spokes gone, graded oculi sunk on the shoulders | **cut, 5.7** | all gates green; the face now reads as a lotus or mandala first; plate arcade, serrated oculus rims, no small scale off the face |

The full JSON is in `showcase/tenebrae/rosa/read-test-{1..4}.json` and `review-round{1,2,3}.json`.

### Why it was cut

- **On the face:** without the spokes, the eight pointed lancets and the two-lobed spandrels above them read as interlaced petals, so the face says "lotus" before "rose window".
- **Off the face:** the cheek arcade sits on a proud plate. The end walls carry only that plate's edge, the trefoil bosses are flat cylinders, and the shank has no frieze.
- **Density:** there is no medium or small scale to stand beside Caiman.
- **Two fixes I could not land as asked:**
  - A three-lancet arcade cut straight into the stock cheek leaves knife edges under 0.8 mm (65 to 375 thin faces, depending on height). The plate is what made it castable.
  - Shoulder piercing through the stock's hollowed head failed the 0.8 mm section.

## Base and process

- **Base:** 001 Cushion at 19 × 19 mm, bore 18.6, palm raised its full 0.5 mm. The 16 mm 013 needs a baked source that travels inline (1.25 MB, over the 1 MB template budget).
- **Process:** lost wax, Gold 18k, 0.8 mm section and 0.15 mm detail. This matches Logan's 2026-10-03 rule.
- **Envelope:** the stock is built with its sand envelope on (`sand_envelope = true`, the undercuts filled toward the parting line). This squares the bare 001 shank's knife edges, which are under 0.8 mm (58 faces on the bare stock), and fills its hollowed head.
- **No sand bonus:** the field also reads Castable, but no two-part ray release was run, so I claim none.
- **Stones:** one Round 3.5 mm ruby in a drawn collet. The seat bur placed itself at the bore and left a 0.2 mm skin, so it was removed; the drawn bearing and a pilot drilled toward the finger's axis do its job.
- **Enablers used:**
  - C-T1: `Sketch::tracery` and `Profile::Regions`.
  - C-T3: `cutters::outline` for the lancets and trefoils, and `cutter.pierce` for the oculi.
  - C-T4: `dfm::cut_lands`.
  - #248: `write_png_framed` close-ups.
  - C-B2's pears were used in read tests 2 and 3 only; the rethink sets no sapphires.

## Gates (round 3, as committed)

| Gate | Draft 768×320 | Export 1536×448 |
|---|---|---|
| Triangles (≤ 2 M) | 500,254 | 1,345,930 |
| Watertight / degenerate faces | yes / 0 | yes / 0 |
| Self-crossings: ring / 16 made parts / pattern | 0 / 0 / 0 | 0 / 0 / 0 |
| Solids and parts notes; CAD features Ok | empty; 29 of 29 | same |
| Finger hole | 0 inside, min r 9.29997 against bore 9.3 | 0 inside, min r 9.29999 |
| Field (lost wax) | Castable, thinnest wall 1.88 | same |
| `thickness(0.8)` on a 384×160 build | 384 rays, 0 below, min 0.850 | same |
| Lands | bar 1.00, oculus collet to lights 0.90, spandrel to table edge 2.01 | same |
| `cut_lands(0.8)` / `findings_in` | 0 / 0 | 0 / 0 |
| Stones reported / previewed | 1 / 1 | 1 / 1 |
| `--verify` cold reload | — | identical |
| Casting pattern | watertight, 0 degenerate, 0 crossings | same |

The 0.8 mm thickness check only samples 384 faces, so I also cast a ray from every face of the 384×160 build (143,796 faces). None was under 0.79 mm.

**Template gate** (class `stock`): 44 nodes, 1 `design.set` patch (`/manufacturing`), 546,160 bytes against the 1 MB budget. Cold source identical, cold graph reload true, mesh parity identical at 1,345,930 triangles, `template_gate_passed: true`. `crisp_relief` is off, as the lift requires. The file is `showcase/tenebrae/rosa/verification.json`.

**Weight:** 35.0 g in Gold 18k.

## Feature tree, as sentences

1. Cushion signet, factory 001 at a 19 mm face, the stock enveloped.
2. A work plane over the table.
3. The rose net, traced into lights a bar (1.0 mm) apart:
   - a hub circle at r 3.2;
   - eight mullions from hub to rim;
   - eight pointed heads, each struck at a full span, apex at r 6.3;
   - a 16-lobe cusped rim at r 7.7, with 0.55 mm sag.
4. Sink the eight cusped spandrels deep (2.2 mm).
5. Sink the eight lancet lights deep (2.2 mm).
6. Corner boss height plane.
7. Four corner trefoils.
8. Raise the corner trefoils as carved bosses (0.6 mm).
9. The ruby, Round 3.5, girdle 0.4 mm over the table.
10. The oculus collet, drawn:
    - a lip-height plane, a lip ring, and "raise the lip to the bearing";
    - a bearing-height plane, a bearing ring, and "stand the bearing on the metal".
11. Drill the oculus pilot to the finger.
12. Sink oculi of the nave 1–4: round, 1.6, 1.4, 1.2 and 0.9 mm, 0.6 mm deep, chamfered 0.15, at θ 90 − (46, 55, 64, 73). Then mirror them through the crown.
13. The near cheek's panel face plane.
14. A gallery panel on a sill, stood on the cheek and mirrored to the far cheek.
15. Outside the near cheek, leaned 6.2° with the wall, the gallery arcade (three lancets, 1.1 × 2.0 mm, 2.0 mm apart): cut into the cheek and mirrored.

The raised-tracery, spoke and moulded-ring variants are still in the example behind `RAISED_TRACERY` (false).

## What I could not do

- **A face that reads as a rose window before a flower,** once both the stones and the spokes were gone.
- **Arcades cut straight into the stock walls without knife edges,** or on the end walls.
- **Pierced shoulder oculi:** they would cut into the head's hollow, and the shoulders are too thin.
- **Rounded or chamfered bar tops, and a beaded collet.** Edge fillets need hand-picked edge references, and a drafted tracery extrude was refused as degenerate.
- **Seam beads:** none on the joined parts. The clustered join's fillet folds in acute light corners, which breaks house rule 10.

## Core changes wanted

1. **The seat bur should read only the band's surface,** with no joined parts and no stock interior. Here it sat at the bore and left a 0.2 mm skin; elsewhere it shaved collets.
2. **A `head.bezel` cast-straight lip option,** so a cast collet meets a 0.8 mm section without the leaning lip.
3. **Per-part seam fillets** that skip acute corners, instead of failing the whole cluster.
4. **`cad::measure::thickness` should return every thin sample,** not just the minimum. The example carries a full-face map behind `ROSA_MAP` for this.
5. **`base.preset { pre_scale }`,** so 013 can reach 16 mm within the template budget.
6. **A wall-normal cut** (a `Placement::Side`-style plane for head walls), so arcades can be cut square to a leaning, curved stock wall without slivers.
