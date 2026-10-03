# Porta (`porta`): final report

Tenebrae, Porta. Lost wax in Gold 18k with a 0.8 mm minimum section, on factory 009 Drop, unmirrored and resized to 14.5 x 19, with an 18.6 mm bore.

- Branch: `claude/tenebrae-porta`.
- Example: `crates/ringdesign-core/examples/tenebrae_porta.rs`.
- Outputs: `showcase/tenebrae/porta/`.

## Outcome

**Cut at 6.7 after round 3** (ship bar 7.5).

- **Gates.** Every gate in the brief is green at draft and at export, and the template gate passes.
- **Score.** The score stayed under 7.5 in the last allowed round, so under the task's rule the ring is cut. The reviewer's verdict stands.

## Read tests and reviews

| Step | Verdict | Score | File |
|---|---|---|---|
| Block-out read test, attempt 1 | does not read (`reads: false`) | – | `read-test-1.json` |
| Block-out read test, attempt 2 | **reads** (`reads: true`) | – | `read-test-2.json` |
| Full review, round 1 | revise | 5.8 | `review-round1.json` |
| Full review, round 2 | revise | 6.4 | `review-round2.json` |
| Full review, round 3 | **cut** | **6.7** | `review-round3.json` |

**Rounds used:** 2 block-out read tests and all 3 full rounds. Each reviewer was a fresh agent, given only `target/review.md`, the name and slug, the mode, the attempt or round number, and the paths.

- **Read test 1, does not read.**
  - "A plain teardrop signet with a red cabochon… nested pointed-arch lines engraved flat", like contour lines.
  - The door slits read as a barcode and the trefoil as a club.
- **Read test 2, reads.**
  - "Face-on, this is now a Gothic church doorway on a teardrop signet… a jeweller would say 'Gothic arch door' or 'cathedral portal' without a caption."
- **Round 1, 5.8, revise.**
  - The portal read face-on, but the design was flat.
  - Gates were red: 3 thickness samples (thinnest 0.261 mm), bore margin −0.0015 mm, and no template gate recorded.
- **Round 2, 6.4, revise.**
  - Four rolled orders with capitals and bases.
  - One thickness sample was red, 0.650 mm on the factory 009 lip.
  - The punch list:
    - the hood was not an ogee, the crockets were hooks and the finial a ball;
    - the ruby was flush, with no collet;
    - the lower table was blank and the crypt read as a club;
    - the arcades read as hairpins and the splay floors were faceted.
