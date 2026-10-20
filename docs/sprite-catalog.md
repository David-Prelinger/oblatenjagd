# Sprite catalog (M1, M2, M3, M4, M5)

The contract between pixel-artist and engine: sprite names, sizes, origins and animation names. The engine looks sprites up by these names and falls back to grey boxes when one is missing, so art and code can be built in parallel. Sizes are maxima unless marked exact; the hitboxes in `game-design.md` are gameplay numbers and live in `content/tuning.toml` / the engine, not here.

Conventions:
- Origin = the point placed at the entity's feet (bottom centre of its hitbox) unless noted.
- `mirror = auto` for anything that faces left/right; the art faces **right**.
- Animation ticks are at 60 Hz (art-style.md §9). "held" = single frame.
- If an animation listed here is missing, the engine falls back to the first animation of the sprite.

## Player (group `player`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `player` (Resi, Normal) | 16×24 exact | 8,24 | `idle` 2f/30t loop · `walk` 4f/8t loop · `run` 4f/5t loop · `skid` held · `jump` held · `fall` held · `die` held (facing the camera, arms up) · `pole` 2f/8t loop (sliding down the Maibaum) |
| `player_big` (Resi, Zuckerschock) | 16×32 exact | 8,32 | same names as `player` |

`skid` faces the direction of motion (Resi brakes while still sliding forward), not the new input direction. The `pole` frames have no pole drawn: the engine places her with the pole just right of her body.

Hurt/invulnerability and the 30-frame transformation are drawn by the engine (blinking / alternating `player` and `player_big`); no extra frames.

## Enemies (group `enemies`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `selfie_tourist` | 16×24 | 8,24 | `walk` 2f/10t loop · `selfie` 2f (frame 0 = stick up; frame 1 = flash — the engine picks the frame and shows the flash for only 2 game frames) · `squished` held |
| `taube` | 16×16 | 8,16 | `fly` 2f/8t loop · `sit` 2f/30t loop · `tumble` held |
| `mozartkugel_verkaeufer` | 16×24 | 8,24 | `idle` 2f/30t loop · `windup` held · `throw` held · `squished` held |
| `mozartkugel_thrown` | 8×8 | 4,8 | `spin` 2f/6t loop (unwrapped brown chocolate ball) |
| `fiaker` | 48×32 | 24,32 | `idle` 2f/30t loop · `telegraph` 2f/6t loop · `charge` 2f/6t loop (faces left; no mirror) |

## Items (group `items`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `oblate` | 12×12 | 6,12 | `spin` 4f/8t loop |
| `mozartkugel` (power-up, wrapped) | 12×12 | 6,12 | `idle` held |
| `zaunerstollen` (1-up) | 16×12 | 8,12 | `idle` held |
| `debris` (brick piece) | 8×8 | 4,8 | `spin` 2f/4t loop |

## Tiles (group `tiles`, 16×16 exact, origin 0,0, name = `tile_<tile name>`)

- A tile with an animation named `auto` of **16 frames** is autotiled: frame index = neighbour mask, bit 1 = same tile above, 2 = right, 4 = below, 8 = left (the level edge counts as "same"). Otherwise the first animation is used and animates by its ticks (e.g. water).
- M1 tile names (the level-designer uses these; more can be added by agreement): `ground` (cobblestone street), `meadow` (grass on earth), `rock` (Festungsberg rock), `rampart` (fortress wall), `hedge` (Mirabell hedge), `stone` (fountain / statue stone), `bridge` (Makartsteg deck), `railing` (love-lock railing, decorative, non-solid variant `railing_bg`), `salzach_water` (water surface row, D-050, animated 4f/12t), `salzach_water_deep` (water, rows below the surface), `brick`, `oblaten_box`, `oblaten_box_empty`, `oneway_sign` (Zunftschild), `oneway_burger` (Zunftschild variant; one-way tiles autotile left/right ends: a run of them reads as one iron arm with a wall bracket on the left and the emblem on the right; top 5 px = walking surface), `oneway_wood` (Jedermann stage, grandstand), `arch` (Dombögen, solid), `facade` (house wall with windows, autotiled — Erker, house blocks), `arch_under` (arch underside with the curved soffit, autotiled left/right; `arch` and `arch_under` count as one autotile family, so an arch on its underside has no edge between them), `invisible` (solid, never drawn, no sprite — under props such as the Sphaera).

