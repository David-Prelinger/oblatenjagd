//! Sound output: the only code that touches `macroquad::audio` (D-114, D-115).
//!
//! Audio is optional. Nothing here may stop, hang or fail the game:
//!
//! - **Panic guard** (native): quad-snd's audio thread panics when there is no
//!   sound device (`Can't open PCM device.`). [`install_panic_guard`] (called
//!   in `main()` before the window opens) recognises that panic with the pure
//!   [`classify_panic`], prints one line and sets [`audio_dead`]. Every
//!   backend call checks the flag first, so quad-snd never prints its
//!   `Audio thread died` line (once per call otherwise).
//! - **Frame-budgeted loading**: `load_sound_from_bytes` never returns on wasm
//!   when `audio.js` is missing. Each load is a [`Load`] polled once per frame
//!   with a no-op waker; after [`LOAD_BUDGET_FRAMES`] audio is switched off
//!   with one log line and the game runs on.
//! - **Mute**: `--mute` (native) loads and plays nothing; the M key toggles
//!   mute at runtime on both platforms.
//!
//! E54 is the skeleton: the only sound is a debug test tone (`--test-tone`,
//! native). The synth (`jnr-audio`) and the cue wiring come with E55/E58/E59.
//! Volumes are `f32` only here (allowed in the app, never in the simulation).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use macroquad::audio::{PlaySoundParams, Sound, load_sound_from_bytes, play_sound, stop_sound};

/// Frames a pending load may take before audio is switched off (the
/// missing-`audio.js` case on wasm; natively a load finishes on its first
/// poll, in the browser after 1–2 frames).
pub const LOAD_BUDGET_FRAMES: u32 = 120;
/// Frames to wait after start-up before the first load. quad-snd's audio
/// thread dies within the first frames when there is no device (the spike
/// saw the flag set before frame 3); loading before that would send to a
/// dead thread and print `Audio thread died`.
pub const STARTUP_GRACE_FRAMES: u32 = 10;
/// Sample rate of every sound we hand to quad-snd (44.1 kHz mono i16, D-114).
pub const SAMPLE_RATE: u32 = 44_100;

/// Set by the panic guard when quad-snd's audio thread has died.
static AUDIO_DEAD: AtomicBool = AtomicBool::new(false);

/// Whether quad-snd's audio thread has died (no device): no backend call may
/// be made any more.
pub fn audio_dead() -> bool {
    AUDIO_DEAD.load(Ordering::SeqCst)
}

/// What the panic guard does with a panic.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanicKind {
    /// quad-snd's audio thread gave up (no device): audio off, one line.
    AudioThread,
    /// Anything else: the default hook (message, backtrace, exit code).
    Other,
}

/// Classifies a panic by the file of its location and the name of the
/// panicking thread. Only a panic inside quad-snd on a thread other than
/// `main` is the audio thread dying; a quad-snd panic on the main thread
/// (e.g. a WAV it can't decode) is a real bug and keeps the default hook.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub fn classify_panic(file: Option<&str>, thread: Option<&str>) -> PanicKind {
    let in_quad_snd = file.is_some_and(|f| f.contains("quad-snd"));
    if in_quad_snd && thread != Some("main") {
        PanicKind::AudioThread
    } else {
        PanicKind::Other
    }
}

/// The panic message as text (panics carry `&str` or `String`).
#[cfg(not(target_arch = "wasm32"))]
fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "unknown reason"
    }
}

/// Installs the quad-snd panic guard (native; call before the window opens,
/// because macroquad creates the audio context while it sets up the window).
/// Every other panic goes to the previous hook unchanged.
#[cfg(not(target_arch = "wasm32"))]
pub fn install_panic_guard() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let file = info.location().map(|l| l.file());
        let thread = std::thread::current();
        match classify_panic(file, thread.name()) {
            PanicKind::AudioThread => {
                if !AUDIO_DEAD.swap(true, Ordering::SeqCst) {
                    eprintln!(
                        "jnr-app: audio unavailable ({}), playing silently",
                        panic_message(info.payload())
                    );
                }
            }
            PanicKind::Other => previous(info),
        }
    }));
}

/// One log line (stderr natively, the browser console on wasm).
fn log_line(msg: &str) {
    #[cfg(not(target_arch = "wasm32"))]
    eprintln!("jnr-app: {msg}");
    #[cfg(target_arch = "wasm32")]
    macroquad::logging::warn!("jnr-app: {}", msg);
}

