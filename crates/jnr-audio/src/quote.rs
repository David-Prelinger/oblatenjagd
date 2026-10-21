//! Melody fixtures (`audio/quotes/<id>.quote`): the first notes of a quoted
//! tune, transcribed from its public-domain edition, which a song's
//! `[[quote]]` must match (D-113, `docs/audio-design.md` §9.3). A pattern
//! edit that breaks a quote fails the `quote` lint.
//!
//! ```toml
//! # audio/quotes/haydn_94_andante.quote
//! source = "Haydn, Symphony No. 94, II. Andante, bars 1-2 (violin I)"
//! edition = "Breitkopf & Haertel (J. Rietz), c. 1855"
//! steps_per_beat = 2          # the fixture's own grid (2: eighths in 2/4)
//! notes = '''
//! c4/1 c4 e4 e4 | g4 g4 e4/2
//! '''
//! ```
//!
//! `notes` uses the score tokens (notes, `r`, `_`; `|` is only for reading,
//! and accidentals need no `!`). The song side names the fixture, the
//! channel and the bar:
//!
//! ```toml
//! [[quote]]
//! fixture = "haydn_94_andante"
//! channel = "lead"
//! bar = 1              # 1-based song bar; `step = 2` starts later in it (a pickup)
//! transpose = 0        # semitones from the fixture to the song
//! augment = 1          # the song's lengths are the fixture's times this
//! compare = "exact"    # exact (pitch and length) | pitches
//! ```

use serde::Deserialize;
use toml::Spanned;

use crate::error::{ErrorKind, ParseError, Src};
use crate::note::Note;
use crate::patch::{file_id, from_toml};
use crate::song::{
    Event, Locator, QuoteMatch, QuoteRef, Song, Sound, strip_comment, token, value_start,
};

