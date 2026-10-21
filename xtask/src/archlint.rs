//! Architecture lint (part of `cargo xtask check`), see `docs/architecture.md`.
//!
//! 1. Dependency rules (from `cargo metadata`, normal + build dependencies,
//!    transitively; dev-dependencies are ignored except for rule c):
//!    a. per-crate forbidden packages ([`DEP_RULES`]): `jnr-core`,
//!    `jnr-render` and `jnr-audio` never reach macroquad/miniquad,
//!    `jnr-core` and `jnr-audio` never reach `rand`/`getrandom`, `jnr-audio`
//!    never reaches `quad-snd`, `jnr-render` never reaches `jnr-audio`;
//!    b. workspace dependencies ([`WORKSPACE_DEPS`]): `jnr-core` depends on
//!    no other workspace crate, `jnr-audio` only on `jnr-core`;
//!    c. no package depends on `xtask` (any dependency kind).
//! 2. Determinism rules for the source of every [`DETERMINISTIC_CRATES`]
//!    crate (`crates/<name>/src`, outside test code; jnr-audio also must not
//!    mention `as_float`, `Value::Float` or `toml::Value`, [`CRATE_FORBIDDEN_TEXT`]):
//!    a. no `f32`, `f64`, `HashMap`, `HashSet`, `Instant`, `SystemTime`
//!    (whole words; `1.0f32` and `1_f32` are caught too);
//!    b. no `std::time`, `std::fs`, `std::thread`, `core::time` (as text);
//!    c. no float literals (`0.35`, `256.0`, `1e5`, `2.5E-3`);
//!    d. no `use std::…;`/`use core::…;` statement (also multi-line and
//!    grouped, e.g. `use std::{fs, time::Instant};`) naming the modules
//!    `time`, `fs`, `thread`, `env`, `io`, `net`, `process` or `sync` (except `sync::Arc`).
//! 3. Tooling-only functions ([`TOOLING_ONLY_FNS`], e.g. `Game::teleport_player`)
//!    are called only from `xtask/` and test code: every `crates/*/src` file
//!    is scanned (outside test code) for `name(` calls; the definition
//!    (`fn name(`) is allowed.
//!
//! Remaining limits of the source scan (deliberately simple, line-based):
//! - `//` comments are stripped by cutting at the first `//` on a line, so a
//!   `"//"` inside a string hides the rest of that line. `/* */` block
//!   comments are not stripped (possible false positives).
//! - Tokens and float-looking text inside string literals are reported
//!   (false positives, rare; e.g. `"v1.2"` is fine, `"1.2"` is not).
//! - Test code is: files named `tests.rs`, files under a `tests/` directory
//!   (relative to `src/`), and the item after a `#[cfg(test)]` attribute.
//!   That item ends at its matching `}`, or at a trailing `;`/`,` if no
//!   brace was opened, or when a `}` closes the enclosing block (e.g. a last
//!   enum variant without a comma). Braces in string/char literals and
//!   `#[cfg(test)]` items spanning lines without `{`, `;` or `,` confuse it.
//!   Only the exact text `#[cfg(test)]` counts (not `#[cfg(all(test, …))]`,
//!   which is therefore scanned like normal code). When the attribute and its
//!   item share a line with other code, the rest of that line is not scanned.
//! - Fully qualified paths are matched as text: `::std::fs` and `std :: fs`
//!   are only caught via the `use` rule, and macros generating code are not
//!   expanded. Aliased crates (`extern crate std as s;`) are not caught.
//!   The `code-reviewer` covers what this misses.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// Per-crate forbidden packages (transitive normal/build dependencies).
const DEP_RULES: [(&str, &[&str]); 3] = [
    ("jnr-core", &["macroquad", "miniquad", "rand", "getrandom"]),
    ("jnr-render", &["macroquad", "miniquad", "jnr-audio"]),
    (
        "jnr-audio",
        &["macroquad", "miniquad", "quad-snd", "rand", "getrandom"],
    ),
];
/// Workspace crates and the only workspace crates they may depend on.
const WORKSPACE_DEPS: [(&str, &[&str]); 2] = [("jnr-core", &[]), ("jnr-audio", &["jnr-core"])];
/// Crates under the determinism source scan (rule 2): integers only, no
/// hash-map iteration, no time/fs/threads (D-117 for jnr-audio).
const DETERMINISTIC_CRATES: [&str; 2] = ["jnr-core", "jnr-audio"];
/// Extra text forbidden per crate (substrings, outside test code): the audio
/// parser must never read TOML floats (every number is an integer key).
const CRATE_FORBIDDEN_TEXT: [(&str, &[&str]); 1] =
    [("jnr-audio", &["as_float", "Value::Float", "toml::Value"])];