/// What one frame's poll of a [`Load`] gave.
#[derive(Debug, PartialEq, Eq)]
pub enum LoadPoll<T> {
    /// The sound is loaded.
    Done(T),
    /// Not yet; poll again next frame.
    Waiting,
    /// The budget is used up: give up on audio.
    TimedOut,
}

/// A pending load: a pinned future polled once per frame with a no-op
/// waker, given up after `budget` frames. Generic so the budget logic is
/// testable without a macroquad context.
pub struct Load<T> {
    future: Pin<Box<dyn Future<Output = T>>>,
    polls: u32,
    budget: u32,
}

impl<T> Load<T> {
    pub fn new(future: impl Future<Output = T> + 'static, budget: u32) -> Load<T> {
        Load {
            future: Box::pin(future),
            polls: 0,
            budget,
        }
    }

    /// Polls the future once (call once per frame).
    pub fn poll_frame(&mut self) -> LoadPoll<T> {
        if self.polls >= self.budget {
            return LoadPoll::TimedOut;
        }
        self.polls += 1;
        let mut cx = Context::from_waker(Waker::noop());
        match self.future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => LoadPoll::Done(value),
            Poll::Pending if self.polls >= self.budget => LoadPoll::TimedOut,
            Poll::Pending => LoadPoll::Waiting,
        }
    }

    /// How often it was polled.
    pub fn polls(&self) -> u32 {
        self.polls
    }
}

/// The sounds the app knows (E59 adds the SFX and songs from `jnr-audio`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundId {
    /// Debug test tone (`--test-tone`): 440 Hz square, 0.3 s.
    TestTone,
}

/// Why audio is off for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Off {
    /// quad-snd's audio thread died (no device).
    NoDevice,
    /// A load did not finish within [`LOAD_BUDGET_FRAMES`] (wasm without `audio.js`).
    LoadTimedOut,
}

type SoundLoad = Load<Result<Sound, macroquad::Error>>;

/// The app's audio state: pending and loaded sounds, mute, and whether
/// audio is off for good.
pub struct Audio {
    muted: bool,
    off: Option<Off>,
    /// Render frames seen by [`Audio::update`].
    frames: u32,
    /// Sounds waiting for the start-up grace to pass, as (id, WAV bytes).
    queued: Vec<(SoundId, Vec<u8>)>,
    /// Loads in flight; polled once per frame.
    pending: Vec<(SoundId, SoundLoad)>,
    loaded: Vec<(SoundId, Sound)>,
    /// Play the test tone once it is loaded.
    play_test_tone: bool,
    /// Backend calls made (loads, plays, stops): `--mute` must keep this 0.
    calls: u32,
}

impl Audio {
    /// `muted` (`--mute`): nothing is loaded or played until M unmutes.
    pub fn new(muted: bool) -> Audio {
        Audio {
            muted,
            off: None,
            frames: 0,
            queued: Vec::new(),
            pending: Vec::new(),
            loaded: Vec::new(),
            play_test_tone: false,
            calls: 0,
        }
    }

    /// Queues the debug test tone; it plays once as soon as it is loaded.
    pub fn queue_test_tone(&mut self) {
        self.queued.push((SoundId::TestTone, test_tone_wav()));
        self.play_test_tone = true;
    }

    /// Whether a backend call may be made now (and counts it). Checked
    /// before every macroquad audio call.
    fn may_call(&mut self) -> bool {
        if audio_dead() && self.off.is_none() {
            self.off = Some(Off::NoDevice);
        }
        if self.off.is_some() || self.muted {
            return false;
        }
        self.calls += 1;
        true
    }

    /// Once per render frame: starts queued loads after the start-up grace
    /// and polls the pending ones. Never blocks.
    pub fn update(&mut self) {
        self.frames = self.frames.saturating_add(1);
        if self.frames > STARTUP_GRACE_FRAMES && !self.queued.is_empty() && self.may_call() {
            for (id, wav) in std::mem::take(&mut self.queued) {
                let load = Load::new(
                    async move { load_sound_from_bytes(&wav).await },
                    LOAD_BUDGET_FRAMES,
                );
                self.pending.push((id, load));
            }
        }
        let mut still_pending = Vec::new();
        for (id, mut load) in std::mem::take(&mut self.pending) {
            if self.off.is_some() {
                break;
            }
            match load.poll_frame() {
                LoadPoll::Done(Ok(sound)) => self.loaded.push((id, sound)),
                LoadPoll::Done(Err(e)) => log_line(&format!("loading sound {id:?} failed: {e}")),
                LoadPoll::Waiting => still_pending.push((id, load)),
                LoadPoll::TimedOut => {
                    log_line(&format!(
                        "audio load timed out after {} frames; audio off",
                        load.polls()
                    ));
                    self.off = Some(Off::LoadTimedOut);
                }
            }
        }
        if self.off.is_none() {
            self.pending = still_pending;
        }
        if self.play_test_tone && self.is_loaded(SoundId::TestTone) {
            self.play_test_tone = false;
            self.play(SoundId::TestTone);
            log_line("test tone played");
        }
    }

