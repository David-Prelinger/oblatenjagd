//! Loads the whole art set from disk: `art/palettes/main.palette`, every
//! `art/sprites/**/*.sprite` and the sprites generated from every
//! `art/generators/*.toml` (see [`crate::generators`]). Used by
//! `jnr-assets/build.rs` and xtask.
//! Paths in diagnostics are relative to the workspace root.

use std::fs;
use std::path::{Path, PathBuf};

use crate::atlas::{Atlas, build_atlas};
use crate::compile::{CompiledSprite, compile_sprite};
use crate::diag::Diagnostic;
use crate::generators;
use crate::lint;
use crate::palette::Palette;
use crate::sprite::{SpriteSource, parse_sprite};

pub const PALETTE_PATH: &str = "art/palettes/main.palette";
pub const SPRITES_DIR: &str = "art/sprites";
pub const GENERATORS_DIR: &str = "art/generators";

/// The parsed art set.
#[derive(Clone, Debug)]
pub struct Project {
    pub palette: Palette,
    /// Hand-made sprites sorted by file path, then generated sprites
    /// (generator files sorted by path).
    pub sprites: Vec<SpriteSource>,
}

impl Project {
    /// Reads and parses everything below `root` (the workspace root).
    /// Returns every syntax error (at most one per file).
    pub fn load(root: &Path) -> Result<Project, Vec<Diagnostic>> {
        let read = |rel: &str| {
            fs::read_to_string(root.join(rel)).map_err(|e| {
                vec![Diagnostic::error(
                    "io",
                    rel,
                    0,
                    0,
                    format!("cannot read: {e}"),
                )]
            })
        };
        let palette = Palette::parse(PALETTE_PATH, &read(PALETTE_PATH)?).map_err(|e| vec![e])?;

        let mut files = Vec::new();
        let dir = root.join(SPRITES_DIR);
        if dir.is_dir() {
            collect_files(&dir, "sprite", &mut files)
                .map_err(|e| vec![Diagnostic::error("io", SPRITES_DIR, 0, 0, e.to_string())])?;
        }
        files.sort();

        let mut sprites = Vec::new();
        let mut errors = Vec::new();
        for path in files {
            let rel_in_sprites = path.strip_prefix(&dir).expect("collected below dir");
            let rel = format!("{SPRITES_DIR}/{}", slash_path(rel_in_sprites));
            let group = rel_in_sprites
                .components()
                .next()
                .filter(|_| rel_in_sprites.components().count() > 1)
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .unwrap_or_default();
            match read(&rel).and_then(|src| parse_sprite(&rel, &group, &src).map_err(|e| vec![e])) {
                Ok(s) => sprites.push(s),
                Err(e) => errors.extend(e),
            }
        }

        let mut gen_files = Vec::new();
        let gen_dir = root.join(GENERATORS_DIR);
        if gen_dir.is_dir() {
            collect_files(&gen_dir, "toml", &mut gen_files)
                .map_err(|e| vec![Diagnostic::error("io", GENERATORS_DIR, 0, 0, e.to_string())])?;
        }
        gen_files.sort();
        for path in gen_files {
            let rel_in_dir = path.strip_prefix(&gen_dir).expect("collected below dir");
            let rel = format!("{GENERATORS_DIR}/{}", slash_path(rel_in_dir));
            match read(&rel)
                .and_then(|src| generators::generate(&rel, &src, &palette).map_err(|e| vec![e]))
            {
                Ok(s) => sprites.extend(s),
                Err(e) => errors.extend(e),
            }
        }

        if errors.is_empty() {
            Ok(Project { palette, sprites })
        } else {
            Err(errors)
        }
    }

    /// All lint findings (errors and warnings), sorted by location.
    pub fn lint(&self) -> Vec<Diagnostic> {
        lint::lint_all(&self.palette, &self.sprites)
    }

    /// Compiles every sprite (base + variants), sorted by name. Fails with
    /// every lint error of the whole set.
    pub fn compile(&self) -> Result<Vec<CompiledSprite>, Vec<Diagnostic>> {
        let errors: Vec<Diagnostic> = self
            .lint()
            .into_iter()
            .filter(Diagnostic::is_error)
            .collect();
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut out = Vec::new();
        for s in &self.sprites {
            out.extend(compile_sprite(s, &self.palette)?);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// Compiles and packs everything into one atlas.
    pub fn atlas(&self) -> Result<Atlas, Vec<Diagnostic>> {
        Ok(build_atlas(&self.compile()?))
    }
}

/// Every file below `dir` (recursively) with extension `ext`.
fn collect_files(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_files(&path, ext, out)?;
        } else if path.extension().is_some_and(|e| e == ext) {
            out.push(path);
        }
    }
    Ok(())
}

