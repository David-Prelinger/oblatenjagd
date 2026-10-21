//! Game events and state → sound cues (D-113, D-114; `docs/audio-design.md`
//! §4 and §6). Pure and deterministic: the same run gives the same cues,
//! so a replay's cues are a text log that is golden-tested
//! (`cargo xtask replay <file> --cue-log`, `cargo xtask cues`).
//!
//! The app (E59) feeds a [`Director`] once per simulation step, after
//! `Game::step`, and plays what it returns:
//!
//! - [`Cue::Sfx`]: stop whatever the app last started on the sound's
//!   [`Voice`], then play the sound from its start (a retrigger of the
//!   same id restarts it; D-114: an id has at most one live copy), at its
//!   [`Play::semitones`] offset (a variant the app pre-renders, [`PITCHED`]).
//! - [`Cue::StopSfx`] stops one sound, [`Cue::CutSfx`] whole voices.
//! - [`Cue::Song`] stops the current song and starts this loop from its
//!   first sample, at the current hold and duck volume (above water);
//!   [`Cue::StopSong`] stops it.
//! - [`Cue::Jingle`] stops the current jingle and plays this one once
//!   (songs and jingles are both "music": the music volume and the mute act
//!   on both, the duck only on the song, D-123); [`Cue::StopJingle`] stops
//!   it.
//! - [`Cue::HoldSong`]: the song's volume to 0 and back (pause, the
//!   ZEIT-60 bells); the loop keeps running, so it resumes in place
//!   (D-114). [`Cue::HoldJingle`] does the same to the playing jingle
//!   (pause only), so a jingle survives a pause (its rest is heard if it
//!   is still running on resume).
//! - [`Cue::StopJingle`] may come for a jingle that already ended
//!   (stopping an idle sound is harmless).
//! - [`Cue::Duck`]: the song (and its twin) −6 dB while an SFX of priority
//!   ≥ 7 plays (not while paused: everything is held then); a jingle is
//!   never ducked (D-123).
//! - [`Cue::Underwater`]: swap the song's volume with its underwater twin
//!   ([`Music::underwater_twin`]), which the app started muted on the same
//!   frame as the song (D-114).
//!
//! The voice rules (§6.1) are [`Voices`]: one trigger per id per step, one
//! sound per voice, a new sound replaces a playing one of lower or equal
//! priority and is dropped otherwise; Died, GameOver and pause cut every
//! voice, a stage clear every voice but the items voice. A voice is busy
//! for the design's length of its sound ([`Sfx::ms`]).

use std::collections::BTreeMap;
use std::fmt;

use jnr_core::enemies::state;
use jnr_core::fx::Fx;
use jnr_core::physics::Aabb;
use jnr_core::{
    ClearPhase, EndingPhase, EnemyKind, Entity, EntityKind, Event, Game, InputFrame, MapPhase,
    Mode, Power, SCREEN_H, SCREEN_W,
};

/// An SFX voice (`docs/audio-design.md` §2.2). The ambience voice `A` is
/// for the nice-to-have loops; no must cue uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Voice {
    /// Player: jump, stroke, throw, hurt, power-ups.
    P,
    /// World: bump, brick, stomp, bounce, axe hit, enemy and boss cues.
    W,
    /// Items and UI: Oblate, 1-up, checkpoint, pause, START, tallies.
    I,
    /// Ambience loops (nice; unused so far).
    A,
}

impl Voice {
    /// Every voice, in slot order.
    pub const ALL: [Voice; 4] = [Voice::P, Voice::W, Voice::I, Voice::A];

    fn slot(self) -> usize {
        match self {
            Voice::P => 0,
            Voice::W => 1,
            Voice::I => 2,
            Voice::A => 3,
        }
    }

    /// The letter used in cue logs.
    pub const fn name(self) -> &'static str {
        match self {
            Voice::P => "P",
            Voice::W => "W",
            Voice::I => "I",
            Voice::A => "A",
        }
    }
}

/// A sound effect the cues can play: the 25 "must" SFX of
/// `docs/audio-design.md` §6. A pitched variant (the big jump, leaving the
/// pause) is the same id with a [`Play::semitones`] offset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sfx {
    Jump,
    Oblate,
    Zucker,
    Hackl,
    Hurt,
    OneUp,
    Bump,
    Brick,
    Stomp,
    RoofBounce,
    Checkpoint,
    Throw,
    AxeHit,
    Stroke,
    UiStart,
    Pause,
    TallyTick,
    TallyEnd,
    BossTelegraph,
    BossBreathe,
    BossHit,
    BillsHang,
    HohesC,
    KassaZeile,
    Muenze,
}

impl Sfx {
    /// Every SFX.
    pub const ALL: [Sfx; 25] = [
        Sfx::Jump,
        Sfx::Oblate,
        Sfx::Zucker,
        Sfx::Hackl,
        Sfx::Hurt,
        Sfx::OneUp,
        Sfx::Bump,
        Sfx::Brick,
        Sfx::Stomp,
        Sfx::RoofBounce,
        Sfx::Checkpoint,
        Sfx::Throw,
        Sfx::AxeHit,
        Sfx::Stroke,
        Sfx::UiStart,
        Sfx::Pause,
        Sfx::TallyTick,
        Sfx::TallyEnd,
        Sfx::BossTelegraph,
        Sfx::BossBreathe,
        Sfx::BossHit,
        Sfx::BillsHang,
        Sfx::HohesC,
        Sfx::KassaZeile,
        Sfx::Muenze,
    ];

