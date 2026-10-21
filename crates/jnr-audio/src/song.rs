//! The `.song` format: a TOML header plus tracker-style scores, one bar
//! structure shared by every channel (see `docs/architecture.md`, "`.song`").
//!
//! ```toml
//! # audio/music/example.song
//! [meta]
//! id = "example"            # = the file stem
//! kind = "loop"             # loop (from sample 0, D-114) | jingle (once)
//! tempo_bpm = 120           # beats per minute; a beat is the meter's lower
//!                           # number (6/8: an eighth, so a dotted quarter of 66 = 198)
//! meter = "2/4"             # 2/4 | 3/4 | 6/8
//! steps_per_beat = 2        # the grid; a bar has beats x steps_per_beat steps
//! key = "bb major"          # notes outside it need a trailing `!`
//! bars = 4                  # the total of `order` (checked)
//! order = ["A", "A"]        # the patterns, in playing order
//! source = "original"       # or the quoted work; then `edition` is required
//!
//! [channels]                # 1..=6, in this order in the piano roll
//! lead = { instrument = "fluegelhorn", volume_pct = 70 }
//! bass = { instrument = "tuba", volume_pct = 80 }
//! chords = { instrument = "begleitung", volume_pct = 40, arp_ms = 17 }
//! drums = { instrument = "kit", volume_pct = 60 }
//!
//! [pattern.A]
//! score = '''
//! lead   | bb4 d5 f5 eb5>/2   | d5/4                 |
//! bass   | bb2/2 f2           | bb2/2 r              |
//! chords | r/1 [d4,f4,bb4]' r [d4,f4,bb4]' | r/1 [d4,f4,bb4]' r/2 |
//! drums  | K/2 h              | K/2 [K,c]            |
//! '''
//! ```
//!
//! Token: `<sound>[/<steps>][>]['][@<velocity>]`. Sounds: a note (`c5`,
//! `f#4`, `bb3`, with `!` if it is outside the key on purpose), a chord
//! (`[d4,f4,bb4]`, up to three notes, arpeggiated by the channel's `arp_ms`),
//! a kit letter (`K`) or several hit together (`[K,c]`), a rest `r`, a tie `_`
//! (holds the previous note longer, also across bar lines and patterns).
//! `/<steps>` is the length (default: the previous token's on this line; the
//! first token of a line: one beat). `>` accents (velocity 125 %), `'` plays
//! staccato (sounds half the length), `@<pct>` sets the velocity (1..=200).
//! A `#` at a line start or after a space starts a comment (`f#4` is a
//! note). Several lines of one channel in a score continue each other (one "system"
//! of a few bars per block keeps the bars of all channels aligned). A channel
//! without a line in a pattern is silent there.

use std::collections::BTreeMap;
use std::fmt;

use serde::Deserialize;
use toml::Spanned;

use crate::error::{ErrorKind, ParseError, Src};
use crate::note::Note;
use crate::patch::{file_id, from_toml, valid_id};

/// Most channels per song.
pub const MAX_CHANNELS: usize = 6;
/// Most notes in a chord (or hits played together).
pub const MAX_CHORD: usize = 3;
/// Velocity of an accent (`>`), percent.
pub const ACCENT_PCT: u32 = 125;
/// Default arpeggio step of a chord (ms): about one 60 Hz frame, the
/// chip-tune chord of `docs/audio-design.md` §2.1.
pub const DEFAULT_ARP_MS: u32 = 17;

/// Whether a song loops from its first sample or plays once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SongKind {
    /// Loops from sample 0; rendered to a multiple of 4096 frames (D-114).
    Loop,
    /// Plays once (a stage intro, a fanfare); its last notes ring out.
    Jingle,
}

/// A time signature: 2/4, 3/4 or 6/8.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meter {
    pub beats: u32,
    pub unit: u32,
}

impl Meter {
    /// Beats (0-based) that count as strong: 1 (and 4 in 6/8).
    pub fn is_strong(self, beat: u32) -> bool {
        beat == 0 || (self.unit == 8 && beat == 3)
    }
}

impl fmt::Display for Meter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.beats, self.unit)
    }
}

