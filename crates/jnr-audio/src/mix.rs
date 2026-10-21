//! Offline playback of a cue timeline (E60, D-114): what the app's
//! per-sound playback makes of the [`cues`](crate::cues) of a run, mixed
//! into one buffer. `cargo xtask replay <file> --audio` uses it to prove
//! the headroom offline (D-114 has no master bus), and tests can use it to
//! check timing; the app (E59) follows the same rules with quad-snd.
//!
//! Playback rules (the cue semantics of `cues`, as D-114 implements them):
//!
//! - A cue on game frame `f` acts at sample `f ×` [`FRAMES_PER_TICK`].
//! - [`Cue::Sfx`]: stop the sound last started on the same [`Voice`] and
//!   any live copy of the same id (an id has at most one live copy), then
//!   play the variant from its first sample at [`SFX_VOLUME_PCT`].
//! - [`Cue::StopSfx`] stops one id, [`Cue::CutSfx`] every id on the voices
//!   it names. Stops are hard cuts (quad-snd has no fades).
//! - [`Cue::Song`]: stop the song (and its twin), start the new one looping
//!   from sample 0, and its underwater twin muted on the same sample.
//!   [`Cue::Jingle`]: stop the jingle, play the new one once.
//! - Music volume: [`MUSIC_VOLUME_PCT`], [`DUCK_VOLUME_PCT`] while ducked;
//!   0 for the song while held, 0 for the jingle while held. Under water the
//!   song and its twin swap volumes; a new song starts above water.
//! - One-shots ring out past the run's end; loops stop at the end of the
//!   run's last frame.
//!
//! Integer only, like the rest of the crate.

use std::collections::{BTreeMap, BTreeSet};

use crate::FRAMES_PER_TICK;
use crate::cues::{Cue, Cut, Music, Play, Sfx, Voice};
use crate::render::{Pcm, Render, to_pcm};

/// Playback volume of every SFX (each is rendered at its design peak,
/// `docs/audio-design.md` §3, so it plays at full volume).
pub const SFX_VOLUME_PCT: u32 = 100;
/// Playback volume of songs and jingles (rendered at the music bus level).
pub const MUSIC_VOLUME_PCT: u32 = 100;
/// The ducked music volume: −6 dB (§3, 50 % = −6.02 dB).
pub const DUCK_VOLUME_PCT: u32 = 50;

/// A sound the cues can start.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sound {
    Sfx(Play),
    Music(Music),
}

impl std::fmt::Display for Sound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            Sound::Sfx(Play { sfx, semitones: 0 }) => f.write_str(sfx.id()),
            Sound::Sfx(Play { sfx, semitones }) => write!(f, "{} {semitones:+}", sfx.id()),
            Sound::Music(m) => f.write_str(m.id()),
        }
    }
}

/// The rendered sounds a mix can use; a sound that is missing plays as
/// silence and is listed in [`Mix::missing`].
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub sounds: BTreeMap<Sound, Pcm>,
}

/// Where a started sound is heard in the mix: samples `[start, end)` at a
/// volume above 0 (a held or muted stretch splits a sound into spans).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub sound: Sound,
    pub start: usize,
    pub end: usize,
    /// A looping song (it may be muted for stretches: held, the twin
    /// above water); otherwise a one-shot heard from its first sample.
    pub looping: bool,
}

/// The offline mix of a timeline.
#[derive(Clone, Debug)]
pub struct Mix {
    /// The clamped PCM and the number of clamped samples.
    pub render: Render,
    /// Where each sound of the bank is heard, in start order.
    pub spans: Vec<Span>,
    /// Sounds the timeline started that the bank lacks (silent).
    pub missing: BTreeSet<Sound>,
    /// The run's length in samples (`(end_frame + 1) ×` [`FRAMES_PER_TICK`]);
    /// the PCM is longer when a one-shot rings out.
    pub run_samples: usize,
}

