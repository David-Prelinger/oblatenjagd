//! The `.level` format: TOML `[meta]`, `[legend]` and an ASCII `[map].grid`.
//! See `docs/architecture.md` → "Level".

use serde::Deserialize;

use crate::ParseError;

/// Size of one tile in pixels.
pub const TILE_PX: i32 = 16;

/// What a tile does in the simulation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TileKind {
    /// Air.
    #[default]
    Empty,
    /// Blocks from all sides.
    Solid,
    /// Blocks only from above (the player can jump through from below).
    OneWay,
    /// Kills the player on touch.
    Hazard,
}

impl TileKind {
    /// Simulation behaviour of a named tile from the legend. Grey-box names
    /// (`solid`, `oneway`, `hazard`, `empty`) and the art tile names known so
    /// far map to a kind; unknown names return `None`.
    pub fn from_tile_name(name: &str) -> Option<TileKind> {
        match name {
            "empty" => Some(TileKind::Empty),
            "solid" | "ground" | "brick" => Some(TileKind::Solid),
            "oneway" | "oneway_sign" => Some(TileKind::OneWay),
            "hazard" | "salzach_water" => Some(TileKind::Hazard),
            _ => None,
        }
    }

    /// Small stable number used by `state_hash`.
    pub const fn id(self) -> u8 {
        match self {
            TileKind::Empty => 0,
            TileKind::Solid => 1,
            TileKind::OneWay => 2,
            TileKind::Hazard => 3,
        }
    }
}

/// One grid cell.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tile {
    /// Simulation behaviour.
    pub kind: TileKind,
    /// Index into [`Level::tile_names`] (the look, for the renderer); 0 = none.
    pub name: u16,
}

/// Kinds of entity spawns known so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EntityKind {
    /// Where the player starts and respawns. Exactly one per level.
    PlayerStart,
    /// Touching it completes the stage.
    Goal,
}

impl EntityKind {
    /// Entity kind for a legend name; `None` if unknown.
    pub fn from_name(name: &str) -> Option<EntityKind> {
        match name {
            "player_start" => Some(EntityKind::PlayerStart),
            "goal" | "goal_maibaum" => Some(EntityKind::Goal),
            _ => None,
        }
    }
}

/// An entity placed in the grid (its cell counts as empty tile).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntitySpawn {
    /// What spawns.
    pub kind: EntityKind,
    /// Grid column (0 = left).
    pub col: i32,
    /// Grid row (0 = top).
    pub row: i32,
}

/// The `[meta]` table.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelMeta {
    /// Level id, e.g. `"salzburg"`.
    pub id: String,
    /// Text key of the stage title (resolved by the renderer).
    #[serde(default)]
    pub title_key: Option<String>,
    /// Time limit in seconds.
    #[serde(default)]
    pub time_limit: Option<u32>,
    /// Background layer ids, far to near.
    #[serde(default)]
    pub background: Vec<String>,
    /// Music id.
    #[serde(default)]
    pub music: Option<String>,
}