/// A key: tonic pitch class (0 = c) and mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Key {
    pub tonic: u8,
    pub minor: bool,
    /// The tonic as written (`bb`, `f#`), for messages.
    pub name: &'static str,
}

const TONICS: [(&str, u8); 21] = [
    ("c", 0),
    ("c#", 1),
    ("db", 1),
    ("d", 2),
    ("d#", 3),
    ("eb", 3),
    ("e", 4),
    ("f", 5),
    ("f#", 6),
    ("gb", 6),
    ("g", 7),
    ("g#", 8),
    ("ab", 8),
    ("a", 9),
    ("a#", 10),
    ("bb", 10),
    ("b", 11),
    ("cb", 11),
    ("e#", 5),
    ("fb", 4),
    ("b#", 0),
];

impl Key {
    /// Parses `<tonic> major|minor`, e.g. `bb major`, `d minor`.
    pub fn parse(s: &str) -> Option<Key> {
        let (tonic, mode) = s.split_once(' ')?;
        let minor = match mode {
            "major" => false,
            "minor" => true,
            _ => return None,
        };
        let &(name, pc) = TONICS.iter().find(|(n, _)| *n == tonic)?;
        Some(Key {
            tonic: pc,
            minor,
            name,
        })
    }

    /// True if the note's pitch class belongs to the key. Major: the major
    /// scale. Minor: the natural minor scale plus the raised 6th and 7th
    /// (harmonic and melodic minor), so leading tones need no `!`.
    pub fn contains(self, note: Note) -> bool {
        let degree = (note.0 as i32 - self.tonic as i32).rem_euclid(12);
        let scale: &[i32] = if self.minor {
            &[0, 2, 3, 5, 7, 8, 9, 10, 11]
        } else {
            &[0, 2, 4, 5, 7, 9, 11]
        };
        scale.contains(&degree)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mode = if self.minor { "minor" } else { "major" };
        write!(f, "{} {mode}", self.name)
    }
}

/// A 1-based source position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

/// One channel (voice) of a song.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Channel {
    pub name: String,
    /// The `.inst` id (file stem in `audio/instruments/`).
    pub instrument: String,
    pub volume_pct: u32,
    /// Arpeggio step of chord tokens (ms); 0 sounds the notes together.
    pub arp_ms: u32,
    pub at: Pos,
}

/// A note of a token, and whether it is marked as outside the key (`!`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub note: Note,
    pub accidental: bool,
}

/// What a token plays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sound {
    /// One note, or a chord of up to [`MAX_CHORD`] (in written order).
    Notes(Vec<Tone>),
    /// Kit letters struck together.
    Hits(Vec<char>),
    Rest,
    /// Holds the previous note (or hit) longer.
    Tie,
}

/// One token of a score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub sound: Sound,
    /// Length in steps (≥ 1).
    pub len: u32,
    pub velocity_pct: u32,
    pub staccato: bool,
    pub at: Pos,
}

/// One bar of one channel line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bar {
    pub tokens: Vec<Token>,
    pub at: Pos,
}

/// A section, played by name from `order`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pattern {
    pub name: String,
    /// The section's key (the song's unless `key` is set in the pattern).
    pub key: Key,
    /// Bars of the section (every present channel has exactly this many).
    pub bars: u32,
    /// Per channel (same index as [`Song::channels`]); empty = silent.
    pub lines: Vec<Vec<Bar>>,
    pub at: Pos,
}

/// How a quote is compared with its fixture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteMatch {
    /// Pitches and lengths (rests included).
    Exact,
    /// Pitches only (a tune squeezed into another meter).
    Pitches,
}

/// A `[[quote]]`: a place where a channel must play a `.quote` fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuoteRef {
    /// The fixture id (`audio/quotes/<id>.quote`).
    pub fixture: String,
    pub channel: usize,
    /// 1-based song bar where the quote starts, and the step in that bar.
    pub bar: u32,
    pub step: u32,
    /// Semitones from the fixture to the song.
    pub transpose: i32,
    /// The song's lengths are the fixture's times this (augmentation).
    pub augment: u32,
    pub compare: QuoteMatch,
    pub at: Pos,
}