    /// The id (`audio/sfx/<id>.sfx`).
    pub const fn id(self) -> &'static str {
        self.spec().0
    }

    /// The voice it plays on.
    pub const fn voice(self) -> Voice {
        self.spec().1
    }

    /// Its priority, 1–9 (stealing; ≥ 7 ducks the music).
    pub const fn prio(self) -> u8 {
        self.spec().2
    }

    /// Its length in the design (ms): how long it keeps its voice busy.
    pub const fn ms(self) -> u32 {
        self.spec().3
    }

    /// The SFX's playback gain in percent, on top of [`SFX_VOLUME_PCT`]
    /// (D-124): the mix headroom table. Each sound is rendered at its design
    /// peak (audio-design §3), but the loud ones that can coincide (hurt
    /// over the 1-up, the high C under either) summed past the −1 dBFS
    /// ceiling over the music; they play a little lower (−1 dB, the high C
    /// −1.5 dB). Every replay's mix then peaks at −1.56 dBFS or less
    /// (`cargo xtask audio mix`). The app's `Mixer` and `mix` both apply it.
    pub const fn gain_pct(self) -> u32 {
        match self {
            Sfx::Hurt => 89,
            Sfx::HohesC => 84,
            Sfx::OneUp => 89,
            _ => 100,
        }
    }

    /// (id, voice, priority, ms) from `docs/audio-design.md` §6.2/§6.3.
    const fn spec(self) -> (&'static str, Voice, u8, u32) {
        match self {
            Sfx::Jump => ("jump", Voice::P, 4, 150),
            Sfx::Oblate => ("oblate", Voice::I, 3, 180),
            Sfx::Zucker => ("zucker", Voice::P, 8, 500),
            Sfx::Hackl => ("hackl", Voice::P, 8, 500),
            Sfx::Hurt => ("hurt", Voice::P, 8, 500),
            Sfx::OneUp => ("oneup", Voice::I, 9, 700),
            Sfx::Bump => ("bump", Voice::W, 4, 90),
            Sfx::Brick => ("brick", Voice::W, 6, 220),
            Sfx::Stomp => ("stomp", Voice::W, 5, 110),
            Sfx::RoofBounce => ("roof_bounce", Voice::W, 5, 160),
            Sfx::Checkpoint => ("checkpoint", Voice::I, 7, 450),
            Sfx::Throw => ("throw", Voice::P, 5, 140),
            Sfx::AxeHit => ("axe_hit", Voice::W, 6, 100),
            Sfx::Stroke => ("stroke", Voice::P, 2, 90),
            Sfx::UiStart => ("ui_start", Voice::I, 6, 200),
            Sfx::Pause => ("pause", Voice::I, 9, 260),
            Sfx::TallyTick => ("tally_tick", Voice::I, 2, 25),
            Sfx::TallyEnd => ("tally_end", Voice::I, 6, 500),
            // The longest telegraph (round 1); it is stopped when the dash
            // starts.
            Sfx::BossTelegraph => ("boss_telegraph", Voice::W, 7, 667),
            Sfx::BossBreathe => ("boss_breathe", Voice::W, 7, 600),
            Sfx::BossHit => ("boss_hit", Voice::W, 8, 500),
            Sfx::BillsHang => ("bills_hang", Voice::W, 6, 400),
            Sfx::HohesC => ("hohes_c", Voice::W, 6, 800),
            Sfx::KassaZeile => ("kassa_zeile", Voice::I, 4, 250),
            Sfx::Muenze => ("muenze", Voice::I, 4, 150),
        }
    }

    /// The SFX with this id.
    pub fn from_id(id: &str) -> Option<Sfx> {
        Sfx::ALL.into_iter().find(|s| s.id() == id)
    }

    /// Frames it keeps its voice busy (rounded up).
    pub fn frames(self) -> u32 {
        ms_to_frames(self.ms())
    }

    /// Played at its own pitch.
    pub const fn play(self) -> Play {
        Play {
            sfx: self,
            semitones: 0,
        }
    }

    /// Played `semitones` higher (negative: lower).
    pub const fn pitched(self, semitones: i8) -> Play {
        Play {
            sfx: self,
            semitones,
        }
    }
}

/// An SFX to play, with a pitch offset. quad-snd can't re-pitch a sound,
/// so the app renders each (id, offset) pair the cues use as its own WAV
/// at startup ([`PITCHED`] lists them); the voice rules treat it as its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Play {
    pub sfx: Sfx,
    /// Semitones up (negative: down); 0 = as authored.
    pub semitones: i8,
}

/// A jump as Big or Hackl: a fourth lower (§6.2 `Jumped`).
pub const JUMP_BIG_SEMITONES: i8 = -5;
/// Leaving the pause: two semitones lower (§6.3 `pause`).
pub const PAUSE_OUT_SEMITONES: i8 = -2;

/// Every pitched variant the cues can play (the app pre-renders them with
/// `bank::transpose`). A variant of a band-passed or noise-heavy patch
/// needs a second look: its filters don't move with the pitch.
pub const PITCHED: [Play; 2] = [
    Sfx::Jump.pitched(JUMP_BIG_SEMITONES),
    Sfx::Pause.pitched(PAUSE_OUT_SEMITONES),
];

/// A music cue: a looping piece ("song") or a one-shot jingle
/// (`docs/audio-design.md` §5, the 25 must music cues and the nice
/// `unterwasser` track).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Music {
    Title,
    RouteMap,
    Salzburg,
    StGilgen,
    Brunnwinkl,
    Falkenstein,
    StWolfgang,
    Wolfgangsee,
    /// Stage 6 under water (nice; the app may use the low-passed fallback
    /// render of `wolfgangsee` under this id, D-114).
    Unterwasser,
    Strobl,
    BadIschl,
    CafeZauner,
    BossIntro,
    BossSurprise,
    Boss,
    BossDefeat,
    FanfareClear,
    FanfareFinale,
    Death,
    GameOver,
    Hurry,
    EndingMap,
    Kaffeehaus,
    Credits,
    Ende,
}

