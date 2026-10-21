//! Drawing the tile grid: `tile_<name>` sprites with autotiling, else
//! grey-box shapes per [`TileKind`].
//!
//! Autotiling (D-020): a tile sprite with a 16-frame animation `auto` is
//! drawn with frame = [`autotile_mask`] of its cell (bit 1 = the same tile
//! name above, 2 = right, 4 = below, 8 = left; cells outside the level count
//! as the same; names of one [`autotile_family`] count as the same, e.g.
//! `arch` on `arch_under`; a few [`one_way_join`]s count a neighbour as
//! the same for one side only, e.g. `kalkfels_bg` under any drawn tile,
//! while that tile keeps its own edges). Any other tile sprite shows its
//! first animation, animated by the game frame (e.g. water). The tile name `invisible` is
//! never drawn (solid, under props such as the Sphaera). Which look a tile
//! name has is resolved once in [`TileLooks::resolve`]; the mask is computed from the live grid
//! (`Game::map`, bricks break and boxes empty; bumped blocks are raised)
//! while drawing (4 lookups per visible tile, no allocation), so tiles that
//! change during play stay correct.

use jnr_artkit::atlas::{Atlas, AtlasAnim};
use jnr_core::level::TILE_PX;
use jnr_core::physics::TileMap;
use jnr_core::{Camera, Game, TileKind};

use crate::Framebuffer;
use crate::blit::blit_frame;
use crate::palette::{DIRNDL_RED, LEATHER_DARK, SKY_BLUE, STONE_GREY};

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

/// Tile name that is drawn as nothing at all (no sprite, no grey box).
pub const INVISIBLE_TILE: &str = "invisible";

/// The autotile family of a tile name: names of one family count as the
/// same neighbour for [`autotile_mask`] (in both directions). A small
/// explicit table; every other name is its own family.
/// - `arch_under` (the arch's lower row) joins `arch`.
/// - `marmor_punkt` (Stage 9's „Abstand halten" floor dot) joins the
///   `marmor` floor (art-director, M7 café art: else each dot shows the
///   floor's cream lip line under it; D-104 addendum).
pub(crate) fn autotile_family(name: &str) -> &str {
    match name {
        "arch_under" => "arch",
        "marmor_punkt" => "marmor",
        other => other,
    }
}

/// One-way autotile joins (art review, M3): the mask bits (directions,
/// seen from a `here` tile) in which a `neighbour` tile name counts as the
/// same, although the two are different families and `neighbour` does not
/// return the favour. Every other pair: 0.
/// - `kalkfels_bg` (the shaft's back wall) runs on behind a `steig_wall`
///   plank on every side; the plank still sees the wall as different and
///   keeps its end brackets. The same behind a `felssteg` deck (M6,
///   Strobl's Bürglstein walkway: the deck draws the joined cliff behind
///   itself); `felssteg` itself joins only `felssteg`.
/// - `kalkfels_bg` under any other drawn tile (`kalkfels`, a box, a brick,
///   a plank, any solid or one-way) counts it as the same above only, so
///   the open top's transparent notches show only against real air (no
///   name, or [`INVISIBLE_TILE`]). The other sides keep their designed
///   shading (the ragged left shadow column beside a shaft's side wall).
/// - `kalkfels` under a solid ground tile (`forest_path`, `meadow`,
///   `stone`, `ground`) counts it as the same above only, so the rock draws
///   no open-top lip and grass under the dirt.
/// - `kalkfels` above `kalkfels_bg` (a shaft entrance, a cave top) counts
///   it as the same below only, so the rock's ragged open bottom shows no
///   sky-coloured notches over the back wall.
pub(crate) fn one_way_join(here: &str, neighbour: &str) -> u8 {
    match (here, neighbour) {
        ("kalkfels_bg", "steig_wall" | "felssteg") => MASK_N | MASK_E | MASK_S | MASK_W,
        ("kalkfels_bg", nb) if !nb.is_empty() && nb != INVISIBLE_TILE => MASK_N,
        ("kalkfels", "forest_path" | "meadow" | "stone" | "ground") => MASK_N,
        ("kalkfels", "kalkfels_bg") => MASK_S,
        _ => 0,
    }
}

