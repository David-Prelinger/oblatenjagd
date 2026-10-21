# audio/

Sound sources for the game (milestone MA; formats in `docs/architecture.md`, section
"jnr-audio"):

- `sfx/*.sfx`: one sound effect per file (id = file stem)
- `jingles/*.sfx`: longer one-shots (stage intro, stage clear, game over, …)
- `instruments/*.inst`: note-driven instruments and drum kits for songs
- `music/*.song`: one song per file, a loop or a music jingle (format: `docs/architecture.md`, "`.song`")
- `quotes/*.quote`: melody fixtures of quoted tunes, which the songs' `[[quote]]`s must match
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

## Instruments and music (S2a)

Instruments (`instruments/`, audio-design §2.1): `fluegelhorn` (M1), `klarinette` (M1 variant),
`tenorhorn` (M2), `helikon` (M3), `begleitung` (M4 Nachschlag, played as chip chords),
`orgel_satz` (sustained band chords: the funeral march, the Tusch), `schlagwerk` (M5 kit:
`K` grosse_trommel, `S` kleine_trommel, `B` becken, `C` becken_kurz (choked), `X` klatschen,
`T` triangel, `M` trommel_dumpf), and the special colours `glocke` (the `hurry` church bell)
and `helikon_plumps` (the `death` fall C3 to G2).

Mixing notes for songs (the music bus wants peak ≤ −8 dBFS at RMS −20 ± 2):
- Every note restarts its oscillator at phase 0, so onsets on the same step add their peaks.
  The Flügelhorn is a 75 % pulse and the Tenorhorn a 25 % pulse: the same spectrum, opposite
  polarity, so lead and second voice don't stack. Keep that pairing in new instruments.
- Kicks and the Helikon land together on beat 1; keep the kit low (5–9 % channel volume) and
  let the sustained voices carry the RMS.
- Chords with `arp_ms = 0` sum three in-phase voices; hold them quietly (the Tusch) or
  arpeggiate them.
- Jingles end on a choked cymbal (`C`) or a release, not the long `B`: a 400 ms linear decay
  leaves more than 50 ms under −40 dBFS (the `tail` lint).

Intros (D-114: a loop plays from sample 0): `title` (bars 1–4, the post-horn call) and
`salzburg` (bars 1–2, the vamp) keep their intro bars inside the loop, so they recur once per loop.

Quotes (`quotes/`): `mozart_k606_1` and `mozart_k596` are read from the AMA pages on IMSLP
(page-1 previews; the PDFs are behind a captcha), `mozart_k525_1` from Mozart's autograph
(IMSLP #562645, the AMA file could not be opened). `oblaten_motiv` and `oblaten_motiv_moll`
are the leitmotif.