impl Music {
    /// Every music cue.
    pub const ALL: [Music; 25] = [
        Music::Title,
        Music::RouteMap,
        Music::Salzburg,
        Music::StGilgen,
        Music::Brunnwinkl,
        Music::Falkenstein,
        Music::StWolfgang,
        Music::Wolfgangsee,
        Music::Unterwasser,
        Music::Strobl,
        Music::BadIschl,
        Music::CafeZauner,
        Music::BossIntro,
        Music::BossSurprise,
        Music::Boss,
        Music::BossDefeat,
        Music::FanfareClear,
        Music::FanfareFinale,
        Music::Death,
        Music::GameOver,
        Music::Hurry,
        Music::EndingMap,
        Music::Kaffeehaus,
        Music::Credits,
        Music::Ende,
    ];

    /// The id (a level's `[meta] music`, `audio/music/<id>.song`).
    pub const fn id(self) -> &'static str {
        self.spec().0
    }

    /// The design's length of a jingle (ms); 0 for a looping song.
    pub const fn jingle_ms(self) -> u32 {
        self.spec().1
    }

    /// A looping piece (as opposed to a one-shot jingle).
    pub const fn is_song(self) -> bool {
        self.jingle_ms() == 0
    }

    /// The cue's playback gain in percent, on top of the music volume
    /// (game-designer decision, audio-design §4): `boss_intro` is played
    /// pianissimo, −12 dB (25 %), before the Paukenschlag; every other cue
    /// at 100 %. The app's `Mixer` and `mix` both apply it.
    pub const fn gain_pct(self) -> u32 {
        match self {
            Music::BossIntro => 25,
            _ => 100,
        }
    }

    /// (id, jingle length in ms or 0 for a loop), `docs/audio-design.md` §5.
    const fn spec(self) -> (&'static str, u32) {
        match self {
            Music::Title => ("title", 0),
            Music::RouteMap => ("route_map", 0),
            Music::Salzburg => ("salzburg", 0),
            Music::StGilgen => ("st_gilgen", 0),
            Music::Brunnwinkl => ("brunnwinkl", 0),
            Music::Falkenstein => ("falkenstein", 0),
            Music::StWolfgang => ("st_wolfgang", 0),
            Music::Wolfgangsee => ("wolfgangsee", 0),
            Music::Unterwasser => ("unterwasser", 0),
            Music::Strobl => ("strobl", 0),
            Music::BadIschl => ("bad_ischl", 0),
            Music::CafeZauner => ("cafe_zauner", 0),
            Music::BossIntro => ("boss_intro", 1500),
            Music::BossSurprise => ("boss_surprise", 600),
            Music::Boss => ("boss", 0),
            Music::BossDefeat => ("boss_defeat", 1500),
            Music::FanfareClear => ("fanfare_clear", 6300),
            Music::FanfareFinale => ("fanfare_finale", 9000),
            Music::Death => ("death", 1300),
            Music::GameOver => ("game_over", 8000),
            Music::Hurry => ("hurry", 1600),
            Music::EndingMap => ("ending_map", 3600),
            Music::Kaffeehaus => ("kaffeehaus", 0),
            Music::Credits => ("credits", 0),
            Music::Ende => ("ende", 4000),
        }
    }

    /// The music cue with this id.
    pub fn from_id(id: &str) -> Option<Music> {
        Music::ALL.into_iter().find(|m| m.id() == id)
    }

    /// A song a level may name as its `[meta] music` (`levels lint`): a
    /// looping piece other than the underwater twin.
    pub fn level_song(id: &str) -> Option<Music> {
        Music::from_id(id).filter(|m| m.is_song() && *m != Music::Unterwasser)
    }

    /// The track that runs in step with this song and replaces it while
    /// Resi is under water (§5 `unterwasser`, D-114).
    pub const fn underwater_twin(self) -> Option<Music> {
        match self {
            Music::Wolfgangsee => Some(Music::Unterwasser),
            _ => None,
        }
    }
}

/// Music cues without a source yet: the one place where a cue id or a
/// level's `music` may lack its `audio/music/<id>.song`. Empty since all 25
/// songs landed (MA); a test keeps it empty, so a new `Music` cue must land
/// with its song.
pub const PENDING_MUSIC: &[Music] = &[];

/// Which voices a [`Cue::CutSfx`] stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cut {
    /// Every voice (Died, GameOver, pause).
    All,
    /// Every voice but [`Voice::I`] (a stage clear keeps a just-collected
    /// Oblate).
    KeepItems,
}

impl Cut {
    /// True if this cut stops voice `v`.
    pub fn hits(self, v: Voice) -> bool {
        match self {
            Cut::All => true,
            Cut::KeepItems => v != Voice::I,
        }
    }
}

/// One thing the app does with its sounds (module docs).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    Sfx(Play),
    StopSfx(Sfx),
    CutSfx(Cut),
    Song(Music),
    StopSong,
    Jingle(Music),
    StopJingle,
    HoldSong(bool),
    HoldJingle(bool),
    Duck(bool),
    Underwater(bool),
}

impl fmt::Display for Cue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let on = |b: bool| if b { "on" } else { "off" };
        match *self {
            Cue::Sfx(Play { sfx: s, semitones }) => {
                write!(f, "sfx {} {}{}", s.id(), s.voice().name(), s.prio())?;
                if semitones != 0 {
                    write!(f, " {semitones:+}")?;
                }
                Ok(())
            }
            Cue::StopSfx(s) => write!(f, "stop-sfx {}", s.id()),
            Cue::CutSfx(Cut::All) => f.write_str("cut-sfx all"),
            Cue::CutSfx(Cut::KeepItems) => f.write_str("cut-sfx P W A"),
            Cue::Song(m) => write!(f, "song {}", m.id()),
            Cue::StopSong => f.write_str("stop-song"),
            Cue::Jingle(m) => write!(f, "jingle {}", m.id()),
            Cue::StopJingle => f.write_str("stop-jingle"),
            Cue::HoldSong(b) => write!(f, "hold-song {}", on(b)),
            Cue::HoldJingle(b) => write!(f, "hold-jingle {}", on(b)),
            Cue::Duck(b) => write!(f, "duck {}", on(b)),
            Cue::Underwater(b) => write!(f, "underwater {}", on(b)),
        }
    }
}

