//! Step results and the JSON/Markdown reports agents read.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// Lines kept from the start and from the end of a failed step's output.
pub const EXCERPT_LINES: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Pass,
    Fail,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Pass => "pass",
            Status::Fail => "fail",
        }
    }
}

/// The result of one validation step.
#[derive(Clone, Debug)]
pub struct Step {
    pub name: String,
    pub status: Status,
    pub duration_ms: u64,
    /// Exact shell command that reproduces this step.
    pub command: String,
    /// Free-form note, e.g. a short result summary.
    pub note: String,
    /// First and last [`EXCERPT_LINES`] lines of output (only kept on failure).
    pub output_tail: String,
}

impl Step {
    fn to_json(&self) -> Value {
        json!({
            "name": self.name,
            "status": self.status.as_str(),
            "duration_ms": self.duration_ms,
            "command": self.command,
            "note": self.note,
            "output_tail": self.output_tail,
        })
    }
}

/// Returns the first `n` and the last `n` lines of `text`, with a marker
/// for the omitted middle. Short text is returned unchanged.
pub fn excerpt(text: &str, n: usize) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= 2 * n {
        return lines.join("\n");
    }
    let omitted = lines.len() - 2 * n;
    format!(
        "{}\n... ({omitted} lines omitted) ...\n{}",
        lines[..n].join("\n"),
        lines[lines.len() - n..].join("\n")
    )
}

pub struct Report {
    pub command: String,
    pub duration_ms: u64,
    pub steps: Vec<Step>,
}

impl Report {
    pub fn passed(&self) -> bool {
        self.steps.iter().all(|s| s.status != Status::Fail)
    }

    pub fn to_json(&self) -> Value {
        json!({
            "command": self.command,
            "status": if self.passed() { "pass" } else { "fail" },
            "duration_ms": self.duration_ms,
            "steps": self.steps.iter().map(Step::to_json).collect::<Vec<_>>(),
        })
    }

    pub fn to_markdown(&self) -> String {
        let mut md = format!(
            "# cargo xtask {}: {}\n\nTotal: {:.1}s\n\n| Step | Status | Time | Command |\n|---|---|---|---|\n",
            self.command,
            if self.passed() { "PASS" } else { "FAIL" },
            self.duration_ms as f64 / 1000.0
        );
        for s in &self.steps {
            let status = s.status.as_str();
            md.push_str(&format!(
                "| {} | {} | {:.1}s | `{}` |\n",
                s.name,
                status,
                s.duration_ms as f64 / 1000.0,
                s.command
            ));
        }
        for s in self.steps.iter().filter(|s| s.status == Status::Fail) {
            md.push_str(&format!(
                "\n## {} failed\n\nReproduce: `{}`\n\n```\n{}\n```\n",
                s.name, s.command, s.output_tail
            ));
        }
        md
    }

    /// Writes `<dir>/<command>.json` and `<dir>/<command>.md`.
    pub fn write(&self, dir: &Path) -> std::io::Result<(PathBuf, PathBuf)> {
        fs::create_dir_all(dir)?;
        let json_path = dir.join(format!("{}.json", self.command));
        let md_path = dir.join(format!("{}.md", self.command));
        let json = serde_json::to_string_pretty(&self.to_json()).expect("report is valid JSON");
        fs::write(&json_path, json + "\n")?;
        fs::write(&md_path, self.to_markdown())?;
        Ok((json_path, md_path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(name: &str, status: Status) -> Step {
        Step {
            name: name.to_owned(),
            status,
            duration_ms: 1500,
            command: format!("cargo {name}"),
            note: String::new(),
            output_tail: "boom".to_owned(),
        }
    }

    #[test]
    fn excerpt_keeps_head_and_tail() {
        assert_eq!(excerpt("a\nb\nc\nd", 2), "a\nb\nc\nd");
        assert_eq!(
            excerpt("a\nb\nc\nd\ne\nf", 2),
            "a\nb\n... (2 lines omitted) ...\ne\nf"
        );
        assert_eq!(excerpt("a", 20), "a");
        assert_eq!(excerpt("", 20), "");
    }

    #[test]
    fn report_fails_if_any_step_failed() {
        let report = Report {
            command: "check".to_owned(),
            duration_ms: 10,
            steps: vec![
                step("fmt", Status::Pass),
                step("clippy", Status::Fail),
                step("solver", Status::Pass),
            ],
        };
        assert!(!report.passed());
        let json = report.to_json();
        assert_eq!(json["status"], "fail");
        assert_eq!(json["steps"][1]["status"], "fail");
        assert_eq!(json["steps"][1]["command"], "cargo clippy");
        assert_eq!(json["steps"][2]["status"], "pass");
        let md = report.to_markdown();
        assert!(md.contains("## clippy failed"));
        assert!(md.contains("Reproduce: `cargo clippy`"));
    }
}