/// A parsed level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Level {
    /// The `[meta]` table.
    pub meta: LevelMeta,
    /// Width in tiles.
    pub width: i32,
    /// Height in tiles.
    pub height: i32,
    /// `width * height` tiles, row-major, row 0 at the top.
    pub tiles: Vec<Tile>,
    /// Tile look names referenced by [`Tile::name`]; index 0 is `""` (none).
    pub tile_names: Vec<String>,
    /// Entity spawns in grid order (top-to-bottom, left-to-right).
    pub entities: Vec<EntitySpawn>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevel {
    meta: LevelMeta,
    #[serde(default)]
    legend: toml::Table,
    map: RawMap,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMap {
    grid: String,
}

/// What a legend character stands for.
#[derive(Clone, Copy)]
enum LegendEntry {
    Tile(Tile),
    Entity(EntityKind),
}

impl Level {
    /// Parses `.level` text. `.` is always empty air; every other grid
    /// character must be in `[legend]`.
    pub fn parse(text: &str) -> Result<Level, ParseError> {
        let raw: RawLevel = toml::from_str(text)
            .map_err(|e| ParseError::new(format!("level: {}", e.message().trim())))?;

        let mut tile_names = vec![String::new()];
        let mut legend: Vec<(char, LegendEntry)> = Vec::new();
        for (key, value) in &raw.legend {
            let mut chars = key.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return Err(ParseError::new(format!(
                    "legend key \"{key}\" must be exactly one character"
                )));
            };
            if c == '.' {
                return Err(ParseError::new(
                    "legend must not redefine '.' (always empty)",
                ));
            }
            let entry = parse_legend_value(key, value, &mut tile_names)?;
            legend.push((c, entry));
        }

        let rows: Vec<&str> = raw
            .map
            .grid
            .lines()
            .map(|l| l.trim_end_matches('\r'))
            .collect();
        let rows: Vec<&str> = match rows.iter().rposition(|r| !r.is_empty()) {
            Some(last) => rows[..=last].to_vec(),
            None => Vec::new(),
        };
        if rows.is_empty() {
            return Err(ParseError::new("map.grid is empty"));
        }
        let width = rows[0].chars().count();
        if width == 0 {
            return Err(ParseError::new("map.grid row 1 is empty"));
        }
        let mut tiles = Vec::with_capacity(width * rows.len());
        let mut entities = Vec::new();
        for (r, row) in rows.iter().enumerate() {
            let n = row.chars().count();
            if n != width {
                return Err(ParseError::new(format!(
                    "map.grid row {} has {n} columns, expected {width} (like row 1)",
                    r + 1
                )));
            }
            for (col, c) in row.chars().enumerate() {
                let entry = if c == '.' {
                    LegendEntry::Tile(Tile::default())
                } else {
                    match legend.iter().find(|(k, _)| *k == c) {
                        Some((_, e)) => *e,
                        None => {
                            return Err(ParseError::new(format!(
                                "unknown grid character '{c}' at row {}, column {} (not in [legend])",
                                r + 1,
                                col + 1
                            )));
                        }
                    }
                };
                match entry {
                    LegendEntry::Tile(t) => tiles.push(t),
                    LegendEntry::Entity(kind) => {
                        tiles.push(Tile::default());
                        entities.push(EntitySpawn {
                            kind,
                            col: col as i32,
                            row: r as i32,
                        });
                    }
                }
            }
        }

        let starts = entities
            .iter()
            .filter(|e| e.kind == EntityKind::PlayerStart)
            .count();
        if starts != 1 {
            return Err(ParseError::new(format!(
                "level needs exactly one player_start, found {starts}"
            )));
        }

        Ok(Level {
            meta: raw.meta,
            width: width as i32,
            height: rows.len() as i32,
            tiles,
            tile_names,
            entities,
        })
    }

    /// The tile at a grid cell, or `None` outside the grid.
    pub fn tile(&self, col: i32, row: i32) -> Option<Tile> {
        if col < 0 || row < 0 || col >= self.width || row >= self.height {
            return None;
        }
        Some(self.tiles[(row * self.width + col) as usize])
    }

    /// The (only) player start spawn.
    pub fn player_start(&self) -> EntitySpawn {
        self.entities
            .iter()
            .copied()
            .find(|e| e.kind == EntityKind::PlayerStart)
            .expect("Level::parse guarantees one player_start")
    }
}