/// What a core event sounds like (`docs/audio-design.md` §6.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventSound {
    /// This SFX (at this pitch).
    Sfx(Play),
    /// A music cue, decided by the [`Director`]'s music rules.
    Music,
    /// Nothing (a nice-to-have SFX not made yet, or silent by design).
    Silent,
}

/// The sound of `event`; `power` is the player's power after the step
/// (the jump's pitch, which power-up). An exhaustive `match`: a new
/// [`Event`] variant doesn't compile until its sound is decided.
pub fn sfx_for(event: Event, power: Power) -> EventSound {
    match event {
        Event::Jumped => EventSound::Sfx(match power {
            Power::Normal => Sfx::Jump.play(),
            Power::Big | Power::Hackl => Sfx::Jump.pitched(JUMP_BIG_SEMITONES),
        }),
        // Nice (`land`, after ≥ 16 airborne frames): not made yet.
        Event::Landed => EventSound::Silent,
        Event::Died => EventSound::Music,
        // The stage piece starts with play (§6.2).
        Event::Respawned => EventSound::Silent,
        Event::LevelComplete => EventSound::Music,
        Event::OblateCollected => EventSound::Sfx(Sfx::Oblate.play()),
        Event::PowerUp => match power {
            Power::Big => EventSound::Sfx(Sfx::Zucker.play()),
            Power::Hackl => EventSound::Sfx(Sfx::Hackl.play()),
            // A power-up always ends Big or Hackl.
            Power::Normal => EventSound::Silent,
        },
        Event::Hurt => EventSound::Sfx(Sfx::Hurt.play()),
        Event::ExtraLife => EventSound::Sfx(Sfx::OneUp.play()),
        Event::BlockBumped => EventSound::Sfx(Sfx::Bump.play()),
        Event::BrickBroken => EventSound::Sfx(Sfx::Brick.play()),
        // Replaced by `roof_bounce` on a step with `Bounced`, and by
        // `boss_hit` on a step that hits the Oberkellner.
        Event::Stomped => EventSound::Sfx(Sfx::Stomp.play()),
        Event::CheckpointReached => EventSound::Sfx(Sfx::Checkpoint.play()),
        Event::GameOver => EventSound::Music,
        Event::HacklThrown => EventSound::Sfx(Sfx::Throw.play()),
        Event::AxeHit => EventSound::Sfx(Sfx::AxeHit.play()),
        // Nice (`crawl`): not made yet.
        Event::CrawlStarted => EventSound::Silent,
        Event::Stroke => EventSound::Sfx(Sfx::Stroke.play()),
        Event::Bounced => EventSound::Sfx(Sfx::RoofBounce.play()),
    }
}

/// Milliseconds to whole 60 Hz frames, rounded up.
pub fn ms_to_frames(ms: u32) -> u32 {
    (ms * 60).div_ceil(1000)
}

/// Frames Resi must stay below (or above) the water line before the
/// underwater track swaps in (or out), §5 `unterwasser`.
pub const UNDERWATER_SWITCH_FRAMES: u16 = 20;
/// Tally ticks sound on every this many frames of the tally (§6.3).
pub const TALLY_TICK_EVERY: u16 = 3;

/// A sound occupying a voice until frame `until` (exclusive).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Playing {
    sfx: Sfx,
    until: u32,
}

/// The SFX voice rules (`docs/audio-design.md` §6.1): what plays on each
/// voice and until when.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Voices {
    slots: [Option<Playing>; 4],
}

impl Voices {
    /// The SFX playing on `voice` at frame `now`.
    pub fn playing(&self, voice: Voice, now: u32) -> Option<Sfx> {
        self.slots[voice.slot()]
            .filter(|p| now < p.until)
            .map(|p| p.sfx)
    }

    /// Takes one step's requests (in order) at frame `now` and returns the
    /// sounds to start, at most one per voice: duplicates of an id count
    /// once; per voice the request of the highest priority wins (the first
    /// one on a tie); it starts if its voice is free or plays a sound of
    /// lower or equal priority (a retrigger restarts), and is dropped
    /// otherwise. The pitch offset doesn't matter to the rules.
    pub fn resolve(&mut self, now: u32, requests: &[Play]) -> Vec<Play> {
        let mut out = Vec::new();
        for voice in Voice::ALL {
            let mut best: Option<Play> = None;
            for &r in requests.iter().filter(|r| r.sfx.voice() == voice) {
                if best.is_none_or(|b| r.sfx.prio() > b.sfx.prio()) {
                    best = Some(r);
                }
            }
            let Some(play) = best else { continue };
            let s = play.sfx;
            if self
                .playing(voice, now)
                .is_some_and(|p| p.prio() > s.prio())
            {
                continue;
            }
            self.slots[voice.slot()] = Some(Playing {
                sfx: s,
                until: now.saturating_add(s.frames()),
            });
            out.push(play);
        }
        out
    }

    /// Stops `sfx` if it is still playing; true if it was.
    pub fn stop(&mut self, sfx: Sfx, now: u32) -> bool {
        let slot = &mut self.slots[sfx.voice().slot()];
        if slot.is_some_and(|p| p.sfx == sfx && now < p.until) {
            *slot = None;
            true
        } else {
            false
        }
    }

