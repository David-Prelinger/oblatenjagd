//! Music lints for `.song` files: what can be checked from the text alone.
//! Errors fail `audio lint`; warnings are heuristics a reviewer reads.
//! (Bar lengths are a parse error, so every pattern and the loop end sit on
//! bar lines.)
//!
//! | Code | Level | Rule |
//! |---|---|---|
//! | `instrument` | error | every channel's instrument exists and fits its tokens (kit letters vs. notes) |
//! | `bars` | error | `bars` = the bars of `order` |
//! | `range` | error | every note lies in its instrument's `range` |
//! | `key` | error | notes outside the pattern's key carry `!` |
//! | `length` | error | loops 20–90 s, jingles ≤ 10 s (at the written tempo) |
//! | `length-of` | error | the `length_of` song exists, is a loop and is within 3 % of this one |
//! | `quote` | error | every `[[quote]]` fixture exists and the channel plays it |
//! | `provenance` | error | a song quoting a fixture whose `source` isn't "original" names its `source` |
//! | `accidental` | warning | `!` on a note that is in the key |
//! | `clash` | warning | two melodic channels a semitone (or 7th, 9th) apart on a beat |
//! | `consonance` | warning | beat 1: highest vs. lowest channel consonant in < 70 % of bars |
//! | `melody-range` | warning | the highest channel spans more than two octaves |
//! | `repeat` | warning | a pattern more than 4 times in a row |
//! | `unused-pattern` | warning | a pattern not in `order` |

use std::collections::BTreeMap;
use std::fmt;

use crate::SAMPLE_RATE;
use crate::note::Note;
use crate::patch::{Instrument, InstrumentKind};
use crate::quote::{Quote, check_quote};
use crate::song::{Event, Pos, Song, SongKind, Sound};
use crate::song_render::{LOOP_ALIGN, RenderOptions, bind, natural_frames, song_frames};

/// Shortest and longest loop (ms; the memory budget, D-114).
pub const LOOP_MS: (u64, u64) = (20_000, 90_000);
/// Longest jingle (ms; `game_over` 8 s, `fanfare_finale` ~9 s).
pub const JINGLE_MAX_MS: u64 = 10_000;
/// Largest tempo stretch for `length_of`, in percent.
pub const LENGTH_OF_MAX_PCT: u64 = 3;
/// Share of bars whose beat 1 must be consonant (percent).
pub const CONSONANCE_PCT: u32 = 70;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Error,
    Warning,
}

/// One finding, at `file:line:col`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lint {
    pub file: String,
    pub at: Pos,
    pub level: Level,
    /// The rule (table above).
    pub code: &'static str,
    pub message: String,
}

impl fmt::Display for Lint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let level = match self.level {
            Level::Error => "error",
            Level::Warning => "warning",
        };
        write!(
            f,
            "{}:{}:{}: {level}[music-{}]: {}",
            self.file, self.at.line, self.at.col, self.code, self.message
        )
    }
}

/// Everything a song may refer to.
pub struct LintContext<'a> {
    pub instruments: &'a BTreeMap<String, Instrument>,
    /// All songs by id (for `length_of`).
    pub songs: &'a BTreeMap<String, Song>,
    /// All `.quote` fixtures by id.
    pub quotes: &'a BTreeMap<String, Quote>,
}

