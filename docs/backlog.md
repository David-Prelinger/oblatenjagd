# Backlog

Deferred work and known issues, picked up by the work loop when relevant.

- **artkit**: `include` and `tileset` headers (autotiling) — needed for the Salzburg tileset in M1.
- **artkit lints not yet implemented**: required anims per entity kind, tile seams, readability contrast, unused/missing sprite references (docs/art-pipeline.md "Automatic lints").
- **Release atlas**: exclude the `test` sprite group from release builds.
- **Text key lint**: check that every text key used in code and levels exists in `content/text/de.toml` (part of `levels lint`, plus a scan of `*_KEY` consts in jnr-render/jnr-core).
- **Font polish**: at x4 the lowercase `e` (with shadow) and short `t` can look like "=" and "d"; reads fine at 1x. Revisit in a later font pass (art-director note, M0 goldens review).
- **levels lint (rest of L3)**: entities inside bounds, referenced sprites exist (tile/entity names vs atlas) — once M1 adds sprite-backed tiles and entities.
- **Per-level `min_jump_window`** (solver, L4): `LevelMeta::min_jump_window` is parsed (E1); the `solver` verify step must still fail when the narrowest window is smaller (target ≥ 6 frames in stages 1–3).
- **Enemies (E1 placeholders)**: enemy entities stand still; contact hurts, the generic stomp rule and the stomp chain work. Movement patterns, activation timers, „bumping a block defeats an enemy standing on it", stomp using the enemy's previous position, squish animations: enemy task. The Festungsbahn entity is parsed and drawn but inert (needs end points in the level format).
- **Items on bumped blocks**: a Mozartkugel/Oblate standing on a block that is bumped is not knocked (SMB does); add with the enemy-bump rule.
- **Score popups for static Oblaten**: none (SMB style); the design says popups float "above whatever scored" – game-designer to confirm.
- **Goal trigger vs visual**: the goal trigger is the whole 16 px tile column, the grey-box pole is drawn thin at its centre, so completion happens ~7 px before the visible touch. Align when the Maibaum sprite lands in M1 (trigger = pole hitbox from the sprite, or draw the pole wider).
- **No ceilings in the testbed**: head-bump/ceiling corners are only unit-tested; add an overhang section to a test level when convenient.
- **M1 art review notes (M0 acceptance)**: (a) lowercase `e` with `k` shadow can fill in — try a 2-row eye if the HUD feels mushy; (b) prove HUD text over `h` haze and `w` clouds with a shot; (c) grey-box hazard and player share `r` — irrelevant once Resi's sprite lands, otherwise give the hazard an `R` top edge; (d) review sprite animations with `filmstrip --every 1/2` + `sprites anim`, 30-tick filmstrips can't show flicker.
- **Recorder vs replay limit**: `jnr-app --record` has no frame cap but the parser rejects > 216 000 frames; stop recording at `MAX_TOTAL_FRAMES` with a warning (code review, M0 QA fixes).
- **scene clamp, odd hitbox widths**: use `(w + 1) / 2` for both bounds in `scene_game` (off by 0.5 px for odd `hitbox_w`; current w = 12 is unaffected).
- **xtask sprites `compile()` duplicates `Project::compile`** to keep groups — expose the group from artkit (e.g. `group` on `CompiledSprite`).
- **Enemy art polish (optional)**: Fiaker telegraph feather open at the frame top (drop hat 1 px); `k` seam between horse hindquarters and carriage; selfie flash lower ray reads as part of the hat (move to (10–11,0)).
- **Golden coverage**: add testbed_route shots mid-run and falling so walk/run/fall animations are covered by goldens (art-director note).
- **Sky generator quirk**: `checker` dither on a transition starting at an odd row produces no dither (row_mix thresholds 12/4 vs a 4/16 mix) in `crates/jnr-artkit/src/generators/sky.rs`; Salzburg sky works around it with `until = 45`.
- **bg_untersberg seam polish**: column 0 rows 41–44 → `f` so the lit face continues across the wrap (art-director note).
- **HUD × before counters**: the `×` in `w` nearly vanishes on `h` haze — draw it in `Y` like the numbers or drop it (art-director note; for the HUD task).
- **Maibaum ribbon polish**: frame 1 inner white ribbons end in 3 px `k` blocks at rows 26–27; carry a 1 px `w` (col 6 / col 25) down rows 26–28.
- **Debris vs brick**: check the `debris` colours match the merged brick tile.