/// A note, chord, hit or rest placed in time (ties already merged).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    /// Start in steps from the song start.
    pub step: u32,
    /// Length in steps (ties included).
    pub len: u32,
    /// Never [`Sound::Tie`].
    pub sound: Sound,
    pub velocity_pct: u32,
    pub staccato: bool,
    /// 0-based song bar of the first token.
    pub bar: u32,
    /// Index into [`Song::patterns`].
    pub pattern: usize,
    pub at: Pos,
}

impl Event {
    /// The highest note (a melody's note in a chord), if any.
    pub fn top(&self) -> Option<Note> {
        match &self.sound {
            Sound::Notes(t) => t.iter().map(|t| t.note).max(),
            _ => None,
        }
    }
}

/// A parsed `.song`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Song {
    pub file: String,
    pub id: String,
    pub kind: SongKind,
    pub tempo_bpm: u32,
    pub meter: Meter,
    pub steps_per_beat: u32,
    pub key: Key,
    pub bars: u32,
    /// Indices into `patterns`.
    pub order: Vec<usize>,
    pub source: String,
    pub edition: Option<String>,
    /// Render to exactly this other song's length (D-114: the underwater
    /// track has Stage 6's length).
    pub length_of: Option<String>,
    pub channels: Vec<Channel>,
    /// Sorted by name.
    pub patterns: Vec<Pattern>,
    pub quotes: Vec<QuoteRef>,
    /// Per channel, in time order.
    pub events: Vec<Vec<Event>>,
    /// Positions of header keys, for lints.
    pub at: HeaderPos,
}

/// Where the header keys are (1-based; the file start if missing).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderPos {
    pub meta: Pos,
    pub bars: Pos,
    pub order: Pos,
    pub tempo: Pos,
    pub source: Pos,
    pub length_of: Pos,
}

impl Song {
    /// Steps in one bar.
    pub fn bar_steps(&self) -> u32 {
        self.meter.beats * self.steps_per_beat
    }

    /// Steps of the whole song (the bars of `order`).
    pub fn total_steps(&self) -> u32 {
        self.order
            .iter()
            .map(|&p| self.patterns[p].bars)
            .sum::<u32>()
            * self.bar_steps()
    }

    /// The `file:line:col` prefix of a position.
    pub fn loc(&self, at: Pos) -> String {
        format!("{}:{}:{}", self.file, at.line, at.col)
    }

