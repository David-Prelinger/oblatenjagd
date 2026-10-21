//! Cue logs (MA, D-113): the sound cues of a replay as text, one
//! `frame cue` line each, from `jnr_audio::cues::Director` (pure, so a
//! replay's log is deterministic).
//!
//! - `cargo xtask replay <file> --cue-log` prints a replay's log.
//! - `cargo xtask cues` (the `cue-logs` verify step) compares the log of
//!   every replay that has a golden `tests/golden/cues/<replay>.cues`; on a
//!   mismatch it writes the actual log to
//!   `$CARGO_TARGET_DIR/reports/cues/<replay>.cues` and names the first
//!   differing line.
//! - `cargo xtask cues --accept <replay>|--all` writes the current logs
//!   (a new replay name adds a golden), only from replays that pass their
//!   expectations. Cue logs are text: no reviewer gate, but a commit that
//!   changes one says why (docs/validation.md L6b, D-121).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use jnr_audio::cues::{Director, log_line};
use jnr_core::Level;

use crate::content;

pub const HELP: &str = "\
cargo xtask cues                    compare the cue log of every replay with a golden
                                    tests/golden/cues/<replay>.cues
cargo xtask cues --accept <replay>  write the current cue log of <replay> (adds a golden)
cargo xtask cues --accept --all     rewrite every existing cue-log golden
cargo xtask replay <file> --cue-log print a replay's cue log
";

pub fn run(root: &Path, args: &[String]) -> ExitCode {
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let result = match args.as_slice() {
        [] => {
            let (ok, text) = check_report(root);
            print!("{text}");
            if ok { Ok(()) } else { Err(String::new()) }
        }
        ["--accept", "--all"] => accept_all(root),
        ["--accept", name] => accept(root, name),
        ["help" | "-h" | "--help"] => {
            print!("{HELP}");
            Ok(())
        }
        _ => Err(format!("unknown arguments: {}\n\n{HELP}", args.join(" "))),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            if !msg.is_empty() {
                eprintln!("error: {msg}");
            }
            ExitCode::FAILURE
        }
    }
}

/// `tests/golden/cues`.
pub fn cues_dir(root: &Path) -> PathBuf {
    content::golden_dir(root).join("cues")
}

/// The cue log of the replay at `path`: a header line, then one line per
/// cue (frame 0 = the game's first frame, before any step).
pub fn cue_log(root: &Path, path: &Path) -> Result<String, String> {
    let replay = content::load_replay(path)?;
    let levels: Vec<Level> = content::all_levels(root)?
        .into_iter()
        .map(|(p, l)| l.map_err(|e| format!("{}: {e}", p.display())))
        .collect::<Result<_, _>>()?;
    let tuning = content::tuning(root)?;
    let mut game = replay
        .new_game(&levels, &tuning)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let name = content::replay_name(path);
    let mut text = format!("# cue log of tests/replays/{name}.replay (cargo xtask cues)\n");
    let mut director = Director::new();
    for cue in director.begin(&game) {
        text.push_str(&log_line(game.frame(), &cue));
        text.push('\n');
    }
    for input in replay.frames() {
        game.step(input);
        for cue in director.step(&game, input) {
            text.push_str(&log_line(game.frame(), &cue));
            text.push('\n');
        }
    }
    Ok(text)
}

/// The goldens: (replay name, golden path), sorted.
fn goldens(root: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    Ok(content::files_with_ext(&cues_dir(root), "cues")?
        .into_iter()
        .map(|p| (content::replay_name(&p), p))
        .collect())
}

/// The first line where `a` and `b` differ (1-based) and both lines.
pub fn first_difference(a: &str, b: &str) -> Option<(usize, String, String)> {
    let (mut la, mut lb) = (a.lines(), b.lines());
    for n in 1.. {
        match (la.next(), lb.next()) {
            (None, None) => return None,
            (x, y) if x == y => {}
            (x, y) => {
                let show = |l: Option<&str>| l.unwrap_or("<end>").to_owned();
                return Some((n, show(x), show(y)));
            }
        }
    }
    None
}

/// Compares every golden; returns (all equal, printable text).
pub fn check_report(root: &Path) -> (bool, String) {
    match check(root) {
        Ok(r) => r,
        Err(e) => (false, format!("error: {e}\n")),
    }
}

