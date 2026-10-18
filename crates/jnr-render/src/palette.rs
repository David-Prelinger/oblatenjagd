//! The few master-palette colours the renderer needs for grey-box drawing,
//! UI text and debug overlays, as `0xRRGGBBAA`.
//!
//! Copied from `art/palettes/main.palette` (the palette is the single
//! truth; a test checks every constant against it). Sprites carry their
//! own colours in the atlas.

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
/// `S` sky_blue (panel highlight).
pub const SKY_BLUE: u32 = 0x64aee6ff;
/// `B` navy (stage intro, game over and „Fortsetzung folgt …" screens).
pub const NAVY: u32 = 0x2a3878ff;
/// `i` silver_light (the stage tagline).
pub const SILVER_LIGHT: u32 = 0xa2abbeff;

#[cfg(test)]
mod tests {
    use super::*;

    /// `0xRRGGBBAA` of palette character `ch` in `main.palette`.
    fn palette_colour(src: &str, ch: char) -> u32 {
        let line = src
            .lines()
            .find(|l| l.split_whitespace().next() == Some(ch.to_string().as_str()))
            .unwrap_or_else(|| panic!("`{ch}` is not in main.palette"));
        let hex = line.split_whitespace().nth(1).unwrap();
        let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap();
        (rgb << 8) | 0xff
    }

    #[test]
    fn constants_match_main_palette() {
        let src = include_str!("../../../art/palettes/main.palette");
        for (ch, value) in [
            ('k', OUTLINE),
            ('m', STONE_GREY),
            ('w', WHITE),
            ('s', SKY_LIGHT),
            ('g', MEADOW),
            ('Y', MOZART_YELLOW),
            ('l', LEATHER_DARK),
            ('r', DIRNDL_RED),
            ('B', NAVY),
            ('S', SKY_BLUE),
            ('i', SILVER_LIGHT),
        ] {
            assert_eq!(value, palette_colour(src, ch), "palette `{ch}`");
        }
    }
}
