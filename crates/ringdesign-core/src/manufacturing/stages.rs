//! Manufacturing stages are derived from source parameters, never repeatedly
//! scaled in place. As-cast is the recipe's ideal uniform-shrink prediction.
use super::{Casting, Setup, cast_alone, part_mesh, prepare_with_library, settle_rollback, source_library};
use crate::{AlphaLibrary, BuildParams, Mesh, RingDesign};

pub struct Stages {
    pub nominal: Mesh,
    pub pattern: Mesh,
    pub as_cast: Mesh,
    pub finished: Mesh,
    pub notes: Vec<String>,
}
pub fn evaluate(
    d: &RingDesign,
    lib: &AlphaLibrary,
    setup: &Setup,
    params: BuildParams,
) -> anyhow::Result<Stages> {
    let resolved = source_library(d, lib);
    let lib = resolved.as_ref();
    let (prepared, _) = prepare_with_library(d, lib, setup, params)?;
    let mut nominal = d.clone();
    let alone = match prepared.casting {
        Casting::Part(id) if d.band_is_procedural() => Some(id),
        _ => None,
    };
    if let Some(doc) = &mut nominal.cad {
        settle_rollback(doc);
        match alone {
            Some(id) => cast_alone(doc, id),
            None => doc.outputs = prepared.design.cad.as_ref().unwrap().outputs.clone(),
        }
    }
    let built = crate::mesh::try_build(&nominal, lib, params)?;
    let nominal = match alone {
        Some(id) => part_mesh(&built, id, &format!("#{id}"))?,
        None => built.mesh,
    };
    let as_cast = prepared.mesh.scaled(1.0 / prepared.scale);
    Ok(Stages {finished:nominal.clone(),nominal,pattern:prepared.mesh,as_cast,notes:vec![
        "Nominal and finished show the intended final dimensions and ornament".into(),
        "Pattern includes stock and shrink compensation; deferred ornament is omitted".into(),
        "As-cast predicts uniform recipe shrink while retaining finishing stock; it is not a flow or distortion simulation".into(),
    ]})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stage_dimensions_separate_allowance_from_shrink() {
        let d = RingDesign::default();
        let setup = Setup {
            radial_stock_mm: 0.2,
            axial_stock_mm: 0.1,
            ..Default::default()
        };
        let s = evaluate(
            &d,
            &AlphaLibrary::builtin(),
            &setup,
            BuildParams {
                theta_steps: 64,
                profile_steps: 64,
                refine: None,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.as_cast.volume_mm3() > s.nominal.volume_mm3());
        assert!(
            (s.pattern.volume_mm3() / s.as_cast.volume_mm3() - setup.scale().powi(3)).abs() < 1e-5
        );
        assert_eq!(s.nominal.vertices, s.finished.vertices);
    }
}