    fn is_loaded(&self, id: SoundId) -> bool {
        self.loaded.iter().any(|(loaded, _)| *loaded == id)
    }

    /// Plays `id` once if it is loaded and audio is on; otherwise drops it.
    pub fn play(&mut self, id: SoundId) {
        if !self.is_loaded(id) || !self.may_call() {
            return;
        }
        if let Some((_, sound)) = self.loaded.iter().find(|(loaded, _)| *loaded == id) {
            play_sound(
                sound,
                PlaySoundParams {
                    looped: false,
                    volume: 0.5,
                },
            );
        }
    }

    /// M key: toggles mute; muting stops everything that is playing.
    pub fn toggle_mute(&mut self) {
        if !self.muted && !self.loaded.is_empty() && self.may_call() {
            for (_, sound) in &self.loaded {
                stop_sound(sound);
            }
        }
        self.muted = !self.muted;
        log_line(if self.muted { "sound off" } else { "sound on" });
    }

    #[cfg(test)]
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Why audio is off for good, if it is.
    #[cfg(test)]
    pub fn off(&self) -> Option<Off> {
        self.off
    }

    /// Backend calls made so far.
    #[cfg(test)]
    pub fn calls(&self) -> u32 {
        self.calls
    }
}

impl Drop for Audio {
    /// Dropping a `Sound` sends a delete to quad-snd's thread; when that
    /// thread is dead, leak them instead (no `Audio thread died` lines).
    fn drop(&mut self) {
        if audio_dead() {
            for (_, sound) in self.loaded.drain(..) {
                std::mem::forget(sound);
            }
        }
    }
}

/// The debug test tone as a WAV: 0.3 s of a 440 Hz square wave, 44.1 kHz
/// mono i16 at amplitude 6000 (about -15 dBFS), with 5 ms linear fades.
pub fn test_tone_wav() -> Vec<u8> {
    const AMPLITUDE: i32 = 6000;
    const FREQ_HZ: u32 = 440;
    let frames = SAMPLE_RATE * 3 / 10;
    let fade = SAMPLE_RATE / 200;
    let samples: Vec<i16> = (0..frames)
        .map(|i| {
            // Square: high for the first half of each period.
            let phase = (i * FREQ_HZ * 2 / SAMPLE_RATE) % 2;
            let level = if phase == 0 { AMPLITUDE } else { -AMPLITUDE };
            let gain = i.min(frames - 1 - i).min(fade) as i32;
            (level * gain / fade as i32) as i16
        })
        .collect();
    wav_bytes(&samples)
}

