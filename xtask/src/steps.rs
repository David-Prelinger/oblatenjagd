//! The individual validation steps. Each step is a function `fn(&Path) -> Step`
//! taking the workspace root. To implement a placeholder, replace its body;
//! to add a step, write a function and list it in `check_steps`/`verify_steps`.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::archlint;
use crate::report::{EXCERPT_LINES, Status, Step, excerpt};

pub type StepFn = fn(&Path) -> Step;

/// Fast checks, run after every change (and by the Stop hook).
pub fn check_steps() -> Vec<StepFn> {
    vec![fmt, clippy, test, wasm_build, arch_lint]
}

/// Full validation: `check` plus the slower layers.
pub fn verify_steps() -> Vec<StepFn> {
    let mut steps = check_steps();
    steps.extend([
        sprites_lint as StepFn,
        levels_lint,
        solver,
        replays,
        goldens,
        vnc_smoke,
    ]);
    steps
}

// ---------------------------------------------------------------- check (L1/L2)

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

// ------------------------------------------------- verify (L3-L8), placeholders

fn sprites_lint(_root: &Path) -> Step {
    Step::skipped("sprites-lint", "cargo xtask sprites lint")
}

fn levels_lint(_root: &Path) -> Step {
    Step::skipped("levels-lint", "cargo xtask levels lint")
}

fn solver(_root: &Path) -> Step {
    Step::skipped("solver", "cargo xtask solve <level>")
}

fn replays(_root: &Path) -> Step {
    Step::skipped("replays", "cargo xtask replay tests/replays/*.replay")
}

fn goldens(_root: &Path) -> Step {
    Step::skipped("goldens", "cargo xtask golden")
}

fn vnc_smoke(_root: &Path) -> Step {
    Step::skipped("vnc-smoke", "cargo xtask vnc-smoke")
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