/// Packages nothing may depend on.
const NO_DEPENDENTS: [&str; 1] = ["xtask"];

/// Identifiers forbidden in [`DETERMINISTIC_CRATES`] (matched as whole words).
const FORBIDDEN_IDENTS: [&str; 6] = ["f32", "f64", "HashMap", "HashSet", "Instant", "SystemTime"];
/// Paths forbidden in [`DETERMINISTIC_CRATES`] (matched as substrings).
const FORBIDDEN_PATHS: [&str; 4] = ["std::time", "std::fs", "std::thread", "core::time"];
/// Functions only xtask and tests may call (rule 3).
const TOOLING_ONLY_FNS: [&str; 1] = ["teleport_player"];

/// Module names forbidden in `use std::…`/`use core::…` statements.
const FORBIDDEN_USE_MODULES: [&str; 8] = [
    "time", "fs", "thread", "env", "io", "net", "process", "sync",
];

/// Runs all architecture checks; returns human-readable problems on failure.
pub fn run(root: &Path) -> Result<(), Vec<String>> {
    let mut problems = Vec::new();

    match cargo_metadata(root) {
        Ok(meta) => problems.extend(dependency_violations(&meta)),
        Err(e) => problems.push(e),
    }

    problems.extend(determinism_violations(root));

    problems.extend(tooling_only_violations(root));

    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems)
    }
}

/// Rule 2 over `crates/<name>/src` of every [`DETERMINISTIC_CRATES`] crate.
pub fn determinism_violations(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for krate in DETERMINISTIC_CRATES {
        let src = root.join("crates").join(krate).join("src");
        let files = match rust_files(&src) {
            Ok(files) => files,
            Err(e) => {
                problems.push(format!("{}: cannot list: {e}", src.display()));
                continue;
            }
        };
        for file in files {
            let rel_to_src = file.strip_prefix(&src).unwrap_or(&file);
            if is_test_file(rel_to_src) {
                continue;
            }
            let rel = file.strip_prefix(root).unwrap_or(&file).display();
            match fs::read_to_string(&file) {
                Ok(text) => {
                    let mut found = scan_source(&text);
                    found.extend(forbidden_text(krate, &text));
                    found.sort_by_key(|(line, _)| *line);
                    for (line, what) in found {
                        problems.push(format!(
                            "{rel}:{line}: {what} is forbidden in {krate} (determinism rules, docs/architecture.md)"
                        ));
                    }
                }
                Err(e) => problems.push(format!("{rel}: cannot read: {e}")),
            }
        }
    }
    problems
}

/// [`CRATE_FORBIDDEN_TEXT`] of `krate` in `text` (outside test code).
fn forbidden_text(krate: &str, text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (name, tokens) in CRATE_FORBIDDEN_TEXT {
        if name != krate {
            continue;
        }
        for (lineno, line) in code_lines(text) {
            for tok in tokens.iter().filter(|t| line.contains(*t)) {
                found.push((lineno, format!("`{tok}`")));
            }
        }
    }
    found
}

