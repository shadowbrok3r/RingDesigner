//! One finger or two on a view, told apart: a tap, a long press, a drag, a pinch.
use egui::{Event, PointerButton, Pos2, TouchPhase};

/// How far a finger may wander and still tap or long-press, points.
pub const SLOP_PT: f32 = 8.0;
/// How long a still finger must stay down to long-press, seconds.
pub const LONG_PRESS_S: f64 = 0.5;
/// The id the mouse's button presses as, on a host with no touch screen.
const MOUSE: u64 = u64::MAX;

/// Where a contact is in its life.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Start,
    Move,
    End,
    Cancel,
}

/// One touch point's event: which finger, what it did and where.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    pub id: u64,
    pub phase: Phase,
    pub pos: Pos2,
}

/// What the gesture in hand has become.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Gesture {
    #[default]
    Idle,
    /// One finger down, still inside the slop and short of the long press.
    Pressed,
    Dragging,
    LongPressed,
    Pinching,
}

/// What a contact or the clock tells the host, each once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    /// The first finger came down.
    Press(Pos2),
    /// The finger left the slop: the drag runs from where it came down.
    DragStart { from: Pos2, at: Pos2 },
    DragMove { at: Pos2 },
    /// The dragging finger lifted.
    DragEnd { at: Pos2 },
    /// Down and up inside the slop before the long press, at where it came down.
    Tap(Pos2),
    /// Held still past the long press, at where it came down.
    LongPress(Pos2),
    /// A second finger landed: whatever one finger was doing ends here, uncommitted.
    Pinch,
    /// The system took the touch away mid-gesture.
    Cancel,
    /// The last finger lifted after a long press or a pinch.
    Released,
}

/// Tells one finger's tap, long press and drag from a two-finger pinch, fed a view's contacts and the clock.
#[derive(Clone, Debug)]
pub struct Tracker {
    slop: f32,
    long_press: f64,
    /// Fingers down, the first one first.
    down: Vec<(u64, Pos2)>,
    /// Where and when the first finger came down.
    press: Option<(Pos2, f64)>,
    state: Gesture,
    /// Touch events have arrived, so pointer events are the first finger's echo and are not read.
    touch_seen: bool,
    /// Where the mouse last was while its button was down.
    mouse: Option<Pos2>,
}

impl Default for Tracker {
    fn default() -> Self {
        Self::new(SLOP_PT, LONG_PRESS_S)
    }
}

impl Tracker {
    /// A tracker that taps within `slop` points and long-presses after `long_press` seconds.
    pub fn new(slop: f32, long_press: f64) -> Self {
        Self { slop, long_press, down: Vec::new(), press: None, state: Gesture::Idle, touch_seen: false, mouse: None }
    }

    pub fn state(&self) -> Gesture {
        self.state
    }

    /// Fingers down now.
    pub fn fingers(&self) -> usize {
        self.down.len()
    }

    /// Where the first finger of the gesture in hand came down.
    pub fn origin(&self) -> Option<Pos2> {
        self.press.map(|(p, _)| p)
    }

    /// Whether only the clock can still turn the gesture into a long press, so a host that draws on demand asks for a frame.
    pub fn waiting(&self) -> bool {
        self.state == Gesture::Pressed
    }

    /// The long press, once, when the still finger has been down long enough by `now`.
    pub fn poll(&mut self, now: f64) -> Option<Signal> {
        let (at, since) = self.press?;
        (self.state == Gesture::Pressed && now - since >= self.long_press).then(|| {
            self.state = Gesture::LongPressed;
            Signal::LongPress(at)
        })
    }

