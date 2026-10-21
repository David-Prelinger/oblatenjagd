//! The game's sound bank (E58b): every `audio/` source parsed and checked
//! once, and the renders the app asks for (D-114: SFX at startup, a song
//! when its cue first comes up).
//!
//! [`Bank::load`] takes `(path, text)` pairs (`jnr_assets::audio_sources()`
//! in the game, the files on disk in tools): `.sfx` under `audio/jingles/`
//! are jingles, every other `.sfx` an SFX; `.inst`, `.song` and `.quote` by
//! extension; anything else (`README.md`, `targets.toml`) is skipped. A
//! parse error, a duplicate id, a music cue of the wrong kind (below) or a
//! music lint *error* fails the load with
//! `file:line:col` messages; `jnr-assets/build.rs` turns that into a build
//! error.
//!
//! Music cues are songs (`music/*.song`) or `.sfx` jingles
//! (`jingles/*.sfx`); [`Bank::has_music`] accepts either. A cue that loops
//! ([`Music::is_song`]) must be a `.song` of `kind = "loop"`; a jingle cue a
//! `.song` of `kind = "jingle"` or a `jingles/*.sfx` (`error[kind]`).

use std::collections::BTreeMap;

use crate::cues::{JingleFrames, Music, Play};
use crate::music_lint::{Level, LintContext, lint_song};
use crate::patch::{FREQ_RANGE, Pitch, Sfx};
use crate::render::{Pcm, render_sfx};
use crate::song::{Song, SongKind};
use crate::song_render::{RenderOptions, render_in};
use crate::{Instrument, Quote, tables};

/// The tempo of the ZEIT-60 render of a stage piece (audio-design §4,
/// nice: +12 %).
pub const HURRY_TEMPO_PCT: u32 = 112;

/// Every parsed source, by id.
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub sfx: BTreeMap<String, Sfx>,
    pub jingles: BTreeMap<String, Sfx>,
    pub instruments: BTreeMap<String, Instrument>,
    pub songs: BTreeMap<String, Song>,
    pub quotes: BTreeMap<String, Quote>,
}

/// Which render of a music cue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// As written (an underwater twin at its `length_of` song's length).
    Normal,
    /// The ZEIT-60 render at [`HURRY_TEMPO_PCT`] (loops only).
    Hurry,
}

/// A rendered music cue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicRender {
    pub pcm: Pcm,
    /// Play it looped (a song of kind `loop`); its length is a multiple of
    /// 4096 frames (D-114).
    pub looped: bool,
    /// Samples clamped in the render (0 for a lint-clean source).
    pub clamped: u32,
}

