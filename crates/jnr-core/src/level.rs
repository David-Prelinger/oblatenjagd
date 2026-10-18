//! The `.level` format (v2): TOML `[meta]`, `[legend]` and an ASCII
//! `[map].grid`. See `docs/architecture.md` → "Level".
//!
//! Legend values are a tile name (`"ground"`), a tile table
//! (`{ tile = "oblaten_box", contains = "mozartkugel" }`) or an entity table
//! (`{ entity = "sign", text = "<key>", style = "bus" }`). The level is the
//! static start state; everything that changes while playing (broken bricks,
//! emptied boxes, entities) lives in `Game`.

use serde::Deserialize;

use crate::ParseError;

/// Size of one tile in pixels.
pub const TILE_PX: i32 = 16;

/// Tile name an Oblatenkiste (or a brick with contents) turns into once empty.
pub const EMPTY_BOX_TILE: &str = "oblaten_box_empty";

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
    /// Kills the player on touch (spikes, Salzach water).
    Hazard,
}

impl TileKind {
    /// Simulation behaviour of a named tile from the legend. Grey-box names
    /// (`solid`, `oneway`, `hazard`, `empty`) and the M1 art tile names of
    /// `docs/sprite-catalog.md` map to a kind; unknown names return `None`.
    pub fn from_tile_name(name: &str) -> Option<TileKind> {
        match name {
            "empty" | "railing_bg" => Some(TileKind::Empty),
            "solid" | "ground" | "meadow" | "rock" | "rampart" | "hedge" | "stone" | "bridge"
            | "railing" | "arch" | "brick" | "oblaten_box" | "oblaten_box_empty" => {
                Some(TileKind::Solid)
            }
            "oneway" | "oneway_sign" | "oneway_burger" | "oneway_wood" => Some(TileKind::OneWay),
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

/// How a solid tile reacts to being hit from below.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Block {
    /// Nothing happens.
    #[default]
    None,
    /// Brick: bumps (Normal) or breaks (Zuckerschock); with contents it
    /// behaves like a box.
    Brick,
    /// Oblatenkiste: bumps and releases its contents once.
    Box,
}

impl Block {
    /// Small stable number used by `state_hash`.
    pub const fn id(self) -> u8 {
        match self {
            Block::None => 0,
            Block::Brick => 1,
            Block::Box => 2,
        }
    }
}

/// What a brick or box releases (`contains = "..."`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Contents {
    /// Nothing (plain brick, empty box).
    #[default]
    None,
    /// One Oblate that pops out and is collected at once.
    Oblate,
    /// The Mozartkugel power-up.
    Mozartkugel,
    /// The Zaunerstollen 1-up.
    Zaunerstollen,
    /// One Oblate per hit, up to 10 or until 240 frames after the first hit
    /// (see `tuning.toml` `[items]`).
    Oblaten10,
}

impl Contents {
    /// The contents for a `contains` value; `None` if unknown.
    pub fn from_name(name: &str) -> Option<Contents> {
        match name {
            "oblate" => Some(Contents::Oblate),
            "mozartkugel" => Some(Contents::Mozartkugel),
            "zaunerstollen" => Some(Contents::Zaunerstollen),
            "oblaten_10" => Some(Contents::Oblaten10),
            _ => None,
        }
    }

    /// Small stable number used by `state_hash`.
    pub const fn id(self) -> u8 {
        match self {
            Contents::None => 0,
            Contents::Oblate => 1,
            Contents::Mozartkugel => 2,
            Contents::Zaunerstollen => 3,
            Contents::Oblaten10 => 4,
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
    /// Reaction to a hit from below.
    pub block: Block,
    /// What a hit releases.
    pub contains: Contents,
}

/// Kinds of entity spawns in a level (legend `entity = "..."`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SpawnKind {
    /// Where the player starts. Exactly one per level.
    PlayerStart,
    /// Touching it completes the stage (`goal` / `goal_maibaum`).
    Goal,
    /// Respawn point once the player's centre has passed it.
    Checkpoint,
    /// A static Oblate.
    Oblate,
    /// A decorative sign with a text key (`text`, `style`).
    Sign,
    /// Enemy `selfie_tourist`.
    SelfieTourist,
    /// Enemy `taube` (flying pigeon).
    Taube,
    /// Enemy `taube_sitting` (sits until the player comes close).
    TaubeSitting,
    /// Enemy `mozartkugel_verkaeufer` (throws Mozartkugeln).
    MozartkugelVerkaeufer,
    /// Enemy `fiaker` (charging carriage, not stompable).
    Fiaker,
    /// The Festungsbahn funicular car (moving platform).
    Festungsbahn,
}

impl SpawnKind {
    /// Every kind, in declaration order.
    pub const ALL: [SpawnKind; 11] = [
        SpawnKind::PlayerStart,
        SpawnKind::Goal,
        SpawnKind::Checkpoint,
        SpawnKind::Oblate,
        SpawnKind::Sign,
        SpawnKind::SelfieTourist,
        SpawnKind::Taube,
        SpawnKind::TaubeSitting,
        SpawnKind::MozartkugelVerkaeufer,
        SpawnKind::Fiaker,
        SpawnKind::Festungsbahn,
    ];