    /// What one contact at `now` says, the clock read first.
    pub fn feed(&mut self, c: Contact, now: f64) -> Vec<Signal> {
        let mut out: Vec<Signal> = self.poll(now).into_iter().collect();
        match c.phase {
            Phase::Start => {
                if self.down.iter().any(|(id, _)| *id == c.id) {
                    return out;
                }
                self.down.push((c.id, c.pos));
                if self.down.len() == 1 {
                    self.press = Some((c.pos, now));
                    self.state = Gesture::Pressed;
                    out.push(Signal::Press(c.pos));
                } else if self.state != Gesture::Pinching {
                    self.state = Gesture::Pinching;
                    out.push(Signal::Pinch);
                }
            }
            Phase::Move => {
                let Some(slot) = self.down.iter_mut().find(|(id, _)| *id == c.id) else { return out };
                slot.1 = c.pos;
                if self.down[0].0 != c.id {
                    return out;
                }
                match (self.state, self.press) {
                    (Gesture::Pressed, Some((from, _))) if from.distance(c.pos) > self.slop => {
                        self.state = Gesture::Dragging;
                        out.push(Signal::DragStart { from, at: c.pos });
                    }
                    (Gesture::Dragging, _) => out.push(Signal::DragMove { at: c.pos }),
                    _ => {}
                }
            }
            Phase::End | Phase::Cancel => {
                let Some(i) = self.down.iter().position(|(id, _)| *id == c.id) else { return out };
                self.down.remove(i);
                if !self.down.is_empty() {
                    return out;
                }
                let from = self.press.map(|(p, _)| p);
                match (self.state, c.phase, from) {
                    (Gesture::Pressed | Gesture::Dragging, Phase::Cancel, _) => out.push(Signal::Cancel),
                    // A finger that lifts far from where it landed with no move between flicked: a drag in one event.
                    (Gesture::Pressed, _, Some(from)) if from.distance(c.pos) > self.slop => {
                        out.push(Signal::DragStart { from, at: c.pos });
                        out.push(Signal::DragEnd { at: c.pos });
                    }
                    (Gesture::Pressed, _, Some(from)) => out.push(Signal::Tap(from)),
                    (Gesture::Dragging, _, _) => out.push(Signal::DragEnd { at: c.pos }),
                    (Gesture::LongPressed | Gesture::Pinching, _, _) => out.push(Signal::Released),
                    _ => {}
                }
                self.state = Gesture::Idle;
                self.press = None;
            }
        }
        out
    }

    /// Drops the fingers still held when the host sees none down, as after a lift it never passed on; what that ends.
    pub fn settle(&mut self, touching: bool) -> Option<Signal> {
        if touching { None } else { self.reset() }
    }

    /// Drops the gesture in hand, as when the host skipped frames whose touches it never passed on; what that ends.
    pub fn reset(&mut self) -> Option<Signal> {
        let ended = match self.state {
            _ if self.down.is_empty() => None,
            Gesture::Pressed | Gesture::Dragging => Some(Signal::Cancel),
            Gesture::LongPressed | Gesture::Pinching => Some(Signal::Released),
            Gesture::Idle => None,
        };
        self.down.clear();
        self.press = None;
        self.state = Gesture::Idle;
        self.mouse = None;
        ended
    }