/// Every lint of one song, errors first, then in file order.
pub fn lint_song(song: &Song, ctx: &LintContext) -> Vec<Lint> {
    let mut out = Vec::new();
    let mut push = |at: Pos, level: Level, code: &'static str, message: String| {
        out.push(Lint {
            file: song.file.clone(),
            at,
            level,
            code,
            message,
        })
    };
    use Level::{Error, Warning};

    let insts = match bind(song, ctx.instruments) {
        Ok(i) => Some(i),
        Err(e) => {
            push(
                Pos {
                    line: e.line,
                    col: e.col,
                },
                Error,
                "instrument",
                e.message,
            );
            None
        }
    };

    let order_bars: u32 = song.order.iter().map(|&p| song.patterns[p].bars).sum();
    if order_bars != song.bars {
        push(
            song.at.bars,
            Error,
            "bars",
            format!(
                "`bars` = {}, but `order` plays {order_bars} bars",
                song.bars
            ),
        );
    }

    // Per pattern (each once, however often `order` plays it).
    for p in &song.patterns {
        for (ci, line) in p.lines.iter().enumerate() {
            let ch = &song.channels[ci];
            for bar in line {
                for t in &bar.tokens {
                    let Sound::Notes(tones) = &t.sound else {
                        continue;
                    };
                    for tone in tones {
                        if let Some(InstrumentKind::Melodic {
                            range: (lo, hi), ..
                        }) = insts.as_ref().map(|i| &i[ci].kind)
                        {
                            if tone.note < *lo || tone.note > *hi {
                                push(
                                    t.at,
                                    Error,
                                    "range",
                                    format!(
                                        "{} is outside `{}`'s range {lo}..{hi}",
                                        tone.note, ch.instrument
                                    ),
                                );
                            }
                        }
                        match (p.key.contains(tone.note), tone.accidental) {
                            (false, false) => push(
                                t.at,
                                Error,
                                "key",
                                format!(
                                    "{} is not in {} (pattern {}); mark it `{}!` if it is meant",
                                    tone.note, p.key, p.name, tone.note
                                ),
                            ),
                            (true, true) => push(
                                t.at,
                                Warning,
                                "accidental",
                                format!("{}! is in {}: drop the `!`", tone.note, p.key),
                            ),
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    let nat_ms = natural_frames(song, 100) * 1000 / SAMPLE_RATE as u64;
    let s = |ms: u64| format!("{}.{} s", ms / 1000, ms % 1000 / 100);
    match song.kind {
        SongKind::Loop if nat_ms < LOOP_MS.0 || nat_ms > LOOP_MS.1 => push(
            song.at.tempo,
            Error,
            "length",
            format!(
                "the loop lasts {}; loops last {}..{} (memory budget, D-114)",
                s(nat_ms),
                s(LOOP_MS.0),
                s(LOOP_MS.1)
            ),
        ),
        SongKind::Jingle if nat_ms > JINGLE_MAX_MS => push(
            song.at.tempo,
            Error,
            "length",
            format!(
                "the jingle lasts {}; at most {}",
                s(nat_ms),
                s(JINGLE_MAX_MS)
            ),
        ),
        _ => {}
    }

    if let Some(target) = &song.length_of {
        match ctx.songs.get(target) {
            Some(t) if t.kind == SongKind::Loop => {
                let want = song_frames(t, &RenderOptions::default()).unwrap_or(LOOP_ALIGN) as u64;
                let nat = natural_frames(song, 100);
                if want.abs_diff(nat) * 100 > nat * LENGTH_OF_MAX_PCT {
                    push(
                        song.at.length_of,
                        Error,
                        "length-of",
                        format!(
                            "`{target}` lasts {want} frames, this song {nat}: more than {LENGTH_OF_MAX_PCT} % apart (change its bars or tempo)"
                        ),
                    );
                }
            }
            Some(_) => push(
                song.at.length_of,
                Error,
                "length-of",
                format!("`{target}` is a jingle, not a loop"),
            ),
            None => push(
                song.at.length_of,
                Error,
                "length-of",
                format!("no song `{target}`"),
            ),
        }
    }

    let quotes_pd = song
        .quotes
        .iter()
        .filter_map(|q| ctx.quotes.get(&q.fixture))
        .any(|f| f.source != "original");
    if quotes_pd && song.source == "original" {
        push(
            song.at.source,
            Error,
            "provenance",
            "the song quotes a public-domain tune, so `source` names the quoted work (and `edition`)"
                .into(),
        );
    }
    for q in &song.quotes {
        match ctx.quotes.get(&q.fixture) {
            None => push(
                q.at,
                Error,
                "quote",
                format!(
                    "no fixture `{}` (audio/quotes/{}.quote)",
                    q.fixture, q.fixture
                ),
            ),
            Some(fx) => {
                if let Err(m) = check_quote(song, q, fx) {
                    push(q.at, Error, "quote", m);
                }
            }
        }
    }

    if let Some(insts) = &insts {
        harmony(song, insts, &mut push);
    }

    let mut run = 1;
    for i in 1..=song.order.len() {
        if i < song.order.len() && song.order[i] == song.order[i - 1] {
            run += 1;
            continue;
        }
        if run > 4 {
            push(
                song.at.order,
                Warning,
                "repeat",
                format!(
                    "pattern {} plays {run} times in a row (more than 4 gets dull)",
                    song.patterns[song.order[i - 1]].name
                ),
            );
        }
        run = 1;
    }
    for (pi, p) in song.patterns.iter().enumerate() {
        if !song.order.contains(&pi) {
            push(
                p.at,
                Warning,
                "unused-pattern",
                format!("pattern {} is not in `order`", p.name),
            );
        }
    }

    out.sort_by_key(|l| (l.level, l.at));
    out
}

/// The event of `events` sounding at `step` (rests don't sound).
fn sounding(events: &[Event], step: u32) -> Option<&Event> {
    let i = events.partition_point(|e| e.step <= step);
    let e = events.get(i.checked_sub(1)?)?;
    (e.step + e.len > step && matches!(e.sound, Sound::Notes(_))).then_some(e)
}

fn notes(e: &Event) -> Vec<Note> {
    match &e.sound {
        Sound::Notes(t) => t.iter().map(|t| t.note).collect(),
        _ => Vec::new(),
    }
}

fn bar_list(bars: &[u32]) -> String {
    let shown: Vec<String> = bars.iter().take(12).map(|b| (b + 1).to_string()).collect();
    let more = if bars.len() > 12 { ", ..." } else { "" };
    format!("{}{more}", shown.join(", "))
}

/// The heuristics between channels: clashes, beat-1 consonance, range.
fn harmony(
    song: &Song,
    insts: &[&Instrument],
    push: &mut impl FnMut(Pos, Level, &'static str, String),
) {
    let melodic: Vec<usize> = (0..song.channels.len())
        .filter(|&ci| matches!(insts[ci].kind, InstrumentKind::Melodic { .. }))
        .filter(|&ci| song.events[ci].iter().any(|e| e.top().is_some()))
        .collect();
    let bar_steps = song.bar_steps();
    let spb = song.steps_per_beat;
    let total = song.total_steps();

    // Clashes on beats, per channel pair.
    for (i, &a) in melodic.iter().enumerate() {
        for &b in &melodic[i + 1..] {
            let mut bars: Vec<u32> = Vec::new();
            let mut first: Option<Pos> = None;
            for step in (0..total).step_by(spb as usize) {
                let (Some(ea), Some(eb)) = (
                    sounding(&song.events[a], step),
                    sounding(&song.events[b], step),
                ) else {
                    continue;
                };
                // Only where one of them starts a note on this beat.
                if ea.step != step && eb.step != step {
                    continue;
                }
                let clash = notes(ea).iter().any(|x| {
                    notes(eb)
                        .iter()
                        .any(|y| matches!((x.0 as i32 - y.0 as i32).rem_euclid(12), 1 | 11))
                });
                let bar = step / bar_steps;
                if clash && bars.last() != Some(&bar) {
                    bars.push(bar);
                    first.get_or_insert(if ea.step == step { ea.at } else { eb.at });
                }
            }
            if let Some(at) = first {
                push(
                    at,
                    Level::Warning,
                    "clash",
                    format!(
                        "`{}` and `{}` are a semitone (or 7th/9th) apart on a beat in bar(s) {}",
                        song.channels[a].name,
                        song.channels[b].name,
                        bar_list(&bars)
                    ),
                );
            }
        }
    }

    if melodic.is_empty() {
        return;
    }
    let mean = |ci: usize| {
        let ns: Vec<u32> = song.events[ci]
            .iter()
            .flat_map(notes)
            .map(|n| n.0 as u32)
            .collect();
        ns.iter().sum::<u32>() * 100 / ns.len().max(1) as u32
    };
    let top = *melodic
        .iter()
        .max_by_key(|&&c| (mean(c), usize::MAX - c))
        .unwrap();
    let low = *melodic.iter().min_by_key(|&&c| (mean(c), c)).unwrap();

    let tops: Vec<Note> = song.events[top].iter().filter_map(Event::top).collect();
    let (lo, hi) = (tops.iter().min(), tops.iter().max());
    if let (Some(lo), Some(hi)) = (lo, hi) {
        if hi.0 - lo.0 > 24 {
            push(
                song.channels[top].at,
                Level::Warning,
                "melody-range",
                format!(
                    "`{}` spans {lo}..{hi} ({} semitones); more than two octaves is hard to sing along",
                    song.channels[top].name,
                    hi.0 - lo.0
                ),
            );
        }
    }

    if top == low {
        return;
    }
    let (mut counted, mut good) = (0u32, 0u32);
    let mut bad: Vec<u32> = Vec::new();
    for bar in 0..total / bar_steps {
        let step = bar * bar_steps;
        let (Some(m), Some(b)) = (
            sounding(&song.events[top], step),
            sounding(&song.events[low], step),
        ) else {
            continue;
        };
        let (Some(m), Some(b)) = (m.top(), notes(b).into_iter().min()) else {
            continue;
        };
        counted += 1;
        if matches!(
            (m.0 as i32 - b.0 as i32).rem_euclid(12),
            0 | 3 | 4 | 7 | 8 | 9
        ) {
            good += 1;
        } else {
            bad.push(bar);
        }
    }
    if counted > 0 && good * 100 < CONSONANCE_PCT * counted {
        push(
            song.channels[top].at,
            Level::Warning,
            "consonance",
            format!(
                "beat 1: `{}` against `{}` is a unison, 3rd, 5th, 6th or octave in {good} of {counted} bars (want {CONSONANCE_PCT} %); dissonant in bar(s) {}",
                song.channels[top].name,
                song.channels[low].name,
                bar_list(&bad)
            ),
        );
    }
}
