use crate::note::Note;
use crate::interval::Interval;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Chord {
    root: Note,
    intervals: Vec<Interval>
}

impl Chord {
    pub fn new(root: Note, intervals: Vec<isize>) -> Chord {
        Chord {
            root,
            intervals: intervals.into_iter()
                .map(Into::into).collect()
        }
    }

    /// Return the notes that make up this chord.
    pub fn notes(&self) -> Vec<Note> {
        let mut notes: Vec<Note> = self.intervals.iter().map(|intv| self.root + *intv).collect();
        notes.sort_by_key(|n| n.semitones);
        notes
    }

    pub fn describe_notes(&self) -> Vec<String> {
        self.notes().iter().map(|n| n.to_string()).collect()
    }
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_chord_notes() {
        let chord = Chord::new(
            "C3".try_into().unwrap(),
            vec![0, 4, 7]);
        let notes = chord.notes();
        let expected = vec![Note {
            semitones: 27
        }, Note {
            semitones: 31
        }, Note {
            semitones: 34
        }];
        assert_eq!(notes.len(), expected.len());
        for (a, b) in notes.iter().zip(expected) {
            assert_eq!(*a, b);
        }
    }
}
