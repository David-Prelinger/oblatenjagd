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
