# core-hide-steadied

- Branch: `claude/core-hide-steadied-v2` (one commit on master 7bbf6da: "Hide::steadied(half): median then mean on rim and wall", which carries this file).
- Change (`crates/ringdesign-core/src/skin.rs`): `Hide` keeps its measured per-column rim and wall (private `measured`); `Hide::steadied(self, half) -> Self` takes the cyclic median over `2 * half + 1` columns, then the same 13-column mean as before. The mean moved to a free `smooth` fn with the identical summation order, so `Hide::of` is bit-identical and `steadied(0)` equals it. Rust API only: nothing serialized, so no format fence was needed.
- Test: `skin::tests::a_steadied_hide_drops_the_end_wall_spikes`. On the test bypass (512 x 192) the rim climbs beyond one rise and fall by 0.123/0.126 mm plain and 0.043/0.050 mm at `steadied(3)`; wall 0.41 -> 0.29/0.26. It asserts plain > 0.1, steadied < 0.05 and under half, the wall improves, and `steadied(0) == of`. It fails without the median.
- CLAUDE.md: one sentence under "Paint the hide in true millimetres".
- Results: `cargo test -p ringdesign-core -- --test-threads=4`: 789 passed, 0 failed, 16 ignored (lib); 1 passed (integration); doc tests 0. Includes the golden/template tests. No new warnings. Graph crate not touched, so not run. Master's crates are unchanged since the tested base; the same commit stacked on the sculpt branch also passed (801 passed, 0 failed).
- Not done: nothing. `claude/core-hide-steadied` already held another session's sculpt commit, so this ships on `-v2`.