impl Bank {
    /// Parses and checks every source (module docs). `Err` lists every
    /// problem, one `file:line:col: …` line each.
    pub fn load(sources: &[(&str, &str)]) -> Result<Bank, Vec<String>> {
        let mut bank = Bank::default();
        let mut errors = Vec::new();
        // The file of each jingle, for messages.
        let mut jingle_files: BTreeMap<String, String> = BTreeMap::new();
        let dup = |errors: &mut Vec<String>, file: &str, kind: &str, id: &str| {
            errors.push(format!("{file}:1:1: error[id]: a second {kind} `{id}`"));
        };
        for &(file, text) in sources {
            let ext = file.rsplit_once('.').map_or("", |(_, e)| e);
            match ext {
                "sfx" => match Sfx::parse(file, text) {
                    Ok(s) => {
                        let jingle = file.starts_with("audio/jingles/");
                        if jingle {
                            jingle_files.insert(s.id.clone(), file.to_owned());
                        }
                        let (map, kind) = if jingle {
                            (&mut bank.jingles, "jingle")
                        } else {
                            (&mut bank.sfx, "sfx")
                        };
                        if map.contains_key(&s.id) {
                            dup(&mut errors, file, kind, &s.id);
                        }
                        map.insert(s.id.clone(), s);
                    }
                    Err(e) => errors.push(e.to_string()),
                },
                "inst" => match Instrument::parse(file, text) {
                    Ok(i) => {
                        if bank.instruments.contains_key(&i.id) {
                            dup(&mut errors, file, "instrument", &i.id);
                        }
                        bank.instruments.insert(i.id.clone(), i);
                    }
                    Err(e) => errors.push(e.to_string()),
                },
                "song" => match Song::parse(file, text) {
                    Ok(s) => {
                        if bank.songs.contains_key(&s.id) {
                            dup(&mut errors, file, "song", &s.id);
                        }
                        bank.songs.insert(s.id.clone(), s);
                    }
                    Err(e) => errors.push(e.to_string()),
                },
                "quote" => match Quote::parse(file, text) {
                    Ok(q) => {
                        if bank.quotes.contains_key(&q.id) {
                            dup(&mut errors, file, "quote", &q.id);
                        }
                        bank.quotes.insert(q.id.clone(), q);
                    }
                    Err(e) => errors.push(e.to_string()),
                },
                _ => {}
            }
        }
        for (id, s) in &bank.songs {
            if bank.jingles.contains_key(id) {
                errors.push(format!(
                    "{}:1:1: error[id]: `{id}` is both a song and a jingle",
                    s.file
                ));
            }
        }
        errors.extend(kind_problems(&bank, &jingle_files));
        if errors.is_empty() {
            let ctx = LintContext {
                instruments: &bank.instruments,
                songs: &bank.songs,
                quotes: &bank.quotes,
            };
            for song in bank.songs.values() {
                errors.extend(
                    lint_song(song, &ctx)
                        .into_iter()
                        .filter(|l| l.level == Level::Error)
                        .map(|l| l.to_string()),
                );
            }
        }
        if errors.is_empty() {
            Ok(bank)
        } else {
            Err(errors)
        }
    }

    /// True if the music cue exists (a song or an `.sfx` jingle).
    pub fn has_music(&self, m: Music) -> bool {
        self.songs.contains_key(m.id()) || self.jingles.contains_key(m.id())
    }

    /// Renders an SFX at its pitch offset; `None` if the id isn't in the
    /// bank.
    pub fn render_sfx(&self, play: Play) -> Option<Pcm> {
        let sfx = self.sfx.get(play.sfx.id())?;
        Some(render_sfx(&transpose(sfx, play.semitones)).pcm)
    }

    /// Every SFX of [`crate::cues::Sfx::ALL`] at its own pitch, then every
    /// [`crate::cues::PITCHED`] variant: what the app renders at startup.
    /// Ids missing from the bank are left out.
    pub fn startup_sfx(&self) -> Vec<(Play, Pcm)> {
        crate::cues::Sfx::ALL
            .iter()
            .map(|s| s.play())
            .chain(crate::cues::PITCHED)
            .filter_map(|p| self.render_sfx(p).map(|pcm| (p, pcm)))
            .collect()
    }

    /// Renders a music cue (`Ok(None)`: not in the bank yet). A song goes
    /// through [`render_in`], so an underwater twin gets its `length_of`
    /// song's length; [`Variant::Hurry`] renders a loop at
    /// [`HURRY_TEMPO_PCT`] (a jingle ignores it).
    pub fn render_music(&self, m: Music, variant: Variant) -> Result<Option<MusicRender>, String> {
        if let Some(song) = self.songs.get(m.id()) {
            let looped = song.kind == SongKind::Loop;
            let opts = RenderOptions {
                tempo_pct: match variant {
                    Variant::Hurry if looped => HURRY_TEMPO_PCT,
                    _ => 100,
                },
                length_frames: None,
            };
            let r = render_in(&self.songs, &self.instruments, m.id(), &opts)
                .map_err(|e| e.to_string())?;
            return Ok(Some(MusicRender {
                pcm: r.render.pcm,
                looped,
                clamped: r.render.clamped,
            }));
        }
        Ok(self.jingles.get(m.id()).map(|s| {
            let r = render_sfx(s);
            MusicRender {
                pcm: r.pcm,
                looped: false,
                clamped: r.clamped,
            }
        }))
    }
}

