# Sprite catalog (M1)

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

## UI (group `ui`)

| Sprite | Size | Notes |
|---|---|---|
| `font` | 8×8 | D-012 |
| `title_logo` | ≤ 240×48 | „Oblatenjagd" logo; origin = centre |
| `hud_oblate` | 8×8 | small Oblate icon before the OBLATEN counter (optional) |

## Backgrounds (group `bg`, produced by generators or hand-made)

Parallax layers, each tileable horizontally (left and right edges match; lint `bg-tileable`), ≤ 640 px wide, ≤ 180 px high (lint `bg-size`), origin 0,0, first animation used. The level's `[meta].background` lists them back to front with a scroll factor in sixteenths (engine: `RenderContext::with_background`). Layers are drawn **bottom-aligned to the screen bottom** and do not move vertically, so a layer's interesting part must sit high enough above the playfield ground (≈ 32–48 px at the bottom are usually hidden by ground tiles). Salzburg: sky generated (`art/generators/salzburg_sky.toml`), Untersberg, Festung and rooftops hand-drawn (`art/sprites/bg/`); factors 0 / 3 / 8 / 13, y = 0.

`bg_sky_salzburg` (factor 0) · `bg_untersberg` (far mountains) · `bg_festung` (Hohensalzburg on its hill, mid) · `bg_rooftops` (old-town roofs and domes, near).
