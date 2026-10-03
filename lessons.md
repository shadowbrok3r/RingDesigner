# What the Bestiarium taught

The Bestiarium shipped 2 of 9 rings. Every cut ring passed its gates, and almost every one failed the same test: at 300 px, without its caption, it did not read as its subject. The final reviews named the causes.

- **Even tubes and smooth primitives read as something else.** Kraken's claws read as "two hands cupping an orb", its coils as bail loops. Harpyia's talons read as ball-ended tubes, and she read as "a figure cradling an orb in two human hands". Corvus's face view read as "a visor or a beetle".
- **A head that is blocky at three-quarters loses the animal.** Basiliscus read face-on as a crowned serpent and lost it in the hero.
- **The subject was small where the camera looks.** Phoenix's bird was an emblem on the side faces; the hero showed "an ornate textured band with an orange cabochon".
- **A motif repeated at a fixed pitch reads as a border.** Phoenix's flames read as a running scroll.
- **Painted relief that stair-steps or smears breaks the outline rule.** Fenrir's ruff rendered as terraced contour stacks; Corvus's skull feathers as smeared highlights.
- **Bare polished walls read as unfinished** beside a dense table (Basiliscus).

What shipped: **Arachne**, where the subject read at once from two unequal stones and eight continuous raised legs, and **Manticora**, where one iconic silhouette (the hooked sting) curls over the stone.

So, before any detail: decide what makes this subject unmistakable at 300 px (its signature silhouette or pattern), and put it where the hero and face cameras look, at a size that reads. Cataphracta's subjects are armoured hides: the pattern is the subject. Gila beadwork, a tuatara's crest and sail, a thorny devil's cones, a tortoise's growth rings must each be legible at 300 px and specific to that animal, never generic scales. Caiman (scores 7) and the Reptilia sheet in `showcase/reptilia/renders/` are the bar; do not repeat the Reptilia tiles.

## What the Cataphracta pilot added (2026-09-27)

**Texture alone never names the animal.** Heloderma's whole-band beadwork failed six read tests across sand and lost wax (sea-urchin shell, granulation, caviar); Sphenodon's reviewer saw "a reptile at once" the one time a lizard head was on the ring. Build the animal itself, head and body where the hero and face cameras look, with the hide as its skin. In two-part sand that is rarely possible (figurative relief only on the crest line and side faces, which both cameras see edge-on), so a figurative ring is lost wax by default.

## For Tenebrae (architecture, not creatures)

The same test applies to a building: at 300 px without a caption, a jeweller must say "a rose window", "a pointed arch", "a flying buttress", "a Gothic cathedral". A ring of evenly spaced holes reads as a perforated band, a moulding as a groove, a pier as a box. What names Gothic is its signature silhouette at full scale where the hero and face cameras look: the pointed arch, cusped tracery (trefoils, quatrefoils), lancets in rows, pinnacles, crockets, and coloured glass with light through it. Detail comes after the silhouette reads. No flat, blocky CAD: every joined part carries a seam bead, piers and plinths carry chamfers or mouldings.

## What Cataphracta and Tenebrae's first rings added (2026-09-28)

Five more rings failed on the read, none on a gate. Moloch reached 7.1 after four rounds, Sphenodon 7.1, Heloderma 6.3; Oculus and Ogiva never passed their block-out.

- **The subject must be in the outline the face and hero cameras see.** Both cameras see a band's side faces nearly edge-on. A wheel window on a side face read as "a thin frame round a void" (Oculus), and a pointed-arch section revolved round the finger showed only end-on, so from the front it was "a rounded band" (Ogiva). Side-face work can support a read, but it cannot carry one.
- **The signature part must be the species' own shape, not a generic one.** Heloderma's round, cracked bulb of a head read as a clump; a Gila's is a flat, square-snouted wedge. Sphenodon's long snout read as a monitor's; a tuatara's is short and blunt. Get right the one shape a naturalist would name before any surface work.
- **Bald polish round a motif reads as a decal.** A polished halo, or bare blocks between a figure and its ground (Moloch round 3, Heloderma), read as a sticker. Let the motif grow out of its ground.
- **A fix must not break a gate.** Moloch's new toes fell from 0.72 mm to 0.18 mm of section and failed the floor. Size thin parts against the floor first, and re-run every gate after each change.
- **Fat, rounded digits read as a gecko, a frog or a glove.** Thin, tapering, clawed ones read as a lizard's.

## What Vepres's first two rings added (2026-10-02)

Rubus (bramble cane, Delft) was cut at 6.7 (6.3, 6.6, 6.7) and Ilex (holly on 006, Delft) at 6.3 (5.6, 6.2, 6.3). Both read as their plant at 300 px; both lost their points on workmanship that three rounds did not fix.

- **Every reviewer fails stair-steps, combing and smeared relief.** The rubric's crisp-outline item failed on Moloch, Heloderma, Rubus and Ilex. Steep walls in the height field (decals, bench veins, masks, painted hides) step with the build grid, about 0.04 x 0.05 mm at export, which is 3 to 4 px in a 1600 px render. Tiled hides showed hairline seams at tile joins. Build every figurative outline as true geometry: a stamp, or a CAD extrude, loft or sculpted stored mesh. Never use a decal or a charted mask for it. Zoom your own renders to 2x before every review. The lead is fixing the platform side tonight; see "Master moves tonight" in TASK.md.
- **A leaf is a sculpted solid, not a cut plate.** Flat sepals, calyx tabs and gabled leaf stamps read as cut sheet. Give leaves a domed two-slope top (higher on the midrib, falling to the margin), a 0.1 mm top-edge round, and a smooth serrated margin.
- **Seat a part on its ground with a fillet, never in a cup.** Round recesses round prickles, and almond bosses under them, read as machining. Use `blend_mm` 0.3 to 0.4.
- **Bark and grain must be broken and wavering.** Ruled parallel striae read as combing. Vary the length, waver the line, and add node rings or scars. The same goes for any hide: no straight grid lines longer than a few cells.
- **One even field, never panels.** Two rectangular matte panels read as panels. Use one uniform stipple over the whole field, inset from the edge, with a narrow polished halo masked round each motif.
- **A row of identical leaves is a fringe.** Alternate each leaf's rotation (about ±25°), overlap tip over stem, dome the tops, and show the stem they grow from.
- **Watch for accidental faces.** Ilex's first block-out had two red berries over a spiny leaf, which read as eyes and a toothy grin. Check the face view for eyes before you submit a read test.

**Update, 2026-10-03:** most of the stair-steps were a rendering artifact, fixed on master (#248). Render close-ups with `render::write_png_framed`, never a cropped mesh. Set `d.crisp_relief = true` for steep height-field relief, and use `StampTop::Pillow` for leaves and petals.