    /// The legend name (`entity = "<name>"`); also the catalog sprite name
    /// for the entities that have one.
    pub const fn name(self) -> &'static str {
        match self {
            SpawnKind::PlayerStart => "player_start",
            SpawnKind::Goal => "goal",
            SpawnKind::Checkpoint => "checkpoint",
            SpawnKind::Oblate => "oblate",
            SpawnKind::Sign => "sign",
            SpawnKind::SelfieTourist => "selfie_tourist",
            SpawnKind::Taube => "taube",
            SpawnKind::TaubeSitting => "taube_sitting",
            SpawnKind::MozartkugelVerkaeufer => "mozartkugel_verkaeufer",
            SpawnKind::Fiaker => "fiaker",
            SpawnKind::Festungsbahn => "festungsbahn",
        }
    }

    /// Spawn kind for a legend name; `None` if unknown.
    pub fn from_name(name: &str) -> Option<SpawnKind> {
        if name == "goal_maibaum" {
            return Some(SpawnKind::Goal);
        }
        SpawnKind::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// Board style of a sign (`style = "..."`, default `street`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignStyle {
    /// Street sign (`sign_street`).
    #[default]
    Street,
    /// Postbus / Haltestelle sign (`sign_bus`).
    Bus,
    /// Ortsende sign with the red diagonal bar (`sign_ortsende`).
    Ortsende,
}

impl SignStyle {
    /// The style for a `style` value; `None` if unknown.
    pub fn from_name(name: &str) -> Option<SignStyle> {
        match name {
            "street" => Some(SignStyle::Street),
            "bus" => Some(SignStyle::Bus),
            "ortsende" => Some(SignStyle::Ortsende),
            _ => None,
        }
    }

    /// The style name, as in the level file.
    pub const fn name(self) -> &'static str {
        match self {
            SignStyle::Street => "street",
            SignStyle::Bus => "bus",
            SignStyle::Ortsende => "ortsende",
        }
    }
}

/// An entity placed in the grid (its cell counts as empty tile).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EntitySpawn {
    /// What spawns.
    pub kind: SpawnKind,
    /// Grid column (0 = left).
    pub col: i32,
    /// Grid row (0 = top).
    pub row: i32,
    /// Text key (`text = "..."`, signs only).
    pub text: Option<String>,
    /// Board style (signs only; default street).
    pub style: SignStyle,
}

/// One parallax layer of `[meta].background`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundLayer {
    /// Sprite name of the layer.
    pub sprite: String,
    /// Scroll speed in 1/16 of the camera speed (0 = fixed, 16 = like the tiles).
    pub factor: u32,
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
    /// Time limit in seconds (required, 1..=999).
    pub time_limit: u32,
    /// Background layers, far to near.
    #[serde(default)]
    pub background: Vec<BackgroundLayer>,
    /// Music id.
    #[serde(default)]
    pub music: Option<String>,
    /// Smallest jump window (frames) the solver must measure on this level.
    #[serde(default)]
    pub min_jump_window: Option<u32>,
}

/// Largest `time_limit` (the HUD shows 3 digits).
pub const MAX_TIME_LIMIT: u32 = 999;
/// Largest background `factor` (2× camera speed, for foreground layers).
pub const MAX_BACKGROUND_FACTOR: u32 = 32;

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
    /// Contains [`EMPTY_BOX_TILE`] whenever a box or brick can turn into it.
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
#[derive(Clone)]
enum LegendEntry {
    Tile(Tile),
    Entity {
        kind: SpawnKind,
        text: Option<String>,
        style: SignStyle,
    },
}

