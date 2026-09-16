# ML experiments for the workshop

The first useful implementation is local casting observations plus a measured scale-regression baseline. It is available under Casting → Shop data and `ringdesign cad calibrate design.ring.json --out observations.csv`. Applying a measured shrink value to a recipe is explicit and undoable. No workshop outcomes are invented, no data is uploaded, and no neural model ships with this change.

## Candidate experiments

| Process | Inputs | Prediction | Baseline / acceptance test |
|---|---|---|---|
| Shop calibration | Paired pattern/as-cast/finished dimensions, alloy, sand/process, pattern material, run, design family | Expected dimensional shrink and finishing change | Existing zero-intercept scale regression; compare held-out error in mm |
| Choosing a repair | Original/candidate release reports, amount of design change, actual user's choice and subsequent outcome | Rank valid corrections | Existing deterministic rank by release status, obstruction count/area, then volume change |
| Finding past designs | User-approved design descriptors or thumbnails and trial records | Similar previous designs and useful shop notes | Search/filter by family, alloy, process, dimensions before adding embeddings |

A model may propose parameters or rank candidates. The ordinary geometric checks still evaluate every candidate and control production export.

## Backend assessment

Checked 2026-09-09. These are source-level assessments; CPU latency, compiled binary size, and predictive performance have not been benchmarked on real workshop data.

| Choice | Documented capabilities | Proposed use here | Evidence still needed |
|---|---|---|---|
| Existing Rust/nalgebra baseline | Closed-form scale regression already implemented | Default shop calibration with readable coefficients | More independent casting runs and dimensional measurements |
| TensorRS 0.4.1 | Tensor operations, autodiff, linear/sequential layers, Adam; documented Rayon CPU execution and a small dependency list | Small CPU regression experiment | Training stability, target builds, inference time, incremental binary size |
| Burn | Training/inference framework, multiple compute backends through CubeCL, weight storage and ONNX tooling | More complex regression, ranking, or a future local similarity model | Same held-out benchmark, backend build cost, portable CPU fallback, device-specific measurements |

TensorRS capability statements come from its [crate documentation](https://docs.rs/tensorrs/0.4.1/tensorrs/). Burn capability statements come from the [project's backend and ecosystem documentation](https://github.com/tracel-ai/burn). A documented backend is not evidence that it is faster or more accurate for this app.

## Evaluation protocol

1. Record trial/run/family identifiers, pattern geometry fingerprint, process/material conditions, and paired measurements in millimeters. Store failed molds and undesirable finishes too.
2. Keep every near-duplicate design family out of its own evaluation fold. Also exclude casting runs shared with that family. The implemented calibration does both and reports how many dimensions could be evaluated independently.
3. Report sample/family/run counts, training error, held-out error, model version, and unsupported conditions. The current baseline reports no invented confidence interval; no held-out result is returned when independent data is insufficient.
4. Benchmark each candidate against the baseline on exactly the same folds and a fixed seed. Record inference latency, training time, target/backend, model bytes, and the release build's size increase.
5. Select a backend only after a repeatable improvement. Add it behind an optional Cargo feature, retain the baseline/manual controls, version model files and feature schemas, and verify an offline CPU path.

The remaining model experiments in `plan.md` need real observations and preferences. Synthetic test measurements verify arithmetic and data separation; they are not evidence of manufacturing accuracy.
