use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Duration {
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
}

impl Duration {
    pub fn ticks_per_bar(&self) -> usize {
        match self {
            Duration::Quarter => 4,
            Duration::Eighth => 8,
            Duration::Sixteenth => 16,
            Duration::ThirtySecond => 32,
        }
    }

    pub fn ticks_per_beat(&self) -> usize {
        match self {
            Duration::Quarter => 1,
            Duration::Eighth => 2,
            Duration::Sixteenth => 4,
            Duration::ThirtySecond => 8,
        }
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Duration::Quarter => "1/4",
            Duration::Eighth => "1/8",
            Duration::Sixteenth => "1/16",
            Duration::ThirtySecond => "1/32",
        };
        write!(f, "{}", name)
    }
}

