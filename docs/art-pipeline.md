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
- Several animations per file (`anim = walk frames=4 ticks=6 loop`).
- `mirror = auto` creates left-facing versions.
- `variant = tourist_b remap r->b l->g` creates palette-swapped copies.
- `include = ...` reuses frames (e.g. same legs, different head).
- Hitboxes are defined next to the art so gameplay and visuals stay in sync.

Tiles use the same format, with `tileset = salzburg` and a 16×16 size; autotiling rules (edges, corners) are declared in a tileset header.

## Procedural generators (`crates/jnr-artkit/src/gen/`, params in `art/generators/*.toml`)

For large or repetitive art where hand-written grids would be tedious:

| Generator | Output |
|---|---|
| `mountains` | parallax mountain silhouettes (Untersberg, Schafberg, Falkenstein) from seeded midpoint displacement + hand-placed peak profiles |
| `sky` | dithered gradient skies with clouds |
| `water` | animated lake / Salzach tiles with highlights |
| `facades` | Getreidegasse house fronts from building blocks (windows, shutters, signs) |
| `foliage` | trees, hedges, meadows with dithering |

Generators are seeded and deterministic and only output palette colours, so their results go through the same lints as hand-made sprites.

## Build and preview

| Command | What it does |
|---|---|
| `cargo build` | `jnr-assets/build.rs` compiles all art into atlases; any error fails the build with `file:line` |
| `cargo xtask sprites preview [filter]` | writes PNGs per sprite, scaled ×4, to `target/preview/sprites/` |
| `cargo xtask sprites sheet` | one contact sheet of all sprites with names (`target/preview/contact_sheet.png`, ImageMagick `montage`) |
| `cargo xtask sprites anim <name>` | animation filmstrip + GIF |
| `cargo xtask scene <level> --x <px>` | renders a real game scene at a position (`target/preview/scenes/`) |
| `cargo xtask sprites lint` | runs the automatic art checks |

## Validation of art

### 1. Automatic lints (`cargo xtask sprites lint`, part of `xtask check`)
- Only palette colours; grid sizes match the declared `size`; all frames the same size.
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