fn slash_path(p: &Path) -> String {
    p.components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn real_art_set_loads_lints_and_packs() {
        let p = Project::load(&workspace_root()).unwrap_or_else(|e| {
            panic!(
                "{}",
                e.iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
        let errors: Vec<String> = p
            .lint()
            .iter()
            .filter(|d| d.is_error())
            .map(|d| d.to_string())
            .collect();
        assert!(errors.is_empty(), "{}", errors.join("\n"));
        let a = p.atlas().unwrap();
        assert_eq!(a.pixels.len() as u32, a.width * a.height * 4);
        assert!(a.sprites.windows(2).all(|w| w[0].name < w[1].name));
        let tb = a.sprite("test_box").expect("test fixture sprite");
        assert!(tb.mirrored);
        assert_eq!(
            p.sprites
                .iter()
                .find(|s| s.name == "test_box")
                .unwrap()
                .group,
            "test"
        );
    }

    #[test]
    fn real_generator_drafts_generate_without_lint_findings() {
        let p = Project::load(&workspace_root()).unwrap();
        for name in [
            "bg_sky_salzburg",
            "bg_untersberg",
            "bg_festung",
            "bg_rooftops",
        ] {
            let s = p
                .sprites
                .iter()
                .find(|s| s.name == name)
                .unwrap_or_else(|| panic!("{name} generated"));
            assert!(s.file.starts_with(GENERATORS_DIR), "{}", s.file);
            assert_eq!(s.group, "bg");
            let findings = lint::lint_sprite(s, &p.palette);
            assert!(findings.is_empty(), "{name}: {findings:?}");
        }
    }

    #[test]
    fn broken_sprite_reports_file_and_line() {
        let dir = std::env::temp_dir().join(format!("artkit-project-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("art/palettes")).unwrap();
        fs::create_dir_all(dir.join("art/sprites/player")).unwrap();
        fs::write(dir.join(PALETTE_PATH), ". none t\nk #000000 k\n").unwrap();
        fs::write(
            dir.join("art/sprites/player/bad.sprite"),
            "sprite = bad\nsize = 2x1\nanim = idle frames=1\n---\nframe 0\nkkk\n",
        )
        .unwrap();
        let p = Project::load(&dir).unwrap();
        let errs = p.compile().unwrap_err();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(errs.len(), 1);
        assert_eq!(
            errs[0].to_string(),
            "art/sprites/player/bad.sprite:6:3: error[row-width]: row of frame `idle 0` is 3 wide, but `size` is 2x1"
        );
    }

    #[test]
    fn generator_files_add_sprites_and_report_errors() {
        let dir = std::env::temp_dir().join(format!("artkit-gen-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("art/palettes")).unwrap();
        fs::create_dir_all(dir.join(GENERATORS_DIR)).unwrap();
        fs::write(
            dir.join(PALETTE_PATH),
            ". none t\nS #0000ff S\nh #ffffff h\n",
        )
        .unwrap();
        fs::write(
            dir.join("art/generators/sky.toml"),
            "[[sky]]\nname = \"bg_a\"\nwidth = 8\nheight = 4\nbands = [{ colour = \"S\" }]\n",
        )
        .unwrap();
        let p = Project::load(&dir).unwrap();
        let s = &p.sprites[0];
        assert_eq!((s.name.as_str(), s.group.as_str()), ("bg_a", "bg"));
        assert_eq!(s.file, "art/generators/sky.toml");
        assert!(p.atlas().unwrap().sprite("bg_a").is_some());

        fs::write(
            dir.join("art/generators/sky.toml"),
            "[[sky]]\nname = \"bg_a\"\nwidth = 8\nheight = 4\nbands = [{ colour = \"x\" }]\n",
        )
        .unwrap();
        let errs = Project::load(&dir).unwrap_err();
        let _ = fs::remove_dir_all(&dir);
        assert_eq!(
            errs[0].to_string(),
            "art/generators/sky.toml:5:21: error[generator]: `x` is not a palette char (see art/palettes/main.palette)"
        );
    }
}
