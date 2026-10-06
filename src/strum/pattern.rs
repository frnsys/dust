//! Strum pattern notation.
//!
//! A pattern is a whitespace-separated list of beats. The characters within
//! a beat subdivide it evenly, so `d.` is two eighths (a down stroke then a
//! rest), `dud` is a triplet, and `d.du` is four sixteenths. `|` may be used
//! as a visual bar separator and is ignored.
//!
//! Tokens:
//! - `d` / `u`: down stroke / up stroke
//! - `D` / `U`: accented down stroke / up stroke
//! - `b` / `B`: bass note (alternating) / accented bass note
//! - `x` / `X`: mute (damp the strings, optionally with a percussive hit)
//! - `.`: rest (let ring)
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Down,
    Up,
    Bass,
    Mute,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StrokeEvent {
    /// Position in beats from the start of the pattern.
    pub beat: f64,
    pub stroke: Stroke,
    pub accent: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StrumPattern {
    /// Length of the pattern in beats.
    pub beats: usize,
    /// Strokes in time order.
    pub strokes: Vec<StrokeEvent>,
}

#[derive(Error, Debug, PartialEq)]
pub enum PatternParseError {
    #[error("Empty strum pattern")]
    Empty,

    #[error("Unknown stroke token `{0}`")]
    UnknownToken(char),
}

impl FromStr for StrumPattern {
    type Err = PatternParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut strokes = vec![];
        let mut beats = 0;
        for group in s.split_whitespace() {
            let group: Vec<char> = group.chars().filter(|c| *c != '|').collect();
            if group.is_empty() {
                continue;
            }
            let n = group.len() as f64;
            for (i, c) in group.iter().enumerate() {
                let beat = beats as f64 + i as f64 / n;
                let (stroke, accent) = match c {
                    'd' => (Stroke::Down, false),
                    'D' => (Stroke::Down, true),
                    'u' => (Stroke::Up, false),
                    'U' => (Stroke::Up, true),
                    'b' => (Stroke::Bass, false),
                    'B' => (Stroke::Bass, true),
                    'x' => (Stroke::Mute, false),
                    'X' => (Stroke::Mute, true),
                    '.' => continue,
                    other => return Err(PatternParseError::UnknownToken(*other)),
                };
                strokes.push(StrokeEvent { beat, stroke, accent });
            }
            beats += 1;
        }
        if beats == 0 {
            return Err(PatternParseError::Empty);
        }
        Ok(StrumPattern { beats, strokes })
    }
}

impl TryFrom<&str> for StrumPattern {
    type Error = PatternParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl StrumPattern {
    pub fn len_beats(&self) -> f64 {
        self.beats as f64
    }

    /// Beats from the stroke at `idx` to the following stroke,
    /// wrapping around to the start of the pattern.
    pub fn gap_after(&self, idx: usize) -> f64 {
        let cur = self.strokes[idx].beat;
        if idx + 1 < self.strokes.len() {
            self.strokes[idx + 1].beat - cur
        } else {
            self.len_beats() - cur + self.strokes.first().map(|s| s.beat).unwrap_or(0.0)
        }
    }

    /// All strokes of the (endlessly repeating) pattern that fall in the
    /// half-open window `[from, to)`, in absolute beats from the start of
    /// the first repetition. Each item is `(absolute_beat, stroke, gap_to_next)`.
    pub fn strokes_between(&self, from: f64, to: f64) -> Vec<(f64, StrokeEvent, f64)> {
        let mut out = vec![];
        if self.strokes.is_empty() || to <= from {
            return out;
        }
        let len = self.len_beats();
        let first_rep = (from / len).floor() as i64;
        let last_rep = ((to - f64::EPSILON) / len).floor() as i64;
        for rep in first_rep..=last_rep {
            let offset = rep as f64 * len;
            for (i, ev) in self.strokes.iter().enumerate() {
                let t = offset + ev.beat;
                if t >= from && t < to {
                    out.push((t, *ev, self.gap_after(i)));
                }
            }
        }
        out
    }
}

impl fmt::Display for StrokeEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let c = match (self.stroke, self.accent) {
            (Stroke::Down, false) => 'd',
            (Stroke::Down, true) => 'D',
            (Stroke::Up, false) => 'u',
            (Stroke::Up, true) => 'U',
            (Stroke::Bass, false) => 'b',
            (Stroke::Bass, true) => 'B',
            (Stroke::Mute, false) => 'x',
            (Stroke::Mute, true) => 'X',
        };
        write!(f, "{}@{}", c, self.beat)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_quarters() {
        let p: StrumPattern = "d u d u".parse().unwrap();
        assert_eq!(p.beats, 4);
        let beats: Vec<f64> = p.strokes.iter().map(|s| s.beat).collect();
        assert_eq!(beats, vec![0.0, 1.0, 2.0, 3.0]);
        assert_eq!(p.strokes[1].stroke, Stroke::Up);
    }

