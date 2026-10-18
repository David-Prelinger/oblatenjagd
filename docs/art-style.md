# Art style guide

Status: **approved v1** (pixel-artist, approved by art-director in M0 after 2 review rounds).

This is the reference the art-director judges against. Source of truth for colours: `art/palettes/main.palette`. Formats and commands: `docs/art-pipeline.md` and the `sprite-authoring` skill.

Look: a sunny, slightly warm Salzkammergut summer. Clear blue skies, turquoise lakes, saturated meadows, warm baroque façades; cool bluish shadows and hazy distant mountains. Friendly and readable, like a late-NES / early-SNES platformer.

## 1. Resolution and scales

| Thing | Size |
|---|---|
| Framebuffer | 320×180, integer-scaled only (×4 720p, ×6 1080p) |
| Tiles | 16×16 (visible screen: 20×11.25 tiles) |
| Resi, normal | 16×24, origin bottom centre (8,24) |
| Resi, Zuckerschock | 16×32, origin (8,32) |
| Standard enemies | 16×16 to 16×24; large enemies (Fiaker) up to 48×32 |
| Items | Oblate 12×12, Mozartkugel 12×12 max, power-ups ≤16×16 |
| UI font | 8 px line height (see §11) |

No sub-pixel art, no rotation or scaling at runtime, no mixed pixel sizes. Every pixel is one framebuffer pixel.

## 2. Palette

30 colours + transparent (`.`); 2 slots are reserved for later (adding one needs art-director approval and an entry in `docs/decisions.md`). Ramps run light → dark. Hue shifts: highlights shift toward yellow/white. Warm materials shade toward red-brown (never grey). Cool and neutral materials shade toward blue/slate.

| Char | Hex | Name | Intended uses |
|---|---|---|---|
| `k` | #1a1c2c | outline | outlines of characters, items, interactive objects; pupils; UI text shadow. Never used for large fills. |
| `d` | #343a50 | slate_dark | darkest stone shadow, iron (guild signs, railings), dark clothes, UI panels |
| `m` | #5d6680 | stone_grey | stone/cobblestone mid, Mozartkugel foil shadow, trousers, camera bodies |
| `i` | #a2abbe | silver_light | limestone light, silver foil, white-cloth shadow, metal highlights |
| `w` | #fbf7ee | white | blouse, clouds, foil glint, water sparkles, UI text |
| `h` | #dff1f0 | sky_haze | sky at the horizon, snow on far peaks, cloud shadow, distant haze |
| `s` | #a6daf2 | sky_light | middle sky band, window glass highlight, water highlight on deep blue |
| `S` | #64aee6 | sky_blue | upper sky, Resi's apron, blue shirts |
| `f` | #a4bad8 | far_light | farthest mountain layer, lit side |
| `F` | #8ea4cc | far_dark | farthest mountain layer, shadow side |
| `v` | #7aa294 | mid_sage | mid-distance hills and forests |
| `t` | #46cabe | lake_turquoise | Wolfgangsee surface, verdigris copper roofs/domes (light) |
| `T` | #1e8e9c | lake_teal | lake mid water, Salzach, copper roof shadow |
| `b` | #2e64ae | lake_deep | deep water, window glass, apron shadow, blue clothes shadow |
| `B` | #2a3878 | navy | deepest water, night, UI panel background, blue-cloth darkest |
| `y` | #b0d65a | meadow_light | sunlit grass edge, leaf highlights, lime shirts |
| `g` | #56a845 | meadow | grass, hedges (Mirabellgarten), leaves |
| `G` | #2a7050 | pine_dark | pine trees, hedge shadow, green shutters, hunter-green clothes |
| `j` | #1b4640 | pine_shadow | deepest foliage shadow, pine trunks in shade |
| `c` | #f9e5b8 | cream | cream façades, window sills, Oblate wafer body, Zaunerstollen glaze light |
| `Y` | #f5c547 | mozart_yellow | Mozart's birthplace façade, blond hair light, gold/brass highlight, HUD numbers |
| `o` | #d9913a | oblate_gold | Oblate waffle pattern, ochre façades, wood light, gold/brass mid, blond hair shadow |
| `O` | #9e5a2e | oblate_brown | Oblate shadow, wood mid, Lederhosen light, earth, brown hair |
| `l` | #5c3226 | leather_dark | leather, Lederhosen shadow, dark chocolate, shoes, wood shadow, dark hair |
| `p` | #f2aeb0 | pastel_pink | pink baroque façades, tourist shirts, cheeks (1 px max) |
| `r` | #cf3a3e | dirndl_red | Resi's bodice and skirt, Mozartkugel wrapper, red roofs, hazard accents, UI titles |
| `R` | #8a2340 | wine_dark | red shadow, roof shadow, dark-red cloth |
| `e` | #f9cfa6 | skin_light | light skin |
| `E` | #dc9068 | skin_shade | light-skin shadow; darker skin light; terracotta roof light |
| `n` | #a85f40 | skin_dark | darker skin; terracotta roof shadow; brick |

