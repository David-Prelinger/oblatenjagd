//! The text table (`content/text/de.toml`): player-visible strings by key.
//!
//! Core and the renderer only ever name keys (`"stage.clear"`); the text
//! itself lives in the TOML file. Nested tables flatten to dotted keys.

use std::borrow::Cow;
use std::collections::BTreeMap;

use crate::ParseError;

/// Player-visible texts by dotted key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Texts {
    map: BTreeMap<String, String>,
}

impl Texts {
    /// Parses a text TOML file. Every leaf must be a string; nested tables
    /// become dotted keys (`[stage] clear = "…"` → `stage.clear`).
    pub fn parse(text: &str) -> Result<Texts, ParseError> {
        let table: toml::Table = toml::from_str(text)
            .map_err(|e| ParseError::new(format!("texts: {}", e.to_string().trim())))?;
        let mut map = BTreeMap::new();
        flatten("", &table, &mut map)?;
        Ok(Texts { map })
    }

    /// The text for `key`. A missing key yields the key in brackets
    /// (`[stage.clear]`), so a typo shows up on screen instead of panicking.
    /// (A `Cow`, because the fallback has to be built.)
    pub fn get<'a>(&'a self, key: &'a str) -> Cow<'a, str> {
        match self.map.get(key) {
            Some(s) => Cow::Borrowed(s),
            None => Cow::Owned(format!("[{key}]")),
        }
    }

    /// True if `key` has a text.
    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// All keys in sorted order.
    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.map.keys().map(String::as_str)
    }
}

fn flatten(
    prefix: &str,
    table: &toml::Table,
    out: &mut BTreeMap<String, String>,
) -> Result<(), ParseError> {
    for (k, v) in table {
        let key = if prefix.is_empty() {
            k.clone()
        } else {
            format!("{prefix}.{k}")
        };
        match v {
            toml::Value::String(s) => {
                out.insert(key, s.clone());
            }
            toml::Value::Table(t) => flatten(&key, t, out)?,
            _ => {
                return Err(ParseError::new(format!(
                    "texts: {key} must be a string or a table"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_tables_to_dotted_keys() {
        let t = Texts::parse(
            "top = \"Oben\"\n[hud]\nlives = \"LEBEN\"\n[stage.salzburg]\ntitle = \"Etappe 1: Salzburg\"\n",
        )
        .unwrap();
        assert_eq!(t.get("top"), "Oben");
        assert_eq!(t.get("hud.lives"), "LEBEN");
        assert_eq!(t.get("stage.salzburg.title"), "Etappe 1: Salzburg");
        assert_eq!(
            t.keys().collect::<Vec<_>>(),
            ["hud.lives", "stage.salzburg.title", "top"]
        );
        assert!(t.contains("hud.lives"));
        assert!(!t.contains("hud"));
    }

    #[test]
    fn missing_key_falls_back_to_bracketed_key() {
        let t = Texts::default();
        assert_eq!(t.get("stage.clear"), "[stage.clear]");
        assert_eq!(t.get(""), "[]");
    }

    #[test]
    fn rejects_non_string_leaves_and_bad_toml() {
        let e = Texts::parse("[hud]\nlives = 3\n").unwrap_err();
        assert!(e.message().contains("hud.lives"), "{e}");
        assert!(Texts::parse("[hud]\nlist = [\"a\"]\n").is_err());
        assert!(Texts::parse("not toml").is_err());
    }

    #[test]
    fn german_text_file_parses_with_umlauts() {
        let t = Texts::parse(include_str!("../../../content/text/de.toml")).unwrap();
        assert_eq!(t.get("stage.clear"), "Etappe geschafft!");
        let u = Texts::parse("a = \"Grüß Gott, „Fiaker“ – Ötzi…\"").unwrap();
        assert_eq!(u.get("a"), "Grüß Gott, „Fiaker“ – Ötzi…");
    }
}
