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
//! What to play comes from `jnr_audio::cues::Director` (E58): the app feeds
//! it after every simulation step and executes its [`Cue`]s (E59). The pure
//! [`Mixer`] turns each cue into backend operations ([`Op`]); [`Audio`]
//! renders and loads the sounds (`jnr_assets::audio_bank()`: every SFX and
//! pitched variant at start-up, a song when its cue first comes up, the
//! previous song unloaded) and makes the macroquad calls. Volumes are `f32`
//! only here (allowed in the app, never in the simulation).
//!
//! Mix levels: every sound is rendered at its design level (D-113's level
//! table, checked offline by `cargo xtask audio lint`), so the app plays
//! SFX at [`SFX`] and music at [`FULL`]; a duck is [`DUCK_GAIN`] (−6 dB), a
//! hold is 0 (the constants are `jnr_audio::cues`'s percents, shared with
//! the offline mix) (D-114: the loop keeps running and resumes in place).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use jnr_audio::bank::{Bank, Variant};
use jnr_audio::cues::{Cue, Cut, Music, Play, Voice};
use macroquad::audio::{
    PlaySoundParams, Sound, load_sound_from_bytes, play_sound, set_sound_volume, stop_sound,
};

/// Frames a pending load may take before audio is switched off (the
/// missing-`audio.js` case on wasm; natively a load finishes on its first
/// poll, in the browser after 1–2 frames).
pub const LOAD_BUDGET_FRAMES: u32 = 120;
/// Music at full level (`jnr_audio::cues::MUSIC_VOLUME_PCT`, shared with
/// the offline mix).
pub const FULL: f32 = jnr_audio::cues::MUSIC_VOLUME_PCT as f32 / 100.0;
/// SFX volume (`jnr_audio::cues::SFX_VOLUME_PCT`).
pub const SFX: f32 = jnr_audio::cues::SFX_VOLUME_PCT as f32 / 100.0;
/// The music's volume while an SFX of priority ≥ 7 plays (−6 dB, §3;
/// `jnr_audio::cues::DUCKED_MUSIC_VOLUME_PCT`).
pub const DUCK_GAIN: f32 = jnr_audio::cues::DUCKED_MUSIC_VOLUME_PCT as f32 / 100.0;

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

/// A sound the app loads: an SFX at a pitch, or a music cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum SoundId {
    Sfx(Play),
    Music(Music),
}

/// One backend operation, the pure output of [`Mixer::apply`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Op {
    /// Play from the start (`looped`: a song).
    Play {
        id: SoundId,
        looped: bool,
        volume: f32,
    },
    Stop(SoundId),
    Volume(SoundId, f32),
}

/// What plays and at which volume: the cue semantics as state (pure; the
/// unit tests drive it without a sound device).
///
/// - `Sfx`: stop what this voice last started, then play (a voice has one
///   live sound, so an id never stacks; D-114).
/// - `Song`: stop the old song and its twin, start the new one looped (its
///   underwater twin muted on the same frame, if the bank has it).
/// - `Jingle`: stop the old jingle, play the new one once.
/// - `HoldSong`/`HoldJingle`: volume 0 and back; `Duck`: music at
///   [`DUCK_GAIN`]; `Underwater`: swap the song's and its twin's volumes
///   (ignored without a twin).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mixer {
    /// The last sound started on each voice (P, W, I, A).
    voices: [Option<Play>; 4],
    song: Option<Music>,
    twin: Option<Music>,
    jingle: Option<Music>,
    hold_song: bool,
    hold_jingle: bool,
    duck: bool,
    underwater: bool,
}

fn slot(v: Voice) -> usize {
    match v {
        Voice::P => 0,
        Voice::W => 1,
        Voice::I => 2,
        Voice::A => 3,
    }
}

impl Mixer {
    fn duck_gain(&self) -> f32 {
        if self.duck { DUCK_GAIN } else { FULL }
    }

    /// The song's (`twin == false`) or its twin's current volume.
    fn song_volume(&self, twin: bool) -> f32 {
        if self.hold_song || twin != self.underwater {
            0.0
        } else {
            self.duck_gain()
        }
    }

    fn jingle_volume(&self) -> f32 {
        if self.hold_jingle {
            0.0
        } else {
            self.duck_gain()
        }
    }

