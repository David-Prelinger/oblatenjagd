//! `cargo xtask`: the single entry point for building, linting and validating
//! Oblatenjagd. See `docs/validation.md`.
//!
//! Each validation step is a plain function returning a [`Step`]. `check` and
//! `verify` are just ordered lists of those functions; later tasks fill in the
//! placeholder steps in `steps.rs`.

mod archlint;
mod content;
mod golden;
mod levels;
mod replay;
mod report;
mod sprites;
mod steps;
mod vnc;
mod wasm;

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use report::{Report, Status, Step};

const HELP: &str = "\
cargo xtask <command>

Commands:
  check     fast validation: fmt, clippy, tests, wasm build, architecture lint,
            sprites lint, levels lint
  verify    full validation: check + solver, replays, goldens, wasm-smoke,
            vnc-smoke (not implemented yet steps are skipped)
  arch-lint only the architecture lint (part of check)
  sprites   lint | preview [filter] [--scale N] | sheet | anim <name>
            (see `cargo xtask sprites help`; images under $CARGO_TARGET_DIR/preview)
  levels    lint [id]
  replay    <file|name|--all> [--shots]   run replays (expectations + determinism)
  filmstrip <file|name> [--every N]       contact sheet of every Nth frame
  scene     <level-id> --x <px>           render the level at camera x
  golden    [--accept <name>|--all]       compare / accept golden screenshots
            (see `cargo xtask replay help`, `cargo xtask golden help`)
  wasm      [--debug] [--serve] [--port N]: build + assemble $CARGO_TARGET_DIR/web
  wasm-smoke  load the wasm build in headless Chromium, check the screenshot
  vnc-smoke   run the native app on $DISPLAY (default :1) with a replay; compare
              its screenshots with the goldens, check an X root screenshot
  help      show this help (also -h, --help)

Reports are written to $CARGO_TARGET_DIR/reports/{check,verify}.{json,md}
(or ./target/reports if CARGO_TARGET_DIR is unset).
Exit code is non-zero if any step failed.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("check") if args.len() == 1 => run_suite("check", steps::check_steps()),
        Some("verify") if args.len() == 1 => run_suite("verify", steps::verify_steps()),
        Some("arch-lint") if args.len() == 1 => run_suite("arch-lint", vec![steps::arch_lint]),
        Some("sprites") => sprites::run(&workspace_root(), &args[1..]),
        Some("levels") => levels::run(&workspace_root(), &args[1..]),
        Some("replay") => replay::run_replay(&workspace_root(), &args[1..]),
        Some("filmstrip") => replay::run_filmstrip(&workspace_root(), &args[1..]),
        Some("scene") => replay::run_scene(&workspace_root(), &args[1..]),
        Some("golden") => golden::run(&workspace_root(), &args[1..]),
        Some("wasm") => wasm::run(&workspace_root(), &args[1..]),
        Some("wasm-smoke") => wasm::smoke(&workspace_root(), &args[1..]),
        Some("vnc-smoke") => vnc::run(&workspace_root(), &args[1..]),
        Some("help" | "-h" | "--help") | None => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some(_) => {
            eprintln!("error: unknown arguments: {}\n", args.join(" "));
            eprint!("{HELP}");
            ExitCode::from(2)
        }
    }
}

/// Runs every step in order (a failure does not stop later steps),
/// writes the reports and prints a summary.
fn run_suite(name: &str, suite: Vec<steps::StepFn>) -> ExitCode {
    let root = workspace_root();
    let started = Instant::now();
    let mut results: Vec<Step> = Vec::new();

    for step_fn in suite {
        let step = step_fn(&root);
        let status = match step.status {
            Status::Skipped => format!("skipped ({})", step.note),
            _ => step.status.as_str().to_owned(),
        };
        println!(
            "xtask {name}: {:<12} {:<8} {:>7.1}s",
            step.name,
            status,
            step.duration_ms as f64 / 1000.0
        );
        results.push(step);
    }

    let report = Report {
        command: name.to_owned(),
        duration_ms: started.elapsed().as_millis() as u64,
        steps: results,
    };
    let dir = reports_dir(&root);
    match report.write(&dir) {
        Ok((json, md)) => {
            println!("report: {}", json.display());
            println!("report: {}", md.display());
        }
        Err(e) => eprintln!("xtask: could not write reports to {}: {e}", dir.display()),
    }

    let failed: Vec<&str> = report
        .steps
        .iter()
        .filter(|s| s.status == Status::Fail)
        .map(|s| s.name.as_str())
        .collect();
    if failed.is_empty() {
        println!(
            "xtask {name}: PASS ({:.1}s)",
            report.duration_ms as f64 / 1000.0
        );
        ExitCode::SUCCESS
    } else {
        println!("xtask {name}: FAIL ({})", failed.join(", "));
        for s in report.steps.iter().filter(|s| s.status == Status::Fail) {
            println!("  reproduce {}: {}", s.name, s.command);
        }
        ExitCode::FAILURE
    }
}

/// The workspace root (parent of the `xtask` crate directory).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask lives inside the workspace")
        .to_path_buf()
}

/// `$CARGO_TARGET_DIR`, or `<root>/target`.
pub fn target_dir(root: &Path) -> PathBuf {
    match std::env::var_os("CARGO_TARGET_DIR") {
        Some(dir) if !dir.is_empty() => {
            let dir = PathBuf::from(dir);
            if dir.is_absolute() {
                dir
            } else {
                root.join(dir)
            }
        }
        _ => root.join("target"),
    }
}

/// `$CARGO_TARGET_DIR/reports`, or `<root>/target/reports`.
fn reports_dir(root: &Path) -> PathBuf {
    target_dir(root).join("reports")
}
