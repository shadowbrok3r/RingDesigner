# In flight at handoff (2026-09-24, late)

Unfinished work, in order. Read README.md here for everything else.

1. **Batch 15 merge (local master, NOT pushed).** All five b15-* lanes merged in the main checkout with follow-up fixes; the integrator was writing PLAN.md "Batch 15" + CLAUDE.md. To finish: in the main checkout, `git status` (commit or finish the CLAUDE.md/PLAN.md edits), run every suite (script: see README "Working rules"; gui --test-threads=1, graph --no-fail-fast), NDK + wasm + `--locked` checks, then `git push origin master`. Remove worktrees `.claude/worktrees/wf_87572ce7-9a0-*` and delete merged `b15-*` branches.
2. **Arachne** — branch `bestiarium-arachne`, worktree `.claude/worktrees/wf_9f675e59-c4d-1`. Round 1 scored 5.5/10 (stiff straight legs, abdomen detached); round 2 in progress (jointed legs, pedicel, spinnerets). Finish the review loop (README), then merge.
3. **Draco** — branch `bestiarium-draco`, worktree `.claude/worktrees/wf_9f675e59-c4d-2`, built on b15-skin. First build done, awaiting review. Faces are now allowed: add a real wyvern head at the palm. Rebase onto master after batch 15 (stamp literals need `tier`/`top`).
4. **Merge this branch** (`collections-handoff`) into master (docs only).
5. **Batch 16**, then the rest of the Bestiarium, per README "Build order".
6. **Logan still to confirm:** the test APK on his S26 (Samsung keyboard: double-tap delete, symbols, decimal key), the LGPL written offer, whether CI embeds the OpenCascade worker, and Tenebrae's lid hinge, alloys and Oculus weight.

**Update:** the batch 15 workflow finished. Local master HEAD: `9992ffa PLAN: batch 15 status; CLAUDE.md: the skin and sand master in core, stamps v2, CAD stones in the record and render::finished, what an open costs` (0 uncommitted files). Its full report, including the reviewers' still-open findings (worker evaluating against a stale library, shared test counters, the phone re-arranging opened graphs, detail findings still measured on the UI thread, and a moved head counted twice as a stone), is in `batch15-report.json` here. Check that the integrator fixed each one before pushing master.

**Update:** the Arachne/Draco workflow finished; neither shipped (Arachne 6.2, Draco last scored 4.5). Their final punch lists are in `rings-report.json` here. Arachne also needs: leg IV kept out of the finger hole (0.36 mm intrusion at 136 deg), a bore-clearance gate, and the lift at 4 or fewer patches (needs P7).