    /// The current volume of `id` if it is the song, its twin or the
    /// jingle (`None`: not playing as music).
    pub fn music_volume(&self, id: Music) -> Option<f32> {
        if Some(id) == self.song {
            Some(self.song_volume(false))
        } else if Some(id) == self.twin {
            Some(self.song_volume(true))
        } else if Some(id) == self.jingle {
            Some(self.jingle_volume())
        } else {
            None
        }
    }

    /// The song and its twin (what must stay loaded).
    pub fn songs(&self) -> [Option<Music>; 2] {
        [self.song, self.twin]
    }

    fn music_volumes(&self, out: &mut Vec<Op>) {
        if let Some(m) = self.song {
            out.push(Op::Volume(SoundId::Music(m), self.song_volume(false)));
        }
        if let Some(m) = self.twin {
            out.push(Op::Volume(SoundId::Music(m), self.song_volume(true)));
        }
    }

    fn stop_voice(&mut self, v: Voice, out: &mut Vec<Op>) {
        if let Some(p) = self.voices[slot(v)].take() {
            out.push(Op::Stop(SoundId::Sfx(p)));
        }
    }

    /// The operations for `cue`; `has_twin` says whether the bank has the
    /// song's underwater twin (D-114).
    pub fn apply(&mut self, cue: Cue, has_twin: impl Fn(Music) -> bool) -> Vec<Op> {
        let mut out = Vec::new();
        match cue {
            Cue::Sfx(p) => {
                self.stop_voice(p.sfx.voice(), &mut out);
                self.voices[slot(p.sfx.voice())] = Some(p);
                out.push(Op::Play {
                    id: SoundId::Sfx(p),
                    looped: false,
                    volume: SFX,
                });
            }
            Cue::StopSfx(s) => {
                if self.voices[slot(s.voice())].is_some_and(|p| p.sfx == s) {
                    self.stop_voice(s.voice(), &mut out);
                }
            }
            Cue::CutSfx(cut) => {
                for v in Voice::ALL {
                    if cut == Cut::All || v != Voice::I {
                        self.stop_voice(v, &mut out);
                    }
                }
            }
            Cue::Song(m) => {
                self.stop_song(&mut out);
                self.song = Some(m);
                self.twin = m.underwater_twin().filter(|t| has_twin(*t));
                self.underwater = false;
                out.push(Op::Play {
                    id: SoundId::Music(m),
                    looped: true,
                    volume: self.song_volume(false),
                });
                if let Some(t) = self.twin {
                    out.push(Op::Play {
                        id: SoundId::Music(t),
                        looped: true,
                        volume: self.song_volume(true),
                    });
                }
            }
            Cue::StopSong => self.stop_song(&mut out),
            Cue::Jingle(m) => {
                if let Some(old) = self.jingle.take() {
                    out.push(Op::Stop(SoundId::Music(old)));
                }
                self.jingle = Some(m);
                out.push(Op::Play {
                    id: SoundId::Music(m),
                    looped: false,
                    volume: self.jingle_volume(),
                });
            }
            Cue::StopJingle => {
                if let Some(old) = self.jingle.take() {
                    out.push(Op::Stop(SoundId::Music(old)));
                }
            }
            Cue::HoldSong(b) => {
                self.hold_song = b;
                self.music_volumes(&mut out);
            }
            Cue::HoldJingle(b) => {
                self.hold_jingle = b;
                if let Some(m) = self.jingle {
                    out.push(Op::Volume(SoundId::Music(m), self.jingle_volume()));
                }
            }
            Cue::Duck(b) => {
                self.duck = b;
                self.music_volumes(&mut out);
                if let Some(m) = self.jingle {
                    out.push(Op::Volume(SoundId::Music(m), self.jingle_volume()));
                }
            }
            Cue::Underwater(b) => {
                if self.twin.is_some() && self.underwater != b {
                    self.underwater = b;
                    self.music_volumes(&mut out);
                }
            }
        }
        out
    }

    fn stop_song(&mut self, out: &mut Vec<Op>) {
        for m in [self.song.take(), self.twin.take()].into_iter().flatten() {
            out.push(Op::Stop(SoundId::Music(m)));
        }
        self.underwater = false;
    }

