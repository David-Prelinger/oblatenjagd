//! Diagnostics: parse errors and lint findings share one type so that the
//! build script, `cargo xtask sprites lint` and tests all print the same
//! `file:line:col: error[rule]: message` lines.

use std::fmt;

/// How bad a finding is. Errors fail the build and `sprites lint`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Warning,
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// One problem in an art source, located at `file:line:col` (1-based;
/// `col` 0 means "whole line", `line` 0 means "whole file").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// Short rule id, e.g. `syntax`, `row-width`, `outline`.
    pub rule: &'static str,
    pub file: String,
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl Diagnostic {
    pub fn error(
        rule: &'static str,
        file: &str,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            rule,
            file: file.to_owned(),
            line,
            col,
            message: message.into(),
        }
    }

    pub fn warning(
        rule: &'static str,
        file: &str,
        line: usize,
        col: usize,
        message: impl Into<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            ..Diagnostic::error(rule, file, line, col, message)
        }
    }

    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file)?;
        if self.line > 0 {
            write!(f, ":{}", self.line)?;
            if self.col > 0 {
                write!(f, ":{}", self.col)?;
            }
        }
        write!(
            f,
            ": {}[{}]: {}",
            self.severity.as_str(),
            self.rule,
            self.message
        )
    }
}

impl std::error::Error for Diagnostic {}

/// Splits a line into whitespace-separated tokens with their 1-based char column.
pub(crate) fn tokens(line: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut start: Option<(usize, usize)> = None; // (byte offset, column)
    for (col0, (byte, ch)) in line.char_indices().enumerate() {
        match (ch.is_whitespace(), start) {
            (false, None) => start = Some((byte, col0 + 1)),
            (true, Some((b, c))) => {
                out.push((c, &line[b..byte]));
                start = None;
            }
            _ => {}
        }
    }
    if let Some((b, c)) = start {
        out.push((c, &line[b..]));
    }
    out
}

/// Removes a trailing `# comment` (everything from the first `#`).
pub(crate) fn strip_comment(line: &str) -> &str {
    match line.find('#') {
        Some(i) => &line[..i],
        None => line,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_formats_location_and_rule() {
        let d = Diagnostic::error("syntax", "a.sprite", 3, 7, "bad");
        assert_eq!(d.to_string(), "a.sprite:3:7: error[syntax]: bad");
        let w = Diagnostic::warning("x", "a.sprite", 3, 0, "meh");
        assert_eq!(w.to_string(), "a.sprite:3: warning[x]: meh");
        let f = Diagnostic::error("dup", "a.sprite", 0, 0, "file");
        assert_eq!(f.to_string(), "a.sprite: error[dup]: file");
    }

    #[test]
    fn tokens_have_columns() {
        assert_eq!(tokens("  ab  c d"), vec![(3, "ab"), (7, "c"), (9, "d")]);
        assert!(tokens("   ").is_empty());
    }
}
