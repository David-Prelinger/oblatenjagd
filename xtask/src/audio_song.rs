//! Songs in xtask (E57), used by `cargo xtask audio` (`audio/mod.rs`):
//! load every instrument, song and quote fixture of an audio directory,
//! render a song file, lint the songs' text ([`lint_songs`]) and draw the
//! piano roll (`<id>_roll.png` in `audio render`).
//!
//! The piano roll is read like a score: one row ("system") per 8 bars, one
//! lane per channel (top to bottom as in `[channels]`), pitch upwards with a
//! line and a label at every C, bar numbers and pattern names above, bar
//! lines bright, beat lines dim. A note is a bar in its channel's colour,
//! brighter for louder (accents), with a white first column (its onset) and
//! as long as it sounds (staccato: half). A note marked `!` has a magenta
//! top edge; a note the lints reject (outside the key unmarked, outside the
//! instrument's range) is red. Kit lanes have one row per patch. A loop ends
//! in a green line (it wraps to bar 1).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use jnr_artkit::image::Image;
use jnr_audio::music_lint::Level;
use jnr_audio::patch::InstrumentKind;
use jnr_audio::song::{Event, Song, SongKind, Sound};
use jnr_audio::{Instrument, LintContext, Quote, RenderOptions, SongRender, lint_song};

/// Every instrument, song and quote fixture under one directory.
pub struct SongBank {
    pub instruments: BTreeMap<String, Instrument>,
    pub songs: BTreeMap<String, Song>,
    pub quotes: BTreeMap<String, Quote>,
}

