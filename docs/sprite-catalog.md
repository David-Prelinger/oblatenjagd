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
| `selfie_tourist` | 16×24 | 8,24 | `walk` 2f/10t loop · `selfie` 2f (stick up, held / flash frame) · `squished` held |
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
- M1 tile names (the level-designer uses these; more can be added by agreement): `ground` (cobblestone street), `meadow` (grass on earth), `rock` (Festungsberg rock), `rampart` (fortress wall), `hedge` (Mirabell hedge), `stone` (fountain / statue stone), `bridge` (Makartsteg deck), `railing` (love-lock railing, decorative, non-solid variant `railing_bg`), `salzach_water` (hazard, animated), `brick`, `oblaten_box`, `oblaten_box_empty`, `oneway_sign` (Zunftschild), `oneway_burger` (Zunftschild variant), `oneway_wood` (Jedermann stage, grandstand), `arch` (Dombögen, solid).

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

## UI (group `ui`)

| Sprite | Size | Notes |
|---|---|---|
| `font` | 8×8 | D-012 |
| `title_logo` | ≤ 240×48 | „Oblatenjagd" logo; origin = centre |
| `hud_oblate` | 8×8 | small Oblate icon before the OBLATEN counter (optional) |

## Backgrounds (group `bg`, produced by generators or hand-made)

Parallax layers, each tileable horizontally (left and right edges match), ≤ 640 px wide, ≤ 180 px high, origin 0,0. The level's `[meta].background` lists them back to front with a scroll factor (engine format).

`bg_sky_salzburg` (factor 0) · `bg_untersberg` (far mountains) · `bg_festung` (Hohensalzburg on its hill, mid) · `bg_rooftops` (old-town roofs and domes, near).