impl Level {
    /// Parses `.level` text. `.` is always empty air; every other grid
    /// character must be in `[legend]`.
    pub fn parse(text: &str) -> Result<Level, ParseError> {
        let raw: RawLevel = toml::from_str(text)
            .map_err(|e| ParseError::new(format!("level: {}", e.to_string().trim())))?;
        validate_meta(&raw.meta)?;

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
        let needs_empty_box = legend.iter().any(|(_, e)| {
            matches!(e, LegendEntry::Tile(t) if t.block == Block::Box || t.contains != Contents::None)
        });
        if needs_empty_box {
            name_index(&mut tile_names, EMPTY_BOX_TILE);
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
                if c == '.' {
                    tiles.push(Tile::default());
                    continue;
                }
                let Some((_, entry)) = legend.iter().find(|(k, _)| *k == c) else {
                    return Err(ParseError::new(format!(
                        "unknown grid character '{c}' at row {}, column {} (not in [legend])",
                        r + 1,
                        col + 1
                    )));
                };
                match entry {
                    LegendEntry::Tile(t) => tiles.push(*t),
                    LegendEntry::Entity { kind, text, style } => {
                        tiles.push(Tile::default());
                        entities.push(EntitySpawn {
                            kind: *kind,
                            col: col as i32,
                            row: r as i32,
                            text: text.clone(),
                            style: *style,
                        });
                    }
                }
            }
        }

        let starts: Vec<&EntitySpawn> = entities
            .iter()
            .filter(|e| e.kind == SpawnKind::PlayerStart)
            .collect();
        if starts.len() != 1 {
            let at: Vec<String> = starts
                .iter()
                .map(|s| format!("row {}, column {}", s.row + 1, s.col + 1))
                .collect();
            return Err(ParseError::new(format!(
                "level needs exactly one player_start, found {}{}",
                starts.len(),
                if at.is_empty() {
                    String::new()
                } else {
                    format!(" (at {})", at.join("; "))
                }
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
    pub fn player_start(&self) -> &EntitySpawn {
        self.entities
            .iter()
            .find(|e| e.kind == SpawnKind::PlayerStart)
            .expect("Level::parse guarantees one player_start")
    }

    /// Index of [`EMPTY_BOX_TILE`] in [`Level::tile_names`]; 0 if the level
    /// has no box or brick that can empty.
    pub fn empty_box_name(&self) -> u16 {
        self.tile_names
            .iter()
            .position(|n| n == EMPTY_BOX_TILE)
            .unwrap_or(0) as u16
    }
}

fn validate_meta(meta: &LevelMeta) -> Result<(), ParseError> {
    if !(1..=MAX_TIME_LIMIT).contains(&meta.time_limit) {
        return Err(ParseError::new(format!(
            "meta.time_limit must be 1..={MAX_TIME_LIMIT} seconds, got {}",
            meta.time_limit
        )));
    }
    for (i, layer) in meta.background.iter().enumerate() {
        if layer.sprite.is_empty() {
            return Err(ParseError::new(format!(
                "meta.background[{i}]: sprite is empty"
            )));
        }
        if layer.factor > MAX_BACKGROUND_FACTOR {
            return Err(ParseError::new(format!(
                "meta.background[{i}] ({}): factor must be 0..={MAX_BACKGROUND_FACTOR} (1/16 of the camera speed), got {}",
                layer.sprite, layer.factor
            )));
        }
    }
    Ok(())
}

/// Index of `name` in `tile_names`, appending it if new.
fn name_index(tile_names: &mut Vec<String>, name: &str) -> u16 {
    match tile_names.iter().position(|n| n == name) {
        Some(i) => i as u16,
        None => {
            tile_names.push(name.to_owned());
            (tile_names.len() - 1) as u16
        }
    }
}

/// A string field of a legend table, if present.
fn string_field<'a>(
    key: &str,
    t: &'a toml::Table,
    field: &str,
) -> Result<Option<&'a str>, ParseError> {
    match t.get(field) {
        None => Ok(None),
        Some(toml::Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(ParseError::new(format!(
            "legend \"{key}\": field \"{field}\" must be a string"
        ))),
    }
}

fn parse_legend_value(
    key: &str,
    value: &toml::Value,
    tile_names: &mut Vec<String>,
) -> Result<LegendEntry, ParseError> {
    match value {
        toml::Value::String(name) => parse_tile(key, name, None, tile_names),
        toml::Value::Table(t) => {
            let tile = string_field(key, t, "tile")?;
            let entity = string_field(key, t, "entity")?;
            let allowed: &[&str] = match (tile, entity) {
                (Some(_), None) => &["tile", "contains"],
                (None, Some("sign")) => &["entity", "text", "style"],
                (None, Some(_)) => &["entity"],
                _ => {
                    return Err(ParseError::new(format!(
                        "legend \"{key}\": needs exactly one of tile = \"...\" or entity = \"...\""
                    )));
                }
            };
            if let Some(k) = t.keys().find(|k| !allowed.contains(&k.as_str())) {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": field \"{k}\" is not allowed here (allowed: {})",
                    allowed.join(", ")
                )));
            }
            match (tile, entity) {
                (Some(name), _) => {
                    parse_tile(key, name, string_field(key, t, "contains")?, tile_names)
                }
                (_, Some(name)) => parse_entity(key, name, t),
                _ => unreachable!("checked above"),
            }
        }
        _ => Err(ParseError::new(format!(
            "legend \"{key}\": expected a tile name or a table"
        ))),
    }
}

