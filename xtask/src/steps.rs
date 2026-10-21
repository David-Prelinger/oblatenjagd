//! The individual validation steps. Each step is a function `fn(&Path) -> Step`
//! taking the workspace root. To implement a placeholder, replace its body;
//! to add a step, write a function and list it in `check_steps`/`verify_steps`.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::report::{EXCERPT_LINES, Status, Step, excerpt};
use crate::{archlint, audio, golden, levels, replay, solver, sprites, vnc, wasm};

pub type StepFn = fn(&Path) -> Step;

/// Fast checks, run after every change (and by the Stop hook).
pub fn check_steps() -> Vec<StepFn> {
    vec![
        fmt,
        clippy,
        test,
        wasm_build,
        arch_lint,
        sprites_lint,
        levels_lint,
        audio_lint,
    ]
}

/// Full validation: `check` plus the slower layers.
pub fn verify_steps() -> Vec<StepFn> {
    let mut steps = check_steps();
    steps.extend([
        solver as StepFn,
        replays,
        goldens,
        audio_goldens,
        wasm_smoke,
        vnc_smoke,
    ]);
    steps
}

// ------------------------------------------------------------- check (L1-L3)

fn fmt(root: &Path) -> Step {
    run_cargo(root, "fmt", &["fmt", "--all", "--check"])
}

fn clippy(root: &Path) -> Step {
    run_cargo(
        root,
        "clippy",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn test(root: &Path) -> Step {
    run_cargo(root, "test", &["test", "--workspace"])
}

fn wasm_build(root: &Path) -> Step {
    run_cargo(
        root,
        "wasm-build",
        &[
            "build",
            "-p",
            "jnr-app",
            "-p",
            "jnr-audio",
            "--target",
            "wasm32-unknown-unknown",
        ],
    )
}

pub fn arch_lint(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = match archlint::run(root) {
        Ok(()) => (true, String::new()),
        Err(problems) => (false, problems.join("\n")),
    };
    finish("arch-lint", "cargo xtask arch-lint", started, ok, &output)
}

fn sprites_lint(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = sprites::lint_report(root);
    finish(
        "sprites-lint",
        "cargo xtask sprites lint",
        started,
        ok,
        &output,
    )
}

fn levels_lint(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = levels::lint_report(root, None);
    finish(
        "levels-lint",
        "cargo xtask levels lint",
        started,
        ok,
        &output,
    )
}

fn audio_lint(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = audio::lint_report(root, None);
    finish("audio-lint", "cargo xtask audio lint", started, ok, &output)
}

// ---------------------------------------------------------------- verify (L4-L8)

fn solver(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output, note) = solver::verify(root);
    let mut step = finish("solver", "cargo xtask solve --all", started, ok, &output);
    step.note = note;
    step
}

fn replays(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = match replay::all_replays(root) {
        Ok(paths) => replay::run_replays(root, &paths, false),
        Err(e) => (false, e),
    };
    finish("replays", "cargo xtask replay --all", started, ok, &output)
}

fn goldens(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = golden::check_report(root);
    finish("goldens", "cargo xtask golden", started, ok, &output)
}

fn audio_goldens(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = audio::golden_report(root);
    finish(
        "audio-goldens",
        "cargo xtask audio golden",
        started,
        ok,
        &output,
    )
}

/// L8 (native): the real app window on the X display, see `vnc.rs`.
fn vnc_smoke(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = match vnc::smoke_check(root) {
        Ok(summary) => (true, summary),
        Err(e) => (false, e),
    };
    finish("vnc-smoke", "cargo xtask vnc-smoke", started, ok, &output)
}

/// L8 (web): the wasm build in headless Chromium, see `wasm.rs`.
fn wasm_smoke(root: &Path) -> Step {
    let started = Instant::now();
    let (ok, output) = match wasm::smoke_check(root) {
        Ok(summary) => (true, summary),
        Err(e) => (false, e),
    };
    finish("wasm-smoke", "cargo xtask wasm-smoke", started, ok, &output)
}

// ---------------------------------------------------------------------- helpers

/// Runs `cargo <args>` in `root` with stdout and stderr merged into one stream.
fn run_cargo(root: &Path, name: &str, args: &[&str]) -> Step {
    let started = Instant::now();
    let shown = format!("cargo {}", args.join(" "));
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let mut cmd = Command::new(cargo);
    cmd.args(args).current_dir(root);
    let (ok, output) = match run_merged(cmd) {
        Ok(result) => result,
        Err(e) => (false, format!("failed to start `{shown}`: {e}")),
    };
    finish(name, &shown, started, ok, &output)
}

/// Spawns `cmd` with stdout+stderr sent to one pipe; returns (success, output).
fn run_merged(mut cmd: Command) -> std::io::Result<(bool, String)> {
    let (mut reader, writer) = std::io::pipe()?;
    cmd.stdout(writer.try_clone()?)
        .stderr(writer)
        .env("CARGO_TERM_COLOR", "never");
    let mut child = cmd.spawn()?;
    // The Command still holds the pipe's write ends; drop them so read_to_end sees EOF.
    drop(cmd);
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes)?;
    let status = child.wait()?;
    Ok((
        status.success(),
        String::from_utf8_lossy(&bytes).into_owned(),
    ))
}

fn finish(name: &str, command: &str, started: Instant, ok: bool, output: &str) -> Step {
    Step {
        name: name.to_owned(),
        status: if ok { Status::Pass } else { Status::Fail },
        duration_ms: started.elapsed().as_millis() as u64,
        command: command.to_owned(),
        note: String::new(),
        output_tail: if ok {
            String::new()
        } else {
            excerpt(output, EXCERPT_LINES)
        },
    }
}