fn cargo_metadata(root: &Path) -> Result<Value, String> {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let out = Command::new(cargo)
        .args(["metadata", "--format-version", "1"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("cannot run `cargo metadata`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`cargo metadata --format-version 1` failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("bad `cargo metadata` JSON: {e}"))
}

// ------------------------------------------------------------ dependency rules

/// The resolved dependency graph from `cargo metadata`, keyed by package id.
struct Graph<'a> {
    names: BTreeMap<&'a str, &'a str>,
    /// (dependency id, counts as normal/build dependency)
    edges: BTreeMap<&'a str, Vec<(&'a str, bool)>>,
    members: Vec<&'a str>,
}

impl<'a> Graph<'a> {
    fn from_metadata(meta: &'a Value) -> Graph<'a> {
        let mut names = BTreeMap::new();
        for pkg in meta["packages"].as_array().into_iter().flatten() {
            if let (Some(id), Some(name)) = (pkg["id"].as_str(), pkg["name"].as_str()) {
                names.insert(id, name);
            }
        }
        let mut edges: BTreeMap<&str, Vec<(&str, bool)>> = BTreeMap::new();
        for node in meta["resolve"]["nodes"].as_array().into_iter().flatten() {
            let Some(id) = node["id"].as_str() else {
                continue;
            };
            let deps = edges.entry(id).or_default();
            for dep in node["deps"].as_array().into_iter().flatten() {
                let runtime = dep["dep_kinds"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|k| matches!(k["kind"].as_str(), None | Some("build")));
                if let Some(pkg) = dep["pkg"].as_str() {
                    deps.push((pkg, runtime));
                }
            }
        }
        let members = meta["workspace_members"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        Graph {
            names,
            edges,
            members,
        }
    }

    fn name(&self, id: &'a str) -> &'a str {
        self.names.get(id).copied().unwrap_or(id)
    }

    fn member(&self, name: &str) -> Option<&'a str> {
        self.members
            .iter()
            .copied()
            .find(|id| self.name(id) == name)
    }

    /// BFS over normal/build edges; maps every reachable id to its parent.
    fn reachable(&self, start: &'a str) -> BTreeMap<&'a str, Option<&'a str>> {
        let mut parent = BTreeMap::from([(start, None)]);
        let mut queue = VecDeque::from([start]);
        while let Some(id) = queue.pop_front() {
            for &(dep, runtime) in self.edges.get(id).into_iter().flatten() {
                if runtime && !parent.contains_key(dep) {
                    parent.insert(dep, Some(id));
                    queue.push_back(dep);
                }
            }
        }
        parent
    }

    fn path(&self, parent: &BTreeMap<&'a str, Option<&'a str>>, id: &'a str) -> String {
        let mut path = vec![self.name(id)];
        let mut cur = id;
        while let Some(Some(p)) = parent.get(cur) {
            path.push(self.name(p));
            cur = p;
        }
        path.reverse();
        path.join(" -> ")
    }
}

/// Checks [`DEP_RULES`], [`WORKSPACE_DEPS`] and [`NO_DEPENDENTS`].
pub fn dependency_violations(meta: &Value) -> Vec<String> {
    let graph = Graph::from_metadata(meta);
    let mut problems = Vec::new();

    for (krate, forbidden) in DEP_RULES {
        let Some(start) = graph.member(krate) else {
            problems.push(format!("workspace crate `{krate}` not found"));
            continue;
        };
        let parent = graph.reachable(start);
        for &id in parent.keys() {
            let name = graph.name(id);
            if forbidden.contains(&name) {
                problems.push(format!(
                    "`{krate}` must not depend on `{name}`: {}",
                    graph.path(&parent, id)
                ));
            }
        }
    }

    for (krate, allowed) in WORKSPACE_DEPS {
        let Some(start) = graph.member(krate) else {
            problems.push(format!("workspace crate `{krate}` not found"));
            continue;
        };
        let parent = graph.reachable(start);
        for &id in parent.keys() {
            if id != start && graph.members.contains(&id) && !allowed.contains(&graph.name(id)) {
                problems.push(format!(
                    "`{krate}` must not depend on workspace crate `{}`: {}",
                    graph.name(id),
                    graph.path(&parent, id)
                ));
            }
        }
    }

    for (&from, deps) in &graph.edges {
        for &(dep, _) in deps {
            let name = graph.name(dep);
            if NO_DEPENDENTS.contains(&name) {
                problems.push(format!(
                    "`{}` must not depend on `{name}` (xtask depends on everything, never the other way round)",
                    graph.name(from)
                ));
            }
        }
    }
    problems
}

// ------------------------------------------------------- tooling-only calls

/// Rule 3 over every `crates/*/src` file (test files and test items skipped).
fn tooling_only_violations(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    let crates = root.join("crates");
    let mut dirs: Vec<PathBuf> = match fs::read_dir(&crates) {
        Ok(entries) => entries.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(e) => return vec![format!("{}: cannot list: {e}", crates.display())],
    };
    dirs.sort();
    for src in dirs.iter().map(|d| d.join("src")).filter(|s| s.is_dir()) {
        let files = match rust_files(&src) {
            Ok(f) => f,
            Err(e) => {
                problems.push(format!("{}: cannot list: {e}", src.display()));
                continue;
            }
        };
        for file in files {
            if is_test_file(file.strip_prefix(&src).unwrap_or(&file)) {
                continue;
            }
            let rel = file.strip_prefix(root).unwrap_or(&file).display();
            match fs::read_to_string(&file) {
                Ok(text) => {
                    for (line, name) in tooling_only_calls(&text) {
                        problems.push(format!(
                            "{rel}:{line}: `{name}` is tooling-only (callable from xtask/ and tests only)"
                        ));
                    }
                }
                Err(e) => problems.push(format!("{rel}: cannot read: {e}")),
            }
        }
    }
    problems
}

/// Calls of [`TOOLING_ONLY_FNS`] outside test code: (line, function name).
/// The definition `fn name(` is not a call.
pub fn tooling_only_calls(text: &str) -> Vec<(usize, &'static str)> {
    let mut found = Vec::new();
    for (lineno, line) in code_lines(text) {
        for name in TOOLING_ONLY_FNS {
            let call = format!("{name}(");
            let def = format!("fn {name}(");
            if line.replace(&def, "").contains(&call) {
                found.push((lineno, name));
            }
        }
    }
    found
}

// ----------------------------------------------------------------- source scan

/// All `.rs` files below `dir`, sorted.
fn rust_files(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files.extend(rust_files(&path)?);
        } else if path.extension().is_some_and(|e| e == "rs") {
            files.push(path);
        }
    }
    Ok(files)
}

/// `rel` is relative to the crate's `src/` directory.
fn is_test_file(rel: &Path) -> bool {
    rel.file_name().is_some_and(|n| n == "tests.rs")
        || rel.components().any(|c| c.as_os_str() == "tests")
}

/// State while skipping the item after `#[cfg(test)]`.
struct Skip {
    depth: i32,
    opened: bool,
}

enum SkipResult {
    /// The skipped item continues on the next line.
    Continue,
    /// The skipped item ended on this line.
    Ended,
    /// A `}` closed the enclosing block: the skip ended and this line
    /// belongs to normal code again.
    LeftBlock,
}

impl Skip {
    fn feed(&mut self, line: &str) -> SkipResult {
        for c in line.chars() {
            match c {
                '{' => {
                    self.depth += 1;
                    self.opened = true;
                }
                '}' => {
                    self.depth -= 1;
                    if self.depth < 0 {
                        return SkipResult::LeftBlock;
                    }
                }
                _ => {}
            }
        }
        let trimmed = line.trim_end();
        let done = if self.opened {
            self.depth <= 0
        } else {
            trimmed.ends_with(';') || trimmed.ends_with(',')
        };
        if done {
            SkipResult::Ended
        } else {
            SkipResult::Continue
        }
    }
}

/// The code lines of Rust source outside `#[cfg(test)]` items, with `//`
/// comments cut off: (1-based line number, code).
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut skip: Option<Skip> = None;
    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        let mut line = raw.split("//").next().unwrap_or("");

        if let Some(s) = &mut skip {
            match s.feed(line) {
                SkipResult::Continue => continue,
                SkipResult::Ended => {
                    skip = None;
                    continue;
                }
                SkipResult::LeftBlock => skip = None,
            }
        }

        if let Some(pos) = line.find("#[cfg(test)]") {
            let rest = &line[pos + "#[cfg(test)]".len()..];
            let mut s = Skip {
                depth: 0,
                opened: false,
            };
            let result = if rest.trim().is_empty() {
                SkipResult::Continue
            } else {
                s.feed(rest)
            };
            match result {
                SkipResult::Continue => {
                    skip = Some(s);
                    continue;
                }
                // The test-only item ends on this line: only the code before
                // the attribute is scanned (anything after the item is not).
                SkipResult::Ended | SkipResult::LeftBlock => line = &line[..pos],
            }
        }
        out.push((lineno, line));
    }
    out
}

