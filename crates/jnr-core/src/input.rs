//! Per-frame player input as bitflags, plus the replay token form.

use std::fmt;

use crate::ParseError;

/// The buttons held during one simulation tick (bitflags in a `u8`).
///
/// Replay token form: one letter per held button in the canonical order
/// `L R U D B J S` (left, right, up, down, run, jump, start), or `-` when
/// nothing is held. Examples: `R`, `RB`, `RBJ`, `-`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct InputFrame(u8);

impl InputFrame {
    /// Nothing held.
    pub const NONE: InputFrame = InputFrame(0);
    /// Left on the d-pad.
    pub const LEFT: InputFrame = InputFrame(1 << 0);
    /// Right on the d-pad.
    pub const RIGHT: InputFrame = InputFrame(1 << 1);
    /// Up on the d-pad.
    pub const UP: InputFrame = InputFrame(1 << 2);
    /// Down on the d-pad.
    pub const DOWN: InputFrame = InputFrame(1 << 3);
    /// Jump button.
    pub const JUMP: InputFrame = InputFrame(1 << 4);
    /// Run button ("B").
    pub const RUN: InputFrame = InputFrame(1 << 5);
    /// Start/pause button.
    pub const START: InputFrame = InputFrame(1 << 6);

    /// Token letters in canonical output order.
    const LETTERS: [(char, InputFrame); 7] = [
        ('L', InputFrame::LEFT),
        ('R', InputFrame::RIGHT),
        ('U', InputFrame::UP),
        ('D', InputFrame::DOWN),
        ('B', InputFrame::RUN),
        ('J', InputFrame::JUMP),
        ('S', InputFrame::START),
    ];

    /// All defined buttons.
    pub const ALL: InputFrame = InputFrame(0x7f);

    /// From raw bits; bits without a button are dropped.
    pub const fn from_bits(bits: u8) -> InputFrame {
        InputFrame(bits & InputFrame::ALL.0)
    }

    /// The raw bits.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// True if every button in `other` is held.
    pub const fn contains(self, other: InputFrame) -> bool {
        self.0 & other.0 == other.0
    }

    /// `self` plus the buttons in `other`.
    pub const fn with(self, other: InputFrame) -> InputFrame {
        InputFrame(self.0 | other.0)
    }

    /// `self` without the buttons in `other`.
    pub const fn without(self, other: InputFrame) -> InputFrame {
        InputFrame(self.0 & !other.0)
    }

    /// True if LEFT is held.
    pub const fn left(self) -> bool {
        self.contains(InputFrame::LEFT)
    }
    /// True if RIGHT is held.
    pub const fn right(self) -> bool {
        self.contains(InputFrame::RIGHT)
    }
    /// True if DOWN is held.
    pub const fn down(self) -> bool {
        self.contains(InputFrame::DOWN)
    }
    /// True if JUMP is held.
    pub const fn jump(self) -> bool {
        self.contains(InputFrame::JUMP)
    }
    /// True if RUN is held.
    pub const fn run(self) -> bool {
        self.contains(InputFrame::RUN)
    }

    /// Parses a replay token (`R`, `RBJ`, `-`). Letters may come in any
    /// order; unknown or repeated letters and the empty string are errors.
    pub fn parse_token(token: &str) -> Result<InputFrame, ParseError> {
        if token == "-" {
            return Ok(InputFrame::NONE);
        }
        if token.is_empty() {
            return Err(ParseError::new(
                "empty input token (use \"-\" for no input)",
            ));
        }
        let mut frame = InputFrame::NONE;
        for c in token.chars() {
            let Some(&(_, flag)) = InputFrame::LETTERS.iter().find(|(l, _)| *l == c) else {
                return Err(ParseError::new(format!(
                    "unknown input letter '{c}' in token \"{token}\" (allowed: L R U D J B S or -)"
                )));
            };
            if frame.contains(flag) {
                return Err(ParseError::new(format!(
                    "input letter '{c}' repeated in token \"{token}\""
                )));
            }
            frame = frame.with(flag);
        }
        Ok(frame)
    }

    /// The canonical replay token (letters in `L R U D B J S` order, or `-`).
    pub fn to_token(self) -> String {
        let s: String = InputFrame::LETTERS
            .iter()
            .filter(|(_, flag)| self.contains(*flag))
            .map(|(c, _)| *c)
            .collect();
        if s.is_empty() { "-".to_owned() } else { s }
    }
}

impl fmt::Display for InputFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_token())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_are_distinct() {
        let all = InputFrame::LETTERS
            .iter()
            .fold(InputFrame::NONE, |acc, (_, f)| acc.with(*f));
        assert_eq!(all.bits(), 0b0111_1111);
        assert!(InputFrame::RIGHT.with(InputFrame::JUMP).jump());
        assert!(
            !InputFrame::RIGHT
                .with(InputFrame::JUMP)
                .without(InputFrame::JUMP)
                .jump()
        );
    }

    #[test]
    fn parse_examples() {
        assert_eq!(InputFrame::parse_token("-").unwrap(), InputFrame::NONE);
        assert_eq!(InputFrame::parse_token("R").unwrap(), InputFrame::RIGHT);
        let rbj = InputFrame::RIGHT
            .with(InputFrame::RUN)
            .with(InputFrame::JUMP);
        assert_eq!(InputFrame::parse_token("RBJ").unwrap(), rbj);
        assert_eq!(InputFrame::parse_token("JBR").unwrap(), rbj, "any order");
    }

    #[test]
    fn parse_errors() {
        assert!(InputFrame::parse_token("").is_err());
        assert!(InputFrame::parse_token("RX").is_err());
        assert!(InputFrame::parse_token("RR").is_err());
        assert!(InputFrame::parse_token("r").is_err());
        assert!(InputFrame::parse_token("R-").is_err());
    }

    #[test]
    fn round_trip_all_256_values() {
        assert_eq!(
            InputFrame::from_bits(0xff),
            InputFrame::ALL,
            "unknown bits masked"
        );
        for bits in 0..=0x7f_u8 {
            let f = InputFrame::from_bits(bits);
            let token = f.to_token();
            assert_eq!(InputFrame::parse_token(&token).unwrap(), f, "token {token}");
        }
        for token in ["-", "R", "RB", "RBJ", "LRUDBJS", "LJ", "DJ"] {
            assert_eq!(InputFrame::parse_token(token).unwrap().to_token(), token);
        }
        assert_eq!(InputFrame::parse_token("JR").unwrap().to_string(), "RJ");
    }
}