fn parse_legend_value(
    key: &str,
    value: &toml::Value,
    tile_names: &mut Vec<String>,
) -> Result<LegendEntry, ParseError> {
    let tile_name = match value {
        toml::Value::String(name) => name.as_str(),
        toml::Value::Table(t) => {
            if let Some(k) = t.keys().find(|k| *k != "tile" && *k != "entity") {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": field \"{k}\" is not supported yet"
                )));
            }
            match (t.get("tile"), t.get("entity")) {
                (Some(toml::Value::String(name)), None) => name.as_str(),
                (None, Some(toml::Value::String(name))) => {
                    return match EntityKind::from_name(name) {
                        Some(kind) => Ok(LegendEntry::Entity(kind)),
                        None => Err(ParseError::new(format!(
                            "legend \"{key}\": unknown entity \"{name}\""
                        ))),
                    };
                }
                _ => {
                    return Err(ParseError::new(format!(
                        "legend \"{key}\": needs exactly one of tile = \"...\" or entity = \"...\""
                    )));
                }
            }
        }
        _ => {
            return Err(ParseError::new(format!(
                "legend \"{key}\": expected a tile name or a table"
            )));
        }
    };
    let Some(kind) = TileKind::from_tile_name(tile_name) else {
        return Err(ParseError::new(format!(
            "legend \"{key}\": unknown tile \"{tile_name}\""
        )));
    };
    let name = match tile_names.iter().position(|n| n == tile_name) {
        Some(i) => i,
        None => {
            tile_names.push(tile_name.to_owned());
            tile_names.len() - 1
        }
    };
    Ok(LegendEntry::Tile(Tile {
        kind,
        name: name as u16,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SMALL: &str = r##"
[meta]
id = "small"
time_limit = 100

[legend]
"#" = "solid"
"-" = "oneway"
"^" = { tile = "hazard" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }

[map]
grid = '''
......G
.P.--..
###^###
'''
"##;

    #[test]
    fn parses_small_level() {
        let l = Level::parse(SMALL).unwrap();
        assert_eq!(l.meta.id, "small");
        assert_eq!(l.meta.time_limit, Some(100));
        assert_eq!((l.width, l.height), (7, 3));
        assert_eq!(l.tile(0, 2).unwrap().kind, TileKind::Solid);
        assert_eq!(l.tile(3, 2).unwrap().kind, TileKind::Hazard);
        assert_eq!(l.tile(3, 1).unwrap().kind, TileKind::OneWay);
        assert_eq!(
            l.tile(1, 1).unwrap().kind,
            TileKind::Empty,
            "entity cell is empty"
        );
        assert_eq!(l.tile(7, 0), None);
        assert_eq!(l.tile(-1, 0), None);
        let names: Vec<&str> = l.tile_names.iter().map(|s| s.as_str()).collect();
        assert_eq!(names[l.tile(0, 2).unwrap().name as usize], "solid");
        assert_eq!(
            l.player_start(),
            EntitySpawn {
                kind: EntityKind::PlayerStart,
                col: 1,
                row: 1
            }
        );
        assert_eq!(
            l.entities,
            vec![
                EntitySpawn {
                    kind: EntityKind::Goal,
                    col: 6,
                    row: 0
                },
                EntitySpawn {
                    kind: EntityKind::PlayerStart,
                    col: 1,
                    row: 1
                },
            ]
        );
    }

    #[test]
    fn unknown_grid_char_reports_row_and_column() {
        let bad = SMALL.replace("###^###", "###^#x#");
        let err = Level::parse(&bad).unwrap_err().to_string();
        assert!(
            err.contains("'x'") && err.contains("row 3") && err.contains("column 6"),
            "{err}"
        );
    }

    #[test]
    fn other_errors() {
        let ragged = SMALL.replace("###^###", "###^##");
        assert!(
            Level::parse(&ragged)
                .unwrap_err()
                .to_string()
                .contains("row 3")
        );

        let no_start = SMALL.replace(".P.--..", "...--..");
        assert!(
            Level::parse(&no_start)
                .unwrap_err()
                .to_string()
                .contains("player_start")
        );

        let two_starts = SMALL.replace("......G", "P.....G");
        assert!(Level::parse(&two_starts).is_err());

        let bad_tile = SMALL.replace("\"solid\"", "\"lava_cake\"");
        assert!(
            Level::parse(&bad_tile)
                .unwrap_err()
                .to_string()
                .contains("lava_cake")
        );

        let bad_entity = SMALL.replace("\"goal\"", "\"unicorn\"");
        assert!(
            Level::parse(&bad_entity)
                .unwrap_err()
                .to_string()
                .contains("unicorn")
        );

        let unsupported = SMALL.replace(
            "{ tile = \"hazard\" }",
            "{ tile = \"hazard\", contains = \"x\" }",
        );
        assert!(
            Level::parse(&unsupported)
                .unwrap_err()
                .to_string()
                .contains("contains")
        );

        let long_key = SMALL.replace("\"#\" = \"solid\"", "\"##\" = \"solid\"");
        assert!(Level::parse(&long_key).is_err());

        let no_meta = SMALL.replace("[meta]\nid = \"small\"", "");
        assert!(Level::parse(&no_meta).is_err());
    }

    #[test]
    fn parses_testbed() {
        let l = Level::parse(include_str!("../../../content/levels/00_testbed.level")).unwrap();
        assert_eq!(l.meta.id, "testbed");
        assert_eq!(l.height, 14);
        assert!(l.width >= 100);
        assert!(l.entities.iter().any(|e| e.kind == EntityKind::Goal));
        for kind in [TileKind::Solid, TileKind::OneWay, TileKind::Hazard] {
            assert!(l.tiles.iter().any(|t| t.kind == kind), "{kind:?} missing");
        }
    }
}
