---
name: sfx-authoring
description: How to make game audio as text - the .sfx, .inst and .song formats for the integer synth, the render/lint/golden commands, and how to judge a sound you can't hear from its stats, spectrogram and piano roll. Use when writing or changing any sound effect, instrument or music, or when reviewing audio.
---

# SFX and music authoring

Status: draft v1 (MA). Formats are specified in `docs/architecture.md` → "jnr-audio"; what to make is in `docs/audio-design.md` (D-113). This skill is the practical guide.

## Files
- Sound effects: `audio/sfx/<id>.sfx` — 1–8 `[[layer]]` tables, all values integers with unit-named keys (`freq_hz`, `decay_ms`, `volume_pct`, …).
- Instruments: `audio/instruments/<id>.inst` — melodic (a `range` plus layers that take their pitch from the note, ADSR) or a drum kit (`[kit]` letters → patches).
- Songs: `audio/music/<id>.song` — TOML header (tempo, meter, key, channels → instruments, loop or jingle, `source`/`edition` for quotes) plus bar-structured note lines per channel.
- The id is the file stem and must match the ids in `audio-design.md` §5/§6 (the cue director refers to them).
- Examples of every key: `crates/jnr-audio/tests/fixtures/` (test fixtures; don't copy their sound, copy their syntax).

## Commands
- `cargo xtask audio render <id>` / `--all` → `$CARGO_TARGET_DIR/audio/<id>.{wav,json,wave.png,spec.png}` (songs also `.roll.png`).
- `cargo xtask audio lint` — signal lints (ceiling, clamps, DC, length bounds, silent tail) and music lints (bar lengths, ranges, key, loop point).
- `cargo xtask audio golden` — PCM hashes; `--accept` only by the orchestrator after a game-designer verdict.
- `cargo xtask replay <file> --audio` — offline mix of a real run (cue timeline + spectrogram) to check timing and the mix.

## Judging a sound without ears
- **Duration and envelope**: the stats' duration and the waveform PNG against the design row (attack click? tail too long? silent head?).
- **Pitch contour**: the spectrogram's fundamental line against the described contour (rises, falls, arpeggio steps, vibrato wobble starting when it should).
- **Loudness**: peak against the class ceiling, RMS for sustained sounds; zero clamps.
- **Family**: render neighbours (e.g. all item sounds) and compare their spectrograms side by side; one character per family.
- **Music**: the piano roll for voice ranges, the Nachschlag on the off-beats, the bass on 1 (and 3), the melody's phrase shape (4+4 bars), the Oblaten-Motiv where required; for quotes the melody fixture must pass.
- **Common mistakes**: aliasing on high square leads (keep leads ≤ C6); noise through a narrow band-pass clamping (check the clamp count); several layers in phase summing over the ceiling; a loop whose last bar doesn't lead back into bar 1.

## Review rubric (game-designer)
For each id: matches its row in `audio-design.md` (character, length, contour, priority class)? Loudness in class? Reads clearly at game speed (a cue timeline from a replay)? For music: meter, tempo, key and form as specified; quote faithful to its edition; jokes land (e.g. the Mozart quote forced into 3/4). Verdict APPROVED or numbered fixes with ids.