    /// Empties the voices `cut` names.
    pub fn cut(&mut self, cut: Cut) {
        for v in Voice::ALL {
            if cut.hits(v) {
                self.slots[v.slot()] = None;
            }
        }
    }

    /// True while a sound of priority ≥ 7 plays (the music ducks, §3).
    pub fn ducking(&self, now: u32) -> bool {
        Voice::ALL
            .into_iter()
            .any(|v| self.playing(v, now).is_some_and(|s| s.prio() >= DUCK_PRIO))
    }
}

/// Playback volumes in percent, shared by the app (E59) and the offline
/// mix (E60), so both play the same thing: every sound is rendered at its
/// design level (D-113's level table); music plays at 100 %, SFX at 89 %
/// (−1 dB of mix headroom, D-124) times their own [`Sfx::gain_pct`]; a
/// duck is 50 % (−6 dB, §3; songs only, D-123); a hold is 0 (D-114).
pub const SFX_VOLUME_PCT: u32 = 89;
/// Music (songs and jingles) at full level.
pub const MUSIC_VOLUME_PCT: u32 = 100;
/// The song (and its twin) while ducked ([`Cue::Duck`]; never a jingle,
/// D-123).
pub const DUCKED_MUSIC_VOLUME_PCT: u32 = 50;

/// SFX of this priority or more duck the music (§3, §6.1).
pub const DUCK_PRIO: u8 = 7;

/// The game's flow position: a mode and, where it has one, its phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Title,
    StageIntro,
    Playing,
    Paused,
    Dying,
    Clear(ClearPhase),
    Map(MapPhase),
    Ending(EndingPhase),
    GameOver,
}

impl Phase {
    fn of(g: &Game) -> Phase {
        match g.mode() {
            Mode::Title { .. } => Phase::Title,
            Mode::StageIntro { .. } => Phase::StageIntro,
            Mode::Playing => Phase::Playing,
            Mode::Paused => Phase::Paused,
            Mode::Dying { .. } => Phase::Dying,
            Mode::StageClear => {
                Phase::Clear(g.stage_clear().map_or(ClearPhase::Slide, |c| c.phase))
            }
            Mode::RouteMap => Phase::Map(g.route_map().map_or(MapPhase::Arrive, |m| m.phase)),
            Mode::Ending => Phase::Ending(g.ending().map_or(EndingPhase::Map, |e| e.phase)),
            Mode::GameOver { .. } => Phase::GameOver,
        }
    }
}

/// What the director remembers of the previous step.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    phase: Phase,
    time_left: u16,
    /// The goal sequence's time bonus so far (0 outside it).
    bonus: u32,
    /// The ending's phase timer (0 outside it).
    ending_timer: u16,
    /// The Oberkellner's state.
    boss: Option<u8>,
    /// Spawn indices of the Operettensänger singing their high C.
    singing: Vec<u16>,
}

impl Snapshot {
    fn of(g: &Game) -> Snapshot {
        let boss = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Oberkellner))
            .map(|e| e.state);
        let singing = g
            .entities()
            .iter()
            .filter(|e| {
                e.kind == EntityKind::Enemy(EnemyKind::Operettensaenger) && e.state == state::SING
            })
            .filter_map(|e| e.spawn)
            .collect();
        Snapshot {
            phase: Phase::of(g),
            time_left: g.time_left(),
            bonus: g.stage_clear().map_or(0, |c| c.bonus),
            ending_timer: g.ending().map_or(0, |e| e.timer),
            boss,
            singing,
        }
    }
}

/// True if `e`'s hitbox overlaps the camera rectangle (§6.1 rule 4).
fn on_screen(g: &Game, e: &Entity) -> bool {
    let c = g.camera();
    let view = Aabb {
        x: Fx::from_px(c.x),
        y: Fx::from_px(c.y),
        w: Fx::from_px(SCREEN_W),
        h: Fx::from_px(SCREEN_H),
    };
    e.hitbox().overlaps(&view)
}

/// True while Resi swims with her hitbox top under the water line (not
/// floating at it, §5 `unterwasser`), read through the core's swim API.
fn under_water(g: &Game) -> bool {
    let p = g.player();
    jnr_core::swim::swim_surface(p, g.map(), &g.tuning().swim).is_some_and(|s| p.y > s)
}

/// The Oberkellner's states that count as "in the fight" (the ZEIT-60
/// bells don't hold the boss music then, §4): from the intro on, and once
/// he is beaten (`DONE`) until the stage piece resumes (`song_due`: the
/// café piece is still scheduled after the Tusch).
fn boss_fighting(boss: Option<u8>, song_due: bool) -> bool {
    use state::oberkellner as st;
    boss.is_some_and(|s| s != st::WAIT && (s != st::DONE || song_due))
}

/// The boss jingles the ZEIT-60 bells wait for instead of cutting them
/// (E63): the bells start when the jingle ends.
fn bells_wait_for(m: Music) -> bool {
    matches!(
        m,
        Music::BossIntro | Music::BossSurprise | Music::BossDefeat
    )
}

/// How long each jingle plays, in game frames (rounded up): the rendered
/// lengths from the bank ([`crate::bank::Bank::jingle_frames`]), so a song
/// that waits for a jingle starts when the jingle is over (E63). A jingle
/// the table lacks (and [`JingleFrames::default`], for tests) uses its
/// design length ([`Music::jingle_ms`]).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JingleFrames {
    pub frames: BTreeMap<Music, u32>,
}

impl JingleFrames {
    /// The frames `m` plays (its rendered length, else its design length).
    pub fn of(&self, m: Music) -> u32 {
        self.frames
            .get(&m)
            .copied()
            .unwrap_or_else(|| ms_to_frames(m.jingle_ms()))
    }
}

