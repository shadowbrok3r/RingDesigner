//! Casting criteria and mesh resolution carried through an editable design graph.

use ringdesign_core::{BuildParams, castability::{CastProcess, DraftSettings, SandProcess}};
use serde::{Serialize, de::DeserializeOwned};

use super::structs::{StructNode, enum_names};
use crate::{graph::Node, registry::{Category, EvalCtx, Inputs, NodeError, NodeSpec, Outputs, PinSpec, Registry}, value::{Value, ValueKind}};

fn wrap<T: Serialize>(value: T) -> Value {
    serde_json::to_value(value).expect("settings serialize").into()
}

fn unwrap<T: DeserializeOwned>(value: &Value) -> Option<T> {
    serde_json::from_value(value.to_json_any()?).ok()
}

fn apply(_: &mut EvalCtx<'_>, _: &Node, inputs: &Inputs) -> Result<Outputs, NodeError> {
    let Value::Design(design) = inputs.get("design") else { return Err(NodeError::input("design", "expected a design")) };
    let mut design = (**design).clone();
    if !inputs.get("build").is_null() {
        design.build = unwrap(inputs.get("build")).ok_or_else(|| NodeError::input("build", "expected mesh build settings"))?;
    }
    if !inputs.get("draft").is_null() {
        design.draft = unwrap(inputs.get("draft")).ok_or_else(|| NodeError::input("draft", "expected casting criteria"))?;
    }
    Ok(Outputs::one("design", design))
}

pub fn register(reg: &mut Registry) {
    let build = StructNode::new(
        NodeSpec::new("build.settings", "Mesh resolution", Category::Band).doc("Sweep resolution, minimum wall and optional local refinement for the saved design."),
        "build", BuildParams::default, wrap::<BuildParams>, unwrap::<BuildParams>,
    )
    .base("build", ValueKind::Json, "Settings to edit; defaults when unset.")
    .field(PinSpec::item("theta_steps", ValueKind::Int).doc("Samples around the ring."))
    .field(PinSpec::item("profile_steps", ValueKind::Int).doc("Samples around its cross-section."))
    .field(PinSpec::item("min_wall_mm", ValueKind::Number).doc("Metal retained between the displaced surface and bore, mm."))
    .field(PinSpec::item("adaptive", ValueKind::Bool).doc("Place sample lines according to surface detail."))
    .field(PinSpec::item("refine", ValueKind::Json).doc("Local refinement tolerances; absent for a fixed grid."))
    .field(PinSpec::item("soften_mm", ValueKind::Number).doc("As-cast preview smoothing radius, mm."))
    .build();
    let draft = StructNode::new(
        NodeSpec::new("draft.settings", "Casting criteria", Category::Band).doc("Casting process, sand and explicit draft, fill and detail floors."),
        "draft", DraftSettings::default, wrap::<DraftSettings>, unwrap::<DraftSettings>,
    )
    .base("draft", ValueKind::Json, "Criteria to edit; defaults when unset.")
    .field(PinSpec::select("process", enum_names(CastProcess::ALL)).doc("The casting process judged by the verdict."))
    .field(PinSpec::select("sand", enum_names(SandProcess::ALL)).doc("The named sand; absent for custom criteria."))
    .field(PinSpec::item("parting_z_mm", ValueKind::Number).doc("Manual parting height, mm."))
    .field(PinSpec::item("auto_parting", ValueKind::Bool).doc("Place the parting plane automatically."))
    .field(PinSpec::item("min_draft_deg", ValueKind::Number).doc("Minimum withdrawal draft, degrees."))
    .field(PinSpec::item("min_section_mm", ValueKind::Number).doc("Minimum section that fills, mm."))
    .field(PinSpec::item("min_detail_mm", ValueKind::Number).doc("Smallest detail the process reproduces, mm."))
    .build();
    let apply = NodeSpec::new("design.settings", "Apply build and casting settings", Category::Band)
        .doc("Carry mesh resolution and casting criteria into the design without changing its geometry fields.")
        .input(PinSpec::item("design", ValueKind::Design).doc("The design to edit."))
        .input(PinSpec::item("build", ValueKind::Json).optional().doc("Mesh build settings; keep the design's settings when unset."))
        .input(PinSpec::item("draft", ValueKind::Json).optional().doc("Casting criteria; keep the design's criteria when unset."))
        .output(PinSpec::item("design", ValueKind::Design).doc("The design with its settings."))
        .eval(apply);
    for spec in [build, draft, apply] {
        reg.register(spec).expect("unique");
    }
}