    /// After unmuting: the song (and twin) again from their start at their
    /// current volumes (a muted session made no calls, so nothing plays).
    pub fn resume_ops(&self) -> Vec<Op> {
        let mut out = Vec::new();
        for (m, twin) in [(self.song, false), (self.twin, true)] {
            if let Some(m) = m {
                out.push(Op::Play {
                    id: SoundId::Music(m),
                    looped: true,
                    volume: self.song_volume(twin),
                });
            }
        }
        out
    }
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

/// The app's audio: the [`Mixer`], the bank, pending and loaded sounds,
/// mute, and whether audio is off for good.
pub struct Audio {
    muted: bool,
    off: Option<Off>,
    /// Whether quad-snd's audio thread has died ([`audio_dead`]; a stub in tests).
    dead: fn() -> bool,
    bank: &'static Bank,
    mixer: Mixer,
    /// The startup SFX were rendered and queued.
    sfx_queued: bool,
    /// Sounds not yet handed to quad-snd, as (id, WAV bytes).
    queued: Vec<(SoundId, Vec<u8>)>,
    /// Loads in flight; polled once per frame.
    pending: Vec<(SoundId, SoundLoad)>,
    loaded: Vec<(SoundId, Sound)>,
    /// Music to start as soon as it is loaded (if the mixer still wants it).
    to_start: Vec<SoundId>,
    /// Backend calls made: load starts, frames with load polls (on wasm a
    /// poll asks `audio.js` whether the buffer is decoded), plays, stops,
    /// volume changes and unloads. `--mute` must keep this 0.
    calls: u32,
}

impl Audio {
    /// `muted` (`--mute`): nothing is rendered, loaded or played until M
    /// unmutes.
    pub fn new(muted: bool) -> Audio {
        Audio::with_probe(muted, audio_dead, jnr_assets::audio_bank())
    }

    /// [`Audio::new`] with the dead-thread check and the bank passed in.
    fn with_probe(muted: bool, dead: fn() -> bool, bank: &'static Bank) -> Audio {
        Audio {
            muted,
            off: None,
            dead,
            bank,
            mixer: Mixer::default(),
            sfx_queued: false,
            queued: Vec::new(),
            pending: Vec::new(),
            loaded: Vec::new(),
            to_start: Vec::new(),
            calls: 0,
        }
    }

    /// Whether a backend call may be made now (and counts it). Checked
    /// before every macroquad audio call.
    fn may_call(&mut self) -> bool {
        if (self.dead)() && self.off.is_none() {
            self.off = Some(Off::NoDevice);
        }
        if self.off.is_some() || self.muted {
            return false;
        }
        self.calls += 1;
        true
    }

    /// Whether sound is on (not muted, not off): only then is anything
    /// rendered or loaded.
    fn active(&mut self) -> bool {
        if (self.dead)() && self.off.is_none() {
            self.off = Some(Off::NoDevice);
        }
        self.off.is_none() && !self.muted
    }

    /// Renders every SFX and pitched variant and queues their loads (once),
    /// logging the cost.
    fn queue_startup_sfx(&mut self) {
        if self.sfx_queued {
            return;
        }
        self.sfx_queued = true;
        let t0 = macroquad::miniquad::date::now();
        let sfx = self.bank.startup_sfx();
        let n = sfx.len();
        for (play, pcm) in sfx {
            self.queued
                .push((SoundId::Sfx(play), jnr_audio::wav::write(&pcm)));
        }
        log_line(&format!(
            "audio: {n} sfx rendered in {:.1} ms",
            (macroquad::miniquad::date::now() - t0) * 1000.0
        ));
    }

    /// Renders and queues a music cue's sound if it isn't loaded or loading;
    /// false if the bank doesn't have it (yet).
    fn ensure_music(&mut self, m: Music) -> bool {
        let id = SoundId::Music(m);
        if self.is_loaded(id)
            || self.pending.iter().any(|(p, _)| *p == id)
            || self.queued.iter().any(|(q, _)| *q == id)
        {
            return true;
        }
        let t0 = macroquad::miniquad::date::now();
        match self.bank.render_music(m, Variant::Normal) {
            Ok(Some(r)) => {
                if r.looped {
                    log_line(&format!(
                        "audio: song {} rendered in {:.1} ms ({:.1} s)",
                        m.id(),
                        (macroquad::miniquad::date::now() - t0) * 1000.0,
                        r.pcm.samples.len() as f64 / 44_100.0
                    ));
                }
                self.queued.push((id, jnr_audio::wav::write(&r.pcm)));
                true
            }
            Ok(None) => false,
            Err(e) => {
                log_line(&format!("rendering {} failed: {e}", m.id()));
                false
            }
        }
    }

