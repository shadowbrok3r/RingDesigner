//! One typed CAD edit, applied by `Document::apply` here and by the graph crate's `nodes::cad::apply_edit`.
use super::{Attach, Component, Document, Feature, Operation, Placement, Stage};
use crate::{RingDesign, sketch::Id};
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// An edit to a CAD document by feature id; refused if it would leave a dangling reference, a cycle or two shanks.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CadEdit {
    /// Insert after `after`, or at the end; an id of 0 asks for a fresh one.
    Add { feature: Feature, after: Option<Id> },
    /// Drop a feature nothing depends on.
    Remove { id: Id },
    /// Reorder to sit after `after`, or first.
    Move { id: Id, after: Option<Id> },
    Enable { id: Id, enabled: bool },
    Rename { id: Id, name: String },
    Operation { id: Id, operation: Operation },
    Component { id: Id, component: Component },
    Placement { id: Id, placement: Placement },
    Attach { id: Id, attach: Attach },
    Stage { id: Id, stage: Stage },
    Blend { id: Id, blend_mm: f64 },
    Outputs { outputs: Vec<Id> },
    Through { through: Option<Id> },
}
impl PartialEq for CadEdit {
    // Compares serialized forms.
    fn eq(&self, other: &Self) -> bool {
        serde_json::to_value(self).ok() == serde_json::to_value(other).ok()
    }
}
impl CadEdit {
    /// The history label with the target named by id only, e.g. "Remove #3".
    pub fn label(&self) -> String {
        self.named(|id| format!("#{id}"))
    }
    /// The history label with the target named by its feature name, e.g. "Remove Cylinder".
    pub fn label_in(&self, doc: &Document) -> String {
        self.named(|id| doc.feature(id).map_or_else(|| format!("#{id}"), |f| f.name.clone()))
    }
    fn named(&self, name: impl Fn(Id) -> String) -> String {
        match self {
            Self::Add { feature, .. } => {
                let what = if feature.name.trim().is_empty() { feature.operation.label() } else { feature.name.as_str() };
                format!("Add {what}")
            }
            Self::Remove { id } => format!("Remove {}", name(*id)),
            Self::Move { id, .. } => format!("Move {}", name(*id)),
            Self::Enable { id, enabled: true } => format!("Enable {}", name(*id)),
            Self::Enable { id, enabled: false } => format!("Suppress {}", name(*id)),
            Self::Rename { id, .. } => format!("Rename {}", name(*id)),
            Self::Operation { id, .. } => format!("Edit {}", name(*id)),
            Self::Component { id, .. } => format!("Component of {}", name(*id)),
            Self::Placement { id, .. } => format!("Place {}", name(*id)),
            Self::Attach { id, attach } => match attach {
                Attach::Separate => format!("Separate {}", name(*id)),
                Attach::Join => format!("Join {}", name(*id)),
                Attach::Cut => format!("Cut {}", name(*id)),
            },
            Self::Stage { id, stage } => match stage {
                Stage::Cast => format!("Cast {}", name(*id)),
                Stage::Bench => format!("Bench {}", name(*id)),
            },
            Self::Blend { id, .. } => format!("Blend {}", name(*id)),
            Self::Outputs { .. } => "Outputs".into(),
            Self::Through { through: Some(id) } => format!("Roll back to {}", name(*id)),
            Self::Through { through: None } => "Roll back off".into(),
        }
    }
    /// The feature the edit acts on; `None` for an addition and the document-wide edits.
    pub fn target(&self) -> Option<Id> {
        match self {
            Self::Add { .. } | Self::Outputs { .. } => None,
            Self::Through { through } => *through,
            Self::Remove { id }
            | Self::Move { id, .. }
            | Self::Enable { id, .. }
            | Self::Rename { id, .. }
            | Self::Operation { id, .. }
            | Self::Component { id, .. }
            | Self::Placement { id, .. }
            | Self::Attach { id, .. }
            | Self::Stage { id, .. }
            | Self::Blend { id, .. } => Some(*id),
        }
    }
}
/// What an applied edit reports back.
#[derive(Clone, Debug, PartialEq)]
pub struct Applied {
    /// The id an `Add` allocated.
    pub id: Option<Id>,
    /// The history label, with the target named.
    pub label: String,
}
fn is_sketch(f: &Feature) -> bool {
    matches!(f.operation, Operation::Sketch { .. })
}
/// Refuses a placement the build could not seat: any value that is not a finite number.
fn check_placement(who: &str, p: &Placement) -> Result<()> {
    if let Placement::Ring { theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg } = p {
        ensure!(
            [theta_deg, across_mm, height_mm, spin_deg, tilt_deg, cant_deg].iter().all(|v| v.is_finite()),
            "{who}: a placement needs finite numbers"
        );
    }
    Ok(())
}
fn check_blend(who: &str, blend_mm: f64) -> Result<()> {
    ensure!(blend_mm.is_finite() && blend_mm >= 0.0, "{who}: blend radius must be finite and not negative");
    Ok(())
}
fn check_component(who: &str, c: &Component) -> Result<()> {
    check_blend(who, c.blend_mm)?;
    check_placement(who, &c.placement)
}
fn distinct(ids: Vec<Id>) -> Vec<Id> {
    let mut out: Vec<Id> = Vec::new();
    for id in ids {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}
impl Document {
    /// One past the highest id in use; never 0.
    pub fn fresh_id(&self) -> Id {
        self.features.iter().map(|f| f.id).max().unwrap_or(0) + 1
    }
    pub fn feature(&self, id: Id) -> Option<&Feature> {
        self.features.iter().find(|f| f.id == id)
    }
    pub fn feature_mut(&mut self, id: Id) -> Option<&mut Feature> {
        self.features.iter_mut().find(|f| f.id == id)
    }
    pub fn position(&self, id: Id) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }
    /// The features this one reads, each once, in the order its operation names them.
    pub fn sources_of(&self, id: Id) -> Vec<Id> {
        self.feature(id).map(|f| distinct(f.operation.sources())).unwrap_or_default()
    }
    /// Every feature that reads this one, directly or through another, in document order.
    pub fn dependents(&self, id: Id) -> Vec<Id> {
        let mut set = vec![id];
        loop {
            let before = set.len();
            for f in &self.features {
                if !set.contains(&f.id) && f.operation.sources().iter().any(|s| set.contains(s)) {
                    set.push(f.id);
                }
            }
            if set.len() == before {
                break;
            }
        }
        self.features.iter().map(|f| f.id).filter(|f| *f != id && set.contains(f)).collect()
    }
    fn who(&self, id: Id) -> String {
        self.feature(id).map_or_else(|| format!("#{id}"), |f| format!("#{id} {}", f.name))
    }
    fn known(&self, id: Id) -> Result<usize> {
        self.position(id).ok_or_else(|| anyhow::anyhow!("No feature #{id} in the document"))
    }
    /// Refuses a second enabled procedural shank beside `except`.
    fn one_band(&self, f: &Feature, except: Id) -> Result<()> {
        if f.enabled && matches!(f.operation, Operation::Band) {
            if let Some(band) = self.band().filter(|b| *b != except) {
                bail!("A document carries one procedural shank; {} is already it", self.who(band));
            }
        }
        Ok(())
    }
    /// Puts a source back in the outputs when no feature consumes it any more.
    fn release(&mut self, source: Id) {
        let consumed = self.features.iter().any(|g| g.operation.consumes().contains(&source));
        let body = self.feature(source).is_some_and(|f| !is_sketch(f));
        if !consumed && body && !self.outputs.contains(&source) {
            self.outputs.push(source);
        }
    }
    /// Applies one edit, or refuses it with the features named and the document untouched.
    pub fn apply(&mut self, edit: &CadEdit) -> Result<Applied> {
        let label = edit.label_in(self);
        let id = match edit {
            CadEdit::Add { feature, after } => Some(self.add(feature.clone(), *after)?),
            CadEdit::Remove { id } => {
                self.remove(*id)?;
                None
            }
            CadEdit::Move { id, after } => {
                self.reorder(*id, *after)?;
                None
            }
            CadEdit::Enable { id, enabled } => {
                self.known(*id)?;
                let mut f = self.feature(*id).unwrap().clone();
                f.enabled = *enabled;
                self.one_band(&f, *id)?;
                self.feature_mut(*id).unwrap().enabled = *enabled;
                None
            }
            CadEdit::Rename { id, name } => {
                self.known(*id)?;
                ensure!(!name.trim().is_empty(), "A feature needs a name");
                self.feature_mut(*id).unwrap().name = name.clone();
                None
            }
            CadEdit::Operation { id, operation } => {
                self.set_operation(*id, operation.clone())?;
                None
            }
            CadEdit::Component { id, component } => {
                self.known(*id)?;
                check_component(&self.who(*id), component)?;
                self.feature_mut(*id).unwrap().component = component.clone();
                None
            }
            CadEdit::Placement { id, placement } => {
                self.known(*id)?;
                check_placement(&self.who(*id), placement)?;
                self.feature_mut(*id).unwrap().component.placement = placement.clone();
                None
            }
            CadEdit::Attach { id, attach } => {
                self.known(*id)?;
                self.feature_mut(*id).unwrap().component.attach = *attach;
                None
            }
            CadEdit::Stage { id, stage } => {
                self.known(*id)?;
                self.feature_mut(*id).unwrap().component.stage = *stage;
                None
            }
            CadEdit::Blend { id, blend_mm } => {
                self.known(*id)?;
                check_blend(&self.who(*id), *blend_mm)?;
                self.feature_mut(*id).unwrap().component.blend_mm = *blend_mm;
                None
            }
            CadEdit::Outputs { outputs } => {
                for (i, id) in outputs.iter().enumerate() {
                    self.known(*id)?;
                    ensure!(!outputs[..i].contains(id), "Outputs list {} twice", self.who(*id));
                    ensure!(!is_sketch(self.feature(*id).unwrap()), "{} is a sketch and has no body to output", self.who(*id));
                }
                self.outputs = outputs.clone();
                None
            }
            CadEdit::Through { through } => {
                if let Some(id) = through {
                    self.known(*id)?;
                }
                self.through = *through;
                None
            }
        };
        Ok(Applied { id, label })
    }
    fn add(&mut self, mut f: Feature, after: Option<Id>) -> Result<Id> {
        ensure!(self.features.len() < 256, "At most 256 CAD features");
        if f.id == 0 {
            f.id = self.fresh_id();
        }
        ensure!(self.feature(f.id).is_none(), "Duplicate feature identity #{}", f.id);
        let at = match after {
            Some(a) => self.known(a).map_err(|_| anyhow::anyhow!("Add after #{a}: no such feature"))? + 1,
            None => self.features.len(),
        };
        let sources = distinct(f.operation.sources());
        for s in &sources {
            let p = self.position(*s).ok_or_else(|| anyhow::anyhow!("Add {}: source #{s} is not in the document", f.name))?;
            ensure!(p < at, "Add {}: its source {} would come after it", f.name, self.who(*s));
        }
        check_component(&format!("Add {}", f.name), &f.component)?;
        self.one_band(&f, f.id)?;
        for s in f.operation.consumes() {
            self.outputs.retain(|v| *v != s);
        }
        if !is_sketch(&f) {
            self.outputs.push(f.id);
        }
        let id = f.id;
        self.features.insert(at, f);
        Ok(id)
    }
    fn remove(&mut self, id: Id) -> Result<()> {
        let pos = self.known(id)?;
        let dependents = self.dependents(id);
        ensure!(
            dependents.is_empty(),
            "Remove {}: {} depend{} on it: {}",
            self.who(id),
            dependents.len(),
            if dependents.len() == 1 { "s" } else { "" },
            dependents.iter().map(|d| self.who(*d)).collect::<Vec<_>>().join(", ")
        );
        let sources = self.sources_of(id);
        self.features.remove(pos);
        self.outputs.retain(|v| *v != id);
        self.joints.retain(|j| j.a != id && j.b != id);
        if self.through == Some(id) {
            self.through = pos.checked_sub(1).map(|p| self.features[p].id);
        }
        for s in sources {
            self.release(s);
        }
        Ok(())
    }
    /// Removes a feature and everything that reads it, dependents first, or nothing; the ids removed.
    pub fn remove_with_dependents(&mut self, id: Id) -> Result<Vec<Id>> {
        self.known(id)?;
        let mut removed = self.dependents(id);
        removed.reverse();
        removed.push(id);
        let mut next = self.clone();
        for r in &removed {
            next.remove(*r)?;
        }
        *self = next;
        Ok(removed)
    }
    fn reorder(&mut self, id: Id, after: Option<Id>) -> Result<()> {
        let pos = self.known(id)?;
        if let Some(a) = after {
            ensure!(a != id, "Move {}: after itself", self.who(id));
            self.known(a)?;
        }
        let mut order: Vec<Id> = self.features.iter().map(|f| f.id).collect();
        order.remove(pos);
        let at = match after {
            Some(a) => order.iter().position(|v| *v == a).unwrap() + 1,
            None => 0,
        };
        order.insert(at, id);
        let index = |v: Id| order.iter().position(|o| *o == v).unwrap();
        for s in self.sources_of(id) {
            ensure!(index(s) < at, "Move {}: it would come before its source {}", self.who(id), self.who(s));
        }
        for d in self.dependents(id) {
            ensure!(index(d) > at, "Move {}: it would come after {}, which depends on it", self.who(id), self.who(d));
        }
        let f = self.features.remove(pos);
        self.features.insert(at, f);
        Ok(())
    }
    fn set_operation(&mut self, id: Id, operation: Operation) -> Result<()> {
        let pos = self.known(id)?;
        let dependents = self.dependents(id);
        let sources = distinct(operation.sources());
        for s in &sources {
            ensure!(*s != id, "{}: a feature cannot read itself", self.who(id));
            let p = self.position(*s).ok_or_else(|| anyhow::anyhow!("{}: source #{s} is not in the document", self.who(id)))?;
            ensure!(
                !dependents.contains(s),
                "{}: reading {} would form a cycle, it already depends on this feature",
                self.who(id),
                self.who(*s)
            );
            ensure!(p < pos, "{}: source {} comes after it; move it first", self.who(id), self.who(*s));
        }
        let mut f = self.feature(id).unwrap().clone();
        let was_sketch = is_sketch(&f);
        let old_sources = self.sources_of(id);
        f.operation = operation;
        self.one_band(&f, id)?;
        let now_sketch = is_sketch(&f);
        let consumed = f.operation.consumes();
        *self.feature_mut(id).unwrap() = f;
        for s in &consumed {
            self.outputs.retain(|v| v != s);
        }
        for s in old_sources {
            if !consumed.contains(&s) {
                self.release(s);
            }
        }
        if was_sketch && !now_sketch && !self.outputs.contains(&id) {
            self.outputs.push(id);
        }
        if now_sketch {
            self.outputs.retain(|v| *v != id);
        }
        Ok(())
    }
}
impl RingDesign {
    /// Edits a plain design's document, created on an `Add` and dropped once empty; refuses a graph-driven design.
    pub fn apply_cad_edit(&mut self, edit: &CadEdit) -> Result<Applied> {
        ensure!(self.graph.is_none(), "This design is driven by its graph; edit the graph");
        let had = self.cad.is_some();
        ensure!(had || matches!(edit, CadEdit::Add { .. }), "No CAD features to edit");
        let result = self.cad.get_or_insert_with(Document::default).apply(edit);
        let empty = self.cad.as_ref().is_some_and(|d| d.features.is_empty());
        if (result.is_ok() && empty) || (result.is_err() && !had) {
            self.cad = None;
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cad::{Boolean, ComponentRole, evaluate, examples};
    use crate::{AlphaLibrary, BuildParams};

    fn feature(id: Id, name: &str, operation: Operation) -> Feature {
        Feature { id, name: name.into(), enabled: true, operation, component: Component::default() }
    }
    fn cylinder(name: &str) -> Feature {
        feature(0, name, Operation::Cylinder { radius_mm: 1.5, height_mm: 2.0 })
    }
    fn params() -> BuildParams {
        BuildParams { theta_steps: 128, profile_steps: 64, refine: None, ..Default::default() }
    }
    fn evaluable(d: &RingDesign) -> Result<()> {
        evaluate(d, &AlphaLibrary::builtin(), params()).map(|_| ())
    }
    fn json(doc: &Document) -> serde_json::Value {
        serde_json::to_value(doc).unwrap()
    }
    /// The band with a joined cylinder standing on it.
    fn band_and_cylinder() -> RingDesign {
        let mut d = RingDesign::default();
        let mut doc = Document::default();
        let mut band = feature(1, "Shank", Operation::Band);
        band.component.role = ComponentRole::Shank;
        doc.append(band).unwrap();
        let mut c = feature(2, "Bezel", Operation::Cylinder { radius_mm: 2.0, height_mm: 2.5 });
        c.component.placement = Placement::ring(90.0, 0.0);
        c.component.attach = Attach::Join;
        doc.append(c).unwrap();
        d.cad = Some(doc);
        d
    }

    #[test]
    fn fresh_ids_start_at_one_and_an_add_lands_where_it_is_asked() {
        let mut doc = Document::default();
        assert_eq!(doc.fresh_id(), 1);
        let a = doc.apply(&CadEdit::Add { feature: cylinder("A"), after: None }).unwrap();
        assert_eq!(a.id, Some(1));
        assert_eq!(a.label, "Add A");
        let b = doc.apply(&CadEdit::Add { feature: cylinder("B"), after: None }).unwrap();
        assert_eq!(b.id, Some(2));
        let c = doc.apply(&CadEdit::Add { feature: cylinder("C"), after: Some(1) }).unwrap();
        assert_eq!(c.id, Some(3));
        assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![1, 3, 2]);
        assert_eq!(doc.outputs, vec![1, 2, 3]);
        assert_eq!(doc.position(3), Some(1));
        let e = doc.apply(&CadEdit::Add { feature: cylinder("D"), after: Some(9) }).unwrap_err();
        assert!(e.to_string().contains("no such feature"), "{e}");
        let mut dup = cylinder("E");
        dup.id = 2;
        assert!(doc.apply(&CadEdit::Add { feature: dup, after: None }).unwrap_err().to_string().contains("Duplicate"));
        // A source must already precede the insertion point.
        let mut t = feature(0, "T", Operation::Transform { source: 2, translation: [0.0; 3], rotation_deg: [0.0; 3] });
        let e = doc.apply(&CadEdit::Add { feature: t.clone(), after: Some(1) }).unwrap_err();
        assert!(e.to_string().contains("would come after it"), "{e}");
        t.id = 0;
        let a = doc.apply(&CadEdit::Add { feature: t, after: None }).unwrap();
        assert_eq!(a.id, Some(4));
        assert_eq!(doc.outputs, vec![1, 3, 4], "the transform's source leaves the outputs");
        assert_eq!(doc.sources_of(4), vec![2]);
        assert_eq!(doc.dependents(2), vec![4]);
    }
    #[test]
    fn a_remove_names_its_dependents_and_restores_what_it_consumed() {
        let mut d = examples::design("solitaire").unwrap();
        let doc = d.cad.as_mut().unwrap();
        // 4 = Open bezel stock reads 2 and 3.
        let e = doc.apply(&CadEdit::Remove { id: 2 }).unwrap_err().to_string();
        assert!(e.contains("#4 Open bezel stock") && e.contains("1 depends on it"), "{e}");
        assert_eq!(doc.dependents(2), vec![4]);
        assert_eq!(json(doc), json(&examples::design("solitaire").unwrap().cad.unwrap()), "a refused edit leaves the document untouched");
        doc.through = Some(4);
        let a = doc.apply(&CadEdit::Remove { id: 4 }).unwrap();
        assert_eq!(a, Applied { id: None, label: "Remove Open bezel stock".into() });
        assert_eq!(doc.features.len(), 4);
        assert_eq!(doc.outputs, vec![1, 5, 2, 3], "the sources come back once nothing reads them");
        assert_eq!(doc.through, Some(3), "the rollback moves to the previous feature");
        assert!(doc.joints.is_empty(), "the joint naming the removed feature is dropped");
        assert!(evaluable(&d).is_ok());
        let mut d = examples::design("solitaire").unwrap();
        let removed = d.cad.as_mut().unwrap().remove_with_dependents(2).unwrap();
        assert_eq!(removed, vec![4, 2]);
        assert_eq!(d.cad.as_ref().unwrap().outputs, vec![1, 5, 3]);
        assert!(evaluable(&d).is_ok());
    }
    #[test]
    fn a_move_keeps_sources_first_and_dependents_last() {
        let mut d = examples::design("solitaire").unwrap();
        let doc = d.cad.as_mut().unwrap();
        let e = doc.apply(&CadEdit::Move { id: 4, after: None }).unwrap_err().to_string();
        assert!(e.contains("before its source #2 Setting stock"), "{e}");
        let e = doc.apply(&CadEdit::Move { id: 2, after: Some(4) }).unwrap_err().to_string();
        assert!(e.contains("after #4 Open bezel stock, which depends on it"), "{e}");
        assert!(doc.apply(&CadEdit::Move { id: 2, after: Some(2) }).is_err());
        doc.apply(&CadEdit::Move { id: 5, after: None }).unwrap();
        assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![5, 1, 2, 3, 4]);
        doc.apply(&CadEdit::Move { id: 1, after: Some(4) }).unwrap();
        assert_eq!(doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), vec![5, 2, 3, 4, 1]);
        assert!(evaluable(&d).is_ok());
    }
    #[test]
    fn an_operation_edit_keeps_the_outputs_honest_and_refuses_a_cycle() {
        let mut d = examples::design("solitaire").unwrap();
        let doc = d.cad.as_mut().unwrap();
        let cycle = Operation::Boolean { a: 4, b: 3, kind: Boolean::Union };
        let e = doc.apply(&CadEdit::Operation { id: 2, operation: cycle }).unwrap_err().to_string();
        assert!(e.contains("cycle"), "{e}");
        let later = Operation::Transform { source: 5, translation: [0.0; 3], rotation_deg: [0.0; 3] };
        let e = doc.apply(&CadEdit::Operation { id: 4, operation: later }).unwrap_err().to_string();
        assert!(e.contains("comes after it"), "{e}");
        let unknown = Operation::Transform { source: 42, translation: [0.0; 3], rotation_deg: [0.0; 3] };
        assert!(doc.apply(&CadEdit::Operation { id: 4, operation: unknown }).is_err());
        doc.apply(&CadEdit::Operation { id: 4, operation: Operation::Sphere { radius_mm: 2.0 } }).unwrap();
        assert_eq!(doc.outputs, vec![1, 4, 5, 2, 3], "the boolean's sources return to the outputs");
        assert!(evaluable(&d).is_ok());
        let doc = d.cad.as_mut().unwrap();
        doc.apply(&CadEdit::Operation { id: 5, operation: Operation::Boolean { a: 2, b: 3, kind: Boolean::Subtract } }).unwrap();
        assert_eq!(doc.outputs, vec![1, 4, 5]);
        assert!(evaluable(&d).is_ok());
        let doc = d.cad.as_mut().unwrap();
        doc.apply(&CadEdit::Operation { id: 5, operation: Operation::Sketch { sketch: crate::sketch::Sketch::circle(1.0) } }).unwrap();
        assert_eq!(doc.outputs, vec![1, 4, 2, 3], "a sketch leaves the outputs and frees what it read");
        doc.apply(&CadEdit::Operation { id: 5, operation: Operation::Cylinder { radius_mm: 1.0, height_mm: 1.0 } }).unwrap();
        assert_eq!(doc.outputs, vec![1, 4, 2, 3, 5]);
    }
    #[test]
    fn a_document_carries_one_procedural_shank() {
        let mut d = band_and_cylinder();
        let doc = d.cad.as_mut().unwrap();
        let mut second = feature(0, "Second shank", Operation::Band);
        second.component.role = ComponentRole::Shank;
        let e = doc.apply(&CadEdit::Add { feature: second.clone(), after: None }).unwrap_err().to_string();
        assert!(e.contains("one procedural shank") && e.contains("#1 Shank"), "{e}");
        second.enabled = false;
        let a = doc.apply(&CadEdit::Add { feature: second, after: None }).unwrap();
        assert_eq!(a.id, Some(3));
        assert!(doc.apply(&CadEdit::Enable { id: 3, enabled: true }).is_err());
        doc.apply(&CadEdit::Enable { id: 1, enabled: false }).unwrap();
        doc.apply(&CadEdit::Enable { id: 3, enabled: true }).unwrap();
        assert_eq!(doc.band(), Some(3));
        assert!(doc.apply(&CadEdit::Operation { id: 2, operation: Operation::Band }).is_err());
        assert!(doc.apply(&CadEdit::Enable { id: 9, enabled: true }).is_err());
    }
    #[test]
    fn the_small_edits_land_and_refuse_what_they_cannot_name() {
        let mut d = band_and_cylinder();
        let doc = d.cad.as_mut().unwrap();
        assert!(doc.apply(&CadEdit::Rename { id: 2, name: "  ".into() }).is_err());
        assert!(doc.apply(&CadEdit::Rename { id: 7, name: "x".into() }).is_err());
        doc.apply(&CadEdit::Rename { id: 2, name: "Collet".into() }).unwrap();
        assert_eq!(doc.feature(2).unwrap().name, "Collet");
        doc.apply(&CadEdit::Placement { id: 2, placement: Placement::ring(45.0, 0.3) }).unwrap();
        doc.apply(&CadEdit::Attach { id: 2, attach: Attach::Cut }).unwrap();
        doc.apply(&CadEdit::Stage { id: 2, stage: Stage::Bench }).unwrap();
        doc.apply(&CadEdit::Blend { id: 2, blend_mm: 0.4 }).unwrap();
        assert!(doc.apply(&CadEdit::Blend { id: 2, blend_mm: -0.1 }).is_err());
        assert!(doc.apply(&CadEdit::Blend { id: 2, blend_mm: f64::NAN }).is_err());
        // Refuses non-finite typed values.
        let e = doc.apply(&CadEdit::Placement { id: 2, placement: Placement::ring(f64::INFINITY, 0.3) }).unwrap_err();
        assert_eq!(e.to_string(), "#2 Collet: a placement needs finite numbers");
        let mut unseated = cylinder("Loose");
        unseated.component.placement = Placement::ring(90.0, f64::NAN);
        let e = doc.apply(&CadEdit::Add { feature: unseated, after: None }).unwrap_err();
        assert_eq!(e.to_string(), "Add Loose: a placement needs finite numbers");
        let e = doc.apply(&CadEdit::Component { id: 2, component: Component { blend_mm: -1.0, ..Component::default() } }).unwrap_err();
        assert_eq!(e.to_string(), "#2 Collet: blend radius must be finite and not negative");
        let c = &doc.feature(2).unwrap().component;
        assert_eq!(c.placement.theta_deg(), Some(45.0));
        assert_eq!((c.attach, c.stage, c.blend_mm), (Attach::Cut, Stage::Bench, 0.4));
        let mut component = Component::default();
        component.role = ComponentRole::Head;
        component.material = "Gold 18k".into();
        doc.apply(&CadEdit::Component { id: 2, component }).unwrap();
        assert_eq!(doc.feature(2).unwrap().component.material, "Gold 18k");
        assert_eq!(doc.feature(2).unwrap().component.blend_mm, 0.0, "a component edit replaces the whole component");
        assert!(doc.apply(&CadEdit::Outputs { outputs: vec![2, 9] }).is_err());
        assert!(doc.apply(&CadEdit::Outputs { outputs: vec![2, 2] }).is_err());
        doc.apply(&CadEdit::Outputs { outputs: vec![2] }).unwrap();
        assert_eq!(doc.outputs, vec![2]);
        assert!(doc.apply(&CadEdit::Through { through: Some(9) }).is_err());
        doc.apply(&CadEdit::Through { through: Some(1) }).unwrap();
        assert_eq!(doc.through, Some(1));
        doc.apply(&CadEdit::Through { through: None }).unwrap();
        assert_eq!(doc.through, None);
        let sketch = feature(0, "Plan", Operation::Sketch { sketch: crate::sketch::Sketch::circle(1.0) });
        let s = doc.apply(&CadEdit::Add { feature: sketch, after: None }).unwrap().id.unwrap();
        assert!(!doc.outputs.contains(&s), "a sketch has no body to output");
        assert!(doc.apply(&CadEdit::Outputs { outputs: vec![s] }).unwrap_err().to_string().contains("sketch"));
        assert_eq!(CadEdit::Remove { id: 2 }.label(), "Remove #2");
        assert_eq!(CadEdit::Remove { id: 2 }.label_in(doc), "Remove Collet");
        assert_eq!(CadEdit::Enable { id: 2, enabled: false }.label_in(doc), "Suppress Collet");
        assert_eq!(CadEdit::Through { through: Some(1) }.target(), Some(1));
        assert_eq!(CadEdit::Outputs { outputs: vec![] }.target(), None);
        assert_eq!(CadEdit::Blend { id: 2, blend_mm: 0.1 }.target(), Some(2));
        assert_eq!(CadEdit::Remove { id: 2 }, CadEdit::Remove { id: 2 });
        assert_ne!(CadEdit::Remove { id: 2 }, CadEdit::Remove { id: 1 });
    }
    #[test]
    fn every_edit_keeps_the_examples_evaluable() {
        let mut names: Vec<&str> = examples::NAMES.to_vec();
        names.push("band+cylinder");
        for name in names {
            let base = if name == "band+cylinder" { band_and_cylinder() } else { examples::design(name).unwrap() };
            let doc = base.cad.as_ref().unwrap();
            let first = doc.features[0].id;
            // The last feature nothing reads.
            let leaf = doc.features.iter().rev().find(|f| doc.dependents(f.id).is_empty()).unwrap().id;
            let mut edits = vec![
                CadEdit::Add { feature: cylinder("Added"), after: None },
                CadEdit::Add { feature: cylinder("Added first"), after: Some(first) },
                CadEdit::Rename { id: first, name: "Renamed".into() },
                CadEdit::Operation { id: leaf, operation: Operation::Sphere { radius_mm: 1.5 } },
                CadEdit::Placement { id: leaf, placement: Placement::ring(45.0, 0.5) },
                CadEdit::Attach { id: leaf, attach: Attach::Join },
                CadEdit::Stage { id: leaf, stage: Stage::Bench },
                CadEdit::Blend { id: leaf, blend_mm: 0.3 },
                CadEdit::Component { id: leaf, component: Component { role: ComponentRole::Head, ..Default::default() } },
                CadEdit::Outputs { outputs: doc.outputs.iter().rev().copied().collect() },
                CadEdit::Through { through: Some(first) },
                CadEdit::Through { through: None },
            ];
            // A leaf that reads a source, as a setting reads its stone, cannot move ahead of it.
            if doc.sources_of(leaf).is_empty() {
                edits.push(CadEdit::Move { id: leaf, after: None });
            }
            if doc.features.len() > 1 {
                edits.push(CadEdit::Move { id: first, after: Some(leaf) });
                edits.push(CadEdit::Remove { id: leaf });
                edits.push(CadEdit::Enable { id: leaf, enabled: false });
            }
            match name {
                "solitaire" => edits.push(CadEdit::Remove { id: 4 }),
                "gallery" => edits.extend([CadEdit::Remove { id: 3 }, CadEdit::Move { id: 3, after: Some(7) }]),
                "two-part-signet" => edits.push(CadEdit::Remove { id: 1 }),
                "band+cylinder" => edits.push(CadEdit::Enable { id: 1, enabled: false }),
                _ => {}
            }
            for edit in edits {
                let mut d = base.clone();
                let applied = d.apply_cad_edit(&edit).unwrap_or_else(|e| panic!("{name}: {edit:?}: {e}"));
                assert!(!applied.label.is_empty());
                evaluable(&d).unwrap_or_else(|e| panic!("{name}: after {edit:?}: {e:#}"));
            }
        }
    }
    #[test]
    fn a_plain_design_owns_its_document_and_a_driven_one_refuses() {
        let mut d = RingDesign::default();
        assert!(d.apply_cad_edit(&CadEdit::Rename { id: 1, name: "x".into() }).is_err());
        assert!(d.cad.is_none(), "a refused first edit leaves no empty document behind");
        let a = d.apply_cad_edit(&CadEdit::Add { feature: cylinder("Post"), after: None }).unwrap();
        assert_eq!(a.id, Some(1));
        assert_eq!(d.cad.as_ref().unwrap().outputs, vec![1]);
        d.apply_cad_edit(&CadEdit::Remove { id: 1 }).unwrap();
        assert!(d.cad.is_none(), "an emptied document is dropped");
        assert!(d.apply_cad_edit(&CadEdit::Add { feature: cylinder("Post"), after: Some(4) }).is_err());
        assert!(d.cad.is_none(), "a refused first add leaves no document behind");
        d.cad = Some(Document::default());
        assert!(d.apply_cad_edit(&CadEdit::Rename { id: 1, name: "x".into() }).is_err());
        assert!(d.cad.as_ref().is_some_and(|doc| doc.features.is_empty()), "a refused edit leaves an empty document as it was");
        let mut driven = band_and_cylinder();
        driven.graph = Some(serde_json::json!({"name": "x"}));
        let e = driven.apply_cad_edit(&CadEdit::Rename { id: 2, name: "x".into() }).unwrap_err();
        assert!(e.to_string().contains("edit the graph"), "{e}");
        assert_eq!(driven.cad.as_ref().unwrap().feature(2).unwrap().name, "Bezel");
    }
}