/// One started sound.
struct Instance {
    sound: Sound,
    start: usize,
    looping: bool,
    /// The sample where it was stopped.
    stop: Option<usize>,
    /// Gain changes: (from sample, volume %), the first at `start`.
    gains: Vec<(usize, u32)>,
}

impl Instance {
    fn new(sound: Sound, start: usize, looping: bool, gain: u32) -> Instance {
        Instance {
            sound,
            start,
            looping,
            stop: None,
            gains: vec![(start, gain)],
        }
    }

    fn stop(&mut self, at: usize) {
        self.stop = Some(self.stop.map_or(at, |s| s.min(at)));
    }

    fn set_gain(&mut self, at: usize, gain: u32) {
        if self.gains.last().map(|g| g.1) != Some(gain) {
            self.gains.push((at, gain));
        }
    }
}

/// Playback state while the cues are applied.
#[derive(Default)]
struct Player {
    instances: Vec<Instance>,
    live_sfx: BTreeMap<Sfx, usize>,
    last_on_voice: BTreeMap<Voice, Sfx>,
    song: Option<usize>,
    twin: Option<usize>,
    jingle: Option<usize>,
    hold_song: bool,
    hold_jingle: bool,
    duck: bool,
    underwater: bool,
}

impl Player {
    fn start(&mut self, sound: Sound, at: usize, looping: bool, gain: u32) -> usize {
        self.instances.push(Instance::new(sound, at, looping, gain));
        self.instances.len() - 1
    }

    fn stop_sfx(&mut self, sfx: Sfx, at: usize) {
        if let Some(i) = self.live_sfx.remove(&sfx) {
            self.instances[i].stop(at);
        }
    }

    fn stop_song(&mut self, at: usize) {
        for i in [self.song.take(), self.twin.take()].into_iter().flatten() {
            self.instances[i].stop(at);
        }
    }

    fn apply(&mut self, cue: Cue, at: usize) {
        match cue {
            Cue::Sfx(play) => {
                let voice = play.sfx.voice();
                if let Some(prev) = self.last_on_voice.get(&voice).copied() {
                    self.stop_sfx(prev, at);
                }
                self.stop_sfx(play.sfx, at);
                let i = self.start(Sound::Sfx(play), at, false, SFX_VOLUME_PCT);
                self.live_sfx.insert(play.sfx, i);
                self.last_on_voice.insert(voice, play.sfx);
            }
            Cue::StopSfx(sfx) => self.stop_sfx(sfx, at),
            Cue::CutSfx(cut) => {
                let hit: Vec<Sfx> = self
                    .live_sfx
                    .keys()
                    .copied()
                    .filter(|s| cut == Cut::All || s.voice() != Voice::I)
                    .collect();
                for s in hit {
                    self.stop_sfx(s, at);
                }
            }
            Cue::Song(m) => {
                self.stop_song(at);
                self.underwater = false;
                self.song = Some(self.start(Sound::Music(m), at, true, 0));
                self.twin = m
                    .underwater_twin()
                    .map(|t| self.start(Sound::Music(t), at, true, 0));
            }
            Cue::StopSong => self.stop_song(at),
            Cue::Jingle(m) => {
                if let Some(i) = self.jingle.take() {
                    self.instances[i].stop(at);
                }
                self.jingle = Some(self.start(Sound::Music(m), at, false, 0));
            }
            Cue::StopJingle => {
                if let Some(i) = self.jingle.take() {
                    self.instances[i].stop(at);
                }
            }
            Cue::HoldSong(b) => self.hold_song = b,
            Cue::HoldJingle(b) => self.hold_jingle = b,
            Cue::Duck(b) => self.duck = b,
            Cue::Underwater(b) => self.underwater = b,
        }
    }

