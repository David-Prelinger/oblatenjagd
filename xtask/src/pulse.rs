//! PulseAudio null sinks for audio smoke tests (D-118).
//!
//! The dev container has no sound device. With `pulseaudio`,
//! `pulseaudio-utils` and `libasound2-plugins` installed, ALSA's `default`
//! device goes to Pulse. [`null_sink_for_run`] makes sure a daemon runs
//! (it starts one on demand, which stays running for later runs) and loads
//! a null sink of its own, `jnr_null_<pid>`; the caller points the game at
//! it with `PULSE_SINK=<name>` and records `<name>.monitor` with `parec`.
//! The sink is unloaded when the [`NullSink`] is dropped, so parallel runs
//! never share a sink and the daemon's default sink is left alone. Without
//! Pulse, the caller notes "audio not exercised" and goes on.
//!
//! `JNR_PULSE=off` makes [`null_sink_for_run`] report Pulse as absent
//! without touching a daemon.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// How long to wait for a freshly started daemon to answer.
const START_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether audio can be exercised.
#[derive(Debug)]
pub enum Pulse {
    /// A null sink for this run; `started` = this call started the daemon.
    Ready { started: bool, sink: NullSink },
    /// No Pulse (not installed, disabled, or it would not start): why.
    Absent(String),
}

/// A null sink loaded for one run; unloaded on drop.
#[derive(Debug)]
pub struct NullSink {
    pub name: String,
    module: String,
}

impl NullSink {
    /// The source that records what the sink plays.
    pub fn monitor(&self) -> String {
        format!("{}.monitor", self.name)
    }
}

