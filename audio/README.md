# audio/

Sound sources for the game (milestone MA; formats in `docs/architecture.md`, section
"jnr-audio"):

- `sfx/*.sfx`: one sound effect per file (id = file stem)
- `jingles/*.sfx`: longer one-shots (stage intro, stage clear, game over, …)
- `instruments/*.inst`: note-driven instruments and drum kits for songs
- `music/*.song`: one loop per file (format: E57)
- `targets.toml`: the lint targets of every `.sfx` (class, peak, duration, centroid band,
  `placeholder`); review data for `cargo xtask audio lint`, not a sound source
  (`docs/validation.md`, "Audio")

The sound-designer authors these. The jnr-audio crate's test fixtures live in
`crates/jnr-audio/tests/fixtures/`, not here.

## SFX families (S1)

The 25 "must" SFX of `docs/audio-design.md` §6.2/§6.3, one character per family:

- **Player** (`jump`, `zucker`, `hackl`, `hurt`, `throw`, `stroke`): pulse 25 %/50 % with
  pitch glides and arpeggios, a 7 kHz low-pass against aliasing; noise only for the axe swish
  and the swim bubble.
- **Items** (`oblate`, `oneup`, `checkpoint`): bright pulse 25 % notes (E6–C7) with a crisp
  click, or a sine bell with inharmonic partials.
- **World / enemies / boss** (`bump`, `brick`, `stomp`, `roof_bounce`, `axe_hit`, `hohes_c`,
  `boss_telegraph`, `boss_breathe`, `boss_hit`, `bills_hang`): low triangle / pulse 50 %
  bodies under short noise transients; the boss cues are crockery ticks, whistles and breath.
- **UI** (`ui_start`, `pause`, `tally_tick`, `tally_end`, `kassa_zeile`, `muenze`): pulse 25 %
  chimes and sine bells (the till, the spoon on the cup, the coin).

Notes for the cue director: `boss_telegraph` is rendered at the longest telegraph (667 ms) and
is cut at the dash start in later rounds; `hohes_c` is cut when the singer is defeated. The
design's variants without an own id (`jump` a fourth lower when Big/Hackl, `pause` two
semitones lower when leaving, `stomp` chain pitch steps) are not authored.

Noise layers through a low-pass carry DC (the LFSR's start is the same every time): give them
a `highpass_hz` of ~100 Hz, but never on the layer that ends last (the high-pass's ring-out
leaves a non-zero last sample, the `edges` lint).
