use regex::Regex;
use thiserror::Error;
use std::{fmt, str::FromStr};
use super::key::{Mode, MAJOR, MINOR};
use lazy_static::lazy_static;

lazy_static! {
    static ref EXT_RE: Regex = Regex::new(r"^([b#]*)(\d+)$").unwrap();
}

#[derive(Error, Debug)]
pub enum DegreeParseError {
    #[error("Invalid degree `{0}`")]
    InvalidDegree(String),

    #[error("Couldn't parse degree")]
    ParseIntError(#[from] std::num::ParseIntError),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Degree {
    pub degree: usize,
    pub adj: isize
}

impl Degree {
    pub fn to_interval(&self, mode: &Mode) -> isize {
        // Convert from 1-indexed to 0-indexed degrees
        let degree_0 = self.degree - 1;
        let octaves = (degree_0/8 * 12) as isize;
        (match mode {
            Mode::Major => MAJOR[degree_0 % 7],
            Mode::Minor => MINOR[degree_0 % 7]
        } as isize) + octaves + self.adj
    }
}

impl FromStr for Degree {
    type Err = DegreeParseError;

    /// Parses a chord extension, e.g. "7", "b7", "#9", "bb7"
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let caps = EXT_RE.captures(s).ok_or(DegreeParseError::InvalidDegree(s.to_string()))?;
        let adjustments = caps.get(1).and_then(|m| Some(m.as_str())).unwrap_or_default();
        let degree = caps.get(2)
            .ok_or(DegreeParseError::InvalidDegree("(none)".to_string()))?
            .as_str().parse::<usize>()?;
        let adj = adjustments.matches('#').count() as isize - adjustments.matches('b').count() as isize;
        Ok(Degree { degree, adj })

    }
}

impl TryFrom<&str> for Degree {
    type Error = DegreeParseError;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Ok(Self::from_str(s)?)
    }
}

impl fmt::Display for Degree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = "".to_string();
        let count = self.adj.abs() as usize;
        if self.adj < 0 {
            s.push_str(&std::iter::repeat("b").take(count).collect::<String>());
        } else if self.adj > 0 {
            s.push_str(&std::iter::repeat("#").take(count).collect::<String>());
        }
        write!(f, "{}{}", s, self.degree)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_degree() {
        let deg: Degree = "7".try_into().unwrap();
        assert_eq!(deg, Degree {degree: 7, adj: 0});

        let deg: Degree = "b7".try_into().unwrap();
        assert_eq!(deg, Degree {degree: 7, adj: -1});

        let deg: Degree = "bb7".try_into().unwrap();
        assert_eq!(deg, Degree {degree: 7, adj: -2});

        let deg: Degree = "#7".try_into().unwrap();
        assert_eq!(deg, Degree {degree: 7, adj: 1});

        let deg: Degree = "b#7".try_into().unwrap();
        assert_eq!(deg, Degree {degree: 7, adj: 0});
    }
}