/// A RIFF/WAVE file (PCM, mono, 16 bit, [`SAMPLE_RATE`]) holding `samples`.
fn wav_bytes(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + samples.len() * 2);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUAD_SND_FILE: &str = "/usr/local/cargo/registry/src/index.crates.io-1949cf8c6b5b557f/quad-snd-0.2.8/src/alsa_snd.rs";

    #[test]
    fn only_quad_snd_panics_off_the_main_thread_turn_audio_off() {
        // The audio thread is spawned without a name.
        assert_eq!(
            classify_panic(Some(QUAD_SND_FILE), None),
            PanicKind::AudioThread
        );
        assert_eq!(
            classify_panic(Some(QUAD_SND_FILE), Some("audio")),
            PanicKind::AudioThread
        );
        // A quad-snd panic on the main thread (undecodable WAV) is a bug.
        assert_eq!(
            classify_panic(Some(QUAD_SND_FILE), Some("main")),
            PanicKind::Other
        );
        // Our own panics, anywhere, keep the default hook.
        assert_eq!(
            classify_panic(Some("crates/jnr-app/src/main.rs"), None),
            PanicKind::Other
        );
        assert_eq!(
            classify_panic(Some("crates/jnr-app/src/main.rs"), Some("main")),
            PanicKind::Other
        );
        assert_eq!(classify_panic(None, None), PanicKind::Other);
    }

    /// A future that is pending `n` times, then ready with `n`.
    struct ReadyAfter(u32);

    impl Future for ReadyAfter {
        type Output = u32;
        fn poll(mut self: Pin<&mut Self>, _cx: &mut Context) -> Poll<u32> {
            if self.0 == 0 {
                Poll::Ready(7)
            } else {
                self.0 -= 1;
                Poll::Pending
            }
        }
    }

    #[test]
    fn a_load_finishes_within_its_budget() {
        let mut load = Load::new(ReadyAfter(0), LOAD_BUDGET_FRAMES);
        assert_eq!(load.poll_frame(), LoadPoll::Done(7));
        // The browser case: ready after two frames.
        let mut load = Load::new(ReadyAfter(2), LOAD_BUDGET_FRAMES);
        assert_eq!(load.poll_frame(), LoadPoll::Waiting);
        assert_eq!(load.poll_frame(), LoadPoll::Waiting);
        assert_eq!(load.poll_frame(), LoadPoll::Done(7));
        assert_eq!(load.polls(), 3);
        // Ready on the very last allowed poll still counts.
        let mut load = Load::new(ReadyAfter(2), 3);
        assert_eq!(load.poll_frame(), LoadPoll::Waiting);
        assert_eq!(load.poll_frame(), LoadPoll::Waiting);
        assert_eq!(load.poll_frame(), LoadPoll::Done(7));
    }

    #[test]
    fn a_load_that_never_finishes_times_out_after_the_budget() {
        // The missing-audio.js case: pending for ever.
        let mut load = Load::new(std::future::pending::<u32>(), LOAD_BUDGET_FRAMES);
        for _ in 1..LOAD_BUDGET_FRAMES {
            assert_eq!(load.poll_frame(), LoadPoll::Waiting);
        }
        assert_eq!(load.poll_frame(), LoadPoll::TimedOut);
        assert_eq!(load.polls(), LOAD_BUDGET_FRAMES);
        // It stays timed out and is not polled again.
        assert_eq!(load.poll_frame(), LoadPoll::TimedOut);
        assert_eq!(load.polls(), LOAD_BUDGET_FRAMES);
    }

    #[test]
    fn muted_audio_makes_no_backend_calls() {
        // `--mute` with the test tone queued: nothing is loaded or played
        // (no macroquad context exists here, so any call would panic).
        let mut audio = Audio::new(true);
        audio.queue_test_tone();
        for _ in 0..(STARTUP_GRACE_FRAMES + LOAD_BUDGET_FRAMES + 5) {
            audio.update();
        }
        audio.play(SoundId::TestTone);
        audio.toggle_mute();
        assert!(!audio.is_muted());
        audio.toggle_mute();
        assert!(audio.is_muted());
        assert_eq!(audio.calls(), 0);
        assert_eq!(audio.off(), None);
    }

    #[test]
    fn nothing_loads_during_the_startup_grace() {
        let mut audio = Audio::new(false);
        audio.queue_test_tone();
        for _ in 0..STARTUP_GRACE_FRAMES {
            audio.update();
        }
        assert_eq!(audio.calls(), 0);
        assert_eq!(audio.queued.len(), 1);
    }

    #[test]
    fn test_tone_is_a_mono_44k_wav() {
        let wav = test_tone_wav();
        let u16_at = |i: usize| u16::from_le_bytes([wav[i], wav[i + 1]]);
        let u32_at = |i: usize| u32::from_le_bytes([wav[i], wav[i + 1], wav[i + 2], wav[i + 3]]);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(u32_at(4) as usize, wav.len() - 8);
        assert_eq!(&wav[8..16], b"WAVEfmt ");
        assert_eq!((u16_at(20), u16_at(22)), (1, 1), "PCM, mono");
        assert_eq!(u32_at(24), SAMPLE_RATE);
        assert_eq!(u16_at(34), 16);
        assert_eq!(&wav[36..40], b"data");
        let samples: Vec<i16> = wav[44..]
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();
        assert_eq!(u32_at(40) as usize, samples.len() * 2);
        assert_eq!(samples.len(), 13_230, "0.3 s");
        assert_eq!(samples.iter().map(|s| s.unsigned_abs()).max(), Some(6000));
        assert_eq!((samples[0], samples[samples.len() - 1]), (0, 0), "faded");
        // 440 Hz: 132 periods in 0.3 s = 264 sign changes (± the fades).
        let changes = samples
            .windows(2)
            .filter(|w| (w[0] >= 0) != (w[1] >= 0))
            .count();
        assert!((262..=265).contains(&changes), "{changes}");
    }
}
