# Art pipeline

Status: **approved v1**. Everything is produced inside the dev container as text plus code. No external pixel editor (D-004).

## Style rules

- Internal resolution **320×180**, integer-scaled. Tiles **16×16**. Player 16×24 (normal) / 16×32 (Zuckerschock).
- **One master palette** of ≤32 colours (`art/palettes/main.palette`). Every pixel of every asset must use it.
- 1 px dark outline on characters and interactive objects; backgrounds without outlines, lower contrast.
- Light from the top left. Max 3 shades per material.
- Parallax layers use desaturated/bluer palette colours the further away they are (atmospheric perspective: Untersberg, Schafberg).
- Animations: idle 2 frames, walk 4, run 4, jump 1, fall 1, enemy walk 2. All animations run at fixed frame counts in the 60 Hz simulation (e.g. change frame every 8 ticks).
- Satire should be readable at sprite size: tourists show selfie sticks and cameras, the Fiaker horse wears a hat, Mozartkugeln have the red-silver wrapper look.

A full written style guide with examples will live in `docs/art-style.md` (written by `pixel-artist`, approved by `art-director` in Milestone 0).

## Palette format (`art/palettes/main.palette`)

Example (illustrative; the source of truth is `art/palettes/main.palette`):

```
# char  hex       name
.       none      transparent
k       #1a1c2c   outline
w       #f4f4f4   white
s       #94b0c2   sky_light
b       #3b5dc9   lake_deep
t       #41a6f6   lake_turquoise
g       #38b764   pine_light
G       #257179   pine_dark
r       #b13e53   dirndl_red
l       #7a4a2a   lederhosen_brown
o       #e8b860   oblate_gold
O       #b8823a   oblate_brown
...
```

Each palette entry is one character, so sprite grids can use it directly. A sprite can choose a sub-palette or remap characters (palette swaps, e.g. tourist shirt colours).

## Sprite format (`art/sprites/**/*.sprite`)

```
sprite  = oblate
size    = 12x12
origin  = 6,12          # anchor (feet / bottom centre)
anim    = spin  frames=4 ticks=8 loop
hitbox  = 1,1,10,10
---
frame 0
....kkkk....
..kkooookk..
.kooOoOoOok.
.koOoOoOoOk.
...
frame 1
...
```

Features:
- Several animations per file (`anim = walk frames=4 ticks=6 loop`; illustrative values, the real frame counts and ticks are in `docs/art-style.md` §9).
- `mirror = auto` creates left-facing versions.
- `variant = tourist_b remap r->b l->g` creates palette-swapped copies.
- `include = ...` reuses frames (e.g. same legs, different head). *(Not implemented yet: the parser rejects it with a clear error.)*
- `frame <anim> <index>` starts a frame; plain `frame <index>` is allowed when the sprite has a single animation. Single-frame animations may omit `ticks` (held).
- Hitboxes are defined next to the art so gameplay and visuals stay in sync.

Tiles use the same format with a 16×16 size. Autotiling (D-020): a tile sprite with a 16-frame animation `auto` is drawn with frame = neighbour mask (bit 1 = same tile name above, 2 = right, 4 = below, 8 = left; the level edge counts as same); other tile sprites show their first animation, animated by its ticks. *(The `tileset` header is not implemented.)*

## Procedural generators (`crates/jnr-artkit/src/generators/`, params in `art/generators/*.toml`)

For large or repetitive art where hand-written grids would be tedious:

| Generator | Output | Status |
|---|---|---|
| `sky` | gradient bands with ordered-dither transitions, optional clouds | implemented (M1) |
| `mountains` | tileable mountain silhouettes (Untersberg, Schafberg, Falkenstein) from seeded periodic midpoint displacement + hand-placed peaks, lit/shadow split along the ridges, optional snow caps | implemented (M1) |
| `skyline` | mid/near layers of building blocks (flat/gable/dome/tower roofs, windows) on an optional hill: Festung, old-town roofs | implemented (M1) |
| `water` | animated lake / Salzach tiles with highlights | planned |
| `facades` | Getreidegasse house fronts from building blocks (windows, shutters, signs) | planned |
| `foliage` | trees, hedges, meadows with dithering | planned |

Generators are seeded and deterministic (own xorshift, integer math only) and only output palette colours, so their results go through the same lints as hand-made sprites. Every generated sprite is a plain sprite to the rest of the pipeline: `jnr-assets/build.rs` packs it into the atlas, and `cargo xtask sprites lint|preview|sheet|anim` show it like any `.sprite` (e.g. `cargo xtask sprites preview bg_`). The pixel-artist may always replace a generated sprite with a hand-drawn `.sprite` of the same name (remove the generator entry; names must stay unique).