### Ramps (which colour shades into which)

| Material | Highlight → mid → shadow | Notes |
|---|---|---|
| Stone, limestone, metal | `w` → `i` → `m` → `d` | cobbles use `i m d`; iron uses `m d` (+ `k` outline) |
| Silver foil (Mozartkugel) | `i` → `m` (+ 1 px `w` glint) | or `w` → `i` → `m` with no extra glint; never 4 entries |
| White cloth (blouse) | `w` → `i` | |
| Sky | `S` (top) → `s` → `h` (horizon) | gets lighter downwards |
| Far mountains | `h` (snow) → `f` → `F` | only these + sky colours |
| Water | `w`/`s` sparkle, `t` → `T` → `b` → `B` | lighter at the surface |
| Copper roofs | `t` → `T` | |
| Foliage | `y` → `g` → `G` → `j` | max 3 per object, e.g. hedge `y g G`, pine `g G j` |
| Gold, brass, blond hair | `c`/`Y` → `o` → `O` | |
| Wood | `o` → `O` → `l` | |
| Oblate wafer | `c` → `o` → `O` | body `c`, pattern `o`, shadow `O` (§10) |
| Leather, Lederhosen | `O` → `l` (+ `k` outline) | |
| Chocolate | `O` → `l` | |
| Yellow façade | `Y` → `o` | |
| Cream façade | `c` → `o` (or `c` → `i` for cool shadows) | |
| Pink façade | `p` → `E` | |
| Red cloth, roof tiles | `p` → `r` → `R` | `p` only as a small highlight |
| Terracotta | `E` → `n` → `R` | |
| Light skin | `e` → `E` | |
| Darker skin | `E` → `n` → `l` | |
| Blue cloth | `S` → `b` → `B` | |

## 3. Outlines