fn check(root: &Path) -> Result<(bool, String), String> {
    let report_dir = crate::target_dir(root).join("reports/cues");
    if report_dir.exists() {
        fs::remove_dir_all(&report_dir)
            .map_err(|e| format!("clean {}: {e}", report_dir.display()))?;
    }
    let goldens = goldens(root)?;
    if goldens.is_empty() {
        return Ok((
            false,
            format!("FAIL: no cue-log goldens in {}\n", cues_dir(root).display()),
        ));
    }
    let results = crate::par::map_ordered(&goldens, crate::par::threads(), |(name, golden)| {
        let replay = content::resolve_replay(root, name)?;
        let actual = cue_log(root, &replay)?;
        let expected = content::read(golden)?;
        Ok::<_, String>((actual, expected))
    });
    let mut text = String::new();
    let mut failed = 0;
    for ((name, _), result) in goldens.iter().zip(results) {
        let problem = match result {
            Err(e) => Some(e),
            Ok((actual, expected)) => match first_difference(&actual, &expected) {
                None => None,
                Some((line, got, want)) => {
                    fs::create_dir_all(&report_dir)
                        .map_err(|e| format!("create {}: {e}", report_dir.display()))?;
                    let path = report_dir.join(format!("{name}.cues"));
                    fs::write(&path, &actual)
                        .map_err(|e| format!("write {}: {e}", path.display()))?;
                    Some(format!(
                        "line {line}: got `{got}`, golden `{want}`\n     actual log: {} \
                         (accept: `cargo xtask cues --accept {name}`)",
                        path.display()
                    ))
                }
            },
        };
        match problem {
            None => text.push_str(&format!("ok   {name}\n")),
            Some(p) => {
                failed += 1;
                text.push_str(&format!("FAIL {name}: {p}\n"));
            }
        }
    }
    text.push_str(&format!(
        "cue logs: {} compared, {failed} failed\n",
        goldens.len()
    ));
    Ok((failed == 0, text))
}

fn accept(root: &Path, name: &str) -> Result<(), String> {
    let replay = content::resolve_replay(root, name)?;
    let name = content::replay_name(&replay);
    write_golden(root, &name, &replay)
}

fn accept_all(root: &Path) -> Result<(), String> {
    for (name, _) in goldens(root)? {
        let replay = content::resolve_replay(root, &name)?;
        write_golden(root, &name, &replay)?;
    }
    Ok(())
}

/// Cue logs are only accepted from replays that pass (expectations and
/// determinism), like golden screenshots.
fn write_golden(root: &Path, name: &str, replay: &Path) -> Result<(), String> {
    let played = crate::replay::play(root, replay, &[])?;
    if !played.failures.is_empty() {
        return Err(format!(
            "refusing to accept the cue log of {name}: the replay fails: {}",
            played.failures.join("; ")
        ));
    }
    let text = cue_log(root, replay)?;
    let dir = cues_dir(root);
    fs::create_dir_all(&dir).map_err(|e| format!("create {}: {e}", dir.display()))?;
    let path = dir.join(format!("{name}.cues"));
    fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))?;
    println!("accepted {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
    }

    #[test]
    fn first_difference_names_the_line() {
        assert_eq!(first_difference("a\nb\n", "a\nb\n"), None);
        assert_eq!(
            first_difference("a\nb\n", "a\nc\n"),
            Some((2, "b".into(), "c".into()))
        );
        assert_eq!(
            first_difference("a\n", "a\nc\n"),
            Some((2, "<end>".into(), "c".into()))
        );
    }

    /// The director timeline of `testbed_route` (E58 criterion): a `jump`
    /// on the frame of each of its 7 `RBJ` presses, and nothing for the
    /// test level's missing music but the fanfare at the goal.
    #[test]
    fn testbed_route_has_a_jump_cue_per_jump_press() {
        let root = root();
        let path = content::resolve_replay(&root, "testbed_route").unwrap();
        let log = cue_log(&root, &path).unwrap();
        let replay = content::load_replay(&path).unwrap();
        // Frames (1-based step numbers) where JUMP is newly pressed.
        let mut presses = Vec::new();
        let mut held = false;
        for (i, input) in replay.frames().enumerate() {
            if input.jump() && !held {
                presses.push(i as u32 + 1);
            }
            held = input.jump();
        }
        assert_eq!(presses.len(), 7);
        let jumps: Vec<u32> = log
            .lines()
            .filter(|l| l.ends_with(" sfx jump P4"))
            .map(|l| l.split_whitespace().next().unwrap().parse().unwrap())
            .collect();
        assert_eq!(jumps, presses, "{log}");
        assert!(log.contains(" jingle fanfare_clear\n"), "{log}");
        assert!(!log.contains(" song "), "the testbed has no music: {log}");
    }

    #[test]
    fn accept_refuses_a_failing_replay() {
        let root = root();
        let dir = crate::target_dir(&root).join("tmp/cues-test");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("failing.replay");
        fs::write(
            &path,
            "level = testbed\nseed = 1\nsource = handwritten\nexpect = complete frames=1..1\n---\nR 5\n",
        )
        .unwrap();
        let err = write_golden(&root, "failing", &path).unwrap_err();
        assert!(err.contains("refusing"), "{err}");
        assert!(!cues_dir(&root).join("failing.cues").exists());
    }
}
