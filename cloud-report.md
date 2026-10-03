# Officina ring: Rivet, final report

**Outcome: stopped at the block-out.** Three read tests, none read. Under TASK.md's rule ("If the third still
does not read, stop and report: the subject needs rethinking, not detailing"), no full-review round ran.
**Rounds used: 0 of 3** (3 of 3 block-out attempts). No review score was given. Every engineering gate passes at
draft and at export. The template gate fails on its patch count (see below).

- Branch: `claude/task-md-implementation-fx7pkw`. This is the session's designated push branch; the harness
  allows no other, so nothing went to `claude/officina-rivet`. It starts from `origin/master` at `8e5a59a`.
- Example: `crates/ringdesign-core/examples/officina_rivet.rs`. Outputs: `showcase/officina/rivet/`.
- Process: Delft clay, two-part sand (`SandProcess::DelftClay.apply`, then `CastProcess::SandTwoPart.apply`).
  Bore 18.2 mm, 14k gold.
- Master merges and enablers: none. TASK.md calls for a merge at the start of each round after the block-out,
  and no round was reached. The block-out needed no enabler (C-B2, C-V1 to C-V5, C-T5 to C-T7 were all unused).

## Read tests (reviewer verdicts, verbatim summaries)

| Attempt | Reads | What the eye saw | File |
|---|---|---|---|
| 1 | **false** | "A plain, smooth low-domed gold band with one row of round gold beads… a beaded or dotted band." | `read-test-1.json` |
| 2 | **false** | "A raised centre rail between two flat side rails… a spinner ring. The heads are still beads." | `read-test-2.json` |
| 3 | **false** | "A raised centre strip carrying a row of round studs… the large heads still read as beads or pearls… 'beaded spinner' or 'studded band'." | `read-test-3.json` |

What each attempt changed:
1. **The section as specified.** LowDome 6.0 × 2.1. Sphere R 0.9 sunk 0.35 (0.55 proud), ×16. Sphere R 0.5 sunk 0.2,
   ×16 half a pitch on. Blends 0.15 and 0.12.
2. **Read test 1's three changes applied.** A strap 3.0 mm wide and 0.4 mm proud, with rounded step edges and a
   5° roof for draft. Low wide heads: sphere R 1.3 sunk 0.8 (2.05 mm foot), and R 0.6 sunk 0.3 (1.04 mm foot).
3. **Read test 2's changes applied.** The strap stands 0.48 proud, and the dome falls from its foot with no shelf.
   The heads became lens caps: large 2.14 mm visible foot and 0.38 proud, small 1.43 mm and 0.30 proud. Blends
   0.10 and 0.08. The heads are turned spherical-cap sections, because a sunk sphere of R 1.8 broke through the
   bore.

Read test 3's unapplied changes, kept for whoever rethinks the ring: conform each cap's foot to the strap so no
dark crescent shows, flatten the crown (arc R ≈ 2.2), and show 1.2–1.5 mm of curved band on each side of the strap.

**Why it did not read (my diagnosis for the rethink).** A row of round domes on a crest line is the visual
vocabulary of a beaded or studded band, whatever their profile. Every reviewer named it "beads" or "studs".
What makes something "riveted" is evidence of fastening: two plates overlapping at a lap, heads in pairs
through that lap or along both edges of a strap, or a strap whose ends meet. A single centred row has none of
these. A rethink should give the strap a visible joint (a lap or a butt plate, with rivet pairs either side of
it), or move the rivets to the strap's two edges. The section's single-row, 32-head concept is the problem, not
its detailing.

## Gates (block-out attempt 3)

| Gate | Draft 768 × 320 | Export 1536 × 448 |
|---|---|---|
| Triangles (≤ 2 M) | 723,228 | 1,527,972 (built in 18.8 s) |
| Watertight / degenerate faces | yes / 0 | yes / 0 |
| `csg::self_crossings`, ring and every made part | 0; parts 0, 0, 0, 0 | 0; parts 0, 0, 0, 0 |
| `solids.notes` / `parts.notes` | empty / empty | empty / empty |
| Every CAD feature `Ok` | yes (5/5) | yes (5/5) |
| Inside finger hole (bore r 9.10) | 0 vertices, closest 9.1000 | 0 vertices, closest 9.1000 |
| Field verdict (Delft) | **Castable**; worst draft −2.39°, undercut 0.016% | **Castable**; worst draft −1.97°, undercut 0.00006% |
| Ray release 0.100 / 0.075 mm | 0 obstructions, 0 unresolved / 0, 0 | 0, 0 / 0, 0 |
| `dfm::findings_in` | 0 | 0 |
| Stones: report vs preview | 0 = 0 | 0 = 0 |
| `--verify` cold reload, empty library | — | identical (vertices, faces, normals) |
| Casting pattern | — | watertight, 0 degenerate, 0 crossings, 1,527,972 triangles |
| Lost-wax extras (recorded only) | thickness@0.8 clean, cut lands 0 | clean, 0 |