/// Turns each step of a game into [`Cue`]s (module docs). Call
/// [`Director::begin`] once for the game's first frame, then
/// [`Director::step`] after every `Game::step`.
#[derive(Clone, Debug, Default)]
pub struct Director {
    /// The jingles' lengths (kept by [`Director::begin`]).
    lengths: JingleFrames,
    /// The frame being observed.
    now: u32,
    prev: Option<Snapshot>,
    start_held: bool,
    voices: Voices,
    /// The song playing (`None`: silence).
    song: Option<Music>,
    /// The last jingle started and not stopped (it may have ended).
    jingle: Option<Music>,
    /// The frame that jingle ends on (its start plus [`JingleFrames::of`]).
    jingle_until: Option<u32>,
    paused: bool,
    /// The frame the pause began on. The frame deadlines below (and
    /// `jingle_until`) count play frames: on resume they move by the
    /// pause's length, and they are not checked while paused (E63).
    paused_at: Option<u32>,
    /// The ZEIT-60 bells hold the song until this frame.
    hurry_until: Option<u32>,
    /// ZEIT reached 60 during a boss jingle: the bells start when it ends.
    bells_due: bool,
    /// A song to start on a frame (the boss loop after the surprise chord,
    /// the café piece after the Tusch, the route map after the fanfare).
    scheduled: Option<(u32, Music)>,
    under_frames: u16,
    over_frames: u16,
    underwater: bool,
    /// Last emitted [`Cue::HoldSong`], [`Cue::HoldJingle`] and
    /// [`Cue::Duck`] states.
    held: bool,
    jingle_held: bool,
    ducked: bool,
}

impl Director {
    /// A director that times jingles by `lengths` (the bank's rendered
    /// lengths in the app and the tools; [`Director::default`] uses the
    /// design lengths).
    pub fn new(lengths: JingleFrames) -> Director {
        Director {
            lengths,
            ..Director::default()
        }
    }

    /// The cues of the game's first frame (before any step): the music of
    /// the mode it starts in.
    pub fn begin(&mut self, game: &Game) -> Vec<Cue> {
        *self = Director::new(std::mem::take(&mut self.lengths));
        self.observe(game, false)
    }

    /// The cues of the step just taken with `input` (`game` is the state
    /// after `Game::step`, its `events()` the step's events).
    pub fn step(&mut self, game: &Game, input: InputFrame) -> Vec<Cue> {
        let start = input.contains(InputFrame::START);
        let pressed = start && !self.start_held;
        self.start_held = start;
        self.observe(game, pressed)
    }

    /// The voice state (for the offline mixer and tests).
    pub fn voices(&self) -> &Voices {
        &self.voices
    }

    fn observe(&mut self, g: &Game, start_pressed: bool) -> Vec<Cue> {
        let now = g.frame();
        self.now = now;
        let snap = Snapshot::of(g);
        let prev = self.prev.take();
        let events = g.events();
        let mut out = Vec::new();
        let mut req: Vec<Play> = Vec::new();

        // Music from events (§4): death, game over, stage clear.
        let died = events.contains(&Event::Died);
        let game_over = events.contains(&Event::GameOver);
        if died {
            self.cut(&mut out, Cut::All);
            self.stop_song(&mut out);
            self.jingle(&mut out, Music::Death);
        }
        if game_over {
            self.cut(&mut out, Cut::All);
            self.stop_song(&mut out);
            self.jingle(&mut out, Music::GameOver);
        }
        let cleared = events.contains(&Event::LevelComplete);
        if cleared {
            self.stop_song(&mut out);
            self.cut(&mut out, Cut::KeepItems);
            let finale = g
                .stage_clear()
                .is_some_and(|c| c.phase == ClearPhase::Serve);
            let fanfare = if finale {
                Music::FanfareFinale
            } else {
                Music::FanfareClear
            };
            self.jingle(&mut out, fanfare);
        }

        // Entering a mode or phase.
        let from = prev.as_ref().map(|p| p.phase);
        if from != Some(snap.phase) {
            self.enter(g, from, snap.phase, start_pressed, &mut out, &mut req);
        }

        // SFX of the step's events.
        if !died && !game_over {
            let power = g.player().power;
            for &e in events {
                if let EventSound::Sfx(s) = sfx_for(e, power) {
                    req.push(s);
                }
            }
            if events.contains(&Event::Bounced) {
                req.retain(|p| p.sfx != Sfx::Stomp);
            }
        }

        // A death or game over step derives nothing more (no boss music,
        // no bells after the death cue).
        if let Some(p) = &prev
            && !died
            && !game_over
        {
            self.derived(g, p, &snap, &mut out, &mut req);
        }

        if req.iter().any(|p| p.sfx == Sfx::BossHit) {
            req.retain(|p| p.sfx != Sfx::Stomp && p.sfx != Sfx::AxeHit);
        }
        if cleared {
            req.retain(|p| p.sfx.voice() == Voice::I);
        }
        if died || game_over {
            req.clear();
        }
        out.extend(self.voices.resolve(now, &req).into_iter().map(Cue::Sfx));

        // The frame deadlines wait while paused (they move on resume).
        if !self.paused {
            if let Some((at, m)) = self.scheduled
                && now >= at
            {
                self.start_song(&mut out, Some(m));
            }
            if self.hurry_until.is_some_and(|t| now >= t) {
                self.hurry_until = None;
            }
            if self.bells_due && !self.boss_jingle_playing(now) {
                self.bells_due = false;
                if snap.phase == Phase::Playing {
                    self.bells(now, snap.boss, &mut out);
                }
            }
        }

        // Under water (Stage 6): a swap after 20 frames below or above.
        if self.song.and_then(Music::underwater_twin).is_some() && snap.phase == Phase::Playing {
            if under_water(g) {
                self.under_frames = self.under_frames.saturating_add(1);
                self.over_frames = 0;
            } else {
                self.over_frames = self.over_frames.saturating_add(1);
                self.under_frames = 0;
            }
            let swap = if self.underwater {
                self.over_frames >= UNDERWATER_SWITCH_FRAMES
            } else {
                self.under_frames >= UNDERWATER_SWITCH_FRAMES
            };
            if swap {
                self.underwater = !self.underwater;
                out.push(Cue::Underwater(self.underwater));
            }
        }

        let hold = self.paused || self.hurry_until.is_some();
        if hold != self.held {
            self.held = hold;
            out.push(Cue::HoldSong(hold));
        }
        if self.paused != self.jingle_held {
            self.jingle_held = self.paused;
            out.push(Cue::HoldJingle(self.paused));
        }
        let duck = !self.paused && self.voices.ducking(now);
        if duck != self.ducked {
            self.ducked = duck;
            out.push(Cue::Duck(duck));
        }
        self.prev = Some(snap);
        out
    }

