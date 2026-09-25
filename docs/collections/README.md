# The four master collections: handoff

Written 2026-09-24 so this work can be picked up from another account with no session history.
Everything needed to continue is in this folder and the repo.

Logan asked for four new master collections, eight rings each, "the most complex, well thought
out, most insane rings we've done", reproducible as templates (File > New from template, editable
graphs people can learn from, like the Reptilia collection and the Stock masterworks). He also asked
for the starter gallery to be rebuilt, the Workshop collection replaced, and templates to open
without stalling the app.

| File | What it holds |
| --- | --- |
| `bestiarium.md` | Dark mythology bestiary: the seven rings not yet started (Draco and Arachne are in progress) |
| `cataphracta.md` | Reptilia II: all eight rings |
| `tenebrae.md` | Gothic cathedral: all eight rings |
| `vepres.md` | Thorned botanical: all eight rings |
| `starters-and-officina.md` | Starter bands, stone settings, the 20 factory signets, Officina (replaces Workshop), and platform enablers P5–P8 |
| `source/plan.md` | The design director's plan: verified facts, verdicts per ring, base allocation, enablers, build order, quality loop, and Logan's answers (section 7, binding) |
| `source/brief-*.md` | The five original design briefs, with every API cited by file and line |

Where a collection file and a brief disagree, the collection file wins: it already folds in the
director's changes and Logan's answers.

## Logan's decisions (binding)

- **Themes:** all four, one collection each: **Bestiarium** (dark mythology, closest to his
  business, Kings of Alchemy), **Cataphracta** (Reptilia II), **Tenebrae** (Gothic),
  **Vepres** (thorned botanical). Each shows a different side of the app: sculpted hide and
  CAD talons; height-field mastery in sand; CAD modelling as lessons; sweeps along paths.
- **Count:** eight per collection. The Bestiarium authors nine (both Harpyia and Arachne) and
  drops the weakest after the first render round. He may trim the count after the first collection.
- **Process:** a mix. Each ring is judged against its own `DraftSettings::process`.
- **Upright factory plans are lost wax only.** Plans 004, 008, 009, 010, 011, 014, 018, 019, 020
  are asymmetric across the parting plane and the sand master mirrors the upper half, so only
  001, 002, 003, 005, 006, 007, 012, 013, 015, 016, 017 pour in sand.
- **Sands:** Delft clay and Petrobond. Moloch and Ouroborus keep Petrobond's rules (0.40 mm
  detail, 2.5° draft); everything else is Delft (0.30 mm, 3°).
- **Harpyia** returns with lofted feathers, not flat region tiers.
- **Starter signets:** all 20 factory bases, bare. Heart and waved-hexagon signets leave the menu;
  Shouldered cushion stays as the one parametric signet.
- **Renders:** studio gold, stones set. Sand rings export as the pattern (no cuts, raised
  drill-start dots).

