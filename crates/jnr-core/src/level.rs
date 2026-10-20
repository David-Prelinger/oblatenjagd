//! The `.level` format (v2): TOML `[meta]`, `[legend]` and an ASCII
//! `[map].grid`. See `docs/architecture.md` → "Level".
//!
//! Legend values are a tile name (`"ground"`), a tile table
//! (`{ tile = "oblaten_box", contains = "mozartkugel" }`) or an entity table
//! (`{ entity = "sign", text = "<key>", style = "bus" }`; any entity may add
//! `tile = "<name>"` for a non-colliding tile drawn in its cell). The level is the
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
    /// Kills the player on touch (spikes).
    Hazard,
    /// Kills the player once her feet have sunk `player.water_sink_px`
    /// below the tile's top (Salzach, Wolfgangsee; D-050); otherwise a
    /// hazard (items and E-Bikers are lost in it).
    Water,
    /// Swim water (Wolfgangsee lake, D-076): never kills; the player swims
    /// while her chest point is in it (`crate::swim`). Not a floor for
    /// anything; items, axes and walking enemies are lost in it as in
    /// water.
    Swim,
}

impl TileKind {
    /// Simulation behaviour of a named tile from the legend. Grey-box names
    /// (`solid`, `oneway`, `hazard`, `empty`) and the M1 art tile names of
    /// `docs/sprite-catalog.md` (M1 and M2) map to a kind; unknown names
    /// return `None`. Decorative back tiles (`railing_bg`,
    /// `wall_village_bg`, `roof_bg`, `kalkfels_bg`, `netz_post`: the post
    /// under a rockfall net) are `Empty`: drawn,
    /// never collide. M3: `kalkfels` is solid, the shaft planks `steig` (and
    /// `steig_wall`, the same plank drawn over the `kalkfels_bg` back wall) and
    /// the rockfall net `netz` are one-way (D-055, D-058). M4: the
    /// souvenir stand's counter `standl` and the road `strasse` are solid,
    /// its awning `standl_dach` and the bus shelter roof `wartehaus_dach`
    /// one-way, the bypass tunnel's inside `tunnel_bg` (and `tunnel_bg_lamp`,
    /// the same with a lamp) and the fallback
    /// rack-track segments `zahnrad_gleis_0..3` decorative (D-067). M5: the
    /// lake bed `seegrund`, the quay wall `kai` and the bollard `poller` are
    /// solid, the pier planks `steg` one-way (D-077); the swim water
    /// `see_wasser` (surface), `see_wasser_tief` (deep) and `steg_pfahl`
    /// (water with a pier post) are swim water (D-076). M6: the Bürglstein
    /// walkway `felssteg` is one-way, the gravel path `kies` solid, the side
    /// street's mouth `einfahrt_bg` decorative.
    pub fn from_tile_name(name: &str) -> Option<TileKind> {
        match name {
            "empty" | "railing_bg" | "wall_village_bg" | "roof_bg" | "kalkfels_bg"
            | "netz_post" | "terrasse_post_l" | "terrasse_post_r" | "tunnel_bg"
            | "tunnel_bg_lamp" | "zahnrad_gleis_0" | "zahnrad_gleis_1" | "zahnrad_gleis_2"
            | "zahnrad_gleis_3" | "einfahrt_bg" => Some(TileKind::Empty),
            "solid" | "ground" | "meadow" | "rock" | "rampart" | "hedge" | "stone" | "bridge"
            | "railing" | "arch" | "facade" | "arch_under" | "invisible" | "brick"
            | "oblaten_box" | "oblaten_box_empty" | "facade_village" | "roof_shingle" | "jetty"
            | "jetty_post" | "forest_path" | "beute" | "chimney" | "kalkfels" | "standl"
            | "strasse" | "seegrund" | "kai" | "poller" | "kies" => Some(TileKind::Solid),
            "oneway" | "oneway_sign" | "oneway_burger" | "oneway_wood" | "balcony" | "steig"
            | "steig_wall" | "netz" | "standl_dach" | "wartehaus_dach" | "steg" | "felssteg" => {
                Some(TileKind::OneWay)
            }
            "hazard" => Some(TileKind::Hazard),
            "salzach_water"
            | "salzach_water_deep"
            | "wolfgangsee_water"
            | "wolfgangsee_water_deep" => Some(TileKind::Water),
            "see_wasser" | "see_wasser_tief" | "steg_pfahl" => Some(TileKind::Swim),
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
            TileKind::Water => 4,
            TileKind::Swim => 5,
        }
    }

    /// True for the kinds that kill (spikes, water). Swim water does not.
    pub const fn is_lethal(self) -> bool {
        matches!(self, TileKind::Hazard | TileKind::Water)
    }