impl Drop for NullSink {
    fn drop(&mut self) {
        let _ = run("pactl", &["unload-module", &self.module]);
    }
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

/// The `pulseaudio` arguments that start a daemon with only the native
/// protocol (no hardware modules, no default script; sinks come per run).
pub fn daemon_args() -> Vec<String> {
    vec![
        "--daemonize=yes".to_owned(),
        "--exit-idle-time=-1".to_owned(),
        "-n".to_owned(),
        "--load=module-native-protocol-unix".to_owned(),
    ]
}

/// The null sink name for the process `pid`.
pub fn sink_name(pid: u32) -> String {
    format!("jnr_null_{pid}")
}

/// Makes sure a Pulse daemon answers (starting one if needed); returns
/// whether this call started it. A failed start may be a race with another
/// run starting the daemon at the same time, so it only counts once the
/// daemon still does not answer after [`START_TIMEOUT`].
fn ensure_daemon() -> Result<bool, String> {
    if run("pactl", &["info"]).is_ok() {
        return Ok(false);
    }
    let args = daemon_args();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let start = run("pulseaudio", &args);
    let begin = Instant::now();
    while run("pactl", &["info"]).is_err() {
        if begin.elapsed() > START_TIMEOUT {
            return Err(match start {
                Err(e) => format!("could not start pulseaudio: {e}"),
                Ok(_) => "pulseaudio started but does not answer".to_owned(),
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(start.is_ok())
}

/// A daemon plus a fresh null sink for this run, or why there is none.
pub fn null_sink_for_run() -> Pulse {
    if std::env::var("JNR_PULSE").as_deref() == Ok("off") {
        return Pulse::Absent("JNR_PULSE=off".to_owned());
    }
    if run("pulseaudio", &["--version"]).is_err() {
        return Pulse::Absent(
            "pulseaudio is not installed (see .devcontainer/Dockerfile)".to_owned(),
        );
    }
    let started = match ensure_daemon() {
        Ok(started) => started,
        Err(e) => return Pulse::Absent(e),
    };
    let name = sink_name(std::process::id());
    match run(
        "pactl",
        &[
            "load-module",
            "module-null-sink",
            &format!("sink_name={name}"),
        ],
    ) {
        Ok(module) => Pulse::Ready {
            started,
            sink: NullSink {
                name,
                module: module.trim().to_owned(),
            },
        },
        Err(e) => Pulse::Absent(format!("could not add a null sink: {e}")),
    }
}

/// Level statistics of a recorded capture (s16 samples).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureStats {
    pub samples: usize,
    /// Largest absolute sample.
    pub peak: u16,
    /// Samples at full scale (±32767/-32768).
    pub clipped: usize,
    /// Sound onsets ([`count_onsets`]).
    pub onsets: usize,
}

/// Window of the onset detector (10 ms at 44.1 kHz).
pub const ONSET_WINDOW: usize = 441;
/// A window this loud after a quiet stretch is an onset.
pub const ONSET_LEVEL: u16 = 1500;
/// Below this a window is quiet ...
pub const QUIET_LEVEL: u16 = 300;
/// ... and this many quiet windows in a row end a sound (30 ms).
pub const QUIET_WINDOWS: usize = 3;

/// Counts the sounds that start after silence: a 10 ms window reaching
/// [`ONSET_LEVEL`] after at least [`QUIET_WINDOWS`] windows below
/// [`QUIET_LEVEL`] (or at the start). Overlapping sounds count once.
pub fn count_onsets(samples: &[i16]) -> usize {
    let mut onsets = 0;
    let mut quiet = QUIET_WINDOWS;
    for w in samples.chunks(ONSET_WINDOW) {
        let level = w.iter().map(|s| s.unsigned_abs()).max().unwrap_or(0);
        if level >= ONSET_LEVEL && quiet >= QUIET_WINDOWS {
            onsets += 1;
        }
        if level < QUIET_LEVEL {
            quiet += 1;
        } else {
            quiet = 0;
        }
    }
    onsets
}

/// Statistics of raw little-endian s16 bytes (a trailing odd byte is ignored).
pub fn capture_stats(raw: &[u8]) -> CaptureStats {
    let mut stats = CaptureStats {
        samples: 0,
        peak: 0,
        clipped: 0,
        onsets: 0,
    };
    let samples: Vec<i16> = raw
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    for &s in &samples {
        stats.samples += 1;
        stats.peak = stats.peak.max(s.unsigned_abs());
        if s == i16::MAX || s == i16::MIN {
            stats.clipped += 1;
        }
    }
    stats.onsets = count_onsets(&samples);
    stats
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_never_exits_idle_and_sinks_are_per_run() {
        let args = daemon_args();
        assert!(args.contains(&"--exit-idle-time=-1".to_owned()));
        assert!(
            args.contains(&"-n".to_owned()),
            "no default.pa (no hardware modules)"
        );
        assert!(!args.iter().any(|a| a.contains("null-sink")));
        assert_eq!(sink_name(42), "jnr_null_42");
        let sink = std::mem::ManuallyDrop::new(NullSink {
            name: sink_name(7),
            module: "3".to_owned(),
        });
        assert_eq!(sink.monitor(), "jnr_null_7.monitor");
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
                clipped: 2,
                onsets: 1
            }
        );
        assert_eq!(capture_stats(&[]).peak, 0);
    }

    #[test]
    fn onsets_are_sounds_after_silence() {
        let beep = |n: usize| vec![4000i16; n];
        let quiet = |n: usize| vec![0i16; n];
        let mut s = quiet(10_000);
        for _ in 0..7 {
            s.extend(beep(6615)); // 150 ms
            s.extend(quiet(26_000));
        }
        assert_eq!(count_onsets(&s), 7);
        // A gap shorter than 30 ms doesn't split a sound.
        let mut s = beep(4410);
        s.extend(quiet(441));
        s.extend(beep(4410));
        assert_eq!(count_onsets(&s), 1);
        // Too soft is no onset.
        assert_eq!(count_onsets(&vec![1000i16; 44_100]), 0);
        let raw: Vec<u8> = [0i16, 2000, -32768, 5]
            .iter()
            .flat_map(|s| s.to_le_bytes())
            .collect();
        let st = capture_stats(&raw);
        assert_eq!(
            (st.samples, st.peak, st.clipped, st.onsets),
            (4, 32768, 1, 1)
        );
    }
}