/// How each tile name of a level is drawn, indexed by
/// [`jnr_core::Tile::name`] (resolved once, see [`crate::RenderContext`]).
#[derive(Clone, Debug, Default)]
pub(crate) struct TileLooks<'a> {
    /// The sprite look (`None`: grey-box by [`TileKind`], unless hidden).
    pub(crate) looks: Vec<Option<TileLook<'a>>>,
    /// The name index of the first name of the same [`autotile_family`].
    pub(crate) family: Vec<u16>,
    /// Never drawn ([`INVISIBLE_TILE`]).
    pub(crate) hidden: Vec<bool>,
    /// [`one_way_join`] per pair of name indices, row-major
    /// (`joins[here * names + neighbour]`; empty: no joins).
    pub(crate) joins: Vec<u8>,
}

impl<'a> TileLooks<'a> {
    /// The looks of `names` (a level's `tile_names`; index 0 = no name):
    /// sprite `<prefix><name>` each.
    pub(crate) fn resolve(atlas: &'a Atlas, names: &[String], prefix: &str) -> TileLooks<'a> {
        let looks = names
            .iter()
            .map(|n| {
                if n.is_empty() || n == INVISIBLE_TILE {
                    None
                } else {
                    TileLook::resolve(atlas, &format!("{prefix}{n}"))
                }
            })
            .collect();
        let family = names
            .iter()
            .map(|n| {
                let f = autotile_family(n);
                names
                    .iter()
                    .position(|m| autotile_family(m) == f)
                    .unwrap_or(0) as u16
            })
            .collect();
        let hidden = names.iter().map(|n| n == INVISIBLE_TILE).collect();
        let joins = names
            .iter()
            .flat_map(|here| names.iter().map(move |nb| one_way_join(here, nb)))
            .collect();
        TileLooks {
            looks,
            family,
            hidden,
            joins,
        }
    }

    /// [`autotile_mask`] plus this level's [`one_way_join`]s.
    pub(crate) fn mask(&self, map: &TileMap, col: i32, row: i32) -> u8 {
        let mut mask = autotile_mask(map, &self.family, col, row);
        let (Some(here), n) = (map.tile(col, row), self.family.len()) else {
            return mask;
        };
        if self.joins.len() != n * n {
            return mask;
        }
        for (dc, dr, bit) in NEIGHBOURS {
            let Some(nb) = map.tile(col + dc, row + dr) else {
                continue;
            };
            let (h, b) = (usize::from(here.name), usize::from(nb.name));
            if h < n && b < n && self.joins[h * n + b] & bit != 0 {
                mask |= bit;
            }
        }
        mask
    }
}

/// The four neighbours (dc, dr) with their mask bit.
const NEIGHBOURS: [(i32, i32, u8); 4] = [
    (0, -1, MASK_N),
    (1, 0, MASK_E),
    (0, 1, MASK_S),
    (-1, 0, MASK_W),
];

/// The 4-neighbour mask of cell (col, row): a bit is set when that
/// neighbour has the same tile name (the same `family` entry, see
/// [`TileLooks::family`]; names past its end are their own family) or
/// lies outside the level.
pub fn autotile_mask(map: &TileMap, family: &[u16], col: i32, row: i32) -> u8 {
    let Some(here) = map.tile(col, row) else {
        return 0;
    };
    let fam = |name: u16| family.get(usize::from(name)).copied().unwrap_or(name);
    let same = |c: i32, r: i32| map.tile(c, r).is_none_or(|t| fam(t.name) == fam(here.name));
    let mut mask = 0;
    for (dc, dr, bit) in NEIGHBOURS {
        if same(col + dc, row + dr) {
            mask |= bit;
        }
    }
    mask
}

/// Tiles inside the camera view.
pub(crate) fn draw_tiles(
    fb: &mut Framebuffer,
    atlas: &Atlas,
    looks: &TileLooks,
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
            let name = usize::from(tile.name);
            if looks.hidden.get(name).copied().unwrap_or(false) {
                continue;
            }
            let x = col * TILE_PX - cam.x;
            // Bumped blocks are drawn raised (E1).
            let y = row * TILE_PX - cam.y - game.tile_bump_px(col, row);
            match looks.looks.get(name).copied().flatten() {
                Some(look) => {
                    let index = if look.auto {
                        usize::from(looks.mask(level, col, row))
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
                    TileKind::Hazard | TileKind::Water => {
                        fb.fill_rect(x, y, TILE_PX, TILE_PX, DIRNDL_RED)
                    }
                    // Swim water (D-076) until its art lands: clear blue,
                    // never the lethal red.
                    TileKind::Swim => fb.fill_rect(x, y, TILE_PX, TILE_PX, SKY_BLUE),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{self, A, B, image};
    use jnr_core::Level;

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
        assert_eq!(
            autotile_mask(&TileMap::from_level(&l), &[], 1, 1),
            MASK_E | MASK_S
        );
        // (2,1): N empty, E empty, S brick (other name), W ground.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), &[], 2, 1), MASK_W);
        // (1,2): N ground, E brick, S level edge (same), W empty.
        assert_eq!(
            autotile_mask(&TileMap::from_level(&l), &[], 1, 2),
            MASK_N | MASK_S
        );
        // (2,2) brick: N ground, E empty, S edge, W ground.
        assert_eq!(autotile_mask(&TileMap::from_level(&l), &[], 2, 2), MASK_S);
        // Corner cell (0,0), the player start, is empty (name 0) like its
        // neighbours (1,0) and (0,1); the outside counts as same.
        assert_eq!(
            autotile_mask(&TileMap::from_level(&l), &[], 0, 0),
            MASK_N | MASK_E | MASK_S | MASK_W
        );
        // (3,1) empty: N, S empty, E outside, W ground.
        assert_eq!(
            autotile_mask(&TileMap::from_level(&l), &[], 3, 1),
            MASK_N | MASK_E | MASK_S
        );
        assert_eq!(
            autotile_mask(&TileMap::from_level(&l), &[], -1, 0),
            0,
            "outside the level"
        );
        // The player start above is an empty cell, not ground.
        let full = level("\"#\" = \"ground\"\n", "P\n#\n");
        assert_eq!(
            autotile_mask(&TileMap::from_level(&full), &[], 0, 1),
            MASK_E | MASK_S | MASK_W
        );
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
        let looks = TileLooks {
            looks: vec![None, Some(look)],
            ..TileLooks::default()
        };
        let mut fb = testutil::fb();
        draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, 0);
        // (1,1): only E -> frame 2, marker at x 16 + 2; (2,1): only W -> frame 8.
        assert_eq!(fb.get(16 + 2, 16), Some(A));
        assert_eq!(fb.get(16 + 3, 16), Some(B));
        assert_eq!(fb.get(32 + 8, 16), Some(A));
        assert_eq!(fb.get(32 + 2, 16), Some(B));
    }

    #[test]
    fn arch_and_arch_under_are_one_autotile_family() {
        assert_eq!(autotile_family("arch_under"), "arch");
        assert_eq!(autotile_family("arch"), "arch");
        assert_eq!(autotile_family("facade"), "facade");
        // An arch row on an arch_under row, ground beside them.
        let l = level(
            "\"A\" = \"arch\"\n\"U\" = \"arch_under\"\n\"#\" = \"ground\"\n",
            "P...\n.AA#\n.UU#\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (1,1) arch: E arch, S arch_under (same family), N/W empty.
        assert_eq!(autotile_mask(&map, &looks.family, 1, 1), MASK_E | MASK_S);
        // (2,2) arch_under: N arch (same family), W arch_under, E ground, S edge.
        assert_eq!(
            autotile_mask(&map, &looks.family, 2, 2),
            MASK_N | MASK_W | MASK_S
        );
        // Without the families the names differ.
        assert_eq!(autotile_mask(&map, &[], 1, 1), MASK_E);
    }

    #[test]
    fn marmor_punkt_joins_the_marmor_floor() {
        assert_eq!(autotile_family("marmor_punkt"), "marmor");
        assert_eq!(autotile_family("marmor"), "marmor");
        // A marble floor two rows deep with a dot in its top row, a
        // vitrine beside it.
        let l = level(
            "\"m\" = \"marmor\"\n\"p\" = \"marmor_punkt\"\n\"V\" = \"vitrine\"\n",
            "P....\n.mpmV\n.mmmV\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (2,1) the dot: marble on both sides and below, open top.
        assert_eq!(
            autotile_mask(&map, &looks.family, 2, 1),
            MASK_E | MASK_S | MASK_W
        );
        // The marble beside it sees it as marble (E / W), the marble
        // under it sees it above (N): no lip line under the dot.
        assert_eq!(autotile_mask(&map, &looks.family, 1, 1), MASK_E | MASK_S);
        assert_eq!(autotile_mask(&map, &looks.family, 3, 1), MASK_S | MASK_W);
        assert_eq!(
            autotile_mask(&map, &looks.family, 2, 2),
            MASK_N | MASK_E | MASK_S | MASK_W
        );
        // Without the families the dot is a different tile.
        assert_eq!(autotile_mask(&map, &[], 2, 1), 0);
        assert_eq!(autotile_mask(&map, &[], 1, 1), MASK_S);
    }

    #[test]
    fn kalkfels_bg_joins_a_steig_wall_plank_one_way() {
        assert_eq!(one_way_join("kalkfels_bg", "steig_wall"), 15);
        assert_eq!(one_way_join("steig_wall", "kalkfels_bg"), 0);
        // Back wall around a 2-tile plank in a shaft.
        let l = level(
            "\":\" = \"kalkfels_bg\"\n\"w\" = \"steig_wall\"\n",
            "P....\n.:::.\n.:ww.\n.:::.\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (2,1) wall above the plank: S is the plank, counted as wall.
        assert_eq!(autotile_mask(&map, &looks.family, 2, 1), MASK_E | MASK_W);
        assert_eq!(looks.mask(&map, 2, 1), MASK_E | MASK_S | MASK_W);
        // (1,2) wall left of the plank: E joins too.
        assert_eq!(looks.mask(&map, 1, 2), MASK_N | MASK_E | MASK_S);
        // The plank keeps its ends: (2,2) sees only the other plank (E).
        assert_eq!(looks.mask(&map, 2, 2), MASK_E);
        assert_eq!(looks.mask(&map, 3, 2), MASK_W);
    }

    #[test]
    fn kalkfels_bg_joins_a_felssteg_deck_on_every_side_and_the_deck_only_itself() {
        assert_eq!(one_way_join("kalkfels_bg", "felssteg"), 15);
        for nb in ["kalkfels_bg", "kalkfels", "steig_wall", "steig", ""] {
            assert_eq!(one_way_join("felssteg", nb), 0, "{nb}");
        }
        // A 2-tile deck bolted to the cliff, cliff all around, a plank
        // right of the deck.
        let l = level(
            "\":\" = \"kalkfels_bg\"\n\"=\" = \"felssteg\"\n\"w\" = \"steig_wall\"\n",
            "P.....\n.::::.\n.:==w.\n.::::.\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (2,1) cliff above the deck: S joined.
        assert_eq!(looks.mask(&map, 2, 1), MASK_E | MASK_S | MASK_W);
        // (1,2) cliff left of the deck: E joined.
        assert_eq!(looks.mask(&map, 1, 2), MASK_N | MASK_E | MASK_S);
        // (3,3) cliff below the deck (the level's bottom row, S counts as
        // the same): N joined.
        assert_eq!(looks.mask(&map, 3, 3), MASK_N | MASK_E | MASK_S | MASK_W);
        assert_eq!(
            autotile_mask(&map, &looks.family, 3, 3),
            MASK_E | MASK_S | MASK_W
        );
        // The deck sees only the deck: (2,2) E, (3,2) W; the cliff around
        // it and the plank beside it stay different.
        assert_eq!(looks.mask(&map, 2, 2), MASK_E);
        assert_eq!(looks.mask(&map, 3, 2), MASK_W);
    }

    #[test]
    fn kalkfels_joins_kalkfels_bg_below_it_only() {
        assert_eq!(one_way_join("kalkfels", "kalkfels_bg"), MASK_S);
        // Shaft entrance: a rock lintel over the back wall, sky above.
        let l = level(
            "\"#\" = \"kalkfels\"\n\":\" = \"kalkfels_bg\"\n",
            "P....\n.###.\n.#:#.\n.#:#.\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (2,1) lintel: S is the back wall, joined (no notches); N is sky
        // and keeps its jagged open top.
        assert_eq!(looks.mask(&map, 2, 1), MASK_E | MASK_S | MASK_W);
        // (1,1) corner: sky N and W stay open.
        assert_eq!(looks.mask(&map, 1, 1), MASK_E | MASK_S);
        // (1,2) shaft side wall: the back wall beside it is not joined.
        assert_eq!(looks.mask(&map, 1, 2), MASK_N | MASK_S);
        // (2,2) back wall: rock above joins (no notches); the rock beside
        // it does not, so the wall keeps its W shadow column.
        assert_eq!(looks.mask(&map, 2, 2), MASK_N | MASK_S);
    }

    #[test]
    fn kalkfels_bg_joins_any_drawn_tile_above_it_only() {
        for nb in [
            "kalkfels",
            "steig",
            "oblaten_box",
            "oblaten_box_empty",
            "brick",
            "ground",
        ] {
            assert_eq!(one_way_join("kalkfels_bg", nb), MASK_N, "{nb}");
            assert_eq!(one_way_join(nb, "kalkfels_bg") & !MASK_S, 0, "{nb}");
        }
        assert_eq!(one_way_join("kalkfels_bg", ""), 0);
        assert_eq!(one_way_join("kalkfels_bg", INVISIBLE_TILE), 0);
        // Back wall under rock, a box, a brick, a ledge and an invisible
        // tile; ledge, rock and air beside it.
        let l = level(
            "\":\" = \"kalkfels_bg\"\n\"#\" = \"kalkfels\"\n\
             \"?\" = { tile = \"oblaten_box\", contains = \"oblate\" }\n\
             \"B\" = \"brick\"\n\"=\" = \"steig\"\n\"i\" = \"invisible\"\n",
            "P.....\n.?B.#.\n.::=:.\n.:::#i\n.:..::\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (1,2) under the box: N joined; W is air and stays open.
        assert_eq!(looks.mask(&map, 1, 2), MASK_N | MASK_E | MASK_S);
        // (2,2) under the brick: N joined; the ledge beside it is not.
        assert_eq!(looks.mask(&map, 2, 2), MASK_N | MASK_S | MASK_W);
        // (4,2) under rock: N joined; the ledge (W), air (E) and rock (S)
        // stay open.
        assert_eq!(looks.mask(&map, 4, 2), MASK_N);
        // (3,3) under the ledge: N joined; rock beside it keeps E open.
        assert_eq!(looks.mask(&map, 3, 3), MASK_N | MASK_W);
        // (4,4) under rock: N joined, W air open.
        assert_eq!(looks.mask(&map, 4, 4), MASK_N | MASK_E | MASK_S);
        // (5,4) under an invisible tile, which counts as air: N open.
        assert_eq!(looks.mask(&map, 5, 4), MASK_E | MASK_S | MASK_W);
        // The box still sees the wall below as different.
        assert_eq!(looks.mask(&map, 1, 1), 0);
    }

    #[test]
    fn kalkfels_joins_solid_ground_above_it_only() {
        for ground in ["forest_path", "meadow", "stone", "ground"] {
            assert_eq!(one_way_join("kalkfels", ground), MASK_N, "{ground}");
            assert_eq!(one_way_join(ground, "kalkfels"), 0, "{ground}");
        }
        assert_eq!(one_way_join("kalkfels", "steig"), 0);
        // Path on rock, rock beside the path's column, path again beside it.
        let l = level(
            "\"#\" = \"kalkfels\"\n\"f\" = \"forest_path\"\n",
            "P....\n.ff#.\n.##f.\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let map = TileMap::from_level(&l);
        // (1,2) rock under the path: N joins (no lip); E rock; S level edge.
        assert_eq!(looks.mask(&map, 1, 2), MASK_N | MASK_E | MASK_S);
        // (2,2): N path, W rock, E path (only N joins), S edge.
        assert_eq!(looks.mask(&map, 2, 2), MASK_N | MASK_W | MASK_S);
        // The path sees the rock below as different.
        assert_eq!(looks.mask(&map, 1, 1), MASK_E);
        // (3,1) rock with air above and path below: open on every side.
        assert_eq!(looks.mask(&map, 3, 1), 0);
    }

    #[test]
    fn invisible_tiles_are_solid_but_never_drawn() {
        let l = level("\"i\" = \"invisible\"\n\"#\" = \"solid\"\n", "P..\ni#.\n");
        assert_eq!(l.tile(0, 1).unwrap().kind, TileKind::Solid);
        // Even with a `tile_invisible` sprite in the atlas.
        let atlas = testutil::atlas(&[testutil::sprite(
            "tile_invisible",
            (0, 0),
            false,
            vec![("tile", 0, vec![marked(0)])],
        )]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let mut fb = testutil::fb();
        let before = fb.get(4, 20);
        draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, 0);
        assert_eq!(fb.get(4, 20), before, "no sprite, no grey box");
        assert_eq!(fb.get(0, 16), before);
        assert_eq!(fb.get(20, 20), Some(STONE_GREY), "the solid next to it");
    }

    #[test]
    fn steg_pfahl_autotiles_on_its_own_name_vertically() {
        // A pier pile under a deck end: surface look at the top (no pile
        // above), deep water below it, the gravel heap above the bed (no
        // pile below). Its 16 frames ignore E/W, so the swim water beside
        // it changes nothing: no family rule needed.
        let l = level(
            "\"-\" = \"steg\"\n\"~\" = \"see_wasser\"\n\"t\" = \"see_wasser_tief\"\n\"|\" = \"steg_pfahl\"\n\"#\" = \"seegrund\"\n",
            "P...\n.--.\n~|~~\nt|tt\nt|tt\n####\n",
        );
        let m = TileMap::from_level(&l);
        let ns = |r: i32| autotile_mask(&m, &[], 1, r) & (MASK_N | MASK_S);
        assert_eq!(ns(2), MASK_S, "top: open above (the deck)");
        assert_eq!(ns(3), MASK_N | MASK_S, "middle");
        assert_eq!(ns(4), MASK_N, "bottom: the bed below");
    }

    #[test]
    fn cafe_tiles_draw_as_grey_boxes_by_kind_until_the_art_lands() {
        // M7 (E48): no `tile_<name>` sprites yet; the five solids fill their
        // cell in stone grey, the chandelier ring is a one-way strip on top.
        let l = level(
            "\"m\" = \"marmor\"\n\"d\" = \"marmor_punkt\"\n\"s\" = \"stuck\"\n\"v\" = \"vitrine\"\n\"t\" = \"theke\"\n\"l\" = \"luster\"\n",
            "Psl\nvt.\nmdm\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let mut fb = testutil::fb();
        let air = fb.get(36, 12);
        draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, 0);
        for (x, y) in [(20, 8), (4, 24), (20, 24), (4, 40), (20, 40), (36, 40)] {
            assert_eq!(fb.get(x, y), Some(STONE_GREY), "solid at {x},{y}");
        }
        assert_eq!(fb.get(36, 1), Some(LEATHER_DARK), "luster: one-way top");
        assert_eq!(fb.get(36, 12), air, "luster: open below its top");
    }

    #[test]
    fn swim_water_grey_box_is_blue_not_the_lethal_red() {
        let l = level(
            "\"~\" = \"see_wasser\"\n\"t\" = \"see_wasser_tief\"\n\"|\" = \"steg_pfahl\"\n\"w\" = \"wolfgangsee_water\"\n\"#\" = \"solid\"\n",
            "P...\n~|w.\nt|w.\n####\n",
        );
        let atlas = testutil::atlas(&[]);
        let looks = TileLooks::resolve(&atlas, &l.tile_names, "tile_");
        let mut fb = testutil::fb();
        draw_tiles(&mut fb, &atlas, &looks, &game(&l), Camera { x: 0, y: 0 }, 0);
        for (x, y) in [(4, 20), (4, 36), (20, 20), (20, 36)] {
            assert_eq!(fb.get(x, y), Some(SKY_BLUE), "swim at {x},{y}");
        }
        assert_eq!(fb.get(36, 20), Some(DIRNDL_RED), "lethal water");
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
        let looks = TileLooks {
            looks: vec![None, Some(water)],
            ..TileLooks::default()
        };
        let draw = |tick| {
            let mut fb = testutil::fb();
            draw_tiles(
                &mut fb,
                &atlas,
                &looks,
                &game(&l),
                Camera { x: 0, y: 0 },
                tick,
            );
            (fb.get(0, 16), fb.get(5, 16))
        };
        assert_eq!(draw(0), (Some(A), Some(B)));
        assert_eq!(draw(3), (Some(A), Some(B)));
        assert_eq!(draw(4), (Some(B), Some(A)));
        assert_eq!(draw(8), (Some(A), Some(B)), "loops");
    }
}
