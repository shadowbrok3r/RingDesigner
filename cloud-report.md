# Core: relief-aware sections, sturdier sculpts, figures grown out of the band

Branch `claude/core-relief-sculpt` (from `master` at `8e5a59a`; master has not moved since). Three commits before this
report: `9831909` (requests 2 and 1), `3d35ac4` (5, 3 and 4), `7732a21` (5 reworked as a collar). Core only;
no graph-crate change. Every existing design, template and example builds as before: the golden and template tests pass.

## Per request

1. **`dfm::face_sections` — fixed.** Each face's single-ray section, in face order (`None` for a degenerate face, one
   turned toward `up`, or one no ray leaves by). `part_sections` is now a fold over the same per-face reads with the
   same arithmetic, bit for bit (the existing claw/collet/cone test passes unchanged).
2. **Relief-aware census — fixed, as a new function.** `dfm::part_sections_relief(solid, up, floor_mm, &Relief)`
   returns `ReliefSections { thinnest_mm, under_mm2, relief_mm2, relief_thinnest_mm }`. A face under the floor is a
   relief chord when its ray runs within `max_deg` of the base surface's tangent plane **and** leaves the metal
   within `height_mm` of the base. The base is either a skin field (`Relief::skin`, the part without its relief —
   Moloch's `land_census` rule, `skin(exit) > -height`, with the angle test added so a thin toe or fin read across
   stays a section) or, with no skin, the mesh's own normals averaged over `radius_mm` (Sphenodon's tangent-plane
   option, for painted relief). `part_sections` is untouched. Limitation, documented: without a skin, a feature
   narrower than `radius_mm` standing on the hide can read as relief; pass a skin there.
3. **Decimation that reports where it crossed — fixed.** `sculpt::clean_decimate_or_sites(raw, target)` is
   `clean_decimate`'s four tries, returning `Err(sites)` (where the first try crossed) instead of the raw mesh.
   `clean_decimate` is unchanged.
4. **Relax that repairs its own folds — fixed.** `sculpt::relax_clean(mesh, field, rounds)` is Moloch's round-4 fix:
   `relax`, then the unrelaxed positions put back within 0.3 / 0.6 / 1.2 mm / everywhere of each crossing until none
   is left; returns how many vertices were put back. Where nothing folds it equals `relax` bit for bit.
5. **Band-blended join for stored sculpts — fixed, as a collar.** `Component::fillet_into_band` (mm; 0 = off, not
   serialized, so every existing file is byte for byte) on a joined `Operation::Stored` part. At build the part is
   grown out of the band as built (`ctx.surface`) by `sculpt::fillet_into`:
   - a **collar** is meshed round the foot from `smin(part + tuck, band + sink + dive, r)`, clipped to the part's
     footprint, ending inside the part above the fillet; it is relaxed (`relax_clean`), decimated
     (`clean_decimate_or_sites`) and united with the **unchanged stored mesh** by `csg`; the result must be closed
     with `self_crossings == 0`;
   - the band is sunk 2% of the radius and the part tucked the same, so the fillet crosses both at a few degrees
     rather than lying on either (coincident sheets are what `csg` cannot take); past the foot the band curves down
     0.2 mm before the clip, so the collar's rim is buried deeper than decimation moves a crease (found when a
     0.02 mm-sunk rim surfaced through the band);
   - the first version remeshed the whole part in the field domain as Moloch proposed; on Moloch's 230 k-face sculpt
     it worked but softened the tubercle hide to the fillet's step, so it was replaced by the collar, which keeps
     every vertex clear of the band bit for bit;
   - the band is read as the exact signed distance to the built band mesh (`sculpt::MeshField`: BVH nearest point,
     side by three skew rays), not `Stock::of` — that needs atlas samples dense enough for nearest-sample distance,
     and works for any band, factory stock included;
   - not joined, no band, or a collar that will not come clean or unite: the part stands as stored with a note on
     its feature; a radius outside (0, 2] mm fails the feature by name;
   - fenced: a design carrying a stored part is already format 6; `fillet_into_band` ≠ 0 anywhere in a graph's JSON
     fences graph format 2 (`template_features_in_json`). The cache signature hashes the band's epoch and the radius
     only when it is set.

## Tests (all pass)

- `cargo test -p ringdesign-core -- --test-threads=4`: **845 passed, 16 ignored; golden 1 passed** (twice: after the
  first two commits and after the collar).
- `cargo check -p ringdesign-graph -p ringdesign-cli -p ringdesign-mcp`: clean.
- New, each failing without its fix:
  - `dfm::measured_tests::relief_is_told_from_a_section` — granules on a rounded slab: plain census reads < 0.1 mm
    and > 0.5 mm² under (the reported failure); relief census sets them apart, keeps a 0.3 mm fin as the thinnest
    section, `under + relief == plain under`, and the no-skin mode also catches the granules.
  - `sculpt::tests::a_clean_relax_undoes_only_its_own_folds` — plain `relax` folds a ball's skin through a bead just
    under it; `relax_clean` leaves no crossing, restores under a quarter of what moved, nothing far from the bead.
  - `sculpt::tests::a_decimation_that_crosses_says_where` — `clean_decimate` hands back a crossing mesh; the new one
    returns sites on the two balls' meeting circle.
  - `sculpt::tests::a_part_grows_out_of_the_stock_with_a_fillet` — closed, uncrossed, post vertices above the
    collar kept bit for bit, metal at the foot where the plain post has none, nothing grows out of reach.
  - `cad::stored::tests::a_stored_part_grows_out_of_the_band_with_a_fillet` — a sculpted dome on the **Court band's
    procedural band** and on a **factory stock** (`PRESETS[0]`): joined with no note, ring closed, part closed and
    uncrossed, 0.05–1.0 mm³ of fillet added, within reach of the stored part, stored vertices clear of the band kept.
  - `cad::stored::tests::growing_into_the_band_is_opt_in_and_bounded` — a separate part notes and stands, 5 mm
    fails by name, key absent when 0, round-trips, fences design 6 and graph 2.
- Probe (not committed): Moloch's `design.ring.json` with `fillet_into_band = 0.4` at 768 steps: joined, closed,
  +3.2 mm³, **88 s** against 3.4 s plain; renders show the feet and tail grown into the band and the hide unchanged.

## For a ring author

Set `component.fillet_into_band = 0.4` (say) on your joined stored sculpt and build as usual: the part grows a real
fillet into whatever band it stands on, its own mesh untouched above the fillet; leave `blend_mm` at 0 (the seam bead
is the other mechanism). Expect a minute or two per uncached build of a large sculpt (the collar's distance queries),
and read the feature's notes — a fillet that will not come clean says so and the part stands as stored. For lands,
replace hand-rolled censuses with `dfm::face_sections` (fold by your own face kinds) and
`dfm::part_sections_relief(&solid, None, floor, &Relief { skin: Some(&body_without_relief), radius_mm: 0.0,
max_deg: 45.0, height_mm: relief + 0.05 })`, quoting `thinnest_mm`/`under_mm2` as sections and `relief_mm2` as relief;
without a skin, `radius_mm` about one relief cell. In your sculpt chain, `relax_clean` replaces relax-then-undo, and
`clean_decimate_or_sites` gives you the crossing sites to mend the field at instead of a raw-mesh fallback.