    #[test]
    fn test_parse_subdivisions() {
        let p: StrumPattern = "d. du .u du".parse().unwrap();
        assert_eq!(p.beats, 4);
        let beats: Vec<f64> = p.strokes.iter().map(|s| s.beat).collect();
        assert_eq!(beats, vec![0.0, 1.0, 1.5, 2.5, 3.0, 3.5]);
    }

    #[test]
    fn test_parse_triplets_and_bars() {
        let p: StrumPattern = "dud | d.u".parse().unwrap();
        assert_eq!(p.beats, 2);
        let beats: Vec<f64> = p.strokes.iter().map(|s| s.beat).collect();
        assert_eq!(beats, vec![0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0, 1.0 + 2.0 / 3.0]);
    }

    #[test]
    fn test_parse_tokens() {
        let p: StrumPattern = "D U b B x X".parse().unwrap();
        let kinds: Vec<(Stroke, bool)> = p.strokes.iter().map(|s| (s.stroke, s.accent)).collect();
        assert_eq!(kinds, vec![
            (Stroke::Down, true), (Stroke::Up, true),
            (Stroke::Bass, false), (Stroke::Bass, true),
            (Stroke::Mute, false), (Stroke::Mute, true),
        ]);
    }

    #[test]
    fn test_parse_rest_only_beats_count() {
        let p: StrumPattern = ". d . .".parse().unwrap();
        assert_eq!(p.beats, 4);
        assert_eq!(p.strokes.len(), 1);
        assert_eq!(p.strokes[0].beat, 1.0);
    }

    #[test]
    fn test_parse_errors() {
        assert_eq!("".parse::<StrumPattern>(), Err(PatternParseError::Empty));
        assert_eq!("d q".parse::<StrumPattern>(), Err(PatternParseError::UnknownToken('q')));
    }

    #[test]
    fn test_gap_after_wraps() {
        let p: StrumPattern = "d. du .u du".parse().unwrap();
        assert_eq!(p.gap_after(0), 1.0);
        assert_eq!(p.gap_after(1), 0.5);
        // Last stroke at 3.5 wraps to the first at 0.0 of the next repeat
        assert_eq!(p.gap_after(5), 0.5);
    }

    #[test]
    fn test_strokes_between_repeats() {
        let p: StrumPattern = "d u".parse().unwrap();
        let got: Vec<f64> = p.strokes_between(1.0, 5.0).iter().map(|(t, _, _)| *t).collect();
        assert_eq!(got, vec![1.0, 2.0, 3.0, 4.0]);
        // Half-open: a stroke exactly at `to` is excluded
        let got: Vec<f64> = p.strokes_between(0.0, 1.0).iter().map(|(t, _, _)| *t).collect();
        assert_eq!(got, vec![0.0]);
        // Fractional windows
        let got: Vec<f64> = p.strokes_between(0.25, 1.25).iter().map(|(t, _, _)| *t).collect();
        assert_eq!(got, vec![1.0]);
    }
}
