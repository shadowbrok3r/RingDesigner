use crate::{
    AlphaLibrary, BuildParams, Mesh, RingDesign,
    manufacturing::{self as mf, release::ReleaseReport},
};

pub struct Study {
    pub pattern: Mesh,
    pub report: ReleaseReport,
    /// The cavity walls are a display approximation of the sampled columns.
    pub upper: Vec<[[f64; 3]; 3]>,
    pub lower: Vec<[[f64; 3]; 3]>,
    pub scale: f64,
    pub investment: bool,
    /// Every part cast on its own and so not in this pattern, one line each.
    pub notes: Vec<String>,
}
pub fn build(d: &RingDesign, lib: &AlphaLibrary) -> anyhow::Result<Study> {
    let setup = d
        .manufacturing
        .clone()
        .unwrap_or_else(|| mf::Setup::from_design(d));
    let prepared = mf::prepare(
        d,
        lib,
        &setup,
        BuildParams {
            theta_steps: 192,
            profile_steps: 96,
            ..Default::default()
        },
    )?;
    let report = mf::release::analyze(&prepared.mesh, &setup)?;
    anyhow::ensure!(
        !report.columns.is_empty(),
        "No usable cavity samples; inspect the pattern in Workshop"
    );
    let upper = cavity(&report, true);
    let lower = cavity(&report, false);
    Ok(Study {
        pattern: prepared.mesh,
        report,
        upper,
        lower,
        scale: prepared.scale,
        investment: setup.recipe.process == crate::castability::CastProcess::LostWax,
        notes: prepared.notes,
    })
}
pub fn translated(p: [f64; 3], pull: [f64; 3], opening: f64, upper: bool) -> [f64; 3] {
    let offset = opening.max(0.0) * if upper { 1.0 } else { -1.0 };
    std::array::from_fn(|i| p[i] + pull[i] * offset)
}
fn cavity(report: &ReleaseReport, upper: bool) -> Vec<[[f64; 3]; 3]> {
    let [nx, ny] = report.grid;
    if nx < 2 || ny < 2 {
        return Vec::new();
    }
    let xs: Vec<usize> = (0..nx)
        .step_by((nx / 40).max(1))
        .chain(std::iter::once(nx - 1))
        .collect();
    let ys: Vec<usize> = (0..ny)
        .step_by((ny / 40).max(1))
        .chain(std::iter::once(ny - 1))
        .collect();
    let at = |x: usize, y: usize| {
        let c = &report.columns[y * nx + x];
        let z = if upper {
            c.top(report.parting_mm)
        } else {
            c.bottom(report.parting_mm)
        };
        report.frame.world([c.x, c.y, z])
    };
    let mut triangles = Vec::new();
    for yy in ys.windows(2) {
        for xx in xs.windows(2) {
            if xx[0] == xx[1] || yy[0] == yy[1] {
                continue;
            }
            let a = at(xx[0], yy[0]);
            let b = at(xx[1], yy[0]);
            let c = at(xx[1], yy[1]);
            let d = at(xx[0], yy[1]);
            triangles.push([a, b, c]);
            triangles.push([a, c, d]);
        }
    }
    triangles
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opposite_halves_follow_the_configured_pull_without_scaling_the_pattern() {
        assert_eq!(
            translated([1.0, 2.0, 3.0], [1.0, 0.0, 0.0], 4.0, true),
            [5.0, 2.0, 3.0]
        );
        assert_eq!(
            translated([1.0, 2.0, 3.0], [1.0, 0.0, 0.0], 4.0, false),
            [-3.0, 2.0, 3.0]
        );
    }
}
