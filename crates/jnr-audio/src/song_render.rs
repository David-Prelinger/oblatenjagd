//! Songs to PCM.
//!
//! - **Time**: a song of `S` steps is `L` samples long; step `k` starts at
//!   sample `k·L / S` (rounded down, so the rounding is spread evenly like
//!   Bresenham). `L` is the natural length (tempo, grid, bars) for a jingle;
//!   for a loop it is rounded to the nearest multiple of [`LOOP_ALIGN`]
//!   frames (D-114: quad-snd's native loop bug), a tempo change of at most
//!   2048 frames per loop (≤ 0.15 % from a 31 s loop, ≤ 0.24 % at 20 s).
//! - **Channels are monophonic** (chip style): a new note, chord or hit cuts
//!   what still sounds on its channel with a [`CUT_FADE`] fade; a rest
//!   doesn't, so releases ring into rests. A chord is one voice arpeggiated
//!   every `arp_ms` (0: its notes sound together); kit hits in `[K,c]` sound
//!   together.
//! - **Loops wrap**: the tails that run past the loop end are added back at
//!   the start (the last notes are cut by the channel's first onset, one
//!   loop later), so the seam has no click: the rendered loop equals the
//!   steady state of playing it forever.
//! - **Cache**: identical voices (channel, notes or hits, gate, velocity, cut)
//!   render identically, so each is rendered once and added wherever it
//!   recurs (a 90 s, 6-channel song in a few ms).

use std::collections::BTreeMap;

use crate::error::{ErrorKind, ParseError};
use crate::patch::{Arp, Instrument, InstrumentKind, Layer};
use crate::render::{Render, add_note, add_sfx, to_pcm};
use crate::song::{Event, Song, SongKind, Sound};
use crate::{SAMPLE_RATE, note::Note};

/// A loop's length is a multiple of this many frames (D-114).
pub const LOOP_ALIGN: u32 = 4096;
/// Fade (samples, 2 ms) when a new onset cuts a sounding voice: the cut of
/// `render::add_note_until`/`add_sfx_until`, applied to the summed voice
/// (identical for one layer, tested; a multi-layer voice may differ by a
/// few LSBs of rounding, since those fade each layer).
pub use crate::render::CUT_FADE;

/// Allowed `tempo_pct` (a render variant, not a new piece).
pub const TEMPO_PCT: (u32, u32) = (50, 200);

/// Render variants of one song (D-114).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// Tempo in percent of the written one (112: the "hurry" render at ZEIT 60).
    pub tempo_pct: u32,
    /// Render to exactly this many frames (a loop: a multiple of
    /// [`LOOP_ALIGN`]), e.g. the underwater track at Stage 6's length.
    pub length_frames: Option<u32>,
}

impl Default for RenderOptions {
    fn default() -> Self {
        RenderOptions {
            tempo_pct: 100,
            length_frames: None,
        }
    }
}

/// A rendered song.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SongRender {
    /// A loop: exactly `length_frames` samples. A jingle: up to the end of
    /// its last release.
    pub render: Render,
    /// The song's timed length `L` (a loop's loop length).
    pub length_frames: u32,
    /// The length at the written tempo before alignment.
    pub natural_frames: u64,
    /// `(L − natural) / natural` in parts per million (+ = slower).
    pub stretch_ppm: i64,
    /// Voices added, and how many of them had to be synthesised.
    pub voices: u32,
    pub rendered_voices: u32,
}

/// The length at `tempo_pct` of the written tempo, rounded (frames).
pub fn natural_frames(song: &Song, tempo_pct: u32) -> u64 {
    let num = song.total_steps() as u64 * 60 * SAMPLE_RATE as u64 * 100;
    let den = song.tempo_bpm as u64 * song.steps_per_beat as u64 * tempo_pct.max(1) as u64;
    (num + den / 2) / den
}

