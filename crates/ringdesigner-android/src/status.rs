//! The status line under the ring: the app's last word, lit while it is fresh, a tap opening it whole.

/// How long a new status stays lit, seconds.
pub const FRESH_S: f64 = 4.0;

/// What the status line remembers between frames.
#[derive(Clone, Debug, Default)]
pub struct Line {
    seen: String,
    /// When the words last changed, on egui's clock.
    since: f64,
    /// Shown whole rather than cut to one line.
    pub open: bool,
}

impl Line {
    /// Notes `text` at `now` and says whether it is still fresh; new words close a line left open.
    pub fn fresh(&mut self, text: &str, now: f64) -> bool {
        if text != self.seen {
            self.seen = text.to_string();
            self.since = now;
            self.open = false;
        }
        now - self.since < FRESH_S
    }

    /// Seconds until the words in hand stop being fresh at `now`; `None` once they have.
    pub fn fades_in(&self, now: f64) -> Option<f64> {
        let left = FRESH_S - (now - self.since);
        (left > 0.0).then_some(left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_words_are_lit_for_four_seconds_and_close_a_line_left_open() {
        let mut line = Line::default();
        assert!(line.fresh("Add Sketch · Add Extrude", 10.0));
        assert!(line.fresh("Add Sketch · Add Extrude", 13.9));
        assert_eq!(line.fades_in(13.0), Some(1.0));
        assert!(!line.fresh("Add Sketch · Add Extrude", 14.1), "the same words fade");
        assert_eq!(line.fades_in(14.1), None);
        line.open = true;
        assert!(line.fresh("649004 tris · 668.7 mm³ · 162 ms · Castable", 20.0), "new words light up again");
        assert!(!line.open, "and close the line to one row");
    }
}