- Characters, enemies, items, projectiles and interactive objects (Oblatenkiste, breakable bricks, moving platforms, one-way guild signs) get a closed **1 px `k` outline**.
- Inner detail lines (the bodice edge, folds, the split between legs) use the **darkest shade of that material**, not `k`. Exceptions: eyes and mouth may use `k`.
- Static solid tiles (ground, walls, hedges) have no `k` outline; their edges use the material's darkest shade, and their top edge gets the lightest shade (`y` grass lip, `i` stone lip).
- Backgrounds (all parallax layers) never use `k` or outlines of any kind.
- No anti-aliasing against the background (we don't know which background it will be on). Inside a sprite, at most 1 px of mid shade to soften a curve.

## 4. Light

- Light comes from the **top left**. Lit surfaces face up and left; shadows are on the bottom and right.
- Cast shadows fall to the bottom right. Characters get no drop shadow on the ground.
- Specular glints (`w`) are single pixels in the upper left of round shiny things (Mozartkugel, glass, water).

## 5. Shading

- **Max 3 shades per material**: any one material in one sprite uses at most 3 *consecutive* entries of its ramp. A single-pixel `w` specular glint does not count. The `k` outline does not count. Many small sprites need only 2.
- Shade by form: a sphere is lit top left and dark bottom right; a cylinder is lit on its left; a flat wall is one tone with a darker right/bottom edge.
- **No pillow shading** (dark rim on all sides with a light centre).
- **No banding**: don't run two shades in parallel lines of the same length along an edge.
- No single-pixel noise: every shade cluster is at least 2 px unless it is a deliberate glint, eye or button.
- Hue shift: highlights shift toward yellow/white. Warm materials shade toward red-brown (never grey). Cool and neutral materials shade toward blue/slate. Never darken with `k` (e.g. a yellow shadow is `o`, not `m`).

## 6. Dithering

- **Backgrounds only** (sky, far/mid/near parallax layers, large generated water/foliage areas). Never on characters, items, UI or playfield tiles.
- Only ordered 2×2 (checkerboard) or 4×4 Bayer patterns, only between **two neighbouring colours of one ramp or one parallax layer** (e.g. `S`/`s`, `f`/`F`, `v`/`G`, `t`/`T`). Never dither colours from different layers or across a value gap of more than one step.
- Transition bands are 2–8 px tall; no dither covering a whole area.
- Exception for the playfield: water tiles from the `water` generator may dither only in their surface band, using only `t`/`T`, at most 4 px tall. The rest of a water tile is flat bands.

## 7. Parallax depth

Atmospheric perspective: the further away, the bluer, lighter and lower in contrast.

| Layer | Scroll factor | Allowed colours | Notes |
|---|---|---|---|
| Sky | 0.0–0.05 | `S s h w` | bands get lighter towards the horizon; clouds `w` with `h` undersides |
| Far (Untersberg, Schafberg, Gaisberg) | 0.1–0.25 | `f F h` (+ sky colours) | 2 tones + snow; lit/shadow split along the ridge (left lit) |
| Mid (hills, forests, Festung/city silhouettes) | 0.4–0.6 | `v G` (+ `i m` only for Festung or city silhouettes) | `v` is the dominant fill (≥60 % of the layer's pixels); `G` only for shadow masses |
| Near (trees, houses behind the street) | 0.75–0.9 | `G j g T t m d` + façade colours `c Y o p E n R` | no `k`, no outlines; flatter shading (≤2 shades) than the playfield; see near-layer rules below |
| Playfield | 1.0 | full palette | tiles, entities, items |
| Foreground (rare) | >1.0 | `j G l d` | silhouettes only, never covering the player for more than 1 tile |

Near-layer rules:
- `Y` is never the main fill of a near-layer façade; use `c`, `p` or `o`. Full `Y` façades (e.g. Mozart's birthplace) are playfield set pieces only.
- Near-layer windows use `d` or `T`, with no `w` glints.
- `d` appears in the near layer only for window or door openings of at most 2×3 px.
- Roofs: terracotta (`E`) next to an ochre façade (`o`) must be separated by an `n` or `R` eave line.

Water at playfield level uses the full water ramp (`t T b B` + `w s` sparkles); dithering rules in §6.

## 8. Contrast hierarchy

From most to least prominent: **Resi > enemies, items, hazards > playfield tiles > near background > mid > far > sky.**

- Resi has the strongest value contrast.
- `r` is reserved for: Resi, Mozartkugeln, hazards and danger signs, roof tiles (playfield/near only), UI titles. Enemies use `r` only as a small accent (≤10 % of the sprite).
- No character sprite other than Resi contains all three of `r`, `S` and `w`. An enemy with an `S` remap may not carry an `r` prop. Items and UI are exempt.
- Collectibles (Oblaten) are cream and gold with a `k` outline so they pop against every sky and water colour.
- On the playfield, only collectibles use `c` as a fill; their `k` outline separates them from `c` façades behind them. (Solid tiles never use `c` as a fill.)
- Backgrounds never contain shapes that look like collectibles (round gold) or hazards (red spikes).
- Solid vs background: solid tiles have a light top lip and darker body; background layers never have a light top lip of the same kind.

## 9. Animation

| Animation | Frames | Ticks per frame (60 Hz) |
|---|---|---|
| Resi idle | 2 | 30 |
| Resi walk | 4 | 8 |
| Resi run | 4 | 5 |
| Resi jump / fall | 1 / 1 | – |
| Enemy walk | 2 | 12 |
| Enemy squished / hit | 1 | held 30 then removed |
| Oblate spin | 4 | 8 |
| Water surface | 4 | 12 |
| Flag/Maibaum ribbons, flowers | 2 | 20 |

- Frame counts are fixed per animation; ticks are integers.
- **Stable origin**: the origin (bottom centre) stays on the same pixel in every frame; feet touch the bottom row on ground frames. No vertical bobbing of the whole sprite except deliberate 1 px body bob in walk/run.
- Change as few pixels as possible between frames; key poses must read on their own.
- Left-facing versions come from `mirror = auto`, so keep sprites lit for the right-facing pose and accept the mirrored light (standard practice).
- No single-frame flashes except the hit-blink (every 4 ticks, invisible frames).

## 10. Characters

### Resi (player)
- Colours: blond hair `Y o` (+ `O` darkest) in a braid that sticks out behind her head; where hair touches skin, the hair edge is `O`, not `o`; skin `e E`; white blouse `w i` with puffed sleeves; **red bodice and red skirt `r R`; blue apron `S b`**; shoes `l`.
- Silhouette: braid + bell-shaped skirt. It must read as "girl in Dirndl" as a solid black shape at 1×.
- The red-skirt-plus-blue-apron combination is unique to Resi; no other sprite may use it.
- Zuckerschock (16×32): same design and colours, taller legs and body, a bit wider skirt; not just a stretched sprite.

### Enemies
- Everyone wears traditional dress too, but in other colour combinations (e.g. Dirndl `G`/`p`, `B`/`Y`; Lederhosen `O l` with `G` or `d` braces).
- **One readable satirical prop per enemy**, at least 3 px wide, outlined in `k`:
  - Selfie-Tourist: selfie stick with a phone held high (`m d` + `w` glint); shirt variants via `remap` (`p`, `y`, `S`, `Y`).
  - Taube (pigeon): grey `i m d` with a teal `T` neck shimmer.
  - Mozartkugel-Verkäufer: tray of Mozartkugeln; Mozartkugel = silver foil `i m` + 1 px `w` glint with a red `r R` band, chocolate `l` showing when bitten.
  - Fiaker: horse with a hat (bowler `d`, or a small Tyrolean hat `G` with a feather `w`); coachman in `d`/`l`.
  - Sound-of-Music guide: umbrella held up in `r` or `Y`.
- Enemies are at most as tall as normal Resi unless they are a "big" type; they face the player by default.

### Items
- Zauner Oblate (coin): round wafer with body `c`, waffle pattern `o`, bottom-right shadow `O`, `k` outline; at least 40 % of its pixels are `c`.
- Oblatenkiste: wooden box `o O l` with an Oblate emblem, `k` outline; used state `O l` without emblem.
- Zaunerstollen: chocolate `l O` log with `c` glaze dots.
- Hackl: wooden handle `o O`, iron head `i m d`.

## 11. UI and font

- Bitmap font, **8 px line height**: rows 0 accents/umlaut dots, rows 1–6 cap height, row 6 baseline, row 7 descenders. x-height 4 px (rows 3–6). Variable width, glyphs ≤5 px wide + 1 px spacing.
- Source: `art/sprites/ui/font.sprite`, one anim `glyphs` (89 frames, frame order = the character string in the file header). Capital umlauts use a 5 px body (rows 2–6), row 1 empty, dots in row 0, columns 1 and 3; lowercase umlaut dots and the i/j dots sit in row 1 (one row gap above the x-height). Ascenders reach row 1. `g p q y j ß` and the low quote `„` use row 7. Digits are 5 px wide so HUD numbers keep a fixed advance; `0` has a diagonal stroke to tell it apart from `O`. The space glyph is empty; its advance is the engine's job.
- Required glyphs: A–Z, a–z, 0–9, `ÄÖÜäöüß`, `„“`, `.,:;!?'-–/()%+×`. Capital umlauts use a 5 px body (rows 2–6), row 1 empty, dots in row 0, columns 1 and 3. `ß` is a real ß, never "ss". German quotes are low `„` (rows 6–7) and high `“` (rows 1–2).
- HUD text: `w` with a 1 px `k` shadow to the bottom right (or a full `k` outline), numbers in `Y`. Must stay readable over every background, including `h` sky and `w` clouds.
- Panels: `B` fill with a `k` border and a 1 px `S` or `i` inner highlight on top/left.
- Titles: `r` with `w` inner highlight and `k` outline; logo accents `Y`.
- No UI dithering, no transparency effects.

## 12. Do / don't

**Do**
- Start from a silhouette and check it at 1×.
- Use the ramps above; follow the hue-shift rule in §5.
- Make every joke readable from the prop alone (selfie stick, Fiaker hat, Mozartkugel wrapper).
- Look at every asset in a scene, not only alone.
- Keep backgrounds calm: big shapes, few colours, low contrast.

**Don't**
- Use colours outside the palette or invent new ones.
- Use more than 3 shades per material, pillow shading, banding, or single-pixel noise.
- Dither on sprites, tiles or UI.
- Outline backgrounds, or use `k` for large fills or inner details.
- Put `r`, `k` or strong contrast in far/mid layers.
- Reuse Resi's red-skirt + blue-apron combination on anything else.
- Draw anything gold and round in the background (looks like an Oblate).
- Make jokes that mock individuals or groups meanly; satire targets tourism, kitsch and bureaucracy.