/// Files under `dir` (recursively) with extension `ext`, sorted.
fn files(dir: &Path, ext: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Loads `*.inst`, `*.song` and `*.quote` under `dir`; `root` shortens the
/// file names in messages. `Err` lists every parse error (`file:line:col`).
pub fn load_bank(root: &Path, dir: &Path) -> Result<SongBank, Vec<String>> {
    let mut errors = Vec::new();
    let read = |p: &Path| -> (String, String) {
        let name = p.strip_prefix(root).unwrap_or(p).display().to_string();
        (name, fs::read_to_string(p).unwrap_or_default())
    };
    let mut bank = SongBank {
        instruments: BTreeMap::new(),
        songs: BTreeMap::new(),
        quotes: BTreeMap::new(),
    };
    for p in files(dir, "inst") {
        let (name, text) = read(&p);
        match Instrument::parse(&name, &text) {
            Ok(i) => {
                bank.instruments.insert(i.id.clone(), i);
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    for p in files(dir, "quote") {
        let (name, text) = read(&p);
        match Quote::parse(&name, &text) {
            Ok(q) => {
                bank.quotes.insert(q.id.clone(), q);
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    for p in files(dir, "song") {
        let (name, text) = read(&p);
        match Song::parse(&name, &text) {
            Ok(s) => {
                bank.songs.insert(s.id.clone(), s);
            }
            Err(e) => errors.push(e.to_string()),
        }
    }
    if errors.is_empty() {
        Ok(bank)
    } else {
        Err(errors)
    }
}

/// The music lints of every song under `dir` (or only song `id`): (number
/// of errors, report). Warnings are listed but don't count.
pub fn lint_songs(root: &Path, dir: &Path, id: Option<&str>) -> (usize, String) {
    let bank = match load_bank(root, dir) {
        Ok(b) => b,
        Err(errors) => {
            let text: String = errors.iter().map(|e| format!("{e}\n")).collect();
            return (errors.len(), text);
        }
    };
    let ctx = LintContext {
        instruments: &bank.instruments,
        songs: &bank.songs,
        quotes: &bank.quotes,
    };
    let mut text = String::new();
    let (mut errors, mut warnings, mut songs) = (0, 0, 0);
    for song in bank.songs.values().filter(|s| id.is_none_or(|i| s.id == i)) {
        songs += 1;
        for l in lint_song(song, &ctx) {
            match l.level {
                Level::Error => errors += 1,
                Level::Warning => warnings += 1,
            }
            text.push_str(&format!("{l}\n"));
        }
    }
    text.push_str(&format!(
        "music lint: {songs} song(s), {errors} error(s), {warnings} warning(s)\n"
    ));
    (errors, text)
}

/// The audio directory of a song file (`<audio>/music/<id>.song`) and the
/// directory its messages are relative to (the workspace root).
fn dirs_of(path: &Path) -> (PathBuf, PathBuf) {
    let audio = path
        .parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new("."))
        .to_path_buf();
    let root = audio.parent().unwrap_or(Path::new(".")).to_path_buf();
    (audio, root)
}

/// The song in `path` with the bank of its audio directory.
fn song_of_file(path: &Path) -> Result<(SongBank, String), String> {
    let (audio, root) = dirs_of(path);
    let bank = load_bank(&root, &audio).map_err(|e| e.join("\n"))?;
    let id = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !bank.songs.contains_key(&id) {
        return Err(format!("{}: no song `{id}`", path.display()));
    }
    Ok((bank, id))
}

/// A song's kind, if the file parses.
pub fn song_kind(path: &Path) -> Option<SongKind> {
    let text = fs::read_to_string(path).ok()?;
    Song::parse(&path.to_string_lossy(), &text)
        .ok()
        .map(|s| s.kind)
}

/// Renders `id` from `bank` through `jnr_audio::song_render::render_in`
/// (a `length_of` song at its target's length).
pub fn render_in(bank: &SongBank, id: &str, opts: &RenderOptions) -> Result<SongRender, String> {
    jnr_audio::song_render::render_in(&bank.songs, &bank.instruments, id, opts)
        .map_err(|e| e.to_string())
}

/// Parses and renders a song file with the instruments, songs and quotes
/// of its audio directory.
pub fn render_song_file(path: &Path) -> Result<(Song, SongRender), String> {
    let (bank, id) = song_of_file(path)?;
    let r = render_in(&bank, &id, &RenderOptions::default())?;
    Ok((bank.songs[&id].clone(), r))
}

/// The piano roll of a song file.
pub fn roll_of_file(path: &Path) -> Result<Image, String> {
    let (bank, id) = song_of_file(path)?;
    Ok(piano_roll(&bank.songs[&id], &bank.instruments))
}

// ------------------------------------------------------------- piano roll

const BARS_PER_SYSTEM: u32 = 8;
const MARGIN: u32 = 44;
const SEMI: u32 = 2;
const KIT_ROW: u32 = 6;
const HEAD: u32 = 9;
const GAP: u32 = 3;

const BG: [u8; 4] = [18, 18, 26, 255];
const LANE: [u8; 4] = [28, 28, 40, 255];
const C_LINE: [u8; 4] = [48, 48, 66, 255];
const BEAT: [u8; 4] = [40, 40, 56, 255];
const BAR: [u8; 4] = [110, 110, 140, 255];
const TEXT: [u8; 4] = [210, 210, 220, 255];
const DIM_TEXT: [u8; 4] = [130, 130, 150, 255];
const LOOP: [u8; 4] = [60, 220, 90, 255];
const WRONG: [u8; 4] = [255, 40, 40, 255];
const MARKED: [u8; 4] = [255, 60, 255, 255];
const ONSET: [u8; 4] = [255, 255, 255, 255];
const COLOURS: [[u8; 3]; 6] = [
    [255, 200, 60],
    [90, 190, 255],
    [255, 120, 80],
    [140, 230, 120],
    [200, 140, 255],
    [230, 230, 230],
];

/// A lane: what it shows and how tall it is.
enum Lane {
    /// Pitches `lo..=hi` (MIDI), plus the instrument's range for colouring.
    Melodic {
        lo: u8,
        hi: u8,
        range: Option<(u8, u8)>,
    },
    /// Kit letters (in sorted order) and their patch names.
    Kit { rows: Vec<(char, String)> },
}

impl Lane {
    fn height(&self) -> u32 {
        match self {
            Lane::Melodic { lo, hi, .. } => (*hi - *lo + 1) as u32 * SEMI + 2,
            Lane::Kit { rows } => rows.len().max(1) as u32 * KIT_ROW + 2,
        }
    }
}

fn lanes(song: &Song, instruments: &BTreeMap<String, Instrument>) -> Vec<Lane> {
    song.events
        .iter()
        .enumerate()
        .map(|(ci, events)| {
            let inst = instruments.get(&song.channels[ci].instrument);
            let hits: Vec<char> = events
                .iter()
                .flat_map(|e| match &e.sound {
                    Sound::Hits(h) => h.clone(),
                    _ => Vec::new(),
                })
                .collect();
            if !hits.is_empty() || matches!(inst.map(|i| &i.kind), Some(InstrumentKind::Kit { .. }))
            {
                let mut letters = hits;
                letters.sort();
                letters.dedup();
                let rows = letters
                    .into_iter()
                    .map(|c| {
                        let name = match inst.map(|i| &i.kind) {
                            Some(InstrumentKind::Kit { hits }) => {
                                hits.get(&c).map_or(c.to_string(), |s| s.id.clone())
                            }
                            _ => c.to_string(),
                        };
                        (c, name)
                    })
                    .collect();
                return Lane::Kit { rows };
            }
            let notes: Vec<u8> = events
                .iter()
                .flat_map(|e| match &e.sound {
                    Sound::Notes(t) => t.iter().map(|t| t.note.0).collect(),
                    _ => Vec::new(),
                })
                .collect();
            let lo = notes.iter().min().copied().unwrap_or(60).saturating_sub(1);
            let hi = notes.iter().max().copied().unwrap_or(60).saturating_add(1);
            let range = match inst.map(|i| &i.kind) {
                Some(InstrumentKind::Melodic { range, .. }) => Some((range.0.0, range.1.0)),
                _ => None,
            };
            Lane::Melodic { lo, hi, range }
        })
        .collect()
}

/// Draws the piano roll of `song` (2x scaled for reading).
pub fn piano_roll(song: &Song, instruments: &BTreeMap<String, Instrument>) -> Image {
    let bar_steps = song.bar_steps();
    let pps = (32u32.div_ceil(bar_steps)).max(2);
    let bar_w = bar_steps * pps;
    let total_bars = song.total_steps() / bar_steps;
    let systems = total_bars.div_ceil(BARS_PER_SYSTEM).max(1);
    let lanes = lanes(song, instruments);
    let lanes_h: u32 = lanes.iter().map(|l| l.height() + GAP).sum();
    let system_h = HEAD + lanes_h + 6;
    let title_h = 10;
    let kind = match song.kind {
        SongKind::Loop => "loop",
        SongKind::Jingle => "jingle",
    };
    let secs10 = jnr_audio::song_render::natural_frames(song, 100) * 10 / 44_100;
    let title = format!(
        "{} {kind} {} {}bpm {} {}bars {}.{}s",
        song.id,
        song.meter,
        song.tempo_bpm,
        song.key,
        total_bars,
        secs10 / 10,
        secs10 % 10
    );
    let w = (4 * title.len() as u32 + 4)
        .max(MARGIN + BARS_PER_SYSTEM.min(total_bars.max(1)) * bar_w + 4);
    let h = title_h + systems * system_h;
    let mut img = Image::new(w, h);
    img.fill(BG);
    text(&mut img, 2, 2, &title, TEXT);

    // Pattern starts (song bar -> name).
    let mut starts = BTreeMap::new();
    let mut b = 0;
    for &pi in &song.order {
        starts.insert(b, song.patterns[pi].name.clone());
        b += song.patterns[pi].bars;
    }

    for sys in 0..systems {
        let y0 = title_h + sys * system_h;
        let first = sys * BARS_PER_SYSTEM;
        let bars = (total_bars - first).min(BARS_PER_SYSTEM);
        let x_of = |step: u32| MARGIN + (step - first * bar_steps) * pps;
        for bi in 0..bars {
            let bar = first + bi;
            let x = MARGIN + bi * bar_w;
            text(&mut img, x + 1, y0 + 1, &(bar + 1).to_string(), DIM_TEXT);
            if let Some(name) = starts.get(&bar) {
                let nx = x + 1 + 4 * (bar + 1).to_string().len() as u32 + 2;
                text(&mut img, nx, y0 + 1, name, LOOP);
            }
        }
        let mut ly = y0 + HEAD;
        for (ci, lane) in lanes.iter().enumerate() {
            let lh = lane.height();
            rect(&mut img, MARGIN, ly, bars * bar_w, lh, LANE);
            text(&mut img, 1, ly, &song.channels[ci].name, TEXT);
            match lane {
                Lane::Melodic { lo, hi, .. } => {
                    for n in *lo..=*hi {
                        if n % 12 == 0 {
                            let y = ly + 1 + (*hi - n) as u32 * SEMI;
                            rect(&mut img, MARGIN, y + SEMI - 1, bars * bar_w, 1, C_LINE);
                            if lh >= 14 {
                                let label = format!("c{}", n / 12 - 1);
                                text(
                                    &mut img,
                                    MARGIN.saturating_sub(4 * label.len() as u32 + 1),
                                    y.saturating_sub(2),
                                    &label,
                                    DIM_TEXT,
                                );
                            }
                        }
                    }
                }
                Lane::Kit { rows } => {
                    for (ri, (_, name)) in rows.iter().enumerate() {
                        let y = ly + 1 + ri as u32 * KIT_ROW;
                        let short: String = name.chars().take(5).collect();
                        text(
                            &mut img,
                            MARGIN.saturating_sub(4 * short.len() as u32 + 1),
                            y,
                            &short,
                            DIM_TEXT,
                        );
                    }
                }
            }
            // Beat and bar lines.
            for s in 0..=bars * bar_steps {
                let x = MARGIN + s * pps;
                if s % bar_steps == 0 {
                    rect(&mut img, x, ly, 1, lh, BAR);
                } else if s % song.steps_per_beat == 0 {
                    rect(&mut img, x, ly, 1, lh, BEAT);
                }
            }
            let range_lo = first * bar_steps;
            let range_hi = (first + bars) * bar_steps;
            for e in &song.events[ci] {
                if e.step >= range_hi || e.step + e.len <= range_lo {
                    continue;
                }
                draw_event(
                    &mut img, song, ci, e, lane, ly, pps, range_lo, range_hi, &x_of,
                );
            }
            ly += lh + GAP;
        }
        if song.kind == SongKind::Loop && first + bars == total_bars {
            let x = MARGIN + bars * bar_w;
            rect(&mut img, x, y0 + HEAD, 2, lanes_h, LOOP);
        }
    }
    img.scaled(2)
}

#[allow(clippy::too_many_arguments)]
fn draw_event(
    img: &mut Image,
    song: &Song,
    ci: usize,
    e: &Event,
    lane: &Lane,
    ly: u32,
    pps: u32,
    lo: u32,
    hi: u32,
    x_of: &dyn Fn(u32) -> u32,
) {
    let len = if e.staccato {
        (e.len / 2).max(1)
    } else {
        e.len
    };
    let s0 = e.step.max(lo);
    let s1 = (e.step + len).min(hi);
    if s1 <= s0 {
        return;
    }
    let x0 = x_of(s0) + 1;
    let wpx = ((s1 - s0) * pps).saturating_sub(1).max(1);
    let base = COLOURS[ci % COLOURS.len()];
    let gain = e.velocity_pct.min(150);
    let colour = |k: u32| -> [u8; 4] {
        let c = |v: u8| ((v as u32 * k / 125).min(255)) as u8;
        [c(base[0]), c(base[1]), c(base[2]), 255]
    };
    let key = song.patterns[e.pattern].key;
    match (&e.sound, lane) {
        (Sound::Notes(tones), Lane::Melodic { hi: top, range, .. }) => {
            for t in tones {
                let n = t.note.0;
                if n > *top {
                    continue;
                }
                let y = ly + 1 + (*top - n) as u32 * SEMI;
                let out_of_range = range.is_some_and(|(a, b)| n < a || n > b);
                let fill = if out_of_range || (!t.accidental && !key.contains(t.note)) {
                    WRONG
                } else {
                    colour(gain)
                };
                rect(img, x0, y, wpx, SEMI, fill);
                if t.accidental {
                    rect(img, x0, y, wpx, 1, MARKED);
                }
                if e.step >= lo {
                    rect(img, x0, y, 1, SEMI, ONSET);
                }
            }
        }
        (Sound::Hits(hits), Lane::Kit { rows }) => {
            for h in hits {
                if let Some(ri) = rows.iter().position(|(c, _)| c == h) {
                    let y = ly + 1 + ri as u32 * KIT_ROW;
                    rect(img, x0, y + 1, wpx.min(2 * pps), KIT_ROW - 2, colour(gain));
                    if e.step >= lo {
                        rect(img, x0, y + 1, 1, KIT_ROW - 2, ONSET);
                    }
                }
            }
        }
        _ => {}
    }
}

fn rect(img: &mut Image, x: u32, y: u32, w: u32, h: u32, px: [u8; 4]) {
    for yy in y..(y + h).min(img.height) {
        for xx in x..(x + w).min(img.width) {
            img.set(xx, yy, px);
        }
    }
}

/// 3x5 glyphs (rows top to bottom, `#` = on); letters in either case.
fn glyph(c: char) -> [&'static str; 5] {
    match c.to_ascii_lowercase() {
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["###", "..#", "###", "#..", "###"],
        '3' => ["###", "..#", ".##", "..#", "###"],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "###", "..#", "###"],
        '6' => ["###", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", ".#.", ".#.", ".#."],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "###"],
        'a' => [".#.", "#.#", "###", "#.#", "#.#"],
        'b' => ["##.", "#.#", "##.", "#.#", "##."],
        'c' => [".##", "#..", "#..", "#..", ".##"],
        'd' => ["##.", "#.#", "#.#", "#.#", "##."],
        'e' => ["###", "#..", "##.", "#..", "###"],
        'f' => ["###", "#..", "##.", "#..", "#.."],
        'g' => [".##", "#..", "#.#", "#.#", ".##"],
        'h' => ["#.#", "#.#", "###", "#.#", "#.#"],
        'i' => ["###", ".#.", ".#.", ".#.", "###"],
        'j' => ["..#", "..#", "..#", "#.#", ".#."],
        'k' => ["#.#", "#.#", "##.", "#.#", "#.#"],
        'l' => ["#..", "#..", "#..", "#..", "###"],
        'm' => ["#.#", "###", "###", "#.#", "#.#"],
        'n' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'o' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'p' => ["##.", "#.#", "##.", "#..", "#.."],
        'q' => [".#.", "#.#", "#.#", "##.", ".##"],
        'r' => ["##.", "#.#", "##.", "#.#", "#.#"],
        's' => [".##", "#..", ".#.", "..#", "##."],
        't' => ["###", ".#.", ".#.", ".#.", ".#."],
        'u' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'v' => ["#.#", "#.#", "#.#", "#.#", ".#."],
        'w' => ["#.#", "#.#", "###", "###", "#.#"],
        'x' => ["#.#", "#.#", ".#.", "#.#", "#.#"],
        'y' => ["#.#", "#.#", ".#.", ".#.", ".#."],
        'z' => ["###", "..#", ".#.", "#..", "###"],
        '#' => ["#.#", "###", "#.#", "###", "#.#"],
        '/' => ["..#", "..#", ".#.", "#..", "#.."],
        '.' => ["...", "...", "...", "...", ".#."],
        ':' => ["...", ".#.", "...", ".#.", "..."],
        '-' => ["...", "...", "###", "...", "..."],
        '_' => ["...", "...", "...", "...", "###"],
        _ => ["...", "...", "...", "...", "..."],
    }
}

fn text(img: &mut Image, x: u32, y: u32, s: &str, px: [u8; 4]) {
    for (i, c) in s.chars().enumerate() {
        let gx = x + 4 * i as u32;
        for (row, bits) in glyph(c).iter().enumerate() {
            for (col, b) in bits.bytes().enumerate() {
                if b == b'#' {
                    let (xx, yy) = (gx + col as u32, y + row as u32);
                    if xx < img.width && yy < img.height {
                        img.set(xx, yy, px);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    fn fixtures() -> PathBuf {
        root().join("crates/jnr-audio/tests/fixtures/music")
    }

    #[test]
    fn the_fixture_songs_lint_clean() {
        let (errors, text) = lint_songs(&root(), &fixtures(), None);
        assert_eq!(errors, 0, "{text}");
        assert!(
            text.contains("2 song(s), 0 error(s), 0 warning(s)"),
            "{text}"
        );
        let (_, one) = lint_songs(&root(), &fixtures(), Some("placeholder_fanfare"));
        assert!(one.contains("music lint: 1 song(s)"), "{one}");
    }

    #[test]
    fn a_broken_song_reports_file_and_line() {
        let dir = std::env::temp_dir().join(format!("jnr-audio-song-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::copy(fixtures().join("lead.inst"), dir.join("lead.inst")).unwrap();
        let good = fs::read_to_string(fixtures().join("placeholder_fanfare.song")).unwrap();
        let song = dir.join("placeholder_fanfare.song");
        // A bar of the wrong length is a parse error at the bar (line 21).
        fs::write(&song, good.replace("e4/2 |", "e4/3 |")).unwrap();
        let (errors, report) = lint_songs(&root(), &dir, None);
        assert_eq!(errors, 1, "{report}");
        assert!(report.contains("placeholder_fanfare.song:21:"), "{report}");
        assert!(report.contains("error[score]"), "{report}");
        // Parsed, the missing `bass` instrument (and quote fixture) are lints.
        fs::write(&song, good).unwrap();
        let (errors, report) = lint_songs(&root(), &dir, None);
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!(errors, 2, "{report}");
        assert!(report.contains("placeholder_fanfare.song:17:"), "{report}");
        assert!(report.contains("error[music-instrument]"), "{report}");
        assert!(report.contains("error[music-quote]"), "{report}");
    }

    #[test]
    fn piano_roll_of_the_polka() {
        let bank = load_bank(&root(), &fixtures()).unwrap();
        let song = &bank.songs["placeholder_polka"];
        let img = piano_roll(song, &bank.instruments);
        // 24 bars in 3 systems of 8; a 2/4 bar of 8 steps is 32 px; 2x scale.
        assert_eq!(img.width, 2 * (MARGIN + 8 * 32 + 4));
        let lanes = lanes(song, &bank.instruments);
        let lanes_h: u32 = lanes.iter().map(|l| l.height() + GAP).sum();
        assert_eq!(img.height, 2 * (10 + 3 * (HEAD + lanes_h + 6)));
        let has =
            |px: [u8; 4]| (0..img.height).any(|y| (0..img.width).any(|x| img.get(x, y) == px));
        assert!(has(LOOP));
        assert!(has(ONSET));
        assert!(has(MARKED), "the eb5! in pattern B");
        assert!(!has(WRONG));
    }

    #[test]
    fn hurry_and_length_of_renders() {
        let bank = load_bank(&root(), &fixtures()).unwrap();
        let normal = render_in(&bank, "placeholder_polka", &RenderOptions::default()).unwrap();
        let hurry = RenderOptions {
            tempo_pct: 112,
            length_frames: None,
        };
        let fast = render_in(&bank, "placeholder_polka", &hurry).unwrap();
        assert!(fast.length_frames < normal.length_frames);
        assert_eq!(fast.length_frames % 4096, 0);
    }
}