    /// Parses a `.song`; the id is the file stem of `file`.
    pub fn parse(file: &str, text: &str) -> Result<Song, ParseError> {
        let src = Src { file, text };
        let id = file_id(&src, ".song")?;
        let raw: RawSong = from_toml(&src)?;
        let pos = |span: std::ops::Range<usize>| {
            let (line, col) = src.line_col(span.start);
            Pos { line, col }
        };
        let m = raw.meta.get_ref();
        let music = |span, msg: String| src.err(span, ErrorKind::Music, msg);

        if m.id.get_ref() != &id {
            return Err(src.err(
                m.id.span(),
                ErrorKind::Id,
                format!("`id` = \"{}\" must be the file stem `{id}`", m.id.get_ref()),
            ));
        }
        let kind = match m.kind.get_ref().as_str() {
            "loop" => SongKind::Loop,
            "jingle" => SongKind::Jingle,
            other => {
                return Err(music(
                    m.kind.span(),
                    format!("unknown kind `{other}` (loop | jingle)"),
                ));
            }
        };
        let tempo_bpm = range(&src, &m.tempo_bpm, "tempo_bpm", 20, 400)?;
        let meter = match m.meter.get_ref().as_str() {
            "2/4" => Meter { beats: 2, unit: 4 },
            "3/4" => Meter { beats: 3, unit: 4 },
            "6/8" => Meter { beats: 6, unit: 8 },
            other => {
                return Err(music(
                    m.meter.span(),
                    format!("meter `{other}`: only the dance meters 2/4, 3/4 and 6/8"),
                ));
            }
        };
        let steps_per_beat = range(&src, &m.steps_per_beat, "steps_per_beat", 1, 12)?;
        let key = key(&src, &m.key)?;
        let bars = range(&src, &m.bars, "bars", 1, 512)?;
        let source = m.source.get_ref().trim().to_owned();
        if source.is_empty() {
            return Err(music(
                m.source.span(),
                "`source` is empty: \"original\" or the quoted work".into(),
            ));
        }
        let edition = m.edition.as_ref().map(|e| e.get_ref().clone());
        if source != "original" && edition.is_none() {
            return Err(music(
                m.source.span(),
                "a quoted `source` needs `edition` (the public-domain edition it was transcribed from)"
                    .into(),
            ));
        }
        let length_of = match &m.length_of {
            None => None,
            Some(l) => {
                if kind != SongKind::Loop || !valid_id(l.get_ref()) {
                    return Err(music(
                        l.span(),
                        "`length_of` takes another song's id and only works on a loop".into(),
                    ));
                }
                Some(l.get_ref().clone())
            }
        };

        // Channels, in file order.
        let mut chans: Vec<_> = raw.channels.get_ref().iter().collect();
        chans.sort_by_key(|(_, c)| c.span().start);
        if chans.is_empty() || chans.len() > MAX_CHANNELS {
            return Err(src.err(
                raw.channels.span(),
                ErrorKind::Channel,
                format!("{} channels; a song has 1..={MAX_CHANNELS}", chans.len()),
            ));
        }
        let mut channels = Vec::new();
        for (name, c) in chans {
            let r = c.get_ref();
            if !valid_id(name) {
                return Err(src.err(
                    c.span(),
                    ErrorKind::Channel,
                    format!("bad channel name `{name}`: lowercase letters, digits, `_`"),
                ));
            }
            channels.push(Channel {
                name: name.clone(),
                instrument: r.instrument.get_ref().clone(),
                volume_pct: range(&src, &r.volume_pct, "volume_pct", 0, 100)?,
                arp_ms: match &r.arp_ms {
                    Some(v) => range(&src, v, "arp_ms", 0, 1000)?,
                    None => DEFAULT_ARP_MS,
                },
                at: pos(c.span()),
            });
        }

        // Patterns, by name.
        let mut patterns = Vec::new();
        for (name, p) in &raw.pattern {
            let r = p.get_ref();
            if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                return Err(src.err(
                    p.span(),
                    ErrorKind::Pattern,
                    format!("bad pattern name `{name}`: letters, digits, `_`"),
                ));
            }
            let pkey = match &r.key {
                Some(k) => self::key(&src, k)?,
                None => key,
            };
            let lines = score(&src, &r.score, &channels, steps_per_beat)?;
            let bars = lines.iter().map(|l| l.len()).max().unwrap_or(0);
            if bars == 0 {
                return Err(src.err(r.score.span(), ErrorKind::Pattern, "the score has no bars"));
            }
            for (ci, line) in lines.iter().enumerate() {
                if !line.is_empty() && line.len() != bars {
                    return Err(src.err(
                        r.score.span(),
                        ErrorKind::Pattern,
                        format!(
                            "channel `{}` has {} bars in pattern {name}, the longest line has {bars}",
                            channels[ci].name,
                            line.len()
                        ),
                    )
                    .at(line[0].at));
                }
            }
            patterns.push(Pattern {
                name: name.clone(),
                key: pkey,
                bars: bars as u32,
                lines,
                at: pos(p.span()),
            });
        }
        if patterns.is_empty() {
            return Err(src.err(0..0, ErrorKind::Pattern, "no `[pattern.<name>]`"));
        }

        let mut order = Vec::new();
        for o in m.order.get_ref() {
            match patterns.iter().position(|p| &p.name == o.get_ref()) {
                Some(i) => order.push(i),
                None => {
                    return Err(src.err(
                        o.span(),
                        ErrorKind::Pattern,
                        format!(
                            "`order` names `{}`, which has no `[pattern.{}]`",
                            o.get_ref(),
                            o.get_ref()
                        ),
                    ));
                }
            }
        }
        if order.is_empty() {
            return Err(src.err(m.order.span(), ErrorKind::Pattern, "`order` is empty"));
        }

