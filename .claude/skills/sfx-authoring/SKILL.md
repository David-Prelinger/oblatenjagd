---
name: sfx-authoring
description: How to make game audio as text - the .sfx, .inst and .song formats for the integer synth, the render/lint/golden commands, and how to judge a sound you can't hear from its stats, spectrogram and piano roll. Use when writing or changing any sound effect, instrument or music, or when reviewing audio.
---

# SFX and music authoring

Status: approved v1 (MA). Formats are specified in `docs/architecture.md` → "jnr-audio"; what to make is in `docs/audio-design.md` (D-113). This skill is the practical guide.

## Files
- Sound effects: `audio/sfx/<id>.sfx` — 1–8 `[[layer]]` tables, all values integers with unit-named keys (`freq_hz`, `decay_ms`, `volume_pct`, …).
- Instruments: `audio/instruments/<id>.inst` — melodic (a `range` plus layers that take their pitch from the note, ADSR) or a drum kit (`[kit]` letters → patches).
- Songs: `audio/music/<id>.song` — TOML `[meta]` (kind loop | jingle, tempo, meter 2/4 | 3/4 | 6/8, grid, key, bars, order, `source`/`edition` for quotes), `[channels]` (≤ 6, instrument + volume, `arp_ms` for chords) and `[pattern.X] score = '''…'''` with one line per channel, bars separated by `|` and aligned under each other. Tokens: `c5/4` (note, length in steps), `c#5!` (an intended accidental), `[d4,f4,bb4]'` (an arpeggiated chord, staccato), `K` / `[K,S]` (kit hits), `r`, `_` (tie), `>` (accent), `@60` (velocity). Full reference with an example: `docs/architecture.md`, "`.song`". A loop plays from bar 1 forever (stage intros are separate jingles, D-114); a music jingle (fanfare, `game_over`, `death`) is a `.song` with `kind = "jingle"`.
- Quote fixtures: `audio/quotes/<id>.quote` — the quoted tune's first notes, transcribed from the PD edition (with `source`, `edition`); the song names it in a `[[quote]]` (channel, bar, step, transpose) and `audio lint` fails if the song stops matching. One per quote, plus the Oblaten-Motiv.
- Lint targets: `audio/targets.toml`, one `[<id>]` table per `.sfx` with `class` (loud | normal | soft | ambience | jingle | song; required; songs need no entry: a loop is class `song`, a music jingle `jingle`, unless an entry marks it `placeholder` or sets a peak), optional `peak_dbfs` (the SFX table's peak where it differs from the class), `duration_ms`, `centroid_hz = [lo, hi]` (set from a measured render: naive pulses alias, so centroids run high) and `placeholder = true` for a stand-in (never a golden).
- The id is the file stem and must match the ids in `audio-design.md` §5/§6 (the cue director refers to them).
- Examples of every key: `crates/jnr-audio/tests/fixtures/` (test fixtures; don't copy their sound, copy their syntax).

## Commands
- `cargo xtask audio render <id>|<file.sfx>|<file.song>|--all` → `$CARGO_TARGET_DIR/preview/audio/<id>.wav`, `<id>.json` (stats; loops also `loop_frames`, `loop_aligned`), `<id>_wave.png`, `<id>_spec.png`, songs also `<id>_roll.png` (the piano roll: 8 bars per row, a lane per channel, C lines labelled, red = a note the lints reject, magenta edge = `!`, green line = the loop end); `--all` also `sheet_spectrograms.png` (every sound side by side on one time scale). How to read the images: `docs/validation.md`, "Audio".
- `cargo xtask audio lint [id]` — signal lints against `audio/targets.toml`: clamp, ceiling (−1 dBFS), peak (class band or `peak_dbfs` ± 1 dB), rms (jingles), dc, onset (≤ 10 ms), tail (≤ 50 ms), edges (no click), length, duration, centroid (loops skip onset, tail and edges: they wrap; songs: peak ≤ −8, RMS −20 ± 2, 20–90 s); plus the music lints with `file:line:col` (notes in key or marked `!`, instrument ranges, length, quotes, provenance; a bar that doesn't fill the meter is already a parse error; warnings for semitone clashes on beats, beat-1 consonance under 70 %, a melody over two octaves, a pattern > 4 times in a row).
- `cargo xtask audio golden` — PCM hashes in `tests/golden/audio.toml`; `--accept <id>` only by the orchestrator after a game-designer verdict (refuses placeholders and sounds that fail lint).
- `cargo xtask replay <file> --audio` — offline mix of a real run (cue timeline + spectrogram) to check timing and the mix.

## Judging a sound without ears
- **Duration and envelope**: the stats' duration and the waveform PNG against the design row (attack click? tail too long? silent head?).
- **Pitch contour**: the spectrogram's fundamental line against the described contour (rises, falls, arpeggio steps, vibrato wobble starting when it should).
- **Loudness**: peak against the class ceiling, RMS for sustained sounds; zero clamps.
- **Family**: render neighbours (e.g. all item sounds) and compare their spectrograms side by side; one character per family.
- **Music**: the piano roll for voice ranges, the Nachschlag on the off-beats, the bass on 1 (and 3), the melody's phrase shape (4+4 bars), the Oblaten-Motiv where required; for quotes the melody fixture must pass.
- **Mixing a song**: a loop needs peak ≤ −8 and RMS ≥ −22 dBFS, so at most ~14 dB crest factor. Sustained voices (melody, second voice, bass) raise the RMS; short kicks and asymmetric narrow pulses (12.5/25 %) raise the peak more than the RMS. If the peak is over but the RMS is low, lower the drums and widen the lead's duty before touching the master levels.
- **Common mistakes**: aliasing on high square leads (keep leads ≤ C6); noise through a narrow band-pass clamping (check the clamp count); several layers in phase summing over the ceiling; a loop whose last bar doesn't lead back into bar 1.

## Review rubric (game-designer)
For each id: matches its row in `audio-design.md` (character, length, contour, priority class)? Loudness in class? Reads clearly at game speed (a cue timeline from a replay)? For music: meter, tempo, key and form as specified; quote faithful to its edition; jokes land (e.g. the Mozart quote forced into 3/4). Verdict APPROVED or numbered fixes with ids.
