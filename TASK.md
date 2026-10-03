# Platform change: CAD operations that stop failing on Gothic geometry

You are fixing, in RingDesigner's core, the CAD failures that the Tenebrae ring sessions hit (this repository; its CLAUDE.md is the design doctrine, and its "CAD parts stand on the band" section is the one that matters here). Tonight seven CAD-heavy rings are being built at once (Rosa, Arcus, Lanterna, Capsa, Porta, Sigillum, and the Officina lessons), so every failure fixed here saves several sessions a workaround. `cadkernel` is an external git dependency (`crates/ringdesign-core/Cargo.toml`): do not fork it. Fix each failure in core's own adapters (`crates/ringdesign-core/src/cad.rs`, `cad/*.rs`, `sketch*.rs`), the way `cad::twist` became our own sweep when the kernel's tessellation left open edges.

The failures, as the ring reports measured them (read the full reports with `git show origin/claude/tenebrae-oculus:cloud-report.md` and `git show origin/claude/tenebrae-ogiva:cloud-report.md`, and their examples `crates/ringdesign-core/examples/tenebrae_oculus.rs` and `tenebrae_ogiva.rs` on those branches, which carry the workarounds):
1. **Loft winding read off the first corner.** `cadkernel::brep::loft::polygon_normal` takes the first non-degenerate fan triangle, so a section starting on a concave corner gets the wrong winding and the loft fails. Normalise every section's winding by signed area, and start it on a convex corner, before the kernel sees it.
2. **Lofts through non-parallel polygon sections tessellate with open seams**: the NURBS quads are refined more finely than their planar neighbours. Make such a loft come out closed.
3. **Revolved arcs do not tessellate**: revolving any sketch arc gives "Kernel could not tessellate 1 faces" or "N nonmanifold edges", because adjacent torus faces sample their shared circle differently. Make a revolve of a sketch with arcs come out closed and manifold.
4. **A drafted extrusion refuses Béziers** (`extrude_tapered` returns `None` for anything but lines and arcs), and refuses outlines whose inset drops a segment (sharp points, and acute notches narrower than the draft offset). Accept both.
5. **Mirrored outlines in one sketch fail the drafted extrude.**
6. **Loft only runs along each section plane's normal.** Reverse the sections when `(centre[1] - centre[0]) . normal[0] > 0`, so fanned planes can be listed in either order.
7. **Ring (Brep) minus niche (Brep) gives `NoClosedForm`.** Fall back to the mesh boolean (`csg`) on the tessellated operands, as the parts resolve does.

Prioritise by how many rings each blocks: 3, 4 and 7 first. For each, write the failing test first (from the report's own geometry), then the fix.

## Setup
Work inside `/home/user/repo`, the only checkout this session can push from: `git remote add origin https://github.com/shadowbrok3r/RingDesigner.git` (or `set-url` if it exists), `git fetch origin master`, `git checkout -b claude/core-cad-robust origin/master`. Read sources from other branches with `git fetch origin <branch>` and `git show origin/<branch>:<path>`; never merge a ring branch.

Cloud VM notes: Ubuntu 24.04, 4 vCPUs, 16 GB RAM, 30 GB disk. CLAUDE.md's `systemd-run` memory guard and `--offline` flag are for the owner's workstation; here run cargo directly and let it fetch. Build only the crates you touch and what depends on your change, set `CARGO_INCREMENTAL=0`, run tests with `-- --test-threads=4`, and put any build or test that may take more than a few minutes in the background (a single command times out after 10 minutes). Keep the target directory under 22 GB.

## Rules
Keep each change as small as it can be. Every existing design, template and example must build byte for byte as before (the golden and template tests prove it): a fix may turn a refusal or a failure into a result, but where a build already produced a result, that result must not move. A new option defaults off, is skipped by serde when default, and is fenced at design format 6 / graph format 2 through `library::format_version_for` and the graph writers (`template_features_in_json`). Pin every fix with a test that fails without it, reproducing the reported failure first. Run the core tests (and the graph tests if you touch the graph crate) before pushing. Update CLAUDE.md only where the doctrine it states changes, in its own voice and briefly. The public repository never carries CrossGems internals in commit messages.

Ring sessions are building tonight and merge master at every round, so push as soon as a fix is solid, in several commits if that lands value sooner: `git push origin HEAD:claude/core-cad-robust`. End every commit message with:
Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
Claude-Session: https://claude.ai/code/session_01DhhBfxWFhPAaJd3cVZw2HV

## Finish
Write `cloud-report.md` at the repository root: what changed, per request (fixed, worked around, or not done and why), the tests and their results, and one paragraph a ring author needs in order to use it. Commit it last, push it, and make it your final message. Never push to master or any other branch, and never tag. The lead opens the pull request.