### Generator file format

One TOML file can hold any number of sprites. Each entry is an array-of-tables item named after its generator (`[[sky]]`, `[[mountains]]`, `[[skyline]]`) and produces exactly one sprite: group `bg` (unless `group` is set), origin 0,0, one held animation `idle`, no mirror. Unknown keys are errors. Every error points to `file:line:col` (TOML syntax errors with rule `syntax`, bad values with rule `generator`; generated rows report the line of the entry's `name`).

```toml
seed = 1                    # optional: default seed of every entry in this file

[[mountains]]
name   = "bg_untersberg"    # sprite name (lowercase, digits, `_`), unique across all art
group  = "bg"               # optional, default "bg"
seed   = 11                 # optional: overrides the file seed (default 0)
width  = 640                # px, 1..=2048; backgrounds <= 640x180 (lint `bg-size`)
height = 120
lit    = "f"                # colour params are single palette chars (opaque, never `.`)
...                         # generator params, below
```

All x coordinates wrap around the width, so every generated layer tiles horizontally by construction. Other numbers are range-checked (coordinates and lengths within ±8192, roughness/smooth/slant 0..=256, `random_clouds` <= 64), so odd values give an error, never a panic in the build script.

**`sky`** — bands top to bottom, transitions dithered, clouds on top:

| Key | Default | Meaning |
|---|---|---|
| `bands` | required | `[{ colour = "S", until = 64 }, ..., { colour = "h" }]`: `until` = first row of the next band (growing; every band but the last needs it, the last reaches the bottom) |
| `dither` | 4 | transition height in px (0..=8, art-style §6), centred on each boundary; a band must be at least this tall |
| `pattern` | `"bayer4"` | `bayer4` (4x4 Bayer) or `checker` (2x2); the width must be a multiple of 4 (or 2) |
| `cloud_colour` / `cloud_shadow` | — / `cloud_colour` | cloud body and its bottom 2 rows; `cloud_colour` is required if there are clouds |
| `[[sky.cloud]]` | none | hand-placed cloud boxes `x`, `y`, `width` (>= 8), `height` (>= 4, <= width): round puffs on a flat base |
| `random_clouds` / `random_cloud_rows` | 0 / `[0, height / 2]` | extra clouds from the seed (24..56 x 8..14 px) with their top in these rows, never overlapping another cloud (skipped if no space) |

**`mountains`** — one silhouette, column heights from periodic midpoint displacement:

| Key | Default | Meaning |
|---|---|---|
| `peaks` | none | `[[x, height], ...]` control points (height in px from the bottom, 1..=height); the profile runs through them and wraps from the last to the first |
| `base` | `height / 2` | height at x 0 when there are no peaks |
| `roughness` | 20 | max displacement in px per 64 px of segment length (0 = straight lines between peaks) |
| `lit` / `shadow` | required | the two tones; a pixel is lit when the terrain rises towards the right near it (light from the top left) |
| `smooth` | 3 | slope window in px at the surface (it widens by depth / 2 below it, so small bumps only shade the skin) |
| `ridge_slant` | 8 | the lit/shadow split runs down and right from each peak: `depth * slant / 16` px to the right |
| `snow` / `snow_line` / `snow_depth` / `snow_shadow` | none / required with `snow` / 8 / `snow` | snow above `snow_line` (px from the bottom, +-3 px wobble), at most `snow_depth` px below the surface; `snow_shadow` on the shadow side |

**`skyline`** — an optional hill plus building blocks drawn in list order (later ones in front):

| Key | Default | Meaning |
|---|---|---|
| `hill` | none | `[[x, height], ...]` points, linear between them (wrapping) |
| `hill_roughness` | 0 | midpoint wobble of the hill (like `roughness`) |
| `hill_colour` / `hill_shadow` | required with `hill` / `hill_colour` | split like `mountains` |
| `[[skyline.building]]` | none | one block per entry, keys below |

Building keys: `x`, `width` (>= 2), `height` (wall rows), `base` (px from the bottom; default: the lowest hill point under the building, else 0), `roof` (`flat` | `gable` | `dome` | `tower`, default `flat`), `roof_height` (default: gable (width + 1) / 2, dome width / 2, tower width; a tower is a narrow block a third as wide with a small gable cap; a dome gets a lantern 2 px wide and max(roof_height / 3, 2) + 1 px tall), `wall`, `wall_shadow` + `shadow_width` (darker right edge, default width / 5), `roof_colour` (required for gable/dome/tower), `roof_shadow` (right half of the roof), `eave` (1 px line on top of the wall), `window` (colour; no windows without it), `window_size` `[2, 3]`, `window_gap` `[3, 4]`, `window_margin` `[2, 3]`, `window_chance` (percent of windows drawn, from the seed, default 100).

Stage 1: the sky is generated (`art/generators/salzburg_sky.toml`); the Untersberg, Festung and rooftop layers started as generator drafts and were replaced by hand-drawn sprites in `art/sprites/bg/` (both ways are allowed).

### Parallax layers in the renderer

`RenderContext::with_background(&[(sprite, factor16)])` sets the layers back to front; the factor is in sixteenths of the camera movement (0 = static sky, 3 ≈ 0.2 far, 8 = 0.5 mid, 13 ≈ 0.8 near, 16 = playfield). Each layer is tiled horizontally from x = `-((camera.x * factor / 16) mod width)` and **bottom-aligned to the screen bottom**, fixed vertically (no vertical parallax yet). The flat sky `s` is drawn behind the layers unless the first layer is fully opaque and at least 180 px tall. Until levels carry the list, preview layers with `cargo xtask scene <level> --x <px> --bg bg_sky_salzburg:0,bg_untersberg:3,bg_festung:8,bg_rooftops:13`.

## Build and preview

Note: the grey-box fixtures in `art/sprites/test/` (group `test`) are currently compiled into the game atlas like every other sprite. They will be excluded from release builds later.

| Command | What it does |
|---|---|
| `cargo build` | `jnr-assets/build.rs` compiles all art into atlases; any error fails the build with `file:line` |
| `cargo xtask sprites preview [filter] [--scale N]` | writes PNGs per sprite (frames side by side, wrapped at 16 frames or ~1600 px per row, mirrored copies below), scaled ×4, to `target/preview/sprites/`; the `ui` group is shown on palette `B` (navy) so white glyphs are visible |
| `cargo xtask sprites sheet [--group <name>]` | one contact sheet of all sprites (or one group) with names, rows wrapped at ~2000 px (`target/preview/contact_sheet.png` or `contact_sheet_<group>.png`, ImageMagick `montage` + `convert`) |
| `cargo xtask sprites anim <name>` | filmstrip `target/preview/anim/<name>.png` (one row per animation) + one GIF per animation `target/preview/anim/<name>_<anim>.gif` (timing approximate: GIF delays are in 1/100 s) |
| `cargo xtask scene <level> --x <px> [--bg sprite:factor,...]` | renders a real game scene at a position (`target/preview/scenes/`), optionally with parallax layers |
| `cargo xtask sprites lint` | runs the automatic art checks |

## Validation of art

### 1. Automatic lints (`cargo xtask sprites lint`, part of `xtask check`)
- Only palette colours; grid sizes match the declared `size`; all frames the same size.
- Background layers (group `bg`, hand-made or generated): warning `bg-tileable` if the seam (last column → column 0) differs in more rows than the most different pair of neighbouring columns inside; warning `bg-size` above 640×180.
- Required animations exist per entity kind (e.g. every enemy has `walk` and `squished`/`hit`).
- No fully transparent frames; outline pixels exist on character sprites.
- Tiles: edges line up seamlessly with their declared neighbours (repeat-tile test).
- **Readability check**: average contrast between a sprite's silhouette and each background it's used on is above a threshold.
- No unused sprites and no references to missing sprites.

### 2. Visual review by the `art-director` agent
- Looks at contact sheets, animation filmstrips and rendered scenes (Claude can view PNGs).
- Reviews against `docs/art-style.md` with a fixed rubric (see `.claude/skills/visual-review/`).
- Returns `APPROVED` or a numbered list of concrete fixes. The `pixel-artist` iterates until approved (max 3 rounds; then the orchestrator decides, e.g. simplifies the asset, and records the decision).

### 3. Golden snapshots
- Approved scenes become golden screenshots in `tests/golden/`. Any unintended pixel change fails `cargo xtask verify` and produces a diff image.

## Human involvement

None required. The art-director approves the palette, the style guide and all assets. The human can look at `target/preview/contact_sheet.png` or the milestone reports whenever they want.