/// Scans Rust source for forbidden constructs outside `#[cfg(test)]` items.
/// Returns (1-based line number, description) pairs.
pub fn scan_source(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    // A `use std::…`/`use core::…` statement being collected: (start line, text).
    let mut pending_use: Option<(usize, String)> = None;

    for (lineno, line) in code_lines(text) {
        scan_line(line, lineno, &mut found);

        if pending_use.is_none() && starts_std_use(line) {
            pending_use = Some((lineno, String::new()));
        }
        if let Some((start, text)) = &mut pending_use {
            text.push_str(line);
            text.push(' ');
            if line.contains(';') {
                for module in forbidden_use_modules(text) {
                    found.push((*start, format!("`use` of std/core module `{module}`")));
                }
                pending_use = None;
            }
        }
    }
    found
}

fn scan_line(line: &str, lineno: usize, found: &mut Vec<(usize, String)>) {
    for tok in FORBIDDEN_IDENTS {
        if contains_word(line, tok) {
            found.push((lineno, format!("`{tok}`")));
        }
    }
    for tok in FORBIDDEN_PATHS {
        if line.contains(tok) {
            found.push((lineno, format!("`{tok}`")));
        }
    }
    if let Some(lit) = find_float_literal(line) {
        found.push((lineno, format!("float literal `{lit}`")));
    }
}

