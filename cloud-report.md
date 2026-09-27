# Seam bead radius now comes from every part on the seam

Pull request: https://github.com/shadowbrok3r/RingDesigner/pull/234 (branch `claude/core-seam-bead-radius`, into master)

A seam's bead radius is now the largest `blend_mm` among every part that meets on it, whatever the face order. Before, it came from whichever part owned the seam's first face. No shipped design changes geometry, and the core and graph test suites pass.

## What changed

The fix is in `Chain::beads` in `crates/ringdesign-core/src/parts.rs`.
- **Radius:** the bead takes the largest `blend_mm` among all parts behind the seam's tool faces. When parts tie, the first in face order wins, so a seam whose parts share one fillet builds exactly as before.
- **Naming:** the bead is named for the part whose radius it took.
- **Notes:** every filleted part on the seam now counts as reached. Otherwise a smaller fillet covered by a larger one would get a false "its fillet found no seam long enough to follow" note.

**Format:** no new saved field, so no format fence.

**CLAUDE.md:** one sentence added under `Component.attach` / `stage` / `blend_mm`: a seam across several parts of a joined cluster takes the largest `blend_mm` among them.

## The new test

`a_seam_between_a_filleted_and_an_unfilleted_part_takes_the_fillet_whatever_the_face_order` joins two overlapping posts on the Court band, so one seam loop runs round both.
- **Cases:** four builds, the 0.3 mm fillet on each post in turn, with each post first in the document.
- **Checks:** each build has one bead, named for the filleted post, and no notes. All four add the same 0.1036 mm³.
- **Against the old code:** it fails with `right: its fillet found no seam long enough to follow`.

Swapping document order alone would not have caught the bug. The seam loop always starts on the left post, so the fillet also has to move between the posts.

## Designs whose geometry changes

None. Only Manticora has CAD parts with a nonzero `blend_mm` (collet 0.0, calyx 0.3, knob 0.4).
- I rebuilt Manticora and Arachne at export resolution (1536 × 448) under the old and the new code. Both are identical bit for bit:
  - Manticora: 1098.6747 mm³, 1,372,332 triangles, 2 beads.
  - Arachne: 1013.4957 mm³, 1,323,510 triangles, 0 beads.
- No bundled template, fixture, graph template or other showcase design has such a part.
- No golden hash, measured figure or template mesh moved.

## Test results

`cargo test -p ringdesign-core -p ringdesign-graph -- --test-threads=4`: everything passes.
- ringdesign-core: 810 passed, 16 ignored; golden: 1 passed.
- ringdesign-graph: 102 passed; bestiarium_templates, cad_edits, imported_bases, showcase_templates and template_nodes all pass.

## Notes

- `/home/user/repo` did not exist, so I worked in `/home/user/RingDesigner`, where the repository was cloned.
- The before/after rebuilds used a throwaway example program, not committed.
