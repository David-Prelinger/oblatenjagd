//! The Durchkriechstein (D-057): when a crawl starts, and the level
//! geometry it needs.
//!
//! Legend: `"K" = { entity = "durchkriechstein", len = 4 }` (`len` 2..=6
//! tiles, default 4) in the tunnel's **first (leftmost) cell**; the tunnel
//! is that cell and the `len − 1` cells to its right. The boulder is drawn
//! by a `durchkriechstein` prop over solid (`invisible`) tiles; the entity
//! itself is not drawn. [`geometry_problems`] lists what `levels lint`
//! should reject.
//!
//! The crawl itself (scripted move, outcome) is in `game/power.rs`.

use crate::entity::{Entity, EntityKind};
use crate::input::InputFrame;
use crate::level::{EntitySpawn, Level, SpawnKind, TileKind};
use crate::player::{Facing, Player};
use crate::tuning::Tuning;

/// `Entity::state` of a Durchkriechstein.
pub mod stone {
    /// Not crawled through in this attempt.
    pub const UNUSED: u8 = 0;
    /// Crawled through (until the level resets: a death, a stage change).
    pub const USED: u8 = 1;
}

/// True if DOWN starts a crawl through `stone` this frame: the stone is
/// active and unused, DOWN is held and JUMP is not, and the player stands
/// on the ground at the tunnel floor's height, facing right, with her
/// hitbox's right edge at most `crawl.mouth_px` px left of the mouth
/// (the tunnel's left edge).
pub(crate) fn starts(p: &Player, stone: &Entity, input: InputFrame, t: &Tuning) -> bool {
    let mouth = stone.x;
    let right = p.x + p.w;
    stone.kind == EntityKind::Durchkriechstein
        && stone.active
        && !stone.removed
        && stone.state == stone::UNUSED
        && input.down()
        && !input.jump()
        && p.on_ground
        && !p.crawling()
        && p.facing == Facing::Right
        && p.y + p.h == stone.y + stone.h
        && right <= mouth
        && mouth - right <= crate::fx::Fx::from_px(t.crawl.mouth_px)
}

/// Geometry problems of a `durchkriechstein` spawn in `level` (empty =
/// fine), for `levels lint` (D-057: never a softlock):
/// - the tunnel cells (`len` from the spawn cell to the right) are empty
///   (`TileKind::Empty`), with solid tiles directly above and below each;
/// - the mouth (the cell left of the tunnel) and the row above it are
///   empty, with a floor (solid or one-way) under the mouth: Resi stands
///   at the tunnel floor's height;
/// - the exit (the cell right of the tunnel) and the row above it are
///   empty (2 rows of headroom for a big Resi standing up), with a floor
///   under it.
///
/// Every message starts with "durchkriechstein at row R, column C"
/// (1-based, like the parser's messages).
pub fn geometry_problems(level: &Level, s: &EntitySpawn) -> Vec<String> {
    let mut out = Vec::new();
    if s.kind != SpawnKind::Durchkriechstein {
        return out;
    }
    let at = format!(
        "durchkriechstein at row {}, column {}",
        s.row + 1,
        s.col + 1
    );
    let kind = |c: i32, r: i32| level.tile(c, r).map(|t| t.kind);
    let floor = |c: i32, r: i32| matches!(kind(c, r), Some(TileKind::Solid | TileKind::OneWay));
    let (row, len) = (s.row, i32::from(s.len));
    for c in s.col..s.col + len {
        if kind(c, row) != Some(TileKind::Empty) {
            out.push(format!(
                "{at}: tunnel cell (row {}, column {}) must be empty",
                row + 1,
                c + 1
            ));
        }
        for (r, what) in [(row - 1, "above"), (row + 1, "below")] {
            if kind(c, r) != Some(TileKind::Solid) {
                out.push(format!(
                    "{at}: the cell {what} the tunnel at column {} (row {}) must be solid",
                    c + 1,
                    r + 1
                ));
            }
        }
    }
    for (c, what) in [(s.col - 1, "mouth"), (s.col + len, "exit")] {
        for r in [row, row - 1] {
            if kind(c, r) != Some(TileKind::Empty) {
                out.push(format!(
                    "{at}: the {what} (column {}) needs 2 empty rows (row {} is not empty)",
                    c + 1,
                    r + 1
                ));
            }
        }
        if !floor(c, row + 1) {
            out.push(format!(
                "{at}: the {what} (column {}) needs a floor at the tunnel floor's height (row {})",
                c + 1,
                row + 2
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn level(rows: &[&str]) -> Level {
        let text = format!(
            "[meta]\nid = \"k\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\"-\" = \"oneway\"\n\"P\" = {{ entity = \"player_start\" }}\n\"K\" = {{ entity = \"durchkriechstein\", len = 3 }}\n[map]\ngrid = '''\n{}\n'''\n",
            rows.join("\n")
        );
        Level::parse(&text).unwrap()
    }

    fn problems(rows: &[&str]) -> Vec<String> {
        let l = level(rows);
        let s = l
            .entities
            .iter()
            .find(|s| s.kind == SpawnKind::Durchkriechstein)
            .unwrap();
        geometry_problems(&l, s)
    }

    #[test]
    fn a_good_stone_has_no_problems() {
        let p = problems(&[
            "........", //
            "...###..", "P..K....", "########",
        ]);
        assert!(p.is_empty(), "{p:?}");
        // One-way floors at the mouth and the exit are fine too.
        let p = problems(&[
            "........", //
            "...###..", "P..K....", "#.-###-#",
        ]);
        assert!(p.is_empty(), "{p:?}");
    }

    #[test]
    fn bad_geometry_is_reported() {
        // Open roof over the last tunnel cell, a block in the exit's upper
        // row, no floor at the mouth.
        let p = problems(&[
            "......#.", //
            "...##.#.", "P..K....", "##.#####",
        ]);
        let all = p.join("\n");
        assert!(
            all.contains("durchkriechstein at row 3, column 4: the cell above the tunnel at column 6 (row 2) must be solid"),
            "{all}"
        );
        assert!(
            all.contains("the exit (column 7) needs 2 empty rows (row 2 is not empty)"),
            "{all}"
        );
        assert!(
            all.contains("the mouth (column 3) needs a floor at the tunnel floor's height (row 4)"),
            "{all}"
        );
        assert_eq!(p.len(), 3, "{all}");
        // A blocked tunnel cell.
        let p = problems(&[
            "........", //
            "...###..", "P..K#...", "########",
        ]);
        assert!(
            p.iter()
                .any(|m| m.ends_with("tunnel cell (row 3, column 5) must be empty")),
            "{p:?}"
        );
    }

    #[test]
    fn len_defaults_to_4_and_is_2_to_6() {
        let src = |len: &str| {
            format!(
                "[meta]\nid = \"k\"\ntime_limit = 100\n[legend]\n\"P\" = {{ entity = \"player_start\" }}\n\"K\" = {{ entity = \"durchkriechstein\"{len} }}\n[map]\ngrid = '''\nPK\n'''\n"
            )
        };
        let l = Level::parse(&src("")).unwrap();
        assert_eq!(l.entities[1].len, 4);
        for (len, ok) in [(1, false), (2, true), (6, true), (7, false)] {
            let r = Level::parse(&src(&format!(", len = {len}")));
            assert_eq!(r.is_ok(), ok, "len {len}: {r:?}");
        }
        let err = Level::parse(&src(", dir = 1")).unwrap_err().to_string();
        assert!(err.contains("\"dir\" is not allowed"), "{err}");
    }
}