/// The rendered length `L` of a song (see the module docs).
pub fn song_frames(song: &Song, opts: &RenderOptions) -> Result<u32, String> {
    let (lo, hi) = TEMPO_PCT;
    if !(lo..=hi).contains(&opts.tempo_pct) {
        return Err(format!(
            "tempo_pct = {} is outside {lo}..={hi}",
            opts.tempo_pct
        ));
    }
    let nat = natural_frames(song, opts.tempo_pct);
    match (opts.length_frames, song.kind) {
        (Some(0), _) => Err("length_frames must be > 0".into()),
        (Some(l), SongKind::Loop) if l % LOOP_ALIGN != 0 => Err(format!(
            "a loop's length must be a multiple of {LOOP_ALIGN} frames, not {l}"
        )),
        (Some(l), _) => Ok(l),
        (None, SongKind::Loop) => {
            let a = LOOP_ALIGN as u64;
            Ok((((nat + a / 2) / a).max(1) * a) as u32)
        }
        (None, SongKind::Jingle) => Ok(nat.max(1) as u32),
    }
}

/// `opts` with `length_frames` resolved: a `length_of` song (D-114, the
/// underwater track) gets its target loop's length at the same tempo.
pub fn resolve_length(
    songs: &BTreeMap<String, Song>,
    song: &Song,
    opts: &RenderOptions,
) -> Result<RenderOptions, ParseError> {
    let (Some(target), None) = (&song.length_of, opts.length_frames) else {
        return Ok(*opts);
    };
    let err = |m: String| ParseError {
        file: song.file.clone(),
        line: song.at.length_of.line,
        col: song.at.length_of.col,
        kind: ErrorKind::Music,
        message: m,
    };
    let t = songs
        .get(target)
        .ok_or_else(|| err(format!("`length_of` names no song `{target}`")))?;
    if t.kind != SongKind::Loop {
        return Err(err(format!("`length_of`: `{target}` is not a loop")));
    }
    let at = RenderOptions {
        length_frames: None,
        ..*opts
    };
    Ok(RenderOptions {
        length_frames: Some(song_frames(t, &at).map_err(err)?),
        ..*opts
    })
}

/// Renders song `id` of `songs` with [`resolve_length`] applied: what the
/// game (E58) and xtask both call.
pub fn render_in(
    songs: &BTreeMap<String, Song>,
    instruments: &BTreeMap<String, Instrument>,
    id: &str,
    opts: &RenderOptions,
) -> Result<SongRender, ParseError> {
    let song = songs.get(id).ok_or_else(|| ParseError {
        file: format!("{id}.song"),
        line: 1,
        col: 1,
        kind: ErrorKind::Id,
        message: format!("no song `{id}`"),
    })?;
    render_song(song, instruments, &resolve_length(songs, song, opts)?)
}

/// Each channel's instrument, checked against its tokens.
pub fn bind<'a>(
    song: &Song,
    instruments: &'a BTreeMap<String, Instrument>,
) -> Result<Vec<&'a Instrument>, ParseError> {
    let err = |at: crate::song::Pos, msg: String| ParseError {
        file: song.file.clone(),
        line: at.line,
        col: at.col,
        kind: ErrorKind::Channel,
        message: msg,
    };
    let mut out = Vec::new();
    for (ci, ch) in song.channels.iter().enumerate() {
        let inst = instruments.get(&ch.instrument).ok_or_else(|| {
            err(
                ch.at,
                format!(
                    "channel `{}`: no instrument `{}` (audio/instruments/{}.inst)",
                    ch.name, ch.instrument, ch.instrument
                ),
            )
        })?;
        for e in &song.events[ci] {
            match (&inst.kind, &e.sound) {
                (InstrumentKind::Melodic { .. }, Sound::Hits(h)) => {
                    return Err(err(
                        e.at,
                        format!(
                            "`{}` is a kit letter, but `{}` plays the melodic `{}`",
                            h[0], ch.name, inst.id
                        ),
                    ));
                }
                (InstrumentKind::Kit { .. }, Sound::Notes(n)) => {
                    return Err(err(
                        e.at,
                        format!(
                            "`{}` is a note, but `{}` plays the kit `{}`",
                            n[0].note, ch.name, inst.id
                        ),
                    ));
                }
                (InstrumentKind::Kit { hits }, Sound::Hits(h)) => {
                    if let Some(c) = h.iter().find(|c| !hits.contains_key(c)) {
                        let known: String = hits.keys().collect();
                        return Err(err(
                            e.at,
                            format!("kit `{}` has no letter `{c}` (it has {known})", inst.id),
                        ));
                    }
                }
                _ => {}
            }
        }
        out.push(inst);
    }
    Ok(out)
}