    /// True for the kinds that items, thrown axes and vehicles (E-Biker,
    /// snow globe) are lost in: spikes, water and swim water (D-076).
    pub const fn swallows_items(self) -> bool {
        matches!(self, TileKind::Hazard | TileKind::Water | TileKind::Swim)
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
    /// St. Wolfgang's Hackl power-up (rises out of the box and stays, D-056).
    Hackl,
}

impl Contents {
    /// The contents for a `contains` value; `None` if unknown.
    pub fn from_name(name: &str) -> Option<Contents> {
        match name {
            "oblate" => Some(Contents::Oblate),
            "mozartkugel" => Some(Contents::Mozartkugel),
            "zaunerstollen" => Some(Contents::Zaunerstollen),
            "oblaten_10" => Some(Contents::Oblaten10),
            "hackl" => Some(Contents::Hackl),
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
            Contents::Hackl => 5,
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
    /// Enemy `pilger` (walks off ledges; `lead = true`: the pennant look).
    Pilger,
    /// Enemy `gams` (ledge-bound hopper).
    Gams,
    /// A `steinschlag` rock spawner in a cliff niche (`period`, `phase`,
    /// `range`).
    Steinschlag,
    /// The Durchkriechstein's tunnel (`len` cells from its cell to the
    /// right): DOWN at its mouth starts the scripted crawl (D-057).
    Durchkriechstein,
    /// Enemy `operettensaenger` (sings a vertical high-C column, D-068).
    Operettensaenger,
    /// A `souvenirstand` snow-globe spawner in front of its counter
    /// (`period`, `phase`, `range`; D-069).
    Souvenirstand,
    /// The Schafbergbahn steam train (`to`): the Festungsbahn's waiting
    /// state machine with a 64×32 body on a long track (D-067).
    Schafbergbahn,
    /// Enemy `reisebus` (stop-and-go charger, `stop`: D-070).
    Reisebus,
    /// A Traundl (`to = [dx, 0]`, `phase`): the Gondel's free-running
    /// machine on the lake with its own size and timing (D-077).
    Traundl,
    /// The Dampfer paddle steamer (`to = [dx, 0]`): the Festungsbahn's
    /// waiting machine, one way, with a paddle-wheel hurt zone (D-077).
    Dampfer,
    /// Enemy `schwan` (floats in a surface-row water cell, charges, D-078).
    Schwan,
    /// Enemy `gelse` (hovers, then homes in; never enters water, D-079).
    Gelse,
    /// Enemy `regattaboot` (the Reisebus's machine on the water, D-080).
    Regattaboot,
}

impl SpawnKind {
    /// Every kind, in declaration order.
    pub const ALL: [SpawnKind; 30] = [
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
        SpawnKind::Pilger,
        SpawnKind::Gams,
        SpawnKind::Steinschlag,
        SpawnKind::Durchkriechstein,
        SpawnKind::Operettensaenger,
        SpawnKind::Souvenirstand,
        SpawnKind::Schafbergbahn,
        SpawnKind::Reisebus,
        SpawnKind::Traundl,
        SpawnKind::Dampfer,
        SpawnKind::Schwan,
        SpawnKind::Gelse,
        SpawnKind::Regattaboot,
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
            SpawnKind::Pilger => "pilger",
            SpawnKind::Gams => "gams",
            SpawnKind::Steinschlag => "steinschlag",
            SpawnKind::Durchkriechstein => "durchkriechstein",
            SpawnKind::Operettensaenger => "operettensaenger",
            SpawnKind::Souvenirstand => "souvenirstand",
            SpawnKind::Schafbergbahn => "schafbergbahn",
            SpawnKind::Reisebus => "reisebus",
            SpawnKind::Traundl => "traundl",
            SpawnKind::Dampfer => "dampfer",
            SpawnKind::Schwan => "schwan",
            SpawnKind::Gelse => "gelse",
            SpawnKind::Regattaboot => "regattaboot",
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

/// An entity placed in the grid. Its cell's tile is empty air, or the
/// non-colliding tile named by the legend entry's `tile = "..."` (e.g.
/// `wall_village_bg` behind a walker); the spawn itself doesn't store it.
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
    /// (`to = [dx, dy]`, required for `festungsbahn`, `gondel`,
    /// `schafbergbahn`, `traundl` and `dampfer`, `None` otherwise).
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
    /// Frames into its cycle at activation (`phase = N`, `gondel`,
    /// `traundl` and `bienenschwarm`; 0 by default).
    pub phase: u16,
    /// Direction of a `bienenschwarm`'s waggle run in 45° steps
    /// (`dir = 0..=7`: 0 right, 2 up, 4 left, 6 down; 0 by default).
    pub dir: u8,
    /// Length of a `bienenschwarm`'s waggle run in tiles (`len = 2..=4`,
    /// [`DEFAULT_SWARM_LEN`] by default), or of a `durchkriechstein`'s
    /// tunnel (`len = 2..=6`, [`DEFAULT_STONE_LEN`] by default).
    pub len: u8,
    /// Vertical offset in px (`dy = N`, 0 by default; negative is up):
    /// of a `bienenschwarm`'s anchor from its cell centre (within
    /// ±[`MAX_SWARM_DY_PX`]), e.g. -8 so a swarm anchored above the ground
    /// never dips its bees into it; of a `taube_sitting`'s feet from its
    /// cell bottom (within ±[`MAX_TAUBE_DY_PX`]), e.g. -8 to sit on a
    /// perch 40 px above the ground.
    pub dy: i32,
    /// `lead = true` on a `pilger`: the group leader's sprite
    /// (`pilger_lead`, pennant on his stick); behaviour is unchanged.
    pub lead: bool,
    /// A `steinschlag`'s or `souvenirstand`'s counter period in frames
    /// (`period = N`, [`STEINSCHLAG_PERIOD_RANGE`],
    /// [`DEFAULT_STEINSCHLAG_PERIOD`] by default; with `phase` = 0 or
    /// [`STEINSCHLAG_MIN_PHASE`] / [`SOUVENIRSTAND_MIN_PHASE`]..period).
    pub period: u16,
    /// A `steinschlag`'s or `souvenirstand`'s range in tiles (`range = N`,
    /// [`STEINSCHLAG_RANGE_TILES`]; [`DEFAULT_STEINSCHLAG_RANGE`] /
    /// [`DEFAULT_SOUVENIRSTAND_RANGE`] by default): its counter runs while
    /// the player is this close.
    pub range: u8,
    /// A `reisebus`'s photo stop (`stop = N`, [`REISEBUS_STOP_TILES`]):
    /// it halts once its hitbox is N tiles left of where it was parked
    /// (D-070). 0 = no stop (the default).
    pub stop: u8,
}

/// Allowed `stop` of a `reisebus`, in tiles (D-070).
pub const REISEBUS_STOP_TILES: std::ops::RangeInclusive<u8> = 1..=16;

/// A `steinschlag`'s `period` without the field, in frames (D-058).
pub const DEFAULT_STEINSCHLAG_PERIOD: u16 = 150;
/// Allowed `period` of a `steinschlag`, in frames.
pub const STEINSCHLAG_PERIOD_RANGE: std::ops::RangeInclusive<u16> = 90..=300;
/// A `steinschlag`'s `phase` is 0 or at least this (and below its
/// `period`): the counter starts there when the player first comes in
/// range, so the first rock always follows the whole 40-frame telegraph
/// (`enemies.steinschlag_telegraph` < this, validated).
pub const STEINSCHLAG_MIN_PHASE: u16 = 41;
/// A `steinschlag`'s `range` without the field, in tiles.
pub const DEFAULT_STEINSCHLAG_RANGE: u8 = 6;
/// Allowed `range` of a `steinschlag`, in tiles.
pub const STEINSCHLAG_RANGE_TILES: std::ops::RangeInclusive<u8> = 4..=10;
/// A `souvenirstand`'s `range` without the field, in tiles (D-069; its
/// `period` defaults to [`DEFAULT_STEINSCHLAG_PERIOD`] and takes
/// [`STEINSCHLAG_PERIOD_RANGE`], its `range` [`STEINSCHLAG_RANGE_TILES`]).
pub const DEFAULT_SOUVENIRSTAND_RANGE: u8 = 8;
/// A `souvenirstand`'s `phase` is 0 or at least this (and below its
/// `period`), so the first globe of an approach always follows the whole
/// 30-frame telegraph (`enemies.souvenirstand_telegraph` < this, validated).
pub const SOUVENIRSTAND_MIN_PHASE: u16 = 31;

/// Largest `|dy|` of a `bienenschwarm`, in px (half a tile).
pub const MAX_SWARM_DY_PX: i32 = 8;
/// Largest `|dy|` of a `taube_sitting`, in px (just under a tile), so it
/// can sit on a perch between two cell bottoms (St. Wolfgang's mitre,
/// 40 px above the ground).
pub const MAX_TAUBE_DY_PX: i32 = 15;

/// A `bienenschwarm`'s waggle run without `len`, in tiles.
pub const DEFAULT_SWARM_LEN: u8 = 3;
/// Allowed `len` of a `bienenschwarm`, in tiles.
pub const SWARM_LEN_RANGE: std::ops::RangeInclusive<u8> = 2..=4;
/// A `durchkriechstein`'s tunnel without `len`, in tiles.
pub const DEFAULT_STONE_LEN: u8 = 4;
/// Allowed `len` of a `durchkriechstein`, in tiles.
pub const STONE_LEN_RANGE: std::ops::RangeInclusive<u8> = 2..=6;
/// Largest `|dx|` of a `gondel`'s `to`, in tiles (D-044: a rider carried
/// back left never reaches the screen's left edge).
pub const MAX_GONDEL_DX_TILES: i32 = 8;
/// Allowed `|dx|` of a `schafbergbahn`'s `to`, in tiles (D-067: a long
/// ride, at least one body length across).
pub const SCHAFBERGBAHN_DX_TILES: std::ops::RangeInclusive<i32> = 4..=32;
/// Allowed `|dx|` of a `traundl`'s `to`, in tiles (D-077; dy must be 0).
pub const TRAUNDL_DX_TILES: std::ops::RangeInclusive<i32> = 2..=8;
/// Allowed `dx` of a `dampfer`'s `to`, in tiles (D-077: it travels right,
/// at least one hull length; dy must be 0).
pub const DAMPFER_DX_TILES: std::ops::RangeInclusive<i32> = 8..=64;

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
    "gasthof_fuerberg",
    "bildstock",
    "steinhaufen",
    "falkensteinkirche",
    "brunnkapelle",
    "hackelwurfkapelle",
    "durchkriechstein",
    "talstation_schafberg",
    "haltestelle_alm",
    "tunnelportal",
    "wartehaus",
    "pilgerbrunnen",
    "wallfahrtskirche",
    "weisses_roessl",
    "roessl_pferd",
    "schiffsstation",
    "adventlaterne",
    "dalben",
    "schilf",
    "startboje",
    "buste",
    "pension",
    "gasthof_strobl",
    "lokalbahn_haltestelle",
    "kaiser_jagdstandbild",
    "katrin_talstation",
    "kaiservilla",
    "seeauerhaus",
    "lehar_villa",
    "trinkhalle",
    "kongresshaus",
    "trafik",
    "zauner_fassade",
    "warteschlange",
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
    /// Vertical parallax in 1/16 (0..=32, default 0 = fixed to the screen
    /// bottom): the layer is drawn `(max camera y - camera y) × factor_y /
    /// 16` px lower, so it sits as before at the level bottom and sinks as
    /// the camera climbs (D-054).
    #[serde(default)]
    pub factor_y: u32,
}

/// How the camera follows the player (`[meta] camera`, D-054, D-075).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CameraMode {
    /// The dead-zone camera of Stages 1–3 (the default).
    #[default]
    Side,
    /// The SMW-style climb camera: settles on the ground, follows falls
    /// only a little way, and a fall below the screen kills.
    Climb,
    /// The side camera with an auto-scroll zone (`[meta] autoscroll =
    /// [from, to]`, D-075): while the camera's left edge lies in the zone
    /// it scrolls right on its own and its edges push and stop the player.
    Autoscroll,
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
    /// Camera behaviour (`"side"` by default, `"climb"`, D-054;
    /// `"autoscroll"`, D-075).
    #[serde(default)]
    pub camera: CameraMode,
    /// The auto-scroll zone `[from, to]` in columns: the camera scrolls on
    /// its own while its left edge lies in `[from × 16, to × 16)`. Required
    /// with `camera = "autoscroll"`, an error with any other camera;
    /// `0 ≤ from < to ≤ width − 20` (D-075).
    #[serde(default)]
    pub autoscroll: Option<[i32; 2]>,
}

impl LevelMeta {
    /// The auto-scroll zone `(from, to)` in columns, if the level has one
    /// (`camera = "autoscroll"`, D-075).
    pub fn autoscroll_zone(&self) -> Option<(i32, i32)> {
        match (self.camera, self.autoscroll) {
            (CameraMode::Autoscroll, Some([from, to])) => Some((from, to)),
            _ => None,
        }
    }
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
        /// The cell's tile: `Tile::default()` or a `TileKind::Empty` tile.
        tile: Tile,
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
        dy: i32,
        lead: bool,
        period: u16,
        range: u8,
        stop: u8,
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
                        tile,
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
                        dy,
                        lead,
                        period,
                        range,
                        stop,
                    } => {
                        tiles.push(*tile);
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
                            dy: *dy,
                            lead: *lead,
                            period: *period,
                            range: *range,
                            stop: *stop,
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
        validate_autoscroll(&raw.meta, width as i32)?;

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
        if layer.factor_y > MAX_BACKGROUND_FACTOR {
            return Err(ParseError::new(format!(
                "meta.background[{i}] ({}): factor_y must be 0..={MAX_BACKGROUND_FACTOR} (1/16 of the camera's climb), got {}",
                layer.sprite, layer.factor_y
            )));
        }
    }
    Ok(())
}

/// The screen's width in columns: an auto-scroll zone ends at least this
/// far left of the level's right edge (`to ≤ width − 20`, D-075), so the
/// camera at `to × 16` still fits into the level.
pub const SCREEN_COLS: i32 = crate::camera::SCREEN_W / TILE_PX;

/// `[meta] autoscroll` (D-075): required with `camera = "autoscroll"`
/// (`0 ≤ from < to ≤ width − 20`), an error with any other camera.
fn validate_autoscroll(meta: &LevelMeta, width: i32) -> Result<(), ParseError> {
    match (meta.camera, meta.autoscroll) {
        (CameraMode::Autoscroll, None) => Err(ParseError::new(
            "meta.autoscroll = [from, to] is required with camera = \"autoscroll\"",
        )),
        (CameraMode::Autoscroll, Some([from, to])) => {
            let last = width - SCREEN_COLS;
            if 0 <= from && from < to && to <= last {
                Ok(())
            } else {
                Err(ParseError::new(format!(
                    "meta.autoscroll = [{from}, {to}]: need 0 <= from < to <= width - {SCREEN_COLS} = {last}"
                )))
            }
        }
        (CameraMode::Side | CameraMode::Climb, Some(_)) => Err(ParseError::new(
            "meta.autoscroll is only allowed with camera = \"autoscroll\"",
        )),
        (CameraMode::Side | CameraMode::Climb, None) => Ok(()),
    }
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
                (_, Some("sign")) => &["entity", "tile", "text", "style"],
                (_, Some("festungsbahn" | "schafbergbahn")) => &["entity", "tile", "to"],
                (_, Some("gondel" | "traundl")) => &["entity", "tile", "to", "phase"],
                (_, Some("dampfer")) => &["entity", "tile", "to"],
                (_, Some("bienenschwarm")) => &["entity", "tile", "dir", "len", "phase", "dy"],
                (_, Some("durchkriechstein")) => &["entity", "tile", "len"],
                (_, Some("taube_sitting")) => &["entity", "tile", "dy"],
                (_, Some("selfie_tourist")) => &["entity", "tile", "still"],
                (_, Some("pilger")) => &["entity", "tile", "lead"],
                (_, Some("steinschlag" | "souvenirstand")) => {
                    &["entity", "tile", "period", "phase", "range"]
                }
                (_, Some("prop")) => &["entity", "tile", "sprite", "layer", "dx"],
                (_, Some("reisebus")) => &["entity", "tile", "stop"],
                (_, Some(_)) => &["entity", "tile"],
                (None, None) => {
                    return Err(ParseError::new(format!(
                        "legend \"{key}\": needs tile = \"...\" or entity = \"...\""
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
                (Some(name), None) => {
                    parse_tile(key, name, string_field(key, t, "contains")?, tile_names)
                }
                (tile, Some(name)) => {
                    let tile = match tile {
                        None => Tile::default(),
                        Some(tile_name) => entity_tile(key, tile_name, tile_names)?,
                    };
                    parse_entity(key, name, tile, t)
                }
                (None, None) => unreachable!("checked above"),
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
                    "legend \"{key}\": unknown contains \"{c}\" (expected oblate, mozartkugel, zaunerstollen, oblaten_10 or hackl)"
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

/// The tile under an entity (`tile = "..."` on an entity entry): it must
/// not collide, so the entity behaves exactly as on empty air, or be swim
/// water (D-076: Oblaten, swans and regatta boats in the lake).
fn entity_tile(key: &str, name: &str, tile_names: &mut Vec<String>) -> Result<Tile, ParseError> {
    match parse_tile(key, name, None, tile_names)? {
        LegendEntry::Tile(t) if matches!(t.kind, TileKind::Empty | TileKind::Swim) => Ok(t),
        _ => Err(ParseError::new(format!(
            "legend \"{key}\": the tile under an entity must not collide (\"{name}\" does; use a decorative tile such as wall_village_bg or railing_bg, or swim water such as see_wasser)"
        ))),
    }
}

fn parse_entity(
    key: &str,
    name: &str,
    tile: Tile,
    t: &toml::Table,
) -> Result<LegendEntry, ParseError> {
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
    if matches!(
        kind,
        SpawnKind::Festungsbahn
            | SpawnKind::Gondel
            | SpawnKind::Schafbergbahn
            | SpawnKind::Traundl
            | SpawnKind::Dampfer
    ) {
        let (min_dx, max_dx) = match kind {
            SpawnKind::Gondel => (0, MAX_GONDEL_DX_TILES),
            SpawnKind::Schafbergbahn => (
                *SCHAFBERGBAHN_DX_TILES.start(),
                *SCHAFBERGBAHN_DX_TILES.end(),
            ),
            SpawnKind::Traundl => (*TRAUNDL_DX_TILES.start(), *TRAUNDL_DX_TILES.end()),
            SpawnKind::Dampfer => (*DAMPFER_DX_TILES.start(), *DAMPFER_DX_TILES.end()),
            _ => (0, MAX_TRACK_TILES),
        };
        // Boats stay on the water line (dy 0); the steamer only goes right.
        let flat = matches!(kind, SpawnKind::Traundl | SpawnKind::Dampfer);
        let rightwards = kind == SpawnKind::Dampfer;
        let bad = || {
            let dx_rule = if rightwards {
                format!("{min_dx} <= dx <= {max_dx}, dy = 0")
            } else if flat {
                format!("{min_dx} <= |dx| <= {max_dx}, dy = 0")
            } else if min_dx > 0 {
                format!("{min_dx} <= |dx| <= {max_dx}, |dy| <= |dx|")
            } else {
                format!("dx within ±{max_dx}")
            };
            ParseError::new(format!(
                "legend \"{key}\": a {name} needs to = [dx, dy] (its other end point in tiles relative to its cell, not both 0, {dx_rule}, dy within ±{MAX_TRACK_TILES})"
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
                    && dx.abs() >= i64::from(min_dx)
                    && (kind != SpawnKind::Schafbergbahn || dy.abs() <= dx.abs())
                    && (!flat || dy == 0)
                    && (!rightwards || dx > 0)
                    && range.contains(&dy)
                    && (dx, dy) != (0, 0) =>
            {
                to = Some([dx as i32, dy as i32]);
            }
            _ => return Err(bad()),
        }
    }
    let bool_field = |field: &str| match t.get(field) {
        None => Ok(false),
        Some(toml::Value::Boolean(b)) => Ok(*b),
        Some(_) => Err(ParseError::new(format!(
            "legend \"{key}\": field \"{field}\" must be true or false"
        ))),
    };
    let still = bool_field("still")?;
    let lead = bool_field("lead")?;
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
    let (len_range, len_default) = match kind {
        SpawnKind::Durchkriechstein => (STONE_LEN_RANGE, DEFAULT_STONE_LEN),
        _ => (SWARM_LEN_RANGE, DEFAULT_SWARM_LEN),
    };
    let len = int_field(
        "len",
        i64::from(*len_range.start())..=i64::from(*len_range.end()),
    )?
    .map_or(len_default, |n| n as u8);
    let max_dy = if kind == SpawnKind::TaubeSitting {
        MAX_TAUBE_DY_PX
    } else {
        MAX_SWARM_DY_PX
    };
    let dy = int_field("dy", -i64::from(max_dy)..=i64::from(max_dy))?.unwrap_or(0) as i32;
    let period = int_field(
        "period",
        i64::from(*STEINSCHLAG_PERIOD_RANGE.start())..=i64::from(*STEINSCHLAG_PERIOD_RANGE.end()),
    )?
    .map_or(DEFAULT_STEINSCHLAG_PERIOD, |n| n as u16);
    let range = int_field(
        "range",
        i64::from(*STEINSCHLAG_RANGE_TILES.start())..=i64::from(*STEINSCHLAG_RANGE_TILES.end()),
    )?
    .map_or(
        if kind == SpawnKind::Souvenirstand {
            DEFAULT_SOUVENIRSTAND_RANGE
        } else {
            DEFAULT_STEINSCHLAG_RANGE
        },
        |n| n as u8,
    );
    let stop = int_field(
        "stop",
        i64::from(*REISEBUS_STOP_TILES.start())..=i64::from(*REISEBUS_STOP_TILES.end()),
    )?
    .map_or(0, |n| n as u8);
    if kind == SpawnKind::Steinschlag
        && phase != 0
        && !(STEINSCHLAG_MIN_PHASE..period).contains(&phase)
    {
        return Err(ParseError::new(format!(
            "legend \"{key}\": a steinschlag's phase must be 0 or {STEINSCHLAG_MIN_PHASE}..={} (below its period {period}), so its first rock shows the whole telegraph",
            period - 1
        )));
    }
    if kind == SpawnKind::Souvenirstand
        && phase != 0
        && !(SOUVENIRSTAND_MIN_PHASE..period).contains(&phase)
    {
        return Err(ParseError::new(format!(
            "legend \"{key}\": a souvenirstand's phase must be 0 or {SOUVENIRSTAND_MIN_PHASE}..={} (below its period {period}), so its first globe shows the whole telegraph",
            period - 1
        )));
    }
    Ok(LegendEntry::Entity {
        tile,
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
        dy,
        lead,
        period,
        range,
        stop,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn salzburg_water_tiles_are_water() {
        assert_eq!(
            TileKind::from_tile_name("salzach_water"),
            Some(TileKind::Water)
        );
        assert_eq!(
            TileKind::from_tile_name("salzach_water_deep"),
            Some(TileKind::Water)
        );
        assert_eq!(TileKind::from_tile_name("hazard"), Some(TileKind::Hazard));
        assert!(TileKind::Water.is_lethal() && TileKind::Hazard.is_lethal());
        assert!(!TileKind::Solid.is_lethal() && !TileKind::Empty.is_lethal());
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
            dy: 0,
            lead: false,
            period: DEFAULT_STEINSCHLAG_PERIOD,
            range: DEFAULT_STEINSCHLAG_RANGE,
            stop: 0,
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
                    y: 0,
                    factor_y: 0
                },
                BackgroundLayer {
                    sprite: "bg_rooftops".into(),
                    factor: 8,
                    y: 24,
                    factor_y: 0
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
        let solid_under = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", tile = \"solid\" }",
        );
        assert!(err(&solid_under).contains("must not collide"));
        let neither = SMALL.replace("{ entity = \"goal\" }", "{ text = \"a\" }");
        assert!(err(&neither).contains("needs tile"));
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
    fn an_entity_can_stand_on_a_decorative_tile() {
        let text = SMALL
            .replace(
                "\"G\" = { entity = \"goal\" }",
                "\"G\" = { entity = \"goal\" }\n\
                 \"w\" = { entity = \"nordic_walker\", tile = \"wall_village_bg\" }\n\
                 \"o\" = { entity = \"oblate\", tile = \"railing_bg\" }\n\
                 \"s\" = { entity = \"sign\", tile = \"wall_village_bg\", text = \"a\" }",
            )
            .replace("......G", "wos...G");
        let l = Level::parse(&text).unwrap();
        let name = |col: i32| {
            let t = l.tile(col, 0).unwrap();
            assert_eq!(t.kind, TileKind::Empty, "col {col}");
            assert_eq!((t.block, t.contains), (Block::None, Contents::None));
            l.tile_names[t.name as usize].as_str()
        };
        assert_eq!(name(0), "wall_village_bg");
        assert_eq!(name(1), "railing_bg");
        assert_eq!(name(2), "wall_village_bg");
        assert_eq!(name(3), "");
        let kinds: Vec<(SpawnKind, i32, i32)> =
            l.entities.iter().map(|e| (e.kind, e.col, e.row)).collect();
        assert_eq!(
            kinds,
            vec![
                (SpawnKind::NordicWalker, 0, 0),
                (SpawnKind::Oblate, 1, 0),
                (SpawnKind::Sign, 2, 0),
                (SpawnKind::Goal, 6, 0),
                (SpawnKind::PlayerStart, 1, 1),
            ]
        );
        assert_eq!(l.entities[2].text.as_deref(), Some("a"));

        // Without `tile` the cell stays empty air (name 0).
        let plain = Level::parse(SMALL).unwrap();
        assert_eq!(plain.tile(6, 0), Some(Tile::default()));

        // Only non-colliding tiles, no contents, known names.
        for (tile, msg) in [
            ("solid", "must not collide"),
            ("oneway", "must not collide"),
            ("salzach_water", "must not collide"),
            ("oblaten_box", "must not collide"),
            ("lava_cake", "unknown tile"),
        ] {
            let bad = SMALL.replace(
                "{ entity = \"goal\" }",
                &format!("{{ entity = \"goal\", tile = \"{tile}\" }}"),
            );
            assert!(err(&bad).contains(msg), "{tile}: {}", err(&bad));
        }
        let contains = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\", tile = \"empty\", contains = \"oblate\" }",
        );
        assert!(err(&contains).contains("field \"contains\""));
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
    fn schafbergbahn_needs_to_with_4_to_32_tiles_across() {
        let with = |fields: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!(
                        "\"G\" = {{ entity = \"goal\" }}\n\"z\" = {{ entity = \"schafbergbahn\"{fields} }}"
                    ),
                )
                .replace(".P.--..", ".Pz--..")
        };
        let train = |fields: &str| {
            Level::parse(&with(fields))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.kind == SpawnKind::Schafbergbahn)
                .unwrap()
        };
        assert_eq!(train(", to = [24, -6]").to, Some([24, -6]));
        assert_eq!(train(", to = [4, 0]").to, Some([4, 0]));
        assert_eq!(train(", to = [-32, -8]").to, Some([-32, -8]));
        assert_eq!(train(", to = [8, -8]").to, Some([8, -8]), "45° at most");
        for bad in [
            "",
            ", to = [3, -1]",
            ", to = [-3, 0]",
            ", to = [0, -6]",
            ", to = [33, -8]",
            ", to = [-33, 0]",
            ", to = [8, 65]",
            ", to = [8, 9]",
            ", to = [4, -5]",
            ", to = [8]",
        ] {
            let e = err(&with(bad));
            assert!(
                e.contains("a schafbergbahn needs to = [dx, dy]")
                    && e.contains("4 <= |dx| <= 32, |dy| <= |dx|"),
                "{bad}: {e}"
            );
        }
        let e = err(&with(", to = [8, -2], phase = 1"));
        assert!(e.contains("field \"phase\" is not allowed"), "{e}");
        assert_eq!(
            SpawnKind::from_name("schafbergbahn"),
            Some(SpawnKind::Schafbergbahn)
        );
    }

    /// [`SMALL`] with a `b` spawn of `entity` and `fields` in front of the
    /// one-way tiles.
    fn boat_level(entity: &str, fields: &str) -> String {
        SMALL
            .replace(
                "\"G\" = { entity = \"goal\" }",
                &format!(
                    "\"G\" = {{ entity = \"goal\" }}\n\"b\" = {{ entity = \"{entity}\"{fields} }}"
                ),
            )
            .replace(".P.--..", ".Pb--..")
    }

    fn boat(entity: &str, fields: &str) -> EntitySpawn {
        let kind = SpawnKind::from_name(entity).unwrap();
        Level::parse(&boat_level(entity, fields))
            .unwrap()
            .entities
            .into_iter()
            .find(|e| e.kind == kind)
            .unwrap()
    }

    #[test]
    fn traundl_needs_a_flat_to_of_2_to_8_tiles_and_takes_a_phase() {
        let b = boat("traundl", ", to = [6, 0]");
        assert_eq!(
            (b.kind, b.to, b.phase),
            (SpawnKind::Traundl, Some([6, 0]), 0)
        );
        let b = boat("traundl", ", to = [-2, 0], phase = 100");
        assert_eq!((b.to, b.phase), (Some([-2, 0]), 100));
        assert_eq!(boat("traundl", ", to = [8, 0]").to, Some([8, 0]));
        for bad in [
            "",
            ", to = [1, 0]",
            ", to = [-1, 0]",
            ", to = [9, 0]",
            ", to = [6, 1]",
            ", to = [0, 0]",
            ", to = [6]",
        ] {
            let e = err(&boat_level("traundl", bad));
            assert!(
                e.contains("a traundl needs to = [dx, dy]") && e.contains("2 <= |dx| <= 8, dy = 0"),
                "{bad}: {e}"
            );
        }
        let e = err(&boat_level("traundl", ", to = [6, 0], dir = 1"));
        assert!(e.contains("field \"dir\" is not allowed"), "{e}");
    }

    #[test]
    fn dampfer_needs_a_flat_to_of_8_to_64_tiles_to_the_right() {
        let b = boat("dampfer", ", to = [40, 0]");
        assert_eq!((b.kind, b.to), (SpawnKind::Dampfer, Some([40, 0])));
        assert_eq!(boat("dampfer", ", to = [8, 0]").to, Some([8, 0]));
        assert_eq!(boat("dampfer", ", to = [64, 0]").to, Some([64, 0]));
        for bad in [
            "",
            ", to = [7, 0]",
            ", to = [65, 0]",
            ", to = [-40, 0]",
            ", to = [40, -1]",
        ] {
            let e = err(&boat_level("dampfer", bad));
            assert!(
                e.contains("a dampfer needs to = [dx, dy]") && e.contains("8 <= dx <= 64, dy = 0"),
                "{bad}: {e}"
            );
        }
        let e = err(&boat_level("dampfer", ", to = [40, 0], phase = 1"));
        assert!(e.contains("field \"phase\" is not allowed"), "{e}");
    }

    #[test]
    fn m6_tiles_and_props_parse_with_their_collision_kinds() {
        use TileKind::*;
        for (name, kind) in [
            ("felssteg", OneWay),
            ("kies", Solid),
            ("einfahrt_bg", Empty),
        ] {
            assert_eq!(TileKind::from_tile_name(name), Some(kind), "{name}");
        }
        // A level using the three tiles and an M6 prop on each layer.
        let text = SMALL
            .replace(
                "\"G\" = { entity = \"goal\" }",
                "\"G\" = { entity = \"goal\" }\n\"f\" = \"felssteg\"\n\"k\" = \"kies\"\n\"e\" = \"einfahrt_bg\"\n\"z\" = { entity = \"prop\", sprite = \"zauner_fassade\", layer = \"back\" }\n\"b\" = { entity = \"prop\", sprite = \"buste\" }",
            )
            .replace("......G", "ez.b..G")
            .replace(".P.--..", ".P.ff..")
            .replace("###^###", "kkk^###");
        let l = Level::parse(&text).unwrap();
        let named = |col, row| {
            let t = l.tile(col, row).unwrap();
            (l.tile_names[t.name as usize].clone(), t.kind)
        };
        assert_eq!(named(0, 0), ("einfahrt_bg".to_string(), Empty));
        assert_eq!(named(3, 1), ("felssteg".to_string(), OneWay));
        assert_eq!(named(4, 1), ("felssteg".to_string(), OneWay));
        assert_eq!(named(0, 2), ("kies".to_string(), Solid));
        let props: Vec<(&str, PropLayer, i32)> = l
            .entities
            .iter()
            .filter(|e| e.kind == SpawnKind::Prop)
            .map(|e| (e.sprite.as_deref().unwrap(), e.layer, e.col))
            .collect();
        assert_eq!(
            props,
            [
                ("zauner_fassade", PropLayer::Back, 1),
                ("buste", PropLayer::Front, 3)
            ]
        );
        for prop in [
            "buste",
            "pension",
            "gasthof_strobl",
            "lokalbahn_haltestelle",
            "kaiser_jagdstandbild",
            "katrin_talstation",
            "kaiservilla",
            "seeauerhaus",
            "lehar_villa",
            "trinkhalle",
            "kongresshaus",
            "trafik",
            "zauner_fassade",
            "warteschlange",
        ] {
            assert!(PROP_SPRITES.contains(&prop), "{prop}");
        }
    }

    #[test]
    fn m5_tile_names_have_their_collision_kinds() {
        use TileKind::*;
        for (name, kind) in [
            ("steg", OneWay),
            ("seegrund", Solid),
            ("kai", Solid),
            ("poller", Solid),
        ] {
            assert_eq!(TileKind::from_tile_name(name), Some(kind), "{name}");
        }
        for prop in ["adventlaterne", "dalben", "schilf", "startboje"] {
            assert!(PROP_SPRITES.contains(&prop), "{prop}");
        }
    }

    #[test]
    fn m4_tile_names_have_their_collision_kinds() {
        use TileKind::*;
        for (name, kind) in [
            ("standl", Solid),
            ("strasse", Solid),
            ("standl_dach", OneWay),
            ("wartehaus_dach", OneWay),
            ("tunnel_bg", Empty),
            ("tunnel_bg_lamp", Empty),
            ("zahnrad_gleis_0", Empty),
            ("zahnrad_gleis_1", Empty),
            ("zahnrad_gleis_2", Empty),
            ("zahnrad_gleis_3", Empty),
        ] {
            assert_eq!(TileKind::from_tile_name(name), Some(kind), "{name}");
        }
        assert_eq!(TileKind::from_tile_name("zahnrad_gleis_4"), None);
    }

    #[test]
    fn m3_enemies_take_lead_period_phase_and_range() {
        let with = |entity: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!("\"G\" = {{ entity = \"goal\" }}\n\"b\" = {entity}"),
                )
                .replace(".P.--..", ".Pb--..")
        };
        let spawn = |entity: &str| {
            Level::parse(&with(entity))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.col == 2)
                .unwrap()
        };
        let p = spawn("{ entity = \"pilger\" }");
        assert_eq!((p.kind, p.lead), (SpawnKind::Pilger, false));
        assert!(spawn("{ entity = \"pilger\", lead = true }").lead);
        assert_eq!(spawn("{ entity = \"gams\" }").kind, SpawnKind::Gams);
        let rock = |fields: &str| {
            let s = spawn(&format!("{{ entity = \"steinschlag\"{fields} }}"));
            assert_eq!(s.kind, SpawnKind::Steinschlag);
            (s.period, s.phase, s.range)
        };
        assert_eq!(rock(""), (150, 0, 6), "defaults");
        assert_eq!(rock(", period = 90, phase = 41, range = 4"), (90, 41, 4));
        assert_eq!(
            rock(", period = 300, phase = 299, range = 10"),
            (300, 299, 10)
        );
        let rock_err =
            |fields: &str| err(&with(&format!("{{ entity = \"steinschlag\"{fields} }}")));
        for (bad, field) in [
            (", period = 89", "period"),
            (", period = 301", "period"),
            (", range = 3", "range"),
            (", range = 11", "range"),
            (", phase = -1", "phase"),
        ] {
            let e = rock_err(bad);
            assert!(
                e.contains(&format!("field \"{field}\" must be")),
                "{bad}: {e}"
            );
        }
        // The phase must not skip into the telegraph.
        for bad in [
            ", phase = 1",
            ", phase = 40",
            ", phase = 150",
            ", period = 100, phase = 100",
        ] {
            let e = rock_err(bad);
            assert!(e.contains("phase must be 0 or 41..="), "{bad}: {e}");
        }
        assert!(
            err(&with("{ entity = \"pilger\", lead = 1 }"))
                .contains("field \"lead\" must be true or false")
        );
        for (entity, field) in [
            ("{ entity = \"gams\", lead = true }", "lead"),
            ("{ entity = \"pilger\", period = 100 }", "period"),
            ("{ entity = \"steinschlag\", lead = true }", "lead"),
            ("{ entity = \"steinschlag\", len = 3 }", "len"),
            ("{ entity = \"selfie_tourist\", range = 5 }", "range"),
        ] {
            let e = err(&with(entity));
            assert!(
                e.contains(&format!("field \"{field}\" is not allowed")),
                "{entity}: {e}"
            );
        }
    }

    #[test]
    fn m4_singer_and_stand_take_their_fields() {
        let with = |entity: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!("\"G\" = {{ entity = \"goal\" }}\n\"b\" = {entity}"),
                )
                .replace(".P.--..", ".Pb--..")
        };
        let spawn = |entity: &str| {
            Level::parse(&with(entity))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.col == 2)
                .unwrap()
        };
        assert_eq!(
            spawn("{ entity = \"operettensaenger\" }").kind,
            SpawnKind::Operettensaenger
        );
        let stand = |fields: &str| {
            let s = spawn(&format!("{{ entity = \"souvenirstand\"{fields} }}"));
            assert_eq!(s.kind, SpawnKind::Souvenirstand);
            (s.period, s.phase, s.range)
        };
        assert_eq!(stand(""), (150, 0, 8), "defaults (D-069)");
        assert_eq!(stand(", period = 90, phase = 31, range = 4"), (90, 31, 4));
        assert_eq!(
            stand(", period = 300, phase = 299, range = 10"),
            (300, 299, 10)
        );
        let stand_err =
            |fields: &str| err(&with(&format!("{{ entity = \"souvenirstand\"{fields} }}")));
        for (bad, field) in [
            (", period = 89", "period"),
            (", period = 301", "period"),
            (", range = 3", "range"),
            (", range = 11", "range"),
            (", phase = -1", "phase"),
        ] {
            let e = stand_err(bad);
            assert!(
                e.contains(&format!("field \"{field}\" must be")),
                "{bad}: {e}"
            );
        }
        // The phase must not skip into the telegraph or past the period.
        for bad in [
            ", phase = 1",
            ", phase = 30",
            ", phase = 150",
            ", period = 100, phase = 100",
        ] {
            let e = stand_err(bad);
            assert!(e.contains("phase must be 0 or 31..="), "{bad}: {e}");
        }
        for (entity, field) in [
            ("{ entity = \"operettensaenger\", range = 5 }", "range"),
            ("{ entity = \"operettensaenger\", phase = 5 }", "phase"),
            ("{ entity = \"souvenirstand\", len = 3 }", "len"),
            ("{ entity = \"souvenirstand\", dir = 1 }", "dir"),
        ] {
            let e = err(&with(entity));
            assert!(
                e.contains(&format!("field \"{field}\" is not allowed")),
                "{entity}: {e}"
            );
        }
    }

    #[test]
    fn taube_sitting_takes_dy_up_to_15_px() {
        let with = |fields: &str| {
            SMALL
                .replace(
                    "\"G\" = { entity = \"goal\" }",
                    &format!(
                        "\"G\" = {{ entity = \"goal\" }}\n\"s\" = {{ entity = \"taube_sitting\"{fields} }}"
                    ),
                )
                .replace(".P.--..", ".Ps--..")
        };
        let dy = |fields: &str| {
            Level::parse(&with(fields))
                .unwrap()
                .entities
                .into_iter()
                .find(|e| e.kind == SpawnKind::TaubeSitting)
                .unwrap()
                .dy
        };
        assert_eq!(dy(""), 0, "default");
        assert_eq!(dy(", dy = -8"), -8);
        assert_eq!(dy(", dy = -15"), -15);
        assert_eq!(dy(", dy = 15"), 15);
        for bad in [", dy = -16", ", dy = 16", ", dy = \"-8\""] {
            let e = err(&with(bad));
            assert!(
                e.contains("field \"dy\" must be a whole number in -15..=15"),
                "{bad}: {e}"
            );
        }
        for other in [", dir = 1", ", phase = 3", ", len = 3"] {
            assert!(err(&with(other)).contains("is not allowed"), "{other}");
        }
        // Only the swarm and the sitting Taube take `dy`.
        let on_taube = SMALL.replace(
            "{ entity = \"goal\" }",
            "{ entity = \"goal\" }\n\"u\" = { entity = \"taube\", dy = -8 }",
        );
        assert!(err(&on_taube).contains("field \"dy\" is not allowed"));
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
            (s.dir, s.len, s.phase, s.dy)
        };
        assert_eq!(swarm(""), (0, DEFAULT_SWARM_LEN, 0, 0), "defaults");
        assert_eq!(swarm(", dir = 7, len = 4, phase = 99"), (7, 4, 99, 0));
        assert_eq!(swarm(", dir = 2, len = 2"), (2, 2, 0, 0));
        assert_eq!(swarm(", dir = 1, dy = -8"), (1, DEFAULT_SWARM_LEN, 0, -8));
        assert_eq!(swarm(", dy = 8"), (0, DEFAULT_SWARM_LEN, 0, 8));
        for (bad, field) in [
            (", dir = 8", "dir"),
            (", dir = -1", "dir"),
            (", len = 1", "len"),
            (", len = 5", "len"),
            (", len = \"3\"", "len"),
            (", phase = 70000", "phase"),
            (", dy = -9", "dy"),
            (", dy = 9", "dy"),
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
            ("wolfgangsee_water", Water),
            ("wolfgangsee_water_deep", Water),
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
    fn m5_swim_tiles_are_swim_water() {
        for name in ["see_wasser", "see_wasser_tief", "steg_pfahl"] {
            assert_eq!(
                TileKind::from_tile_name(name),
                Some(TileKind::Swim),
                "{name}"
            );
        }
        // Swim water never kills, but swallows items like water (D-076);
        // the lethal water tiles are unchanged (D-050).
        assert!(!TileKind::Swim.is_lethal() && TileKind::Swim.swallows_items());
        assert!(TileKind::Water.swallows_items() && TileKind::Hazard.swallows_items());
        assert!(!TileKind::Solid.swallows_items() && !TileKind::Empty.swallows_items());
        assert!(!TileKind::OneWay.swallows_items());
        assert_eq!(
            TileKind::from_tile_name("wolfgangsee_water"),
            Some(TileKind::Water)
        );
        assert_ne!(TileKind::Swim.id(), TileKind::Water.id());
    }

    #[test]
    fn an_entity_may_stand_on_a_swim_tile() {
        let with = |tile: &str| {
            SMALL.replace(
                "{ entity = \"goal\" }",
                &format!("{{ entity = \"goal\", tile = \"{tile}\" }}"),
            )
        };
        let l = Level::parse(&with("see_wasser")).unwrap();
        let goal = l
            .entities
            .iter()
            .find(|e| e.kind == SpawnKind::Goal)
            .unwrap();
        let t = l.tile(goal.col, goal.row).unwrap();
        assert_eq!(t.kind, TileKind::Swim);
        assert_eq!(l.tile_names[t.name as usize], "see_wasser");
        assert!(Level::parse(&with("steg_pfahl")).is_ok());
        assert!(err(&with("wolfgangsee_water")).contains("must not collide"));
    }

    #[test]
    fn m3_tile_names_have_their_collision_kinds() {
        use TileKind::*;
        for (name, kind) in [
            ("kalkfels", Solid),
            ("kalkfels_bg", Empty),
            ("netz_post", Empty),
            ("terrasse_post_l", Empty),
            ("terrasse_post_r", Empty),
            ("steig", OneWay),
            ("steig_wall", OneWay),
            ("netz", OneWay),
        ] {
            assert_eq!(TileKind::from_tile_name(name), Some(kind), "{name}");
        }
    }

    /// A one-cell level with extra `[meta]` lines.
    fn with_meta(meta: &str) -> String {
        format!(
            "[meta]\nid = \"c\"\ntime_limit = 1\n{meta}\n[legend]\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\nP\n'''\n"
        )
    }

    #[test]
    fn camera_mode_defaults_to_side_and_takes_climb() {
        let mode = |meta: &str| Level::parse(&with_meta(meta)).unwrap().meta.camera;
        assert_eq!(mode(""), CameraMode::Side);
        assert_eq!(mode("camera = \"side\""), CameraMode::Side);
        assert_eq!(mode("camera = \"climb\""), CameraMode::Climb);
        assert!(err(&with_meta("camera = \"top\"")).contains("camera"));
    }

    #[test]
    fn autoscroll_needs_its_zone_and_the_zone_its_camera() {
        // 40 columns wide: `to` may be at most 40 - 20 = 20.
        let wide = |meta: &str| {
            format!(
                "[meta]\nid = \"a\"\ntime_limit = 1\n{meta}\n[legend]\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\nP{}\n'''\n",
                ".".repeat(39)
            )
        };
        let zone = |meta: &str| Level::parse(&wide(meta)).unwrap().meta.autoscroll_zone();
        let l = Level::parse(&wide("camera = \"autoscroll\"\nautoscroll = [8, 20]")).unwrap();
        assert_eq!(l.meta.camera, CameraMode::Autoscroll);
        assert_eq!(l.meta.autoscroll_zone(), Some((8, 20)));
        assert_eq!(
            zone("camera = \"autoscroll\"\nautoscroll = [0, 1]"),
            Some((0, 1))
        );
        let e = err(&wide("camera = \"autoscroll\""));
        assert!(e.contains("autoscroll = [from, to] is required"), "{e}");
        for bad in ["[8, 21]", "[8, 8]", "[9, 8]", "[-1, 4]"] {
            let e = err(&wide(&format!(
                "camera = \"autoscroll\"\nautoscroll = {bad}"
            )));
            assert!(
                e.contains("need 0 <= from < to <= width - 20 = 20"),
                "{bad}: {e}"
            );
        }
        for camera in ["", "camera = \"side\"", "camera = \"climb\""] {
            let e = err(&wide(&format!("{camera}\nautoscroll = [8, 20]")));
            assert!(
                e.contains("only allowed with camera = \"autoscroll\""),
                "{camera}: {e}"
            );
            assert_eq!(zone(camera), None);
        }
    }

    #[test]
    fn background_factor_y_defaults_to_0_and_is_at_most_32() {
        let bg = |layer: &str| with_meta(&format!("background = [{layer}]"));
        let l = Level::parse(&bg("{ sprite = \"bg\", factor = 8 }")).unwrap();
        assert_eq!(l.meta.background[0].factor_y, 0);
        let l = Level::parse(&bg("{ sprite = \"bg\", factor = 8, factor_y = 32 }")).unwrap();
        assert_eq!(l.meta.background[0].factor_y, 32);
        let e = err(&bg("{ sprite = \"bg\", factor = 8, factor_y = 33 }"));
        assert!(e.contains("factor_y must be 0..=32"), "{e}");
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
