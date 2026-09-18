//! Touch-friendly workshop shared by the configurator and Android.
pub mod artwork;
pub mod imported_base;
pub mod construction;
pub mod focus;
pub mod icons;
pub mod navigation;
#[cfg(feature = "glow")]
pub mod loupe;
pub mod job;
mod ui;
pub mod visual;
mod worker;
pub mod workflow;
use job::{Action, Artifact, Done, Job, Output, Stage, View};
use ringdesign_core::{AlphaLibrary, RingDesign, manufacturing as mf};

pub fn key(d: &RingDesign) -> u64 {
    mf::package::fingerprint(&serde_json::to_vec(d).expect("serializable design"))
}

#[derive(Default)]
pub struct Session {
    source_key: Option<u64>,
    pub draft: Option<RingDesign>,
    pub view: Option<View>,
    view_key: Option<u64>,
    pub error: Option<String>,
    request: u64,
    pub busy: bool,
    undo: Vec<RingDesign>,
    redo: Vec<RingDesign>,
}
impl Session {
    pub fn sync(&mut self, source: &RingDesign) {
        let next = key(source);
        if self.source_key != Some(next) {
            if self.source_key.is_some() {
                self.undo.clear();
                self.redo.clear();
            }
            self.source_key = Some(next);
            self.cancel();
        }
    }
    pub fn current<'a>(&'a self, source: &'a RingDesign) -> &'a RingDesign {
        self.draft.as_ref().unwrap_or(source)
    }
    pub fn edit(&mut self, source: &RingDesign) -> &mut RingDesign {
        self.draft.get_or_insert_with(|| source.clone())
    }
    pub fn cancel(&mut self) {
        self.draft = None;
        self.view = None;
        self.view_key = None;
        self.request += 1;
        self.busy = false;
        self.error = None;
    }
    pub fn is_current(&self, source: &RingDesign) -> bool {
        self.view_key == Some(key(self.current(source)))
    }
    fn job(
        &mut self,
        source: &RingDesign,
        lib: &AlphaLibrary,
        stage: Stage,
        action: Action,
    ) -> Job {
        self.request += 1;
        self.busy = true;
        self.error = None;
        let key = key(self.current(source));
        let mut design = self.current(source).clone();
        design.embed_alphas(&mf::source_library(&design, lib));
        Job {
            id: self.request,
            key,
            design,
            stage,
            action,
        }
    }
    fn receive(&mut self, source: &RingDesign, done: Done) -> Option<Artifact> {
        if done.id != self.request {
            return None;
        }
        self.busy = false;
        if done.key != key(self.current(source)) {
            return None;
        }
        match done.result {
            Ok(Output::View(view)) => {
                self.view_key = Some(done.key);
                self.view = Some(view);
            }
            Ok(Output::Artifact(file)) => return Some(file),
            Err(e) => {
                self.error = Some(e);
                self.view_key = None;
            }
        }
        None
    }
    pub fn apply(&mut self, source: &mut RingDesign) -> bool {
        if self.draft.is_none() || !self.is_current(source) || self.busy {
            return false;
        }
        self.undo.push(source.clone());
        if self.undo.len() > 16 {
            self.undo.remove(0);
        }
        self.redo.clear();
        *source = self.view.as_ref().unwrap().design.clone();
        self.source_key = Some(key(source));
        self.draft = None;
        self.view_key = self.source_key;
        true
    }
    pub fn undo(&mut self, source: &mut RingDesign) -> bool {
        let Some(d) = self.undo.pop() else {
            return false;
        };
        self.redo.push(std::mem::replace(source, d));
        self.source_key = Some(key(source));
        self.cancel();
        true
    }
    pub fn redo(&mut self, source: &mut RingDesign) -> bool {
        let Some(d) = self.redo.pop() else {
            return false;
        };
        self.undo.push(std::mem::replace(source, d));
        self.source_key = Some(key(source));
        self.cancel();
        true
    }
}

