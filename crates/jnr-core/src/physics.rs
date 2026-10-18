//! Axis-aligned boxes and collision against the tile grid.
//!
//! Boxes are half-open in subpixels: a box covers `[x, x + w)` × `[y, y + h)`.
//! Movement is resolved one axis at a time (X, then Y, done by the caller).
//! A move checks every tile column/row the leading edge newly enters, in
//! order, so moves of any length stop at the first blocking tile.

use crate::fx::Fx;
use crate::hash::Fnv;
use crate::level::{Level, TILE_PX, Tile, TileKind};

/// One tile in subpixels.
pub const TILE: Fx = Fx::from_px(TILE_PX);

/// An axis-aligned box in subpixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aabb {
    /// Left edge.
    pub x: Fx,
    /// Top edge.
    pub y: Fx,
    /// Width.
    pub w: Fx,
    /// Height.
    pub h: Fx,
}

impl Aabb {
    /// Exclusive right edge.
    pub fn right(&self) -> Fx {
        self.x + self.w
    }
    /// Exclusive bottom edge.
    pub fn bottom(&self) -> Fx {
        self.y + self.h
    }
    /// True if the two boxes share any area.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// Grid column/row containing a subpixel coordinate (floor division).
pub fn tile_of(v: Fx) -> i32 {
    v.raw().div_euclid(TILE.raw())
}

/// The tile grid as the simulation sees it: collision kinds plus the
/// look and block data of every cell. `Game` keeps the current grid
/// (bricks break, boxes empty); the level keeps the start state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TileMap {
    /// Width in tiles.
    pub width: i32,
    /// Height in tiles.
    pub height: i32,
    tiles: Vec<Tile>,
}

impl TileMap {
    /// The grid of a level in its start state.
    pub fn from_level(level: &Level) -> TileMap {
        TileMap {
            width: level.width,
            height: level.height,
            tiles: level.tiles.clone(),
        }
    }

    /// Feeds the whole grid into a state hash.
    pub(crate) fn hash(&self, h: &mut Fnv) {
        h.i32(self.width);
        h.i32(self.height);
        for t in &self.tiles {
            hash_tile(t, h);
        }
    }

    /// The tile at a grid cell, or `None` outside the grid.
    pub fn tile(&self, col: i32, row: i32) -> Option<Tile> {
        self.index(col, row).map(|i| self.tiles[i])
    }

    /// The tile at a row-major index (see [`TileMap::index`]).
    pub fn tile_at_index(&self, i: usize) -> Tile {
        self.tiles[i]
    }

    /// Replaces the tile at a grid cell (ignored outside the grid).
    pub fn set_tile(&mut self, col: i32, row: i32, tile: Tile) {
        if let Some(i) = self.index(col, row) {
            self.tiles[i] = tile;
        }
    }

    /// Row-major index of a cell inside the grid.
    pub fn index(&self, col: i32, row: i32) -> Option<usize> {
        if col < 0 || row < 0 || col >= self.width || row >= self.height {
            return None;
        }
        Some((row * self.width + col) as usize)
    }

    /// Level width in subpixels.
    pub fn width_fx(&self) -> Fx {
        TILE.mul_int(self.width)
    }

    /// Level height in subpixels.
    pub fn height_fx(&self) -> Fx {
        TILE.mul_int(self.height)
    }

    /// Tile kind at a cell, with the out-of-bounds rules: left and right of
    /// the level is solid wall; above the level, solid tiles of row 0 extend
    /// upwards (walls can't be jumped over through the sky); below the level
    /// is empty (falling out kills).
    pub fn kind_at(&self, col: i32, row: i32) -> TileKind {
        if col < 0 || col >= self.width {
            return TileKind::Solid;
        }
        if row >= self.height {
            return TileKind::Empty;
        }
        if row < 0 {
            return match self.tiles[col as usize].kind {
                TileKind::Solid => TileKind::Solid,
                _ => TileKind::Empty,
            };
        }
        self.tiles[(row * self.width + col) as usize].kind
    }