    /// Once per render frame: renders and starts queued loads, polls the
    /// pending ones, starts music that finished loading (none of it while
    /// muted or off). Never blocks.
    ///
    /// Loading right at start-up is safe even when the audio thread is about
    /// to die: a send before it dies succeeds silently, and the panic guard
    /// sets the dead flag before the thread drops its receiver, so every
    /// later call is refused before it can print `Audio thread died`.
    pub fn update(&mut self) {
        if !self.active() {
            return;
        }
        self.queue_startup_sfx();
        if !self.queued.is_empty() && self.may_call() {
            for (id, wav) in std::mem::take(&mut self.queued) {
                let load = Load::new(
                    async move { load_sound_from_bytes(&wav).await },
                    LOAD_BUDGET_FRAMES,
                );
                self.pending.push((id, load));
            }
        }
        if self.pending.is_empty() || !self.may_call() {
            return;
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
        if self.off.is_some() {
            return;
        }
        self.pending = still_pending;
        // Music whose cue came before its load finished starts now, at the
        // mixer's current volume (natively the same frame, in the browser
        // 1–2 frames late).
        for id in std::mem::take(&mut self.to_start) {
            let SoundId::Music(m) = id else { continue };
            match self.mixer.music_volume(m) {
                Some(volume) if self.is_loaded(id) => {
                    self.exec(Op::Play {
                        id,
                        looped: m.is_song(),
                        volume,
                    });
                }
                Some(_) => self.to_start.push(id),
                None => {}
            }
        }
    }

    /// Executes the cues of one simulation step.
    pub fn cues(&mut self, cues: &[Cue]) {
        for &cue in cues {
            let bank = self.bank;
            let ops = self.mixer.apply(cue, |m| bank.has_music(m));
            if let Cue::Song(_) = cue {
                self.unload_old_songs();
            }
            if !self.active() {
                continue;
            }
            for op in ops {
                self.exec(op);
            }
        }
    }

    /// Drops loaded songs the mixer no longer plays (D-114: one stage's
    /// songs at a time; jingles stay loaded).
    fn unload_old_songs(&mut self) {
        let keep = self.mixer.songs();
        let dead = (self.dead)();
        let (old, kept): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.loaded)
                .into_iter()
                .partition(|(id, _)| match id {
                    SoundId::Music(m) => m.is_song() && !keep.contains(&Some(*m)),
                    SoundId::Sfx(_) => false,
                });
        self.loaded = kept;
        for (_, sound) in old {
            if dead || self.muted || self.off.is_some() {
                // A drop sends a delete to quad-snd's thread.
                std::mem::forget(sound);
            } else {
                self.calls += 1;
                drop(sound);
            }
        }
    }

    fn sound(&self, id: SoundId) -> Option<&Sound> {
        self.loaded
            .iter()
            .find(|(loaded, _)| *loaded == id)
            .map(|(_, s)| s)
    }

    fn is_loaded(&self, id: SoundId) -> bool {
        self.sound(id).is_some()
    }

    /// One backend operation (SFX not loaded yet are dropped; music is
    /// rendered and started once loaded).
    fn exec(&mut self, op: Op) {
        match op {
            Op::Play { id, looped, volume } => {
                if self.sound(id).is_none() {
                    if let SoundId::Music(m) = id
                        && self.ensure_music(m)
                        && !self.to_start.contains(&id)
                    {
                        self.to_start.push(id);
                    }
                    return;
                }
                if !self.may_call() {
                    return;
                }
                let sound = self.sound(id).expect("checked above");
                // At most one live copy per id (D-114).
                stop_sound(sound);
                play_sound(sound, PlaySoundParams { looped, volume });
            }
            Op::Stop(id) => {
                self.to_start.retain(|t| *t != id);
                if self.is_loaded(id) && self.may_call() {
                    stop_sound(self.sound(id).expect("checked above"));
                }
            }
            Op::Volume(id, v) => {
                if self.is_loaded(id) && self.may_call() {
                    set_sound_volume(self.sound(id).expect("checked above"), v);
                }
            }
        }
    }