        let mut quotes = Vec::new();
        for q in &raw.quote {
            let r = q.get_ref();
            let channel = channels
                .iter()
                .position(|c| &c.name == r.channel.get_ref())
                .ok_or_else(|| {
                    src.err(
                        r.channel.span(),
                        ErrorKind::Music,
                        format!("no channel `{}`", r.channel.get_ref()),
                    )
                })?;
            let compare = match r.compare.as_ref().map(|c| c.get_ref().as_str()) {
                None | Some("exact") => QuoteMatch::Exact,
                Some("pitches") => QuoteMatch::Pitches,
                Some(other) => {
                    return Err(src.err(
                        r.compare.as_ref().map_or(q.span(), |c| c.span()),
                        ErrorKind::Music,
                        format!("compare = `{other}` (exact | pitches)"),
                    ));
                }
            };
            quotes.push(QuoteRef {
                fixture: r.fixture.get_ref().clone(),
                channel,
                bar: range(&src, &r.bar, "bar", 1, 512)?,
                step: match &r.step {
                    Some(v) => range(&src, v, "step", 0, 1000)?,
                    None => 0,
                },
                transpose: match &r.transpose {
                    Some(v) => bounded_i(&src, v, "transpose", -48, 48)?,
                    None => 0,
                },
                augment: match &r.augment {
                    Some(v) => range(&src, v, "augment", 1, 8)?,
                    None => 1,
                },
                compare,
                at: pos(q.span()),
            });
        }

        let mut song = Song {
            file: file.to_owned(),
            id,
            kind,
            tempo_bpm,
            meter,
            steps_per_beat,
            key,
            bars,
            order,
            source,
            edition,
            length_of,
            channels,
            patterns,
            quotes,
            events: Vec::new(),
            at: HeaderPos {
                meta: pos(raw.meta.span()),
                bars: pos(m.bars.span()),
                order: pos(m.order.span()),
                tempo: pos(m.tempo_bpm.span()),
                source: pos(m.source.span()),
                length_of: m
                    .length_of
                    .as_ref()
                    .map_or(Pos::default(), |l| pos(l.span())),
            },
        };
        song.events = timeline(&song).map_err(|(at, msg)| ParseError {
            file: file.to_owned(),
            line: at.line,
            col: at.col,
            kind: ErrorKind::Score,
            message: msg,
        })?;
        Ok(song)
    }
}

impl ParseError {
    /// The same error at another position.
    pub(crate) fn at(mut self, at: Pos) -> ParseError {
        self.line = at.line;
        self.col = at.col;
        self
    }
}