/// True if the line starts a `use` (or `pub use`, `pub(crate) use`) of `std`/`core`.
fn starts_std_use(line: &str) -> bool {
    let mut t = line.trim_start();
    if let Some(rest) = t.strip_prefix("pub") {
        t = rest.trim_start();
        if t.starts_with('(') {
            t = t.split_once(')').map_or("", |(_, r)| r).trim_start();
        }
    }
    let Some(rest) = t.strip_prefix("use ") else {
        return false;
    };
    let path = rest.trim_start().trim_start_matches("::");
    path.starts_with("std") || path.starts_with("core")
}

/// Forbidden module names mentioned in a complete `use std::…;` statement.
/// `sync::Arc` is allowed (immutable sharing, no global state); any other
/// `sync` item (Mutex, atomics, …) is still reported.
fn forbidden_use_modules(statement: &str) -> Vec<&'static str> {
    let statement = without_allowed_arc(statement);
    let words: BTreeSet<&str> = statement
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .collect();
    FORBIDDEN_USE_MODULES
        .into_iter()
        .filter(|m| words.contains(m))
        .collect()
}

/// Removes `sync::Arc` items (not e.g. `sync::ArcSwap`) from a `use` statement.
fn without_allowed_arc(statement: &str) -> String {
    let mut out = String::new();
    let mut rest = statement;
    while let Some(i) = rest.find("sync::Arc") {
        let end = i + "sync::Arc".len();
        let next = rest[end..].chars().next();
        out.push_str(&rest[..i]);
        if next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
            out.push_str("sync::Arc");
        }
        rest = &rest[end..];
    }
    out.push_str(rest);
    out
}

/// True if `word` occurs in `line` not preceded by a letter and not followed by
/// an identifier character. Digits/underscore before are allowed so that
/// literal suffixes like `1.0f32` and `1_f32` are caught.
fn contains_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(i, _)| {
        let before = line[..i].chars().next_back();
        let after = line[i + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphabetic())
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
    })
}