    /// True if any tile overlapped by `b` has kind `kind`.
    pub fn touches(&self, b: &Aabb, kind: TileKind) -> bool {
        let (c0, c1) = (tile_of(b.x), tile_of(b.right() - Fx(1)));
        let (r0, r1) = (tile_of(b.y), tile_of(b.bottom() - Fx(1)));
        (r0..=r1).any(|r| (c0..=c1).any(|c| self.kind_at(c, r) == kind))
    }

    /// True if `b` stands on at least one one-way tile and no solid tile
    /// (the row directly below its bottom edge); it may then drop through.
    pub fn on_one_way_only(&self, b: &Aabb) -> bool {
        let (c0, c1) = (tile_of(b.x), tile_of(b.right() - Fx(1)));
        let row = tile_of(b.bottom());
        let kinds = (c0..=c1).map(|c| self.kind_at(c, row));
        let mut one_way = false;
        for k in kinds {
            match k {
                TileKind::Solid => return false,
                TileKind::OneWay => one_way = true,
                TileKind::Empty | TileKind::Hazard => {}
            }
        }
        one_way
    }

    /// Moves `b` horizontally by `dx` and stops it at solid
    /// tiles. Returns true if a wall was hit (`b.x` is then flush with it).
    pub fn move_x(&self, b: &mut Aabb, dx: Fx) -> bool {
        let (r0, r1) = (tile_of(b.y), tile_of(b.bottom() - Fx(1)));
        let column_solid = |c: i32| (r0..=r1).any(|r| self.kind_at(c, r) == TileKind::Solid);
        if dx > Fx::ZERO {
            let old_edge = tile_of(b.right() - Fx(1));
            let new_edge = tile_of(b.right() + dx - Fx(1));
            for c in old_edge + 1..=new_edge {
                if column_solid(c) {
                    b.x = TILE.mul_int(c) - b.w;
                    return true;
                }
            }
        } else if dx < Fx::ZERO {
            let old_edge = tile_of(b.x);
            let new_edge = tile_of(b.x + dx);
            for c in (new_edge..old_edge).rev() {
                if column_solid(c) {
                    b.x = TILE.mul_int(c + 1);
                    return true;
                }
            }
        }
        b.x += dx;
        false
    }

    /// Moves `b` vertically by `dy` and stops it at tiles.
    /// Moving down, solid tiles and one-way tiles (unless `drop_through`)
    /// stop it, but only rows the bottom edge newly enters, so one-way tiles
    /// block from above only. Moving up, only solid tiles stop it.
    pub fn move_y(&self, b: &mut Aabb, dy: Fx, drop_through: bool) -> YHit {
        let (c0, c1) = (tile_of(b.x), tile_of(b.right() - Fx(1)));
        if dy > Fx::ZERO {
            let old_edge = tile_of(b.bottom() - Fx(1));
            let new_edge = tile_of(b.bottom() + dy - Fx(1));
            for r in old_edge + 1..=new_edge {
                let floor = (c0..=c1).any(|c| match self.kind_at(c, r) {
                    TileKind::Solid => true,
                    TileKind::OneWay => !drop_through,
                    TileKind::Empty | TileKind::Hazard => false,
                });
                if floor {
                    b.y = TILE.mul_int(r) - b.h;
                    return YHit::Floor;
                }
            }
        } else if dy < Fx::ZERO {
            let old_edge = tile_of(b.y);
            let new_edge = tile_of(b.y + dy);
            for r in (new_edge..old_edge).rev() {
                if (c0..=c1).any(|c| self.kind_at(c, r) == TileKind::Solid) {
                    b.y = TILE.mul_int(r + 1);
                    return YHit::Ceiling;
                }
            }
        }
        b.y += dy;
        YHit::None
    }
}

/// Feeds one tile (kind, look, block data) into a state hash.
pub(crate) fn hash_tile(t: &Tile, h: &mut Fnv) {
    h.u8(t.kind.id());
    h.u16(t.name);
    h.u8(t.block.id());
    h.u8(t.contains.id());
}

