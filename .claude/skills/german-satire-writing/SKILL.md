---
name: german-satire-writing
description: Tone and language guide for all player-visible German text - Austrian/Salzkammergut flavour, satire limits, glossary of local terms and tropes, and how to add text keys. Use when writing or reviewing any in-game text or comedic content.
---

# German satire writing

Status: approved v1. Real names of places, businesses and historical people are allowed (D-008).

## Language
- Standard German that is understandable everywhere, seasoned with Austrian words. Not full dialect.
- Correct umlauts and ß (ä ö ü Ä Ö Ü ß), Austrian quotation marks „…".
- Short: UI text ≤ 24 characters per line (320 px wide screen, 8 px font). Signs in the level ≤ 12 characters.

## Glossary (use these)
| Term | Meaning / use |
|---|---|
| Servus, Grüß Gott, Pfiat di | greetings / goodbye |
| oida | "dude" – sparingly |
| Jause | snack |
| Hackl | small axe (St. Wolfgang's legend) |
| Fiaker | horse carriage |
| Sommerfrische | the 19th-century summer escape of the Viennese to the Salzkammergut |
| Kurgast | spa guest |
| Zuckerschock | sugar rush (power-up state) |
| Traundl | traditional wooden boat (as used in this game) |
| Oblaten | the coins: Zauner Oblaten |

## Satire guidelines
- Targets: mass tourism, kitsch and souvenirs, Mozart marketing, Sound of Music tours, bureaucracy, k.u.k. nostalgia, holiday-flat prices, traffic around the lake, local pride.
- Base jokes on real facts (Postbus 150, Mozart's mother born in St. Gilgen, St. Wolfgang's axe throw, Kaiser Franz Joseph's summers in Bad Ischl, the Ischlerbahn closing in 1957).
- Affectionate, never mean to individuals or groups; no politics of today, no religion mocked (pilgrims are funny, faith is not the joke).

## Adding text
- All text goes into `content/text/de.toml` under a dotted key (`stage.salzburg.sign_bus`). Code and levels only reference keys.
- After adding keys, run `cargo xtask levels lint` (checks that every referenced key exists).