    /// M key: toggles mute; muting stops everything that is playing,
    /// unmuting starts the current song again from its start.
    pub fn toggle_mute(&mut self) {
        if !self.muted && !self.loaded.is_empty() && self.may_call() {
            for (_, sound) in &self.loaded {
                stop_sound(sound);
            }
        }
        self.muted = !self.muted;
        log_line(if self.muted { "sound off" } else { "sound on" });
        if !self.muted {
            for op in self.mixer.resume_ops() {
                self.exec(op);
            }
        }
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
        if (self.dead)() {
            for (_, sound) in self.loaded.drain(..) {
                std::mem::forget(sound);
            }
        }
    }
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

    use jnr_audio::cues::{Director, PAUSE_OUT_SEMITONES, Sfx};

    /// The cues of `tests/replays/testbed_route.replay` (7 jumps, the
    /// fanfare), step by step.
    fn testbed_cues() -> Vec<Vec<Cue>> {
        let replay =
            jnr_core::Replay::parse(include_str!("../../../tests/replays/testbed_route.replay"))
                .unwrap();
        let tuning = jnr_core::Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap();
        let mut game = replay
            .new_game(&crate::options::embedded_levels(), &tuning)
            .unwrap();
        let mut d = Director::new();
        let mut out = vec![d.begin(&game)];
        for input in replay.frames() {
            game.step(input);
            out.push(d.step(&game, input));
        }
        out
    }

    #[test]
    fn muted_audio_makes_no_backend_calls_with_real_cues() {
        // `--mute` through a whole replay's cues: nothing is rendered,
        // loaded or played (no macroquad context exists here, so any call
        // would panic).
        let steps = testbed_cues();
        let sfx: usize = steps
            .iter()
            .flatten()
            .filter(|c| matches!(c, Cue::Sfx(_)))
            .count();
        assert!(sfx >= 7, "the replay has its jump cues");
        let mut audio = Audio::new(true);
        for cues in &steps {
            audio.update();
            audio.cues(cues);
        }
        assert_eq!(audio.calls(), 0);
        assert!(audio.queued.is_empty() && audio.pending.is_empty() && audio.loaded.is_empty());
        assert_eq!(audio.off(), None);
        assert!(audio.is_muted());
    }

    #[test]
    fn a_dead_audio_thread_gets_no_calls_and_turns_audio_off() {
        // No macroquad context exists here, so any backend call would panic.
        let mut audio = Audio::with_probe(false, || true, jnr_assets::audio_bank());
        for cues in testbed_cues().iter().take(400) {
            audio.update();
            audio.cues(cues);
        }
        audio.toggle_mute();
        audio.toggle_mute();
        assert_eq!(audio.calls(), 0);
        assert_eq!(audio.off(), Some(Off::NoDevice));
        assert!(audio.pending.is_empty() && audio.loaded.is_empty());
    }

    fn sfx(id: Play) -> SoundId {
        SoundId::Sfx(id)
    }

    fn music(m: Music) -> SoundId {
        SoundId::Music(m)
    }

