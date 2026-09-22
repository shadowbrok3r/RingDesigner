//! The headless command session: every viewport tool as one state machine over one token vocabulary.
use super::snap::{RingPoint, SnapHit};
use crate::icons::Icon;
use ringdesign_core::cad::{Attach, Feature, Operation, Placement, Stage};
use ringdesign_core::interaction::pick::Pick;

/// What a dimension is measured in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unit {
    Mm,
    Deg,
    Ratio,
    Count,
}
impl Unit {
    /// The unit a field's name carries; empty for a count.
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Mm => "mm",
            Self::Deg => "°",
            Self::Ratio => "×",
            Self::Count => "",
        }
    }
    /// The value as a field's hint reads it.
    pub fn format(self, v: f64) -> String {
        match self {
            Self::Mm => format!("{v:.2} mm"),
            Self::Deg => format!("{v:.1}°"),
            Self::Ratio => format!("{v:.3}×"),
            Self::Count => format!("{}", v.round() as i64),
        }
    }
}

/// One editable number of the live step; `locked` once a typed value holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Dimension {
    pub key: &'static str,
    pub label: &'static str,
    pub unit: Unit,
    pub value: f64,
    pub locked: bool,
}

/// A degree of freedom the pointer can be restricted to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Theta,
    Across,
    Height,
    X,
    Y,
    Z,
    Spin,
    Tilt,
    Cant,
}
impl Axis {
    pub fn key(self) -> &'static str {
        match self {
            Self::Theta => "theta",
            Self::Across => "across",
            Self::Height => "height",
            Self::X => "x",
            Self::Y => "y",
            Self::Z => "z",
            Self::Spin => "spin",
            Self::Tilt => "tilt",
            Self::Cant => "cant",
        }
    }
}

