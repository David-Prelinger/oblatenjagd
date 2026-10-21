//! PulseAudio null sink for audio smoke tests (D-118).
//!
//! The dev container has no sound device. With `pulseaudio`,
//! `pulseaudio-utils` and `libasound2-plugins` installed, ALSA's `default`
//! device goes to Pulse; a null sink named [`SINK`] swallows what the game
//! plays, and its monitor ([`MONITOR`]) can be recorded with `parec`.
//! [`ensure_null_sink`] starts a daemon on demand (it stays running for
//! later runs) or reuses a running one. Without Pulse, the caller notes
//! "audio not exercised" and goes on.
//!
//! `JNR_PULSE=off` makes [`ensure_null_sink`] report Pulse as absent without
//! touching a daemon (to check the no-device path; stop a running daemon
//! first with `pulseaudio --kill`).

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Name of the null sink.
pub const SINK: &str = "jnr_null";
/// The sink's monitor source (what `parec -d` records).
pub const MONITOR: &str = "jnr_null.monitor";
/// How long to wait for a freshly started daemon to answer.
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether audio can be exercised.
#[derive(Debug, PartialEq, Eq)]
pub enum Pulse {
    /// The null sink is the default sink; `started` = this call started the daemon.
    Ready { started: bool },
    /// No Pulse (not installed, disabled, or it would not start): why.
    Absent(String),
}

/// Runs a command quietly; `Ok(stdout)` if it exited with 0.
fn run(cmd: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("{cmd}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "`{cmd} {}` failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The `pulseaudio` arguments that start a daemon with only the null sink
/// and the native protocol (no hardware modules, no autospawn config).
pub fn daemon_args() -> Vec<String> {
    vec![
        "--daemonize=yes".to_owned(),
        "--exit-idle-time=-1".to_owned(),
        "-n".to_owned(),
        format!("--load=module-null-sink sink_name={SINK}"),
        "--load=module-native-protocol-unix".to_owned(),
    ]
}

/// Whether `pactl list short sinks` output names our sink.
pub fn has_sink(list: &str) -> bool {
    list.lines()
        .any(|l| l.split_whitespace().nth(1) == Some(SINK))
}

/// Makes sure a Pulse daemon runs with [`SINK`] as its default sink.
pub fn ensure_null_sink() -> Pulse {
    if std::env::var("JNR_PULSE").as_deref() == Ok("off") {
        return Pulse::Absent("JNR_PULSE=off".to_owned());
    }
    if run("pulseaudio", &["--version"]).is_err() {
        return Pulse::Absent(
            "pulseaudio is not installed (see .devcontainer/Dockerfile)".to_owned(),
        );
    }
    let mut started = false;
    if run("pactl", &["info"]).is_err() {
        let args = daemon_args();
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        if let Err(e) = run("pulseaudio", &args) {
            return Pulse::Absent(format!("could not start pulseaudio: {e}"));
        }
        started = true;
        let begin = Instant::now();
        while run("pactl", &["info"]).is_err() {
            if begin.elapsed() > START_TIMEOUT {
                return Pulse::Absent("pulseaudio started but does not answer".to_owned());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let sinks = run("pactl", &["list", "short", "sinks"]).unwrap_or_default();
    if !has_sink(&sinks) {
        if let Err(e) = run(
            "pactl",
            &[
                "load-module",
                "module-null-sink",
                &format!("sink_name={SINK}"),
            ],
        ) {
            return Pulse::Absent(format!("could not add the null sink: {e}"));
        }
    }
    if let Err(e) = run("pactl", &["set-default-sink", SINK]) {
        return Pulse::Absent(e);
    }
    Pulse::Ready { started }
}

/// Level statistics of a recorded capture (s16 samples).
#[derive(Debug, PartialEq, Eq)]
pub struct CaptureStats {
    pub samples: usize,
    /// Largest absolute sample.
    pub peak: u16,
    /// Samples at full scale (±32767/-32768).
    pub clipped: usize,
}

/// Statistics of raw little-endian s16 bytes (a trailing odd byte is ignored).
pub fn capture_stats(raw: &[u8]) -> CaptureStats {
    let mut stats = CaptureStats {
        samples: 0,
        peak: 0,
        clipped: 0,
    };
    for b in raw.chunks_exact(2) {
        let s = i16::from_le_bytes([b[0], b[1]]);
        stats.samples += 1;
        stats.peak = stats.peak.max(s.unsigned_abs());
        if s == i16::MAX || s == i16::MIN {
            stats.clipped += 1;
        }
    }
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_loads_the_null_sink_and_never_exits_idle() {
        let args = daemon_args();
        assert!(args.contains(&"--load=module-null-sink sink_name=jnr_null".to_owned()));
        assert!(args.contains(&"--exit-idle-time=-1".to_owned()));
        assert!(
            args.contains(&"-n".to_owned()),
            "no default.pa (no hardware modules)"
        );
        assert_eq!(MONITOR, format!("{SINK}.monitor"));
    }

    #[test]
    fn sink_list_is_parsed() {
        let list = "1\tauto_null\tmodule-null-sink.c\ts16le 2ch 44100Hz\tIDLE\n\
                    2\tjnr_null\tmodule-null-sink.c\ts16le 2ch 44100Hz\tSUSPENDED\n";
        assert!(has_sink(list));
        assert!(!has_sink("1\tjnr_null_other\tx\n"));
        assert!(!has_sink(""));
    }

    #[test]
    fn capture_stats_find_peak_and_clipping() {
        let raw: Vec<u8> = [0i16, 1800, -2000, i16::MAX, i16::MIN, 5]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .chain([7u8])
            .collect();
        assert_eq!(
            capture_stats(&raw),
            CaptureStats {
                samples: 6,
                peak: 32768,
                clipped: 2
            }
        );
        assert_eq!(capture_stats(&[]).peak, 0);
    }
}