/// Cache key of one voice: channel, notes (MIDI) or hits, gate, gain.
type Key = (usize, Vec<u8>, u32, u32);

/// Renders `song` (see the module docs).
pub fn render_song(
    song: &Song,
    instruments: &BTreeMap<String, Instrument>,
    opts: &RenderOptions,
) -> Result<SongRender, ParseError> {
    let insts = bind(song, instruments)?;
    let l = song_frames(song, opts).map_err(|m| ParseError {
        file: song.file.clone(),
        line: song.at.meta.line,
        col: song.at.meta.col,
        kind: ErrorKind::Music,
        message: m,
    })?;
    let s = song.total_steps() as u64;
    let at = |step: u32| (step as u64 * l as u64 / s) as usize;
    let looped = song.kind == SongKind::Loop;

    let mut mix: Vec<i32> = vec![0; l as usize];
    let mut cache: BTreeMap<Key, Vec<i32>> = BTreeMap::new();
    let (mut voices, mut rendered) = (0u32, 0u32);
    for (ci, ch) in song.channels.iter().enumerate() {
        let onsets: Vec<&Event> = song.events[ci]
            .iter()
            .filter(|e| !matches!(e.sound, Sound::Rest))
            .collect();
        for (i, e) in onsets.iter().enumerate() {
            let start = at(e.step);
            let mut gate = (at(e.step + e.len) - start) as u32;
            if e.staccato {
                gate = (gate / 2).max(1);
            }
            let next = match onsets.get(i + 1) {
                Some(n) => Some(at(n.step)),
                None if looped => Some(at(onsets[0].step) + l as usize),
                None => None,
            };
            // Events are in time order (the parser checks every bar); if a
            // hand-made song isn't, a later onset cuts at once instead of
            // underflowing.
            let cut = next.map_or(usize::MAX, |n| n.saturating_sub(start));
            let gain = ch.volume_pct * e.velocity_pct / 100;
            let ids: Vec<u8> = match &e.sound {
                Sound::Notes(t) => t.iter().map(|t| t.note.0).collect(),
                Sound::Hits(h) => h.iter().map(|&c| c as u8).collect(),
                Sound::Rest | Sound::Tie => continue,
            };
            let key: Key = (ci, ids, gate, gain);
            let voice = cache.entry(key).or_insert_with_key(|k| {
                rendered += 1;
                voice(insts[ci], ch.arp_ms, &k.1, gate, gain)
            });
            voices += 1;
            add_cut(&mut mix, start, voice, cut);
        }
    }
    if looped {
        let (head, tail) = mix.split_at_mut(l as usize);
        for chunk in tail.chunks(l as usize) {
            for (d, v) in head.iter_mut().zip(chunk) {
                *d += v;
            }
        }
        mix.truncate(l as usize);
    }
    let nat = natural_frames(song, opts.tempo_pct);
    Ok(SongRender {
        render: to_pcm(&mix),
        length_frames: l,
        natural_frames: nat,
        stretch_ppm: (l as i64 - nat as i64) * 1_000_000 / nat.max(1) as i64,
        voices,
        rendered_voices: rendered,
    })
}

