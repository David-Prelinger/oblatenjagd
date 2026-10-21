# Audio design (Milestone MA)

Status: **design v1** (game-designer, MA planning; decision D-113). Owner of the musical and sound intent: game-designer. Owner of the patches and patterns: `sound-designer` (created in MA). The engine decides the implementation (synth, pattern format, mixer, streaming vs. pre-render) and records it; this document fixes **what** is heard, **when**, and **how loud**.

Nobody can listen during development. Everything below is written to be checked without ears: exact triggers (events or state), durations in ms and frames (1 frame = 16.7 ms at 60 Hz), pitches as note names (A4 = 440 Hz, equal temperament), envelopes as A/D/S/R in ms, levels in dBFS, and for music: tempo, meter, key, bars, loop point, channel roles and the source of every quoted melody. "Must" ships in MA; "nice" ships if time allows (cut order at the end).

## 1. Sound identity

**8/16-bit chiptune Blasmusik and Ländler.** The game sounds as if a village brass band (*Blaskapelle*) had been squeezed into a 1980s sound chip: oom-pah bass (*Helikon*), after-beat chords (*Nachschlag*), a Flügelhorn or clarinet tune with a second voice in parallel thirds and sixths (the Alpine *Zwiegesang*), a bass drum and a cymbal. Austrian dance meters only: **3/4** (Ländler, Walzer), **2/4** (Polka, Marsch, Galopp), **6/8** (Barcarole, Mozart's May song).

**The satire is in the quotes.** The Salzkammergut sells its composers and its customs on every corner, so the music does too, with affection: Mozart's own Ländler in Salzburg, *Eine kleine Nachtmusik* forced into 3/4, Mozart's *Komm, lieber Mai* at every Maibaum, Haydn's *Surprise* chord when the Oberkellner wakes up, the Radetzky clap-along in the Kurpark, Strauss's *Tritsch-Tratsch* (chit-chat) in the café queue, a Christmas cradle song from Bad Ischl under the water of the Wolfgangsee. Every quote is public domain worldwide (section 7); everything else is original. **No Sound of Music** (Rodgers † 1979), **no Im weißen Rößl** (Benatzky † 1957, Stolz † 1975), **no Lehár** quote (§7: not yet free everywhere), **no Stille Nacht** (too sacred to the region to be a joke; brief).

**One leitmotif ties it together: the „Oblaten-Motiv"**: scale degrees **1–3–5–4**, rhythm ♪ ♪ ♪ ♩ (it scans as „O-bla-ten-jagd"). In B♭: B♭4 D5 F5 E♭5. It opens the title march, appears in the route map, the game over (in minor), the boss (in minor), the café waltz, the credits and the last bar before Ende. Stage pieces may cite it once per loop; they don't have to.