/// Result of [`TileMap::move_y`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum YHit {
    /// Moved freely.
    None,
    /// Landed on a floor (solid or one-way).
    Floor,
    /// Bumped the head on a solid tile.
    Ceiling,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Map from rows of `.`, `#`, `-`, `^` (plus a dummy player start).
    fn map(rows: &[&str]) -> TileMap {
        let mut grid = rows.join("\n");
        let first_air = grid.find('.').unwrap();
        grid.replace_range(first_air..first_air + 1, "P");
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"^\" = \"hazard\"\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
        );
        TileMap::from_level(&Level::parse(&text).unwrap())
    }

    fn bx(x_px: i32, y_px: i32) -> Aabb {
        Aabb {
            x: Fx::from_px(x_px),
            y: Fx::from_px(y_px),
            w: Fx::from_px(12),
            h: Fx::from_px(22),
        }
    }

    // 6 columns × 5 rows; floor at row 4 (y = 64).
    const ROOM: [&str; 5] = ["......", "......", "......", "......", "######"];

    #[test]
    fn lands_on_floor() {
        let m = map(&ROOM);
        let mut b = bx(20, 40); // bottom at 62
        assert_eq!(m.move_y(&mut b, Fx::from_px(3), false), YHit::Floor);
        assert_eq!(b.bottom(), Fx::from_px(64));
        // Standing exactly on the floor: any further downward step lands again.
        assert_eq!(m.move_y(&mut b, Fx(1), false), YHit::Floor);
        assert_eq!(b.bottom(), Fx::from_px(64));
        // Moving down freely when there is room.
        let mut b = bx(20, 10);
        assert_eq!(m.move_y(&mut b, Fx::from_px(3), false), YHit::None);
        assert_eq!(b.y, Fx::from_px(13));
    }

    #[test]
    fn walls_stop_horizontal_motion() {
        let m = map(&["......", "...#..", "...#..", "...#..", "######"]);
        // Box left of the wall at column 3 (x = 48), right edge at 46.
        let mut b = bx(34, 42);
        assert!(m.move_x(&mut b, Fx::from_px(4)));
        assert_eq!(b.right(), Fx::from_px(48));
        assert!(
            m.move_x(&mut b, Fx(1)),
            "flush against the wall stays blocked"
        );
        assert_eq!(b.right(), Fx::from_px(48));
        // From the right side, moving left.
        let mut b = bx(66, 42);
        assert!(m.move_x(&mut b, Fx::from_px(-5)));
        assert_eq!(b.x, Fx::from_px(64));
        // Free movement.
        let mut b = bx(66, 42);
        assert!(!m.move_x(&mut b, Fx::from_px(2)));
        assert_eq!(b.x, Fx::from_px(68));
    }

    #[test]
    fn long_moves_stop_at_the_first_blocking_tile() {
        let m = map(&["......", "...#..", "...#..", "...#..", "######"]);
        let mut b = bx(2, 42);
        assert!(m.move_x(&mut b, Fx::from_px(60)));
        assert_eq!(b.right(), Fx::from_px(48));
        let mut b = bx(66, 0);
        assert_eq!(m.move_y(&mut b, Fx::from_px(50), false), YHit::Floor);
        assert_eq!(b.bottom(), Fx::from_px(64));
    }

    #[test]
    fn on_one_way_only() {
        // One-way at columns 2-3, solid at columns 4-5, all in row 2 (top y = 32).
        let m = map(&["......", "......", "..--##", "......", "......"]);
        assert!(
            m.on_one_way_only(&bx(30, 10)),
            "half on air, half on one-way"
        );
        assert!(
            !m.on_one_way_only(&bx(54, 10)),
            "straddling one-way and solid"
        );
        assert!(!m.on_one_way_only(&bx(70, 10)), "on solid");
        assert!(!m.on_one_way_only(&bx(2, 10)), "in the air");
    }

    #[test]
    fn level_edges_are_walls() {
        let m = map(&ROOM);
        let mut b = bx(1, 40);
        assert!(m.move_x(&mut b, Fx::from_px(-3)));
        assert_eq!(b.x, Fx::ZERO);
        let mut b = bx(82, 40);
        assert!(m.move_x(&mut b, Fx::from_px(3)));
        assert_eq!(b.right(), Fx::from_px(96));
    }

    #[test]
    fn ceiling_bump() {
        let m = map(&["######", "......", "......", "......", "######"]);
        let mut b = bx(20, 18);
        assert_eq!(m.move_y(&mut b, Fx::from_px(-4), false), YHit::Ceiling);
        assert_eq!(b.y, Fx::from_px(16));
    }

    #[test]
    fn corners_only_block_when_overlapping() {
        // A single block at column 2, row 2 (x 32..48, y 32..48).
        let m = map(&["......", "......", "..#...", "......", "......"]);
        // Box right edge exactly at x = 32: falling past the block's side
        // does not land on it.
        let mut b = bx(20, 0);
        assert_eq!(m.move_y(&mut b, Fx::from_px(12), false), YHit::None);
        // One subpixel of horizontal overlap: lands on the corner.
        let mut b = Aabb {
            x: Fx::from_px(20) + Fx(1),
            ..bx(20, 0)
        };
        assert_eq!(m.move_y(&mut b, Fx::from_px(12), false), YHit::Floor);
        assert_eq!(b.bottom(), Fx::from_px(32));
        // Box bottom exactly at the block's top (y = 32): walking right
        // over the corner is not blocked.
        let mut b = bx(18, 10);
        assert!(!m.move_x(&mut b, Fx::from_px(4)));
        // One subpixel lower: the corner blocks.
        let mut b = Aabb {
            y: Fx::from_px(10) + Fx(1),
            ..bx(18, 10)
        };
        assert!(m.move_x(&mut b, Fx::from_px(4)));
        assert_eq!(b.right(), Fx::from_px(32));
    }

    #[test]
    fn one_way_blocks_only_from_above() {
        let m = map(&["......", "......", "..--..", "......", "######"]);
        // From above: lands on top (y = 32).
        let mut b = bx(30, 8);
        assert_eq!(m.move_y(&mut b, Fx::from_px(4), false), YHit::Floor);
        assert_eq!(b.bottom(), Fx::from_px(32));
        // Holding DOWN drops through.
        let mut b = bx(30, 8);
        assert_eq!(m.move_y(&mut b, Fx::from_px(4), true), YHit::None);
        // From below: jumps through.
        let mut b = bx(30, 40);
        assert_eq!(m.move_y(&mut b, Fx::from_px(-4), false), YHit::None);
        // Overlapping it while falling (bottom already inside the row): no snap up.
        let mut b = bx(30, 12); // bottom at 34, inside row 2
        assert_eq!(m.move_y(&mut b, Fx::from_px(2), false), YHit::None);
        // Sideways it never blocks.
        let mut b = bx(18, 20);
        assert!(!m.move_x(&mut b, Fx::from_px(4)));
    }

    #[test]
    fn touches_and_out_of_bounds() {
        let m = map(&["#.....", "......", "......", "......", "##^^##"]);
        assert!(m.touches(&bx(30, 45), TileKind::Hazard));
        assert!(
            !m.touches(&bx(30, 41), TileKind::Hazard),
            "bottom at 63 < 64"
        );
        assert_eq!(m.kind_at(-1, 2), TileKind::Solid);
        assert_eq!(m.kind_at(6, 2), TileKind::Solid);
        assert_eq!(m.kind_at(0, -3), TileKind::Solid, "row-0 walls extend up");
        assert_eq!(m.kind_at(1, -3), TileKind::Empty);
        assert_eq!(m.kind_at(2, 5), TileKind::Empty, "below the level is empty");
        assert_eq!(tile_of(Fx(-1)), -1);
        assert_eq!(tile_of(Fx::from_px(16)), 1);
    }
}