/// A parsed `.quote` fixture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Quote {
    pub id: String,
    pub source: String,
    pub edition: String,
    pub steps_per_beat: u32,
    /// `None` = a rest; lengths in the fixture's steps, ties merged.
    pub notes: Vec<(Option<Note>, u32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawQuote {
    source: Spanned<String>,
    edition: Spanned<String>,
    steps_per_beat: Spanned<i64>,
    notes: Spanned<String>,
}

impl Quote {
    /// Parses a `.quote` file; the id is the file stem of `file`.
    pub fn parse(file: &str, text: &str) -> Result<Quote, ParseError> {
        let src = Src { file, text };
        let id = file_id(&src, ".quote")?;
        let raw: RawQuote = from_toml(&src)?;
        let spb = *raw.steps_per_beat.get_ref();
        if !(1..=12).contains(&spb) {
            return Err(src.err(
                raw.steps_per_beat.span(),
                ErrorKind::Range,
                format!("`steps_per_beat` = {spb} is outside 1..=12"),
            ));
        }
        for s in [&raw.source, &raw.edition] {
            if s.get_ref().trim().is_empty() {
                return Err(src.err(s.span(), ErrorKind::Music, "provenance must not be empty"));
            }
        }
        let loc = Locator {
            src: &src,
            start: value_start(&src, &raw.notes),
            fallback: raw.notes.span().start,
        };
        // Comments blanked out, so offsets stay those of the source.
        let text: String = raw
            .notes
            .get_ref()
            .split_inclusive('\n')
            .map(|l| {
                let code = strip_comment(l);
                format!("{code}{}", " ".repeat(l.len() - code.len()))
            })
            .collect();
        let text = text.as_str();
        let mut prev = spb as u32;
        let mut notes: Vec<(Option<Note>, u32)> = Vec::new();
        let mut off = 0;
        for word in text.split_whitespace() {
            let at = off + text[off..].find(word).unwrap_or(0);
            off = at + word.len();
            if word == "|" {
                continue;
            }
            let t = token(word, &mut prev).map_err(|m| loc.err(at, ErrorKind::Score, m))?;
            match t.sound {
                Sound::Notes(n) if n.len() == 1 => notes.push((Some(n[0].note), t.len)),
                Sound::Rest => match notes.last_mut() {
                    Some((None, len)) => *len += t.len,
                    _ => notes.push((None, t.len)),
                },
                Sound::Tie => match notes.last_mut() {
                    Some((Some(_), len)) => *len += t.len,
                    _ => {
                        return Err(loc.err(at, ErrorKind::Score, "a tie needs a note before it"));
                    }
                },
                _ => {
                    return Err(loc.err(
                        at,
                        ErrorKind::Score,
                        format!("`{word}`: a fixture is one melody line (single notes and rests)"),
                    ));
                }
            }
        }
        if notes.is_empty() {
            return Err(src.err(raw.notes.span(), ErrorKind::Score, "no notes"));
        }
        Ok(Quote {
            id,
            source: raw.source.get_ref().clone(),
            edition: raw.edition.get_ref().clone(),
            steps_per_beat: spb as u32,
            notes,
        })
    }
}

fn show(n: Option<Note>) -> String {
    n.map_or("r".into(), |n| n.to_string())
}

/// Checks that `song` plays `quote` where `r` says. `Err` describes the
/// first difference (with the song's `file:line:col` of the wrong note).
pub fn check_quote(song: &Song, r: &QuoteRef, quote: &Quote) -> Result<(), String> {
    let start = (r.bar - 1) * song.bar_steps() + r.step;
    // The channel's line as (note or rest, start, len), gaps as rests.
    let mut line: Vec<(Option<Note>, u32, u32, Option<&Event>)> = Vec::new();
    let mut at = start;
    for e in song.events[r.channel]
        .iter()
        .filter(|e| e.step + e.len > start)
    {
        if e.step < start {
            return Err(format!(
                "{}: quote `{}`: a note is still sounding at bar {} step {}",
                song.loc(e.at),
                quote.id,
                r.bar,
                r.step
            ));
        }
        if e.step > at {
            push_rest(&mut line, at, e.step - at);
        }
        match &e.sound {
            Sound::Rest => push_rest(&mut line, e.step, e.len),
            _ => line.push((e.top(), e.step, e.len, Some(e))),
        }
        at = e.step + e.len;
    }
    let qs = quote.steps_per_beat;
    let ss = song.steps_per_beat;
    for (i, &(want, wlen)) in quote.notes.iter().enumerate() {
        let want = want.map(|n| Note((n.0 as i32 + r.transpose) as u8));
        let Some(&(got, gstart, glen, ev)) = line.get(i) else {
            return Err(format!(
                "{}: quote `{}`: note {} ({}) is missing: the song ends first",
                song.loc(r.at),
                quote.id,
                i + 1,
                show(want)
            ));
        };
        let where_ = ev.map_or_else(|| song.loc(r.at), |e| song.loc(e.at));
        let bar = gstart / song.bar_steps() + 1;
        let is_last = i + 1 == quote.notes.len();
        // The last fixture note may be held longer in the song (a cadence).
        let len_ok = match r.compare {
            QuoteMatch::Pitches => true,
            QuoteMatch::Exact => {
                let w = wlen as u64 * r.augment as u64 * ss as u64;
                let g = glen as u64 * qs as u64;
                g == w || (is_last && g > w)
            }
        };
        if got != want || !len_ok {
            return Err(format!(
                "{where_}: quote `{}` note {} (song bar {bar}): expected {} for {} beat(s), found {} for {} beat(s)",
                quote.id,
                i + 1,
                show(want),
                beats(wlen as u64 * r.augment as u64, qs),
                show(got),
                beats(glen as u64, ss),
            ));
        }
    }
    Ok(())
}

fn push_rest(line: &mut Vec<(Option<Note>, u32, u32, Option<&Event>)>, step: u32, len: u32) {
    match line.last_mut() {
        Some((None, _, l, _)) => *l += len,
        _ => line.push((None, step, len, None)),
    }
}

/// `steps` of `per_beat` as a reduced fraction of beats (`3/2`).
fn beats(steps: u64, per_beat: u32) -> String {
    let (mut a, mut b) = (steps, per_beat as u64);
    let (mut x, mut y) = (a, b);
    while y != 0 {
        (x, y) = (y, x % y);
    }
    if x > 1 {
        a /= x;
        b /= x;
    }
    if b == 1 {
        a.to_string()
    } else {
        format!("{a}/{b}")
    }
}