## Props and goal (group `props`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `checkpoint` (Haltestelle) | 16×32 | 8,32 | `idle` held · `active` held |
| `maibaum_top` (wreath, ribbons, Wurstkranz) | 32×48 | 16,48 (sits on top of the pole) | `idle` 2f/20t loop (ribbons) |
| `maibaum_pole` | 16×16 exact | 8,16 | `idle` held — repeated vertically, pole ~4 px wide, centred |
| `sign_street` | ≤ 80×28 | bottom centre | `idle` held — board with a text area ≥ 74×20 px (2 lines of 12 chars) |
| `sign_bus` | ≤ 80×40 | bottom centre | `idle` held — Postbus/Haltestelle style, same text area |
| `sign_ortsende` | ≤ 80×28 | bottom centre | `idle` held — white board; the engine draws the text, then the red diagonal bar sprite `sign_ortsende_bar` (same size) over it |
| `festungsbahn` (funicular car) | 48×24 | 24,24 | `idle` held |
| `sphaera` (Kapitelplatz gold ball with the man on top) | ≤ 32×48 | bottom centre | `idle` held — drawn as a `prop` over 2×2 solid `stone` tiles; the statue on top stands on a 1×1 tile |
| `pegasus` (Mirabell Pegasus statue) | ≤ 32×40 | bottom centre | `idle` held — stands on the fountain plinth |
| `fountain_water` (basin water surface) | 48×8 | bottom centre | `idle` 4f/12t loop — decorative, not a hazard |
| `zwerg` (Zwergerlgarten dwarf statue) | 16×16 | bottom centre | `idle` held — stands on a 1×1 stone |
| `residenzbrunnen` (Residenzbrunnen tiers) | ≤ 80×64 | bottom centre | `idle` 2f/12t loop (water) — over the 5-wide fountain tiles |
| `geburtshaus` (Mozart's birthplace façade, yellow) | ≤ 80×112 | bottom centre | `idle` held — `layer = "back"` prop behind the street (only playfield set piece with a `Y` façade, art-style §7) |
| `dombogen` (Dombögen passage) | 112×80 | bottom centre | `idle` held — `layer = "back"` prop under the Dombögen (7 tiles wide; the top 16 px fill the `arch_under` soffit cut-out with dark vault, the lower 64 px are the passage with a limestone pier at each end); placed on the centre column of the passage, bottom row |

## UI (group `ui`)

| Sprite | Size | Notes |
|---|---|---|
| `font` | 8×8 | D-012 |
| `title_logo` | ≤ 240×48 | „Oblatenjagd" logo; origin = centre |
| `hud_oblate` | 8×8 | small Oblate icon before the OBLATEN counter (optional) |

## Backgrounds (group `bg`, produced by generators or hand-made)

Parallax layers, each tileable horizontally (left and right edges match; lint `bg-tileable`), ≤ 640 px wide, ≤ 180 px high (lint `bg-size`), origin 0,0, first animation used. The level's `[meta].background` lists them back to front with a scroll factor in sixteenths (engine: `RenderContext::with_background`). Layers are drawn **bottom-aligned to the screen bottom** and do not move vertically, so a layer's interesting part must sit high enough above the playfield ground (≈ 32–48 px at the bottom are usually hidden by ground tiles). Salzburg: sky generated (`art/generators/salzburg_sky.toml`), Untersberg, Festung and rooftops hand-drawn (`art/sprites/bg/`); factors 0 / 3 / 8 / 13, y = 0.

`bg_sky_salzburg` (factor 0) · `bg_untersberg` (far mountains) · `bg_festung` (Hohensalzburg on its hill, mid) · `bg_rooftops` (old-town roofs and domes, near).

## M2 additions: St. Gilgen, Brunnwinkl, route map

Specs in `game-design.md` → "Stage 2", "Stage 3", "Route map"; decisions D-041 … D-046. Same conventions as above. Colour notes are starting points for the pixel-artist; art-style.md and the art-director decide.

### Enemies (group `enemies`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `nordic_walker` | 16×24 | 8,24 | `walk` 4f/8t loop (poles planted alternately, 1 px body bob) · `squished` held. Functional outdoor jacket over Lederhosen/Dirndl, cap, two poles (`m d`, ≥ 3 px of pole visible in front – the readable prop) |
| `blasmusiker` | 24×24 | 12,24 | `idle` 2f/30t loop (sways) · `inhale` held (the telegraph: cheeks puffed, tuba bell raised – must read at 1× from 10 tiles) · `blow` held · `squished` held. Tracht (green `G` jacket, hat with a feather), brass tuba `Y o O` with a `w` glint on the bell (bell on his front side, mirror auto) |
| `note` | 8×8 | 4,8 | `fly` 2f/8t loop (slight wobble). An eighth note: head and stem `d`/`m`, `k` outline, 1 px `w` glint; never gold and round (not an Oblate) |
| `e_biker` | 32×24 | 16,24 | `idle` 2f/30t loop (foot down) · `telegraph` 2f/4t loop (bell glint + headlight blink, front wheel lifted 1–2 px) · `ride` 2f/4t loop (wheels, pedals) · `crash` held (bike on its side, rider sitting, stars or a little dust cloud) · `squished` held (rider sitting dazed beside the bike). Faces left, no mirror (like the Fiaker). Lycra top over Lederhosen, helmet, battery pack on the frame |
| `bienenschwarm` | 24×24 | 12,19 (bottom centre of the 14×14 hitbox, which is centred in the sprite) | `buzz` 2f/4t loop (loose cluster of 5–6 bees) · `waggle` 2f/4t loop (tighter cluster, bodies aligned; the engine also shakes it ±2 px). Bees `Y`/`o` with `l` stripes, wings `w`/`h`, `k` outline per bee; no `r` |

### Platforms (group `props`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `gondel` (Zwölferhorn cabin) | 32×52 | 16,52 | `idle` held (optional `sway` 2f/30t). Rows 0–27: hanger arm with the cable grip at the top centre (the engine's cable meets the sprite at (16,0)); rows 28–51: the 4-person cabin, yellow `Y o` with windows (`b`/`s`), a thin red-brown band (`R`, `r` only as a small accent, art-style §8), `k` outline (moving platform, §3). The standing surface is the cabin roof (row 28); hitbox = cabin 32×24 (`platforms` tuning). The cable itself is drawn by the engine (1 px, `d`) |

### Items and props (group `props`)

All are `prop` entities (bottom centre at the cell, optional `dx`, `layer`); collision stays with tiles (D-030/D-031). `PROP_SPRITES` in jnr-core must list the new names (test against this table).

| Sprite | Size | Origin | Animations / notes |
|---|---|---|---|
| `prellbock` (old Ischlerbahn buffer stop) | ≤ 32×24 | bottom centre | `idle` held — two rail stubs and a buffer beam, rust `O l` and iron `m d`; front layer |
| `talstation` (Zwölferhorn valley station) | ≤ 112×96 | bottom centre | `idle` held — `layer = "back"`; concrete-and-timber station hall with the cable wheel housing, yellow cabin colour as an accent |
| `seilbahn_stuetze` (cable-car pylon) | ≤ 32×128 | bottom centre | `idle` held — `layer = "back"`; lattice steel pylon `m d` behind the pylon platform |
| `mozartbrunnen` (young Mozart with violin on the fountain) | 48×72 | 24,72 (bottom centre) | `idle` 2f/12t loop (water from the birds) — front layer over the 5-wide basin and the 1-wide plinth tiles; bronze `O l` (+ `t` patina optional) |
| `mozarthaus` (former Pfleggericht, Mozart's mother's birthplace) | ≤ 128×112 | bottom centre | `idle` held — `layer = "back"`; whitewashed façade, hipped roof, a commemorative plaque (no text) |
| `musikpavillon` (bandstand) | ≤ 128×80 | bottom centre | `idle` held — `layer = "back"`; wooden posts, railing and the roof fringe around the `oneway_wood` floor and the solid `roof_shingle` roof; must not suggest a surface where there is none (D-038) |
| `muehlrad` (water wheel of the Kornmühle) | 48×48 | bottom centre | `turn` 4f/12t loop — front layer, wood `o O l`, water `t T` + `w` drops |
| `bienenhaus` (bee house) | ≤ 128×80 | bottom centre | `idle` held — `layer = "back"`; open wooden shed with rows of painted hive fronts (small folk-art panels, no text) |
| `futterschaelchen` (bee bonus marker) | 16×8 | bottom centre | `idle` held — a small glass dish (`i w`) on a blue card (`S b`); front layer. Never gold and round |
| `boathouse` (Brunnwinkl Bootshaus) | 96×80 | 48,80 (bottom centre) | `idle` held — `layer = "back"`; steep wooden-shingle roof, plank wall with a cross window, `o O l` stilts over the dark `d` slip with a white rowing boat hung on two `l` hoist slings; a stair tower and a woodpile on its left. Drawn = solid (D-038): placed at col 223 with `dx = 8`, bottom on the top of the jetty row, its outline matches the shore block 221–226: woodpile top 2 tiles, stair tower 4 tiles, roof ridge (4 tiles, `o` lip) 5 tiles = the launch. Collision stays with tiles (`invisible` under it); under it the level puts `jetty` (row 15: the lake surface 4 px below the deck line, as on the bathing jetty) and `jetty_post` (row 16) |

### Tiles (group `tiles`, 16×16, `tile_<name>`)

New tile names (engine: map each to its collision kind in `TileKind::from_tile_name`):

| Tile | Kind | Look |
|---|---|---|
| `wall_village_bg` | decorative (non-solid, like `railing_bg`) | whitewashed village house wall `w i` with green `G` shutters and windows, autotiled (16f `auto`); drawn full height from the street up, behind balconies; flatter than solid tiles (no light top lip) |
| `balcony` | one-way | wooden balcony with a flower box (geraniums: `p`/`R` with `g`; no `r` fill), autotiled left/right ends; top 5 px = walking surface, like `oneway_sign` |
| `facade_village` | solid | the same village wall as a solid block (Brunnwinkl houses, the mill), autotiled; distinct from `wall_village_bg` by a light top lip and darker edges (art-style §8) |
| `roof_shingle` | solid | wooden shingle roof `O l` (or `n R` terracotta for variety), autotiled left/right; light `o` lip on top |
| `jetty` | solid | wooden pier deck `o O l`, light plank lip |
| `jetty_post` | solid | wooden posts standing in water (water `T b` between the posts), placed under `jetty` |
| `wolfgangsee_water` | water (D-050) | lake surface, turquoise `t T` + `w s` sparkles, animated 4f/12t (water generator, art-style §6) |
| `wolfgangsee_water_deep` | water (D-050) | rows below the surface, `T b B` bands |
| `forest_path` | solid | earth path `O l` with a `y g` grass lip and roots, autotiled |
| `beute` | solid | bee hive box (`o O l` wood) with a small painted front panel; stackable (autotile vertical neighbours so a stack reads as boxes, not a wall) |
| `roof_bg` | decorative (non-solid, like `wall_village_bg`) | shaded terracotta Biberschwanz roof `n R` (+ `l` ridge/eave shadow) over walk-through `wall_village_bg` houses; no light lip, never the solid `roof_shingle` look. Autotiled (16f `auto`): left end open = left 45° slope, right end open = right slope (2 px `R` shaded verge), both closed = full roof, both open = small pyramid; top open = ridge cap, bottom open = eave; a slope end with roof above it becomes a straight edge. One row over a house = trapezoid; stack rows each one tile narrower per side (6 + 4 + 2) for a full gable |
| `chimney` | solid | stone chimney stack, 14 px fieldstone masonry `w i m` + `d` edge, full-width cap slab with a `w` lip over a dark `d` opening; no smoke (D-038). Autotiled vertically only (cap on top of a stack, base course at its bottom; left/right bits ignored), so a stack of 2 reads as one chimney |

Reused in M2: `ground`, `meadow`, `rock`, `stone`, `brick`, `oblaten_box`, `oblaten_box_empty`, `oneway_wood`, `invisible`.

### UI (group `ui`)

| Sprite | Size | Notes |
|---|---|---|
| `font` | 8×8 | append `é` (90th glyph, D-012 append rule; for „Café Zauner", D-043) |
| `map_salzkammergut` | 320×180 exact, origin 0,0 | route-map backdrop: stylised painted Wanderkarte (land `y g`, forest `G`, mountains `f F v` with `h` snow, lakes `t T` with `s` highlights, a wooden frame `o O l` allowed). No text, no route line (the engine draws it), calm bottom band y ≥ 136. Landmarks: tiny Festung (Salzburg), Fuschlsee, Zwölferhorn, Schafberg, Falkenstein cliff, the Wolfgangsee, Bad Ischl, a small cake at Café Zauner |
| `map_station` | 8×8, origin 4,4 | `todo` held (grey `m` dot, `k` outline) · `done` held (a small red-white pennant or pin, `r w`; UI may use `r`) · `next` 2f/15t loop (blinking ring in `Y`) |

The map's Resi is the normal `player` / `player_big` sprite (`idle`, `walk`).

### Backgrounds (group `bg`)

Same rules as Stage 1 (tileable, ≤ 640×180, bottom-aligned, art-style §7 colours per depth).

| Layer | Factor (1/16) | Stages | Content |
|---|---|---|---|
| `bg_sky_salzburg` (reused) | 0 | 2, 3 | the Stage 1 sky |
| `bg_schafberg` (far) | 3 | 2, 3 | the Schafberg ridge across the lake with its summit hotel as a tiny bump, `f F h` |
| `bg_wolfgangsee` (mid) | 8 | 2, 3 | the lake band and the far shore's wooded hills (St. Wolfgang as a small silhouette); distant water within the mid layer's colours (art-director: `v G` + a calm `s`/`f` water band, or move the water to the near layer) |
| `bg_stgilgen` (near) | 13 | 2 | village houses with wooden balconies, the parish church tower, the Zwölferhorn gondola line as a thin line going up-left |
| `bg_forest` (near) | 13 | 3 | shore forest (`G j g`) with gaps showing the lake (`t T`), a boathouse or two |

## M3 additions: Falkenstein, Hackl, Durchkriechstein

Specs in `game-design.md` → "Stage 4: Falkenstein" and "Power states"; decisions D-053 … D-058. Same conventions as above. Colour notes are starting points; art-style.md and the art-director decide.

### Player (group `player`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `player_hackl` (Resi, Hackl) | 16×32 exact | 8,32 | `idle` 2f/30t loop · `walk` 4f/8t loop · `run` 4f/5t loop · `skid` held · `jump` held · `fall` held · `die` held · `pole` 2f/8t loop (same names and poses as `player_big`) · **`throw`** 2f (frame 0 = arm back with the Hackl raised behind her head, 4 ticks; frame 1 = arm forward, empty hand, 6 ticks; the engine picks the frame for the 10-frame throw pose, on the ground and in the air) · **`crawl`** 2f/8t loop (see below) |
| `player`, `player_big` (existing) | as before | as before | add **`crawl`** 2f/8t loop |

- **Look of Hackl Resi**: the Zuckerschock Resi in a green loden jacket (Janker, `G` with `j` shadow, `g` light) over the red dirndl (skirt stays `r`), so the state reads at 1× by its dominant colour, like SMB's fire palette. A small Hackl (axe) hangs at her belt on the back side (blade `i`/`w` with a `d` edge, handle `O l`), visible in `idle`/`walk`/`run` and missing in `throw` frame 1. The apron stays blue `S` and the hair blond, so she's still Resi.
- **`crawl`** (all three player sprites): Resi on hands and knees, facing right, within the **bottom 14 rows** of the canvas (the tunnel is 16 px high) and the full 16 px width, head down. In `player_big`/`player_hackl` she is the big Resi squeezed flat (the legend joke "even a large person fits": a slightly wider, comic squash). Frame 1 moves the knees and hands. Drawn while the engine's crawl hitbox (12×12) moves through the tunnel. No mirror needed (she always crawls right), but `mirror = auto` is fine.
- The transformation freeze alternates `player_hackl` with `player` or `player_big` (engine, no extra frames). The stage intro and the route map draw `player_hackl` (`idle`, `walk`) when Resi carries the Hackl.

### Items (group `items`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `hackl` (power-up) | 16×16 | 8,16 | `idle` 2f/20t loop: a small woodcutter's axe standing upright, blade top-right, with a 1 px `w` glint that moves along the blade on frame 1. Blade `i w` + `d` edge, handle `O l`, `k` outline. Must read as a tool, not a weapon of war: short handle, rounded blade. Never gold and round |
| `hackl_thrown` | 12×12 | 6,10 (bottom centre of the 8×8 hitbox, centred in the sprite) | `spin` 4f/3t loop (blade up → right → down → left: four hand-drawn rotations, no engine rotation) · `stuck` held (blade buried in a wall to the right, the handle sticking out up-left; the engine mirrors it for left throws) |
| `stein_debris` | 8×8 | 4,8 | `spin` 2f/4t loop: limestone chip `i m` with `d` shadow (the `debris` brick piece's counterpart for shattered rocks) |

### Enemies (group `enemies`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `pilger` | 16×24 | 8,24 | `walk` 2f/12t loop (steady pilgrim pace, stick planted on frame 0) · `squished` held (sits down on his stone and rests, hat pushed back: a rest, not a squash). Grey-brown loden (`m`/`O`), green felt hat `G` with a small badge `Y`, walking stick `O l`, a backpack with a grey **penance stone** (`i m d`) strapped on top (the Steinetragen custom: the readable prop at 1×). Friendly face; pilgrims are funny, faith is not the joke: no rosary or cross props |
| `pilger_lead` (`lead = true`) | 16×32 | 8,32 | same anims as `pilger`; the walking stick is taller and carries a small red-white pennant (`r w`) above his hat (the group leader, like a tour guide's umbrella). Pennant ≤ 8 rows above the hat; the hitbox is the pilgrim's, the pennant is harmless decoration |
| `gams` | 24×20 | 12,20 | `idle` 2f/30t loop (chewing, ear flick) · `crouch` held (the telegraph: head down, horns forward, legs bent – must read at 1× from 6 tiles) · `hop` held (legs tucked, body stretched) · `squished` held (sits dazed, a little star or two over the horns). Summer coat `O` with `l` back stripe, the black-and-white face mask `k`/`w`, short hooked horns `k`, light belly `c`. Faces right, `mirror = auto` |
| `stein` (falling rock and the rock in its niche) | 16×16 | 8,14 (bottom centre of the 12×12 hitbox) | `rest` held (sits in the niche) · `wobble` 2f/4t loop (the telegraph: tilts 1 px left/right) · `fall` 2f/6t loop (tumbling). Pale limestone `i` with `m` facets and `d` shadow, `k` outline; clearly a rock, never round-gold (not an Oblate, not a Mozartkugel) |
| `kiesel` (telegraph pebbles) | 4×4 | 2,4 | `fall` 2f/4t loop: 1–2 px grey grains `m i`; drawn by the renderer as a trickle down the rock's column during the telegraph (renderer only, no collision) |

### Tiles (group `tiles`, 16×16, `tile_<name>`)

| Tile | Kind | Look |
|---|---|---|
| `kalkfels` | solid | the Falkensteinwand's pale limestone: `i` faces, `m` cracks, `d` shadowed undersides, a light `w` lip on top, occasional `G`/`g` grass tufts on top edges. Autotiled (16f `auto`): undersides (bottom open) show a dark `d` overhang edge, because the Steinschlag overhang is mostly underside. The mountain mass under every path is this tile. As built (M3, round 2): one quiet stratum per tile (a wavy `m` bedding line in rows 9–10 that wraps left/right and one short `m` joint in rows 11–13; no `w` in the body); open top = `w` lip + one `y g G` tuft; open bottom = 3 px ragged `d` edge with 1 px notches; open left `m`, open right `d`. Underside frames use 4 stone shades (`w i m d`), as the catalog colours ask |
| `kalkfels_bg` | decorative (non-solid, like `wall_village_bg`) | the cliff face *behind* the playfield: the shaft's back wall, the Einsiedelei cave interior. Darker and flatter than `kalkfels` (`m d` with `F` haze, no light lip, no top grass), so it never reads as a surface (D-038). Autotiled. As built (M3): the `kalkfels` stratum recoloured (`m` body, `d` bedding line and joint); edges are recessed, never lit: open top = 2 px ragged `d` shadow whose row 0 has 1–2 px transparent notches (runs ≤ 5 px) (cast by the rock, a plank or the cave ceiling above), open left = ragged `d` shadow column, open right/bottom flat |
| `steig` | one-way | the wall's wooden stairway: a plank (top 5 px = walking surface, `o O l`) on two black iron brackets (`d`) bolted into the rock below it, a thin steel-cable handrail (`m`, 1 px) 10 px above on short posts. Autotiled left/right ends (the bracket on the end tiles). The handrail must not look walkable: 1 px, dashed with posts, no lip. **As built (M3)**: `k`-outlined plank in rows 0–4, iron knee brackets (`d` post + diagonal brace, `m` bolts, `k` outline) under the end tiles, one Y-bracket under a single tile; transparent below (for the Kreuzweg's open air). **No handrail**: 10 px above the plank lies outside the 16×16 cell (the cell above is air or another tile), so it can't be drawn by the tile |
| `steig_wall` (new, M3) | one-way | the same plank drawn over the `kalkfels_bg` back wall, with a 2 px `d` cast shadow on the wall under it: for the shaft, where a plank cell replaces a back-wall cell (one tile per cell, as `balcony` over `wall_village_bg`). Autotiled left/right ends like `steig`. Engine: map it to one-way like `steig` |
| `netz` | one-way | rockfall net: a steel-cable net `m d` stretched flat between posts (diamond mesh, 1 px), a `d` edge rope with a light `i` highlight on top as the walking surface (top 4 px). Autotiled left/right ends (an end tile has a post going down to the ground, decoration only, drawn in the tile). As built (M3, round 2): rows 0–3 `k`/`i`/`d`/`k` rope over the full tile width, a 1 px `d` diamond mesh (period 8) in rows 4–10 back to a `d` rope in row 11, transparent between; end tiles have a 4 px `k i m k` post in columns 6–9 with a `d` clamp every 8 px over the full tile height (the mesh stops at the post, the rope runs on to a `k` end cap); `netz_post` continues it down to the ground |
| `netz_post` (new, M3 round 2) | decorative (non-solid, like `jetty_post`) | the `netz` end post continued down to the ground: the same 4 px `k i m k` post in columns 6–9 with a `d` clamp every 8 px; vertical autotile (16f `auto`, left/right bits ignored), an open bottom gets a `k`-outlined `i m d` concrete footing. Placed in the column under each `netz` end tile |

Reused in M3: `forest_path` (Kreuzweg steps), `stone` (Fürberg garden wall), `jetty`, `jetty_post`, `wolfgangsee_water`, `meadow` (goal meadow), `brick`, `oblaten_box`, `oblaten_box_empty`, `invisible` (under the Durchkriechstein prop).

### Props (group `props`)

All are `prop` entities (bottom centre at the cell, optional `dx`, `layer`); collision stays with tiles (D-030/D-031). `PROP_SPRITES` in jnr-core must list the new names.

| Sprite | Size | Origin | Animations / notes |
|---|---|---|---|
| `gasthof_fuerberg` | ≤ 128×96 | bottom centre | `idle` held — `layer = "back"`; lakeside inn at the Fürberg landing: whitewashed ground floor, dark wooden upper floor with a balcony, shingle roof, a beer-garden umbrella or two. No text. As built: 128×96, origin 64,96; house on x 0–100 (wide eaves), beer garden with two green umbrellas on x 88–127 |
| `bildstock` (wayside shrine on the Kreuzweg) | 16×40 | bottom centre | `idle` held — `layer = "back"`; a slim stone pillar (`i m`) with a small roofed niche on top holding a tiny painted panel (abstract colour, no figure detail needed). Respectful decoration. As built: 16×40, origin 8,40 |
| `steinhaufen` (pilgrims' stone pile) | 32×16 | bottom centre | `idle` held — front layer; a heap of grey stones of mixed sizes (`i m d`), on the Kreuzweg start and at the church. As built: 32×16, origin 16,16, stones mostly `m` with `i` tops, separated and outlined in `d` (no `k`: scenery, not a hazard rock) |
| `falkensteinkirche` | ≤ 96×112 | bottom centre | `idle` held — `layer = "back"`; the small white church built into the cliff: whitewashed walls `w i`, a shingle roof `O l`, a small onion or pointed ridge turret with a bell, an arched door, the grey rock face (`m d`) wrapping round its back and over its roof (it stands in the wall's cave). No text. As built: 96×112, origin 48,112; rock fills the top (rows 0–~14) and the right side (x ≥ 84) down to the ground, the cave's open left side below row ~52 is transparent; façade x 18–62, nave side to x 76. Meant to sit under a `kalkfels` overhang whose underside meets the prop's top row |
| `brunnkapelle` (spring chapel) | ≤ 48×48 | bottom centre | `idle` 2f/12t loop — `layer = "back"`; a tiny chapel over the spring, with a trickle of water `t s` from a pipe into a stone trough (animated). As built: 48×44, origin 24,44 |
| `hackelwurfkapelle` | ≤ 48×64 | bottom centre | `idle` held — `layer = "back"`; an open wayside chapel with a stone figure of St. Wolfgang (bishop's mitre and crozier as a simple silhouette, `i m`) with one arm raised, throwing an axe. His head is where the level places the `taube_sitting` (the level-designer aligns them with `dx`). As built: 48×64, origin 24,64; the mitre tip (the pigeon's seat) is sprite pixel (22, 24): 2 px left of the origin, 40 px above the ground. The Hackl is a 1 px `m` handle diagonal (5 px) from the raised hand to a 4×4 `i` blade (`m` lower-right edge, `w` glint) at x 34–37, y 21–24, clear of the pigeon area |
| `durchkriechstein` (the Schliefstein boulder) | 64×48 exact | 32,48 (bottom centre) | `idle` held — `layer = "back"`; a rounded limestone boulder 4 tiles wide and 3 tall, sitting on the plateau, with a **low, dark tunnel** through its bottom row (rows 32–47 of the sprite, `d`/`k` interior, worn smooth light edges `i` at the mouth, the far opening visible as a lighter patch). Drawn = solid (D-038): the level puts `invisible` solid tiles on the boulder's top two rows (sprite rows 0–31) and leaves the tunnel row empty, so the top edge matches the boulder's drawn top within 2 px. Resi is drawn over it while crawling (player layer). As built (round 2): top row 0 (corners round off by ≤ 2 px over x 0–5 and 58–63); a curved `m` shadow terminator in the right quarter and a curved `m` underside; the tunnel ceiling is an irregular arch (lowest rock rows 31–33, radius-3 corners at the mouths, a dip near the middle, ≥ 14 clear rows above the floor); the boulder rests on two rounded 3–5 px feet at the mouths (Resi is drawn over them) |

### Backgrounds (group `bg`)

Same size rules as before (tileable, ≤ 640×180). New: layers may move vertically with `factor_y` (D-054). A layer with `factor_y > 0` sinks as the camera climbs, so its **top** part matters at the summit and its bottom part at the start. The sky must stay opaque and full height behind them.

| Layer | Factor (1/16) | `factor_y` | Content |
|---|---|---|---|
| `bg_sky_salzburg` (reused) | 0 | 0 | the Stage 1 sky |
| `bg_wolfgangsee` (reused) | 8 | 3 | the lake and the far (south) shore. Climbing the 29 tiles sinks it by ~87 px, so at the church the lake is a band low on the screen: "you're up high now" |
| `bg_wolfgangsee_tief` (mid, new) | 8 | 3 | deep variant of `bg_wolfgangsee` for Stage 4 only (Stages 2/3 keep the original). 640×116, origin 0,0: rows 0–51 are `bg_wolfgangsee` unchanged, rows 52–115 continue the lake (`s` with sparse `f` streaks, slightly longer and further apart towards the bottom; mid-layer water rule `v G` + `s`/`f`; no dither, no shore, no `k`), so mid-climb shots no longer show a pale-sky gap under the lake band |
| `bg_falkenstein` (near, new) | 13 | 8 | pines (`G j`), larches (`g y`) and pale limestone outcrops (`i m`) climbing the slope, with a rockfall net strung between two posts in one place (decoration). At the summit it has sunk fully out of view (~232 px): above the treeline only the lake and the sky remain. As built: 640×180 (no need to exceed the screen: layers only move down, and 180 < 232 px so it is gone at the summit); larches are `g G` (no `y`, not a near-layer colour); outcrops `m` with a 4 px `i` rim along the upper-left silhouette (≈ 14 % of their pixels; `i` approved by the art-director for this layer), no strata; the decorative net was dropped (round 2). Opaque everywhere from row 84 down, so the mid layer's lake band (`y = 40`, sinking only 3/16) never shows a sky gap under it while the near layer sinks faster |

## M4 additions: St. Wolfgang

Specs in `game-design.md` → "Stage 5: St. Wolfgang"; decisions D-066 … D-072. Same conventions as above. Colour notes are starting points; art-style.md and the art-director decide. No new player sprites: Resi's three forms are complete. The intro and the map draw `player_hackl` when she carries the Hackl in.

### Enemies (group `enemies`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `operettensaenger` (the singing head waiter) | 16×24 | 8,24 | `idle` 2f/30t loop (sways, hand on heart) · `inhale` held (the telegraph: chest out, arms spread wide, mouth open, head back; must read at 1× from 10 tiles and be clearly different from `idle`) · `sing` 2f/6t loop (arms up, mouth wide, a 1 px tremble) · `squished` held (sits, takes a bow with the napkin). Dark waiter's jacket `d` with a `w` shirt front and a white napkin over the forearm, a small `R` bow tie, slicked hair. Faces right, `mirror = auto`. Not a Blasmusiker: no instrument, no hat, no green Tracht |
| `hohes_c` (the high-C sound column) | 16×8 | 8,8 | `ring` 2f/4t loop: one flat sound ring (an ellipse 12 px wide, 3–4 px high), `w` with `i` shading and a `k` outline on its lower half so it reads against light sky and white façades. The renderer stacks it every 8 px from the singer's hitbox top up to the column's current height and animates alternate rings half a period apart, so the column shimmers upwards. Never `Y`/`o` (not gold), no musical note shapes (those are the Blasmusiker's) |
| `souvenirstand` (the vendor behind the counter) | 16×16 | 8,16 | `idle` 2f/30t loop (leans on the counter) · `telegraph` 2f/8t loop (holds a snow globe up high with a `w` glint, the other hand on the globe's base; frame 0 = up, frame 1 = glint). Upper body only: the renderer draws it with its origin on the **top of the counter's first tile** (one column right of the spawner cell, on the counter row's top edge), so the counter tile in front hides nothing and the vendor stands behind it. Apron `S` or `G` over a `c` blouse, friendly face. Faces left (towards the spawner cell and the player); no mirror needed |
| `schneekugel` (rolling snow globe) | 12×12 | 6,11 (bottom centre of the 10×10 hitbox, centred) | `roll` 4f/6t loop: a glass dome (`h` with `s` reflections, 1 px `w` highlight that stays top-left while the snow inside swirls), a tiny white horse (`w` + `i`) inside, snow flecks `w` that move between frames, a wooden base `O l` that rotates with the roll. `k` outline. Never gold and round: the dome is pale blue-white, the base brown |
| `schneekugel_scherben` (shatter) | 8×8 | 4,8 | `spin` 2f/4t loop: a glass shard `h w` with `i` edge and a fleck of snow; the renderer spawns 4 (like brick debris) when a globe shatters on a wall, is stomped or hit by an axe |
| `reisebus` (tour coach) | 64×32 | 32,32 | `idle` 2f/30t loop (engine running: 1 px body shake, exhaust puff at the rear right) · `telegraph` 2f/6t loop (hazard lights `Y`/`o` blink at both ends, door closed) · `drive` 2f/4t loop (wheels turn, slight bounce) · `fotostopp` 2f/16t loop (stopped; frame 1 has 2–3 camera flashes `w` in different windows; no one gets out). A modern white coach `w i` with a blue `S b` swoosh, dark tinted windows `d` with faint heads `E`, two `k d` wheels with `i` hubs, headlights on the left (front). Faces left, **no mirror** (like the Fiaker and the E-Biker). Not a Postbus: never yellow, no Postbus horn emblem. The roof line is flat over the whole length (the roof bounce covers all of it) |

### Platforms (group `props`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `schafbergbahn` (the steam cog train) | 64×40 | 32,40 (bottom centre of the 64×32 body) | `idle` held (standing) · `ride` 2f/8t loop (connecting rods move, the loco shakes 1 px). The loco on the **left** (downhill end, it pushes uphill like on the real line): black boiler `k d` with `i` bands, `R` wheels, a short chimney. The coach on the right: wooden body in wine `R` with a cream `c` window band, `o`/`O` frame accents. As with real rack trains, the bodies are built level on the slope: draw the body **horizontal** (the hitbox is a horizontal box) and only the undercarriage hints at the incline. The cog wheel under the loco may tilt 1–2 px. **The walking surface is one flat line**: sprite row 8, the top of the body, across the full 64 px (loco cab roof and coach roof). Only the chimney top and the steam dome may rise above it (rows 0–7, ≤ 6 px wide, at the loco's left end). Nothing else that looks walkable goes above row 8 (D-038). **As built (M4)**: hitbox 0,8,64,32; roof row 8 `k`, row 9 `i` lip, row 10 `m`, row 11 `k` over the full 64 px (the loco gets a full-length canopy so the line is unbroken); only the 6 px funnel (x 0–5, rows 0–7) rises above it. Loco x 0–24: level boiler `k d` + `m` top light, `i` bands, dome, `G j` cab with the driver, red `R` frame and wheels on the incline, `i` coupling rod, `d m` cog wheel. Coach x 24–63: `R` body, window frames and band in `o` (not `c`: a 36 px `c` band would be a fill, art-style §8), `b s` windows with `B` passengers, `l` panel joints. The undercarriage follows the full 1:4 track (wheel bottoms on the rail), so the coach's underside steps up to the right: the hitbox's bottom-right corner is open (at x 63 the coach ends at row 25). `ride` moves the rod ±1 px and drops the boiler, dome and funnel 1 px in frame 1 (the roof never moves) |
| `dampf` (steam puff) | 8×8 | 4,8 | `puff` 3f/8t (plays once): a small cloud `w h` growing and thinning out. The renderer emits one every 20 frames from the chimney top while the train moves; it drifts up 1 px/frame and is removed after the animation. Never `i`/`m` grey (not a rock, not the Steinschlag's pebbles) |

The **track** is drawn by the renderer (like the Gondel cable): two 1 px rails (`d`) 3 px apart with the rack (`m`, dashed) between them, along the body's bottom line from end to end, plus one tile beyond each end, at the 1:4 slope (a 4 px run per 1 px rise: clean pixel steps). Sleepers (`O`) every 8 px. The rails run on a 1 px `d` girder line along the whole track, with no posts. **As built (M4)** — the track contract the sprite is drawn for: with the body's left edge at world x `L` and its bottom edge at `B` (the origin's y) **at the spawn position**, the upper rail's row under world column `X` is `Y = B + 1 − floor((X − L) / 4)` (sprite-local: row `41 − floor(x / 4)`; the wheels stand on it). Per column, with `u = X − L`: `Y` = `d` (upper rail), `Y+1` = `m` rack tooth on even `u` (odd `u` empty), `Y+2` = `m` rack bar, `Y+3` = `d` (lower rail); sleeper columns (`u mod 8` ∈ {0, 1}) are `O` over `Y+1 … Y+4` (the rails stay on top); a `d` girder at `Y+5` under the **whole** track (art-director, M4 round 1: not only over the gorge, so the track never floats over the step tiles). The top rail stays `d`, with no light lip (the track must not read as a walkable surface). Drawn behind the train and behind tiles. **Fallback** (D-067, if the drawn track is dropped): the decorative tiles `zahnrad_gleis_0` … `zahnrad_gleis_3` (see Tiles).

### Tiles (group `tiles`, 16×16, `tile_<name>`)

| Tile | Kind | Look |
|---|---|---|
| `standl` | solid | the souvenir stand's counter, 1 tile high: a wooden counter `o O l` with a light `o` lip on top (walkable surface), its front showing tiny wares: snow globes `h w`, Christmas stars `Y`, mini cowbells `i`, a Mozartkugel `r`-foil dot, Hackl keychains. Autotiled left/right ends (end posts). Must read as a counter, not as a brick or a box. As built (M4): rows 0–2 `o O l` countertop (the `o` lip is the surface), a dark `l` display with a snow globe (`h w s` on an `o O` base), a star (`Y o`) and a cowbell (`w i m`), an `O o` shelf board, then a rail with a Mozartkugel (`i r R m`), a Hackl keychain (`i m d` + `o`) and a brass bell (`Y o m`), `O l` base. No `k` (static solid tile). Open left: lit `o O` end post; open right: `O l` post; only the left/right bits change the look |
| `standl_dach` | one-way | the stand's striped awning: red-and-white canvas `R w` (wine, not Resi's `r`) in vertical stripes with a scalloped lower edge, a thin `o` rod as the top 5 px walking surface. Autotiled left/right ends (an end tile shows the awning's side flap). Over the counter with 1 tile of air between them (the vendor stands in that gap). As built (M4): rows 0–4 `k o O l k` rod; canvas rows 5–12 in 4 px stripes (`w` with an `i` shadow row under the rod, `R`); rows 13–14 one `k`-outlined tongue per stripe (`i`/`R` tips), transparent below. Open ends: `k` edge + a 2 px `R` side flap |
| `wartehaus_dach` | one-way | the bus shelter's glass roof: a thin `d` steel frame with `h s` glass, top 4 px = the frame (walking surface), the glass below it faintly transparent-looking (no actual transparency needed). Autotiled left/right ends. As built (M4): rows 0–3 `k i d k` steel frame (the surface), rows 4–8 `h` glass with two 2 px `s` reflection streaks per tile, row 9 `d` rail, row 10 `k`, transparent below. Open ends: `k` end + a `d` frame post |
| `strasse` | solid | road asphalt `m d` with a light `i` kerb lip on top and a dashed `w` lane marking low in the tile (every second tile has a dash). Autotiled left/right (kerb ends). Used for the bus park; the cobbled `ground` stays the village street. As built (M4): `m` asphalt with sparse 2 px `d` grain; open top = `i i d` kerb (the lip) and a `w`/`i` dash (x 4–11, rows 11–12): the autotile can't see column parity, so every surface tile carries half a dash (8 px on, 8 off) and the rows below the surface have none. Open left: lit `i`/`m` kerb end; open right: `d`; open bottom: `d` edge |
| `tunnel_bg` | decorative (non-solid, like `wall_village_bg`) | the inside of the bypass tunnel: dark `d k` with a faint `m` arch line and one small `Y` lamp every 4 tiles (by column parity in the autotile frame, or a 2-frame variant). Darker and flatter than any solid tile, no lip (D-038). As built (M4): flat `d` (no `k` fill: art-style §3), seamless left/right. A cell with an open top and tunnel below (the ceiling row) shows an `m` lining line 3 px down with a segment rib; every other cell shows an `m` walkway line 4 px above the road. **The lamp is a separate tile `tunnel_bg_lamp`** (below): the renderer has no column parity, and a tunnel cell under the lamp (open above and below) keeps the lower look, so no autotile family entry is needed |
| `tunnel_bg_lamp` (new, M4) | decorative (non-solid, like `tunnel_bg`) | the `tunnel_bg` ceiling cell plus a small `m` lamp housing with a `Y` light and an `o` glow. One look, not autotiled. The level puts it in the tunnel's ceiling row (row 13) every 4 tiles instead of `tunnel_bg`. Engine: map it to `Empty` like `tunnel_bg` |
| `terrasse_post_l` (new, M4) | decorative (non-solid, like `netz_post`; engine kind `Empty`) | the Kaiserterrasse's left pile: the `oneway_wood` left end leg `k O l k` (outline, lit left, shadow right) continued down in columns 1–4, plain wood with no joints, the rest transparent. Vertical autotile (16f `auto`, only bits 1/4 matter, left/right ignored); an open bottom gets a `k`-outlined `i m d` stone footing in rows 12–15 (columns 0–6). Placed in the column under the terrace's left end tile at the Weißes Rössl |
| `terrasse_post_r` (new, M4) | decorative (non-solid, like `netz_post`; engine kind `Empty`) | the same pile under the terrace's right end: `k O l k` in columns 11–14 (the `oneway_wood` right end leg), footing in columns 9–15. Drawn by hand, not mirrored, so the `O`/`l` light stays lit-left |
| `zahnrad_gleis_0` … `_3` (**fallback only**, D-067) | decorative (non-solid) | one 16 px segment of the 1:4 rack track, rising 4 px across the tile, at vertical offset 0/4/8/12 px (the level-designer lays them in order along the track). Same rail/rack/sleeper colours as the drawn track |

Reused in M4: `ground`, `meadow`, `rock`, `stone`, `kalkfels` (the church's rock terrace), `balcony`, `oneway_wood` (the Kaiserterrasse), `chimney`, `jetty`, `jetty_post`, `wolfgangsee_water`, `wolfgangsee_water_deep`, `brick`, `oblaten_box`, `oblaten_box_empty`, `invisible` (under the Pilgerbrunnen canopy and the Schiffsstation).

### Props (group `props`, M4)

All are `prop` entities (bottom centre at the cell, optional `dx`, `layer`); collision stays with tiles (D-030/D-031). `PROP_SPRITES` in jnr-core and the catalog test in `xtask/src/levels.rs` must include this table (engine task; the test reads the tables by heading).

| Sprite | Size | Origin | Animations / notes |
|---|---|---|---|
| `talstation_schafberg` (valley station) | ≤ 128×96 | bottom centre | `idle` held — `layer = "back"`; the station hall: a long timber-and-plaster building with a shallow roof, a platform canopy on wooden posts, a station clock (no text), a small water tower for the steam locos. Behind the platform; must not suggest a surface above the platform's `stone` (D-038). As built: 128×88, origin 64,88. Plaster `w i` ground floor (door, 4 windows) and timber `o O l` upper storey under a shallow `m d` roof with a white clock gable (x 31–49); a lean-to platform canopy (x 82–117, dark `d l`, sloping, no lip) on two posts; the water tower (x 116–127, `o O l` staves, `d` hoops, `m d` cone). Placed at col 46 (x 0 = col 42 + 8 px): the platform's `stone` (45–51, 2 up) covers x ≥ 40 below y 56, the step (col 44) x 24–39 below y 72 |
| `haltestelle_alm` (the halt on the Alm) | ≤ 48×48 | bottom centre | `idle` held — `layer = "back"`; a small wooden shelter with a bench and a halt sign without text, a few flowers. As built: 48×40, origin 24,40; shelter x 1–38 (`o O l`, dark roof top), halt sign `w`/`G`/`i` on an `m d` post at x 37–46, flowers `p Y w` on `g G`. Place at col 83, row 6 (on the platform's row-7 top) |
| `tunnelportal` (the bypass tunnel's portal) | ≤ 64×64 | bottom centre | `idle` held — `layer = "back"`; a concrete portal face (`i m` with `d` joints) set into the Alm's slope around the 2-tile tunnel mouth (rows 13–14), meadow `g` hanging over its top edge. The mouth itself stays transparent (the `tunnel_bg` tiles show through). As built: 64×48, origin 32,48; place at col 105, row 14, `dx = 8` (left edge = col 104, rows 12–14). x 0–15 is the mouth (transparent below y 16, pointed top) under the meadow and `tunnel_bg` tiles; the arch ring (`d` reveal + `i` voussoirs with `m` joints, x 16–21: the only light pixels) and the wing wall show in cols 105–107: `m` face with `d` joints and a 1 px `d` coping, its top 10 px below the terrace top at x 16 falling to the road at x 63, no grass and no light top (art-director round 1: it must never read as a ramp, D-038); a `g G` tuft hangs from the terrace lip at x 16–21, rows 0–3; `m d` footing |
| `wartehaus` (bus shelter posts and bench) | 48×48 | bottom centre | `idle` held — `layer = "back"`; two thin `d` posts, a bench, a timetable case (no text) under the `wartehaus_dach` tiles. Decoration only: the bench is not a platform, so it is drawn low and dark (D-038). As built: 48×48, origin 24,48 (place at col 126, row 14: cols 125–127); posts at x 2–3 and 44–45, faint glass back panel `s` with `h` streaks and `m` rails, timetable case `d`/`i`/`m` at x 32–41, a low `d m` bench, `m d` plinth. Rows 0–15 lie behind the roof tiles |
| `pilgerbrunnen` (the 1515 pilgrims' fountain) | 48×72 | 24,72 (bottom centre) | `idle` 2f/12t loop (water jets into the basin) — front layer over the 5-wide basin tiles and the 1-wide column; a Gothic canopy on slim columns (`i m d`) over a bronze basin (`O l`, + `t` patina), a pointed canopy roof (`d m`) with a finial. Drawn = solid (D-038): the canopy roof's top edge is exactly 4 tiles above the street over the 3 `invisible` tiles at 164–166 (the level aligns it with `dx`). The finial tip is the `taube_sitting`'s seat (state the pixel in the as-built note, as for the `hackelwurfkapelle`). As built: 48×72, origin 24,72, `k` outline (front layer); place at col 165, row 14, `dx = 0`. The canopy's `k` top edge is row 8 (64 px up) over x 1–46, the `i` lip row 9 over x 2–45, only x 0/47 chamfered (they start at row 9); two pointed Gothic arches either side of the central pier (x 17–30, over the stone column); bronze bowl `o O l` + `t` at rows 34–39; the basin's `k` top edge is row 56 (= the basin tiles' top). Water jets `t s w` (drawn after the outline, no `k`) alternate in the 2 frames. **Finial tip (the pigeon's seat): pixels (23–24, 0)**, 0–1 px left of the origin, 72 px above the street = 8 px above the canopy top (row 10 + 8 px) |
| `wallfahrtskirche` (the pilgrimage church) | ≤ 160×144 | bottom centre | `idle` held — `layer = "back"`; the late-Gothic church on its rock terrace above the lake: whitewashed walls `w i`, a steep roof `d m`, tall pointed windows, the tower (check its silhouette against a reference photo), a gate to the arcaded courtyard. Respectful decoration: no figures, no text. Its base sits on the terrace's `kalkfels` top (row 11), not on the Markt. As built: 160×84, origin 80,84 (kept low on purpose: at the side camera's rest the terrace top is 84 px below the screen top, so the whole silhouette with the onion stays on screen); place at col 195, row 10, `dx = 8` (cols 191–200). Nave x 2–99 with five lancets and buttresses, a hipped `m d` slate roof (lit `i` ridge), the square tower x 100–126 (belfry louvres, clock face `i w d` without numerals, slit window) under a bulbous onion helm (`i m d`, silver `i` knob, `m` cross), courtyard wall x 127–159 with a pointed gate and two arcade arches under a lean-to roof (`d` top falling from row 58 to row 60, `m` below); the nave ridge is `m` with a `d` right end, `i` only on the left hip |
| `weisses_roessl` (Hotel „Im Weißen Rößl") | ≤ 192×128 | bottom centre | `idle` held — `layer = "back"`; the white hotel façade by the lake: white walls `w i`, green shutters `G`, dark wooden balconies (their railings line up with the level's `balcony` tiles; the walking surface is the tile, not the art), a steep roof with dormers. No lettering: the name is carried by the horse sign and the text signs. Drawn behind the `balcony` and `oneway_wood` tiles. As built: 192×128, origin 96,128; place at col 222, row 14, `dx = 8` (cols 217–228; the hotel can't span 213–241 within the size limit, so it sits behind B1's right part, the door and B2). Left half: steep `m d` roof with a hipped end, a lit `i` ridge and two dormers; right half: a wall dormer up to the top under a deep `l` eave. French doors behind B1 (x 0–63, B1 = y 64–79) and behind B2 (x 96–191, B2 = y 16–31), `G`-shuttered windows with `p g` geraniums, the arched entrance at x 143–160 (top y 95 = 33 px up), and the signage art: abstract `d` script marks painted straight on the `w` wall (x 69–137, y 71–76), no letters, no banner frame. Left roof row 0: `d` over x 13–94, `i` only on the hip; the string course runs at y 84, with a gap at x 134–169 behind the roessl_pferd. No balconies in the art (D-038: the tiles carry them) |
| `roessl_pferd` (the white horse sign) | 32×24 | bottom centre | `idle` held — front layer; a white horse `w i` with a `k` outline on a wrought-iron bracket (`d`), rearing slightly, over the hotel door. The stage's emblem (the snow globes contain it too). As built: 32×24, origin 16,24; a rearing horse facing left (`w` with `i` shading on its right/under edges, `k` outline): two pointed ears, a 1 px `i` mane along the neck crest, a 3 px muzzle (rows 3–5) with a `k` mouth notch; raised forelegs as two 2 px strokes (upper: forearm rows 9–10, cannon x 3–4 rows 11–12; lower: rows 14–15, cannon x 7–8 rows 16–17), each ending in a `k` hoof, with open background between the legs and the chest; on a short `m d` plinth whose `d` wrought-iron arm with a 1 px scroll runs from its back end to the sprite's right edge (the wall). Place at col 226, row 12 (bottom 32 px up, just over the door's arch) |
| `schiffsstation` (boat landing hall) | 96×96 | 48,96 (bottom centre) | `idle` held — `layer = "back"`; the wooden landing hall with a ticket window, a timetable board (no text), a life ring `r w` (accent), a flagpole. Drawn = solid (D-038): its outline matches the `invisible` collision profile of the block at 280–285 (the level gives the as-built profile), its roof ridge = the launch 6 tiles above the meadow. As built: 96×96, origin 48,96; place at col 282, row 15, `dx = 8` (cols 280–285, bottom on the row-16 top). **Profile: the full box**, `invisible` at cols 280–285 × rows 10–15; the flat copper roof (`t` lip rows 0–1, `T` with `t` seam ribs, `d l` eave) is the launch. Wooden walls `o O l`, ticket window, timetable board (no text), life ring `r w` (`R i` rope), doors with `b` glass, flagpole with a `R w R` flag inside the box, `m i d` plinth (row 15) |

### Backgrounds (group `bg`)

Same rules as Stages 1–3 (tileable, ≤ 640×180, bottom-aligned, `factor_y` 0 in this side level).

| Layer | Factor (1/16) | Content |
|---|---|---|
| `bg_sky_salzburg` (reused) | 0 | the Stage 1 sky |
| `bg_schafberg_sued` (far, new) | 3 | the Schafberg seen from St. Wolfgang: much closer and steeper than `bg_schafberg` (Stages 2–3 see it across the lake). A steep south face with rock bands `F f` and `h` snow patches only at the top. The summit with the small hotel as a bump, and the rack line as a thin diagonal `F` line climbing it (the train's real route; no train). Far-layer colours only. As built: 640×150; summit at x 362 (148 px up) with the hotel bump (`h` with an `F` roof), lit `f` west flank (x 200–361 all `f`: one light/shadow boundary, running down-right from the summit), shadowed `F` east face, ~70 short `F`/`f` rock-band streaks across the face, `h` snow patches only within ~30 px of the summit, the rack line a 1 px `F` diagonal (2:1 steps) from x 68 at the bottom up to the summit |
| `bg_stwolfgang` (near, new) | 13 | village houses climbing the slope behind the street: white walls, wooden balconies with geraniums, steep roofs (`n R`), one or two big gabled inns, a few `G j` trees. Opaque from its lowest roofline down. As built: 640×120; a `g` meadow slope with `G` hedgerows, a back tier of small gable-fronted houses with spruces/round trees `G j`, a front tier of 19 houses (two wide inns) along the bottom: walls `c p o` (white walls are `c`: the near-layer palette has no `w`, art-style §7), `o`/`E` shaded right edges, `o` wooden gables with `n` boards on most, `n` balcony rails with `p` geranium dots, roofs `n`/`R` (left/right slope, every apex `n`, no `R` finial pixels), `T` windows 2×3, `d` gable openings 2×3. Opaque in the bottom 30 rows (`G` fill behind the houses) |

## M5 additions: Wolfgangsee

Specs in `game-design.md` → "Stage 6: Wolfgangsee" (incl. "Swimming"); decisions D-074 … D-081. Same conventions as above. Colour notes are starting points; art-style.md and the art-director decide. Vertical layout to draw for: decks (quays, piers, boat decks) on row 11 (their top is the walking line), the water line = the top of row 12, 4 tiles of water, the lake bed on row 16. At the side camera's rest the water line is at screen y 100.

### Player (group `player`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `player`, `player_big`, `player_hackl` (existing) | as before | as before | add **`swim`** 4f/8t loop: breaststroke facing right, the body near horizontal-ish but upright enough to read as Resi (head up, arms sweeping out on frames 1–2, legs kicking on 2–3). Hair and dirndl colours as usual. At the surface the engine places her with the water line 8 px below her hitbox top, so the face, hair and shoulders (the top ~10 rows of the canvas) are what shows above the waterline in Normal and big form: keep the face in those rows. The engine restarts the anim at frame 0 on every stroke. No separate surface/underwater frames: the same anim is used under water |
| `blase` (bubble, optional) | 4×4 | 2,4 | `rise` 2f/8t loop: a 2–3 px ring `w h` with a 1 px `w` highlight. The renderer lets one rise from her mouth every 40 frames while her chest is under water, 0.5 px/frame up to the water line, then removes it (renderer only) |

### Enemies (group `enemies`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `schwan` (territorial mute swan) | 24×24 | 12,20 (bottom centre of the 16×12 hitbox, which floats with its bottom 4 px under the water line: draw the waterline across sprite row 16) | `idle` 2f/30t loop (floats, neck in a calm S-curve, slow head turn) · `telegraph` 2f/6t loop (the hiss: wings arched up over the back like sails, neck drawn back low, bill open; must read at 1× from 6 tiles and be clearly bigger than `idle`) · `charge` 2f/4t loop (wings half raised, neck stretched forward low, a `w` bow wave at the breast) · `squished` held (sulking: head tucked under a wing, body turned slightly away). White `w` with `i` shading and a `k` outline, orange bill `o` with the black knob (`k`) of the mute swan (Höckerschwan), black eye. Only its upper part is drawn above the waterline; rows 17–23 are a faint `t`/`T` underwater body and paddling feet (or transparent) so it sits *in* the water. Faces right, `mirror = auto`. Proud, not menacing: a beaked diva |
| `gelse` (mosquito) | 16×16 | 8,12 (centre of the 8×8 hitbox at sprite (8, 8)) | `fly` 2f/2t loop (wing blur: frame 0 wings up `h w`, frame 1 down; the body stays) · `wait` 2f/4t loop (the telegraph: the same with a 1 px side-to-side wiggle and two tiny `w` "sss" whine dashes) · `leave` held (body tilted up, a small round `r` belly: full) · `squished` held (flattened, a `w` „klatsch" star). Thin dark body `d k`, long legs `d` (1 px), striped abdomen `d m`, a `r` proboscis tip. Readable at 1× against sky `s`, the far shore and the reeds: give it a `k` outline on the body only. Faces right, `mirror = auto` |
| `regattaboot` (racing dinghy) | 32×64 | 16,64 (bottom centre, on the bottom of its surface-row cell; the water line is sprite row 48, the hitbox 24×56 runs from row 4 (masthead) to row 60 (keel)) | `idle` 2f/30t loop (parked at the start: sail luffing, crew sitting) · `telegraph` 2f/6t loop (the start: a small flag `r w` on the stern drops, the crew hikes out, a `w` horn puff) · `drive` 2f/4t loop (heeled 1–2 px, sail full, a `w` bow wave on the left, crew hiking out on the windward side). A white hull `w i` with a `S` boot stripe, a tall white sail `w` with `h i` shading and a red `R` class emblem (no letters or numbers), a thin `d` mast, a `O` boom. Two crew in `S`/`Y` life vests, friendly. Below the water line (rows 49–63) only a dark `T`/`b` hull bottom and centreboard shape. Faces left, **no mirror** (it only races left; like the bus). The masthead row 4 is the bounce top: no pennant above it |

### Platforms (group `props`)

| Sprite | Size | Origin | Animations |
|---|---|---|---|
| `traundl` (the Plätte, rowed standing) | 48×32 | 24,32 (bottom centre of the 48×16 body; the body's top = sprite row 16 = the walking line) | `idle` 2f/30t loop (rower leans on the oar) · `ride` 2f/12t loop (rower pulls the long oar: frame 0 oar forward, frame 1 back; small `w` ripples at the stern). A long, flat, dark wooden hull `l O o` with a **raised, upturned bow** on the **left** end (the landing ramp of the real boat), the stern on the right where the rower stands (rows 0–15: a local in shirt sleeves and a green `G` hat with a long oar `O`). **The walking surface is one flat line**: sprite row 16 across the full 48 px (the gunwale/thwart line, a light `o` lip). The bow may rise 4 px above it only in the leftmost 6 px, and must not read as a step (pointed). The rower is decoration (Resi is drawn in front of him); keep him within x 34–47. No water drawn below row 31. Faces left (bow first); no mirror needed (the boat ping-pongs; the engine may mirror it when moving right, `mirror = auto` fine) As built: 48×32, origin 24,32, hitbox 0,16,48,16, `mirror = auto`; `idle` 2f/30t, `ride` 2f/12t. The deck's `k` top edge is row 16 over all 48 px with the `o` gunwale lip at row 17 (the Festungsbahn/Gondel convention: edge on the hitbox top, lip under it); the oar never breaks that `k` line. The bow rises only in x 0–4 (tip at (0, 12), pointed); the bow ramp's underside slopes from (0, 14) to (17, 31); the stern is raked from x 42 (the oar blade and `w` ripples sit in that corner, rows 26–31). Hull `o O l` with a dark `l` tarred bottom strake; the rower in x 34–47 (Lederhosen `O l` with `G` braces, `i` stockings, `G j` hat). Ride: frame 0 hands forward, oar to (45, 31); frame 1 hands back, oar upright at the stern (x 46–47). |
| `dampfer` (paddle steamer „Kaiser Franz Josef I.") | 160×48 | 80,48 (bottom centre of the 160×16 body; the deck line = sprite row 32) | `idle` held (moored/waiting) · `ride` 2f/8t loop (the paddle box's slots move, a flag flutters). Side view of a slim white lake steamer: black/dark hull below `k d` with a white `w i` bulwark to the deck line, a **paddle box** amidships (a half-round `w` housing with `o` gilt rays, spanning ~x 64–95 and rising 12 px above the deck, no flat top: rounded so it never reads as a step), one thin funnel (`o` with a `k` top, ≤ 8 px wide, x ~70–77; **no smoke**: diesel since 1954, the „ohne Dampf" joke), a small wheelhouse at the bow end drawn as **windows on posts with a pointed or rounded roof** (≤ 20 px above the deck, no flat walkable top), white railings `i` (1 px), a flagstaff with a red-white-red `r w r` flag at the stern, a few passengers `E`/`c` at the railing (max. 100). **One walking surface**: sprite row 32 across the full 160 px (the deck, a light `i` lip). Nothing above it reads as a platform (D-038, the Schafbergbahn rule). Below the deck (rows 33–47) the hull to the water line at row 47. Faces right (bow on the right, it travels right); no mirror As built: 160×48, origin 80,48, hitbox 0,32,160,16, no mirror; `idle` 1f, `ride` 2f/8t. The deck's `k` top edge is row 32 over all 160 px with a `w` cap-rail lip at row 33 and an `i` shadow line at row 34 (on a white bulwark the `k` edge is the walk line; an `i` top would read grey). Paddle box rows 19–31, x 63–96 incl. outline, `o` gilt rim, rays and a `Y o` hub; below the deck a `k`-framed `B` opening (x 66–93, rows 41–46) shows the paddles in hazard red `r`/`R` with 2 px floats on rows 45–46 (rays and paddles step 15° per ride frame). Funnel x 70–77: `k` flare on row 3, a `d` top band rows 4–6 inside the `k` sides. Wheelhouse x 131–143 with its apex at row 12 (20 px above the deck), open below the windows (rows 24–31, x 135–139 transparent but for a small skipper `B w e`) between two posts, so it never reads as a wall. Flag limp in idle, fluttering in ride. |
| `gischt` (paddle-wheel spray) | 16×16 | 8,16 | `spray` 3f/4t loop: white `w h` droplets thrown up and back. The renderer draws two of them over the paddle-wheel zone (at the water line under the paddle box) while the Dampfer travels As built: 16×16, origin 8,16, `spray` 3f/4t loop, no outline; the drops are thrown up and to the left (the steamer travels right): frame 0 a low burst, 1 the highest sheet, 2 falling drops. |

### Tiles (group `tiles`, 16×16, `tile_<name>`)

| Tile | Kind | Look |
|---|---|---|
| `see_wasser` | **swim** (new kind, D-076) | the swimmable lake's surface row: lighter and clearer than `wolfgangsee_water` (which stays lethal elsewhere): a `t` surface band in the top 3 rows with `w s` glints, animated 4f/12t (the water generator), and **below it clear water** `t`/`T` thin enough in pattern that the tile reads as see-through (sparse light caustic lines `s`, no dense bands). Must be clearly different from the lethal water at 1×: brighter, calmer, "Badewasser" |
| `see_wasser_tief` | swim | the rows below the surface: clear `T` with slow `t` caustic ripples (animated 4f/16t, subtle), darker towards row 15 so the lake bed reads as "down there" |
| `steg_pfahl` | swim | `see_wasser`/`see_wasser_tief` with a wooden pile `O l` (4 px wide, centred) going down through it; vertical autotile (N bit: with an open top it carries the surface band; S bit: with the lake bed below it ends in the bed). Used under the ends of `steg` decks. Engine: map it to Swim |
| `seegrund` | solid | the lake bed: pale gravel and sand `y m i` with a few dark `G j` weed tufts on the top edge and the odd `i` pebble; seen through clear water. Autotiled (16f `auto`): an open top = the bed surface with weeds; open left/right = a rounded gravel slope edge (shallows) |
| `steg` | one-way | pier/raft/pontoon planks: a `o O l` plank deck (top 5 px = walking surface, a light `o` lip) on a dark `l d` stringer, transparent below (the water shows). Autotiled left/right ends (an end tile shows the end beam and a pile top `O`). Same height as `jetty`'s deck so quays, piers and boats line up |
| `kai` | solid | the stone quay wall of the landings (St. Wolfgang, Strobl): dressed limestone blocks `i m` with `d` joints and a light `w`/`i` coping on top (walkable), a few iron rings `d`; where it stands in water, its lower rows get a darker `m T` wet band. Autotiled |
| `poller` | solid | a cast-iron mooring bollard, 1 tile: a squat `d k` post with a rounded cap and a light `i` highlight, standing on a deck. Its top (row 1–2) is walkable (it is solid); keep it obviously a bollard, not a box |

Reused in M5: `brick`, `oblaten_box`, `oblaten_box_empty`, `invisible` (under the Strobl `schiffsstation`), `meadow` (the goal meadow), `wolfgangsee_water(_deep)` stays as the lethal water of Stages 2–5 and is **not** used in Stage 6.

### Props (group `props`, M5)

All are `prop` entities (bottom centre at the cell, optional `dx`, `layer`); collision stays with tiles (D-030/D-031). `PROP_SPRITES` in jnr-core and the catalog test in `xtask/src/levels.rs` must include this table (engine task; the test reads the tables by heading). Props standing **in** the water must be `layer = "front"` (back-layer props are drawn before the tiles, so the water would cover them).

| Sprite | Size | Origin | Animations / notes |
|---|---|---|---|
| `adventlaterne` (the floating Advent lantern) | ≤ 48×80 | bottom centre | `idle` held — `layer = "back"`, standing on the pontoon's deck (row 11). A tall, slim lantern: a lattice body `d` with `i` glass panes, a pointed roof `d m` with a small star finial `Y`, on a square base. **Unlit** (it's summer): glass `i h`, no glow, a coil of LED string `m` hanging slack. Stylised (the real one is 16 m), its top stays below the HUD (≤ 80 px above the deck). No text. Its roof is 5 tiles above the deck: out of reach, and it must not look like a ledge (pointed) |
| `dalben` (mooring piles, the Traundl track markers) | 16×48 | 8,48 | `idle` held — `layer = "front"`; a cluster of 3 tarred piles `l d` lashed with a rope `o` and a pointed or rounded **conical cap** (never flat: it is not a platform, D-038), rising 2 tiles above the water line and continuing 1 tile under it (draw a `T` tint on the underwater part). Placed in the air cell above the surface at each end of a Traundl's track (`dx` to align) |
| `schilf` (reed bed) | 48×32 | 24,32 | `idle` 2f/30t loop (stalks sway 1 px) — `layer = "front"`; dense reed stalks `G g j` with brown `O` cattail heads, standing in the shallows (bottom on the water line ± a few px of stalks under it). Keep it ≤ 2 tiles above the water line and sparse at the top, so a Gelse hovering over it stays readable (the level keeps Gelsen above the reeds' top) |
| `startboje` (regatta start buoy) | 16×32 | 8,32 | `idle` 2f/20t loop (bobs 1 px, its flag waves) — `layer = "front"`; a round orange `o`/`r` buoy with a `w` band, a short pole and a small `Y` flag, sitting on the water line. Placed beside each parked Regattaboot |

Reused props: `schiffsstation` (the boat landing hall, M4) at the St. Wolfgang start (back layer, behind cols 0–5, the hall where Stage 5 ended) and at Strobl as the Maibaum launch (over `invisible` collision, the M4 profile: roof row 10).

### Backgrounds (group `bg`)

Same rules as Stages 1–5 (tileable, ≤ 640×180, bottom-aligned, `factor_y` 0 in this side level).

| Layer | Factor (1/16) | Content |
|---|---|---|
| `bg_sky_salzburg` (reused) | 0 | the Stage 1 sky |
| `bg_suedufer` (new) | 8 | the south shore seen from the middle of the lake: wooded hills `v G` rising to a few far limestone peaks (`f F`, `h` snow only on the highest), the lakeside road (B158) as a thin `m` line at the foot of the hills with a queue of tiny cars (`R b w` dots) and **one tiny yellow Postbus** (`Y` with a `k` window band, ~8×4 px) stuck in the queue (the checkpoint's „Bus 150 / im Stau. Ha!" made visible; once per 640 px), a few white houses (Strobl's direction), and **below the shore line the far lake** (`s`/`f` water streaks on `t`/`T`) down to the bottom edge, so the layer is opaque from the hill line down (the gap between the decks and the meadow at Strobl never shows sky). The shore line sits ~10 px above the screen's water line (y ~90) at the side camera's rest; mid-layer colour rules apply. 640×180 |
