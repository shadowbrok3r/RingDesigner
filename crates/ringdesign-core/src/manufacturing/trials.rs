//! Opt-in, local workshop observations. Calibration is a measured baseline;
//! no training data is fabricated and release checks never depend on a model.
use super::Recipe;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReleaseOutcome {
    NotTried,
    Clean,
    Dragged,
    BrokenMold,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measurement {
    pub label: String,
    pub pattern_mm: f64,
    pub as_cast_mm: f64,
    pub finished_mm: Option<f64>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub id: String,
    pub design_family: String,
    pub run_id: String,
    pub pattern_fingerprint: String,
    pub recipe: Recipe,
    pub pattern_material: String,
    pub release: ReleaseOutcome,
    pub detail_quality: u8,
    pub measurements: Vec<Measurement>,
    pub defects: String,
    pub notes: String,
}
impl Default for Trial {
    fn default() -> Self {
        Self {
            id: String::new(),
            design_family: String::new(),
            run_id: String::new(),
            pattern_fingerprint: String::new(),
            recipe: Recipe::default(),
            pattern_material: "Printed pattern".into(),
            release: ReleaseOutcome::NotTried,
            detail_quality: 0,
            measurements: vec![],
            defects: String::new(),
            notes: String::new(),
        }
    }
}
impl Trial {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.id.trim().is_empty()
                && !self.design_family.trim().is_empty()
                && !self.run_id.trim().is_empty(),
            "Trial, design family, and casting run identities are required"
        );
        self.recipe.validate()?;
        ensure!(
            self.detail_quality <= 5 && self.measurements.len() <= 100,
            "Detail rating is 0–5; at most 100 measurements"
        );
        for m in &self.measurements {
            ensure!(
                m.pattern_mm.is_finite()
                    && m.as_cast_mm.is_finite()
                    && m.pattern_mm > 0.0
                    && m.as_cast_mm > 0.0
                    && m.pattern_mm < 1000.0
                    && m.as_cast_mm < 1000.0,
                "Measurements must be positive finite millimeters"
            );
            if let Some(v) = m.finished_mm {
                ensure!(
                    v.is_finite() && v > 0.0 && v < 1000.0,
                    "Invalid finished measurement"
                );
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Calibration {
    pub model_version: &'static str,
    pub shrink_pct: f64,
    pub dimensions: usize,
    pub runs: usize,
    pub families: usize,
    pub training_mae_mm: f64,
    pub held_out_family_mae_mm: Option<f64>,
    pub held_out_dimensions: usize,
    pub finishing_loss_mm: Option<f64>,
    pub limitations: Vec<String>,
}

/// Least-squares zero-intercept scale: cast = pattern * (1 − shrink).
/// Samples are filtered by the actual process/alloy; a held-out family is never
/// allowed into its training fit. The user explicitly applies any recipe change.
pub fn calibrate(trials: &[Trial], recipe: &Recipe) -> Result<Calibration> {
    let relevant: Vec<_> = trials
        .iter()
        .filter(|t| {
            t.recipe.alloy == recipe.alloy
                && t.recipe.process == recipe.process
                && t.recipe.sand == recipe.sand
        })
        .collect();
    for t in &relevant {
        t.validate()?;
    }
    let samples: Vec<_> = relevant
        .iter()
        .flat_map(|t| {
            t.measurements
                .iter()
                .map(move |m| (t.design_family.as_str(), t.run_id.as_str(), m))
        })
        .collect();
    ensure!(
        samples.len() >= 3,
        "Record at least three measured pattern/as-cast dimension pairs for this alloy and process"
    );
    let fit = |rows: &[(&str, &str, &Measurement)]| {
        rows.iter()
            .map(|(_, _, m)| m.pattern_mm * m.as_cast_mm)
            .sum::<f64>()
            / rows
                .iter()
                .map(|(_, _, m)| m.pattern_mm.powi(2))
                .sum::<f64>()
    };
    let ratio = fit(&samples);
    let shrink_pct = (1.0 - ratio) * 100.0;
    ensure!(
        (0.0..=15.0).contains(&shrink_pct),
        "Measurements imply shrink outside 0–15%; check units and stage labels"
    );
    let families: std::collections::BTreeSet<_> = samples.iter().map(|(f, _, _)| *f).collect();
    let runs: std::collections::BTreeSet<_> = relevant
        .iter()
        .filter(|t| !t.measurements.is_empty())
        .map(|t| &t.run_id)
        .collect();
    let training_mae_mm = samples
        .iter()
        .map(|(_, _, m)| (m.pattern_mm * ratio - m.as_cast_mm).abs())
        .sum::<f64>()
        / samples.len() as f64;
    let mut errors = Vec::new();
    for family in &families {
        let held_runs: std::collections::BTreeSet<_> = samples
            .iter()
            .filter(|(f, _, _)| f == family)
            .map(|(_, r, _)| *r)
            .collect();
        let train: Vec<_> = samples
            .iter()
            .copied()
            .filter(|(f, r, _)| f != family && !held_runs.contains(r))
            .collect();
        if train.len() >= 3 {
            let r = fit(&train);
            for (_, _, m) in samples.iter().filter(|(f, _, _)| f == family) {
                errors.push((m.pattern_mm * r - m.as_cast_mm).abs());
            }
        }
    }
    let finish: Vec<_> = samples
        .iter()
        .filter_map(|(_, _, m)| m.finished_mm.map(|f| m.as_cast_mm - f))
        .collect();
    Ok(Calibration {model_version:"measured-scale-v2",shrink_pct,dimensions:samples.len(),runs:runs.len(),families:families.len(),training_mae_mm,held_out_family_mae_mm:(!errors.is_empty()).then(||errors.iter().sum::<f64>()/errors.len() as f64),held_out_dimensions:errors.len(),finishing_loss_mm:(!finish.is_empty()).then(||finish.iter().sum::<f64>()/finish.len() as f64),limitations:vec!["Measured scale baseline, not a trained neural model".into(),"Keep pattern material, sand preparation, alloy, and measurement method consistent".into(),"Finishing loss is a signed dimension change; bores can enlarge while external dimensions shrink".into(),"Held-out families also exclude their casting runs from training; a single family or run does not establish generalization".into()]})
}
pub fn csv(trials: &[Trial]) -> Result<String> {
    let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out="trial_id,design_family,run_id,pattern_fingerprint,alloy,process,sand,pattern_material,release,detail_quality,dimension,pattern_mm,as_cast_mm,finished_mm,defects,notes\n".to_string();
    for t in trials {
        t.validate()?;
        let blank = Measurement {
            label: String::new(),
            pattern_mm: 0.0,
            as_cast_mm: 0.0,
            finished_mm: None,
        };
        let rows = if t.measurements.is_empty() {
            vec![&blank]
        } else {
            t.measurements.iter().collect()
        };
        for m in rows {
            let fields = [
                t.id.clone(),
                t.design_family.clone(),
                t.run_id.clone(),
                t.pattern_fingerprint.clone(),
                t.recipe.alloy.clone(),
                format!("{:?}", t.recipe.process),
                format!("{:?}", t.recipe.sand),
                t.pattern_material.clone(),
                format!("{:?}", t.release),
                t.detail_quality.to_string(),
                m.label.clone(),
                if m.pattern_mm == 0.0 {
                    String::new()
                } else {
                    m.pattern_mm.to_string()
                },
                if m.as_cast_mm == 0.0 {
                    String::new()
                } else {
                    m.as_cast_mm.to_string()
                },
                m.finished_mm.map_or(String::new(), |v| v.to_string()),
                t.defects.clone(),
                t.notes.clone(),
            ];
            out.push_str(
                &fields
                    .iter()
                    .map(|s| quote(s))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            out.push('\n');
        }
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn measured_baseline_recovers_scale_and_holds_out_families() {
        let samples = (0..2)
            .map(|i| Trial {
                id: format!("trial{i}"),
                design_family: format!("family{i}"),
                run_id: format!("run{i}"),
                measurements: [10.0, 20.0, 30.0]
                    .map(|x| Measurement {
                        label: "Width".into(),
                        pattern_mm: x,
                        as_cast_mm: x * 0.98,
                        finished_mm: Some(x * 0.98 - 0.1),
                    })
                    .to_vec(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let c = calibrate(&samples, &Recipe::default()).unwrap();
        assert!((c.shrink_pct - 2.0).abs() < 1e-9);
        assert!(c.held_out_family_mae_mm.unwrap() < 1e-9);
        assert_eq!(c.families, 2);
        assert!(csv(&samples).unwrap().contains("family1"));
    }
    #[test]
    fn shared_casting_runs_do_not_leak_into_evaluation() {
        let samples = (0..2)
            .map(|i| Trial {
                id: format!("trial{i}"),
                design_family: format!("family{i}"),
                run_id: "same-run".into(),
                measurements: [10.0, 20.0, 30.0]
                    .map(|x| Measurement {
                        label: "Width".into(),
                        pattern_mm: x,
                        as_cast_mm: x * 0.98,
                        finished_mm: None,
                    })
                    .to_vec(),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let c = calibrate(&samples, &Recipe::default()).unwrap();
        assert!(c.held_out_family_mae_mm.is_none());
        assert_eq!(c.held_out_dimensions, 0);
    }
    #[test]
    fn no_data_produces_no_prediction() {
        assert!(calibrate(&[], &Recipe::default()).is_err());
    }
}