**Mono**, one master bus. No positional audio (a 320 px screen doesn't need it).

## 2. Instruments and channels

### 2.1 Music channels (5 required, a 6th allowed)

| Ch | Role | Default patch | Notes |
|---|---|---|---|
| M1 | **Melodie** (lead) | `fluegelhorn`: pulse 25 %, A 8 / D 80 / S 0.7 / R 60; a scoop from −30 cents to 0 over 20 ms at each note start (brass attack); vibrato 5 Hz ±15 cents after 200 ms | per piece also `klarinette` (pulse 50 %, lowpass 2.5 kHz: odd harmonics like a real clarinet), `stehgeiger` (saw, lowpass 3 kHz, vibrato 6 Hz ±25 cents from 40 ms, A 40), `zither` (pulse 12.5 %, A 2 / D 400 / S 0 / R 200, two voices ±6 cents), `spieluhr` (triangle + sine bell an octave up at −12 dB, A 2 / D 600 / S 0; *as built, S2b review*: plus a quiet sine at the note with D 2 000 ms, the comb's resonance, so a held last note fades instead of stopping dead; recheck for smearing when `unterwasser` uses it) |
| M2 | **Zweite Stimme** (thirds/sixths below M1, or counter-melody) | `tenorhorn`: pulse 25 %, 3 dB under M1, no scoop. *As built (S2a review, accepted)*: the `fluegelhorn` uses duty 75 %, the 25 % pulse inverted (identical harmonics, opposite polarity), so lead and second-voice onsets don't stack their peaks | `klavier` in the café (pulse 12.5 %, A 1 / D 250 / S 0.2 / R 80) |
| M3 | **Bass „Helikon"** | `helikon`: triangle, lowpass 1.2 kHz, A 5 / D 120 / S 0.4 / R 40 (staccato) | `pizzicato` in the café (triangle, A 1 / D 80 / S 0) |
| M4 | **Nachschlag** (the "pah" of oom-pah) | `begleitung`: pulse 50 %, the chord as a frame-rate arpeggio (root, third, fifth, 16.7 ms per note, the chip-tune chord), staccato 90 ms | `orgel` in Falkenstein (square 50 % + octave, sustained, no arpeggio) |
| M5 | **Schlagwerk** (noise and drums) | `grosse_trommel` (sine 120 → 50 Hz in 60 ms + noise 10 ms), `kleine_trommel` (noise bandpass 1.5 kHz 150 ms + sine 180 Hz body 40 ms), `becken` (noise highpass 6 kHz, D 400), `klatschen` (4 noise bursts 10 ms apart, bandpass 1.2 kHz: a hand clap), `triangel` (sine 4.2 kHz, D 300), `glocke` (see `hurry`) | one hit at a time; a new hit cuts the old |
| M6 | optional: ornament / echo | – | only if the engine has the voice; nothing in the "must" list needs it |

**Accompaniment patterns** (the default unless a piece says otherwise):
- **2/4 Polka/Marsch**: M3 on beats 1 and 2 (root, then fifth), M4 on the off-beats ("&" of 1 and 2), M5 `grosse_trommel` on 1 and 2, `becken` with the first beat of each section, `kleine_trommel` roll (32nds) in the last half bar before a new section. *As built (S2a review)*: rolls are 16ths on the songs' sixteenth grid (129 ms apart at 116 BPM); accepted, the chip-style roll reads as a roll and a 32nd grid would double every pattern's step counts.
- **3/4 Ländler/Walzer**: M3 on 1, M4 on 2 and 3 (um-pa-pa), M5 `grosse_trommel` on 1 only; in Ländler B sections `klatschen` on 2 and 3 (the Salzkammergut Ländler is danced with singing and clapping, Austria-Forum „Landlertanz").
- **6/8**: M3 on 1 and 4, M4 on 3 and 6.

Brass-band keys: B♭, E♭, F (as real bands play). Quotes stay in the source's key where that makes checking against the score easier (said per piece).

### 2.2 SFX voices

| Voice | Use | Rule |
|---|---|---|
| **P** player | jump, stroke, throw, crawl, hurt, power-ups, land | one sound at a time |
| **W** world | bump, brick, stomp, roof bounce, axe hit, enemy and boss cues | one sound at a time |
| **I** items / UI | Oblate, 1-up, checkpoint, popups, pause, START, tallies | one sound at a time |
| **A** ambience (nice) | loops: Gelse whine, bee buzz, steam chuffs | one loop at a time, the nearest source (on screen) wins |

Music jingles (fanfares, death, game over, hurry, Tusch) are **music cues**, not SFX: they replace the current piece on the music channels and never compete with SFX for a voice.

## 3. Mix and loudness

| Bus / class | Target |
|---|---|
| Master | soft clip above −3 dBFS, hard ceiling **−1 dBFS**; no sample beyond it in any render |
| Music bus (pieces) | sample peak ≤ −8 dBFS, RMS over one full loop **−20 ± 2 dBFS** |
| Jingles (music cues) | peak ≤ −6 dBFS, RMS over the cue −17 ± 2 dBFS |
| SFX "loud" (1-up, power-ups, hurt, brick, boss hit) | peak −6 dBFS |
| SFX "normal" (jump, Oblate, stomp, bump, bounce, checkpoint, START) | peak −9 dBFS (`jump` is the 0 dB reference for "relative") |
| SFX "soft" (stroke, tally tick, selfie click, footsteps, telegraph extras) | peak −14 … −16 dBFS |
| Ambience loops (nice) | peak −24 … −26 dBFS |

- **Relative balance**: music sits ~6 dB under the "normal" SFX, so a jump is always heard over a full band.
- **Ducking**: while an SFX with priority ≥ 7 plays, the music bus is lowered **−6 dB** (attack 20 ms, release 250 ms).
- **All content below 10 kHz** except clicks and paper-rustle noise (`bills_hang`, centroid ~9 kHz, accepted in the S1 review: the engine renders at 44 100 Hz), so a 22 050 Hz output rate is acceptable (engine's call; 44 100 Hz is fine too). Band-limited oscillators recommended for square/saw above C6 (aliasing would read as noise).
- **DC offset** < 0.5 % of full scale in every render; every cue starts and ends at zero (fade-in/out ≥ 2 ms) so nothing clicks.

## 4. Music: transitions and rules

| Moment | Music |
|---|---|
| **Title** (also the marked title after the ending) | `title` from bar 1, loops |
| START on the title / on „Weiter?" / in the map's Wait | the current piece stops (10 ms fade), SFX `ui_start` |
| **Stage intro** (150 frames) | silence; SFX `station_gong` (nice) when it follows the title, the map or a continue, **not** after a death (the death cue's tail is still ringing) |
| **Playing** begins (after the intro; in `start = playing` games: the first frame and every `Respawned`) | the stage piece from **bar 1** (intro bars included), normal tempo |
| **Pause** (START) | music **pauses in place** (no duck, silence), every SFX voice is cut (10 ms), SFX `pause`; on resume `pause` (2 semitones lower is nice, section 6.3) and the music continues from where it stopped |
| **ZEIT drops to 60** (the HUD turns red; the first frame of play with `time_left == 60` after a frame with 61) | music cue `hurry` (the piece pauses for 1.6 s, then resumes where it was); **nice**: from then on the piece plays **+12 % tempo** (until the next respawn or stage). In the boss fight the bells play on SFX voice I over the boss music instead (no pause mid-fight) |
| **Died** | music cut at once, all SFX voices cut, cue `death` (1.25 s in all, release included; the Dying phase is 60 frames) |
| **GameOver** event | cue `game_over` (cuts any tail of `death`); after it, silence until START |
| **LevelComplete** (Maibaum) | music cut, all SFX cut except voice I, cue `fanfare_clear`; then the SFX `tally_tick` / `tally_end`; the Clear panel is silent |
| **LevelComplete** (`goal_stollen`) | cue `fanfare_finale` |
| **Route map** (Arrive) | `route_map` from bar 1, loops through Walk and Wait (also through Teaser on test routes) |
| **Swimming under water** (Stage 6) | crossfade to `unterwasser` (section 5, S6u) |
| **Arena lock** (Stage 9: the Oberkellner leaves `WAIT`) | café piece fades out over 250 ms; cue `boss_intro`, played at a **cue gain of −12 dB** (S2b review: the render sits at the jingle lint level; the cue director applies the gain, so the Paukenschlag lands ~12 dB above it) |
| **First Telegraph** | cue `boss_surprise`, then `boss` loops |
| **Boss beaten** (`DONE`) | boss loop cut, cue `boss_defeat` (Tusch), 1 s silence, then `cafe_zauner` from its **loop point** until the bell. *As built (D-114, S2b review)*: the loop point is bar 1, so the café resumes with its two cup clinks (service is back) |
| **Ending** | Map: `ending_map` (once); Scene and Bill: `kaffeehaus` (one continuous loop across both phases); Credits: `credits`; Ende: `ende` (once); then the marked title with `title`. A START skip starts the next phase's cue at once (100 ms crossfade); a credits page skip doesn't touch the music |
| Death in the arena | as every death; after the respawn at the checkpoint the café piece starts from bar 1, and the boss cues come again at the next lock |

**Loops**: every piece loops at a bar line from its last bar to its **loop point** (the first bar after its intro). The last bar must lead into the loop bar (usually a dominant). *As built (D-114, accepted in the S2a review)*: a loop plays from sample 0, so the loop point is always bar 1 and a piece's intro bars come round once per loop (`title`: the post-horn call every 37 s, like a Postbus at its stop; `salzburg`: a 2-bar um-pa-pa breath before the K. 606 strain). No separate intro jingles are made; the last bar leads into bar 1. The seam is sample-continuous (no gap, no click; checked, section 9).

**Zuckerschock and Hackl**: no own music (they aren't timed states, and swapping pieces on every power change would be noise). **Nice**: a percussion layer on M5 while big: `zucker_shaker` (noise highpass 7 kHz, 30 ms) on every eighth for Zuckerschock; a woodblock `hackl_tock` (sine 900 Hz + noise 10 ms, D 60) on beat 1 for Hackl.

## 5. Music pieces

Bar length = beats per bar × 60 / BPM. "Loop bars" = the looped part; total = intro + loop. Sections are in bars. Instrumentation is the default from 2.1 unless stated.

### Overview

| ID | Used for | Meter | BPM | Key | Bars (intro + loop) | Loop length | Source | Prio |
|---|---|---|---|---|---|---|---|---|
| `title` | title | 2/4 | 116 | B♭, Trio E♭ | 36 (intro inside the loop, D-114) | 37.2 s | original | must |
| `route_map` | route map | 3/4 | 132 | F | 0 + 16 | 21.8 s | original | must |
| `salzburg` | Stage 1 | 3/4 | 144 | B♭ | 42 (intro inside the loop, D-114) | 52.5 s | Mozart K. 606 No. 1, K. 525 | must |
| `st_gilgen` | Stage 2 | 2/4 | 126 | E♭, Trio A♭ | 4 + 40 | 38.1 s | original | must |
| `brunnwinkl` | Stage 3 | 3/4 | 120 | G | 2 + 32 | 48.0 s | original + „Summ, summ, summ" (trad.) | must |
| `falkenstein` | Stage 4 | 3/4 | 96 | D minor → F | 4 + 32 | 60.0 s | original | must |
| `st_wolfgang` | Stage 5 | 3/4 | 168 | C | 4 + 32 | 34.3 s | original (operetta style) | must |
| `wolfgangsee` | Stage 6 | 6/8 | ♩. = 66 | F | 2 + 32 | 58.2 s | original | must |
| `unterwasser` | Stage 6 under water | 3/4 | 84 | A | 0 + 16 | 34.3 s | „Es wird scho glei dumpa" (trad. / Reidinger) | nice (fallback must) |
| `strobl` | Stage 7 | 2/4 | 132 | B♭, Trio E♭ | 2 + 40 | 36.4 s | original | must |
| `bad_ischl` | Stage 8 | 2/4 | 112 | D, Trio G | 4 + 36 | 38.6 s | Strauss I Radetzky-Marsch; Haydn Kaiserhymne (4 bars, nice) | must |
| `cafe_zauner` | Stage 9 | 2/4 | 144 | A | 34 (intro inside the loop, D-114) | 28.3 s | Strauss II Tritsch-Tratsch-Polka | must |
| `boss_intro` | arena lock | 2/4 | 160 | C | 2 (once) | 1.5 s | Haydn Symphony 94, Andante | must |
| `boss_surprise` | first telegraph | – | – | G major chord | once | 0.6 s | Haydn Symphony 94 („Paukenschlag") | must |
| `boss` | the fight | 2/4 | 160 (nice: 168/176) | C minor, B E♭ | 0 + 32 (A B A′ B) | 24.0 s | original | must |
| `boss_defeat` | third hit | – | – | C | once | 1.5 s | Tusch (original) | must |
| `fanfare_clear` | Maibaum | 6/8 | ♩. = 96 | F | once, ~5 bars | ~6.3 s | Mozart K. 596 | must |
| `fanfare_finale` | `goal_stollen` | 6/8 | ♩. = 88 | F | once, 7 bars | 9.6 s | Mozart K. 596 + leitmotif | must |
| `death` | Died | – | – | – | once | 1.25 s | original | must |
| `game_over` | GameOver | 2/4 | 60 | B♭ minor → B♭ | once, 4 bars | 8.0 s | original | must |
| `hurry` | ZEIT 60 | – | – | bells | once | 1.6 s | original | must |
| `ending_map` | ending Map | 2/4 | 132 | B♭ | once, 4 bars | ~3.7 s | Tusch + leitmotif | must |
| `kaffeehaus` | ending Scene + Bill | 3/4 | 132 | E♭ | 0 + 32 | 43.6 s | original (title theme as a waltz) | must |
| `credits` | ending Credits | 2/4 | 116 | E♭ → B♭ | 0 + 34 | 35.2 s | original (title march) | must |
| `ende` | ending Ende | 3/4 | 84 | B♭ | once, 2 bars | ~4 s | leitmotif | must |

### `title` „Oblatenjagd-Marsch" (must)
- 2/4, 116 BPM (bar 1.034 s), B♭ major, Trio in E♭ (the classic march form goes to the subdominant for the Trio). Intro 4 + A 16 + Trio 16 = 36 bars; **loop point bar 1** (D-114; the call recurs every loop, 37.2 s).
- **Intro (1–4)**: a **post-horn call** on M1 alone (the post horn is the Post's emblem, and Postbus 150 is the running gag): natural harmonics of B♭ only (B♭3, F4, B♭4, D5, F5). Bar 1 F4 B♭4 D5 (♪ ♪ ♩), bar 2 F5 (𝅗𝅥), bar 3 D5 B♭4 F4 D5 (four ♬), bar 4 B♭4 (♩) + rest, under it a `kleine_trommel` roll crescendo in bar 4.
- **A (5–20)**: the march tune, opening with the leitmotif B♭4 D5 F5 E♭5; 8 + 8 bars, the first ending on a half cadence (F), the second on B♭. M2 thirds below. Full accompaniment (2/4 pattern).
- **Trio (21–36)**: E♭ major, softer (M1 −3 dB, M5 kick only on 1), a broad cantabile tune with the leitmotif in augmentation (♩ ♩ ♩ 𝅗𝅥) at bar 21; bar 36 is an **F7** bar (back in B♭: its dominant) leading into the post-horn call's B♭ in bar 1. (v1 said B♭7, which is the Trio's own dominant and would lead to E♭; corrected in the S2a review.)
- **Character**: cheeky and determined: „Bus voll? Dann marschier' ma." The tune the whole game is built on.

### `route_map` „Wanderkarte-Landler" (must)
- 3/4, 132 BPM (bar 1.364 s), F major, A 8 + B 8 = 16 bars, loops from bar 1 (no intro: the map starts on a downbeat). Restarts at bar 1 on every map screen.
- **A**: M1 `klarinette` with yodel-like leaps (sixths, an octave flip on beat 3 of bars 2 and 6), opening with the leitmotif as ♪ ♪ ♪ ♩. (F4 A4 C5 B♭4); M2 parallel thirds (the *Zwiegesang*). **B**: answer phrase in C (dominant) and back; `klatschen` on 2 and 3.
- A map visit lasts 1.3–7.5 s, so mostly A is heard; that is intended (A is the identity).
- **Nice**: the piece is transposed up one semitone per station already walked (the journey "climbs"); a tuning param, no new patterns.

### `salzburg` „Mozart-Landler, eh klar" (must)
- 3/4, 144 BPM (bar 1.25 s), **B♭ major** (the key of Mozart's K. 606). Intro 2 + A 16 + B 16 + C 8 = 42 bars = 52.5 s; **loop point bar 1** (D-114: the vamp recurs every loop and carries K. 606's pickup in bar 2; bar 42's F leads into it). 52.5 s is over the 40 s typical of the other stages, accepted: the form fixes 42 bars at 144 BPM, and the song lint allows 20–90 s.
- **Intro (1–2)**: um-pa-pa alone (M3 + M4).
- **A (3–18)**: **Mozart, *6 Ländlerische Tänze* K. 606, No. 1, the first strain**, transcribed from the old complete edition (AMA, Breitkopf 1886) and arranged for the band: bars 3–10 the strain as written (M1 Flügelhorn), bars 11–18 its repeat with M2 in thirds and `klatschen` on 2 and 3. If the strain is not 8 bars long, repeat it to fill 16 (record the bar count).
- **B (19–34)**: original "Getreidegasse" strain in F major: busy eighth-note runs (the crowd), a 2-bar stop-and-start in bars 27–28 (the selfie stop: M1 holds one note while M4 drops out).
- **C (35–42)**: „Eine kleine Nachtmusik im Ländlertakt": **K. 525, 1st movement, bars 1–4 of the first violin**, transposed to B♭, its 4/4 bars squeezed into 3/4 (the joke: Salzburg's most-sold melody forced into the local dance and limping a little; as built, each of a bar's six notes and rests becomes one eighth, and the quote lint compares pitches only). M1 alone with M3, played twice; bar 42 ends with a cymbal choke (`becken` cut after 60 ms) and leads back to bar 1. The K. 606 strain reaches D6 on the Flügelhorn (the violin's octave): accepted over the "leads ≤ C6" rule for quote fidelity (eighth notes only; the patch is low-passed at 5 kHz).
- **Stage identity**: the only stage built on Mozart himself. Satire target: Mozart on every chocolate box. Sound of Music is deliberately absent; the „Do-Re-Mi / Fotopunkt" sign makes that joke silently.

### `st_gilgen` „Platzkonzert-Polka" (must)
- 2/4, 126 BPM (bar 0.952 s), E♭ major, Trio A♭. Intro 4 + A 16 + Trio 16 + „Zugabe" 8 = 44 bars; **loop point bar 5**.
- **Intro (1–4)**: the bandmaster counts in: bar 1–2 four stick clicks (`kleine_trommel` without the body, rim-click, on each beat), bars 3–4 a full-band chord swell (E♭ → B♭7).
- **A (5–20)**: the fullest Blasmusik of the game: every channel busy, M1 Flügelhorn, M2 thirds, `becken` with every kick in bars 13–20.
- **Trio (21–36)**: A♭ major, cantabile; the leitmotif in the Trio's pickup bar.
- **„Zugabe" (37–44)**: the Trio's last 8 bars **again, a semitone higher (A major)**: the encore nobody asked for („Platzkonzert / Zugabe droht", the bandstand sign); bar 44 slides back down (M3 chromatic A–A♭–G–B♭) into bar 5.
- **Stage identity**: the village band at full blast; the Blasmusiker enemies come from this band. **Nice**: their note-train SFX (`tuba_toot`) is pitched in E♭ major.

### `brunnwinkl` „Bienen-Landler" (must)
- 3/4, 120 BPM (bar 1.5 s), G major. Intro 2 + A 16 + B 8 + A′ 8 = 34 bars; **loop point bar 3**. **No percussion** (a hamlet of five houses).
- **Intro**: `zither` (M1) alone, an open G chord broken upwards.
- **A (3–18)**: M1 `zither` tune drawn as a **waggle dance**: each 4-bar phrase traces a figure eight around its first note (degrees 3-4-5-4-3-2-1-2-3, crossing the start note twice), a mordent (fast up-and-back, 30 ms per note) on every phrase's second bar. M3 soft (−6 dB) on 1, M4 very soft on 2 and 3.
- **B (19–26)**: **„Summ, summ, summ"** (traditional Bohemian folk melody; Hoffmann von Fallersleben's text is not used): first 8 bars, played by M2 `biene` (pulse 12.5 % with a fast 18 Hz vibrato ±20 cents, the buzz); M1 rests.
- **A′ (27–34)**: A's second half, ending on D7 into bar 3.
- **Stage identity**: Karl von Frisch's bees; the quietest stage piece.

### `falkenstein` „Pilgerweg" (must)
- 3/4, 96 BPM (bar 1.875 s), D minor, B in F major. Intro 4 + A 16 + B 16 = 36 bars; **loop point bar 5**.
- **Intro (1–4)**: a recitation on one note, the prayer leader's call: M1 on A4 (♩ ♩ ♩ | ♩ ♩ ♩ | 𝅗𝅥. on G4 | 𝅗𝅥. on A4), under it M4 `orgel` sustaining D minor.
- **A (5–20)**: a slow procession hymn (original): M1 Flügelhorn, M2 a fifth under M1, M3 on 1, M4 `orgel` sustained chords (no um-pa-pa), M5 one `glocke` strike (section `hurry` patch, one strike, −12 dB) on the first beat of bars 5, 9, 13, 17: the Falkensteinkirche's bell. The melody climbs stepwise over the section (the path up the wall).
- **B (21–36)**: F major, the **Hackelwurf**: M1 leaps up a sixth (the throw) and falls in a 4-note arc (the axe's flight) every 4 bars; a yodel phrase (octave flips) in bars 29–32; M4 back to um-pa-pa, `klatschen` on 2 and 3; bar 36 on A7 back to bar 5.
- **Stage identity**: earnest pilgrim hymn and a yodel, gently mocked (the pilgrim jam, „Steinschlag / inklusive").
- **Nice**: a climb layer: while the climb camera has gained more than 8 tiles of height since the checkpoint, M3 plays an octave higher.

### `st_wolfgang` „Operettenwalzer (nicht der aus dem Rößl)" (must)
- 3/4, 168 BPM (bar 1.071 s), **C major** (so the Operettensänger's high C is the tonic), B section in A minor. Intro 4 + A 16 + B 16 = 36 bars; **loop point bar 5**.
- **Intro (1–4)**: the operetta-waltz curtain: a held G7 (M4 sustained), M1 `stehgeiger` sliding up an octave from G4 to G5 over 400 ms (the "Schmalz" scoop), a `triangel` on bar 4.
- **A (5–20)**: an original Viennese-operetta waltz tune, M1 `stehgeiger`, M2 thirds, M3 on 1, M4 on 2 and 3, `triangel` on 1 of every second bar.
- **B (21–36)**: the "duet": M1 and M2 answer each other in 2-bar phrases (tenor and soprano), climbing to a long **C6 held for 2 bars (33–34)**, then the cadence (G7 in bar 36).
- **Stage identity**: operetta schmaltz in the Rößl's village, with **no** Benatzky, Stolz or Lehár melody: the style is free, their tunes are not (section 7). Joke: the band's climax note is the same C6 the Operettensänger sings as a hazard.

### `wolfgangsee` „Dampfer-Barcarole" (must)
- 6/8, ♩. = 66 (bar 1.818 s), F major. Intro 2 + A 16 + B 8 + A′ 8 = 34 bars; **loop point bar 3**.
- **Intro**: the paddle wheel alone: M5 soft noise "chuff" (lowpass 800 Hz, 60 ms) on 1 and 4, M3 rocking F–C.
- **A (3–18)**: a lilting boat song, M1 `klarinette`, M2 sixths below, M3 on 1 and 4, M4 on 3 and 6, the chuff continues (−6 dB).
- **B (19–26)**: **the moving state border** („Landesgrenze / wandert"): the key changes every 4 bars, D minor → B♭ major, as if the music crossed into another *Land*.
- **A′ (27–34)**: A's second half, back in F.
- **Stage identity**: the paddle steamer *Kaiser Franz Josef I.* (1873); „Wie Venedig / nur kälter" is why it's a barcarole. **Nice**: SFX `dampfer_horn`.

### `unterwasser` „Es wird scho glei dumpa" (nice; the fallback treatment is must)
- 3/4, 84 BPM (bar 2.143 s), A major (as in the common printed versions). 16 bars, loops from bar 1.
- **Bars 1–8**: the song's first melodic period (pickup + 8 bars of the tune), M1 `spieluhr`; bars 9–16 the same an octave lower on M2 with M1 adding a high pedal note (E6, 1 per bar). M3 soft triangle on 1, M4 off, M5 a "plip" (sine 600 → 1 200 Hz in 40 ms, −18 dB) on beat 3 of every second bar (bubbles). Lowpass 1.2 kHz on the whole piece.
- **Switching**: both `wolfgangsee` and `unterwasser` run in parallel from the stage start (the muted one keeps advancing), and the mix crossfades in **300 ms**: to `unterwasser` once Resi has been **below the surface** (swimming, her hitbox top under the water line, i.e. not floating at it) for **20 consecutive frames**; back once she has been floating, jumped out or left the water for 20 frames. Floating at the surface counts as above water.
- **Fallback (must)**: if two parallel tracks are too costly, the underwater treatment is a mix state of `wolfgangsee`: lowpass 700 Hz on all channels, M1 an octave lower on a triangle, M4 and M5 muted, same crossfade rules.
- **Why this song**: the „Kripplied" was published in 1884 by Anton Reidinger, who probably wrote its text around 1865 **in Bad Ischl**, and the Wolfgangseer Advent's weekly evening programme is called „Wann's dumpa wird im Dorf". Under the lake it really does get *dumpa*. The floating Advent lantern is in the stage, and „Advent / ganzjährig" is a St. Wolfgang sign. No lyrics are used.

### `strobl` „Durchfahrts-Polka" (must)
- 2/4, 132 BPM (bar 0.909 s), B♭ major, Trio E♭. Intro 2 + A 16 + B 8 + Trio 16 = 42 bars; **loop point bar 3**.
- **Intro**: two car-horn honks as the pickup (M4: A4 + C♯5 together, two ♪ with a ♪ rest, then a ♩ chord on beat 1 of bar 2).
- **A (3–18)**: a busy polka of short repeated notes (traffic), full accompaniment.
- **B (19–26) „Stop-and-go"**: one bar played, one bar **silent except an indicator tick** (M5 `triangel` at −18 dB, filtered to a click, on every eighth), alternating; the music stops at the red lights.
- **Trio (27–42)**: E♭ major, lighter (the old Ischlerbahn trackbed, now a cycle path): a bicycle-bell `triangel` (2 kHz, two strikes) on the first beat of every 4-bar phrase.
- **Stage identity**: the town everybody only drives through.

### `bad_ischl` „Kurkonzert" (must; section C nice)
- 2/4, 112 BPM (bar 1.071 s), **D major** (the Radetzky-Marsch's key), Trio G major. Intro 4 + A 16 + B 16 + C 4 = 40 bars (36 without C); **loop point bar 5**.
- **Intro (1–4) "Die Kurkapelle stimmt"**: all of M1–M4 on A4, starting detuned (−40, +25, −15, +40 cents) and sliding to 0 cents over bars 1–2 (the band tuning to the A), then a `kleine_trommel` roll in bars 3–4.
- **A (5–20)**: **Johann Strauss (Vater), Radetzky-Marsch op. 228**, the first strain's melody (the 16 bars after the march's 4-bar introduction), transcribed from the Haslinger first edition (1848) and arranged for the band. Bars 5–12 as written; bars 13–20 its repeat **with the New Year's Concert clap-along**: M5 `klatschen` on every beat (the audience claps), the drums drop out.
- **B (21–36)**: original "Kurpark-Trio" in G major: a slow promenade (the Kurgäste in bathrobes), M5 a soft `schlurf` (noise lowpass 600 Hz, 80 ms) on beat 2: slippers.
- **C (37–40, nice)**: **Haydn, „Gott erhalte Franz den Kaiser" (Hob. XXVIa:43), bars 1–4 of the melody**, transposed to D, as a solemn chorale: half notes, M1–M4 sustained, no percussion, M1 −3 dB. Then the march resumes at bar 5 with a `becken` crash. **Usage rule** (D-113): this phrase only, only here, no text, never as a victory, ending or credits music. Reason: in Bad Ischl it is the Kaisertage's own ritual (it is sung at the *Kaisermesse* every 18 August, the emperor's birthday), which is what the stage laughs at (k.u.k. nostalgia, the officers' drill); the same melody is also Germany's anthem, so it must stay a local k.u.k. quote and never become a joke about Germany.
- **Stage identity**: k.u.k. nostalgia and spa-park brass. The target is the clap-along ritual and the Kaisertage, not the march's 1848 occasion.

### `cafe_zauner` „Tritsch-Tratsch in der Schlange" (must)
- 2/4, 144 BPM (bar 0.833 s; a *Polka schnell*), **A major** (the polka's key). Intro 2 + A 16 + B 8 + A′ 8 = 34 bars; **loop point bar 3**.
- **Salon instrumentation** (the café's own sound, unlike every other stage): M1 `stehgeiger`, M2 `klavier` (after-beats and thirds), M3 `pizzicato`, M4 `klavier` off-beat chords, M5 only `triangel` on 1 of every second bar and a brush (noise lowpass 3 kHz, 40 ms, −18 dB) on the off-beats.
- **Intro**: two cup clinks (sine 2 637 Hz + 3 951 Hz, D 150, ♩ ♩).
- **A (3–18)**: **Johann Strauss (Sohn), Tritsch-Tratsch-Polka op. 214**, the polka's first strain (16 bars), transcribed from the Haslinger first edition (1858–59, piano) and arranged for the salon trio.
- **B (19–26)**: original „Komm' gleich!" strain in D major: running sixteenths (the waiters' hurry), ending on a counter bell (`triangel` at 2.6 kHz, one strike).
- **A′ (27–34)**: A's last 8 bars, E7 into bar 3.
- **Stage identity**: *Tritsch-Tratsch* means chit-chat; everybody gossips in the queue. Strauss spent his summers in Ischl.
- *As built (S2b review, accepted)*: intro inside the loop (D-114), 34 bars = 28.3 s; the strain is read from the Haslinger first edition's IMSLP preview (piano, p. 3) and played an octave down on the Stehgeiger (E6 → E5, leads ≤ C6); its double stops stay double stops. The run in bars 14 and 32 dips to F♯3, a semitone under a real violin's G string: accepted for a chip patch (range F3–E6) to keep the quote intact.

### Boss: `boss_intro`, `boss_surprise`, `boss`, `boss_defeat` (all must)
- **`boss_intro`** (on the lock frame, when „Hinten anstellen!" appears): **Haydn, Symphony No. 94 „mit dem Paukenschlag" (*Surprise*), Andante, bars 1–2 of the theme**: C4 C4 E4 E4 | G4 G4 E4 (♪ ♪ ♪ ♪ | ♪ ♪ ♩), *pianissimo* (−18 dB under the music bus norm), M1 pulse 12.5 % with A 1 / D 120 / S 0 (plucked strings), M3 C3 on beat 1. At **160 BPM** the 8 eighths last exactly **1 500 ms = the intro's 90 frames**. Verify the notes against the Rietz/Breitkopf score.
- **`boss_surprise`** (first frame of the first `TELEGRAPH` = intro frame 91): the *fortissimo* **G major chord**, all channels (M3 G2, M2 B3, M4 D4, M1 G4) + `grosse_trommel` + `becken`, A 2 / D 600. Haydn's chord to wake up the audience wakes up the waiter.
- **`boss`** „Zahlen-bitte-Galopp" (starts on the bar after the chord, loops): 2/4, **160 BPM** (bar 0.75 s), C minor; 24 bars, loop from bar 1.
  - A (1–8): driving, M3 eighth-note ostinato C–G, M4 off-beat stabs, M1 a rising "Vorsicht, heiß!" phrase ending in a slide up a fifth (steam whistle) in bar 8.
  - B (9–16) „Zahlen, bitte!": E♭ major, M1 a repeated-note "adding up the bill" figure (four ♬ climbing a step per beat), a `triangel` "ding" on beat 1 of bars 12 and 16 (the till).
  - A′ (17–24): A with the leitmotif in minor (1–♭3–5–4: C5 E♭5 G5 F5) in bar 23, G7 in bar 24.
  - The music does **not** follow the dash and breather (their lengths vary); those reads are SFX (section 6).
  - **Nice**: tempo by hits taken, 160 / 168 / 176 BPM, switching on the next bar line after a hit.
- *As built (S2b review, accepted)*:
  - `boss_intro`: the notes are read from **Clementi's piano adaptation** („The Surprise … adapted for the piano forte", London: Clementi, Collard & Collard, c. 1823; Internet Archive `surprisehaydnsce00hayd`), because the Rietz/Breitkopf score is captcha-gated on IMSLP. It is a 19th-century print, PD everywhere, and two bars of the theme leave no room for editing. The pianissimo can't be rendered (jingle RMS floor −19 dBFS), so the cue director plays it at **−12 dB cue gain** (effective peak ≈ −18, RMS ≈ −31 dBFS; section 4). The Helikon holds C3 through both bars (it keeps the cue sounding to its last eighth; tail lint).
  - `boss_surprise`: the chord rolls up from G2 at 17 ms per note (50 ms in all, a chip chord, like the Nachschlag) so the onsets don't stack into one spike; it still reads as one hit.
  - `boss`: A B A′ B = 32 bars = 24 s (the loop lint's 20 s floor); B's second pass ends on G7 into bar 1. The bar-7 climb touches D6 for one quarter (over the ≤ C6 rule, accepted as in `salzburg`).
- **`boss_defeat`** (first frame of `DONE`): a **Tusch** (the brass band's honour flourish): `kleine_trommel` roll (32nds, crescendo) under a held C major chord (M3 C3, M2 E4, M4 G4 arpeggio, M1 C5) for 1 200 ms, then a final `grosse_trommel` + `becken` hit; 1.5 s in all. Then 1 s silence and `cafe_zauner` from bar 3.

### `fanfare_clear` „Komm, lieber Mai" (must)
- On `LevelComplete` at a Maibaum. 6/8, ♩. = 96 (bar 1.25 s), **F major** (the song's key).
- **Mozart, *Sehnsucht nach dem Frühling* K. 596 („Komm, lieber Mai, und mache"), the voice part's first phrase** (pickup to the first cadence, about 4 bars; record the exact count from the AMA score), M1 Flügelhorn, M2 thirds, M3 + M4 in 6/8 pattern, `becken` on its first downbeat; then **one bar of Tusch** (held F major + snare roll + final hit). *As built (S2a review, accepted)*: pickup + 4 bars (AMA, Serie VII No. 37); bar 1 is an oom-pa vamp that carries the pickup (a jingle must not open on 5/6 of a bar of silence), and the Tusch is the cadence bar itself (the lead holds „grün" A4 over the held F major, the roll and the final bass drum + choked cymbal), so the cue stays at 5 bars = 6.25 s. About 6.3 s; it covers Slide, Hop and Walk and may overlap the first Tally ticks (they are SFX, they mix).
- The Maibaum is a May custom, and Mozart wrote the May song four months before he died. **Nice**: a Wurstkranz touch adds the first 300 ms of `oneup` (the Juchizer) over the fanfare.

### `fanfare_finale` (must)
- On `LevelComplete` at the `goal_stollen`. The same K. 596 phrase at ♩. = 80, full band with M2 in sixths and the `becken` on every phrase start, then a 2-bar coda: the **leitmotif** in augmentation (F A C B♭ → F) and a 1.5 s Tusch. About 9 s: it covers the Serve (1.5 s) and the Tally. *As built (S2b review, accepted)*: ♩. = 88, not 80 (at 80 the 7 bars, vamp + phrase + coda, run 10.5 s, over the 10 s jingle cap); 9.6 s, the Tusch bar is 1.36 s.

### `death` „Autsch-Jodler" (must)
- 1 000 ms + 300 ms release, on the music channels (the piece is cut). *As built (S2a review, accepted)*: 1 250 ms in all (600 ms yodel, 100 ms rest, the 300 ms slide, then its ring-out and release); it ends inside the 60-frame Dying phase plus the respawn and still reads as one fall.
- M1 pulse 25 %, legato, a **yodel falling apart**: G5 G4 F5 F4 E5 E4, 100 ms each (octave flips falling by step: the yodel's throat-break gone wrong), 100 ms rest, then M3 triangle **C3 sliding to G2** over 300 ms with a `grosse_trommel` hit at its start („plumps"). Peak −6 dBFS.

### `game_over` „A schöne Leich'" (must)
- 2/4, 60 BPM (bar 2 s), B♭ minor, 4 bars = 8 s, once, then silence.
- A brass-band funeral march (the Viennese love of a beautiful funeral, *a schöne Leich'*): M1 the **leitmotif in minor and augmented** (B♭4 D♭5 F5 E♭5 as ♩ ♩ ♩ ♩), M2 thirds, M3 on every beat, M4 sustained chords, M5 a muffled drum (`grosse_trommel` with D 400 and lowpass 300 Hz) on every beat. The **last chord turns major** (B♭ major, the *Picardy third*): „Weiter?" is a hopeful question. START cuts it (`ui_start`). *As built (S2a review, accepted)*: the leitmotif fixture is checked by pitches only (four even quarters are not a ×2 augmentation of ♪ ♪ ♪ ♩); M4's sustained chords are chip chords arpeggiated at 17 ms (three in-phase held voices overshoot the jingle peak).

### `hurry` „Kirchturmuhr" (must)
- Three strikes of a church bell, 500 ms apart, the last rings out: 1.6 s. Bell = sines at ratios 0.5 (hum), 1.0 (prime), 1.19 (minor third, the bell's *tierce*), 1.5 (fifth), 2.0 (nominal) of A4 = 440 Hz, decays 1.6 s / 1.2 s / 0.9 s / 0.7 s / 0.6 s, strike transient: noise 5 ms bandpass 3 kHz. Peak −8 dBFS.
- Every Salzkammergut stage has a church tower; the bells say „Zeit wird's".

### Ending cues (must)
- **`ending_map`** (Map phase, once): a 1.5 s Tusch on B♭, then the leitmotif broad (B♭4 D5 F5 E♭5 → D5, ♩ ♩ ♩ ♩ | 𝅗𝅥) at 100 BPM, M1 + M2 sixths + full accompaniment; ≈ 3.6 s, then silence until the Scene (or the next cue on a skip). *S2b review*: the Tusch bar plus ♩ ♩ ♩ ♩ | 𝅗𝅥 needs **4** 2/4 bars (v1's "3 bars" was an arithmetic slip); at 100 BPM that is 4.8 s, longer than the Map phase (240 frames = 4.0 s), so the cue plays at **132 BPM** (4 bars = 3.64 s + release, Tusch 0.9 s) and ends before the Scene.
- **`kaffeehaus`** „Kaffeehaus-Walzer" (Scene and Bill, one loop across both): 3/4, 132 BPM, E♭ major, 32 bars (A 16 + B 16), loops from bar 1. The **title march's A tune recast as a salon waltz** (the classic march-into-waltz variation), salon instrumentation as `cafe_zauner`, no percussion except one cup clink on bar 1. Resi finally sits.
- **`credits`**: 2/4, 116 BPM, the title march **Trio (16 bars, E♭) then A (16 bars, B♭)**, full band, ending in a 2-bar Tusch; 34 bars ≈ 35 s (longer than six 3 s pages; at Ende it crossfades to `ende`).
- **`ende`** „Pfiat di": 3/4, 84 BPM, 2 bars, `spieluhr` alone: the leitmotif (B♭4 D5 F5 | E♭5 → B♭4 𝅗𝅥.), ≈ 4 s, then silence until the title.

## 6. SFX

Columns: **Trigger** (a core `Event`, or a state change the app derives; "first frame" = the step on which the state/phase/entity begins), **Voice / priority** (1–9), **Duration**, **Synthesis and pitch contour**, **Peak** (dBFS), **Prio** (must/nice).

### 6.1 Voice rules (must)
1. **Per-frame dedupe**: each SFX id triggers at most once per step (two Oblaten, five officers saluting, three bills: one sound).
2. **Same id retriggers restart** the sound from its start on its voice (never stacked). This is the Oblate rattle of a coin run, by design.
3. **Stealing**: a new SFX on a busy voice replaces the playing one if its priority is ≥ the playing one's (old one faded out in 5 ms); otherwise the new one is dropped.
4. **Off-screen silence**: enemy and world cues derived from entities play only if the entity's hitbox overlaps the camera rectangle on that step.
5. **Cuts**: Died, GameOver and Pause cut all SFX voices; LevelComplete cuts P, W and A (I keeps a just-collected Oblate).
6. **Ducking**: priority ≥ 7 ducks the music (section 3).
7. A mute (M) silences everything without stopping the clocks: pieces and loops keep advancing, so unmuting is in place.

### 6.2 Core events

| Event | SFX id | Voice / prio | Duration | Synthesis and pitch contour | Peak | Prio |
|---|---|---|---|---|---|---|
| `Jumped` | `jump` „Hupf" | P / 4 | 150 ms | pulse 25 %, D4 (294 Hz) → A5 (880 Hz) exponential over 120 ms; A 2 / D 140 / S 0. **Nice** (S1 review: needs a cue-time pitch offset or a second id): Big or Hackl a fourth lower (A3 → E5). A jump-out from water is the same | −9 | must |
| `Landed` | `land` | P / 1 | 50 ms | noise lowpass 500 Hz, A 1 / D 45; only after ≥ 16 airborne frames (the app counts frames off the ground) | −20 | nice |
| `Died` | cue `death` | music | 1.3 s | section 5 | −6 | must |
| `Respawned` | – | – | – | none (the stage piece starts with Playing) | – | – |
| `LevelComplete` | cue `fanfare_clear` / `fanfare_finale` | music | 6–9 s | section 5 (the goal kind decides: a Serve phase = finale) | −6 | must |
| `OblateCollected` | `oblate` „Knusper" | I / 3 | 180 ms | layer 1: noise click 12 ms, highpass 3 kHz (the wafer's crunch); layer 2: pulse 25 % E6 (1 319 Hz) 45 ms then A6 (1 760 Hz) 135 ms, A 1 / D 170 / S 0 | −9 | must |
| `PowerUp`, power now Zuckerschock | `zucker` „Zuckerschock" | P / 8 | 500 ms (the 30-frame freeze) | pulse 50 % arpeggio C5 E5 G5 C6 E6 G6 C7, 70 ms each, vibrato growing from 0 to ±50 cents at 9 Hz (the sugar rush) | −6 | must |
| `PowerUp`, power now Hackl | `hackl` „Hau-ruck" | P / 8 | 500 ms | two anvil strikes at 0 and 160 ms (sines 1 047 Hz + 2 888 Hz + 5 650 Hz, D 150), then saw lowpass 2 kHz G4 → C5 (a fourth up) 180 ms | −6 | must |
| `Hurt` | `hurt` „Schrumpf" | P / 8 | 500 ms | pulse 50 % falling arpeggio G6 E6 C6 G5 E5 C5 G4, 70 ms each, each note bending −50 cents (deflating) | −6 | must |
| `ExtraLife` | `oneup` „Juchizer" | I / 9 | 700 ms | pulse 25 %, a yodel whoop: C6 G5 C6 E6 G6 (90 ms each, 15 ms portamento between: the yodel flips), then C7 250 ms with vibrato 6 Hz ±30 cents, gliding −2 semitones over its last 80 ms | −6 | must |
| `BlockBumped` | `bump` „Klopf" | W / 4 | 90 ms | triangle 140 → 80 Hz over 60 ms + noise click 8 ms bandpass 1.2 kHz (knocking on wood) | −9 | must |
| `BrickBroken` | `brick` | W / 6 | 220 ms | noise bandpass 2 kHz (wide), A 1 / D 200, plus three 15 ms noise ticks at 60, 110, 160 ms (debris) | −6 | must |
| `Stomped` (and no `Bounced` this step) | `stomp` „Patsch" | W / 5 | 110 ms | pulse 50 % A4 → A3 over 70 ms + noise 25 ms lowpass 1 kHz. **Nice**: +2 semitones per chain step (read off the step's score popup: 100, 200, 400 …, capped at +14) | −8 | must |
| `CheckpointReached` | `checkpoint` „Haltewunsch" | I / 7 | 450 ms | the bus stop-request bell: two strikes at 0 and 120 ms of sines E6 + 2.0× + 2.9× partials, D 300 each | −8 | must |
| `GameOver` | cue `game_over` | music | 8 s | section 5 | −6 | must |
| `HacklThrown` | `throw` „Wusch" | P / 5 | 140 ms | noise bandpass, centre 2.5 kHz → 700 Hz, tremolo 25 Hz depth 60 % (the spinning axe) | −10 | must |
| `AxeHit` | `axe_hit` „Tschak" | W / 6 | 100 ms | noise 30 ms lowpass 3 kHz + pulse 50 % 180 → 90 Hz 70 ms. **Nice** `axe_pling` instead when nothing was defeated or hit that step (a bounce off a tray, bus or Fiaker): sine 2.4 kHz + 3.6 kHz, D 200 | −8 | must |
| `CrawlStarted` | `crawl` | P / 5 | 600 ms | noise lowpass 700 Hz, amplitude tremolo 7 Hz (knees on stone), fade-out from 400 ms | −14 | nice |
| `Stroke` | `stroke` „Blub" | P / 2 | 90 ms | sine 280 → 560 Hz over 60 ms + noise 40 ms lowpass 1.5 kHz | −14 | must |
| **`Bounced`** (new, 6.4) | `roof_bounce` „Boing" | W / 5 | 160 ms | pulse 50 % 330 → 392 Hz with vibrato 22 Hz ±100 cents decaying to 0 (sheet metal); plus **nice** `tray_ding` when the bouncer is a Kellner or the Oberkellner: sines 2 093 + 3 136 Hz, D 350 | −8 | must |

### 6.3 Derived moments (no new event)

The app derives these from `Game` state (modes, phases, timers, entity kinds, `state` and `count`) and the step's input, comparing with the previous step where needed. The engine designs the mapper; section 9 says how it is tested.

| Moment (trigger) | SFX id | Voice / prio | Duration | Synthesis and pitch contour | Peak | Prio |
|---|---|---|---|---|---|---|
| START accepted: Title → intro, „Weiter?" → intro, map skip or Wait → intro, an ending phase skip (a new START press on a step whose mode or phase changes) | `ui_start` „Los!" | I / 6 | 200 ms | pulse 25 %, C5+G5 (arpeggio) 60 ms then C6 140 ms | −9 | must |
| Pause entered / left | `pause` „Löffel am Häferl" | I / 9 | 260 ms | sines 2 637 + 3 951 + 5 274 Hz, A 1 / D 250 (a spoon on a cup: Jausenpause); leaving plays the same sound. **Nice** (S1 review): leaving 2 semitones lower | −10 | must |
| StageIntro entered, not from Dying | `station_gong` „Nächster Halt" | I / 3 | 900 ms | sine + 2nd partial at −10 dB, G5 400 ms then E5 500 ms | −12 | nice |
| `ClearPhase::Tally`, every 3rd frame while the bonus grows | `tally_tick` | I / 2 | 25 ms | pulse 12 % B6 (1 976 Hz) (duty in whole percent; S1 review) | −16 | must |
| First frame of `ClearPhase::Clear` | `tally_end` „Kassa" | I / 6 | 500 ms | noise click 10 ms + sines 1 568 + 2 349 Hz, D 450 (an old till's bell) | −9 | must |
| ZEIT drops to 60 | cue `hurry` | music | 1.6 s | section 5 | −8 | must |
| Oberkellner: first frame of each `TELEGRAPH` | `boss_telegraph` „Vorsicht, heiß!" | W / 7 | the telegraph's length: 667 / 600 / 533 ms | cup rattle (noise ticks 8 ms at 15 Hz, bandpass 3 kHz) + a kettle whistle rising sine 1 800 → 2 400 Hz with 20 % noise, getting louder; it ends as the dash starts (rendered at 667 ms; the cue director cuts it at the dash start in the 600 / 533 ms rounds, S1 review) | −8 | must |
| Oberkellner: `DASH` (every 8th frame) | `boss_dash` | W / 3 | 30 ms | noise lowpass 800 Hz (footsteps) | −16 | nice |
| Oberkellner: first frame of `BREATHE` | `boss_breathe` „Schnauf" | W / 7 | 600 ms | noise lowpass, cutoff 1.2 kHz → 300 Hz, amplitude hump (a sigh): **now you may stomp** | −10 | must |
| Oberkellner: first frame of `STUN` (hits 1 and 2) | `boss_hit` „Unerhört!" | W / 8 | 500 ms | crockery clatter: six noise ticks bandpass 4 kHz at 0, 40, 70, 130, 180, 260 ms + slide whistle sine 1 200 → 300 Hz over 400 ms. It replaces that step's `stomp` or `axe_hit` | −6 | must |
| Oberkellner: first frame of `VOLLEY` | `bills_hang` „Zahlen, bitte!" | W / 6 | 400 ms | paper rustle: four noise bursts 60 ms (gaps 20 ms), highpass 3 kHz | −12 | must |
| Rechnung: first frame of `FALL` (one per step) | `bills_fall` | W / 5 | 600 ms | sine 1 500 → 500 Hz with tremolo 10 Hz (paper fluttering down) | −14 | nice |
| Oberkellner: first frame of `INTRO` / of the first `TELEGRAPH` / of `DONE` | cues `boss_intro` / `boss_surprise` / `boss_defeat` | music | – | section 5 | – | must |
| Operettensänger: first frame of `SING` | `hohes_c` „Das hohe C" | W / 6 | 800 ms (the 48 SING frames) | pulse 50 % **C6 (1 046.5 Hz)**, A 60 / R 80, vibrato 6 Hz ±35 cents from 150 ms (rates are whole Hz; S1 review), plus C5 at −12 dB (chest); cut at once if he is defeated | −8 | must |
| Operettensänger: first frame of `INHALE` | `einatmen` | W / 3 | 600 ms | noise bandpass, centre 1 kHz → 2.5 kHz (a big breath) | −16 | nice |
| Popup „Bravo!" spawns | `applaus` | I / 5 | 900 ms | 40 noise grains (5–10 ms, bandpass 1–5 kHz) at fixed pseudo-random offsets (a seeded table, the same every time), swell 100 ms, decay | −12 | nice |
| Kellner: first frame of `TRAY_BOUNCE` | `tray_ding` | – | – | see `Bounced` | – | nice |
| Ampel: cycle frame = `ampel_telegraph` (green) | `car_honk` „Hup-hup" | W / 4 | 300 ms | pulse 50 % A4 + C♯5 (+8 cents) 120 ms, 40 ms gap, 140 ms | −12 | nice |
| Ampel: light changes | `ampel_click` | W / 1 | 10 ms | click | −20 | nice |
| Postbus (`reisebus`, look `postbus`): first frame of its telegraph | `postbus_horn` | W / 6 | 900 ms | saw lowpass 1.4 kHz, D4 + F♯4 together 700 ms, then sagging −1 semitone over 200 ms (it's full) | −10 | nice |
| Other Reisebus: first frame of its telegraph | `bus_horn` | W / 5 | 450 ms | the same patch, E4 + G♯4, two blasts of 200 ms | −10 | nice |
| Fiaker: first frame of its telegraph | `wiehern` | W / 5 | 700 ms | pulse 25 % G5 → C6 → G5 with vibrato 12 Hz ±150 cents + noise breath | −12 | nice |
| Selfie-Tourist / Wartender: the flash frame | `selfie` „Klick" | I / 2 | 40 ms | noise 6 ms + sine 3.5 kHz 30 ms; at least 12 frames between two | −16 | nice |
| Steinschlag: first frame of its telegraph / a rock shatters | `kiesel` / `stein_bruch` | W / 3 / 4 | 600 / 200 ms | noise ticks lowpass 2 kHz (a trickle) / noise lowpass 1.5 kHz | −14 | nice |
| Schneekugel shatters | `glas` | W / 4 | 300 ms | five sines 3–6 kHz at offsets 0–60 ms, D 300 | −12 | nice |
| Schwan: first frame of its telegraph | `zischen` | W / 4 | 500 ms | noise highpass 2.5 kHz (hiss) | −14 | nice |
| Gelse hunting, on screen | `gelse` (loop) | A / – | while hunting | saw 680 Hz, vibrato 6 Hz ±40 cents | −26 | nice |
| Regattaboot: first frame of its telegraph | `startschuss` | W / 5 | 250 ms | noise 50 ms + lowpass tail 200 ms | −10 | nice |
| Dampfer starts its travel | `dampfer_horn` | W / 6 | 1.5 s | saw A2 + C♯3 + E3, lowpass 600 Hz, A 80 | −10 | nice |
| Schafbergbahn leaves a station / while moving | `pfiff` / `dampf` (loop, A) | W / 5 | 600 ms / every 20 frames | steam whistle sine 1 100 Hz + 30 % noise / noise chuff lowpass 800 Hz | −12 / −22 | nice |
| Offizier: first frame of „Habt Acht!" / „Präsentiert!" | `trommelwirbel` / `saebel` | W / 4 | 400 / 400 ms | snare strikes every 25 ms ending in one hit / sines 2.9 + 4.3 kHz, D 400 | −12 | nice |
| Popup „Kurruhe!" spawns | `liegestuhl` | W / 3 | 200 ms | pulse 50 % 90 Hz with vibrato 30 Hz (a creak) | −14 | nice |
| Blasmusiker: a note train starts | `tuba_toot` | W / 3 | 180 ms | saw lowpass 800 Hz, E♭3 G3 B♭3, 60 ms each | −12 | nice |
| Bienenschwarm on screen | `summen` (loop) | A / – | while on screen | saw 220 Hz + 223 Hz (beating), vibrato 9 Hz | −26 | nice |
| E-Biker starts / crashes („Hoppala!") | `radglocke` / `crash` | W / 4 | 200 / 300 ms | sines 2.8 + 4 kHz, two strikes / noise 300 ms + `radglocke` | −12 | nice |
| A power-up rises out of a box | `sprout` | W / 3 | 530 ms | pulse 25 % C4 → C5 in 8 steps over the 32-frame rise | −12 | nice |
| Mozartkugel-Verkäufer throws | `wurf` | W / 2 | 100 ms | noise bandpass 1.5 kHz, short sweep down | −16 | nice |
| Popup „Ballast weg!" spawns | `ballast` | P / 5 | 400 ms | triangle arpeggio C6 → C5 in 8 steps (a harp glissando down: sins shed) | −10 | nice |
| Resi enters swim water / dies in water | `platsch` | P / 4 | 250 ms | noise lowpass 2 kHz + sine 200 → 80 Hz | −10 | nice |
| Route map: Arrive frame 20 (the station marker turns done) | `stempel` | I / 3 | 80 ms | sine 150 → 60 Hz in 60 ms + noise 20 ms (the hiking-pass stamp) | −12 | nice |
| Ending Bill: lines 1–3 appear (frames 20, 40, 60) | `kassa_zeile` | I / 4 | 250 ms | the `tally_end` patch, D 250 | −10 | must |
| Ending Bill: the „Trinkgeld" line appears (frame 80) | `muenze` | I / 4 | 150 ms | sines 3.2 + 4.8 kHz, two strikes 60 ms apart | −12 | must |
| Credits page changes | `seite` | I / 1 | 150 ms | noise sweep highpass 1.5 → 4 kHz | −20 | nice |
| While Zuckerschock / Hackl | `zucker_shaker` / `hackl_tock` (music layer) | M5 | – | section 4 | – | nice |

**Count**: 20 rows for the 19 events (the 18 existing plus `Bounced`; `PowerUp` has two rows): 16 SFX ids, 3 music cues, 1 silent. 44 derived rows. **25 SFX are must** (14 event SFX + 11 derived), ~35 SFX are nice (plus 2 music layers). **25 music cues are must** (title, route map, 9 stages, the underwater fallback, 4 boss cues, 2 fanfares, death, game over, hurry, 4 ending cues); 1 is nice (the `unterwasser` track), plus the nice tempo and layer variants.

Popups that already come with an event don't get a second sound: „+1 Leben" (ExtraLife), „Zwischenstopp" (Checkpoint), „Wirf, Resi, wirf!" (PowerUp), „Zaunerstollen!" (LevelComplete), „Hinten anstellen!", „Unerhört!", „Bitte sehr!" (boss cues), „Komm' gleich!" (`tray_ding`).

### 6.4 The one new core event: `Bounced`

`Event::Stomped` is emitted for real stomps **and** for no-damage bounces off non-stompable tops (D-105 addendum point 5: Fiaker, Reisebus/Postbus, Auto, Regattaboot masthead, Kellner tray, Oberkellner tray). The two must sound different: the bounce tells the player "this one can't be stomped", which is a rule the boss is built on. Deriving it from state is fragile (several contacts per step, no-point Kurgast stomps, boss hits), so core gets **one new output event**:

- **`Bounced`**: emitted on a step whose bounce came **only** from non-stompable tops (no enemy stomped, no Kurgast laid down, no boss hit on that step). `Stomped` stays exactly as it is (emitted on every bounce, real or not), so every replay and test that names `Stomped` keeps its meaning; `Bounced` is extra.
- Events are outputs only: not hashed, no state change, no effect on replays' outcomes, the solver or goldens. Add it to `Event::ALL` and `Event::name`; replays may use `no_event=Bounced`.
- Tests: one unit test per non-stompable top (Fiaker, bus, Postbus, car, masthead, Kellner, Oberkellner tray up), one where a stomp and a roof bounce happen on the same step (no `Bounced`), the Oberkellner's breather stomp (no `Bounced`), and the state hash of an existing replay is unchanged.

Everything else is derived. No other core change is needed for MA.

## 7. Melody sources and the public-domain rule (D-113)

**Rule**: a quoted or paraphrased melody must be by a composer who died **more than 100 years ago** (≤ 1925), or be a traditional melody published by then. Reason: "dead > 70 years" is not worldwide. Colombia protects for life + 80 and Mexico for life + 100 (not retroactive there), so 100 years is the safe line everywhere, and every work below was also published before 1931 (US). We **transcribe the melody ourselves from a public-domain edition** (preferably the 19th-century first or complete editions on IMSLP, not modern critical editions such as the Neue Mozart-Ausgabe, whose editorial work can carry rights in some countries), then harmonise and arrange it ourselves. We never copy a modern arrangement, MIDI file or a CC-licensed engraving, and use no lyrics. Each pattern file records its source in a header (`source`, `edition`, `bars`).

| Melody | Used in | Composer († year) / origin | PD reasoning | Sources |
|---|---|---|---|---|
| *6 Ländlerische Tänze* K. 606, No. 1 (1791, B♭) | `salzburg` A | W. A. Mozart († 1791) | composer † 235 years; edition AMA (Breitkopf 1886) | [IMSLP K. 606](https://imslp.org/wiki/6_L%C3%A4ndlerische_T%C3%A4nze,_K.606_(Mozart,_Wolfgang_Amadeus)) |
| *Eine kleine Nachtmusik* K. 525, I., bars 1–4 (1787) | `salzburg` C | Mozart | as above; transcribed from **Mozart's autograph** (Vienna, 10 Aug 1787; IMSLP #562645), accepted in the S2a review: the AMA file (IMSLP #01776) was captcha-gated, and a manuscript carries no editorial layer at all, so it satisfies the PD rule at least as well | [IMSLP K. 525](https://imslp.org/wiki/Eine_kleine_Nachtmusik,_K.525_(Mozart,_Wolfgang_Amadeus)) |
| *Sehnsucht nach dem Frühling* K. 596 (14 Jan 1791, F) | `fanfare_clear`, `fanfare_finale` | Mozart (text Overbeck † 1821, not used) | as above; AMA Serie VII | [IMSLP K. 596](https://imslp.org/wiki/Sehnsucht_nach_dem_Fr%C3%BChling,_K.596_(Mozart,_Wolfgang_Amadeus)) |
| Symphony No. 94 „Surprise", Andante theme + the *fortissimo* G major chord (1791, premiered London 1792) | `boss_intro`, `boss_surprise` | Joseph Haydn († 1809) | † 217 years; read from Clementi's piano adaptation (London, c. 1823, [Internet Archive](https://archive.org/details/surprisehaydnsce00hayd)), accepted in the S2b review because the Rietz/Breitkopf score (c. 1855) is captcha-gated | [IMSLP Hob. I:94](https://imslp.org/wiki/Symphony_No.94_in_G_major,_Hob.I:94_(Haydn,_Joseph)), [Wikipedia](https://en.wikipedia.org/wiki/Symphony_No._94_(Haydn)) |
| „Gott erhalte Franz den Kaiser" Hob. XXVIa:43 (1797), bars 1–4 | `bad_ischl` C (nice, restricted) | Haydn | as above; text Haschka not used. Restricted use (section 5) because the tune is also Germany's anthem | [IMSLP Kaiserlied](https://imslp.org/wiki/Das_Kaiserlied,_Hob.XXVIa:43_(Haydn,_Joseph)), [Wikipedia](https://en.wikipedia.org/wiki/Gott_erhalte_Franz_den_Kaiser), [Kaisermesse Bad Ischl, 18 Aug](https://www.salzkammergut.at/oesterreich-veranstaltung/detail/430078154/kaisermesse.html) |
| Radetzky-Marsch op. 228 (premiered 31 Aug 1848), first strain | `bad_ischl` A | Johann Strauss (Vater) († 1849) | † 177 years; Haslinger 1848 | [IMSLP op. 228](https://imslp.org/wiki/Radetzky-Marsch,_Op.228_(Strauss_Sr.,_Johann)), [Wikipedia](https://en.wikipedia.org/wiki/Radetzky_March) |
| Tritsch-Tratsch-Polka op. 214 (1858, A major), first strain | `cafe_zauner` A | Johann Strauss (Sohn) († 3 June 1899) | † 127 years; Haslinger 1858–59 | [IMSLP op. 214](https://imslp.org/wiki/Tritsch-Tratsch-Polka,_Op.214_(Strauss_Jr.,_Johann)), [Wikipedia J. Strauss II](https://en.wikipedia.org/wiki/Johann_Strauss_II); Ischl summers: [Bad Ischl music history (sample)](https://content.e-bookshelf.de/media/reading/L-22583705-8ddf81bddb.pdf) |
| „Es wird scho glei dumpa", first period (3/4) | `unterwasser` | melody: a Marian hymn first printed 1765 (Paderborn), shaped by Anton Reidinger († 1912), published 1884; text (not used) Reidinger, probably written in Bad Ischl c. 1865 | both candidates † > 100 years / published 1765 and 1884. The Wikibooks engraving is a CC BY-SA arrangement: use it only to check pitches, harmonise ourselves | [Austria-Forum](https://austria-forum.org/af/AustriaWiki/Es_wird_scho_glei_dumpa), [Wikibooks Liederbuch](https://de.wikibooks.org/wiki/Liederbuch/_Es_wird_schon_gleich_dumpa); „Wann's dumpa wird im Dorf": [Wolfgangseer Advent](https://www.salzkammergut.at/advent/oesterreich-veranstaltung/detail/430005373/wolfgangseer-advent.html) |
| „Summ, summ, summ", first 8 bars | `brunnwinkl` B | traditional melody from Bohemia (text Hoffmann von Fallersleben † 1874, not used) | traditional, 19th century | [Breitkopf sample („M.: trad., aus Böhmen")](https://www.breitkopf.com/assets/haendler/samples/9790004412138.pdf), [Goethe-Institut worksheet („Volksweise")](https://www.goethe.de/resources/files/pdf94/arbeitsblatt_unit_3_summsummsumm.pdf) |

**Style only, no melody** (styles are free, these tunes are not):
- **Franz Lehár** († 24 Oct 1948, Bad Ischl; Lehár Villa): life + 70 has expired in the EU, but under Colombia's life + 80 he is protected until the end of 2028, and his status in Mexico is unclear. The St. Wolfgang waltz and the café scene use operetta *style*, never his tunes. [Lehár Villa](https://en.wikipedia.org/wiki/Leh%C3%A1r_Villa), [Colombia term](https://blogs.ashmolean.org/commons.wikimedia.org/wiki/Commons_Copyright_rules_by_territory/Colombia.html), [Mexico life + 100](https://www.plagiarismtoday.com/2015/09/23/which-country-has-the-longest-copyright-term/).
- **„Im weißen Rößl"** (1930): Ralph Benatzky died **1957** (not 1951), and the show has interpolations by Robert Stolz († 1975) and others, with lyrics by Robert Gilbert. All of it is out. [Benatzky](https://en.wikipedia.org/wiki/Ralph_Benatzky), [The White Horse Inn](https://en.wikipedia.org/wiki/The_White_Horse_Inn_(operetta)).
- **The Sound of Music** (1959): Richard Rodgers († 1979). Out. [encyclopedia.com](https://www.encyclopedia.com/media/encyclopedias-almanacs-transcripts-and-maps/sound-music).
- **The Swiss PostAuto three-tone horn** (from Rossini's *Wilhelm Tell*, PD) is **not** used for Postbus 150: it is Swiss, and no Austrian equivalent was found. The Postbus gets an ordinary two-tone horn. [PostAuto](https://www.postauto.ch/en/about-us-and-news/news/2024/the-postbus-three-tone-horn).
- **Ländler** as a form: slow-to-moderate 3/4, from the Upper Austrian „Landl", and in the Salzkammergut combined with singing and clapping. [Austria-Forum „Landlertanz"](https://austria-forum.org/af/Heimatlexikon/Landlertanz).

## 8. Controls, browser and text

- **M toggles mute** (master gain to 0 and back, 10 ms ramp) in every mode, including pause, the title and the ending. M is free (`jnr-app` `KEY_MAP` uses arrows, X, Space, Y, Z, Shift, Enter; F1/F12 are debug, Escape quits). It is app state, **not** an `InputFrame` bit, so it is never recorded in replays and never reaches core. Session only (no saves, D-042). **Nice**: Minus and Equal/Plus step the master volume through 100 / 70 / 50 / 30 %.
- **Mute toast (nice)**: for 90 frames, `ui.sound_off` „Ton aus – Kurruhe" or `ui.sound_on` „Ton an – Musi, bitte!" (de.toml), top right below the HUD band (y ≥ 22) in `w` with the `k` outline; needs a renderer hook (engine's call).
- **Browser autoplay**: native builds have sound from the first frame (the title march plays at boot). In the wasm build the browser keeps audio locked until a user gesture: the **first key press of any kind** unlocks (creates or resumes) the audio output. Cues before the unlock are dropped, not queued. If that first press is START on the title, the title is left on the same step, so the first sounds are `ui_start` and the intro; the title march is heard on later title visits, or at once if the player presses any other key first. No input is swallowed. An HTML "click to start" overlay is MW's call (the web page), not MA's.
- **No audio device** (the dev container, VNC, CI): the app runs silently and never fails (architecture.md: audio is optional).

## 9. How audio is tested without ears

1. **Offline renders and stats** (`xtask`, engine's naming, e.g. `cargo xtask audio render|stats`): every SFX and piece rendered to WAV into the target dir (not git), with duration, sample peak, RMS, DC offset, and the **loop seam** (the last and first 10 ms around the loop point: no discontinuity above a threshold, no level jump > 1 dB). Checked against sections 3, 5 and 6 (e.g. `jump` 150 ms ± 10, peak −9 ± 1 dBFS). A **spectral-centroid band** per SFX holds its character numerically (`oblate` > 1.5 kHz, `bump` < 600 Hz, `boss_breathe` falling over time).
2. **Piano rolls** (`cargo xtask audio roll <piece>` → PNG): notes per channel over bars, bar lines, the loop marker. The game-designer reviews each piece against this doc, and each quote against its source score bar by bar.
3. **Melody fixtures**: for every quote, the first notes (pitch and length) as a unit-test fixture taken from the source edition, so a pattern edit that breaks a quote fails `check`. The Haydn `boss_intro` (C C E E G G E, 1 500 ms at 160 BPM = 90 frames) is the first fixture.
4. **Audio cue logs from replays** (`cargo xtask replay <file> --audio-log`): the cue mapper is a pure function of (previous step, current step, events, input), so a replay yields a deterministic text log, e.g. `frame 812: cue boss_intro`, `frame 902: sfx boss_telegraph W7`. Text goldens for a handful of replays (Stage 1 tour, `cafe_zauner_boss_win_normal`, a death, a game over and continue, a pause, `qa_m7_accept_full_route` through the ending). They prove the triggers, the dedupe, the stealing and the off-screen rule. Telegraph checks: every `TELEGRAPH`, `BREATHE` and `VOLLEY` start in the boss replays has its cue on that frame.
5. **Mixdown check**: render a replay's full audio (music + SFX) offline and check the master stays ≤ −1 dBFS with no hard clipping, and the music RMS stays in range while ducked and unducked.
6. **Live smoke**: the app starts and plays a replay with audio enabled where a device exists, and without one where none exists (VNC), with no panic or warning spam.

## 10. Scope, cut order and minimum

**Minimum that ships in every case**: the synth, the voice rules, all **must** SFX (25), `title`, `route_map`, `salzburg`, `fanfare_clear`, `death`, `game_over`, `hurry`, the boss cues, the ending cues, the underwater fallback, M to mute, the wasm unlock, the `Bounced` event, cue logs.

**Cut order if MA runs long** (first cut first):
1. Nice SFX (all of them).
2. The nice music variants: tempo steps (hurry +12 %, boss rounds), the map transposition, the climb layer, the power-state layers.
3. `bad_ischl` section C (the Kaiserhymne).
4. `unterwasser` as its own track (the fallback filter stays).
5. Stages 3, 4 and 7 as **reworks** of other pieces instead of their own: `brunnwinkl` = `route_map` in G on the zither without percussion, `falkenstein` = `route_map` in D minor on the organ at 96 BPM, `strobl` = `st_gilgen` at 132 BPM in B♭ with the stop-and-go bars. Each stage keeps its own key, tempo or instrumentation.
6. `kaffeehaus` shortened to 16 bars; `credits` = `title` from bar 21.

## 11. Text

No player-visible text is needed for MA except the optional mute toast: `ui.sound_off` „Ton aus – Kurruhe", `ui.sound_on` „Ton an – Musi, bitte!" (in `content/text/de.toml` now, unused until the toast ships). **Nice**: a music credits page, inserted as page 5 (before the thanks): title „Musik:", lines „frei nach Mozart, Haydn,", „Strauß Vater und Sohn", „und alten Volksweisen.", „Es spielt: 8-Bit-Musi". This makes `ending.credits_pages` 7 and the ending 180 frames longer, so the ending replays and credits goldens are re-cut with it. The text goes into de.toml with that task.

## Proposed MA tasks (design view)

The orchestrator merges this with the engine plan. G = game-designer, S = sound-designer, E = engine, Q = qa-tester, O = orchestrator.

- [x] (G) Audio design (`docs/audio-design.md`) and D-113; mute toast texts in de.toml
- [ ] (O) Create the `sound-designer` agent and the `sfx-authoring` skill (patch and pattern conventions, the PD rule of section 7, the level table of section 3, how to read a piano roll and a cue log)
- [ ] (E) `jnr-audio`: oscillators (pulse with duty, triangle, saw, sine, noise), ADSR, pitch slides, vibrato, arpeggio, simple lowpass/highpass/bandpass; offline render to WAV; stats (peak, RMS, DC, centroid, loop seam); `xtask audio render|stats`
- [ ] (E) Pattern text format (tracker style, rows per beat, channels M1–M5 (+M6), per-channel patch, loop point, header with `source`/`edition`/`bars`), parser and lints (bar lengths per meter, loop point on a bar line, notes in range); `xtask audio roll` piano rolls; melody fixture tests
- [ ] (E) Core: `Event::Bounced` with the tests of section 6.4 (no hash or replay change)
- [ ] (E) Cue mapper (pure: events + state diff + input → cues), voice manager (P/W/I/A, priorities, stealing, dedupe, off-screen rule), music player (intro + loop, pause in place, cuts, crossfades, the parallel underwater track or the fallback filter, nice tempo steps), mixer (buses, ducking, soft clip, ceiling); `xtask replay --audio-log` and cue-log goldens
- [ ] (E) `jnr-app`: re-enable macroquad's `audio` feature; play the mixer's output (streaming or pre-rendered, engine's call: record it, incl. memory on wasm); M mute; wasm unlock on the first key; silent and panic-free without a device; nice: volume keys and the toast
- [ ] (S) The 25 must SFX (section 6), with stats in range
- [ ] (S) Music, must, in this order: `title`, `death`, `fanfare_clear`, `hurry`, `game_over`, `route_map`, `salzburg`, the boss set, `cafe_zauner`, `fanfare_finale`, the ending set, then stages 2–8 and the underwater fallback; each quote transcribed from its PD edition with its header filled in
- [ ] (S) Nice SFX and nice music (the cut order of section 10 in reverse)
- [ ] (G) Review: every piano roll against section 5 and every quote against its score; the cue logs of a Stage 1 tour and the Normal boss win; the levels table; verdict per piece
- [ ] (Q) Cue logs over every flow: title → intro → play → pause → hurry → death → game over → continue → clear → map → … → ending → marked title; the Stage 6 dive and surface switch; the boss telegraph/breather/volley/defeat cues; no SFX twice per step; mute in every mode; the wasm smoke with audio
- [ ] (E/G, nice) The music credits page (section 11) with the ending re-cut
- [ ] (O) Docs: architecture.md (jnr-audio, the cue mapper, the app's audio), validation.md (audio checks), agentic-setup.md (the new agent and skill), a `Bounced` line in game-design.md's event notes