fn parse_tile(
    key: &str,
    name: &str,
    contains: Option<&str>,
    tile_names: &mut Vec<String>,
) -> Result<LegendEntry, ParseError> {
    let Some(kind) = TileKind::from_tile_name(name) else {
        return Err(ParseError::new(format!(
            "legend \"{key}\": unknown tile \"{name}\""
        )));
    };
    let block = match name {
        "brick" => Block::Brick,
        "oblaten_box" => Block::Box,
        _ => Block::None,
    };
    let contains = match contains {
        None if block == Block::Box => Contents::Oblate,
        None => Contents::None,
        Some(c) => {
            if block == Block::None {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": only brick and oblaten_box can have contains (tile \"{name}\")"
                )));
            }
            Contents::from_name(c).ok_or_else(|| {
                ParseError::new(format!(
                    "legend \"{key}\": unknown contains \"{c}\" (expected oblate, mozartkugel, zaunerstollen or oblaten_10)"
                ))
            })?
        }
    };
    Ok(LegendEntry::Tile(Tile {
        kind,
        name: name_index(tile_names, name),
        block,
        contains,
    }))
}

fn parse_entity(key: &str, name: &str, t: &toml::Table) -> Result<LegendEntry, ParseError> {
    let Some(kind) = SpawnKind::from_name(name) else {
        return Err(ParseError::new(format!(
            "legend \"{key}\": unknown entity \"{name}\""
        )));
    };
    let mut text = None;
    let mut style = SignStyle::default();
    if kind == SpawnKind::Sign {
        match string_field(key, t, "text")? {
            Some(s) if !s.is_empty() => text = Some(s.to_owned()),
            _ => {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": a sign needs text = \"<text key>\""
                )));
            }
        }
        if let Some(s) = string_field(key, t, "style")? {
            style = SignStyle::from_name(s).ok_or_else(|| {
                ParseError::new(format!(
                    "legend \"{key}\": unknown sign style \"{s}\" (expected street, bus or ortsende)"
                ))
            })?;
        }
    }
    Ok(LegendEntry::Entity { kind, text, style })
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

    fn spawn(kind: SpawnKind, col: i32, row: i32) -> EntitySpawn {
        EntitySpawn {
            kind,
            col,
            row,
            text: None,
            style: SignStyle::Street,
        }
    }

    #[test]
    fn parses_small_level() {
        let l = Level::parse(SMALL).unwrap();
        assert_eq!(l.meta.id, "small");
        assert_eq!(l.meta.time_limit, 100);
        assert_eq!(l.meta.min_jump_window, None);
        assert!(l.meta.background.is_empty());
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
        assert_eq!(l.empty_box_name(), 0, "no boxes, no empty-box name");
        assert_eq!(*l.player_start(), spawn(SpawnKind::PlayerStart, 1, 1));
        assert_eq!(
            l.entities,
            vec![
                spawn(SpawnKind::Goal, 6, 0),
                spawn(SpawnKind::PlayerStart, 1, 1)
            ]
        );
    }

    const V2: &str = r##"
