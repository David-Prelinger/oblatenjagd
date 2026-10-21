//! Parse errors of the audio text formats: `file:line:col: error[kind]: message`.

use std::fmt;
use std::ops::Range;

/// What kind of mistake a [`ParseError`] reports (one test per kind).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not valid TOML.
    Syntax,
    /// Valid TOML of the wrong shape: an unknown or missing key, a wrong
    /// type (floats included: every number is an integer).
    Schema,
    /// A key that exists but is not allowed here (e.g. `hold_ms` in an
    /// instrument, `duty_pct` on a saw).
    Key,
    /// A number outside its range.
    Range,
    /// An unknown `wave` name.
    Wave,
    /// No layers or more than [`crate::patch::MAX_LAYERS`].
    Layers,
    /// A bad note name or note range.
    Note,
    /// A drum-kit mistake: a bad letter, an unknown or unused patch.
    Kit,
    /// The file name is not a valid id (`[a-z][a-z0-9_]*` plus extension).
    Id,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKind::Syntax => "syntax",
            ErrorKind::Schema => "schema",
            ErrorKind::Key => "key",
            ErrorKind::Range => "range",
            ErrorKind::Wave => "wave",
            ErrorKind::Layers => "layers",
            ErrorKind::Note => "note",
            ErrorKind::Kit => "kit",
            ErrorKind::Id => "id",
        }
    }
}

/// An error in an audio source at `file:line:col` (1-based).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub kind: ErrorKind,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}:{}: error[{}]: {}",
            self.file,
            self.line,
            self.col,
            self.kind.as_str(),
            self.message
        )
    }
}

impl std::error::Error for ParseError {}

/// The file being parsed, for turning byte spans into `line:col`.
pub(crate) struct Src<'a> {
    pub file: &'a str,
    pub text: &'a str,
}

impl Src<'_> {
    /// 1-based line and column of a byte offset.
    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let before = &self.text[..offset.min(self.text.len())];
        let line = before.matches('\n').count() + 1;
        let col = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
        (line, col)
    }

    pub fn err(&self, span: Range<usize>, kind: ErrorKind, msg: impl Into<String>) -> ParseError {
        let (line, col) = self.line_col(span.start);
        ParseError {
            file: self.file.to_owned(),
            line,
            col,
            kind,
            message: msg.into(),
        }
    }
}
