//! Compiles a checked [`SpriteSource`] into RGBA frames, one
//! [`CompiledSprite`] for the base sprite plus one per palette-swap variant.

use crate::atlas::{Point, Rect};
use crate::diag::Diagnostic;
use crate::image::Image;
use crate::lint;
use crate::palette::Palette;
use crate::sprite::{FrameSource, SpriteSource};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledAnim {
    pub name: String,
    pub ticks: u32,
    pub looping: bool,
    /// Right-facing frames in index order.
    pub frames: Vec<Image>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledSprite {
    pub name: String,
    /// Directory group of the source (`ui`, `bg`, `test`, ...); variants
    /// share their base's.
    pub group: String,
    pub width: u32,
    pub height: u32,
    pub origin: Point,
    pub hitbox: Rect,
    /// `mirror = auto`: the atlas also holds left-facing copies.
    pub mirror: bool,
    pub anims: Vec<CompiledAnim>,
}

/// Runs the sprite lints and, if there are no errors, renders every frame.
/// Returns the base sprite followed by its variants (declaration order).
pub fn compile_sprite(
    src: &SpriteSource,
    palette: &Palette,
) -> Result<Vec<CompiledSprite>, Vec<Diagnostic>> {
    let errors: Vec<Diagnostic> = lint::lint_sprite(src, palette)
        .into_iter()
        .filter(Diagnostic::is_error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }

    let mut out = vec![render(src, &src.name, palette, &[])];
    for v in &src.variants {
        let remap: Vec<(char, char)> = v.remap.iter().map(|r| (r.from, r.to)).collect();
        out.push(render(src, &v.name, palette, &remap));
    }
    Ok(out)
}

fn render(
    src: &SpriteSource,
    name: &str,
    palette: &Palette,
    remap: &[(char, char)],
) -> CompiledSprite {
    let anims = src
        .anims
        .iter()
        .map(|a| CompiledAnim {
            name: a.name.clone(),
            ticks: a.ticks,
            looping: a.looping,
            frames: (0..a.frames)
                .map(|i| {
                    let frame = src.frame(&a.name, i).expect("frame-count lint passed");
                    render_frame(frame, src.width, src.height, palette, remap)
                })
                .collect(),
        })
        .collect();
    CompiledSprite {
        name: name.to_owned(),
        group: src.group.clone(),
        width: src.width,
        height: src.height,
        origin: src.origin,
        hitbox: src.hitbox,
        mirror: src.mirror,
        anims,
    }
}

fn render_frame(
    frame: &FrameSource,
    w: u32,
    h: u32,
    palette: &Palette,
    remap: &[(char, char)],
) -> Image {
    let mut img = Image::new(w, h);
    for (y, row) in frame.rows.iter().enumerate() {
        for (x, ch) in row.text.chars().enumerate() {
            let ch = remap
                .iter()
                .find(|(from, _)| *from == ch)
                .map_or(ch, |&(_, to)| to);
            if let Some(Some(px)) = palette.colour(ch) {
                img.set(x as u32, y as u32, px);
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite::parse_sprite;

    const PAL: &str = ". none t\nk #000000 k\nw #ffffff w\nr #ff0000 r\nb #0000ff b\n";

    fn pal() -> Palette {
        Palette::parse("p", PAL).unwrap()
    }

    const SRC: &str = "\
sprite = arrow
size = 3x2
mirror = auto
anim = idle frames=2 ticks=4 loop
variant = arrow_blue remap r->b b->r
---
frame idle 0
kr.
kw.
frame idle 1
kb.
kw.
";

    #[test]
    fn renders_frames_with_palette_colours() {
        let s = parse_sprite("t", "test", SRC).unwrap();
        let out = compile_sprite(&s, &pal()).unwrap();
        assert_eq!(out.len(), 2);
        let f0 = &out[0].anims[0].frames[0];
        assert_eq!(f0.get(0, 0), [0, 0, 0, 255]);
        assert_eq!(f0.get(1, 0), [255, 0, 0, 255]);
        assert_eq!(f0.get(2, 0), [0, 0, 0, 0]);
        assert!(out[0].mirror);
    }

    #[test]
    fn variant_remaps_simultaneously() {
        let s = parse_sprite("t", "test", SRC).unwrap();
        let out = compile_sprite(&s, &pal()).unwrap();
        let v = &out[1];
        assert_eq!(v.name, "arrow_blue");
        // r->b and b->r swap, k and w stay.
        assert_eq!(v.anims[0].frames[0].get(1, 0), [0, 0, 255, 255]);
        assert_eq!(v.anims[0].frames[1].get(1, 0), [255, 0, 0, 255]);
        assert_eq!(v.anims[0].frames[0].get(0, 0), [0, 0, 0, 255]);
        assert_eq!(v.anims[0].frames[0].get(1, 1), [255, 255, 255, 255]);
    }

    #[test]
    fn lint_errors_block_compilation() {
        let bad = SRC.replace("kw.\nframe idle 1", "kw\nframe idle 1");
        let s = parse_sprite("t", "test", &bad).unwrap();
        let errs = compile_sprite(&s, &pal()).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].rule, "row-width");
        assert_eq!(errs[0].line, 9);
    }
}