Thinnest wall 1.26 mm. Every number is in `showcase/officina/rivet/report.json` (export at the top, draft under
`draft`).

## Template gate

I ran the brief's commands verbatim (`collection_templates officina target/tpl-src --output-dir target/tpl --only
rivet --verify-export`). **It fails:** `rivet: 32 design.set patches exceed four`. Because the tool stops there,
I have no graph size against the 300 KB procedural budget and no cold-source or mesh-parity result.

The cause is the strapped crown. The strap is drawn into the band's section as a 16-point hand-drawn
`DropCurve` (see "What I could not do"). `lift::set_fields` skips object-valued fields unless the pin is Json,
so `profile.drop_curve` falls through to per-leaf patches: 16 points × (x, d) = 32. The block-out-1 design
(stock LowDome, no drop curve) would not have tripped this. The first core change below fixes it.

## Feature tree (as built, the timeline's order)

1. **Band.** LowDome 6.0 × 2.1, bore 18.2, its crown redrawn with a 3.0 mm strap stepped 0.48 mm up along the
   crest, roofed 5° for draft. The reader learns that the band's own section can carry a strap, keeping the
   parting clean.
2. **Large rivet head.** A spherical cap (foot r 1.2, rise 0.5) on a 0.4 mm buried shank, turned whole about its
   axis. It is placed with `Placement::ring(90°, −0.12)`, joined with a 0.10 mm seam bead, and stands 0.38 proud
   on a 2.14 mm foot. The reader learns placement, stand-off, Join, and the seam-bead radius.
3. **Sixteen round the ring.** A ring array of 2 with count 16. The reader learns that a whole turn steps
   span/count, so integer counts close seamlessly.
4. **Small rivet head.** A cap (foot r 0.8, rise 0.4) half a pitch on (101.25°), sunk 0.10, bead 0.08, 0.30 proud
   (the Delft detail floor) on a 1.43 mm foot.
5. **Sixteen between them.** A ring array of 4. Gap between heads: 0.42 mm, above the 0.30 floor.

Parts: four joined cast parts (two heads, two arrays), with no stones. Each head is centred on the crest line,
so each half faces its own mould half. That is why the ring stays Castable with 0 release obstructions.

## What I could not do

- **Strap as its own CAD feature.** I first built the strap as a joined `Revolve`. Rivets on it then lost their
  seam beads ("its fillet found no seam long enough to follow"). `parts::resolve_inner` unites a touching
  cluster with `tool_of` before beading, and beads only seams against the band. I moved the strap into the
  band's drop curve instead, which is what costs the template gate.
- **A cap whose arc closes on the axis.** A `Revolve` of an `Arc` ending at r = 0 fails with "Kernel could not
  tessellate 1 faces". I stopped the arc 0.02 mm short of the axis and closed it with a flat.
- **A sharp ridge on the strap roof tilts the seat normal.** Heads placed on the ridge leaned by about the roof
  angle and undercut (−4° to −6°, verdict "with care"). Rounding the crest over its middle 0.3 mm fixed it.

## Core changes wanted (exact code)

1. **Lift a hand-drawn crown as one value** (`crates/ringdesign-graph/src/lift.rs`, in `from_design`, after
   `set_fields(&mut g, profile, reg, &json_of(&d.profile), &[]);`). This assumes the `band.profile` node gains a
   `drop_curve` pin of `ValueKind::Json` with default `null`, and that its evaluator does
   `if !v.is_null() { p.drop_curve = serde_json::from_value(v)? }`:
   ```rust
   if d.profile.drop_curve.is_active() {
       g.set_input(profile, "drop_curve", Literal::Json(json_of(&d.profile.drop_curve)))?;
   }
   ```
2. **Bead the seams inside a joined cluster** (`crates/ringdesign-core/src/parts.rs`, `resolve_inner`). When any
   part after the first in a group asks for a bead, join the group one part at a time, in feature order (the
   existing fallback loop), so a head on a joined strap gets its own seam bead:
   ```rust
   let beaded_inside = group.len() > 1 && group[1..].iter().any(|g| joins[*g].blend_mm > 0.0);
   let united = if beaded_inside { Err(Snag::Refused("beaded one by one".into())) } else { tool_of(&joins, &group, cancel) };
   match united { /* existing arms; the Err arm already joins one by one and beads each seam */ }
   ```
   The `Err` arm's note should be skipped when `beaded_inside` holds. Use whichever `Snag` variant fits; this
   needs a non-cancel variant.
3. **Turn a face that closes on the axis** (`cad` revolve tessellation): accept an arc or line endpoint at r = 0
   as a pole, as `make::sphere` does, so a cap section needs no 0.02 mm flat.
