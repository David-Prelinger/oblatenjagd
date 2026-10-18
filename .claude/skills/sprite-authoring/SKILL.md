---
name: sprite-authoring
description: How to create pixel art as text - the .palette and .sprite formats, tilesets, procedural generators, and the preview/lint commands. Use when writing or changing any sprite, tile, font glyph or background.
---

# Sprite authoring

Status: approved v1. Formats are specified in `docs/art-pipeline.md`; this skill is the practical guide.

## Files
- Palette: `art/palettes/main.palette` – one char per colour, `.` = transparent.
- Sprites: `art/sprites/<group>/<name>.sprite` (groups: `player`, `enemies`, `items`, `tiles/<tileset>`, `bg`, `ui`, `fonts`).
- Generator params: `art/generators/<name>.toml`.

## Sprite skeleton
```
sprite  = selfie_tourist
size    = 16x24
origin  = 8,24
hitbox  = 3,8,10,16
mirror  = auto
anim    = walk     frames=2 ticks=10 loop
anim    = squished frames=1
---
frame walk 0
<16 chars × 24 lines>
frame walk 1
...
frame squished 0
...
```

## Workflow
1. Start from a silhouette: fill the shape with one mid colour, check it reads at 1× (`cargo xtask sprites preview <name> --scale 1`).
2. Add the outline (`k`), then 2–3 shades per material, light from the top left.
3. Animate by changing as few pixels as possible between frames; keep the origin stable (no jitter).
4. `cargo xtask sprites lint` → fix all errors.
5. Look at your previews: `sprites preview <name>` (×4), `sprites anim <name>` (filmstrip/GIF), and `scene <level> --x <px>` where it appears.
6. Hand off to art-director with the image paths.

## Tips and common mistakes
- Count characters: every line must be exactly the declared width. The linter reports `file:line`.
- Avoid single-pixel noise and "pillow shading" (shading from all edges inwards).
- Use dithering only in backgrounds.
- Don't put the outline colour inside a shape for details; use a dark shade of the material.
- For palette-swapped variants use `variant = … remap a->b` instead of copying grids.
- Large art (mountains, facades, water) → use a generator, don't hand-type 320-wide grids. Generators `sky`, `mountains`, `skyline` exist; format and params in `docs/art-pipeline.md` ("Generator file format"). Preview with `cargo xtask sprites preview bg_` and in context with `cargo xtask scene <level> --x <px> --bg bg_sky_salzburg:0,bg_untersberg:3,...`.
- Autotiled tiles: a 16-frame anim `auto`, frame index = neighbour mask (N=1, E=2, S=4, W=8; same tile name or level edge = set).
