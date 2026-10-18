---
name: visual-review
description: How to render review images (contact sheets, animation filmstrips, scenes, replay filmstrips) and the rubric for judging pixel art and scenes. Use when reviewing or self-checking any visual change.
---

# Visual review

Status: approved v1.

## Rendering review images
| Purpose | Command | Output |
|---|---|---|
| all sprites at a glance | `cargo xtask sprites sheet` | `target/preview/contact_sheet.png` |
| one sprite, scaled | `cargo xtask sprites preview <name>` | `target/preview/sprites/<name>.png` |
| animation | `cargo xtask sprites anim <name>` | `target/preview/anim/<name>.png` (+ one `<name>_<anim>.gif` per animation) |
| in context | `cargo xtask scene <level> --x <px>` (camera left edge at x, player on the floor) | `target/preview/scenes/<level>_<px>.png` (+ `_x4`) |
| replay shots | `cargo xtask replay <replay> --shots` | `target/preview/replays/<replay>_<shot>.png` (+ `_x4`) |
| motion over time | `cargo xtask filmstrip <replay> [--every N]` | `target/preview/filmstrips/<replay>.png` |
| golden mismatch | (from verify) | `target/reports/golden/<name>_{actual,expected,diff}.png` |

Use the Read tool to look at the PNGs. Always look at the asset **in a scene** too, not only in isolation.

## Rubric
1. **Readability**: the silhouette is clear at 1× against its real background; the player always stands out most.
2. **Palette**: only palette colours; shading uses ≤3 shades per material; backgrounds are lower contrast than the playfield.
3. **Consistency**: same outline rules, light from the top left, proportions match neighbouring sprites (player-to-enemy scale).
4. **Gameplay clarity**: solid vs background vs hazard is obvious; collectibles pop; hitbox fits the visible shape.
5. **Animation**: stable origin (no jitter), readable key poses, no single-frame flicker.
6. **Tiles**: no visible seams when repeated; edges and corners correct.
7. **UI/Text**: German spelling and umlauts correct, nothing clipped, HUD readable over every background.
8. **Humour**: is the joke readable without explanation (selfie stick, Fiaker hat, Mozartkugel wrapper)?

Every problem needs a concrete fix (which frame, which pixels, which colour).
