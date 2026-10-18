# Sprite catalog (M1, M2)

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
- M1 tile names (the level-designer uses these; more can be added by agreement): `ground` (cobblestone street), `meadow` (grass on earth), `rock` (Festungsberg rock), `rampart` (fortress wall), `hedge` (Mirabell hedge), `stone` (fountain / statue stone), `bridge` (Makartsteg deck), `railing` (love-lock railing, decorative, non-solid variant `railing_bg`), `salzach_water` (hazard surface row, animated 4f/12t), `salzach_water_deep` (hazard, rows below the surface), `brick`, `oblaten_box`, `oblaten_box_empty`, `oneway_sign` (Zunftschild), `oneway_burger` (Zunftschild variant; one-way tiles autotile left/right ends: a run of them reads as one iron arm with a wall bracket on the left and the emblem on the right; top 5 px = walking surface), `oneway_wood` (Jedermann stage, grandstand), `arch` (Dombögen, solid), `facade` (house wall with windows, autotiled — Erker, house blocks), `arch_under` (arch underside with the curved soffit, autotiled left/right; `arch` and `arch_under` count as one autotile family, so an arch on its underside has no edge between them), `invisible` (solid, never drawn, no sprite — under props such as the Sphaera).

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
| `wolfgangsee_water` | hazard | lake surface, turquoise `t T` + `w s` sparkles, animated 4f/12t (water generator, art-style §6) |
| `wolfgangsee_water_deep` | hazard | rows below the surface, `T b B` bands |
| `forest_path` | solid | earth path `O l` with a `y g` grass lip and roots, autotiled |
| `beute` | solid | bee hive box (`o O l` wood) with a small painted front panel; stackable (autotile vertical neighbours so a stack reads as boxes, not a wall) |

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
