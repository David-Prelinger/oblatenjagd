# audio/

Sound sources for the game (milestone MA; formats in `docs/architecture.md`, section
"jnr-audio"):

- `sfx/*.sfx`: one sound effect per file (id = file stem)
- `jingles/*.sfx`: longer one-shots (stage intro, stage clear, game over, …)
- `instruments/*.inst`: note-driven instruments and drum kits for songs
- `music/*.song`: one loop per file (format: E57)

The sound-designer authors these. The jnr-audio crate's test fixtures live in
`crates/jnr-audio/tests/fixtures/`, not here.
