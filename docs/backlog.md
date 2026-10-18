# Backlog

Deferred work and known issues, picked up by the work loop when relevant.

- **artkit**: `include` and `tileset` headers (autotiling) — needed for the Salzburg tileset in M1.
- **artkit lints not yet implemented**: required anims per entity kind, tile seams, readability contrast, unused/missing sprite references (docs/art-pipeline.md "Automatic lints").
- **Release atlas**: exclude the `test` sprite group from release builds.
- **Text key lint**: check that every text key used in code and levels exists in `content/text/de.toml` (part of `levels lint`, plus a scan of `*_KEY` consts in jnr-render/jnr-core).
- **levels lint (rest of L3)**: entities inside bounds, referenced sprites exist (tile/entity names vs atlas) — once M1 adds sprite-backed tiles and entities.
- **Replay expectations for M1**: `oblaten>=N` (and score) once Oblaten exist; currently a "not supported yet" parse error.