[meta]
id = "v2"
time_limit = 300
min_jump_window = 6
background = [{ sprite = "bg_sky_salzburg", factor = 0 }, { sprite = "bg_rooftops", factor = 8 }]

[legend]
"#" = "ground"
"B" = "brick"
"T" = { tile = "brick", contains = "oblaten_10" }
"?" = { tile = "oblaten_box" }
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"Z" = { tile = "oblaten_box", contains = "zaunerstollen" }
"o" = { entity = "oblate" }
"C" = { entity = "checkpoint" }
"s" = { entity = "sign", text = "stage.salzburg.sign_bus", style = "bus" }
"S" = { entity = "sign", text = "stage.salzburg.sign_mirabell" }
"t" = { entity = "selfie_tourist" }
"f" = { entity = "fiaker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal_maibaum" }

[map]
grid = '''
..B?MZT..
.o.......
P.sSC.tfG
#########
'''
"##;

    #[test]
    fn parses_v2_tables_and_meta() {
        let l = Level::parse(V2).unwrap();
        assert_eq!(l.meta.min_jump_window, Some(6));
        assert_eq!(
            l.meta.background,
            vec![
                BackgroundLayer {
                    sprite: "bg_sky_salzburg".into(),
                    factor: 0
                },
                BackgroundLayer {
                    sprite: "bg_rooftops".into(),
                    factor: 8
                },
            ]
        );
        let t = |c| l.tile(c, 0).unwrap();
        assert_eq!((t(2).block, t(2).contains), (Block::Brick, Contents::None));
        assert_eq!(
            (t(3).block, t(3).contains),
            (Block::Box, Contents::Oblate),
            "box default"
        );
        assert_eq!(t(4).contains, Contents::Mozartkugel);
        assert_eq!(t(5).contains, Contents::Zaunerstollen);
        assert_eq!(
            (t(6).block, t(6).contains),
            (Block::Brick, Contents::Oblaten10)
        );
        assert!(
            [2, 3, 4, 5, 6]
                .iter()
                .all(|&c| t(c).kind == TileKind::Solid)
        );
        assert_eq!(l.tile_names[l.empty_box_name() as usize], EMPTY_BOX_TILE);
        assert_eq!(l.tile_names[t(3).name as usize], "oblaten_box");

        let kinds: Vec<SpawnKind> = l.entities.iter().map(|e| e.kind).collect();
        use SpawnKind::*;
        assert_eq!(
            kinds,
            vec![
                Oblate,
                PlayerStart,
                Sign,
                Sign,
                Checkpoint,
                SelfieTourist,
                Fiaker,
                Goal
            ]
        );
        let bus = &l.entities[2];
        assert_eq!(bus.text.as_deref(), Some("stage.salzburg.sign_bus"));
        assert_eq!(bus.style, SignStyle::Bus);
        assert_eq!(l.entities[3].style, SignStyle::Street, "default style");
        assert_eq!((bus.col, bus.row), (2, 2));
    }

    #[test]
    fn every_spawn_kind_round_trips() {
        for k in SpawnKind::ALL {
            assert_eq!(SpawnKind::from_name(k.name()), Some(k));
        }
        assert_eq!(SpawnKind::from_name("goal_maibaum"), Some(SpawnKind::Goal));
        for s in [SignStyle::Street, SignStyle::Bus, SignStyle::Ortsende] {
            assert_eq!(SignStyle::from_name(s.name()), Some(s));
        }
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

    fn err(text: &str) -> String {
        Level::parse(text).unwrap_err().to_string()
    }

    #[test]
    fn other_errors() {
        let ragged = SMALL.replace("###^###", "###^##");
        assert!(err(&ragged).contains("row 3"));

        let no_start = SMALL.replace(".P.--..", "...--..");
        assert!(err(&no_start).contains("player_start"));

        let two_starts = SMALL.replace("......G", "P.....G");
        let e = err(&two_starts);
        assert!(
            e.contains("found 2") && e.contains("row 1, column 1") && e.contains("row 2, column 2"),
            "{e}"
        );

        assert!(err(&SMALL.replace("\"solid\"", "\"lava_cake\"")).contains("lava_cake"));
        assert!(err(&SMALL.replace("\"goal\"", "\"unicorn\"")).contains("unicorn"));

        let contains_on_hazard = SMALL.replace(
            "{ tile = \"hazard\" }",
            "{ tile = \"hazard\", contains = \"oblate\" }",
        );
        assert!(
            err(&contains_on_hazard).contains("only brick and oblaten_box"),
            "{}",
            err(&contains_on_hazard)
        );
        let unknown_field = SMALL.replace("{ tile = \"hazard\" }", "{ tile = \"hazard\", x = 1 }");
        assert!(err(&unknown_field).contains("field \"x\""));
        let entity_field = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", text = \"a\" }",
        );
        assert!(err(&entity_field).contains("field \"text\""));
        let both = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", tile = \"solid\" }",
        );
        assert!(err(&both).contains("exactly one of"));
        let non_string = SMALL.replace("{ entity = \"goal\" }", "{ entity = 3 }");
        assert!(err(&non_string).contains("must be a string"));

        let long_key = SMALL.replace("\"#\" = \"solid\"", "\"##\" = \"solid\"");
        assert!(Level::parse(&long_key).is_err());

        let no_meta = SMALL.replace("[meta]\nid = \"small\"", "");
        assert!(Level::parse(&no_meta).is_err());

        // TOML errors keep line, column and the snippet.
        let syntax = SMALL.replace("time_limit = 100", "time_limit = ");
        let e = err(&syntax);
        assert!(e.contains("line") && e.contains("time_limit"), "{e}");
        let unknown_meta = SMALL.replace("time_limit = 100", "time_limt = 100");
        let e = err(&unknown_meta);
        assert!(e.contains("line") && e.contains("time_limt"), "{e}");
    }

    #[test]
    fn v2_errors() {
        let no_time = SMALL.replace("time_limit = 100\n", "");
        assert!(err(&no_time).contains("time_limit"), "{}", err(&no_time));
        let zero_time = SMALL.replace("time_limit = 100", "time_limit = 0");
        assert!(err(&zero_time).contains("1..=999"));
        let long_time = SMALL.replace("time_limit = 100", "time_limit = 1000");
        assert!(err(&long_time).contains("1..=999"));

        let bad_factor = V2.replace("factor = 8", "factor = 33");
        assert!(
            err(&bad_factor).contains("bg_rooftops"),
            "{}",
            err(&bad_factor)
        );
        let old_background = V2.replace(
            "background = [{ sprite = \"bg_sky_salzburg\", factor = 0 }, { sprite = \"bg_rooftops\", factor = 8 }]",
            "background = [\"sky\"]",
        );
        assert!(Level::parse(&old_background).is_err(), "v1 string list");
        let layer_field = V2.replace("factor = 0 }", "factor = 0, speed = 1 }");
        assert!(err(&layer_field).contains("speed"));

        let bad_contains = V2.replace(
            "contains = \"zaunerstollen\"",
            "contains = \"kaiserschmarrn\"",
        );
        assert!(err(&bad_contains).contains("kaiserschmarrn"));
        let no_text = V2.replace(", text = \"stage.salzburg.sign_mirabell\"", "");
        assert!(err(&no_text).contains("needs text"), "{}", err(&no_text));
        let bad_style = V2.replace("style = \"bus\"", "style = \"neon\"");
        assert!(err(&bad_style).contains("neon"));
        let window = V2.replace("min_jump_window = 6", "min_jump_window = -1");
        assert!(Level::parse(&window).is_err());
    }

    #[test]
    fn parses_testbed() {
        let l = Level::parse(include_str!("../../../content/levels/00_testbed.level")).unwrap();
        assert_eq!(l.meta.id, "testbed");
        assert_eq!(l.height, 14);
        assert!(l.width >= 100);
        assert!(l.entities.iter().any(|e| e.kind == SpawnKind::Goal));
        for kind in [TileKind::Solid, TileKind::OneWay, TileKind::Hazard] {
            assert!(l.tiles.iter().any(|t| t.kind == kind), "{kind:?} missing");
        }
    }
}
