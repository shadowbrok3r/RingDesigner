# Two audits, one day — what they agree on

On 2026-08-30 this repo was audited twice, independently and simultaneously, by two sessions that
could not see each other's work:

- **The core audit** — [`AUDIT-2026-08-30.md`](AUDIT-2026-08-30.md). 26 agents over 13 dimensions
  (signet-heads, sand-process, lost-wax, ux-workflow, ring-typology, code-architecture, interop,
  stone-setting, sizing-fit, ornament, product-business, dfm-verdict, competitor-parity), each
  finding checked by an adversarial verifier. **243 findings**, milestones M10–M24.
- **The mobile audit** — started from `examples/ringdesigner-android` in the EguiMobile workspace
  and read back into the core. 7 lenses (sand-caster, lost-wax, bench-jeweller, design-breadth,
  mobile/S-Pen, desktop-to-mobile parity, on-device AI), same verify-then-keep method.
  **114 surviving recommendations**, filed as 74 issues.

Neither knew the other existed. That makes the overlap the most useful thing in either document:
**where two independent passes with different starting points reached the same conclusion, the
conclusion is probably right.** Where they differ, it is almost always because one had a vantage
the other lacked, not because one was wrong.

The core audit's numbering won. Its M10–M24 are the tracked plan; the mobile audit's 53
RingDesigner issues were folded into those milestones, and its 29 phone-only issues live in
[`shadowbrok3r/ios-egui`](https://github.com/shadowbrok3r/ios-egui) under a `P1`–`P9` set that does
not pretend to share this repo's numbering.

---

## Reached independently by both

Ranked by how much the corroboration is worth.

| Finding | Core audit | Mobile audit | Now |
|---|---|---|---|
| **`CastProcess::apply` never restores the sand floors.** Flip to lost wax and back, and a Delft-clay ring is judged at 0.5 / 0.15 and reads Castable. | F21 (`high`), also found as F114 | Filed as #101 | #101 closed as a duplicate of **#169**; its one additive detail (`apply` never writes `min_draft_deg` either, in either direction) carried over as a comment |
| **The app models the pull and nothing about the pour.** No gating plan, no sprue, no feeder, no vents, no flask, no charge — while `modulus_scan` already computes the Chvorinov input and survives as one grey label. | F1 (`critical`), also found as F124 and F242 — three of their own dimensions; plus F2, F12, F13, F14 | The single largest theme; 14 issues | **M19**, now holding 16 issues from both passes |
| **The phone has no undo, and buttons that destroy work without confirming.** | F181 (`high`) | Filed with the mechanism: `history.rs` has *no egui dependency* — only `std::time`, `RingDesign` and `serde_json` — so it moves to core verbatim | #106 (core move) + ios-egui#3 |
| **The 28 procedural patterns are frozen.** Zero-parameter `fn(f64,f64)->f64`, no seed. | F75 (`critical`) | Same, plus the DFM consequence: a generated or imported tile must be measured by `min_feature_px` before it is offered | **M16**, #126 |
| **"Many combinations" has no tool.** | F168 (`critical`) | Same, with the mechanism named: the evaluator does longest-list matching and there is **no cross-reference node**, so 3 profiles × 4 outlines runs 4 times, not 12 | **M17**, #123 |
| **Nothing refits when the size changes.** | F148 — angular windows drift against millimetre-placed ornament across a size run | #103 — tiling cells stretch ~58% across a 3→13 run, because `ringdesign-cli/src/main.rs:174` assigns `d.size` without the `fit_to_side_faces` / `repeats_for_square_cells` pair that `templates.rs:61-62` calls | Same root cause, two symptoms; both in **M10** |
| **Sizing is US-only, untested, and ignores band width.** | F132, F134, F143, F144, F147, F160 | #136, #137, #141 | **M21** |
| **The setter is handed no setting numbers** — no bearing angle, bur size or seat depth. | F67 | #138 | **M20** |
| **Wax is cut at the sand patternmaker's allowance.** | F27 (`critical`) — "about one ring size of error" | #148 — the arithmetic for the single step: a size 7 is 54.35 mm of inner circumference, sterling's +1.94% adds 1.054 mm, and 1.054 / 2.55 = **0.41 US size** | **M18**, #148 |
| **No pierced gallery or open back.** | F107 | #144 | **M15** |
| **No quote: no pour weight, no labour, no stone cost.** | F125, also F210 | #145 | **M23** |
| **The library has no catalogue.** | F212 | #135 | **M13** |
| **The graph has alpha sources but no alpha operators.** | F80 | #133 | **M16** |

---

## Where the core audit is sharper, and mine was corrected

Four places where the concurrent pass had the better answer. Each is now a comment on the mobile
audit's issue rather than a silent divergence.

**The bore is a bench surface, not a cast one.** The mobile audit filed "give the bore its own
height field" as XL new geometry. The core audit's refusal registry settles what it is *for*: the
sand column in the finger hole is one rigid piece pulled along Z, so nothing cast goes inside the
ring — the bore is cored and reamed, and an inscription is a bench operation. The field should
drive a **1:1 mirrored bench readout** with the engraving depth and the wall remaining after the
cut. Their **N5** then makes it useful in a way neither of my lenses reached: you cannot strike a
hallmark into a 1.0 mm shank without bellying the ring, and you need a flat land about 2.5 mm wide
to land the punch — and the app knows the wall thickness at every angle and never says it. That
reframing probably takes #146 below XL.

**Piercing is a rule about axis, not a capability.** A hole whose axis is ±Z, punched side face to
side face, is parallel to the pull and casts. A hole through the band's *thickness* is a horizontal
core and does not. The mobile audit's #144 treated "pierced" as one thing.

**The shrink ladder has five stages, not two.** Master → rubber mould (−1.5 to −3%) → wax (−0.4%)
→ investment (+0.2%) → metal (−1.3 to −1.9%), compounding (their **N3**). The mobile audit's
`total_scale` modelled wax and investment only. Both numbers hold — mine measures the single wrong
step, theirs the third-generation ring — and the ladder is what `total_scale` should take.

**The fill floor is a property of the pair, not of the sand.** Their **N8**: bronze fills sections
sterling will not. The mobile audit filed a pour-temperature ceiling per `SandProcess` (#102) and
missed that `min_section_mm` should be a function of (sand, alloy, pour temperature) rather than a
constant of the sand alone.

### One correction the other way

Their **N1** says *"Nothing is ever shown at actual size — no life-size viewport lock, no DPI
calibration, no scale reference."* That is true of the desktop and **false of the phone**, which
has shipped a true-DPI 1:1 mode since 0.3.0: `HostExt::display_dpi()` reads the panel's real
`xdpi` rather than the density bucket Android rounds to, guarded above 40 dpi, with the toggle
hidden when the panel reports nothing trustworthy (`app.rs:2160-2166`). The phone is the device
that already answers N1, and ios-egui#22 extends it into a ring sizer and a finger caliper — lay a
real ring on the glass, match a circle, read the size.

---

## What only the mobile audit found

Its starting point was the phone, so this is everything a core-first read has no vantage on. None
of it appears among the 243 findings.

**The S-Pen is being thrown away.** `Axis::Tilt` and `Axis::Orientation` exist in the patched
`android-activity` and are read at `record_pointer_from_event` — and `PointerProbe` stores only
tool, buttons and hover, so both are discarded one line before they would be useful. Separately the
paint loop decimates a 120–240 Hz pen to the frame rate, and pairs a palm's pressure with the pen's
position. (P3, ios-egui#10, #11.)

**The phone cannot manage a layer.** It can *create* them — pavé, channel, painted band — and the
only stack operation it has is Clear layers. No list, no select, no mute, no reorder, no delete, no
per-entry blend / opacity / window / mask / remap. 3,081 lines on the desktop; the phone-shaped 10%
is one sheet. (P2, ios-egui#4.)

**Dead capability, reachable on the desktop and not on the phone.** `CastProcess::LostWax` and the
`SandProcess` presets have no picker at all — `grep 'LostWax\|CastProcess\|SandProcess' src/`
returns nothing, so the only way to reach lost wax on the phone is to sync a design that already
has it set. Nor is there any control for `min_draft_deg`, `min_section_mm`, `min_detail_mm` or the
parting plane, which between them drive every verdict, every DFM finding, the wall legend, the
as-cast radius **and the pen's depth ceiling**. (P1, ios-egui#1.)

**Its own report is behind a hover.** `dfm::findings_in` runs on every settled build and the result
is a bare count chip whose messages live only in `.on_hover_text` — unreachable with a finger.
(ios-egui#2, and the gate everything generative depends on.)

**The pen can author what nothing else can.** `CustomOutline::from_points` takes ≥8 points of a
closed polyline; `DropCurve` carries a `monotone` flag that *is* the no-undercut guarantee. So a
face plan drawn with the S-Pen, and a band section drawn with `monotone` on, cannot produce an
uncastable profile by construction. (#108, ios-egui#16 — the phone half of their F182.)

**A filing bug in the shared host.** `egui-android/src/host.rs:445-453` hardcodes
`Pictures/ComfyUI` and `Movies/ComfyUI`, so a RingDesigner render is saved into another app's
album. (ios-egui#9.)

**The silhouette cache is a hardcoded array of 11.** `field.rs:2772` — `static T: [OnceLock<Silhouette>; 11]`
indexed by `SignetOutline::index()`. The twelfth builtin outline panics before anything else can be
tested, which blocks their F188 as much as my #127. (#104.)

**On-device AI is already sitting in the same workspace.** `comfyui-android` ships working
`qnn-rs`, `local-sd`, `local-anima`, `local-clip`, `local-wd14` and `local-rewrite` against the
Qualcomm HTP. The interesting half is not generation — it is that this app can *judge* the result:
a generated tile goes through `Alpha::min_feature_px` at the layer's own mm-per-texel and the sand
answers. (P9, gated on ios-egui#2.) The mobile audit also refused four uses on measured grounds:
alpha super-resolution (512 texels at a 3 mm cell is ~6 µm/texel against a 0.4 mm floor — two
orders the mould cannot use), LAION aesthetic ranking (a linear probe over CLIP embeddings of
photographs, out of distribution for a grayscale relief tile), the WD14 tagger (a danbooru anime
classifier would label a herringbone "monochrome, greyscale"), and LLM graph authoring
(Qwen2.5-0.5B will not author valid JSON over a 130-node registry).

**Photo-to-height is done on an assumption the UI itself warns is wrong.** `Alpha::from_bytes`
treats luminance as height, and the picker tells the user a photograph records the lighting as much
as the surface. Flash/no-flash differencing fixes it with no model, no pack and no NPU.
(#153, ios-egui#26.)

---

## What only the core audit found

Read [`AUDIT-2026-08-30.md`](AUDIT-2026-08-30.md) for all of it; the mobile pass reached none of
these. The structural ones:

- **`LayerEntry::stage`** — the model has one stage where the trade has two. Every layer is cast
  geometry judged against the sand's detail floor, so bright-cut, wriggle, guilloché, intaglio
  seals and inside lettering are called NotCastable or measured as mush. One concept unlocks four
  families (F93, M14).
- **The axial web** — `thinnest_wall` and `bore_span_wall` are both radial. Nothing measures metal
  *across* the band, which is exactly where the doctrine sends every deep carve, and
  `OpenworkLayer`'s own cap is documented as opening up there. Four of their dimensions found this
  independently (F115 ≡ F4 ≡ F226 ≡ F47, M11).
- **There is no CI.** 474 tests and no `.github` (F155).
- **The report panel prints the retired mesh analyzer's numbers under the field verdict's banner**
  (F123 ≡ F166).
- **The refusal registry** — generalising what `pave::halo` and the prong report already do well:
  reason, measurement, remedy, for baskets, tension settings, galleries, cross-thickness piercing,
  bore features, two-tone, inlay retention and blind pockets in the cope.
- **Free mode ships dark** — `ringdesign-gui` declares `default = []`, so no UI can enter a whole
  documented, tested mode (F33, M22).
- **N2 stock list and stretch-out** — cut length is `π(ID + t)`, the app knows both exactly at every
  angle, and it is the most-used calculation at a bench.
- **N4 the trade quotes in pennyweight**, and there is no melt/alloy calculator.
- **N6 nothing declares which way is up** on any drawing, sheet, stone map or parting-line SVG.
- **N7 no reference-image underlay** to trace a sketch against — the natural front door for a drawn
  head plan, in a shop whose `examples/sketches.rs` proves it works from pencil.
- **N9 the inside edge break should be a default**; **N10 mirror the design** for a left hand or a
  matched pair, which is one boolean over the chart's `u`.

---

## Where everything lives now

| | RingDesigner | ios-egui |
|---|---|---|
| Milestones | M10–M24 (the core audit's) | P1–P9 (phone-only) |
| Mobile-audit issues | 52 open, folded into M10–M24; #101 closed as a duplicate | 29 |
| Evidence | `AUDIT-2026-08-30.md`, this file | — |
| Roadmap | `docs/ROADMAP.md` M10–M24 | — |

The mobile audit's own milestone set on this repo was deleted after its issues were re-milestoned;
nothing was lost. `docs/ROADMAP.md` was briefly appended to by the mobile pass before the collision
was noticed, and reverted — its M10–M24 content is the core audit's alone.