    /// The cues of entering `to` (from `from`; `None` on the first frame).
    fn enter(
        &mut self,
        g: &Game,
        from: Option<Phase>,
        to: Phase,
        start_pressed: bool,
        out: &mut Vec<Cue>,
        req: &mut Vec<Play>,
    ) {
        // START accepted on the title, „Weiter?", the map or the ending
        // (§6.3 `ui_start`); pause has its own sound.
        let by_start = matches!(
            from,
            Some(Phase::Title | Phase::GameOver | Phase::Map(_) | Phase::Ending(_))
        );
        if start_pressed && by_start {
            req.push(Sfx::UiStart.play());
        }
        match to {
            Phase::Title => {
                self.stop_jingle(out);
                self.start_song(out, Some(Music::Title));
            }
            // After a death the death cue's tail keeps ringing; otherwise
            // the piece stops (START on the title, „Weiter?", the map).
            Phase::StageIntro => {
                if from != Some(Phase::Dying) {
                    self.stop_song(out);
                    self.stop_jingle(out);
                }
            }
            Phase::Playing => {
                if from == Some(Phase::Paused) {
                    self.resume(g.frame());
                    req.push(Sfx::Pause.pitched(PAUSE_OUT_SEMITONES));
                } else {
                    let song = g.level().meta.music.as_deref().and_then(Music::level_song);
                    self.start_song(out, song);
                }
            }
            Phase::Paused => {
                self.paused = true;
                self.paused_at = Some(g.frame());
                self.cut(out, Cut::All);
                req.push(Sfx::Pause.play());
            }
            // The events start their jingles (`Died`, `GameOver`,
            // `LevelComplete`).
            Phase::Dying | Phase::GameOver => {}
            Phase::Clear(ClearPhase::Clear) => req.push(Sfx::TallyEnd.play()),
            Phase::Clear(_) => {}
            // The map's piece waits for the fanfare to end (E63).
            Phase::Map(_) => {
                if !matches!(from, Some(Phase::Map(_))) {
                    match self.jingle_until.filter(|&t| g.frame() < t) {
                        Some(t) => self.scheduled = Some((t, Music::RouteMap)),
                        None => self.start_song(out, Some(Music::RouteMap)),
                    }
                }
            }
            Phase::Ending(EndingPhase::Map) => {
                self.stop_song(out);
                self.jingle(out, Music::EndingMap);
            }
            Phase::Ending(EndingPhase::Scene) => {
                self.stop_jingle(out);
                self.start_song(out, Some(Music::Kaffeehaus));
            }
            // The Kaffeehaus waltz runs on across the bill.
            Phase::Ending(EndingPhase::Bill) => {}
            Phase::Ending(EndingPhase::Credits) => self.start_song(out, Some(Music::Credits)),
            Phase::Ending(EndingPhase::Ende) => {
                self.stop_song(out);
                self.jingle(out, Music::Ende);
            }
        }
    }

    /// Moments derived from state changes (§4, §6.3).
    fn derived(
        &mut self,
        g: &Game,
        prev: &Snapshot,
        snap: &Snapshot,
        out: &mut Vec<Cue>,
        req: &mut Vec<Play>,
    ) {
        let now = g.frame();
        // The tally: a tick on every 3rd frame while the bonus grows.
        if snap.phase == Phase::Clear(ClearPhase::Tally)
            && snap.bonus > prev.bonus
            && g.stage_clear()
                .is_some_and(|c| c.timer.is_multiple_of(TALLY_TICK_EVERY))
        {
            req.push(Sfx::TallyTick.play());
        }
        // ZEIT drops to 60: the church bells (after a boss jingle that
        // plays, which they would cut).
        if snap.phase == Phase::Playing && prev.time_left == 61 && snap.time_left == 60 {
            if self.boss_jingle_playing(now) {
                self.bells_due = true;
            } else {
                self.bells(now, snap.boss, out);
            }
        }
        // The ending's bill: a till per line, a coin for the tip.
        if snap.phase == Phase::Ending(EndingPhase::Bill)
            && prev.phase == snap.phase
            && snap.ending_timer != prev.ending_timer
        {
            let every = g.tuning().ending.bill_line_frames.max(1);
            if snap.ending_timer.is_multiple_of(every) {
                match snap.ending_timer / every {
                    1..=3 => req.push(Sfx::KassaZeile.play()),
                    4 => req.push(Sfx::Muenze.play()),
                    _ => {}
                }
            }
        }
        self.boss(g, prev.boss, snap.boss, out, req);
        self.singers(g, prev, snap, out, req);
    }

