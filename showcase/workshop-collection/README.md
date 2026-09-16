# Workshop collection

Four editable rings, all with an exact nominal 18.200 mm bore (approximately US 8.11). Open `index.html` for views and downloads. The renders show actual geometry; surface-finishing instructions describe the intended bench finish.

| Ring | Route | Approximate metal weight | Evidence |
|---|---|---:|---|
| [Aster](aster/design.ring.json) | Sand · one piece | 9.98 g | 0 sampled obstructions · mold trial required; screened section 1.28 mm |
| [Tide](tide/design.ring.json) | Sand · one piece | 8.30 g | 0 sampled obstructions · mold trial required; screened section 1.88 mm |
| [Lantern](lantern/design.ring.json) | Investment · fit and join | 15.06 g | Blocked in all six tested sand pulls; screened section 1.20 mm |
| [Aureole](aureole/design.ring.json) | Investment · one piece | 7.44 g | Blocked in all six tested sand pulls; screened section 1.99 mm |

The sand patterns were screened at 0.100 mm and 0.075 mm ray spacing, including both mold halves and the bore. Both remain **Review**, due to low-draft surface area; neither has a physical mold trial. Pattern compensation is applied once (1.9% starting shrink for sterling, 1.3% for 14k gold). These values require shop calibration.

Lantern is a two-component assembly. Its nominal preview mesh is for visualization; the head feet contain fitting stock. Use `component-1-pattern/` and `component-8-pattern/`, fit the contact faces, and solder or laser join. The application’s analytic interference operation is unresolved for this combination; `independent-checks.json` records the separate OpenCascade intersection measurement. Its weight is the sum before removing fitting stock. The head uses an explicit placement transform; inspect and adjust both joints when resizing this assembly.

For each ring: `design.ring.json` is the editable nominal source; `pattern-package/` contains the compensated STL/3MF, source, recipe, report, and manufacturing sheet. CAD designs also include STEP and component packages: Lantern preserves analytic surfaces; Aureole exports its faceted twisted surface (a large file). Lantern’s top-level pattern package selects the head. Never shrink-scale a pattern file again.

Reproduce with `cargo run -p ringdesign-core --example workshop_collection -- NEW_DIRECTORY`, then `-- NEW_DIRECTORY --verify`. Run `tools/check_workshop_collection.py` using a Python environment containing `cadquery-ocp`, and `tools/catalog_workshop_collection.py` to rebuild this catalog. See the root `plan.md` for application validation and remaining physical/ML work.
