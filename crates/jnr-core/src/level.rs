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
    /// `docs/sprite-catalog.md` (M1 and M2) map to a kind; unknown names
    /// return `None`. Decorative back tiles (`railing_bg`,
    /// `wall_village_bg`, `roof_bg`) are `Empty`: drawn, never collide.
    pub fn from_tile_name(name: &str) -> Option<TileKind> {
        match name {
            "empty" | "railing_bg" | "wall_village_bg" | "roof_bg" => Some(TileKind::Empty),
            "solid" | "ground" | "meadow" | "rock" | "rampart" | "hedge" | "stone" | "bridge"
            | "railing" | "arch" | "facade" | "arch_under" | "invisible" | "brick"
            | "oblaten_box" | "oblaten_box_empty" | "facade_village" | "roof_shingle" | "jetty"
            | "jetty_post" | "forest_path" | "beute" | "chimney" => Some(TileKind::Solid),
            "oneway" | "oneway_sign" | "oneway_burger" | "oneway_wood" | "balcony" => {
                Some(TileKind::OneWay)
            }
            "hazard"
            | "salzach_water"
            | "salzach_water_deep"
            | "wolfgangsee_water"
            | "wolfgangsee_water_deep" => Some(TileKind::Hazard),
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
    /// A decorative catalog sprite (`sprite`), no collision (D-030).
    Prop,
    /// Enemy `nordic_walker` (turns at ledges).
    NordicWalker,
    /// Enemy `blasmusiker` (blows note trains).
    Blasmusiker,
    /// Enemy `e_biker` (parked charger that follows the ground).
    EBiker,
    /// Enemy `bienenschwarm` (`dir`, `len`, `phase`: figure-eight dance).
    Bienenschwarm,
    /// A Zwölferhorn cabin (`to`, `phase`): free-running moving platform.
    Gondel,
}

impl SpawnKind {
    /// Every kind, in declaration order.
    pub const ALL: [SpawnKind; 17] = [
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
        SpawnKind::Prop,
        SpawnKind::NordicWalker,
        SpawnKind::Blasmusiker,
        SpawnKind::EBiker,
        SpawnKind::Bienenschwarm,
        SpawnKind::Gondel,
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
            SpawnKind::Prop => "prop",
            SpawnKind::NordicWalker => "nordic_walker",
            SpawnKind::Blasmusiker => "blasmusiker",
            SpawnKind::EBiker => "e_biker",
            SpawnKind::Bienenschwarm => "bienenschwarm",
            SpawnKind::Gondel => "gondel",
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
    /// The other end point `[dx, dy]` in tiles, relative to the spawn cell
    /// (`to = [dx, dy]`, required for `festungsbahn` and `gondel`, `None`
    /// otherwise).
    pub to: Option<[i32; 2]>,
    /// `still = true` on a `selfie_tourist`: it never walks (a fixed
    /// target for grey-box test levels, `00_*.level` only, checked by
    /// `levels lint`; D-026).
    pub still: bool,
    /// The sprite name (`sprite = "..."`, required for `prop`, `None`
    /// otherwise; must be one of [`PROP_SPRITES`], checked by `levels lint`).
    pub sprite: Option<String>,
    /// Where a `prop` is drawn (`layer = "back"`; front by default, D-031).
    pub layer: PropLayer,
    /// Horizontal offset of a `prop` in px (`dx = N`, 0 by default, within
    /// ±[`MAX_PROP_DX_PX`]), e.g. 8 to centre it between two columns.
    pub dx: i32,
    /// Frames into its cycle at activation (`phase = N`, `gondel` and
    /// `bienenschwarm`; 0 by default).
    pub phase: u16,
    /// Direction of a `bienenschwarm`'s waggle run in 45° steps
    /// (`dir = 0..=7`: 0 right, 2 up, 4 left, 6 down; 0 by default).
    pub dir: u8,
    /// Length of a `bienenschwarm`'s waggle run in tiles (`len = 2..=4`,
    /// [`DEFAULT_SWARM_LEN`] by default).
    pub len: u8,
}

/// A `bienenschwarm`'s waggle run without `len`, in tiles.
pub const DEFAULT_SWARM_LEN: u8 = 3;
/// Allowed `len` of a `bienenschwarm`, in tiles.
pub const SWARM_LEN_RANGE: std::ops::RangeInclusive<u8> = 2..=4;
/// Largest `|dx|` of a `gondel`'s `to`, in tiles (D-044: a rider carried
/// back left never reaches the screen's left edge).
pub const MAX_GONDEL_DX_TILES: i32 = 8;

/// Largest `|dx|` of a `prop`, in px.
pub const MAX_PROP_DX_PX: i32 = 64;

/// Draw layer of a `prop` (D-031).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PropLayer {
    /// After the tiles, behind the player (the default).
    #[default]
    Front,
    /// After the parallax background, before the tiles (façades behind
    /// the street).
    Back,
}

impl PropLayer {
    /// The layer for a `layer` value; `None` if unknown.
    pub fn from_name(name: &str) -> Option<PropLayer> {
        match name {
            "front" => Some(PropLayer::Front),
            "back" => Some(PropLayer::Back),
            _ => None,
        }
    }
}

/// Catalog sprites a `prop` may draw (`docs/sprite-catalog.md`, group
/// `props`). Add a name here when the catalog gets a new prop sprite.
pub const PROP_SPRITES: &[&str] = &[
    "sphaera",
    "pegasus",
    "fountain_water",
    "zwerg",
    "residenzbrunnen",
    "geburtshaus",
    "dombogen",
    "prellbock",
    "talstation",
    "seilbahn_stuetze",
    "mozartbrunnen",
    "mozarthaus",
    "musikpavillon",
    "muehlrad",
    "bienenhaus",
    "futterschaelchen",
    "boathouse",
];

/// Largest `|dx|` / `|dy|` of a Festungsbahn's `to`, in tiles.
pub const MAX_TRACK_TILES: i32 = 64;

/// One parallax layer of `[meta].background`.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackgroundLayer {
    /// Sprite name of the layer.
    pub sprite: String,
    /// Scroll speed in 1/16 of the camera speed (0 = fixed, 16 = like the tiles).
    pub factor: u32,
    /// Offset in px above the screen bottom (default 0: bottom-aligned).
    #[serde(default)]
    pub y: u32,
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
/// Most entity spawns in one level (entities refer to their spawn by `u16`).
pub const MAX_SPAWNS: usize = u16::MAX as usize;
/// Largest background `y` offset in px (one screen height).
pub const MAX_BACKGROUND_Y: u32 = 180;

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
        to: Option<[i32; 2]>,
        still: bool,
        sprite: Option<String>,
        layer: PropLayer,
        dx: i32,
        phase: u16,
        dir: u8,
        len: u8,
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
                    LegendEntry::Entity {
                        kind,
                        text,
                        style,
                        to,
                        still,
                        sprite,
                        layer,
                        dx,
                        phase,
                        dir,
                        len,
                    } => {
                        tiles.push(Tile::default());
                        entities.push(EntitySpawn {
                            kind: *kind,
                            col: col as i32,
                            row: r as i32,
                            text: text.clone(),
                            style: *style,
                            to: *to,
                            still: *still,
                            sprite: sprite.clone(),
                            layer: *layer,
                            dx: *dx,
                            phase: *phase,
                            dir: *dir,
                            len: *len,
                        });
                    }
                }
            }
        }

        if entities.len() > MAX_SPAWNS {
            return Err(ParseError::new(format!(
                "level has {} entity spawns, at most {MAX_SPAWNS} are allowed",
                entities.len()
            )));
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
        if layer.y > MAX_BACKGROUND_Y {
            return Err(ParseError::new(format!(
                "meta.background[{i}] ({}): y must be 0..={MAX_BACKGROUND_Y} px, got {}",
                layer.sprite, layer.y
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
                (None, Some("festungsbahn")) => &["entity", "to"],
                (None, Some("gondel")) => &["entity", "to", "phase"],
                (None, Some("bienenschwarm")) => &["entity", "dir", "len", "phase"],
                (None, Some("selfie_tourist")) => &["entity", "still"],
                (None, Some("prop")) => &["entity", "sprite", "layer", "dx"],
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
    let mut to = None;
    if matches!(kind, SpawnKind::Festungsbahn | SpawnKind::Gondel) {
        let max_dx = if kind == SpawnKind::Gondel {
            MAX_GONDEL_DX_TILES
        } else {
            MAX_TRACK_TILES
        };
        let bad = || {
            ParseError::new(format!(
                "legend \"{key}\": a {name} needs to = [dx, dy] (its other end point in tiles relative to its cell, not both 0, dx within ±{max_dx}, dy within ±{MAX_TRACK_TILES})"
            ))
        };
        let Some(toml::Value::Array(a)) = t.get("to") else {
            return Err(bad());
        };
        let v: Vec<i64> = a.iter().filter_map(toml::Value::as_integer).collect();
        let range = -i64::from(MAX_TRACK_TILES)..=i64::from(MAX_TRACK_TILES);
        let range_x = -i64::from(max_dx)..=i64::from(max_dx);
        match v.as_slice() {
            &[dx, dy]
                if a.len() == 2
                    && range_x.contains(&dx)
                    && range.contains(&dy)
                    && (dx, dy) != (0, 0) =>
            {
                to = Some([dx as i32, dy as i32]);
            }
            _ => return Err(bad()),
        }
    }
    let still = match t.get("still") {
        None => false,
        Some(toml::Value::Boolean(b)) => *b,
        Some(_) => {
            return Err(ParseError::new(format!(
                "legend \"{key}\": field \"still\" must be true or false"
            )));
        }
    };
    let mut sprite = None;
    let mut layer = PropLayer::default();
    let mut dx = 0;
    if kind == SpawnKind::Prop {
        match t.get("dx") {
            None => {}
            Some(toml::Value::Integer(n))
                if (-i64::from(MAX_PROP_DX_PX)..=i64::from(MAX_PROP_DX_PX)).contains(n) =>
            {
                dx = *n as i32
            }
            Some(_) => {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": a prop's dx must be a whole number of px within ±{MAX_PROP_DX_PX}"
                )));
            }
        }
        if let Some(l) = string_field(key, t, "layer")? {
            layer = PropLayer::from_name(l).ok_or_else(|| {
                ParseError::new(format!(
                    "legend \"{key}\": unknown prop layer \"{l}\" (expected front or back)"
                ))
            })?;
        }
        match string_field(key, t, "sprite")? {
            Some(s) if !s.is_empty() => sprite = Some(s.to_owned()),
            _ => {
                return Err(ParseError::new(format!(
                    "legend \"{key}\": a prop needs sprite = \"<sprite name>\""
                )));
            }
        }
    }
    let int_field = |field: &str, range: std::ops::RangeInclusive<i64>| match t.get(field) {
        None => Ok(None),
        Some(toml::Value::Integer(n)) if range.contains(n) => Ok(Some(*n)),
        Some(_) => Err(ParseError::new(format!(
            "legend \"{key}\": field \"{field}\" must be a whole number in {}..={}",
            range.start(),
            range.end()
        ))),
    };
    let phase = int_field("phase", 0..=i64::from(u16::MAX))?.unwrap_or(0) as u16;
    let dir = int_field("dir", 0..=7)?.unwrap_or(0) as u8;
    let len = int_field(
        "len",
        i64::from(*SWARM_LEN_RANGE.start())..=i64::from(*SWARM_LEN_RANGE.end()),
    )?
    .map_or(DEFAULT_SWARM_LEN, |n| n as u8);
    Ok(LegendEntry::Entity {
        kind,
        text,
        style,
        to,
        still,
        sprite,
        layer,
        dx,
        phase,
        dir,
        len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salzburg_water_tiles_are_hazards() {
        assert_eq!(
            TileKind::from_tile_name("salzach_water"),
            Some(TileKind::Hazard)
        );
        assert_eq!(
            TileKind::from_tile_name("salzach_water_deep"),
            Some(TileKind::Hazard)
        );
    }

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
            to: None,
            still: false,
            sprite: None,
            layer: PropLayer::Front,
            dx: 0,
            phase: 0,
            dir: 0,
            len: DEFAULT_SWARM_LEN,
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
background = [{ sprite = "bg_sky_salzburg", factor = 0 }, { sprite = "bg_rooftops", factor = 8, y = 24 }]

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
                    factor: 0,
                    y: 0
                },
                BackgroundLayer {
                    sprite: "bg_rooftops".into(),
                    factor: 8,
                    y: 24
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

        let bad_y = V2.replace("y = 24", "y = 181");
        assert!(err(&bad_y).contains("y must be"), "{}", err(&bad_y));
        let bad_factor = V2.replace("factor = 8", "factor = 33");
        assert!(
            err(&bad_factor).contains("bg_rooftops"),
            "{}",
            err(&bad_factor)
        );
        let old_background = V2.replace(
            "background = [{ sprite = \"bg_sky_salzburg\", factor = 0 }, { sprite = \"bg_rooftops\", factor = 8, y = 24 }]",
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
    fn a_selfie_tourist_can_be_still() {
        let with = |field: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!("\"G\" = {{ entity = \"goal\" }}\n\"t\" = {{ entity = \"selfie_tourist\"{field} }}"),
                )
                .replace(".P.--..", ".Pt--..")
        };
        let still = |field: &str| {
            Level::parse(&with(field))
                .unwrap()
                .entities
                .iter()
                .find(|e| e.kind == SpawnKind::SelfieTourist)
                .unwrap()
                .still
        };
        assert!(!still(""));
        assert!(!still(", still = false"));
        assert!(still(", still = true"));
        assert!(err(&with(", still = 1")).contains("true or false"));
        let on_fiaker = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", still = true }",
        );
        assert!(err(&on_fiaker).contains("field \"still\""));
    }

    #[test]
    fn festungsbahn_needs_its_other_end_point() {
        let with = |to: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!("\"G\" = {{ entity = \"goal\" }}\n\"F\" = {{ entity = \"festungsbahn\"{to} }}"),
                )
                .replace(".P.--..", ".PF--..")
        };
        let l = Level::parse(&with(", to = [8, -5]")).unwrap();
        let car = l
            .entities
            .iter()
            .find(|e| e.kind == SpawnKind::Festungsbahn)
            .unwrap();
        assert_eq!((car.col, car.row, car.to), (2, 1, Some([8, -5])));
        assert!(
            l.entities
                .iter()
                .all(|e| e.kind == SpawnKind::Festungsbahn || e.to.is_none())
        );
        for bad in [
            "",
            ", to = [0, 0]",
            ", to = [1]",
            ", to = [1, 2, 3]",
            ", to = [65, 0]",
            ", to = \"up\"",
            ", to = [\"1\", 2]",
        ] {
            assert!(
                err(&with(bad)).contains("needs to = [dx, dy]"),
                "{bad}: {}",
                err(&with(bad))
            );
        }
        let on_sign = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", to = [1, 1] }",
        );
        assert!(err(&on_sign).contains("field \"to\""));
    }

    #[test]
    fn props_need_a_sprite_and_take_an_optional_layer() {
        let with = |fields: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!(
                        "\"G\" = {{ entity = \"goal\" }}\n\"S\" = {{ entity = \"prop\"{fields} }}"
                    ),
                )
                .replace(".P.--..", ".PS--..")
        };
        let prop = |fields: &str| {
            let l = Level::parse(&with(fields)).unwrap();
            let p = l
                .entities
                .iter()
                .find(|e| e.kind == SpawnKind::Prop)
                .cloned();
            (p.unwrap(), l)
        };
        let (p, l) = prop(", sprite = \"sphaera\"");
        assert_eq!((p.col, p.row), (2, 1));
        assert_eq!(
            (p.sprite.as_deref(), p.layer),
            (Some("sphaera"), PropLayer::Front)
        );
        assert_eq!(l.tile(2, 1).unwrap().kind, TileKind::Empty, "no collision");
        assert_eq!(p.dx, 0);
        let (p, _) = prop(", sprite = \"geburtshaus\", layer = \"back\"");
        assert_eq!(p.layer, PropLayer::Back);
        assert_eq!(prop(", sprite = \"sphaera\", dx = 8").0.dx, 8);
        assert_eq!(prop(", sprite = \"sphaera\", dx = -64").0.dx, -64);
        for bad in [
            ", sprite = \"a\", dx = 65",
            ", sprite = \"a\", dx = -65",
            ", sprite = \"a\", dx = -9223372036854775808",
            ", sprite = \"a\", dx = true",
            ", sprite = \"a\", dx = \"8\"",
        ] {
            let e = err(&with(bad));
            assert!(e.contains("dx must be"), "{bad}: {e}");
        }
        assert_eq!(
            prop(", sprite = \"x\", layer = \"front\"").0.layer,
            PropLayer::Front
        );
        for bad in ["", ", sprite = \"\"", ", sprite = 3"] {
            let e = err(&with(bad));
            assert!(e.contains("sprite"), "{bad}: {e}");
        }
        let e = err(&with(", sprite = \"zwerg\", layer = \"middle\""));
        assert!(e.contains("unknown prop layer \"middle\""), "{e}");
        let on_goal = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", sprite = \"a\" }",
        );
        assert!(err(&on_goal).contains("field \"sprite\""));
        assert!(PROP_SPRITES.contains(&"sphaera"));
    }

    #[test]
    fn gondel_needs_to_with_at_most_8_tiles_across_and_takes_a_phase() {
        let with = |fields: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!(
                        "\"G\" = {{ entity = \"goal\" }}\n\"g\" = {{ entity = \"gondel\"{fields} }}"
                    ),
                )
                .replace(".P.--..", ".Pg--..")
        };
        let gondel = |fields: &str| {
            Level::parse(&with(fields))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.kind == SpawnKind::Gondel)
                .unwrap()
        };
        let g = gondel(", to = [6, -1]");
        assert_eq!((g.to, g.phase), (Some([6, -1]), 0));
        let g = gondel(", to = [-8, 2], phase = 120");
        assert_eq!((g.to, g.phase), (Some([-8, 2]), 120));
        for bad in ["", ", to = [9, 0]", ", to = [-9, 1]", ", to = [0, 0]"] {
            let e = err(&with(bad));
            assert!(e.contains("a gondel needs to = [dx, dy]"), "{bad}: {e}");
        }
        for bad in [", to = [1, 0], phase = -1", ", to = [1, 0], phase = 65536"] {
            let e = err(&with(bad));
            assert!(
                e.contains("field \"phase\" must be a whole number"),
                "{bad}: {e}"
            );
        }
        let e = err(&with(", to = [1, 0], dir = 1"));
        assert!(e.contains("field \"dir\" is not allowed"), "{e}");
    }

    #[test]
    fn bienenschwarm_takes_dir_len_and_phase() {
        let with = |fields: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!(
                        "\"G\" = {{ entity = \"goal\" }}\n\"b\" = {{ entity = \"bienenschwarm\"{fields} }}"
                    ),
                )
                .replace(".P.--..", ".Pb--..")
        };
        let swarm = |fields: &str| {
            let s = Level::parse(&with(fields))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.kind == SpawnKind::Bienenschwarm)
                .unwrap();
            (s.dir, s.len, s.phase)
        };
        assert_eq!(swarm(""), (0, DEFAULT_SWARM_LEN, 0), "defaults");
        assert_eq!(swarm(", dir = 7, len = 4, phase = 99"), (7, 4, 99));
        assert_eq!(swarm(", dir = 2, len = 2"), (2, 2, 0));
        for (bad, field) in [
            (", dir = 8", "dir"),
            (", dir = -1", "dir"),
            (", len = 1", "len"),
            (", len = 5", "len"),
            (", len = \"3\"", "len"),
            (", phase = 70000", "phase"),
        ] {
            let e = err(&with(bad));
            assert!(
                e.contains(&format!("field \"{field}\" must be")),
                "{bad}: {e}"
            );
        }
        assert!(err(&with(", to = [1, 1]")).contains("field \"to\" is not allowed"));
        let on_walker = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\" }\n\"w\" = { entity = \"nordic_walker\", len = 3 }",
        );
        assert!(err(&on_walker).contains("field \"len\" is not allowed"));
    }

    #[test]
    fn m2_tile_names_have_their_collision_kinds() {
        use TileKind::*;
        for (name, kind) in [
            ("wall_village_bg", Empty),
            ("balcony", OneWay),
            ("facade_village", Solid),
            ("roof_shingle", Solid),
            ("jetty", Solid),
            ("jetty_post", Solid),
            ("forest_path", Solid),
            ("beute", Solid),
            ("wolfgangsee_water", Hazard),
            ("wolfgangsee_water_deep", Hazard),
        ] {
            assert_eq!(TileKind::from_tile_name(name), Some(kind), "{name}");
        }
    }

    #[test]
    fn facade_arch_under_and_invisible_are_solid() {
        for name in ["facade", "arch_under", "invisible"] {
            assert_eq!(
                TileKind::from_tile_name(name),
                Some(TileKind::Solid),
                "{name}"
            );
        }
    }

    #[test]
    fn too_many_spawns_is_an_error() {
        let row = "o".repeat(MAX_SPAWNS / 2 + 1);
        let grid = format!("P{row}\n.{row}\n");
        let text = format!(
            "[meta]\nid = \"many\"\ntime_limit = 1\n[legend]\n\"o\" = {{ entity = \"oblate\" }}\n\
             \"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{grid}'''\n"
        );
        let e = err(&text);
        assert!(
            e.contains("65537 entity spawns") && e.contains("65535"),
            "{e}"
        );
        let ok = text.replacen(".o", "..", 1).replacen(".o", "..", 1);
        assert!(Level::parse(&ok).is_ok(), "exactly 65535");
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