- **Round 3, 6.7, cut.** Every gate green.
  - **Credited:**
    - the immediate portal read at face-300;
    - the four true roll orders with capitals and bases;
    - the rolled collet round the à jour ruby (round 2's P0, done);
    - the head-wall arcade, now pointed niches with capitals instead of hairpins;
    - every gate verified green against the JSON.
  - **Still short:**
    - the hood reads as a faceted wedge roof, not an ogee (asked in all three rounds);
    - the crockets are hooked curls with balls, not solid leaves, and the finial reads only at full size;
    - the shoulder lancets read as dashes;
    - the doors read as two black notches and the crypt frame as a screw head at 300 px;
    - a blank field of about 5 x 4 mm remains on the lower table, and the threshold steps are absent;
    - the splay floors still show facet bands;
    - the band from the shoulders to the palm is bare stock, and the silhouette is plain 009.
  - The reviewer ranked it with Kraken (7.0) and Corvus (6.6), below Caiman (7) and the 7.5 bar.

## Gates

### Round 3, the final build

Draft is 768 x 320 and export 1536 x 448. Both resolve to the same 219,230-triangle mesh, because the imported stock and the CAD parts set the tessellation.

| Gate (brief) | Draft | Export |
|---|---|---|
| 1. Watertight; boundary and non-manifold edges | yes; 0; 0 | yes; 0; 0 |
| 1. Degenerate faces | 0 | 0 |
| 1. Self-crossings, ring / every made part (55) | 0 / all 0 | 0 / all 0 |
| 1. Notes (parts, solids) | empty, empty | empty, empty |
| 1. Every feature Ok (196 features) | yes | yes |
| 2. Nothing in the finger hole (margin ≥ −0.01 mm) | −2.8e-8 mm, pass | −2.8e-8 mm, pass |
| 3. `cad::measure::thickness(&mesh, 0.8)` | clean: 0 of 384 below, thinnest 0.887 mm | clean: 0 of 384, 0.887 mm |
| 3. `dfm::cut_lands` at 0.8 | clean | clean |
| 3. Asserted lands (all ≥ 0.8) | trumeau 1.10; ruby light to door points 1.96; crypt to threshold 1.30; crypt to round end 1.74 | same |
| 4. DFM findings | 0 | 0 |
| 5. Stones reported = previewed | 1 = 1 | 1 = 1 |
| 6. `--verify` cold reload with an empty library | – | identical |
| 7. Casting pattern | – | closed; 219,230 triangles; 0 degenerate; 0 crossings |
| **Pass** | **yes** | **yes** (`gates_passed: true`) |

- **Field verdict (informational).** Castable. 0.56% of the surface would undercut a two-part pull, which matters only for sand.
- **Weight and stone.** 20.0 g of 18k; the ruby is 0.10 ct.
- **Design file.** 743,722 bytes at format 6.
- **The same thickness screen on the bare stock** finds 13 of 374 samples below 0.8, the thinnest 0.0002 mm. See "What I could not do".

### Earlier rounds, draft = export

| Round | Thickness | Bore margin | Everything else | Pass |
|---|---|---|---|---|
| 1 | 3 below 0.8 (0.261 mm) | −0.0015 mm (red) | green | no |
| 2 | 1 below 0.8 (0.650 mm, on the stock's lip) | −2.8e-8 mm | green; template gate 995,989 bytes, passed | no |
| 3 | 0 below | −2.8e-8 mm | green | **yes** |

## Template gate (after the last round)

`collection_templates tenebrae <src> --only porta --verify-export`, class **stock**. The record is `showcase/tenebrae/porta/verification.json`, folded into `report.json` as `template_gate`.

| | Final (round 3) | Round 2 |
|---|---|---|
| Template bytes / budget | **788,269 / 1,000,000** | 995,989 / 1,000,000 |
| Design-set patches (at most 4) | **1** (`/manufacturing`) | 1 |
| Nodes | 209 | 184 |
| Triangles; vertices, faces and normals identical | 219,230; identical | 228,764; identical |
| Cold design and graph reload, source identical, export geometry verified | all true | all true |
| Detail findings | 0 | 0 |
| **Passed** | **yes** | yes |

## Enablers used

Master was merged at the start of every round. The last merge was `47f8e88`, before round 3.

- **C-T7 / C-V3, the twisted sweep along a sketch curve.** Used for every roll, splay, hood, crocket, petal, band and collar.
  - `TwistPath::Sketch` takes Line, Arc, Bézier and Circle entities. These replaced the interim point-list paths, and saved 207 KB of template size.
  - `scale` laws swell the crocket leaves and petals, and taper the palm bevel in and out.
  - `closed` makes the ruby's collar ring.
- **C-V1 placements.**
  - `Placement::Relative` seats the arcade bosses in their roll's frame.
  - `Placement::Ring { level: false }` is used throughout.
- **Crisp-edge renders (PR #248).** The close-ups `portal-2x.png` and `stones.png` use `render::write_png_framed(…, render::yaw_facing(θ), pitch, render::Framing::new(centre, half_width), 1600)`, never a cropped mesh.
- **Not used:**
  - **C-V2 (`PatternKind::Along`) and C-V5.** They reached master in `dca7f61` (PR #258) only after the round 3 review. There was no round left to use them in, so the six crockets stay hand-placed, the interim form the task allows.
  - **`crisp_relief` and `StampTop::Pillow`.** Not needed: the ring has no height-field relief and no stamps.
- **Format 6.** `design.ring.json` is written at format 6. That comes from the preset stock source, the extended sweeps (sketch paths and scale laws) and the extended placements, not from the crisp opt-ins.

## The feature tree, as sentences

The tree has 196 features, all Ok. On the band: 54 CAD parts, 23 joined and 31 cut.

1. **The band.**
   - Factory 009 Drop is attached as the band.
   - It is resized to 14.5 x 19 on an 18.6 mm bore, unmirrored, and drafted for lost wax with a 0.8 mm minimum section.
2. **Four archivolts.**
   - Each order is a pointed drop arch, sunk one 0.45 mm step deeper than the order outside it.
   - Its floor splays toward the next riser, through a wedge swept round the arch and united with the pocket.
   - A half-round roll (r 0.3) runs up the foot of each riser, from sill to sill over the apex.
   - Each order's cut is taken round its roll, so the roll stays the stock's own metal. The same is done for a capital where the roll springs and a base where it stands, on each side.
   - Every apex finishes on a short straight tangent, so the sweep's mitre never meets a curve.
3. **The tympanum and door opening.**
   - They are sunk inside the inner order.
   - Spared out of that cut: a chamfered lintel beam across the doorway; the trumeau's round column between the doors, with a capital and a base; and a round collar ring (a closed sweep) round the ruby.
4. **The doors.**
   - Two lancet-headed door leaves are pierced through to the finger, either side of the trumeau, so they read dark.
5. **The ruby.**
   - A 3.0 mm round ruby is set as high in the tympanum as its collar allows.
   - A bezel collet holds it in the tympanum floor.
   - Its seat is burred into the floor and collar, not through.
   - A light is opened under it through to the finger, à jour.
6. **The hood.**
   - The ogee hood-mould rolls over the arch with a 1.0 mm gap.
   - Its path is a circular arc that turns into a reversing Bézier S-curve, mirrored to meet at the point.
   - The curve is chosen as the strongest reverse that still clears the arch by 0.22 mm; it clears by 0.235 mm.
   - Each end stops on a label-stop boss.
7. **The crockets.**
   - Three crockets grow up each side of the hood.
   - Each is a leaf swept along an arc. It swells and narrows on a scale law, curls through 95°, and ends in a bud.
8. **The fleur finial.**
   - A centre petal rises and is pointed.
   - Two side petals curl outward along arcs, swelling on a scale law, each with a bud.
   - A band binds the three.
9. **The crypt window.**
   - Below the portal, a round frame is sunk 0.3 mm, with a roll round its foot spared from the cut.
   - A quatrefoil is pierced through the round end inside it.
10. **The wall arcade.**
    - Down each long wall of the head stand five blind lancet arches.
    - Each is a pocket sunk 0.3 mm with a 10° draft.
    - A half-round roll runs round its foot, with a boss at its point and a capital at each springing. All of them are spared from the pocket.
11. **The palm bevel.**
    - Front and back, a 45° wedge is swept round the bore's edge across the palm.
    - It swells in from nothing and dies away again on a scale law, so it leaves no step.
    - It also removes the stock's 14 folded facets at the palm edges.
12. **The shoulder lancets.**
    - Four lancets are sunk 0.2 mm down each shoulder, at 47°, 56°, 65° and 74° off the crown.
    - They diminish from 2.1 to 1.45 mm, and each points to the head.
    - Each is drafted 11.6 to 15° (eight times its length, at most 15°), so its rim is never acute where the shoulder rounds away.

## Parts, stamps and stones, and why

- **Joined parts (23).**
  - What they are: the hood-mould and its 2 label stops; 6 crocket leaves and 6 buds; and the fleur, which is its centre petal and point, 2 curled petals, 2 buds and the band.
  - Why: they are the only things that stand above the table, as a hood stands proud of a portal's wall. Each is bedded 0.06 mm into the table, so it unites with it.
- **Cut parts (31 on the band).**
  - What they are: the 4 orders with their splays; the tympanum and door opening; the 2 door leaves; the ruby's light; the crypt frame and quatrefoil; the 10 arcade pockets; the 2 palm bevels; and the 8 shoulder lancets.
  - Spared tools: every roll, capital, base, boss, the lintel, the trumeau and the collar is a cut tool with its moulding taken out by a Boolean.
  - Why: a moulding inside a recess then stays the stock's own metal, instead of being a join that a later cut would swallow. Each roll's circle centre sits under its floor, so it passes the thickness screen by construction.
- **Stamps: none.**
  - The arcade was a stamp candidate. It became sunk pockets with spared rolls so that it reads as architecture, not texture, and so its 0.8 mm walls are controlled by cuts.
- **Stones: one Round 3.0 mm ruby** (0.10 ct, `reference-ruby.stl`).
  - How it is set: in a bezel collet in the tympanum, its seat burred, its light open to the finger (à jour).
  - Why: the spec puts it in the tympanum, where a portal carries its image. The collar ring makes it read as set, not dropped. Stones reported 1 = previewed 1.

## What I could not do

- **The reviewer's design asks, not met in three rounds:**
  - a hood that reads as a true ogee, not a wedge roof;
  - solid lobed crockets and a larger fleur;
  - architecture carried onto the band and silhouette;
  - lancets that read as windows;
  - reveals on the doors;
  - a larger cusped crypt;
  - a filled lower table;
  - smooth splay floors.
  - These are why the score is 6.7.
- **Threshold steps.**
  - I tried two steps down into the portal in round 3. They left walls under 0.8 mm against the crypt frame and the order sills, so I removed them, and the lower table stays plain.
- **The factory stock's skirt.**
  - 009 has a thin, outward-flared skirt under the head at the round end and the point. It sits about 8.9 to 9.5 mm over the axis, under a waist groove.
  - The bare stock fails the screen there 13 times in 374, and in rounds 1 and 2 that skirt kept the thickness gate red.
  - In the final build the cuts move the sampled faces, and the screen does not land on the skirt (0 of 384).
  - The skirt itself is unchanged. Trimming or filling it would mean redrawing the factory profile. I say this plainly so the green screen is not taken as proof that the skirt is 0.8 mm.
- **Acute rims.**
  - A straight extrude on a curving surface leaves one rim slightly acute, and the screen reads a sample beside it as a few hundredths of a millimetre.
  - I drafted every such pocket (10° on the arcade, up to 15° on the shoulders). The core has no direct measure of rim angle (see core change 3).
- **Apex tangents.**
  - Each sweep's apex ends on a short straight tangent, a workaround for the twist mitre (core change 1).
- **CSG degeneracies.**
  - Two tools (archivolt 1 and one arcade arch) were silently refused with "solids meet degenerately". Archivolt 1 vanished from the render.
  - I cleared both by moving the roll's bed by 4 µm (`ROLL_BED`) and a capital by 20 µm (core change 2).

## Core changes wanted (exact code; `src/` was not edited)

### 1. Twisted sweep: let a mitre's keep allow for a curved neighbour

**Where.** `crates/ringdesign-core/src/cad/twist.rs`, in `stations`, and in the same line of `stations_round` and the closed-loop variant.

**Problem.** A pointed arch built from two arcs folds near its apex ("folds through itself 48% along the path"). The arcs bend the same way as the corner, so the station at the keep's edge already carries extra turn.

**Fix.**

```rust
        let keep = if cos < 1.0 - 1e-12 {
            let half = cos.acos();
            let plain = 1.1 * reach * half.tan();
            // A neighbour bending the corner's way turns the section further inside the keep; widen the half-turn by
            // what either neighbour turns over the plain keep, so the station at the keep's edge clears the mitre.
            let extra = [&walk[k].0, &walk[k + 1].0]
                .iter()
                .map(|c| bend_of(plane, c).1)
                .filter(|r| r.is_finite() && *r > reach)
                .map(|r| plain / (r - reach))
                .fold(0.0, f64::max);
            1.1 * reach * (half + extra).min(1.45).tan()
        } else {
            0.0
        };
```

### 2. Parts: nudge a degenerate cut, as `Snag::Degenerate`'s own doc says

**Where.** `crates/ringdesign-core/src/parts.rs`, in the loop over `cuts` (about line 349).

**Problem.** `Snag::Degenerate` is documented as "a nudge resolves it", but the cut is simply refused. Here that silently dropped the whole outer archivolt.

**Fix.**

```rust
    for p in &cuts {
        check(ctx.cancel)?;
        let mut tool = std::borrow::Cow::Borrowed(&p.solid);
        let mut tried = chain.combine(&tool, Op::Subtract);
        // A coincidence the exact predicates found breaks under a nudge far below any tolerance that matters.
        for k in 1..=3 {
            if !matches!(tried, Err(Snag::Degenerate(_))) {
                break;
            }
            let e = 2e-6 * k as f64;
            tool = std::borrow::Cow::Owned(Solid { v: p.solid.v.iter().map(|q| [q[0] + 0.577 * e, q[1] + 0.578 * e, q[2] + 0.576 * e]).collect(), f: p.solid.f.clone() });
            tried = chain.combine(&tool, Op::Subtract);
        }
        match tried {
            Ok(t) => {
                let own = vec![p.index; tool.f.len()];
                let beads = chain.beads(&t, &tool, false, &[p], &own, &|_| None, &in_stand_ins, &mut out.notes, &mut touched)?;
                chain.take(t, &own, p.index, base);
                chain.lay(beads, Op::Subtract, base, None, &mut out)?;
                out.cut += 1;
            }
            Err(Snag::Cancelled) => anyhow::bail!(cad::CANCELLED),
            Err(e) => {
                refused.push(p.index);
                out.notes.push(format!("{}: could not be cut from the band ({e})", p.name));
            }
        }
    }
```

The same retry belongs in the join loop, where "could not be united with each other … joined one by one" comes from the same snag.

### 3. Thickness screen: do not count a rim as a wall

**Where.** `crates/ringdesign-core/src/cad/measure.rs`, in `thickness`.

**Problem.** A ray from a face beside a convex edge reports the few hundredths of a millimetre to the face across that edge. An 80° to 89° rim reads as a failure, while a real knife edge, sharper than 80°, should still count.

**Fix.**

```rust
    let unit_normal = |(a, b, c): &([f64; 3], [f64; 3], [f64; 3])| {
        let n = cross(sub(*b, *a), sub(*c, *a));
        let l = norm(n);
        n.map(|v| v / l.max(1e-300))
    };
    let shares = |i: usize, j: usize| mesh.faces[i].iter().any(|v| mesh.faces[j].contains(v));
    // ... inside the sample loop, replacing the `nearest` search:
        let own = unit_normal(&triangles[index]);
        let nearest = triangles
            .iter()
            .enumerate()
            .filter(|(j, _)| *j != index)
            // A neighbour across a rim no sharper than 80° is the rim, not the far wall.
            .filter(|(j, t)| !(shares(index, *j) && dot(own, unit_normal(t)) > -0.17))
            .filter_map(|(_, (a, b, c))| hit(center, inward, *a, *b, *c))
            .min_by(f64::total_cmp);
```

## Commits on `claude/tenebrae-porta`

| Commit | What |
|---|---|
| `9b2f1dc` | Block-out, read tests 1 and 2 |
| `c12bc50` / `4b5a8b7` | Round 1 build / review |
| `b077573` / `e1a4909` | Round 2 build / review |
| `47f8e88` | Master merge for round 3 |
| `56f8bef` / `7f4e9ac` | Round 3 build / review |

This report is the last commit.
