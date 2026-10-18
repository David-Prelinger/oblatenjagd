//! The few master-palette colours the renderer needs for grey-box drawing,
//! UI text and debug overlays, as `0xRRGGBBAA`.
//!
//! Copied from `art/palettes/main.palette` (the palette is the single
//! truth; keep these in sync if it ever changes). Sprites carry their own
//! colours in the atlas.

/// `k` outline.
pub const OUTLINE: u32 = 0x1a1c2cff;
/// `m` stone_grey (grey-box solid tiles).
pub const STONE_GREY: u32 = 0x5d6680ff;
/// `w` white (text).
pub const WHITE: u32 = 0xfbf7eeff;
/// `s` sky_light (background sky).
pub const SKY_LIGHT: u32 = 0xa6daf2ff;
/// `g` meadow (debug: goal boxes).
pub const MEADOW: u32 = 0x56a845ff;
/// `Y` mozart_yellow (grey-box goal pole, debug: player hitbox).
pub const MOZART_YELLOW: u32 = 0xf5c547ff;
/// `l` leather_dark (grey-box one-way platforms).
pub const LEATHER_DARK: u32 = 0x5c3226ff;
/// `r` dirndl_red (grey-box hazards, player placeholder, goal flag).
pub const DIRNDL_RED: u32 = 0xcf3a3eff;
