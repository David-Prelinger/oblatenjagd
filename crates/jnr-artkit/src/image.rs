//! A minimal RGBA8 image used for compiled frames, atlases and previews.

/// RGBA8 image, row-major, `width * height * 4` bytes. Starts transparent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32) -> Image {
        Image {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    fn index(&self, x: u32, y: u32) -> usize {
        ((y * self.width + x) * 4) as usize
    }

    pub fn get(&self, x: u32, y: u32) -> [u8; 4] {
        let i = self.index(x, y);
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }

    pub fn set(&mut self, x: u32, y: u32, px: [u8; 4]) {
        let i = self.index(x, y);
        self.rgba[i..i + 4].copy_from_slice(&px);
    }

    /// Fills the whole image with one colour.
    pub fn fill(&mut self, px: [u8; 4]) {
        for chunk in self.rgba.chunks_exact_mut(4) {
            chunk.copy_from_slice(&px);
        }
    }

    /// True if every pixel is fully transparent.
    pub fn is_transparent(&self) -> bool {
        self.rgba.chunks_exact(4).all(|p| p[3] == 0)
    }

    /// Horizontally flipped copy (left-facing version).
    pub fn flipped_h(&self) -> Image {
        let mut out = Image::new(self.width, self.height);
        for y in 0..self.height {
            for x in 0..self.width {
                out.set(self.width - 1 - x, y, self.get(x, y));
            }
        }
        out
    }

    /// Copies `src` onto `self` at (x, y); opaque pixels of `src` replace the
    /// destination, transparent ones keep it. Clips at the edges.
    pub fn blit(&mut self, src: &Image, x: u32, y: u32) {
        for sy in 0..src.height {
            for sx in 0..src.width {
                let (dx, dy) = (x + sx, y + sy);
                let px = src.get(sx, sy);
                if dx < self.width && dy < self.height && px[3] != 0 {
                    self.set(dx, dy, px);
                }
            }
        }
    }

    /// Nearest-neighbour upscale by an integer factor.
    pub fn scaled(&self, factor: u32) -> Image {
        let f = factor.max(1);
        let mut out = Image::new(self.width * f, self.height * f);
        for y in 0..out.height {
            for x in 0..out.width {
                out.set(x, y, self.get(x / f, y / f));
            }
        }
        out
    }

    /// Encodes as an RGBA PNG (feature `png`, used by xtask only).
    #[cfg(feature = "png")]
    pub fn encode_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, self.width, self.height);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut writer = enc.write_header().expect("png header into Vec");
            writer
                .write_image_data(&self.rgba)
                .expect("png data into Vec");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: [u8; 4] = [255, 0, 0, 255];
    const G: [u8; 4] = [0, 255, 0, 255];

    #[test]
    fn flip_mirrors_columns() {
        let mut img = Image::new(3, 1);
        img.set(0, 0, R);
        img.set(1, 0, G);
        let f = img.flipped_h();
        assert_eq!(f.get(2, 0), R);
        assert_eq!(f.get(1, 0), G);
        assert_eq!(f.get(0, 0), [0; 4]);
        assert_eq!(f.flipped_h(), img);
    }

    #[test]
    fn blit_keeps_transparent_and_clips() {
        let mut dst = Image::new(2, 2);
        dst.fill(G);
        let mut src = Image::new(2, 2);
        src.set(1, 1, R);
        dst.blit(&src, 1, 1); // only src(0,0) lands inside; it is transparent
        assert_eq!(dst.get(1, 1), G);
        dst.blit(&src, 0, 0);
        assert_eq!(dst.get(1, 1), R);
        assert_eq!(dst.get(0, 0), G);
    }

    #[test]
    fn scale_and_transparency() {
        let mut img = Image::new(1, 1);
        assert!(img.is_transparent());
        img.set(0, 0, R);
        assert!(!img.is_transparent());
        let s = img.scaled(3);
        assert_eq!((s.width, s.height), (3, 3));
        assert_eq!(s.get(2, 2), R);
    }

    #[cfg(feature = "png")]
    #[test]
    fn png_roundtrip_header() {
        let bytes = Image::new(2, 3).encode_png();
        assert_eq!(&bytes[1..4], b"PNG");
    }
}