/// One token fed to a command: the whole vocabulary a viewport, MCP or a replay speaks.
#[derive(Clone, Debug, PartialEq)]
pub enum StepInput {
    /// The pointer on the ring: world point and normal, and the same point in the ring frame (the snap's when `snapped`).
    Pointer {
        world: [f64; 3],
        normal: [f64; 3],
        theta_deg: f64,
        across_mm: f64,
        height_mm: f64,
        snapped: Option<SnapHit>,
        dragging: bool,
    },
    Click,
    Typed { key: &'static str, value: f64 },
    Cleared { key: &'static str },
    Lock(Axis),
    Unlock,
    Confirm,
    Back,
    Cancel,
}
impl StepInput {
    /// The pointer over a pick, `height_mm` off the surface; a snap replaces the point and its ring frame.
    pub fn pointer(pick: &Pick, height_mm: f64, snapped: Option<SnapHit>, dragging: bool) -> Self {
        let (world, ring) = match &snapped {
            Some(s) => (s.world, s.ring),
            None => (pick.world, RingPoint::of_world(pick.world, height_mm)),
        };
        Self::Pointer {
            world,
            normal: pick.normal,
            theta_deg: ring.theta_deg,
            across_mm: ring.across_mm,
            height_mm: ring.height_mm,
            snapped,
            dragging,
        }
    }
    /// The token a history line records.
    pub fn token(&self) -> String {
        match self {
            Self::Pointer { world, theta_deg, across_mm, height_mm, snapped, dragging, .. } => {
                let [x, y, z] = world;
                let mut t = format!("pointer θ={theta_deg:.2} v={across_mm:.3} h={height_mm:.3} at {x:.3},{y:.3},{z:.3}");
                if let Some(s) = snapped {
                    t.push_str(&format!(" snap={}", s.label));
                }
                if *dragging {
                    t.push_str(" drag");
                }
                t
            }
            Self::Click => "click".into(),
            Self::Typed { key, value } => format!("typed {key}={value}"),
            Self::Cleared { key } => format!("cleared {key}"),
            Self::Lock(axis) => format!("lock {}", axis.key()),
            Self::Unlock => "unlock".into(),
            Self::Confirm => "confirm".into(),
            Self::Back => "back".into(),
            Self::Cancel => "cancel".into(),
        }
    }
}

/// What a command did with a token.
#[derive(Clone, Debug)]
pub enum Outcome {
    Continue,
    NextStep,
    Commit(Vec<Effect>),
    Cancelled,
    Refused(String),
}
impl Outcome {
    pub fn is_commit(&self) -> bool {
        matches!(self, Self::Commit(_))
    }
    pub fn is_refused(&self) -> bool {
        matches!(self, Self::Refused(_))
    }
}

/// A committed change, as data for the integrator's edit funnel.
#[derive(Clone, Debug)]
pub enum Effect {
    Placement { feature: u64, placement: Placement },
    Operation { feature: u64, operation: Operation },
    Attach { feature: u64, attach: Attach },
    Stage { feature: u64, stage: Stage },
    Add { feature: Feature },
}

/// What the viewport draws while a command is live.
#[derive(Clone, Debug, Default)]
pub struct Preview {
    pub placement: Option<Placement>,
    pub operation: Option<Operation>,
    /// Points to draw: anchors, centres, the pointer.
    pub ghost: Vec<[f64; 3]>,
    pub caption: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepInfo {
    pub name: &'static str,
    pub prompt: &'static str,
}

/// A viewport tool as a state machine over tokens.
pub trait ViewCommand {
    fn key(&self) -> &'static str;
    fn title(&self) -> String;
    fn step(&self) -> usize;
    fn steps(&self) -> Vec<StepInfo>;
    fn dimensions(&self) -> Vec<Dimension>;
    fn feed(&mut self, input: &StepInput) -> Outcome;
    fn preview(&self) -> Preview;
}

/// A command's reflected metadata: what a tool rail, a palette and MCP list.
#[derive(Clone, Debug)]
pub struct CommandInfo {
    pub key: &'static str,
    pub title: String,
    pub hotkey: Option<char>,
    pub steps: Vec<StepInfo>,
    pub icon: Icon,
}

/// Holds the live command, records every token and runs the escape ladder.
#[derive(Default)]
pub struct Session {
    cmd: Option<Box<dyn ViewCommand>>,
    history: Vec<String>,
    /// Typed dimension keys, latest last.
    typed: Vec<&'static str>,
}
impl Session {
    /// Replaces any live command without committing it.
    pub fn start(&mut self, cmd: Box<dyn ViewCommand>) {
        self.history.clear();
        self.history.push(format!("start {}", cmd.key()));
        self.typed.clear();
        self.cmd = Some(cmd);
    }
    /// Feeds one token; a commit or a cancel ends the command.
    pub fn feed(&mut self, input: StepInput) -> Outcome {
        let Some(cmd) = self.cmd.as_mut() else {
            return Outcome::Refused("No command is live".into());
        };
        // Consecutive pointer tokens coalesce to the last one before the next token.
        let token = input.token();
        if token.starts_with("pointer") && self.history.last().is_some_and(|t| t.starts_with("pointer")) {
            self.history.pop();
        }
        self.history.push(token);
        let outcome = cmd.feed(&input);
        match input {
            StepInput::Typed { key, .. } if !outcome.is_refused() => {
                self.typed.retain(|k| *k != key);
                self.typed.push(key);
            }
            StepInput::Cleared { key } => self.typed.retain(|k| *k != key),
            _ => {}
        }
        if matches!(outcome, Outcome::Commit(_) | Outcome::Cancelled) {
            self.cmd = None;
            self.typed.clear();
        }
        outcome
    }
    /// The ladder: the step's latest typed field clears, else the step backs out, else the command cancels.
    pub fn escape(&mut self) -> Outcome {
        let Some(cmd) = self.cmd.as_ref() else {
            return Outcome::Refused("No command is live".into());
        };
        let locked: Vec<&'static str> = cmd.dimensions().into_iter().filter(|d| d.locked).map(|d| d.key).collect();
        let latest = self.typed.iter().rev().copied().find(|k| locked.contains(k));
        if let Some(key) = latest.or(locked.last().copied()) {
            return self.feed(StepInput::Cleared { key });
        }
        if cmd.step() > 0 {
            return self.feed(StepInput::Back);
        }
        self.feed(StepInput::Cancel)
    }
    pub fn enter(&mut self) -> Outcome {
        self.feed(StepInput::Confirm)
    }
    pub fn is_live(&self) -> bool {
        self.cmd.is_some()
    }
    /// The live step's prompt under the command's title; empty when nothing is live.
    pub fn prompt(&self) -> String {
        let Some(cmd) = self.cmd.as_ref() else {
            return String::new();
        };
        let steps = cmd.steps();
        match steps.get(cmd.step()) {
            Some(s) if steps.len() > 1 => format!("{}: {} — {}", cmd.title(), s.name, s.prompt),
            Some(s) => format!("{}: {}", cmd.title(), s.prompt),
            None => cmd.title(),
        }
    }
    pub fn dimensions(&self) -> Vec<Dimension> {
        self.cmd.as_ref().map(|c| c.dimensions()).unwrap_or_default()
    }
    pub fn preview(&self) -> Option<Preview> {
        self.cmd.as_ref().map(|c| c.preview())
    }
    pub fn command(&self) -> Option<&dyn ViewCommand> {
        self.cmd.as_deref()
    }
    /// The tokens fed since `start`, for a replay.
    pub fn history(&self) -> &[String] {
        &self.history
    }
    /// Every command the viewport offers, with its hotkey, steps and mark.
    pub fn catalog() -> Vec<CommandInfo> {
        super::commands::catalog()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::snap::SnapKind;
    use ringdesign_core::interaction::pick::Entity;

    struct Counter {
        step: usize,
        typed: [Option<f64>; 2],
    }
    impl ViewCommand for Counter {
        fn key(&self) -> &'static str {
            "counter"
        }
        fn title(&self) -> String {
            "Counter".into()
        }
        fn step(&self) -> usize {
            self.step
        }
        fn steps(&self) -> Vec<StepInfo> {
            vec![StepInfo { name: "One", prompt: "first" }, StepInfo { name: "Two", prompt: "second" }]
        }
        /// Both fields on the first step, only `b` on the second.
        fn dimensions(&self) -> Vec<Dimension> {
            let keys: &[&'static str] = if self.step == 0 { &["a", "b"] } else { &["b"] };
            keys.iter()
                .map(|&key| {
                    let t = self.typed[usize::from(key == "b")];
                    Dimension { key, label: key, unit: Unit::Count, value: t.unwrap_or(0.0), locked: t.is_some() }
                })
                .collect()
        }
        fn feed(&mut self, input: &StepInput) -> Outcome {
            match input {
                StepInput::Click | StepInput::Confirm if self.step == 0 => {
                    self.step = 1;
                    Outcome::NextStep
                }
                StepInput::Confirm => Outcome::Commit(vec![Effect::Stage { feature: 1, stage: Stage::Bench }]),
                StepInput::Typed { key, value } => {
                    self.typed[usize::from(*key == "b")] = Some(*value);
                    Outcome::Continue
                }
                StepInput::Cleared { key } => {
                    self.typed[usize::from(*key == "b")] = None;
                    Outcome::Continue
                }
                StepInput::Back if self.step > 0 => {
                    self.step -= 1;
                    Outcome::Continue
                }
                StepInput::Back | StepInput::Cancel => Outcome::Cancelled,
                _ => Outcome::Continue,
            }
        }
        fn preview(&self) -> Preview {
            Preview::default()
        }
    }
    fn counter() -> Box<Counter> {
        Box::new(Counter { step: 0, typed: [None; 2] })
    }
    fn pointer(theta: f64) -> StepInput {
        StepInput::Pointer { world: [0.0, 9.5, 0.0], normal: [0.0, 1.0, 0.0], theta_deg: theta, across_mm: 0.0, height_mm: 0.0, snapped: None, dragging: false }
    }

    #[test]
    fn the_escape_ladder_clears_the_latest_typed_then_cancels_on_the_first_step() {
        let mut s = Session::default();
        assert!(s.feed(StepInput::Click).is_refused());
        assert!(s.escape().is_refused());
        s.start(counter());
        assert_eq!(s.prompt(), "Counter: One — first");
        s.feed(StepInput::Typed { key: "b", value: 3.0 });
        s.feed(StepInput::Typed { key: "a", value: 2.0 });
        assert!(matches!(s.escape(), Outcome::Continue));
        assert_eq!(s.dimensions().iter().map(|d| d.locked).collect::<Vec<_>>(), [false, true], "the latest typed clears first");
        assert!(matches!(s.escape(), Outcome::Continue));
        assert!(s.dimensions().iter().all(|d| !d.locked));
        assert!(matches!(s.escape(), Outcome::Cancelled));
        assert!(!s.is_live(), "on the first step with nothing typed the command cancels");
        assert_eq!(s.history(), ["start counter", "typed b=3", "typed a=2", "cleared a", "cleared b", "cancel"]);
    }

    #[test]
    fn a_field_the_step_backs_into_still_typed_clears_before_the_command_cancels() {
        let mut s = Session::default();
        s.start(counter());
        s.feed(StepInput::Typed { key: "b", value: 3.0 });
        s.feed(StepInput::Typed { key: "a", value: 2.0 });
        assert!(matches!(s.feed(StepInput::Click), Outcome::NextStep));
        assert!(matches!(s.escape(), Outcome::Continue));
        assert!(!s.dimensions()[0].locked, "the second step's own field clears");
        assert!(matches!(s.escape(), Outcome::Continue));
        assert_eq!((s.command().unwrap().step(), s.dimensions()[0].locked), (0, true), "then the step backs out onto a still-typed field");
        assert!(matches!(s.escape(), Outcome::Continue));
        assert!(s.dimensions().iter().all(|d| !d.locked), "which clears before anything is cancelled");
        assert!(matches!(s.escape(), Outcome::Cancelled));
        assert_eq!(s.history(), ["start counter", "typed b=3", "typed a=2", "click", "cleared b", "back", "cleared a", "cancel"]);
    }

    #[test]
    fn enter_confirms_and_the_history_keeps_the_last_pointer_before_each_token() {
        let mut s = Session::default();
        s.start(counter());
        s.feed(pointer(10.0));
        s.feed(pointer(20.0));
        s.feed(pointer(30.0));
        assert!(matches!(s.enter(), Outcome::NextStep));
        s.feed(pointer(40.0));
        let out = s.enter();
        assert!(matches!(&out, Outcome::Commit(e) if matches!(e[0], Effect::Stage { feature: 1, stage: Stage::Bench })));
        assert!(!s.is_live());
        assert_eq!(
            s.history(),
            [
                "start counter",
                "pointer θ=30.00 v=0.000 h=0.000 at 0.000,9.500,0.000",
                "confirm",
                "pointer θ=40.00 v=0.000 h=0.000 at 0.000,9.500,0.000",
                "confirm"
            ]
        );
        assert!(s.preview().is_none());
        assert_eq!(s.prompt(), "");
        assert!(s.feed(StepInput::Confirm).is_refused(), "a committed command takes no more tokens");
    }

    #[test]
    fn a_pick_becomes_a_pointer_in_the_ring_frame_and_a_snap_replaces_it() {
        let pick = Pick { entity: Entity::Band, world: [-9.5, 0.0, 0.75], normal: [-1.0, 0.0, 0.0], depth: 40.0, px: 0.0 };
        let StepInput::Pointer { world, theta_deg, across_mm, height_mm, .. } = StepInput::pointer(&pick, 0.25, None, true) else { panic!() };
        assert_eq!((world, theta_deg, across_mm, height_mm), ([-9.5, 0.0, 0.75], 180.0, 0.75, 0.25));
        let snap = SnapHit {
            kind: SnapKind::PartingPlane,
            world: [-9.5, 0.0, 0.0],
            ring: RingPoint { theta_deg: 180.0, across_mm: 0.0, height_mm: 0.25 },
            label: "parting plane".into(),
        };
        let input = StepInput::pointer(&pick, 0.25, Some(snap), true);
        assert_eq!(input.token(), "pointer θ=180.00 v=0.000 h=0.250 at -9.500,0.000,0.000 snap=parting plane drag");
    }

    #[test]
    fn the_catalog_lists_every_command_with_a_mark_and_the_three_hotkeys() {
        let cat = Session::catalog();
        let keys: Vec<_> = cat.iter().map(|c| c.key).collect();
        assert_eq!(keys, ["move", "rotate", "scale", "place", "add-box", "add-cylinder", "add-sphere", "attach"]);
        let hot: Vec<_> = cat.iter().filter_map(|c| c.hotkey.map(|h| (c.key, h))).collect();
        assert_eq!(hot, [("move", 'G'), ("rotate", 'R'), ("scale", 'S')]);
        assert!(cat.iter().all(|c| !c.steps.is_empty() && !c.title.is_empty()));
        let marks: std::collections::HashSet<_> = cat.iter().map(|c| c.icon).collect();
        assert_eq!(marks.len(), cat.len(), "every command has its own mark");
        assert_eq!(cat.iter().find(|c| c.key == "add-cylinder").unwrap().steps.len(), 3);
        assert_eq!(cat.iter().find(|c| c.key == "add-sphere").unwrap().steps.len(), 2);
    }
}