    /// What a frame's events say at `now`: its touches, else the mouse's primary button standing in for one finger.
    pub fn feed_events(&mut self, events: &[Event], now: f64) -> Vec<Signal> {
        self.touch_seen |= events.iter().any(|e| matches!(e, Event::Touch { .. }));
        let mut out = Vec::new();
        for e in events {
            let contact = match *e {
                Event::Touch { id, phase, pos, .. } => {
                    let phase = match phase {
                        TouchPhase::Start => Phase::Start,
                        TouchPhase::Move => Phase::Move,
                        TouchPhase::End => Phase::End,
                        TouchPhase::Cancel => Phase::Cancel,
                    };
                    Some(Contact { id: id.0, phase, pos })
                }
                _ if self.touch_seen => None,
                Event::PointerButton { pos, button: PointerButton::Primary, pressed, .. } => {
                    self.mouse = pressed.then_some(pos);
                    Some(Contact { id: MOUSE, phase: if pressed { Phase::Start } else { Phase::End }, pos })
                }
                Event::PointerMoved(pos) if self.mouse.is_some() => {
                    self.mouse = Some(pos);
                    Some(Contact { id: MOUSE, phase: Phase::Move, pos })
                }
                Event::PointerGone => self.mouse.take().map(|pos| Contact { id: MOUSE, phase: Phase::Cancel, pos }),
                _ => None,
            };
            if let Some(c) = contact {
                out.extend(self.feed(c, now));
            }
        }
        out.extend(self.poll(now));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::pos2;

    fn touch(id: u64, phase: Phase, x: f32, y: f32) -> Contact {
        Contact { id, phase, pos: pos2(x, y) }
    }

    #[test]
    fn a_short_still_touch_is_a_tap_where_it_came_down() {
        let mut t = Tracker::default();
        assert_eq!(t.feed(touch(3, Phase::Start, 100.0, 200.0), 0.0), [Signal::Press(pos2(100.0, 200.0))]);
        assert_eq!(t.state(), Gesture::Pressed);
        // A finger never lands still: a wobble inside the slop is still a tap.
        assert!(t.feed(touch(3, Phase::Move, 105.0, 204.0), 0.08).is_empty());
        assert_eq!(t.feed(touch(3, Phase::End, 106.0, 203.0), 0.21), [Signal::Tap(pos2(100.0, 200.0))]);
        assert_eq!((t.state(), t.fingers(), t.origin()), (Gesture::Idle, 0, None));
    }

    #[test]
    fn a_held_still_touch_long_presses_once_and_never_also_taps() {
        let mut t = Tracker::default();
        t.feed(touch(1, Phase::Start, 50.0, 50.0), 10.0);
        assert!(t.waiting());
        assert_eq!(t.poll(10.3), None);
        assert_eq!(t.poll(10.5), Some(Signal::LongPress(pos2(50.0, 50.0))));
        assert_eq!(t.poll(10.9), None, "once");
        assert!(!t.waiting());
        assert!(t.feed(touch(1, Phase::Move, 53.0, 51.0), 11.0).is_empty(), "a held finger's wobble after the press drags nothing");
        assert_eq!(t.feed(touch(1, Phase::End, 53.0, 51.0), 11.2), [Signal::Released]);
        // Released at 0.62 s with no frame polled between: the clock is read before the lift.
        t.feed(touch(2, Phase::Start, 50.0, 50.0), 20.0);
        assert_eq!(t.feed(touch(2, Phase::End, 50.0, 50.0), 20.62), [Signal::LongPress(pos2(50.0, 50.0)), Signal::Released]);
    }

    #[test]
    fn a_finger_that_leaves_the_slop_drags_from_where_it_landed_and_never_taps() {
        let mut t = Tracker::default();
        t.feed(touch(0, Phase::Start, 100.0, 100.0), 0.0);
        assert!(t.feed(touch(0, Phase::Move, 107.0, 100.0), 0.03).is_empty(), "7 pt is inside the 8 pt slop");
        assert_eq!(t.feed(touch(0, Phase::Move, 130.0, 100.0), 0.06), [Signal::DragStart { from: pos2(100.0, 100.0), at: pos2(130.0, 100.0) }]);
        assert_eq!(t.feed(touch(0, Phase::Move, 150.0, 90.0), 0.09), [Signal::DragMove { at: pos2(150.0, 90.0) }]);
        // A drag held past the long press is still a drag.
        assert_eq!(t.poll(2.0), None);
        assert_eq!(t.feed(touch(0, Phase::End, 151.0, 90.0), 2.1), [Signal::DragEnd { at: pos2(151.0, 90.0) }]);
        assert_eq!(t.state(), Gesture::Idle);
        // A flick: down and up far apart with no move between.
        t.feed(touch(0, Phase::Start, 0.0, 0.0), 3.0);
        assert_eq!(t.feed(touch(0, Phase::End, 40.0, 0.0), 3.05), [Signal::DragStart { from: pos2(0.0, 0.0), at: pos2(40.0, 0.0) }, Signal::DragEnd { at: pos2(40.0, 0.0) }]);
    }

    #[test]
    fn a_second_finger_makes_a_pinch_that_ends_a_drag_uncommitted_and_taps_nothing() {
        let mut t = Tracker::default();
        t.feed(touch(0, Phase::Start, 100.0, 100.0), 0.0);
        assert_eq!(t.feed(touch(1, Phase::Start, 180.0, 100.0), 0.02), [Signal::Pinch]);
        assert_eq!(t.state(), Gesture::Pinching);
        // Both fingers spread: neither is read as a drag.
        assert!(t.feed(touch(0, Phase::Move, 60.0, 100.0), 0.1).is_empty());
        assert!(t.feed(touch(1, Phase::Move, 240.0, 100.0), 0.1).is_empty());
        assert_eq!(t.poll(5.0), None, "a pinch never long-presses");
        assert!(t.feed(touch(1, Phase::End, 240.0, 100.0), 5.1).is_empty(), "one finger still down keeps the pinch");
        assert!(t.feed(touch(0, Phase::Move, 20.0, 100.0), 5.2).is_empty(), "the finger left behind drags nothing");
        assert_eq!(t.feed(touch(0, Phase::End, 20.0, 100.0), 5.3), [Signal::Released]);
        // A drag already under way when the second finger lands ends as a pinch, never as a drag's end.
        t.feed(touch(4, Phase::Start, 0.0, 0.0), 6.0);
        assert!(matches!(t.feed(touch(4, Phase::Move, 30.0, 0.0), 6.1)[..], [Signal::DragStart { .. }]));
        assert_eq!(t.feed(touch(5, Phase::Start, 90.0, 0.0), 6.2), [Signal::Pinch]);
        let tail: Vec<Signal> = [touch(4, Phase::Move, 40.0, 0.0), touch(5, Phase::End, 90.0, 0.0), touch(4, Phase::End, 40.0, 0.0)].into_iter().flat_map(|c| t.feed(c, 6.3)).collect();
        assert_eq!(tail, [Signal::Released]);
    }

    #[test]
    fn a_cancelled_touch_cancels_and_a_repeated_start_is_one_finger() {
        let mut t = Tracker::default();
        t.feed(touch(0, Phase::Start, 0.0, 0.0), 0.0);
        assert!(t.feed(touch(0, Phase::Start, 0.0, 0.0), 0.0).is_empty(), "a pass that repeats the landing adds no finger");
        assert_eq!(t.fingers(), 1);
        t.feed(touch(0, Phase::Move, 20.0, 0.0), 0.1);
        assert_eq!(t.feed(touch(0, Phase::Cancel, 20.0, 0.0), 0.2), [Signal::Cancel]);
        assert!(t.feed(touch(9, Phase::End, 1.0, 1.0), 0.3).is_empty(), "a finger never seen lifts nothing");
    }

    #[test]
    fn a_lift_the_host_never_passed_on_is_settled_and_the_next_touch_starts_fresh() {
        let mut t = Tracker::default();
        t.feed(touch(0, Phase::Start, 10.0, 10.0), 0.0);
        assert_eq!(t.settle(true), None, "a finger the host still sees down stays");
        // The lift landed while the view was not drawn: the host sees nothing down and the drag is cancelled.
        assert_eq!(t.settle(false), Some(Signal::Cancel));
        assert_eq!((t.state(), t.fingers(), t.origin()), (Gesture::Idle, 0, None));
        assert_eq!(t.settle(false), None, "once");
        // Unsettled, the same finger id landing again much later would add no finger and long-press at once.
        assert_eq!(t.feed(touch(0, Phase::Start, 50.0, 50.0), 30.0), [Signal::Press(pos2(50.0, 50.0))]);
        assert_eq!(t.poll(30.1), None);
        assert_eq!(t.feed(touch(0, Phase::End, 50.0, 50.0), 30.2), [Signal::Tap(pos2(50.0, 50.0))]);
        // A stale finger under a new id would have made a pinch that never ends; settled, a pinch ends as a release.
        t.feed(touch(1, Phase::Start, 0.0, 0.0), 40.0);
        t.feed(touch(2, Phase::Start, 60.0, 0.0), 40.0);
        assert_eq!(t.settle(false), Some(Signal::Released));
        assert_eq!(t.feed(touch(3, Phase::Start, 5.0, 5.0), 41.0), [Signal::Press(pos2(5.0, 5.0))]);
    }

    #[test]
    fn frame_events_read_touches_and_let_the_mouse_stand_in_for_a_finger() {
        let touch_event = |id: u64, phase: TouchPhase, x: f32| Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos: pos2(x, 10.0), force: None };
        let button = |pressed: bool, x: f32| Event::PointerButton { pos: pos2(x, 10.0), button: PointerButton::Primary, pressed, modifiers: Default::default() };
        // The mouse alone: its press and release are a tap, its move while down a drag.
        let mut mouse = Tracker::default();
        assert_eq!(mouse.feed_events(&[button(true, 5.0)], 0.0), [Signal::Press(pos2(5.0, 10.0))]);
        assert_eq!(mouse.feed_events(&[button(false, 6.0)], 0.1), [Signal::Tap(pos2(5.0, 10.0))]);
        mouse.feed_events(&[button(true, 5.0)], 1.0);
        assert!(matches!(mouse.feed_events(&[Event::PointerMoved(pos2(50.0, 10.0))], 1.1)[..], [Signal::DragStart { .. }]));
        assert_eq!(mouse.feed_events(&[Event::PointerMoved(pos2(60.0, 10.0)), button(false, 60.0)], 1.2), [Signal::DragMove { at: pos2(60.0, 10.0) }, Signal::DragEnd { at: pos2(60.0, 10.0) }]);
        // A touch screen reports each finger and echoes the first as the pointer; the echo is not a second finger.
        let mut screen = Tracker::default();
        let landing = [button(true, 5.0), touch_event(7, TouchPhase::Start, 5.0), Event::PointerMoved(pos2(5.0, 10.0))];
        assert_eq!(screen.feed_events(&landing, 0.0), [Signal::Press(pos2(5.0, 10.0))]);
        assert_eq!(screen.fingers(), 1);
        assert_eq!(screen.feed_events(&[button(false, 5.0), touch_event(7, TouchPhase::End, 5.0)], 0.1), [Signal::Tap(pos2(5.0, 10.0))]);
        // A still finger's frames with no events long-press on the clock.
        screen.feed_events(&[touch_event(8, TouchPhase::Start, 5.0)], 1.0);
        assert!(screen.feed_events(&[], 1.2).is_empty());
        assert_eq!(screen.feed_events(&[], 1.6), [Signal::LongPress(pos2(5.0, 10.0))]);
    }
}