impl Bank {
    /// The rendered length of every jingle music cue in the bank, in game
    /// frames rounded up (one render each, at load): what the cue
    /// [`Director`](crate::cues::Director) times the jingles by (E63), so a
    /// song that waits for a jingle starts when its last sample has played.
    pub fn jingle_frames(&self) -> Result<JingleFrames, String> {
        let mut out = JingleFrames::default();
        for m in Music::ALL.into_iter().filter(|m| !m.is_song()) {
            if let Some(r) = self.render_music(m, Variant::Normal)? {
                let samples = r.pcm.samples.len() as u32;
                out.frames
                    .insert(m, samples.div_ceil(crate::FRAMES_PER_TICK));
            }
        }
        Ok(out)
    }
}

/// Music cues whose source has the wrong kind (module docs).
fn kind_problems(bank: &Bank, jingle_files: &BTreeMap<String, String>) -> Vec<String> {
    let mut out = Vec::new();
    for m in Music::ALL {
        let id = m.id();
        let song = bank.songs.get(id);
        if m.is_song() {
            if let Some(file) = jingle_files.get(id) {
                out.push(format!(
                    "{file}:1:1: error[kind]: `{id}` is a looping music cue: it must be audio/music/{id}.song with kind = \"loop\", not a jingle"
                ));
            }
            if let Some(s) = song.filter(|s| s.kind != SongKind::Loop) {
                out.push(format!(
                    "{}:1:1: error[kind]: `{id}` is a looping music cue: kind must be \"loop\"",
                    s.file
                ));
            }
        } else if let Some(s) = song.filter(|s| s.kind != SongKind::Jingle) {
            out.push(format!(
                "{}:1:1: error[kind]: `{id}` is a one-shot music cue: kind must be \"jingle\" (or make it audio/jingles/{id}.sfx)",
                s.file
            ));
        }
    }
    out
}

