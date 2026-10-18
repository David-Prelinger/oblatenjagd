//! Drawing the tile grid: `tile_<name>` sprites with autotiling, else
//! grey-box shapes per [`TileKind`].
//!
//! Autotiling (D-020): a tile sprite with a 16-frame animation `auto` is
//! drawn with frame = [`autotile_mask`] of its cell (bit 1 = the same tile
//! name above, 2 = right, 4 = below, 8 = left; cells outside the level count
//! as the same). Any other tile sprite shows its first animation, animated
//! by the game frame (e.g. water). Which look a tile name has is resolved
//! once in [`TileLook::resolve`]; the mask is computed from the live grid
//! (`Game::map`, bricks break and boxes empty; bumped blocks are raised)
//! while drawing (4 lookups per visible tile, no allocation), so tiles that
//! change during play stay correct.

use jnr_artkit::atlas::{Atlas, AtlasAnim};
use jnr_core::level::TILE_PX;
use jnr_core::physics::TileMap;
use jnr_core::{Camera, Game, TileKind};

use crate::Framebuffer;
use crate::blit::blit_frame;
use crate::palette::{DIRNDL_RED, LEATHER_DARK, STONE_GREY};

/// Name of the autotile animation.
pub const AUTO_ANIM: &str = "auto";
/// Frames of the autotile animation (one per neighbour mask).
pub const AUTO_FRAMES: usize = 16;
/// Height of a grey-box one-way platform in pixels (from the tile top).
const ONEWAY_PX: i32 = 4;

/// Neighbour bits of [`autotile_mask`].
pub const MASK_N: u8 = 1;
pub const MASK_E: u8 = 2;
pub const MASK_S: u8 = 4;
pub const MASK_W: u8 = 8;

/// How one tile name is drawn.
#[derive(Clone, Copy, Debug)]
pub struct TileLook<'a> {
    pub anim: &'a AtlasAnim,
    /// True: frame = neighbour mask; false: frame by game ticks.
    pub auto: bool,
}