    #[test]
    fn an_sfx_stops_what_its_voice_played_last() {
        let mut m = Mixer::default();
        let jump = Sfx::Jump.play();
        let none = |_| false;
        assert_eq!(
            m.apply(Cue::Sfx(jump), none),
            [Op::Play {
                id: sfx(jump),
                looped: false,
                volume: SFX
            }]
        );
        // A retrigger: stop, then play (one live copy).
        let ops = m.apply(Cue::Sfx(jump), none);
        assert_eq!(ops[0], Op::Stop(sfx(jump)));
        // Another sound on the same voice replaces it; other voices don't.
        let hurt = Sfx::Hurt.play();
        assert_eq!(m.apply(Cue::Sfx(hurt), none)[0], Op::Stop(sfx(jump)));
        let oblate = Sfx::Oblate.play();
        assert_eq!(m.apply(Cue::Sfx(oblate), none).len(), 1);
        // StopSfx only stops the id its voice plays.
        assert!(m.apply(Cue::StopSfx(Sfx::Jump), none).is_empty());
        assert_eq!(
            m.apply(Cue::StopSfx(Sfx::Hurt), none),
            [Op::Stop(sfx(hurt))]
        );
        // A stage clear keeps the items voice; a cut of all doesn't.
        m.apply(Cue::Sfx(Sfx::Bump.play()), none);
        assert_eq!(
            m.apply(Cue::CutSfx(Cut::KeepItems), none),
            [Op::Stop(sfx(Sfx::Bump.play()))]
        );
        assert_eq!(
            m.apply(Cue::CutSfx(Cut::All), none),
            [Op::Stop(sfx(oblate))]
        );
        // A pitched variant is its own sound on the same voice.
        let out = Sfx::Pause.pitched(PAUSE_OUT_SEMITONES);
        m.apply(Cue::Sfx(Sfx::Pause.play()), none);
        assert_eq!(
            m.apply(Cue::Sfx(out), none)[0],
            Op::Stop(sfx(Sfx::Pause.play()))
        );
    }

    #[test]
    fn songs_holds_ducks_and_the_underwater_twin() {
        let mut m = Mixer::default();
        let twin = |t: Music| t == Music::Unterwasser;
        // The stage piece and its twin start together, the twin muted.
        assert_eq!(
            m.apply(Cue::Song(Music::Wolfgangsee), twin),
            [
                Op::Play {
                    id: music(Music::Wolfgangsee),
                    looped: true,
                    volume: FULL
                },
                Op::Play {
                    id: music(Music::Unterwasser),
                    looped: true,
                    volume: 0.0
                },
            ]
        );
        // Diving swaps the volumes; a duck lowers the audible one.
        assert_eq!(
            m.apply(Cue::Underwater(true), twin),
            [
                Op::Volume(music(Music::Wolfgangsee), 0.0),
                Op::Volume(music(Music::Unterwasser), FULL)
            ]
        );
        assert_eq!(
            m.apply(Cue::Duck(true), twin),
            [
                Op::Volume(music(Music::Wolfgangsee), 0.0),
                Op::Volume(music(Music::Unterwasser), DUCK_GAIN)
            ]
        );
        // A jingle plays at the ducked volume; the pause holds both.
        assert_eq!(
            m.apply(Cue::Jingle(Music::Hurry), twin),
            [Op::Play {
                id: music(Music::Hurry),
                looped: false,
                volume: DUCK_GAIN
            }]
        );
        m.apply(Cue::Duck(false), twin);
        assert_eq!(
            m.apply(Cue::HoldSong(true), twin),
            [
                Op::Volume(music(Music::Wolfgangsee), 0.0),
                Op::Volume(music(Music::Unterwasser), 0.0)
            ]
        );
        assert_eq!(
            m.apply(Cue::HoldJingle(true), twin),
            [Op::Volume(music(Music::Hurry), 0.0)]
        );
        assert_eq!(
            m.apply(Cue::HoldJingle(false), twin),
            [Op::Volume(music(Music::Hurry), FULL)]
        );
        m.apply(Cue::HoldSong(false), twin);
        // Unmuting restarts the song and twin at their volumes.
        assert_eq!(
            m.resume_ops(),
            [
                Op::Play {
                    id: music(Music::Wolfgangsee),
                    looped: true,
                    volume: 0.0
                },
                Op::Play {
                    id: music(Music::Unterwasser),
                    looped: true,
                    volume: FULL
                },
            ]
        );
        // A new song stops both and starts above water.
        let ops = m.apply(Cue::Song(Music::Strobl), twin);
        assert_eq!(
            ops[..2],
            [
                Op::Stop(music(Music::Wolfgangsee)),
                Op::Stop(music(Music::Unterwasser))
            ]
        );
        assert_eq!(m.songs(), [Some(Music::Strobl), None]);
        // Without the twin in the bank, the dive changes nothing.
        let mut m = Mixer::default();
        assert_eq!(m.apply(Cue::Song(Music::Wolfgangsee), |_| false).len(), 1);
        assert!(m.apply(Cue::Underwater(true), |_| false).is_empty());
        assert_eq!(
            m.apply(Cue::StopSong, |_| false),
            [Op::Stop(music(Music::Wolfgangsee))]
        );
    }
}