His taste, from many sessions: one theme face to palm (Palisade was retired as "random elements
placed in various spots"); figurative motifs are stamps with true outlines (painted moons "look
like arrows"); stones in made settings, never height-field prongs ("quite awful"); no faces, no
snake eyes; signets start from factory stock, which has the hard angles where wall meets face;
reptiles land, the celestial Zenith did not. His own ZBrush rings are in
`/home/shadowbroker/jewelry-scan/RING/` (winged bands, honeycomb-scale signet faces, knotwork and
scrollwork shanks, skulls and deity heads). The bar is Caiman
(`showcase/stock-masterworks/caiman/hero.png`) and the Reptilia sheet
(`showcase/reptilia/renders/Reptilia-collection.png`).

## State at handoff

| Item | State |
| --- | --- |
| Batches 1–14 | On `origin/master` at `8a1db4d`, CI green |
| Batch 15 (platform I) | Merged into local `master` (merge commits `58f6ad8`, `5fd9390`, `537451a`, `d3dfa34`, `a8e3425` plus follow-ups; the integrator was writing its PLAN.md block at handoff time). Lanes: `b15-open-speed` (template opening hardened, per-build artwork re-bake removed, responsiveness audit), `b15-cad-stones` (P2), `b15-skin` (P3: `core/skin.rs`, `imported_base::sand_master`, and the prickle, parting-stamp, side-gate and stock spikes), `b15-stamps` (P4: tiers, `StampTop`, `core/outline.rs`, `parting_monotone`, `stamp_row`), `b15-leftovers` |
| Draco | Authored on branch `bestiarium-draco` (on `b15-skin`), first build done, in art-director review |
| Arachne | Authored on branch `bestiarium-arachne` (on `8a1db4d`), scored 5.5/10 in round 1, being revised (jointed legs, a pedicel, spinnerets) |
| Everything else | Not started |

Check before resuming:
1. `git log --oneline master` for "Merge b15-…" commits and a PLAN.md **Batch 15** block. If they
   are missing, merge the `b15-*` branches in the order leftovers, stamps, skin, cad-stones,
   open-speed, then run every suite (below).
2. `git branch --list 'bestiarium-*'` and `showcase/bestiarium/<slug>/` in their worktrees
   (`git worktree list`). Finish their review rounds before starting new rings.
3. Push `master` only when every suite is green.

## Open decisions for Logan

The collection files were checked against the batch 15 probes, which changed four plans:

1. **Chelonia (factory 007) and Phrynosoma (factory 016) fail in sand before any ornament.** The
   bare 007 sand master reads NotCastable (14.7% at −64°) and 016 reads 2.9% at −7.6°; the sand
   envelope cannot rescue either. Recommended: keep them in sand as procedural lofted heads on the
   bundled "CG Quatrefoil" and "CG Star" outlines. Alternative: the real stock in lost wax (the
   collection then pours seven in sand). `cataphracta.md` step 0 re-measures both at their own size.
2. **Chamaeleo's 17 × 13 face is outside the stock's resize range** (70–130% of the master); the
   file uses 17 × 14.5, and the bare 001 is borderline (0.10% at −1.6°), measured first.
3. **Viscum cannot pour in sand on 003 Clover** (the envelope needs 4.2 mm). Recommended: lost wax
   on the native 003, keeping the factory lobes (Vepres then pours three in sand, five in wax).
   Alternative: a procedural clover signet in sand.
4. **Skulls and gargoyles.** The writers read "no faces or eyes" strictly: Capsa's skull became
   crossed bones under an hourglass, and Arcus's gargoyle a plain silhouette. Logan's rule came
   from the reptile rings (no snake faces or eyes) and his own business rings are full of skulls,
   so ask whether a skull is wanted in the reliquary.

Also open in `tenebrae.md`: the lid hinge (a real bench hinge or cast shut), the alloys, and
Oculus's weight (about 18 g of silver).

## Build order from here

1. **Finish Draco and Arachne** (at most two more review rounds each).
2. **Batch 16, platform II and the starters** (`starters-and-officina.md`): P6 claw styles, P7
   template nodes and lift (stock templates drop from ~13 MB to under 1 MB), P5 station-aware
   side-face gates, P8 collection tooling, the starter gallery part 1. Parallel lanes with
   disjoint files.
3. **The rest of the Bestiarium.** Buildable once batch 15 is merged: Corvus, Basiliscus,
   Manticora, Harpyia, Phoenix (with its keys adjusted or after P5). Fenrir and Kraken wait for
   P6 claw styles, and Kraken also for C-B1 curve widths.
4. **Package the Bestiarium, then stop for Logan's review.** Drop the weakest of the nine.
5. **Cataphracta, then Tenebrae, then Vepres**, each with its own enablers first.

## How one ring is built

One lane per ring, in its own git worktree, on its own branch, with the ring in ONE example file:
`crates/ringdesign-core/examples/<collection>_<slug>.rs`, plus
`crates/ringdesign-core/examples/<collection>/art/<slug>/` for artwork. No `src/` edits from a ring
lane: a needed core change goes to an enabler lane.

The example takes `--draft` (768 × 320), a default export build (1536 × 448) and `--verify` (a cold
reload with an empty library gives identical vertices). It writes `showcase/<collection>/<slug>/`:
`design.ring.json`, `report.json`, and studio-gold renders with stones set (hero, face, palm,
side, a stone close-up, bare stock against finished).

**Gates (all must pass before review):**
- watertight, 0 degenerate faces; `csg::self_crossings` 0 on every made part;
  `built.solids.notes` empty;
- the field verdict against the ring's own process. Sand: **Castable**, not "with care"; ray
  release 0 obstructions and 0 unresolved at 0.100 and 0.075 mm; draft-clamp bite ≤ 0.05 mm.
  Lost wax: fill ≥ 0.8 section on the investment recipe;
- `dfm::findings_in` returns **0** findings (Caiman had 2, so this bar is higher than Caiman);
- the stones report's count equals the gem preview's.

**Review loop, at most three rounds.** A fresh reviewer scores the renders against Caiman, the
Reptilia sheet and the ZBrush rings with this checklist: one theme face to palm; motifs as stamps
with true outlines; no faces or eyes; stones in made settings and visible; shoulder ornament not
cut off toward the face; no flat or blocky CAD; factory stock keeps its hard angles; a silhouette
distinct from the other rings; reads as its subject at a glance; density and legibility at least
Caiman's. Ship at ≥ 7.5/10 with every gate green. A ring still failing after round three is cut,
not shipped weak.

**Then the template gate.** The lift (`ringdesign_graph::lift`, `Graph::from_design`) or an
authored builder evaluates cold to the same design byte for byte, with at most four `design.set`
patches; record its size and its open time per phase (`template_open_probe`).

**Packaging, per collection** (the Reptilia format, `showcase/reptilia/README.md` is the model):
per-ring folders (design, editable graph, artwork, finished-metal STL, casting-pattern STL,
reference stone STL, reports, renders, reel); a collection README with reproduce commands under
the memory guard; `index.html`; the collection sheet; graph templates in
`graphs/templates/<slug>-<collection>.graph.json` (bundled by `ringdesign-assets/build.rs`); a
static per collection in `ringdesign-graph/src/templates.rs`; a menu group in
`ringdesign-workbench/src/templates.rs` with 160 px thumbnails; the preview test count updated.

## Working rules for this repo

- Tests always run under the memory guard, after an unguarded build of the same `-p` set:
  `cargo test --offline -p <crate> --no-run`, then
  `systemd-run --user --scope -p MemoryMax=4G --quiet -- cargo test --offline -p <crate> -- --test-threads=4`
  (`ringdesign-gui` with `--test-threads=1`; the graph suite with `--no-fail-fast`). Never edit
  sources between the two. The machine has no swap.
- Heavy ring builds run in release under `systemd-run --user --scope -p MemoryMax=8G --quiet --`.
- Zero warnings (`cargo check --offline --workspace --all-targets`). Also: the phone
  (`cargo test --offline -p ringdesigner_android`, and from `crates/ringdesigner-android` with
  `ANDROID_NDK_HOME=~/Android/Sdk/ndk/28.0.12674087`: `cargo ndk -t arm64-v8a check -p ringdesigner_android`),
  and wasm when core changes
  (`cargo check --offline -p ringdesign-core -p ringdesign-configurator --no-default-features --target wasm32-unknown-unknown`).
- Comments: one line, impersonal, what the code does; no rationale, no plan references.
- New serde fields default so saved designs stay byte-identical. Geometry an older build would
  ignore is fenced: `library::format_version_for` returns 6 (unreleased; desktop 0.6.0 reads up to 5).
- Commit messages end with a `Co-Authored-By:` line for the model doing the work.
- Never publish (app store, releases, tags) or bump versions unless Logan asks. Push `master` after
  each batch is merged and green.
- Emulators: the AVD named `s26ultra` belongs to other apps, never touch it. `rdsmoke` is this
  project's throwaway AVD; its serial moves (emulator-5554 when s26ultra is off), so check
  `adb -s <serial> emu avd name` first. Start emulators through the ARTEMIS tool
  `mobile_diagnose(launch_avd="rdsmoke")`, never `emulator -avd` in a shell.
- Driving the desktop app: `cargo build --offline --features eframe/inspection --bin ringdesigner`,
  back up `~/.local/share/ringdesignerdesktop/app.ron` first and restore it after, run with
  `EGUI_INSPECTION=1 DISPLAY=:0 XAUTHORITY=$(ls /run/user/1000/xauth_* | head -1)`, drive with
  `python3 tools/egui_drive.py`, close with `xdotool windowactivate --sync <win> key --clearmodifiers alt+F4`.

## Prompt templates

These are the prompts the ring lanes and reviewers ran with. Fill in the angle brackets.

**Ring author:**

> You are authoring one ring of the <Collection>, a new master collection for RingDesigner (repo
> path; the CLAUDE.md casting doctrine applies). Read docs/collections/<collection>.md (your ring's
> section) and docs/collections/source/plan.md sections 0, 1, 5 and 7. Logan's taste: one theme face
> to palm; figurative motifs as stamps with true outlines; no faces or eyes; stones in made settings;
> no flat, blocky CAD; stand beside Caiman and his ZBrush rings. Work in your own worktree on branch
> <collection>-<slug> at <commit>. Put the ring in ONE example file
> crates/ringdesign-core/examples/<collection>_<slug>.rs (artwork under
> examples/<collection>/art/<slug>/); no src/ edits (write needed core changes in your report as
> exact code). Build unguarded in release, run under systemd-run --user --scope -p MemoryMax=8G.
> The example takes --draft, a default export build and --verify, and writes
> showcase/<collection>/<slug>/ (design.ring.json, report.json, studio-gold renders with stones set:
> hero, face, palm, side, stone close-up, bare against finished). Pass every gate (list above)
> before handing over. Report: branch, HEAD, worktree path, render paths, every gate's numbers, what
> each layer/part/stamp is and why, what you could not do, core changes you would want.

**Art-director review** (returns verdict ship / revise / cut, a score out of 10 with Caiman at 7,
and a numbered punch list):

> You are a demanding jewellery art director reviewing one ring for Logan (Kings of Alchemy,
> dark-mythology cast rings). Look at every render the author lists, plus Caiman, the Reptilia
> sheet and Logan's ZBrush rings. Score it against the checklist above and check the gate numbers
> (any failed gate is an automatic revise). Ship only at ≥ 7.5 with every gate green; cut if it
> cannot get there. The punch list must be concrete: what to change, where, by how much.

**Reviser:** the author prompt again, plus "continue in the same worktree", the previous report,
the score and the punch list to apply, then rebuild, rerun every gate and render, commit, report.
