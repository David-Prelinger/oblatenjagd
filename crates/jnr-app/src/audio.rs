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
//! SFX at [`SFX`] and music at [`FULL`]; a duck is [`DUCK_GAIN`] (−6 dB, the
//! song and its twin only: a jingle is never ducked, D-123), a hold is 0 (the constants are `jnr_audio::cues`'s percents, shared with
//! the offline mix) (D-114: the loop keeps running and resumes in place).

use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll, Waker};

use jnr_audio::bank::{Bank, Variant};
use jnr_audio::cues::{Cue, Music, Play, Voice};
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
/// The song's (and twin's, never a jingle's) volume while an SFX of priority ≥ 7 plays (−6 dB, §3;
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
///   [`DUCK_GAIN`] for the song and its twin, never the jingle (D-123);
///   `Underwater`: swap the song's and its twin's volumes
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

/// A music cue's own gain ([`Music::gain_pct`]) as a factor.
fn cue_gain(m: Option<Music>) -> f32 {
    m.map_or(1.0, |m| m.gain_pct() as f32 / 100.0)
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

    /// The song's (`twin == false`) or its twin's current volume, with
    /// the cue's own gain ([`Music::gain_pct`]).
    fn song_volume(&self, twin: bool) -> f32 {
        let cue = if twin { self.twin } else { self.song };
        if self.hold_song || twin != self.underwater {
            0.0
        } else {
            self.duck_gain() * cue_gain(cue)
        }
    }

    fn jingle_volume(&self) -> f32 {
        if self.hold_jingle {
            0.0
        } else {
            // Never ducked (D-123).
            FULL * cue_gain(self.jingle)
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
                    if cut.hits(v) {
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

    /// Forgets the underwater twin (its render or load failed): the stage
    /// song plays alone and `Underwater` is ignored, as in `jnr_audio::mix`
    /// without the twin. Returns the song's volume op (it is audible again
    /// if the mixer was under water).
    pub fn drop_twin(&mut self) -> Vec<Op> {
        let mut out = Vec::new();
        if self.twin.take().is_some() {
            self.underwater = false;
            if let Some(m) = self.song {
                out.push(Op::Volume(SoundId::Music(m), self.song_volume(false)));
            }
        }
        out
    }

    /// The song and its twin start together (D-114: they must stay in
    /// step): `Some(ops)` once every song the mixer plays is loaded (song
    /// first, then twin, at their current volumes), `None` while one still
    /// loads. Empty when no song plays.
    pub fn start_ops(&self, is_loaded: impl Fn(SoundId) -> bool) -> Option<Vec<Op>> {
        self.songs()
            .into_iter()
            .flatten()
            .all(|m| is_loaded(SoundId::Music(m)))
            .then(|| self.resume_ops())
    }

    /// The song (and twin) from their start at their current volumes (a
    /// song start, and after unmuting: a muted session made no calls).
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

// ------------------------------------------------------------ bookkeeping
// The decisions `Audio` makes about loads and unloads, as pure functions
// (tested below without a sound device).

/// Whether a finished load is kept: SFX and jingles always, a song only
/// while it is the mixer's song or twin (else it was replaced while loading).
pub fn keep_loaded(id: SoundId, songs: [Option<Music>; 2]) -> bool {
    match id {
        SoundId::Music(m) if m.is_song() => songs.contains(&Some(m)),
        _ => true,
    }
}

/// What happens to a sound that is unloaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unload {
    /// Dropped now (a delete call to quad-snd).
    Drop,
    /// Kept until sound is on again (muted: no backend calls).
    Park,
    /// Leaked: the backend is dead or off, a delete would hit a dead thread.
    Forget,
}

/// [`Unload`] for a backend that is `gone` (dead or off) or `muted`.
pub fn unload_action(gone: bool, muted: bool) -> Unload {
    if gone {
        Unload::Forget
    } else if muted {
        Unload::Park
    } else {
        Unload::Drop
    }
}

/// What giving up on a sound (load error or timeout) changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GiveUp {
    /// It was the jingle waiting to start.
    pub clear_jingle: bool,
    /// It was the stage song: the song group can't start.
    pub cancel_songs: bool,
    /// It was the underwater twin: the song starts alone.
    pub drop_twin: bool,
}

/// [`GiveUp`] for `id`, given the waiting jingle and the mixer's songs.
pub fn give_up_effects(
    id: SoundId,
    jingle_to_start: Option<Music>,
    songs: [Option<Music>; 2],
) -> GiveUp {
    let SoundId::Music(m) = id else {
        return GiveUp::default();
    };
    GiveUp {
        clear_jingle: jingle_to_start == Some(m),
        cancel_songs: songs[0] == Some(m),
        drop_twin: songs[1] == Some(m),
    }
}

/// Whether a load timeout turns audio off: only while nothing has loaded
/// yet (the missing-`audio.js` case); later it gives up that one sound.
pub fn timeout_turns_audio_off(ever_loaded: bool) -> bool {
    !ever_loaded
}

/// Why audio is off for good.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Off {
    /// quad-snd's audio thread died (no device).
    NoDevice,
    /// The first loads did not finish within [`LOAD_BUDGET_FRAMES`] (wasm
    /// without `audio.js`).
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
    /// Some load finished: from then on a slow load is that load's problem,
    /// not the missing-`audio.js` case.
    ever_loaded: bool,
    /// The mixer's songs wait to start together once loaded.
    songs_to_start: bool,
    /// A jingle to start as soon as it is loaded.
    jingle_to_start: Option<Music>,
    /// Songs unloaded while muted: dropped at the next active frame (a drop
    /// is a backend call).
    to_unload: Vec<Sound>,
    /// Backend calls made: load starts, frames with load polls (on wasm a
    /// poll asks `audio.js` whether the buffer is decoded), plays, stops,
    /// volume changes and unloads (a dropped `Sound`). `--mute` must keep
    /// this 0.
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
            ever_loaded: false,
            songs_to_start: false,
            jingle_to_start: None,
            to_unload: Vec::new(),
            calls: 0,
        }
    }

    /// Whether a backend call may be made now (and counts it). Checked
    /// before every macroquad audio call.
    fn may_call(&mut self) -> bool {
        if !self.active() {
            return false;
        }
        self.calls += 1;
        true
    }

    /// Whether sound is on (not muted, not off): only then is anything
    /// rendered, loaded or played.
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

    /// Whether the mixer still wants `id` loaded ([`keep_loaded`]).
    fn wanted(&self, id: SoundId) -> bool {
        keep_loaded(id, self.mixer.songs())
    }

    /// Once per render frame, after the frame's simulation steps (so a cue's
    /// sound loads and starts on the frame of its cue natively): drops songs
    /// parked while muted, renders and starts queued loads, polls the
    /// pending ones and starts music that finished loading (none of it
    /// while muted or off). Never blocks.
    ///
    /// Loading right at start-up is safe even when the audio thread is about
    /// to die: a send before it dies succeeds silently, and the panic guard
    /// sets the dead flag before the thread drops its receiver, so every
    /// later call is refused before it can print `Audio thread died`.
    pub fn update(&mut self) {
        if !self.active() {
            return;
        }
        for sound in std::mem::take(&mut self.to_unload) {
            self.calls += 1;
            drop(sound);
        }
        self.queue_startup_sfx();
        // Songs replaced before their load started are not loaded at all.
        let queued = std::mem::take(&mut self.queued);
        self.queued = queued
            .into_iter()
            .filter(|(id, _)| self.wanted(*id))
            .collect();
        if !self.queued.is_empty() && self.may_call() {
            for (id, wav) in std::mem::take(&mut self.queued) {
                let load = Load::new(
                    async move { load_sound_from_bytes(&wav).await },
                    LOAD_BUDGET_FRAMES,
                );
                self.pending.push((id, load));
            }
        }
        if !self.pending.is_empty() && self.may_call() {
            self.poll_loads();
        }
        if self.off.is_none() {
            self.try_start();
        }
    }

    fn poll_loads(&mut self) {
        let mut still_pending = Vec::new();
        for (id, mut load) in std::mem::take(&mut self.pending) {
            if self.off.is_some() {
                break;
            }
            match load.poll_frame() {
                LoadPoll::Done(Ok(sound)) => {
                    self.ever_loaded = true;
                    if self.wanted(id) {
                        self.loaded.push((id, sound));
                    } else {
                        // A song replaced while it loaded.
                        self.calls += 1;
                        drop(sound);
                    }
                }
                LoadPoll::Done(Err(e)) => {
                    log_line(&format!("loading sound {id:?} failed: {e}"));
                    self.give_up(id);
                }
                LoadPoll::Waiting => still_pending.push((id, load)),
                // Nothing ever loaded: the missing-audio.js case.
                LoadPoll::TimedOut if timeout_turns_audio_off(self.ever_loaded) => {
                    log_line(&format!(
                        "audio load timed out after {} frames; audio off",
                        load.polls()
                    ));
                    self.off = Some(Off::LoadTimedOut);
                }
                LoadPoll::TimedOut => {
                    log_line(&format!(
                        "loading sound {id:?} timed out after {} frames",
                        load.polls()
                    ));
                    self.give_up(id);
                }
            }
        }
        if self.off.is_none() {
            self.pending = still_pending;
        }
    }

    /// A sound that will never load ([`give_up_effects`]): nothing waits for
    /// it any more; a failed twin leaves the stage song to start alone.
    fn give_up(&mut self, id: SoundId) {
        let g = give_up_effects(id, self.jingle_to_start, self.mixer.songs());
        if g.clear_jingle {
            self.jingle_to_start = None;
        }
        if g.cancel_songs {
            self.songs_to_start = false;
        }
        if g.drop_twin {
            self.drop_twin();
        }
    }

    /// The twin is gone: the mixer forgets it (the song plays alone).
    fn drop_twin(&mut self) {
        for op in self.mixer.drop_twin() {
            self.exec(op);
        }
    }

    /// Starts the waiting jingle and the waiting song group once loaded.
    fn try_start(&mut self) {
        if let Some(m) = self.jingle_to_start {
            match self.mixer.music_volume(m) {
                Some(volume) if self.is_loaded(SoundId::Music(m)) => {
                    self.jingle_to_start = None;
                    self.backend_play(SoundId::Music(m), false, volume);
                }
                Some(_) => {}
                None => self.jingle_to_start = None,
            }
        }
        if self.songs_to_start {
            let ops = self.mixer.start_ops(|id| self.is_loaded(id));
            if let Some(ops) = ops {
                self.songs_to_start = false;
                for op in ops {
                    if let Op::Play { id, looped, volume } = op {
                        self.backend_play(id, looped, volume);
                    }
                }
            }
        }
    }

    /// Executes the cues of one simulation step.
    pub fn cues(&mut self, cues: &[Cue]) {
        for &cue in cues {
            let bank = self.bank;
            let ops = self.mixer.apply(cue, |m| bank.has_music(m));
            if let Cue::Song(_) | Cue::StopSong = cue {
                self.unload_old_songs();
            }
            if !self.active() {
                continue;
            }
            for op in ops {
                self.exec(op);
            }
        }
        if self.active() {
            self.try_start();
        }
    }

    /// Drops loaded songs the mixer no longer plays (D-114: one stage's
    /// songs at a time; jingles stay loaded). Off or dead: leaked (a drop
    /// would call into a dead thread); muted: parked until sound is on.
    fn unload_old_songs(&mut self) {
        let keep = self.mixer.songs();
        let (old, kept): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.loaded)
                .into_iter()
                .partition(|(id, _)| match id {
                    SoundId::Music(m) => m.is_song() && !keep.contains(&Some(*m)),
                    SoundId::Sfx(_) => false,
                });
        self.loaded = kept;
        let gone = (self.dead)() || self.off.is_some();
        for (_, sound) in old {
            match unload_action(gone, self.muted) {
                Unload::Forget => std::mem::forget(sound),
                Unload::Park => self.to_unload.push(sound),
                Unload::Drop => {
                    self.calls += 1;
                    drop(sound);
                }
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

    /// Plays a loaded sound from its start (at most one live copy, D-114).
    fn backend_play(&mut self, id: SoundId, looped: bool, volume: f32) {
        if self.is_loaded(id) && self.may_call() {
            let sound = self.sound(id).expect("checked above");
            stop_sound(sound);
            play_sound(sound, PlaySoundParams { looped, volume });
        }
    }

    /// The song group (song and twin) to start together once loaded. A
    /// twin that can't be rendered is dropped (the song plays alone); a song
    /// that isn't in the bank (yet) starts nothing. With no song at all the
    /// group is empty: `start_ops` then gives no ops and the flag clears in
    /// `try_start`.
    fn start_songs(&mut self) {
        let [song, twin] = self.mixer.songs();
        if let Some(t) = twin
            && !self.ensure_music(t)
        {
            self.drop_twin();
        }
        if let Some(m) = song
            && !self.ensure_music(m)
        {
            return;
        }
        self.songs_to_start = true;
    }

    /// One backend operation (an SFX not loaded yet is dropped; music is
    /// rendered on first use and started once loaded, a song with its twin).
    fn exec(&mut self, op: Op) {
        match op {
            Op::Play {
                id: SoundId::Music(_),
                looped: true,
                ..
            } => self.start_songs(),
            Op::Play {
                id: SoundId::Music(m),
                volume,
                ..
            } => {
                if self.is_loaded(SoundId::Music(m)) {
                    self.backend_play(SoundId::Music(m), false, volume);
                } else if self.ensure_music(m) {
                    self.jingle_to_start = Some(m);
                }
            }
            Op::Play { id, looped, volume } => self.backend_play(id, looped, volume),
            Op::Stop(id) => {
                if let SoundId::Music(m) = id
                    && self.jingle_to_start == Some(m)
                {
                    self.jingle_to_start = None;
                }
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
        if self.muted {
            // A jingle cued before the mute is not started after it.
            self.jingle_to_start = None;
        }
        log_line(if self.muted { "sound off" } else { "sound on" });
        if self.active() {
            self.start_songs();
            self.try_start();
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
            for sound in self.to_unload.drain(..) {
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

    use jnr_audio::cues::{Cut, Director, PAUSE_OUT_SEMITONES, Sfx};

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

    /// The app's `Mixer` and the offline `jnr_audio::mix` agree on the
    /// music volumes after the same cue timeline, with and without the
    /// underwater twin in the bank (E59 review: one model).
    #[test]
    fn the_app_and_the_offline_mix_end_at_the_same_volumes() {
        use jnr_audio::mix::{Sound as MixSound, Sounds, mix};
        use jnr_audio::render::Pcm;
        const T: usize = jnr_audio::FRAMES_PER_TICK as usize;
        let timelines: [&[Cue]; 8] = [
            &[Cue::Song(Music::Wolfgangsee), Cue::Jingle(Music::BossIntro)],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Jingle(Music::BossIntro),
                Cue::Duck(true),
                Cue::Underwater(true),
            ],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Underwater(true),
                Cue::Duck(true),
            ],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Jingle(Music::Hurry),
                Cue::HoldSong(true),
                Cue::Duck(true),
            ],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Underwater(true),
                Cue::Jingle(Music::Hurry),
                Cue::HoldJingle(true),
                Cue::HoldJingle(false),
                Cue::Song(Music::Wolfgangsee),
            ],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Underwater(true),
                Cue::HoldSong(true),
                Cue::HoldSong(false),
                Cue::Duck(true),
                Cue::Duck(false),
                Cue::StopJingle,
            ],
            // Duck during a jingle: only the song drops (D-123).
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Jingle(Music::Hurry),
                Cue::Duck(true),
            ],
            &[
                Cue::Song(Music::Wolfgangsee),
                Cue::Duck(true),
                Cue::Jingle(Music::BossIntro),
                Cue::Underwater(true),
            ],
        ];
        for with_twin in [true, false] {
            for tl in timelines {
                // The app's volumes after the timeline.
                let mut m = Mixer::default();
                for &c in tl {
                    m.apply(c, |t| with_twin && t == Music::Unterwasser);
                }
                let vol = |x: Music| m.music_volume(x).unwrap_or(0.0);
                // Values that every gain (100, 50, 25, 12.5 %) divides exactly.
                let want = 8_000.0 * vol(Music::Wolfgangsee)
                    + 800.0 * vol(Music::Unterwasser)
                    + 80.0 * (vol(Music::Hurry) + vol(Music::BossIntro));
                // The offline mix, one cue per frame, read on the last frame.
                let pcm = |v: i16| Pcm {
                    rate: 44_100,
                    samples: vec![v; 2 * T * (tl.len() + 2)],
                };
                let mut sounds = Sounds::default();
                sounds
                    .sounds
                    .insert(MixSound::Music(Music::Wolfgangsee), pcm(8_000));
                sounds.sounds.insert(MixSound::Music(Music::Hurry), pcm(80));
                sounds
                    .sounds
                    .insert(MixSound::Music(Music::BossIntro), pcm(80));
                if with_twin {
                    sounds
                        .sounds
                        .insert(MixSound::Music(Music::Unterwasser), pcm(800));
                }
                let timeline: Vec<(u32, Cue)> =
                    tl.iter().enumerate().map(|(f, &c)| (f as u32, c)).collect();
                let last = tl.len() - 1;
                let got = mix(&sounds, &timeline, last as u32 + 1).render.pcm.samples[last * T];
                assert_eq!(f32::from(got), want, "twin {with_twin}: {tl:?}");
            }
        }
    }

    /// The bookkeeping decisions; each would fail without the E59 review
    /// fixes (all loads kept, muted unloads leaked, any timeout killing
    /// audio, a failed load leaving its waiters waiting).
    #[test]
    fn bookkeeping_keeps_parks_and_gives_up_as_decided() {
        let songs = [Some(Music::Wolfgangsee), Some(Music::Unterwasser)];
        // A finished load: a replaced song is not kept; SFX and jingles are.
        assert!(keep_loaded(music(Music::Wolfgangsee), songs));
        assert!(keep_loaded(music(Music::Unterwasser), songs));
        assert!(!keep_loaded(music(Music::Strobl), songs));
        assert!(keep_loaded(music(Music::Death), [None, None]));
        assert!(keep_loaded(sfx(Sfx::Jump.play()), [None, None]));
        // Unloading: muted parks, only a dead/off backend leaks.
        assert_eq!(unload_action(false, false), Unload::Drop);
        assert_eq!(unload_action(false, true), Unload::Park);
        assert_eq!(unload_action(true, true), Unload::Forget);
        assert_eq!(unload_action(true, false), Unload::Forget);
        // Timeouts: only before anything loaded.
        assert!(timeout_turns_audio_off(false));
        assert!(!timeout_turns_audio_off(true));
        // Giving up: the waiting jingle clears, the song cancels the group,
        // the twin only drops itself.
        let jingle = Some(Music::Hurry);
        assert_eq!(
            give_up_effects(music(Music::Hurry), jingle, songs),
            GiveUp {
                clear_jingle: true,
                ..GiveUp::default()
            }
        );
        assert_eq!(
            give_up_effects(music(Music::Wolfgangsee), jingle, songs),
            GiveUp {
                cancel_songs: true,
                ..GiveUp::default()
            }
        );
        assert_eq!(
            give_up_effects(music(Music::Unterwasser), None, songs),
            GiveUp {
                drop_twin: true,
                ..GiveUp::default()
            }
        );
        assert_eq!(
            give_up_effects(sfx(Sfx::Jump.play()), jingle, songs),
            GiveUp::default()
        );
    }

    /// A failed twin: the stage song plays alone, audible even if the
    /// mixer was under water, and the dive is ignored from then on.
    #[test]
    fn dropping_the_twin_leaves_the_song_alone_and_audible() {
        let mut m = Mixer::default();
        let twin = |t: Music| t == Music::Unterwasser;
        m.apply(Cue::Song(Music::Wolfgangsee), twin);
        m.apply(Cue::Underwater(true), twin);
        assert_eq!(m.drop_twin(), [Op::Volume(music(Music::Wolfgangsee), FULL)]);
        assert_eq!(m.songs(), [Some(Music::Wolfgangsee), None]);
        assert!(m.apply(Cue::Underwater(true), twin).is_empty());
        assert_eq!(m.music_volume(Music::Wolfgangsee), Some(FULL));
        assert!(m.drop_twin().is_empty(), "only once");
        // The group now starts with the song alone.
        assert_eq!(
            m.start_ops(|id| id == music(Music::Wolfgangsee))
                .map(|o| o.len()),
            Some(1)
        );
    }

    /// D-114: the song and its twin start in the same update, even when
    /// the twin's load finishes a frame later (wasm).
    #[test]
    fn the_song_and_its_twin_start_together() {
        let mut m = Mixer::default();
        m.apply(Cue::Song(Music::Wolfgangsee), |t| t == Music::Unterwasser);
        // Frame 1: the song is loaded, the twin isn't.
        let song_only = |id: SoundId| id == music(Music::Wolfgangsee);
        assert_eq!(m.start_ops(song_only), None);
        // Frame 2: both.
        assert_eq!(
            m.start_ops(|_| true),
            Some(vec![
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
            ])
        );
        // Without a song nothing waits.
        assert_eq!(Mixer::default().start_ops(|_| false), Some(vec![]));
    }

    #[test]
    fn music_volume_follows_hold_duck_and_underwater() {
        let mut m = Mixer::default();
        let twin = |t: Music| t == Music::Unterwasser;
        m.apply(Cue::Song(Music::Wolfgangsee), twin);
        m.apply(Cue::Jingle(Music::Hurry), twin);
        let vols = |m: &Mixer| {
            [
                Music::Wolfgangsee,
                Music::Unterwasser,
                Music::Hurry,
                Music::Death,
            ]
            .map(|x| m.music_volume(x))
        };
        assert_eq!(vols(&m), [Some(FULL), Some(0.0), Some(FULL), None]);
        m.apply(Cue::Duck(true), twin);
        // The duck lowers the song, never the jingle (D-123).
        assert_eq!(vols(&m), [Some(DUCK_GAIN), Some(0.0), Some(FULL), None]);
        m.apply(Cue::Underwater(true), twin);
        assert_eq!(vols(&m), [Some(0.0), Some(DUCK_GAIN), Some(FULL), None]);
        m.apply(Cue::HoldSong(true), twin);
        assert_eq!(vols(&m), [Some(0.0), Some(0.0), Some(FULL), None]);
        m.apply(Cue::HoldJingle(true), twin);
        m.apply(Cue::Duck(false), twin);
        assert_eq!(vols(&m), [Some(0.0), Some(0.0), Some(0.0), None]);
        m.apply(Cue::HoldSong(false), twin);
        m.apply(Cue::HoldJingle(false), twin);
        assert_eq!(vols(&m), [Some(0.0), Some(FULL), Some(FULL), None]);
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
        // A jingle plays at full volume during a duck (D-123); unducking
        // touches only the song and its twin; the pause holds both.
        assert_eq!(
            m.apply(Cue::Jingle(Music::Hurry), twin),
            [Op::Play {
                id: music(Music::Hurry),
                looped: false,
                volume: FULL
            }]
        );
        assert_eq!(
            m.apply(Cue::Duck(false), twin),
            [
                Op::Volume(music(Music::Wolfgangsee), 0.0),
                Op::Volume(music(Music::Unterwasser), FULL)
            ]
        );
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
