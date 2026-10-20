//! Swim water (D-076): where Resi swims, where the surface is, and the
//! level lint for it. The player's swim physics live in
//! [`crate::player::Player::update`].
//!
//! - **Swimming** = her chest point (hitbox centre x, `swim.chest_px` below
//!   the hitbox top, every form) lies in a [`TileKind::Swim`] tile.
//! - **Surface** = the top edge of the topmost swim tile of the run of swim
//!   tiles the chest is in (the cell above it is not swim water).
//! - **At the surface** = swimming with the chest less than
//!   `swim.surface_band` px below the surface.

use crate::fx::Fx;
use crate::level::{Level, TileKind};
use crate::physics::{TILE, TileMap, tile_of};
use crate::player::Player;
use crate::tuning::SwimTuning;

/// Highest a solid face may rise above a swim surface in front of it in
/// an auto-scroll zone, tiles (D-084, `levels lint`): the weakest form's
/// held jump-out (big Resi: feet ~45 px above the water line, Normal ~53)
/// must land on its top. The feel test
/// `jump_out_heights_bound_the_squeeze_face_over_swim` keeps it honest.
pub const MAX_FACE_TILES: i32 = 2;

/// The player's chest point: her hitbox centre x, `chest_px` below its top.
pub fn chest(p: &Player, t: &SwimTuning) -> (Fx, Fx) {
    (p.x + p.w.div_int(2), p.y + Fx::from_px(t.chest_px))
}

/// The surface (y of the top edge of the topmost swim tile) of the swim
/// water at the point `(x, y)`, or `None` if the point is not in swim water.
pub fn surface(map: &TileMap, x: Fx, y: Fx) -> Option<Fx> {
    let (col, mut row) = (tile_of(x), tile_of(y));
    if map.kind_at(col, row) != TileKind::Swim {
        return None;
    }
    while map.kind_at(col, row - 1) == TileKind::Swim {
        row -= 1;
    }
    Some(TILE.mul_int(row))
}

/// The surface over the player's chest if she is swimming (see the module
/// docs), else `None`.
pub fn swim_surface(p: &Player, map: &TileMap, t: &SwimTuning) -> Option<Fx> {
    let (x, y) = chest(p, t);
    surface(map, x, y)
}

/// True if a chest at `y` under the surface `surface` is at the surface
/// (less than `surface_band` px below it).
pub fn at_surface(y: Fx, surface: Fx, t: &SwimTuning) -> bool {
    y - surface < Fx::from_px(t.surface_band)
}

/// The swim lint (`levels lint`, D-076):
/// - every column of swim water ends on a solid tile inside the level (the
///   lake bed: nobody sinks out of the level);
/// - inside `zone` (columns `from..to`, both inclusive: the columns an
///   auto-scroll camera shows while it scrolls), no solid tile sits directly
///   on a swim surface (a surface swimmer would be blocked or squeezed
///   there).
///
/// `levels lint` passes the columns an auto-scroll level's camera shows
/// while it scrolls (`None` without a zone).
pub fn problems(level: &Level, zone: Option<(i32, i32)>) -> Vec<String> {
    let kind = |c: i32, r: i32| level.tile(c, r).map(|t| t.kind);
    let mut out = Vec::new();
    for col in 0..level.width {
        let mut row = 0;
        while row < level.height {
            if kind(col, row) != Some(TileKind::Swim) {
                row += 1;
                continue;
            }
            let top = row;
            while kind(col, row) == Some(TileKind::Swim) {
                row += 1;
            }
            match kind(col, row) {
                Some(TileKind::Solid) => {}
                below => out.push(format!(
                    "swim water at column {}, rows {}–{}: it must end on a solid tile (the lake bed), not on {} (D-076)",
                    col + 1,
                    top + 1,
                    row,
                    match below {
                        None => "the level's bottom".to_owned(),
                        Some(k) => format!("{k:?}").to_lowercase(),
                    }
                )),
            }
            let in_zone = zone.is_some_and(|(from, to)| (from..=to).contains(&col));
            if in_zone && top > 0 && kind(col, top - 1) == Some(TileKind::Solid) {
                out.push(format!(
                    "solid tile at row {}, column {} sits on a swim surface inside the auto-scroll zone (D-076)",
                    top,
                    col + 1
                ));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A level from rows of `.`, `#` (solid), `-` (one-way), `~` (swim
    /// water), `w` (lethal water), plus a player start in the first `.`.
    fn level(rows: &[&str]) -> Level {
        let mut grid = rows.join("\n");
        let first_air = grid.find('.').unwrap();
        grid.replace_range(first_air..first_air + 1, "P");
        let text = format!(
            "[meta]\nid = \"t\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"~\" = \"see_wasser\"\n\"w\" = \"wolfgangsee_water\"\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}\n'''\n"
        );
        Level::parse(&text).unwrap()
    }

    #[test]
    fn surface_is_the_top_of_the_swim_run() {
        let l = level(&["....", "#~~#", "#~~#", "#~~#", "####"]);
        let m = TileMap::from_level(&l);
        let px = |v: i32| Fx::from_px(v);
        assert_eq!(surface(&m, px(20), px(16)), Some(px(16)));
        assert_eq!(surface(&m, px(20), px(63)), Some(px(16)));
        assert_eq!(surface(&m, px(20), px(15)), None, "air above");
        assert_eq!(surface(&m, px(20), px(64)), None, "the bed");
        assert_eq!(surface(&m, px(4), px(20)), None, "solid");
        let t = crate::Tuning::parse(include_str!("../../../content/tuning.toml"))
            .unwrap()
            .swim;
        assert!(at_surface(px(16), px(16), &t));
        assert!(at_surface(px(23) + Fx(255), px(16), &t));
        assert!(
            !at_surface(px(24), px(16), &t),
            "8 px down is below the band"
        );
    }

    #[test]
    fn lint_wants_a_solid_bed_under_every_swim_column() {
        let ok = level(&["....", "#~~#", "#~~#", "####"]);
        assert!(problems(&ok, None).is_empty());
        let open = level(&["....", "#~~#", "#~~#", "#~.#"]);
        let p = problems(&open, None);
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(
            p[0].contains("column 2, rows 2–4") && p[0].contains("level's bottom"),
            "{p:?}"
        );
        assert!(p[1].contains("column 3") && p[1].contains("empty"), "{p:?}");
        let one_way = level(&["....", "#~~#", "#--#", "####"]);
        assert!(problems(&one_way, None)[0].contains("oneway"));
        let lethal = level(&["....", "#~~#", "#ww#", "####"]);
        assert!(problems(&lethal, None)[0].contains("water"));
    }

    #[test]
    fn lint_wants_no_solid_on_a_swim_surface_in_the_zone() {
        let l = level(&["..#.", "#~~~", "#~~~", "####"]);
        assert!(problems(&l, None).is_empty(), "outside a zone it's fine");
        assert!(problems(&l, Some((0, 1))).is_empty(), "column 3 not in it");
        let p = problems(&l, Some((2, 3)));
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(p[0].contains("row 1, column 3"), "{p:?}");
    }
}