    /// Sets the music volumes from the hold, duck and underwater state.
    fn music_gains(&mut self, at: usize) {
        let music = if self.duck {
            DUCK_VOLUME_PCT
        } else {
            MUSIC_VOLUME_PCT
        };
        let song = if self.hold_song { 0 } else { music };
        let (main, twin) = if self.underwater {
            (0, song)
        } else {
            (song, 0)
        };
        let jingle = if self.hold_jingle { 0 } else { music };
        for (i, g) in [(self.song, main), (self.twin, twin), (self.jingle, jingle)] {
            if let Some(i) = i {
                self.instances[i].set_gain(at, g);
            }
        }
    }
}

/// Renders an SFX patch as the variant `semitones` away from its authored
/// pitch (`cues::PITCHED`; 0 = [`crate::render_sfx`]). The app (E58b/E59)
/// renders its pitched WAVs the same way.
pub fn render_variant(patch: &crate::patch::Sfx, semitones: i8) -> Render {
    let mut out = Vec::new();
    crate::render::add_sfx_shifted(&mut out, 0, patch, 100, semitones as i32 * 100);
    to_pcm(&out)
}

/// Mixes `timeline` (game frame, cue; in order) over a run that ends with
/// frame `end_frame`, using the sounds in `bank`.
pub fn mix(bank: &Bank, timeline: &[(u32, Cue)], end_frame: u32) -> Mix {
    let tick = FRAMES_PER_TICK as usize;
    let run_samples = (end_frame as usize + 1) * tick;
    let mut player = Player::default();
    let mut k = 0;
    while k < timeline.len() {
        let frame = timeline[k].0;
        let at = frame as usize * tick;
        while k < timeline.len() && timeline[k].0 == frame {
            player.apply(timeline[k].1, at);
            k += 1;
        }
        player.music_gains(at);
    }
    let mut out: Vec<i32> = vec![0; run_samples];
    let mut spans: Vec<Span> = Vec::new();
    let mut missing = BTreeSet::new();
    for inst in &player.instances {
        let Some(pcm) = bank.sounds.get(&inst.sound) else {
            missing.insert(inst.sound);
            continue;
        };
        let len = pcm.samples.len();
        let natural = if inst.looping {
            run_samples.max(inst.start)
        } else {
            inst.start + len
        };
        let end = inst
            .stop
            .map_or(natural, |s| s.min(natural))
            .max(inst.start);
        if len == 0 || end == inst.start {
            continue;
        }
        if out.len() < end {
            out.resize(end, 0);
        }
        let first_span = spans.len();
        for (g, &(from, gain)) in inst.gains.iter().enumerate() {
            let to = inst.gains.get(g + 1).map_or(end, |n| n.0.min(end));
            if gain == 0 || from >= to {
                continue;
            }
            let own = spans.len() > first_span;
            match spans.last_mut() {
                Some(sp) if own && sp.end == from => sp.end = to,
                _ => spans.push(Span {
                    sound: inst.sound,
                    start: from,
                    end: to,
                    looping: inst.looping,
                }),
            }
            for (t, o) in out.iter_mut().enumerate().take(to).skip(from) {
                let s = pcm.samples[(t - inst.start) % len] as i32;
                *o += if gain == 100 {
                    s
                } else {
                    s * gain as i32 / 100
                };
            }
        }
    }
    Mix {
        render: to_pcm(&out),
        spans,
        missing,
        run_samples,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SAMPLE_RATE;

    const T: usize = FRAMES_PER_TICK as usize;

    fn pcm(samples: Vec<i16>) -> Pcm {
        Pcm {
            rate: SAMPLE_RATE,
            samples,
        }
    }

    fn bank(entries: &[(Sound, Vec<i16>)]) -> Bank {
        Bank {
            sounds: entries.iter().map(|(s, p)| (*s, pcm(p.clone()))).collect(),
        }
    }

    const JUMP: Sound = Sound::Sfx(Sfx::Jump.play());
    const OBLATE: Sound = Sound::Sfx(Sfx::Oblate.play());

    #[test]
    fn a_cue_starts_its_sound_at_frame_times_735() {
        let b = bank(&[(JUMP, vec![1000; 10])]);
        let m = mix(&b, &[(3, Cue::Sfx(Sfx::Jump.play()))], 5);
        let s = &m.render.pcm.samples;
        assert_eq!(m.run_samples, 6 * T);
        assert_eq!(s[3 * T - 1], 0);
        assert_eq!(s[3 * T..3 * T + 10], [1000; 10]);
        assert_eq!(s[3 * T + 10], 0);
        assert_eq!(m.spans[0].start, 3 * T);
        assert_eq!(m.spans[0].end, 3 * T + 10);
    }

    #[test]
    fn a_retrigger_restarts_and_the_voice_holds_one_sound() {
        // Two jumps one frame apart: the first is cut where the second starts.
        let b = bank(&[(JUMP, vec![100; 2 * T])]);
        let m = mix(
            &b,
            &[
                (0, Cue::Sfx(Sfx::Jump.play())),
                (1, Cue::Sfx(Sfx::Jump.play())),
            ],
            4,
        );
        let s = &m.render.pcm.samples;
        assert!(s[..3 * T].iter().all(|&v| v == 100), "never stacked");
        assert_eq!(s[3 * T], 0);
        assert_eq!(m.spans[0].end, T);
        // A different id on the same voice (P) also stops the playing one.
        let hurt = Sound::Sfx(Sfx::Hurt.play());
        let b = bank(&[(JUMP, vec![100; 2 * T]), (hurt, vec![7; 2 * T])]);
        let m = mix(
            &b,
            &[
                (0, Cue::Sfx(Sfx::Jump.play())),
                (1, Cue::Sfx(Sfx::Hurt.play())),
            ],
            4,
        );
        let s = &m.render.pcm.samples;
        assert_eq!(s[T - 1], 100);
        assert_eq!(s[T], 7);
        // Different voices overlap and add up.
        let b = bank(&[(JUMP, vec![100; 2 * T]), (OBLATE, vec![7; 2 * T])]);
        let m = mix(
            &b,
            &[
                (0, Cue::Sfx(Sfx::Jump.play())),
                (1, Cue::Sfx(Sfx::Oblate.play())),
            ],
            4,
        );
        assert_eq!(m.render.pcm.samples[T], 107);
    }

    #[test]
    fn cuts_stop_sounds_and_keep_items_keeps_the_items_voice() {
        let b = bank(&[(JUMP, vec![100; 4 * T]), (OBLATE, vec![7; 4 * T])]);
        let start = [
            (0, Cue::Sfx(Sfx::Jump.play())),
            (0, Cue::Sfx(Sfx::Oblate.play())),
        ];
        let keep = [start[0], start[1], (1, Cue::CutSfx(Cut::KeepItems))];
        let s = mix(&b, &keep, 3).render.pcm.samples;
        assert_eq!(s[T], 7);
        let all = [start[0], start[1], (1, Cue::CutSfx(Cut::All))];
        let s = mix(&b, &all, 3).render.pcm.samples;
        assert_eq!(s[T], 0);
        let one = [start[0], start[1], (2, Cue::StopSfx(Sfx::Oblate))];
        let s = mix(&b, &one, 3).render.pcm.samples;
        assert_eq!((s[T], s[2 * T]), (107, 100));
    }

    #[test]
    fn a_pitched_variant_is_its_own_sound_but_the_same_id() {
        let low = Sound::Sfx(Sfx::Jump.pitched(-5));
        let b = bank(&[(JUMP, vec![100; 4 * T]), (low, vec![3; 4 * T])]);
        let m = mix(
            &b,
            &[
                (0, Cue::Sfx(Sfx::Jump.play())),
                (1, Cue::Sfx(Sfx::Jump.pitched(-5))),
            ],
            2,
        );
        assert_eq!(m.render.pcm.samples[T], 3);
        assert_eq!(m.spans[1].sound, low);
    }

    #[test]
    fn songs_loop_and_follow_hold_duck_and_underwater() {
        let see = Sound::Music(Music::Wolfgangsee);
        let unter = Sound::Music(Music::Unterwasser);
        // A loop of 1.5 frames: 400, then 200.
        let mut lp = vec![400; T];
        lp.extend(vec![200; T / 2]);
        let b = bank(&[(see, lp.clone()), (unter, vec![-30; T])]);
        let tl = [
            (1, Cue::Song(Music::Wolfgangsee)),
            (3, Cue::Duck(true)),
            (4, Cue::Duck(false)),
            (5, Cue::HoldSong(true)),
            (6, Cue::HoldSong(false)),
            (7, Cue::Underwater(true)),
            (8, Cue::StopSong),
        ];
        let m = mix(&b, &tl, 9);
        let s = &m.render.pcm.samples;
        assert_eq!(s[T - 1], 0);
        assert_eq!(s[T], 400);
        // Looped: sample T + len is the loop's first sample again.
        assert_eq!(s[T + lp.len()], 400);
        assert_eq!(s[T + lp.len() - 1], 200);
        let at = |f: usize| s[f * T] as i32;
        let loop_at = |f: usize| lp[(f * T - T) % lp.len()] as i32;
        assert_eq!(at(3), loop_at(3) / 2, "ducked −6 dB");
        assert_eq!(at(4), loop_at(4));
        assert_eq!(at(5), 0, "held");
        assert_eq!(at(6), loop_at(6), "resumes in place");
        assert_eq!(at(7), -30, "the twin under water");
        assert_eq!(at(8), 0);
        // Heard: the song up to the hold and after it, then the twin.
        let spans: Vec<(Sound, usize, usize)> = m
            .spans
            .iter()
            .map(|sp| (sp.sound, sp.start, sp.end))
            .collect();
        assert_eq!(
            spans,
            [(see, T, 5 * T), (see, 6 * T, 7 * T), (unter, 7 * T, 8 * T)]
        );
        assert!(m.missing.is_empty());
    }

    #[test]
    fn jingles_play_once_and_a_held_jingle_is_silent() {
        let fan = Sound::Music(Music::FanfareClear);
        let b = bank(&[(fan, vec![50; 3 * T])]);
        let tl = [
            (0, Cue::Jingle(Music::FanfareClear)),
            (1, Cue::HoldJingle(true)),
            (2, Cue::HoldJingle(false)),
        ];
        let m = mix(&b, &tl, 1);
        let s = &m.render.pcm.samples;
        assert_eq!((s[0], s[T], s[2 * T]), (50, 0, 50));
        // Rings out past the run's end (2 frames), then stops.
        assert_eq!(s.len(), 3 * T);
    }

    #[test]
    fn missing_sounds_are_silent_and_listed() {
        let m = mix(
            &Bank::default(),
            &[
                (0, Cue::Song(Music::Salzburg)),
                (0, Cue::Sfx(Sfx::Jump.play())),
            ],
            1,
        );
        assert!(m.render.pcm.samples.iter().all(|&v| v == 0));
        assert_eq!(
            m.missing.into_iter().collect::<Vec<_>>(),
            vec![JUMP, Sound::Music(Music::Salzburg)]
        );
    }

    #[test]
    fn overlapping_loud_sounds_clamp_and_are_counted() {
        let hurt = Sound::Sfx(Sfx::Hurt.play());
        let b = bank(&[
            (JUMP, vec![20_000; 10]),
            (hurt, vec![-1; 10]),
            (OBLATE, vec![20_000; 10]),
        ]);
        let m = mix(
            &b,
            &[
                (0, Cue::Sfx(Sfx::Jump.play())),
                (0, Cue::Sfx(Sfx::Oblate.play())),
            ],
            0,
        );
        assert_eq!(m.render.clamped, 10);
        assert_eq!(m.render.pcm.samples[0], i16::MAX);
    }
}