    /// The Oberkellner (§5 boss cues, §6.3).
    fn boss(
        &mut self,
        g: &Game,
        was: Option<u8>,
        now_state: Option<u8>,
        out: &mut Vec<Cue>,
        req: &mut Vec<Play>,
    ) {
        use state::oberkellner as st;
        let (Some(was), Some(is)) = (was, now_state) else {
            return;
        };
        if was == is {
            return;
        }
        let now = g.frame();
        let Some(boss) = g
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Oberkellner))
        else {
            return;
        };
        let seen = on_screen(g, boss);
        match is {
            st::INTRO if was == st::WAIT => {
                self.stop_song(out);
                self.jingle(out, Music::BossIntro);
            }
            st::TELEGRAPH => {
                if was == st::INTRO {
                    self.jingle(out, Music::BossSurprise);
                    let at = now.saturating_add(self.lengths.of(Music::BossSurprise));
                    self.scheduled = Some((at, Music::Boss));
                }
                if seen {
                    req.push(Sfx::BossTelegraph.play());
                }
            }
            st::DASH => {
                if self.voices.stop(Sfx::BossTelegraph, now) {
                    out.push(Cue::StopSfx(Sfx::BossTelegraph));
                }
            }
            st::BREATHE if seen => req.push(Sfx::BossBreathe.play()),
            st::STUN if seen => req.push(Sfx::BossHit.play()),
            st::VOLLEY if seen => req.push(Sfx::BillsHang.play()),
            st::DONE => {
                self.stop_song(out);
                self.jingle(out, Music::BossDefeat);
                // The Tusch, 1 s of silence, then the stage piece again.
                let at = now.saturating_add(self.lengths.of(Music::BossDefeat) + 60);
                let song = g.level().meta.music.as_deref().and_then(Music::level_song);
                self.scheduled = song.map(|m| (at, m));
            }
            _ => {}
        }
    }

    /// The Operettensänger: the high C on the first frame of `SING`, cut
    /// at once when he is defeated mid-note.
    fn singers(
        &mut self,
        g: &Game,
        prev: &Snapshot,
        snap: &Snapshot,
        out: &mut Vec<Cue>,
        req: &mut Vec<Play>,
    ) {
        let now = g.frame();
        let by_spawn = |spawn: u16| g.entities().iter().find(|e| e.spawn == Some(spawn));
        for &s in snap.singing.iter().filter(|s| !prev.singing.contains(s)) {
            if by_spawn(s).is_some_and(|e| on_screen(g, e)) {
                req.push(Sfx::HohesC.play());
            }
        }
        let silenced = prev.singing.iter().any(|&s| {
            by_spawn(s).is_none_or(|e| e.kind != EntityKind::Enemy(EnemyKind::Operettensaenger))
        });
        if silenced && self.voices.stop(Sfx::HohesC, now) {
            out.push(Cue::StopSfx(Sfx::HohesC));
        }
    }

    /// The ZEIT-60 church bells; outside the boss fight they hold the
    /// stage piece while they ring.
    fn bells(&mut self, now: u32, boss: Option<u8>, out: &mut Vec<Cue>) {
        self.jingle(out, Music::Hurry);
        if !boss_fighting(boss, self.scheduled.is_some()) {
            self.hurry_until = Some(now.saturating_add(self.lengths.of(Music::Hurry)));
        }
    }

    /// True while a boss jingle the bells wait for plays.
    fn boss_jingle_playing(&self, now: u32) -> bool {
        self.jingle.is_some_and(bells_wait_for) && self.jingle_until.is_some_and(|t| now < t)
    }

    /// Leaving the pause on frame `now`: the frame deadlines move by the
    /// pause's length (the held jingle resumes in place too).
    fn resume(&mut self, now: u32) {
        self.paused = false;
        let d = self.paused_at.take().map_or(0, |p| now.saturating_sub(p));
        self.hurry_until = self.hurry_until.map(|t| t.saturating_add(d));
        self.jingle_until = self.jingle_until.map(|t| t.saturating_add(d));
        self.scheduled = self.scheduled.map(|(t, m)| (t.saturating_add(d), m));
    }

    fn cut(&mut self, out: &mut Vec<Cue>, cut: Cut) {
        self.voices.cut(cut);
        out.push(Cue::CutSfx(cut));
    }

    /// Starts `song` from its first sample (`None`: silence) and clears
    /// what belonged to the old one (the bells' hold, a scheduled song,
    /// the underwater state).
    fn start_song(&mut self, out: &mut Vec<Cue>, song: Option<Music>) {
        self.scheduled = None;
        self.hurry_until = None;
        self.underwater = false;
        self.under_frames = 0;
        self.over_frames = 0;
        match song {
            Some(m) => {
                self.song = Some(m);
                out.push(Cue::Song(m));
            }
            None => self.stop_song(out),
        }
    }

    fn stop_song(&mut self, out: &mut Vec<Cue>) {
        self.scheduled = None;
        self.hurry_until = None;
        self.bells_due = false;
        self.underwater = false;
        if self.song.take().is_some() {
            out.push(Cue::StopSong);
        }
    }

    fn jingle(&mut self, out: &mut Vec<Cue>, m: Music) {
        self.jingle = Some(m);
        self.jingle_until = Some(self.now.saturating_add(self.lengths.of(m)));
        out.push(Cue::Jingle(m));
    }

    /// Stops the last jingle, also when it has ended already (the design
    /// lengths are not the rendered ones).
    fn stop_jingle(&mut self, out: &mut Vec<Cue>) {
        self.jingle_until = None;
        if self.jingle.take().is_some() {
            out.push(Cue::StopJingle);
        }
    }
}

/// One cue-log line: the frame, right-aligned to 6 digits, and the cue.
pub fn log_line(frame: u32, cue: &Cue) -> String {
    format!("{frame:>6} {cue}")
}

#[cfg(test)]
mod tests;