// ------------------------------------------------------------- raw TOML shape

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSong {
    meta: Spanned<RawMeta>,
    channels: Spanned<BTreeMap<String, Spanned<RawChannel>>>,
    #[serde(default)]
    pattern: BTreeMap<String, Spanned<RawPattern>>,
    #[serde(default)]
    quote: Vec<Spanned<RawQuoteRef>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMeta {
    id: Spanned<String>,
    kind: Spanned<String>,
    tempo_bpm: Spanned<i64>,
    meter: Spanned<String>,
    steps_per_beat: Spanned<i64>,
    key: Spanned<String>,
    bars: Spanned<i64>,
    order: Spanned<Vec<Spanned<String>>>,
    source: Spanned<String>,
    edition: Option<Spanned<String>>,
    length_of: Option<Spanned<String>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChannel {
    instrument: Spanned<String>,
    volume_pct: Spanned<i64>,
    arp_ms: Option<Spanned<i64>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPattern {
    key: Option<Spanned<String>>,
    score: Spanned<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuoteRef {
    fixture: Spanned<String>,
    channel: Spanned<String>,
    bar: Spanned<i64>,
    step: Option<Spanned<i64>>,
    transpose: Option<Spanned<i64>>,
    augment: Option<Spanned<i64>>,
    compare: Option<Spanned<String>>,
}

// ------------------------------------------------------------------ checking

fn bounded_i(src: &Src, v: &Spanned<i64>, key: &str, lo: i64, hi: i64) -> Result<i32, ParseError> {
    let n = *v.get_ref();
    if (lo..=hi).contains(&n) {
        Ok(n as i32)
    } else {
        Err(src.err(
            v.span(),
            ErrorKind::Range,
            format!("`{key}` = {n} is outside {lo}..={hi}"),
        ))
    }
}

fn range(src: &Src, v: &Spanned<i64>, key: &str, lo: i64, hi: i64) -> Result<u32, ParseError> {
    bounded_i(src, v, key, lo, hi).map(|n| n as u32)
}

fn key(src: &Src, k: &Spanned<String>) -> Result<Key, ParseError> {
    Key::parse(k.get_ref()).ok_or_else(|| {
        src.err(
            k.span(),
            ErrorKind::Music,
            format!(
                "bad key `{}` (e.g. \"bb major\", \"d minor\", \"f# major\")",
                k.get_ref()
            ),
        )
    })
}

/// Where the text of a TOML string value starts in the file, so token
/// offsets map to exact `line:col` (literal and escape-free strings; others
/// fall back to the string's start).
pub(crate) fn value_start(src: &Src, s: &Spanned<String>) -> Option<usize> {
    let span = s.span();
    let raw = &src.text[span.clone()];
    let mut skip = if raw.starts_with("'''") || raw.starts_with("\"\"\"") {
        3
    } else {
        1
    };
    let rest = &raw[skip..];
    if skip == 3 {
        if rest.starts_with("\r\n") {
            skip += 2;
        } else if rest.starts_with('\n') {
            skip += 1;
        }
    }
    let start = span.start + skip;
    src.text[start..]
        .starts_with(s.get_ref().as_str())
        .then_some(start)
}

/// A position inside a string value: exact if possible.
pub(crate) struct Locator<'a> {
    pub src: &'a Src<'a>,
    pub start: Option<usize>,
    pub fallback: usize,
}

impl Locator<'_> {
    pub fn pos(&self, offset: usize) -> Pos {
        let at = self.start.map_or(self.fallback, |s| s + offset);
        let (line, col) = self.src.line_col(at);
        Pos { line, col }
    }

    pub fn err(&self, offset: usize, kind: ErrorKind, msg: impl Into<String>) -> ParseError {
        let p = self.pos(offset);
        ParseError {
            file: self.src.file.to_owned(),
            line: p.line,
            col: p.col,
            kind,
            message: msg.into(),
        }
    }
}

/// Splits a score into per-channel bars.
fn score(
    src: &Src,
    s: &Spanned<String>,
    channels: &[Channel],
    steps_per_beat: u32,
) -> Result<Vec<Vec<Bar>>, ParseError> {
    let loc = Locator {
        src,
        start: value_start(src, s),
        fallback: s.span().start,
    };
    let text = s.get_ref();
    let mut lines: Vec<Vec<Bar>> = vec![Vec::new(); channels.len()];
    // The previous token's length, per channel (lines continue each other).
    let mut prev_len: Vec<u32> = vec![steps_per_beat; channels.len()];
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let base = offset;
        offset += line.len();
        let code = strip_comment(line).trim_end();
        let lead = code.len() - code.trim_start().len();
        let code = code.trim_start();
        if code.is_empty() {
            continue;
        }
        let name_end = code.find([' ', '\t', '|']).unwrap_or(code.len());
        let name = &code[..name_end];
        let Some(ci) = channels.iter().position(|c| c.name == name) else {
            let names: Vec<&str> = channels.iter().map(|c| c.name.as_str()).collect();
            return Err(loc.err(
                base + lead,
                ErrorKind::Channel,
                format!("`{name}` is not a channel ({})", names.join(", ")),
            ));
        };
        let body_off = base + lead + name_end;
        let body = &code[name_end..];
        let bstart = body.len() - body.trim_start().len();
        if !body[bstart..].starts_with('|') {
            return Err(loc.err(
                body_off + bstart,
                ErrorKind::Score,
                format!("after `{name}` the bars start with `|`"),
            ));
        }
        let mut bar_off = body_off + bstart + 1;
        let pieces: Vec<&str> = body[bstart + 1..].split('|').collect();
        for (i, piece) in pieces.iter().enumerate() {
            let this_off = bar_off;
            bar_off += piece.len() + 1;
            if piece.trim().is_empty() {
                if i + 1 == pieces.len() {
                    continue; // the closing `|`
                }
                return Err(loc.err(
                    this_off,
                    ErrorKind::Score,
                    "an empty bar: write a rest, e.g. `r/8`",
                ));
            }
            let mut tokens = Vec::new();
            let mut pos = 0;
            for word in piece.split_whitespace() {
                let at_word = pos + piece[pos..].find(word).unwrap_or(0);
                pos = at_word + word.len();
                let t = token(word, &mut prev_len[ci])
                    .map_err(|m| loc.err(this_off + at_word, ErrorKind::Score, m))?;
                tokens.push(Token {
                    at: loc.pos(this_off + at_word),
                    ..t
                });
            }
            let at = tokens[0].at;
            lines[ci].push(Bar { tokens, at });
        }
    }
    Ok(lines)
}

/// The line up to a comment: a `#` at the line start or after whitespace
/// (a `#` inside a word is a sharp, `f#4`).
pub(crate) fn strip_comment(line: &str) -> &str {
    let mut prev_space = true;
    for (i, c) in line.char_indices() {
        if c == '#' && prev_space {
            return &line[..i];
        }
        prev_space = c.is_whitespace();
    }
    line
}

/// Parses one token; `prev_len` is the default length and is updated.
pub(crate) fn token(word: &str, prev_len: &mut u32) -> Result<Token, String> {
    let base_end = if word.starts_with('[') {
        word.find(']')
            .map(|i| i + 1)
            .ok_or("a chord needs its closing `]`")?
    } else {
        word.find(['/', '>', '\'', '@']).unwrap_or(word.len())
    };
    let base = &word[..base_end];
    let sound = match base {
        "r" => Sound::Rest,
        "_" => Sound::Tie,
        b if b.starts_with('[') => {
            let inner = &b[1..b.len() - 1];
            let items: Vec<&str> = inner.split(',').map(str::trim).collect();
            if items.len() > MAX_CHORD || items.iter().any(|i| i.is_empty()) {
                return Err(format!(
                    "`{b}`: a chord has 1..={MAX_CHORD} notes or hits, comma-separated"
                ));
            }
            let parsed = items
                .iter()
                .map(|i| item(i))
                .collect::<Result<Vec<_>, _>>()?;
            if parsed.iter().all(|p| matches!(p, Item::Tone(_))) {
                Sound::Notes(
                    parsed
                        .into_iter()
                        .filter_map(|p| match p {
                            Item::Tone(t) => Some(t),
                            Item::Hit(_) => None,
                        })
                        .collect(),
                )
            } else if parsed.iter().all(|p| matches!(p, Item::Hit(_))) {
                Sound::Hits(
                    parsed
                        .into_iter()
                        .filter_map(|p| match p {
                            Item::Hit(h) => Some(h),
                            Item::Tone(_) => None,
                        })
                        .collect(),
                )
            } else {
                return Err(format!("`{b}` mixes notes and kit letters"));
            }
        }
        b => match item(b)? {
            Item::Tone(t) => Sound::Notes(vec![t]),
            Item::Hit(h) => Sound::Hits(vec![h]),
        },
    };

    let mut len = *prev_len;
    let mut velocity_pct = 100;
    let mut staccato = false;
    let mut rest = &word[base_end..];
    let number = |s: &str| -> (Option<u32>, usize) {
        let n = s.bytes().take_while(u8::is_ascii_digit).count();
        (s[..n].parse().ok(), n)
    };
    while let Some(c) = rest.chars().next() {
        match c {
            '/' => {
                let (n, used) = number(&rest[1..]);
                len = n
                    .filter(|&n| (1..=4096).contains(&n))
                    .ok_or_else(|| format!("`{word}`: `/` takes a length in steps, 1..=4096"))?;
                rest = &rest[1 + used..];
            }
            '@' => {
                let (n, used) = number(&rest[1..]);
                velocity_pct = n
                    .filter(|&n| (1..=200).contains(&n))
                    .ok_or_else(|| format!("`{word}`: `@` takes a velocity in percent, 1..=200"))?;
                rest = &rest[1 + used..];
            }
            '>' => {
                velocity_pct = ACCENT_PCT;
                rest = &rest[1..];
            }
            '\'' => {
                staccato = true;
                rest = &rest[1..];
            }
            _ => {
                return Err(format!(
                    "`{word}`: unexpected `{c}` (a token is <sound>[/steps][>]['][@velocity])"
                ));
            }
        }
    }
    let articulated = velocity_pct != 100 || staccato;
    if articulated && matches!(sound, Sound::Rest | Sound::Tie) {
        return Err(format!(
            "`{word}`: rests and ties take no accent, staccato or velocity"
        ));
    }
    *prev_len = len;
    Ok(Token {
        sound,
        len,
        velocity_pct,
        staccato,
        at: Pos::default(),
    })
}

enum Item {
    Tone(Tone),
    Hit(char),
}

fn item(s: &str) -> Result<Item, String> {
    let (name, accidental) = match s.strip_suffix('!') {
        Some(n) => (n, true),
        None => (s, false),
    };
    if let Some(note) = Note::parse(name) {
        return Ok(Item::Tone(Tone { note, accidental }));
    }
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphabetic() && !accidental && c != 'r' => Ok(Item::Hit(c)),
        _ => Err(format!(
            "`{s}` is not a note (c4, f#3, bb2; `!` = outside the key on purpose), a kit letter, `r` or `_`"
        )),
    }
}