impl<'a> TileLook<'a> {
    /// The look of sprite `name`: its `auto` anim if that has exactly 16
    /// frames, else its first anim. `None` without sprite or frames.
    pub fn resolve(atlas: &'a Atlas, name: &str) -> Option<TileLook<'a>> {
        let sprite = atlas.sprite(name)?;
        if let Some(anim) = sprite
            .anim(AUTO_ANIM)
            .filter(|a| a.frames.len() == AUTO_FRAMES)
        {
            return Some(TileLook { anim, auto: true });
        }
        let anim = sprite.anims.first().filter(|a| !a.frames.is_empty())?;
        Some(TileLook { anim, auto: false })
    }
}

/// The 4-neighbour mask of cell (col, row): a bit is set when that
/// neighbour has the same tile name or lies outside the level.
pub fn autotile_mask(map: &TileMap, col: i32, row: i32) -> u8 {
    let Some(here) = map.tile(col, row) else {
        return 0;
    };
    let same = |c: i32, r: i32| map.tile(c, r).is_none_or(|t| t.name == here.name);
    let mut mask = 0;
    for (dc, dr, bit) in [
        (0, -1, MASK_N),
        (1, 0, MASK_E),
        (0, 1, MASK_S),
        (-1, 0, MASK_W),
    ] {
        if same(col + dc, row + dr) {
            mask |= bit;
        }
    }
    mask
}

/// Tiles inside the camera view. `looks` is indexed by
/// [`jnr_core::Tile::name`] (see [`crate::RenderContext`]).
pub fn draw_tiles(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    looks: &[Option<TileLook>],
    game: &Game,
    cam: Camera,
    tick: u32,
) {
    let level = game.map();
    let col0 = cam.x.div_euclid(TILE_PX).max(0);
    let col1 = (cam.x + fb.w as i32 - 1)
        .div_euclid(TILE_PX)
        .min(level.width - 1);
    let row0 = cam.y.div_euclid(TILE_PX).max(0);
    let row1 = (cam.y + fb.h as i32 - 1)
        .div_euclid(TILE_PX)
        .min(level.height - 1);
    for row in row0..=row1 {
        for col in col0..=col1 {
            let Some(tile) = level.tile(col, row) else {
                continue;
            };
            let x = col * TILE_PX - cam.x;
            // Bumped blocks are drawn raised (E1).
            let y = row * TILE_PX - cam.y - game.tile_bump_px(col, row);
            match looks.get(tile.name as usize).copied().flatten() {
                Some(look) => {
                    let index = if look.auto {
                        usize::from(autotile_mask(level, col, row))
                    } else {
                        look.anim.frame_index_at(tick)
                    };
                    blit_frame(fb, atlas, look.anim.frame(index, false), x, y, false);
                }
                None => match tile.kind {
                    TileKind::Empty => {}
                    TileKind::Solid => {
                        let c = crate::entities::solid_tile_colour(game, tile, STONE_GREY);
                        fb.fill_rect(x, y, TILE_PX, TILE_PX, c);
                    }
                    TileKind::OneWay => fb.fill_rect(x, y, TILE_PX, ONEWAY_PX, LEATHER_DARK),
                    TileKind::Hazard => fb.fill_rect(x, y, TILE_PX, TILE_PX, DIRNDL_RED),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jnr_core::Level;
    use crate::testutil::{self, A, B, image};

    fn game(l: &Level) -> Game {
        let t = jnr_core::Tuning::parse(include_str!("../../../content/tuning.toml")).unwrap();
        Game::new(l, &t, 1)
    }

    fn level(legend: &str, grid: &str) -> Level {
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 300\n\n[legend]\n{legend}\"P\" = {{ entity = \"player_start\" }}\n\
             \n[map]\ngrid = '''\n{grid}'''\n"
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn mask_counts_same_names_and_level_edges() {
        let l = level(
            "\"#\" = \"ground\"\n\"B\" = \"brick\"\n",
            "P...\n.##.\n.#B.\n",
        );
        // (1,1): N empty, E ground, S ground, W empty.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 1, 1), MASK_E | MASK_S);
        // (2,1): N empty, E empty, S brick (other name), W ground.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 2, 1), MASK_W);
        // (1,2): N ground, E brick, S level edge (same), W empty.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 1, 2), MASK_N | MASK_S);
        // (2,2) brick: N ground, E empty, S edge, W ground.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 2, 2), MASK_S);
        // Corner cell (0,0), the player start, is empty (name 0) like its
        // neighbours (1,0) and (0,1); the outside counts as same.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 0, 0), MASK_N | MASK_E | MASK_S | MASK_W);
        // (3,1) empty: N, S empty, E outside, W ground.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), 3, 1), MASK_N | MASK_E | MASK_S);
        assert_eq!(autotile_mask(&TileMap::from_level(&l), -1, 0), 0, "outside the level");
        // The player start above is an empty cell, not ground.
        let full = level("\"#\" = \"ground\"\n", "P\n#\n");
        assert_eq!(autotile_mask(&TileMap::from_level(&full), 0, 1), MASK_E | MASK_S | MASK_W);
    }

    /// 16x16 frame: all `b`, with an `a` marker at (marker, 0).
    fn marked(marker: usize) -> jnr_artkit::Image {
        let mut row0 = "b".repeat(16);
        row0.replace_range(marker..marker + 1, "a");
        let mut rows = vec![row0];
        rows.extend((0..15).map(|_| "b".repeat(16)));
        let rows: Vec<&str> = rows.iter().map(String::as_str).collect();
        image(&rows)
    }

    #[test]
    fn auto_tiles_pick_the_frame_by_mask() {
        let frames = (0..16).map(marked).collect();
        let atlas = testutil::atlas(&[testutil::sprite(
            "tile_ground",
            (0, 0),
            false,
            vec![("auto", 0, frames)],
        )]);
        let look = TileLook::resolve(&atlas, "tile_ground").unwrap();
        assert!(look.auto);
        let l = level("\"#\" = \"ground\"\n", "P...\n.##.\n....\n");
        let looks = vec![None, Some(look)];
        let mut fb = testutil::fb();
        draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, 0);
        // (1,1): only E -> frame 2, marker at x 16 + 2; (2,1): only W -> frame 8.
        assert_eq!(fb.get(16 + 2, 16), Some(A));
        assert_eq!(fb.get(16 + 3, 16), Some(B));
        assert_eq!(fb.get(32 + 8, 16), Some(A));
        assert_eq!(fb.get(32 + 2, 16), Some(B));
    }

    #[test]
    fn other_tiles_animate_with_the_game_frame() {
        let atlas = testutil::atlas(&[
            testutil::sprite(
                "tile_water",
                (0, 0),
                false,
                vec![("flow", 4, vec![marked(0), marked(5)])],
            ),
            // `auto` with the wrong frame count is not an autotile.
            testutil::sprite(
                "tile_odd",
                (0, 0),
                false,
                vec![("auto", 0, vec![marked(1)])],
            ),
        ]);
        let water = TileLook::resolve(&atlas, "tile_water").unwrap();
        assert!(!water.auto);
        assert!(!TileLook::resolve(&atlas, "tile_odd").unwrap().auto);
        assert!(TileLook::resolve(&atlas, "tile_none").is_none());
        let l = level("\"~\" = \"salzach_water\"\n", "P\n~\n");
        let looks = vec![None, Some(water)];
        let draw = |tick| {
            let mut fb = testutil::fb();
            draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, tick);
            (fb.get(0, 16), fb.get(5, 16))
        };
        assert_eq!(draw(0), (Some(A), Some(B)));
        assert_eq!(draw(3), (Some(A), Some(B)));
        assert_eq!(draw(4), (Some(B), Some(A)));
        assert_eq!(draw(8), (Some(A), Some(B)), "loops");
    }
}
