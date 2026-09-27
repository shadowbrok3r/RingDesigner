# core-comfort-apex: report

PR: https://github.com/shadowbrok3r/RingDesigner/pull/236 (branch `claude/core-comfort-apex`)

## Change
The comfort dome's apex now sits on the parting plane. In `profile.rs` `sample_spaced`, `bore_r` uses `apex = 0.0.clamp(b_lo, b_hi)`, and each side reaches full depth at its own edge. The result is bit-identical when `z_center_frac == 0`. There is no new saved field, so the format is unchanged. CLAUDE.md is updated in two places: the casting constraint and the Bypass entry.

## Measured
On a 6 × 2.8 mm LowDome bypass at comfort 0.2 (Corvus's band), with ray release at a 0.1 mm pitch:
- **Before:** 36 bore obstructions, up to 0.90 mm deep.
- **After:** none on the bore.

The request's figure of 46 at 1.2 mm was measured on the full Corvus. What remains on the test band is one 0.024 mm crest-row sample, which was there before and is not the bore's. The new test pins this.

## Built geometry that changes
I hashed every mesh before and after (85 builds).
- **Showcase designs and stocks:** none change.
- **Bestiarium (Arachne, Manticora):** neither changes.
- **Template Toi et moi:**
  - Volume 388.980 → 388.977 mm³.
  - Obstructions 53 → 49. Hits at the bore radius go 21 → 7, and the deepest bore-flank hit goes 0.48 → 0.10 mm.
  - The field is unchanged: Castable, 0.0629%.
  - Of the 7 left, 2 are its stone seats. The other 5 are chord samples of 0.02–0.10 mm at the apex, explained under Open below.
- **Template Shouldered cushion signet:** float noise only (centre 3.6e-16). Its figures are unchanged.
- **Fixtures (not shipped):**
  - Heart signet: volume −0.001 mm³.
  - Toi et moi (two heads): obstructions 9 → 0.
  - Wishbone wave: obstructions 20 → 0.
  - Waved hexagon signet: noise only.

## Pins updated (only because of this change)
- The golden corpus: 8 rows (Toi et moi; the Heart, two-heads and Wishbone fixtures; the Wave, Twist, Bypass and Signet shanks).
- The Heart signet hash in `stamps_strike_as_master_struck_them`.

The rewrite also flipped Braided band's `worst_draft_deg` from 0.0 to −0.0. I reverted that because its geometry is unchanged.

## Tests
- ringdesign-core: all 810 lib tests pass, and golden passes.
- ringdesign-graph: all targets pass.

## Is teaching the field verdict to read bore samples worth doing?
As a verdict change, no. The verdict deliberately treats the bore as a cored or reamed vertical wall. Read naively, every straight bore would come back as zero draft. The ray-release analysis already caught this defect, and after this fix the bore widens away from the plane by construction.

A cheap guard would be worthwhile, though. It would check each section's bore samples for "r non-decreasing away from z = 0", as a debug assert or a template-corpus gate. That would catch any future bore change that re-centres the dome, without adding noise to the verdict.

## Open
On off-centre sections no bore row lands on z = 0. The chord across the apex then leans by less than 0.001 mm radially over one row, which accounts for Toi et moi's 5 residual samples. Holding a bore row on the apex, without letting rows jump as the apex slides along the sweep, would clear them. I left it out as beyond the request.

Note: TASK.md named `/home/user/repo`, but that directory did not exist, so I worked in `/home/user/RingDesigner`.