/// Lays the order out in time: one event list per channel, ties merged.
fn timeline(song: &Song) -> Result<Vec<Vec<Event>>, (Pos, String)> {
    let bar_steps = song.bar_steps();
    let mut out: Vec<Vec<Event>> = vec![Vec::new(); song.channels.len()];
    let mut start = 0u32;
    let mut bar0 = 0u32;
    for &pi in &song.order {
        let p = &song.patterns[pi];
        for (ci, line) in p.lines.iter().enumerate() {
            let mut step = start;
            for (bi, bar) in line.iter().enumerate() {
                for t in &bar.tokens {
                    match &t.sound {
                        Sound::Tie => {
                            let events = &mut out[ci];
                            match events.last_mut() {
                                Some(e)
                                    if !matches!(e.sound, Sound::Rest)
                                        && e.step + e.len == step =>
                                {
                                    e.len += t.len;
                                }
                                _ => {
                                    return Err((
                                        t.at,
                                        "a tie `_` needs a note or hit right before it".into(),
                                    ));
                                }
                            }
                        }
                        sound => out[ci].push(Event {
                            step,
                            len: t.len,
                            sound: sound.clone(),
                            velocity_pct: t.velocity_pct,
                            staccato: t.staccato,
                            bar: bar0 + bi as u32,
                            pattern: pi,
                            at: t.at,
                        }),
                    }
                    step += t.len;
                }
            }
        }
        start += p.bars * bar_steps;
        bar0 += p.bars;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens() {
        let mut prev = 4;
        let t = token("c#5!/6>'", &mut prev).unwrap();
        assert_eq!(t.len, 6);
        assert_eq!(prev, 6);
        assert_eq!(t.velocity_pct, ACCENT_PCT);
        assert!(t.staccato);
        assert_eq!(
            t.sound,
            Sound::Notes(vec![Tone {
                note: Note::parse("c#5").unwrap(),
                accidental: true
            }])
        );
        let t = token("[K,c]@60", &mut prev).unwrap();
        assert_eq!(t.sound, Sound::Hits(vec!['K', 'c']));
        assert_eq!((t.len, t.velocity_pct), (6, 60));
        assert_eq!(token("_/2", &mut prev).unwrap().sound, Sound::Tie);
        for bad in [
            "r>",
            "x9",
            "[c4,K]",
            "[c4,e4,g4,b4]",
            "c4/0",
            "c4/",
            "c4?",
            "[c4",
            "r!",
        ] {
            assert!(token(bad, &mut prev).is_err(), "{bad}");
        }
    }

    #[test]
    fn keys() {
        let k = Key::parse("bb major").unwrap();
        assert!(k.contains(Note::parse("eb5").unwrap()));
        assert!(!k.contains(Note::parse("e5").unwrap()));
        let m = Key::parse("d minor").unwrap();
        assert!(m.contains(Note::parse("c#4").unwrap()), "leading tone");
        assert!(m.contains(Note::parse("bb4").unwrap()));
        assert!(!m.contains(Note::parse("d#4").unwrap()));
        assert_eq!(Key::parse("h major"), None);
        assert_eq!(m.to_string(), "d minor");
    }
}