/// `sfx` with every oscillator's pitch moved by `semitones` (filters,
/// envelopes and timing stay; arpeggios and vibrato are relative anyway),
/// clamped to [`FREQ_RANGE`]. A noise layer's `freq_hz` (its LFSR rate)
/// moves too; band-pass centres don't.
pub fn transpose(sfx: &Sfx, semitones: i8) -> Sfx {
    if semitones == 0 {
        return sfx.clone();
    }
    let cents = i32::from(semitones) * 100;
    // Hz in 16.16 fixed point, scaled, rounded back to whole Hz.
    let shift = |hz: u32| -> u32 {
        let scaled = tables::scale_by_cents(u64::from(hz) << 16, cents);
        let hz = (scaled + (1 << 15)) >> 16;
        hz.clamp(FREQ_RANGE.0 as u64, FREQ_RANGE.1 as u64) as u32
    };
    let mut out = sfx.clone();
    for layer in &mut out.layers {
        if let Pitch::Fixed {
            freq_hz,
            slide_to_hz,
        } = &mut layer.pitch
        {
            *freq_hz = shift(*freq_hz);
            if let Some(to) = slide_to_hz {
                *to = shift(*to);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cues::{JUMP_BIG_SEMITONES, PITCHED};

    const JUMP: &str = include_str!("../tests/fixtures/jump.sfx");

    fn fixtures() -> Vec<(&'static str, &'static str)> {
        vec![
            ("audio/sfx/jump.sfx", JUMP),
            ("audio/README.md", "# not a source"),
            ("audio/targets.toml", "[jump]\nclass = \"normal\"\n"),
            (
                "audio/instruments/lead.inst",
                include_str!("../tests/fixtures/music/lead.inst"),
            ),
            (
                "audio/instruments/bass.inst",
                include_str!("../tests/fixtures/music/bass.inst"),
            ),
            (
                "audio/instruments/chords.inst",
                include_str!("../tests/fixtures/music/chords.inst"),
            ),
            (
                "audio/instruments/drums.inst",
                include_str!("../tests/fixtures/music/drums.inst"),
            ),
            (
                "audio/quotes/oblaten_motiv.quote",
                include_str!("../tests/fixtures/music/oblaten_motiv.quote"),
            ),
            (
                "audio/quotes/haydn_94_andante.quote",
                include_str!("../tests/fixtures/music/haydn_94_andante.quote"),
            ),
            (
                "audio/music/placeholder_polka.song",
                include_str!("../tests/fixtures/music/placeholder_polka.song"),
            ),
            (
                "audio/music/placeholder_fanfare.song",
                include_str!("../tests/fixtures/music/placeholder_fanfare.song"),
            ),
        ]
    }

    #[test]
    fn loads_by_kind_and_skips_other_files() {
        let bank = Bank::load(&fixtures()).expect("the fixtures load");
        assert_eq!(bank.sfx.keys().collect::<Vec<_>>(), ["jump"]);
        assert_eq!(bank.instruments.len(), 4);
        assert_eq!(bank.quotes.len(), 2);
        assert_eq!(bank.songs.len(), 2);
        assert!(bank.jingles.is_empty());
    }

    #[test]
    fn errors_name_file_line_and_col() {
        let mut src = fixtures();
        src.push((
            "audio/sfx/bad.sfx",
            "[[layer]]\nwave = \"pulse\"\nvolume_pct = 1.5\n",
        ));
        src.push(("audio/jingles/jump.sfx", JUMP));
        src.push(("audio/sfx/jump.sfx", JUMP));
        let errors = Bank::load(&src).unwrap_err();
        assert!(errors[0].starts_with("audio/sfx/bad.sfx:3:"), "{errors:?}");
        assert!(
            errors
                .iter()
                .any(|e| e.starts_with("audio/sfx/jump.sfx:1:1: error[id]: a second sfx `jump`")),
            "{errors:?}"
        );
        // A jingle may share an id with an sfx (other namespace).
        assert_eq!(errors.len(), 2, "{errors:?}");
    }

    /// `fixtures()` with the fixture song `from` renamed to music cue `to`.
    fn as_cue(from: &str, to: &str) -> Vec<(String, String)> {
        fixtures()
            .into_iter()
            .map(|(f, t)| {
                if f == format!("audio/music/{from}.song") {
                    (
                        format!("audio/music/{to}.song"),
                        t.replace(&format!("id = \"{from}\""), &format!("id = \"{to}\"")),
                    )
                } else {
                    (f.to_owned(), t.to_owned())
                }
            })
            .collect()
    }

    fn load(src: &[(String, String)]) -> Result<Bank, Vec<String>> {
        let refs: Vec<(&str, &str)> = src.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        Bank::load(&refs)
    }

    #[test]
    fn a_music_cue_needs_a_source_of_its_kind() {
        // The right kinds load.
        assert!(load(&as_cue("placeholder_polka", "title")).is_ok());
        assert!(load(&as_cue("placeholder_fanfare", "death")).is_ok());
        let mut jingle = as_cue("placeholder_polka", "salzburg");
        jingle.push(("audio/jingles/death.sfx".into(), JUMP.into()));
        assert!(load(&jingle).is_ok());
        // A looping cue as a jingle song.
        let e = load(&as_cue("placeholder_fanfare", "title")).unwrap_err();
        assert_eq!(
            e,
            [
                "audio/music/title.song:1:1: error[kind]: `title` is a looping music cue: kind must be \"loop\""
            ]
        );
        // A one-shot cue as a loop.
        let e = load(&as_cue("placeholder_polka", "death")).unwrap_err();
        assert!(
            e[0].starts_with(
                "audio/music/death.song:1:1: error[kind]: `death` is a one-shot music cue"
            ),
            "{e:?}"
        );
        // A looping cue as an .sfx jingle.
        let mut src = as_cue("placeholder_polka", "placeholder_polka");
        src.push(("audio/jingles/title.sfx".into(), JUMP.into()));
        let e = load(&src).unwrap_err();
        assert!(
            e[0].starts_with(
                "audio/jingles/title.sfx:1:1: error[kind]: `title` is a looping music cue"
            ),
            "{e:?}"
        );
        // Only `audio/jingles/` makes a jingle.
        let mut src = as_cue("placeholder_polka", "placeholder_polka");
        src.push(("audio/sfx/jingles/title.sfx".into(), JUMP.into()));
        assert!(load(&src).unwrap().jingles.is_empty());
    }

    #[test]
    fn a_song_and_a_jingle_may_not_share_an_id() {
        let mut src = as_cue("placeholder_fanfare", "death");
        src.push(("audio/jingles/death.sfx".into(), JUMP.into()));
        let e = load(&src).unwrap_err();
        assert_eq!(
            e,
            ["audio/music/death.song:1:1: error[id]: `death` is both a song and a jingle"]
        );
    }

    #[test]
    fn a_music_lint_error_fails_the_load() {
        let mut src = fixtures();
        // The polka without its bass instrument: the `instrument` lint.
        src.retain(|(f, _)| !f.ends_with("bass.inst"));
        let errors = Bank::load(&src).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.contains("placeholder_polka.song:") && e.contains("bass")),
            "{errors:?}"
        );
    }

    #[test]
    fn pitched_variants_render_lower_and_as_long() {
        let bank = Bank::load(&fixtures()).unwrap();
        let jump = Play {
            sfx: crate::cues::Sfx::Jump,
            semitones: 0,
        };
        let plain = bank.render_sfx(jump).unwrap();
        let big = bank.render_sfx(PITCHED[0]).unwrap();
        assert_eq!(PITCHED[0].semitones, JUMP_BIG_SEMITONES);
        assert_eq!(plain.samples.len(), big.samples.len());
        assert_ne!(plain, big);
        let crossings = |p: &Pcm| {
            p.samples
                .windows(2)
                .filter(|w| (w[0] < 0) != (w[1] < 0))
                .count()
        };
        // A fourth lower: about 2^(-5/12) = 0.75 of the zero crossings.
        let ratio = crossings(&big) * 100 / crossings(&plain);
        assert!((70..=80).contains(&ratio), "{ratio}");
        // Every SFX the bank has, plus its pitched variants, at startup.
        let all = bank.startup_sfx();
        assert_eq!(
            all.len(),
            1 + 1,
            "jump and the big jump (pause isn't in the fixtures)"
        );
    }

    #[test]
    fn transpose_moves_fixed_pitches_by_semitones() {
        let sfx = Sfx::parse("audio/sfx/jump.sfx", JUMP).unwrap();
        let up = transpose(&sfx, 12);
        let mut slides = 0;
        for (a, b) in sfx.layers.iter().zip(&up.layers) {
            let (
                Pitch::Fixed {
                    freq_hz: f0,
                    slide_to_hz: s0,
                },
                Pitch::Fixed {
                    freq_hz: f1,
                    slide_to_hz: s1,
                },
            ) = (a.pitch, b.pitch)
            else {
                panic!("an .sfx has fixed pitches");
            };
            assert!(f1.abs_diff(f0 * 2) <= 1, "{f0} -> {f1}");
            if let (Some(s0), Some(s1)) = (s0, s1) {
                assert!(s1.abs_diff(s0 * 2) <= 1, "slide {s0} -> {s1}");
                slides += 1;
            }
            assert_eq!(s0.is_some(), s1.is_some());
        }
        assert!(slides > 0, "the fixture jump glides");
        assert_eq!(transpose(&sfx, 0), sfx);
        // Clamped to the patch format's range.
        let way_down = transpose(&sfx, -100);
        assert!(way_down.layers.iter().all(|l| matches!(
            l.pitch,
            Pitch::Fixed { freq_hz, .. } if freq_hz as i64 == FREQ_RANGE.0
        )));
    }

    #[test]
    fn music_renders_by_variant_and_missing_is_none() {
        let mut bank = Bank::load(&fixtures()).unwrap();
        // Pretend the fixture polka is the title march.
        let polka = bank.songs.remove("placeholder_polka").unwrap();
        bank.songs.insert("title".into(), polka);
        let normal = bank
            .render_music(Music::Title, Variant::Normal)
            .unwrap()
            .unwrap();
        let hurry = bank
            .render_music(Music::Title, Variant::Hurry)
            .unwrap()
            .unwrap();
        assert!(normal.looped && hurry.looped);
        assert_eq!(normal.pcm.samples.len() % 4096, 0);
        assert!(hurry.pcm.samples.len() < normal.pcm.samples.len(), "faster");
        assert!(bank.has_music(Music::Title));
        assert!(!bank.has_music(Music::Death));
        assert_eq!(bank.render_music(Music::Death, Variant::Normal), Ok(None));
    }
}
