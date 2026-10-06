//! Tunable parameters controlling how strums are rendered.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StrumParams {
    // --- Voicing ---
    /// Number of "strings" the chord is voiced across. The chord's notes
    /// are stacked upwards from the bass note, repeating in higher octaves,
    /// until this many notes are reached. 0 uses the chord's notes as-is.
    pub strings: usize,
    /// Number of (highest) strings an up stroke hits. 0 hits all of them.
    pub up_strings: usize,

    // --- Strum timing ---
    /// Time from the first to the last string on a full down stroke.
    pub spread_ms: f64,
    /// Up strokes are usually quicker; their spread is scaled by this.
    pub up_spread_scale: f64,
    /// Accented strokes are hit harder and therefore faster; scale for their spread.
    pub accent_spread_scale: f64,
    /// Relative standard deviation of the spread from stroke to stroke.
    pub spread_jitter: f64,
    /// Per-string timing jitter, relative to the gap between strings.
    pub string_jitter: f64,
    /// Where the beat falls within the strum: 0 means the strum starts on
    /// the beat, 1 means it ends on the beat, 0.5 centers it.
    pub anchor: f64,
    /// Spread is never longer than this fraction of the time until the next stroke.
    pub max_spread_fraction: f64,

    // --- Stroke timing ---
    /// Standard deviation of each stroke's timing offset from the grid.
    pub timing_jitter_ms: f64,
    /// Systematic lateness of up strokes.
    pub up_late_ms: f64,
    /// Swing amount from 0 (straight) to 1 (full triplet feel), delaying off-beat subdivisions.
    pub swing: f64,

    // --- Velocity ---
    /// Base velocity of an unaccented down stroke.
    pub velocity: u8,
    /// Velocity scale for up strokes.
    pub up_velocity: f64,
    /// Velocity scale for accented strokes.
    pub accent_velocity: f64,
    /// Velocity scale for bass notes.
    pub bass_velocity: f64,
    /// Velocity change across the strings of one stroke: the last string
    /// struck is scaled by (1 + gradient/2), the first by (1 - gradient/2).
    pub velocity_gradient: f64,
    /// Standard deviation of random per-note velocity variation.
    pub velocity_jitter: f64,

    // --- Articulation ---
    /// Gap between a string being damped and being struck again.
    pub release_ms: f64,
    /// Whether a mute (`x`) produces a short percussive hit, or only damps the strings.
    pub mute_hit: bool,
    /// Length of the percussive hit on a mute.
    pub mute_ms: f64,
    /// Velocity scale of the percussive hit on a mute.
    pub mute_velocity: f64,
    /// Whether successive bass notes (`b`) alternate between the two lowest strings.
    pub alternate_bass: bool,
}

impl Default for StrumParams {
    fn default() -> Self {
        StrumParams {
            strings: 6,
            up_strings: 4,

            spread_ms: 35.0,
            up_spread_scale: 0.8,
            accent_spread_scale: 0.85,
            spread_jitter: 0.25,
            string_jitter: 0.2,
            anchor: 0.5,
            max_spread_fraction: 0.6,

            timing_jitter_ms: 6.0,
            up_late_ms: 4.0,
            swing: 0.0,

            velocity: 92,
            up_velocity: 0.8,
            accent_velocity: 1.2,
            bass_velocity: 1.05,
            velocity_gradient: 0.15,
            velocity_jitter: 6.0,

            release_ms: 4.0,
            mute_hit: true,
            mute_ms: 35.0,
            mute_velocity: 0.5,
            alternate_bass: true,
        }
    }
}

impl StrumParams {
    /// Parameters that render every stroke exactly on the grid with no
    /// randomness, for tests and for A/B comparisons.
    pub fn quantized() -> Self {
        StrumParams {
            spread_jitter: 0.0,
            string_jitter: 0.0,
            timing_jitter_ms: 0.0,
            up_late_ms: 0.0,
            swing: 0.0,
            velocity_jitter: 0.0,
            ..Default::default()
        }
    }
}