pub struct Workshop {
    pub session: Session,
    worker: Option<worker::Worker>,
    stage: Stage,
    tab: usize,
    feature: usize,
    layer: usize,
    bore: f64,
    project_text: String,
    feature_text: String,
    feature_text_id: Option<u64>,
    yaw: f32,
    pitch: f32,
    section_axis: usize,
    section_offset: f64,
    section: bool,
    selected_finding: Option<usize>,
    pub message: String,
}
impl Default for Workshop {
    fn default() -> Self {
        Self {
            session: Session::default(),
            worker: None,
            stage: Stage::Nominal,
            tab: 0,
            feature: 0,
            layer: 0,
            bore: 17.8,
            project_text: String::new(),
            feature_text: String::new(),
            feature_text_id: None,
            yaw: 0.6,
            pitch: 0.7,
            section_axis: 2,
            section_offset: 0.0,
            section: false,
            selected_finding: None,
            message: String::new(),
        }
    }
}
/// Files are offered to the host only after an explicit export action.
#[derive(Default)]
pub struct Events {
    pub changed: bool,
    pub files: Vec<Artifact>,
}
impl Workshop {
    fn send(&mut self, ui: &egui::Ui, d: &RingDesign, lib: &AlphaLibrary, action: Action) {
        if self.session.busy {
            return;
        }
        if self.worker.is_none() {
            match worker::Worker::new(ui.ctx().clone()) {
                Ok(w) => self.worker = Some(w),
                Err(e) => {
                    self.session.error = Some(e);
                    return;
                }
            }
        }
        let job = self.session.job(d, lib, self.stage, action);
        if let Err(e) = self.worker.as_mut().unwrap().send(job) {
            self.session.error = Some(e);
            self.session.busy = false;
        }
    }
    /// Import is a candidate and requires Preview / Apply. No existing source is overwritten.
    pub fn import(&mut self, text: &str) {
        match ringdesign_core::library::load_design_str(text) {
            Ok(d) => {
                self.session.cancel();
                self.session.draft = Some(d);
                self.feature_text_id = None;
                self.message = "Imported candidate — Preview, then Apply".into();
            }
            Err(e) => self.session.error = Some(e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_cancel_stale_result_apply_and_undo() {
        let mut d = RingDesign::default();
        let original = key(&d);
        let lib = AlphaLibrary::builtin();
        let mut s = Session::default();
        s.sync(&d);
        s.edit(&d).name = "Candidate".into();
        let canceled = s.job(&d, &lib, Stage::Nominal, Action::Inspect);
        s.cancel();
        assert_eq!(key(&d), original);
        assert!(s.receive(&d, job::process(canceled)).is_none());
        assert!(s.view.is_none());
        s.edit(&d).name = "Applied".into();
        let j = s.job(&d, &lib, Stage::Nominal, Action::Inspect);
        s.receive(&d, job::process(j));
        assert!(s.apply(&mut d));
        assert_eq!(d.name, "Applied");
        assert!(s.undo(&mut d));
        assert_eq!(key(&d), original);
        assert!(s.redo(&mut d));
        assert_eq!(d.name, "Applied");
        s.edit(&d).name = "Stale".into();
        let j = s.job(&d, &lib, Stage::Nominal, Action::Project);
        d.name = "External".into();
        s.sync(&d);
        assert!(s.receive(&d, job::process(j)).is_none());
        assert!(s.draft.is_none());
    }
    #[test]
    fn editing_during_evaluation_requires_new_preview() {
        let mut d = RingDesign::default();
        let lib = AlphaLibrary::builtin();
        let mut s = Session::default();
        s.sync(&d);
        s.edit(&d).name = "First".into();
        let j = s.job(&d, &lib, Stage::Nominal, Action::Inspect);
        s.edit(&d).name = "Second".into();
        s.receive(&d, job::process(j));
        assert!(!s.busy);
        assert!(!s.apply(&mut d));
        assert_ne!(d.name, "Second");
    }
}