/// Returns the first float literal in `line` (`1.5`, `256.0`, `1e5`, `2.5E-3`).
/// Numbers preceded by `.` or an identifier character are ignored, so tuple
/// access (`t.0.1`), ranges (`0..1`) and names (`x1`) are not reported.
fn find_float_literal(line: &str) -> Option<&str> {
    let b = line.as_bytes();
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut i = 0;
    while i < b.len() {
        if !b[i].is_ascii_digit() || (i > 0 && (is_ident(b[i - 1]) || b[i - 1] == b'.')) {
            i += 1;
            continue;
        }
        let start = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
            i += 1;
        }
        let mut float = false;
        if i + 1 < b.len() && b[i] == b'.' && b[i + 1].is_ascii_digit() {
            float = true;
            i += 1;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
                i += 1;
            }
        }
        if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
            let mut j = i + 1;
            if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
                j += 1;
            }
            if j < b.len() && b[j].is_ascii_digit() {
                float = true;
                i = j;
                while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'_') {
                    i += 1;
                }
            }
        }
        if float {
            return Some(&line[start..i]);
        }
        // Skip the rest of this token (e.g. `0x1e5`, `10u8`).
        while i < b.len() && is_ident(b[i]) {
            i += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn f(line: usize, what: &str) -> (usize, String) {
        (line, what.to_owned())
    }

    #[test]
    fn finds_forbidden_tokens() {
        let src = "pub struct P { x: f32 }\nuse std::collections::HashMap;\nlet t = std::time::Instant::now();\nlet a = 1f64;\n";
        assert_eq!(
            scan_source(src),
            vec![
                f(1, "`f32`"),
                f(2, "`HashMap`"),
                f(3, "`Instant`"),
                f(3, "`std::time`"),
                f(4, "`f64`"),
            ]
        );
    }

    #[test]
    fn tooling_only_fns_are_flagged_outside_tests() {
        let src = "\
pub fn teleport_player(&mut self, x: i32) {}
fn play(g: &mut Game) {
    g.teleport_player(10); // a call in normal code
    // g.teleport_player(1) in a comment is fine
}
#[cfg(test)]
mod tests {
    fn t(g: &mut Game) { g.teleport_player(3); }
}
";
        assert_eq!(tooling_only_calls(src), vec![(3, "teleport_player")]);
    }

    #[test]
    fn real_crates_call_tooling_only_fns_only_from_tests() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(tooling_only_violations(&root), Vec::<String>::new());
    }

    #[test]
    fn ignores_comments_and_similar_identifiers() {
        let src = "// f32 is forbidden\n/// no HashMap here\nlet x = 1; // f64 0.5\nstruct MyHashMapLike;\nlet f32x = 0;\n";
        assert!(scan_source(src).is_empty());
    }

    #[test]
    fn skips_cfg_test_blocks() {
        let src = "\
pub fn a() {}
#[cfg(test)]
mod tests {
    fn helper() {
        let x: f32 = 1.0;
    }
}
pub fn b(v: f64) {}
#[cfg(test)]
mod more_tests;
let bad = std::thread::current();
#[cfg(test)]
#[allow(dead_code)]
fn only_in_tests(
    x: f32,
) {
}
let t: f32 = 0;
";
        assert_eq!(
            scan_source(src),
            vec![f(8, "`f64`"), f(11, "`std::thread`"), f(18, "`f32`")]
        );
    }

    #[test]
    fn cfg_test_on_field_ends_at_comma() {
        let src = "\
pub struct Player {
    #[cfg(test)]
    debug_tag: u8,
    pos: f32,
}
";
        assert_eq!(scan_source(src), vec![f(4, "`f32`")]);
    }

    #[test]
    fn cfg_test_on_enum_variant_and_inline() {
        let src = "\
pub enum Kind {
    #[cfg(test)]
    TestOnly,
    Real(f32),
}
pub enum Last {
    A(f64),
    #[cfg(test)]
    TestOnly
}
let x: f32 = 0;
pub struct S { y: HashSet<u8>, #[cfg(test)] t: f32 }
";
        assert_eq!(
            scan_source(src),
            vec![
                f(4, "`f32`"),
                f(7, "`f64`"),
                f(11, "`f32`"),
                f(12, "`HashSet`")
            ]
        );
    }

    #[test]
    fn cfg_test_match_arm_ends_at_comma() {
        let src = "\
match x {
    #[cfg(test)]
    9 => 1,
    _ => 2.5 as i32,
}
";
        assert_eq!(scan_source(src), vec![f(4, "float literal `2.5`")]);
    }

    #[test]
    fn finds_float_literals() {
        assert_eq!(
            find_float_literal("let a = (0.35 * 256.0) as i32;"),
            Some("0.35")
        );
        assert_eq!(find_float_literal("x = 3.0 / 4.0"), Some("3.0"));
        assert_eq!(find_float_literal("let big = 1e5;"), Some("1e5"));
        assert_eq!(find_float_literal("let s = 2.5E-3;"), Some("2.5E-3"));
        assert_eq!(find_float_literal("let s = 1_000.5;"), Some("1_000.5"));
        assert_eq!(
            scan_source("const G: i32 = (0.35 * 256.0) as i32;\n"),
            vec![f(1, "float literal `0.35`")]
        );
    }

    #[test]
    fn ignores_tuple_access_ranges_and_integers() {
        for ok in [
            "let v = t.0.1;",
            "for i in 0..10 {}",
            "let r = 0..=1;",
            "let h = 0x1e5;",
            "let n = 10u8 + x1 + 256;",
            "let p = pair.0 + 1;",
            "let s = &a[1..];",
        ] {
            assert_eq!(find_float_literal(ok), None, "{ok}");
        }
    }

    #[test]
    fn allows_only_arc_from_sync() {
        assert!(forbidden_use_modules("use std::sync::Arc;").is_empty());
        assert!(forbidden_use_modules("use std::{fmt, sync::Arc};").is_empty());
        assert_eq!(
            forbidden_use_modules("use std::sync::{Arc, Mutex};"),
            ["sync"]
        );
        assert_eq!(forbidden_use_modules("use std::sync::ArcSwap;"), ["sync"]);
        assert_eq!(
            forbidden_use_modules("use core::sync::atomic::AtomicU32;"),
            ["sync"]
        );
    }

    #[test]
    fn forbids_std_modules_in_use_statements() {
        let src = "\
use std::{fs, time::Duration};
use std::collections::BTreeMap;
use core::{
    cell::Cell,
    sync::atomic::AtomicU32,
};
pub(crate) use std::io::Write;
use std::env as e;
use crate::time::Clock;
let d = core::time::Duration::ZERO;
";
        assert_eq!(
            scan_source(src),
            vec![
                f(1, "`use` of std/core module `time`"),
                f(1, "`use` of std/core module `fs`"),
                f(3, "`use` of std/core module `sync`"),
                f(7, "`use` of std/core module `io`"),
                f(8, "`use` of std/core module `env`"),
                f(10, "`core::time`"),
            ]
        );
    }

    #[test]
    fn test_files_are_recognised_relative_to_src() {
        assert!(is_test_file(Path::new("physics/tests.rs")));
        assert!(is_test_file(Path::new("tests/feel.rs")));
        assert!(!is_test_file(Path::new("physics.rs")));
        // A checkout below a directory named `tests` must not hide sources.
        let src = Path::new("/home/tests/jumpnrun/crates/jnr-core/src");
        let file = src.join("physics.rs");
        assert!(!is_test_file(file.strip_prefix(src).unwrap()));
    }

    /// (package id, [(dependency id, dependency kind)])
    type Node<'a> = (&'a str, &'a [(&'a str, Option<&'a str>)]);

    /// Synthetic `cargo metadata`. Each node is (id, [(dep, kind)]), kind
    /// `None` = normal; package names equal ids.
    fn metadata(members: &[&str], nodes: &[Node]) -> Value {
        json!({
            "packages": nodes.iter().map(|(id, _)| json!({"id": id, "name": id})).collect::<Vec<_>>(),
            "workspace_members": members,
            "resolve": {"nodes": nodes.iter().map(|(id, deps)| json!({
                "id": id,
                "deps": deps.iter().map(|(d, k)| json!({"pkg": d, "dep_kinds": [{"kind": k}]})).collect::<Vec<_>>(),
            })).collect::<Vec<_>>()},
        })
    }

    const MEMBERS: [&str; 5] = ["jnr-core", "jnr-render", "jnr-audio", "jnr-app", "xtask"];

    #[test]
    fn dependency_rules_pass_for_the_intended_layout() {
        let meta = metadata(
            &MEMBERS,
            &[
                ("jnr-core", &[("fixed", None), ("macroquad", Some("dev"))]),
                ("jnr-render", &[("jnr-core", None)]),
                ("jnr-audio", &[]),
                ("jnr-app", &[("jnr-render", None), ("macroquad", None)]),
                ("xtask", &[("jnr-core", None)]),
                ("fixed", &[]),
                ("macroquad", &[]),
            ],
        );
        assert_eq!(dependency_violations(&meta), Vec::<String>::new());
    }

    #[test]
    fn dependency_rules_find_transitive_build_dependency() {
        let meta = metadata(
            &MEMBERS,
            &[
                ("jnr-core", &[]),
                ("jnr-render", &[("jnr-core", None), ("gen", Some("build"))]),
                ("jnr-audio", &[]),
                ("jnr-app", &[]),
                ("xtask", &[]),
                ("gen", &[("macroquad", None)]),
                ("macroquad", &[]),
            ],
        );
        assert_eq!(
            dependency_violations(&meta),
            vec!["`jnr-render` must not depend on `macroquad`: jnr-render -> gen -> macroquad"]
        );
    }

    #[test]
    fn core_forbids_rand_and_workspace_crates() {
        let meta = metadata(
            &MEMBERS,
            &[
                ("jnr-core", &[("helper", None), ("jnr-render", None)]),
                ("jnr-render", &[]),
                ("jnr-audio", &[]),
                ("jnr-app", &[]),
                ("xtask", &[]),
                ("helper", &[("getrandom", None)]),
                ("getrandom", &[]),
            ],
        );
        assert_eq!(
            dependency_violations(&meta),
            vec![
                "`jnr-core` must not depend on `getrandom`: jnr-core -> helper -> getrandom",
                "`jnr-core` must not depend on workspace crate `jnr-render`: jnr-core -> jnr-render",
            ]
        );
    }

    #[test]
    fn nothing_may_depend_on_xtask() {
        let meta = metadata(
            &MEMBERS,
            &[
                ("jnr-core", &[]),
                ("jnr-render", &[]),
                ("jnr-audio", &[]),
                ("jnr-app", &[("xtask", Some("dev"))]),
                ("xtask", &[]),
            ],
        );
        assert_eq!(
            dependency_violations(&meta),
            vec![
                "`jnr-app` must not depend on `xtask` (xtask depends on everything, never the other way round)"
            ]
        );
    }

    #[test]
    fn dependency_rules_report_missing_crate() {
        let meta = metadata(&["jnr-render"], &[("jnr-render", &[])]);
        assert_eq!(
            dependency_violations(&meta),
            vec![
                "workspace crate `jnr-core` not found",
                "workspace crate `jnr-audio` not found",
                "workspace crate `jnr-core` not found",
                "workspace crate `jnr-audio` not found",
            ]
        );
    }

    #[test]
    fn audio_may_reach_core_only_and_render_never_reaches_audio() {
        let meta = metadata(
            &MEMBERS,
            &[
                ("jnr-core", &[]),
                ("jnr-render", &[("jnr-audio", None)]),
                (
                    "jnr-audio",
                    &[("jnr-core", None), ("jnr-render", None), ("quad-snd", None)],
                ),
                ("jnr-app", &[]),
                ("xtask", &[]),
                ("quad-snd", &[]),
            ],
        );
        assert_eq!(
            dependency_violations(&meta),
            vec![
                "`jnr-render` must not depend on `jnr-audio`: jnr-render -> jnr-audio",
                "`jnr-audio` must not depend on `quad-snd`: jnr-audio -> quad-snd",
                "`jnr-audio` must not depend on workspace crate `jnr-render`: jnr-audio -> jnr-render",
            ]
        );
    }

    /// A throwaway workspace root with one source file per (crate, text).
    fn fake_root(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("xtask-archlint-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        for krate in DETERMINISTIC_CRATES {
            fs::create_dir_all(root.join("crates").join(krate).join("src")).unwrap();
        }
        for (path, text) in files {
            fs::write(root.join(path), text).unwrap();
        }
        root
    }

    #[test]
    fn an_f32_or_a_toml_float_in_jnr_audio_fails_the_lint() {
        let root = fake_root(
            "audio-f32",
            &[
                ("crates/jnr-core/src/lib.rs", "pub fn ok() -> i32 { 1 }\n"),
                (
                    "crates/jnr-audio/src/synth.rs",
                    "pub fn gain(x: i32) -> i32 {\n    (x as f32 * 0.5) as i32\n}\n#[cfg(test)]\nmod tests { fn t() -> f64 { 1.0 } }\n",
                ),
                (
                    "crates/jnr-audio/src/patch.rs",
                    "fn f(v: &toml::Value) -> bool {\n    v.as_float().is_some()\n}\n",
                ),
            ],
        );
        let found = determinism_violations(&root);
        let _ = fs::remove_dir_all(&root);
        assert_eq!(
            found,
            vec![
                "crates/jnr-audio/src/patch.rs:1: `toml::Value` is forbidden in jnr-audio (determinism rules, docs/architecture.md)",
                "crates/jnr-audio/src/patch.rs:2: `as_float` is forbidden in jnr-audio (determinism rules, docs/architecture.md)",
                "crates/jnr-audio/src/synth.rs:2: `f32` is forbidden in jnr-audio (determinism rules, docs/architecture.md)",
                "crates/jnr-audio/src/synth.rs:2: float literal `0.5` is forbidden in jnr-audio (determinism rules, docs/architecture.md)",
            ]
        );
    }

    #[test]
    fn the_real_deterministic_crates_pass() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        assert_eq!(determinism_violations(&root), Vec::<String>::new());
    }
}
