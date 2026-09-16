//! The same jobs execute on a native thread and in the browser worker.
use ringdesign_core::{AlphaLibrary, BuildParams, Mesh, RingDesign, cad, manufacturing as mf};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Stage {
    #[default]
    Nominal,
    Pattern,
    AsCast,
    Finished,
}
impl Stage {
    pub const ALL: [Self; 4] = [Self::Nominal, Self::Pattern, Self::AsCast, Self::Finished];
    pub fn label(self) -> &'static str {
        match self {
            Self::Nominal => "Nominal",
            Self::Pattern => "Pattern",
            Self::AsCast => "As cast",
            Self::Finished => "Finished",
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Action {
    Inspect,
    Pattern { diagnostic: bool },
    Assembly,
    Project,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: u64,
    pub key: u64,
    pub design: RingDesign,
    pub stage: Stage,
    pub action: Action,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Shape {
    pub vertices: Vec<[f32; 3]>,
    pub faces: Vec<[u32; 3]>,
}
impl Shape {
    pub fn from_mesh(m: Mesh) -> Self {
        Self {
            vertices: m.vertices.iter().map(|v| [v.0, v.1, v.2]).collect(),
            faces: m.faces,
        }
    }
    pub fn mesh(&self) -> Mesh {
        Mesh {
            vertices: self
                .vertices
                .iter()
                .map(|v| ringdesign_core::Vec3(v[0], v[1], v[2]))
                .collect(),
            faces: self.faces.clone(),
            ..Default::default()
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub mime: String,
    pub bytes: Vec<u8>,
}
#[derive(Serialize, Deserialize)]
pub struct View {
    pub design: RingDesign,
    pub shape: Shape,
    pub report: serde_json::Value,
    pub inspection_error: Option<String>,
    pub stage: Stage,
}
#[derive(Serialize, Deserialize)]
pub enum Output {
    View(View),
    Artifact(Artifact),
}
#[derive(Serialize, Deserialize)]
pub struct Done {
    pub id: u64,
    pub key: u64,
    pub result: Result<Output, String>,
}

pub fn process(job: Job) -> Done {
    let result = execute(&job).map_err(|e| format!("{e:#}"));
    Done {
        id: job.id,
        key: job.key,
        result,
    }
}
fn execute(job: &Job) -> anyhow::Result<Output> {
    let mut d = job.design.clone();
    let builtin = AlphaLibrary::builtin();
    let lib = mf::source_library(&d, &builtin).into_owned();
    if let Some(g) = &d.graph {
        let graph = serde_json::from_value(g.clone())?;
        let mut ev = ringdesign_graph::eval::Evaluator::with_exprs(ringdesign_script::engine());
        let out = ringdesign_graph::eval::evaluate_design(
            &mut ev,
            &graph,
            &ringdesign_script::registry(),
            &lib,
            0,
        )?;
        let mut evaluated = (*out.design).clone();
        evaluated.graph = d.graph.clone();
        evaluated.manufacturing = d.manufacturing.clone();
        evaluated.casting_trials = d.casting_trials.clone();
        evaluated.name = d.name.clone();
        evaluated.embedded.extend(d.embedded.clone());
        d = evaluated;
    }
    let setup = d
        .manufacturing
        .clone()
        .unwrap_or_else(|| mf::Setup::from_design(&d));
    setup.validate()?;
    let preview = BuildParams {
        theta_steps: 160,
        profile_steps: 96,
        refine: None,
        ..Default::default()
    };
    let export = BuildParams {
        theta_steps: 768,
        profile_steps: 256,
        refine: None,
        ..Default::default()
    };
    match job.action {
        Action::Project => {
            d.embed_alphas(&mf::source_library(&d, &lib));
            Ok(Output::Artifact(Artifact {
                name: "design.ring.json".into(),
                mime: "application/json".into(),
                bytes: ringdesign_core::library::design_json(&d)?.into_bytes(),
            }))
        }
        Action::Pattern { diagnostic } => {
            let p = mf::package::files(&d, &lib, &setup, export, diagnostic)?;
            Ok(Output::Artifact(Artifact {
                name: if diagnostic {
                    "diagnostic-pattern.zip"
                } else {
                    "pattern-package.zip"
                }
                .into(),
                mime: "application/zip".into(),
                bytes: p.zip(),
            }))
        }
        Action::Assembly => {
            let p = cad::assembly::files(&d, &lib, export)?;
            Ok(Output::Artifact(Artifact {
                name: "assembly-nominal.zip".into(),
                mime: "application/zip".into(),
                bytes: p.zip(),
            }))
        }
        Action::Inspect => {
            // A multi-component assembly is viewable before choosing a casting component.
            let inspected = mf::inspect(&d, &lib, &setup, preview);
            let (report, error) = match &inspected {
                Ok(i) => (mf::package::report(&d, &setup, i, false), None),
                Err(e) => (serde_json::Value::Null, Some(format!("{e:#}"))),
            };
            let mesh = match job.stage {
                Stage::Nominal | Stage::Finished => {
                    ringdesign_core::mesh::try_build(&d, &mf::source_library(&d, &lib), preview)?
                        .mesh
                }
                Stage::Pattern => {
                    inspected
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?
                        .prepared
                        .mesh
                }
                Stage::AsCast => inspected
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                    .prepared
                    .mesh
                    .scaled(1.0 / setup.scale()),
            };
            d.manufacturing = Some(setup);
            Ok(Output::View(View {
                design: d,
                shape: Shape::from_mesh(mesh),
                report,
                inspection_error: error,
                stage: job.stage,
            }))
        }
    }
}