/// Adds `voice` at `start`, cut after `cut` samples with a [`CUT_FADE`]
/// linear fade (a new onset on the same channel).
fn add_cut(mix: &mut Vec<i32>, start: usize, voice: &[i32], cut: usize) {
    if cut == 0 {
        return; // cut before it starts: silent, as in `add_note_until`
    }
    let keep = cut.min(voice.len());
    let end = cut.saturating_add(CUT_FADE).min(voice.len());
    if mix.len() < start + end {
        mix.resize(start + end, 0);
    }
    for (d, v) in mix[start..start + keep].iter_mut().zip(voice) {
        *d += v;
    }
    let fade = CUT_FADE as i64;
    for (j, i) in (keep..end).enumerate() {
        mix[start + i] += (voice[i] as i64 * (fade - j as i64) / fade) as i32;
    }
}

/// One voice: notes (one, stacked, or arpeggiated) or kit hits, uncut.
fn voice(inst: &Instrument, arp_ms: u32, ids: &[u8], gate: u32, gain: u32) -> Vec<i32> {
    let mut out = Vec::new();
    match &inst.kind {
        InstrumentKind::Kit { hits } => {
            for &c in ids {
                if let Some(sfx) = hits.get(&(c as char)) {
                    add_sfx(&mut out, 0, sfx, gain);
                }
            }
        }
        InstrumentKind::Melodic { layers, .. } => {
            if ids.len() > 1 && arp_ms > 0 {
                let root = ids[0];
                let arp = Arp {
                    every_ms: arp_ms,
                    semitones: ids.iter().map(|&n| n as i32 - root as i32).collect(),
                };
                let arped: Vec<Layer> = layers
                    .iter()
                    .map(|l| Layer {
                        arp: arp.clone(),
                        ..l.clone()
                    })
                    .collect();
                add_note(&mut out, 0, &arped, Note(root), gate, gain);
            } else {
                for &n in ids {
                    add_note(&mut out, 0, layers, Note(n), gate, gain);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::{add_note_until, add_sfx_until};

    const LEAD: &str = "range = [\"c3\", \"c6\"]\n[[layer]]\nwave = \"pulse\"\nattack_ms = 5\ndecay_ms = 50\nsustain_pct = 60\nrelease_ms = 80\nvolume_pct = 40\n";
    const KIT: &str = "[kit]\nK = \"kick\"\n[[patch.kick.layer]]\nwave = \"sine\"\nfreq_hz = 100\ndecay_ms = 200\nvolume_pct = 50\n";

    /// The cached voice cut at mix time equals the synth's own cut.
    #[test]
    fn cached_cut_matches_add_until() {
        let lead = Instrument::parse("lead.inst", LEAD).unwrap();
        let kit = Instrument::parse("kit.inst", KIT).unwrap();
        let InstrumentKind::Melodic { layers, .. } = &lead.kind else {
            panic!()
        };
        let InstrumentKind::Kit { hits } = &kit.kind else {
            panic!()
        };
        for cut in [0, 1, 500, 4000, 7000, 100_000] {
            let mut want = Vec::new();
            add_note_until(&mut want, 300, layers, Note(60), 3000, 70, 300 + cut);
            let mut got = Vec::new();
            add_cut(&mut got, 300, &voice(&lead, 0, &[60], 3000, 70), cut);
            want.resize(got.len().max(want.len()), 0);
            got.resize(want.len(), 0);
            assert_eq!(got, want, "note cut {cut}");

            let mut want = Vec::new();
            add_sfx_until(&mut want, 10, &hits[&'K'], 80, 10 + cut);
            let mut got = Vec::new();
            add_cut(&mut got, 10, &voice(&kit, 0, b"K", 0, 80), cut);
            want.resize(got.len().max(want.len()), 0);
            got.resize(want.len(), 0);
            assert_eq!(got, want, "hit cut {cut}");
        }
    }
}
