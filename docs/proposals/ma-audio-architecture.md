# MA audio: architecture spike and plan

Status: **proposal** (engine-engineer spike, 2026-10-21, branch `ma-spike`). Nothing here is
implemented yet; the spike code was thrown away. Once accepted, the decisions below go into
`docs/decisions.md` (proposed D-113 … D-117) and the tasks into `docs/milestones.md`.

## Summary

- **Playback:** per-sound playback through macroquad's `audio` feature (quad-snd 0.2.8),
  fed with WAV bytes that our own synth renders **at runtime** from text sources. No custom
  streaming mixer: quad-snd has no callback/streaming API, and building one for ALSA and
  WebAudio ourselves costs far more than it gains.
- **New pure crate `jnr-audio`:** integer-only synth, text formats for SFX patches,
  instruments and songs, a parser with `file:line` errors, a renderer to 44.1 kHz mono i16,
  a WAV writer and the `cues` module (game events and modes → sound cues). It is bit-for-bit
  reproducible on native and wasm, so rendered PCM can be hashed like golden screenshots.
- **Sources** live in `audio/` (as `docs/agentic-setup.md` says), and `jnr-assets/build.rs`
  embeds and lints them like the art. A broken sound is a build error.
- **Validation without ears:** `cargo xtask audio {render,lint,golden}` (WAV, waveform and
  spectrogram PNGs, a piano roll for songs, JSON stats, lints for loudness, timing and music
  theory, PCM hash goldens), `cargo xtask replay <file> --audio` (the offline soundtrack of a
  run) and a real audio capture in `vnc-smoke` through a PulseAudio null sink.
- **Robustness:** audio is optional. A missing device, a missing `audio.js` or a sound that
  fails to decode never stops, hangs or fails the game. Each of these cases was tried in the
  spike (below).

## Spike findings (evidence)

All runs used the `wt-ma-spike` worktree with `CARGO_TARGET_DIR=/workspaces/.cargo-target/wt-ma-spike`.
The throwaway spike turned on `features = ["audio"]` for macroquad, added a module that
renders square-wave WAVs in memory and plays them, and was reverted afterwards.

### Q1: building, devices, smoke tests

| Question | Result |
|---|---|
| Native build with `audio` | Builds (debug in ~8 s). New crates: quad-snd 0.2.8, quad-alsa-sys 0.3.2, audrey 0.3.0, hound, lewton, ogg, dasp_*, smallvec 0.6, maybe-uninit, audir-sles. All are permissive (MIT/Apache, ogg BSD-3-Clause). |
| Sound device in the container | None: `/dev/snd` is missing, `/proc/asound/cards` is missing and `aplay` isn't installed. Only `libasound2`, `libasound2-dev` and `libpulse0` are present. |
| Native runtime, no device | **The game keeps running and exits 0.** macroquad always creates the `AudioContext` while it sets up the window. quad-snd spawns an audio thread, and that thread panics with `Can't open PCM device.` (`alsa_snd.rs:30`). The main thread doesn't notice. libasound prints 9 `ALSA lib … Unknown PCM default` lines to stderr. After that, **every** `play_sound`/`stop_sound`/`set_sound_volume` and every drop of a `Sound` prints `Audio thread died` to **stdout** (once per call: 64 plays gave 64 lines). |
| ALSA `null` device (`pcm.!default { type null }`) | Opens without error, but **the audio thread then spins at 100 % CPU** (`top -H`: one `jnr-app` thread at 100 %) because the null PCM never blocks in `snd_pcm_wait`. Rejected. |
| PulseAudio null sink | `apt-get install --no-install-recommends pulseaudio pulseaudio-utils libasound2-plugins` (36 packages, including libavcodec59). Start it with `pulseaudio --daemonize=yes --exit-idle-time=-1 -n --load="module-null-sink sink_name=jnr_null" --load=module-native-protocol-unix` and `pactl set-default-sink jnr_null`. ALSA `default` then goes to Pulse, the audio thread blocks normally (not in `top`), and `parec -d jnr_null.monitor` **records what the game plays**: a looped 220 Hz square at volume 0.3 showed up as 440 zero crossings/s at peak 1800 (= 6000 × 0.3). `set_sound_volume` works. When Pulse is installed but **not running**, ALSA falls back to the no-device case (no autospawn). |
| `vnc-smoke` with audio on | **PASS** both with Pulse running and without it (spike: a beep on every `Event::Jumped`). With Pulse, the capture held exactly **7 beep onsets, one per `RBJ` jump segment of `testbed_route.replay`**. |
| wasm link (`cargo xtask wasm`, rustc 1.99) | **Fails**: `rust-lld: undefined symbol: audio_init` (and `audio_add_buffer`, `audio_play_buffer`, `audio_source_*`). quad-snd 0.2.8 (the newest on crates.io) declares its JS imports in a plain `extern "C"` without `#[link(wasm_import_module = "env")]` (miniquad 0.4.11 has it), and current rustc no longer passes `--allow-undefined` for wasm32. **Fix that was tested:** a `jnr-app/build.rs` that, only for `wasm32`, emits `cargo:rustc-link-arg-bins=--allow-undefined-file=<manifest>/wasm-audio-imports.txt` and lists the 9 symbols. Only those symbols may stay undefined (all others still fail the link). The module then imports `env.audio_init`, `env.audio_add_buffer`, `env.audio_play_buffer`, `env.audio_source_is_loaded`, `env.audio_source_set_volume`, `env.audio_source_stop` and `env.audio_source_delete`. |
| Which JS file | **`js/audio.js` from the quad-snd 0.2.8 crate** (`~/.cargo/registry/src/*/quad-snd-0.2.8/js/audio.js`, sha256 `764a9bc5634f36d9b508fe2ef6b32befb356b6c797238a8e5cf0d1c881def522`, MIT/Apache). It registers the miniquad plugin `macroquad_audio` version 1, which matches the crate's `macroquad_audio_crate_version() = 1`. It goes after `gl.js` and before `load(...)`. sapp_jsutils is not needed (macroquad's `mq_js_bundle.js` bundles audio.js, sapp_jsutils and quad_net; we only need audio.js). |
| wasm **without** audio.js | gl.js replaces each missing import with a stub that only warns (`No audio_init function in gl.js`, `Missed function: …`). `audio_source_is_loaded` then returns `undefined` for ever, so **`load_sound_from_bytes(..).await` never returns and the game hangs** (the probe saw 0 title-prompt changes in 537 frames; plain `wasm-smoke` failed with "frozen frame"). |
| wasm **with** audio.js in headless Chromium | Loads in 1–2 frames. The game runs (8 prompt changes, as before). `decodeAudioData` works in headless (1 s mono WAV in 2.4 ms, `sampleRate` 44100, `baseLatency` 11.6 ms). The `AudioContext` is `suspended` until a user gesture (autoplay policy; audio.js resumes it on the first keydown, and the title needs Enter anyway). With `--autoplay-policy=no-user-gesture-required`, Chromium's output reached the Pulse null sink (the capture showed the music at peak 1800). With `--virtual-time-budget`, the capture timing isn't real time, so a browser-audio check can only prove presence, not timing. |
| Guard against the hang | **Tested:** pin the `load_sound_from_bytes` future and poll it once per frame with a no-op waker, giving up after N frames. Without audio.js: "load timed out after 60 frames; audio off" and the game runs normally (15 prompt changes). With audio.js: loaded after 2 polls. Natively: loaded on the first poll. |
| Guard against log spam | **Tested:** a panic hook installed in `main()` before `Window::from_config`. It recognises panics whose location file contains `quad-snd` on a thread other than `main`, sets a static `AUDIO_DEAD` flag, prints one line (`jnr-app: audio unavailable (Can't open PCM device.), playing silently`) and passes every other panic to the default hook. The flag was set before frame 3. The hook runs before the dying thread drops its channel, so checking the flag before every call removes the `Audio thread died` lines. The 9 libasound stderr lines can't be removed without a variadic C callback, so they stay (cosmetic). |

### Q2: in-memory WAVs, looping, latency, mixing

- `load_sound_from_bytes(&wav)` takes any RIFF PCM (audrey decodes it). **Native** stores
  every sound as **44.1 kHz stereo f32** (352.8 KB/s, mono is duplicated, any other rate is
  resampled nearest-neighbour), so we render at **44 100 Hz mono i16**. Native load is
  synchronous: 1.5 ms for 0.2 s and 15 ms for 2 s (debug). **Web**: `decodeAudioData` keeps
  mono, resamples to the context rate and finishes 1–2 frames later.
- `PlaySoundParams { looped, volume }`, `set_sound_volume` and `stop_sound` all work. Both
  act on **all** playing instances of a `Sound`. `play_sound` returns `()`: macroquad has no
  per-instance handle, no pause/resume and no seek.
- **Concurrent sounds:** no limit in either backend (native: a `Vec` of voices summed in
  f32 without limiting; web: one `AudioBufferSourceNode` + gain per play). The voice limits
  and the headroom have to be ours.
- **Native latency and jitter:** quad-snd writes fixed blocks of 4096 frames (92.9 ms) into
  an ALSA buffer of 4096 frames and picks up new commands once per block. Clicks requested
  every 250 ms (logged at 0.5001, 0.7505, 1.0001 … s) came out 186 or 279 ms apart:
  **onsets are quantised to 92.9 ms, and latency is 1–2 blocks (~93–186 ms)**. Web:
  `start(0)` plus `baseLatency` 11.6 ms. Native audio is only a dev path here (noVNC carries
  no sound); the browser is where players hear it (MW).
- **Native loop bug:** quad-snd's `Mixer::fill_audio_buffer` doesn't add the write offset
  after a `rewind()`. A loop that wraps in the middle of a block adds the loop start a second
  time at the block start (peak 3600 instead of 1800) and leaves the rest of the block silent
  (a 1912-frame = 43 ms gap). Measured with a 2 s loop (88 200 frames): 4 windows of 10 ms
  with the double-add and one 43 ms gap. **A loop whose length is a multiple of 4096 frames
  is seamless** (90 112 = 22 × 4096: no double-add, no gap). Playback always starts at a
  block boundary, so this alignment is enough. Web looping (`source.loop = true`) is
  sample-exact.
- **Intro + loop:** two sounds can't be chained seamlessly (native block quantisation, and
  no scheduling API). A loop that starts somewhere other than the beginning isn't possible
  either. **Decision: a song is one seamless loop from its first sample. An intro is a
  separate jingle** played on the 150-frame `StageIntro` screen, and the song starts when
  play starts.
- **Custom mixer (our callback):** quad-snd exposes only `Sound::{load, play, stop,
  set_volume, delete}`, so there is no hook for streaming. Our own mixer would need our own
  ALSA thread (unsafe FFI via quad-alsa-sys) **and** a WebAudio plugin. That plugin would
  be either a `ScriptProcessorNode` that calls into wasm on the main thread (deprecated; it
  drops out when rAF/the tab is throttled) or an `AudioWorklet` fed by `postMessage` or a
  `SharedArrayBuffer` (needs COOP/COEP headers, which a plain static host doesn't send).
  Either way it is platform code in two languages that unit tests can't cover. Its benefits
  (sample-exact timing natively, our own voice stealing) matter little: the native latency
  isn't heard by anyone, and the web path is already exact. **Recommendation: per-sound
  playback with pre-rendered PCM.** Determinism doesn't depend on runtime mixing, because
  the rendered PCM is what we hash.

### Q3: render cost (measured)

Throwaway benchmark: 4 voices (pulse, triangle, saw, noise; per-sample envelope and one-pole
low-pass), 16th notes at 140 bpm, **120 s at 44.1 kHz**:

| Build | integer synth | f32 synth | f32 + `sin`/`powf` |
|---|---|---|---|
| native release (aarch64) | **23 ms** | 33 ms | 55 ms |
| wasm32 release in Node 24 (V8) | **42 ms** | 79 ms | 100 ms |
| native debug (opt-level 0) | 389 ms | 445 ms | 495 ms |

The output hashes were equal across native and wasm for all three variants. For the
`sin`/`powf` variant that was luck: wasm uses Rust's `libm` and native uses glibc, and their
results aren't guaranteed to match. **Decision: integer synth** (fastest, and bit-identical
by construction). This also lets the same determinism lint as `jnr-core` cover it. Debug
builds need `[profile.dev.package.jnr-audio] opt-level = 2` (like jnr-core).

Memory decides when to render, not CPU: a 60 s loop takes 21 MB in quad-snd natively
(f32 stereo) and ~10.6 MB as a mono `AudioBuffer` in the browser. Pre-rendering into the
binary would add 5.3 MB of i16 per minute to the wasm. So we **embed the text and render
at runtime**: all SFX at startup (a few ms) and **only the current stage's song**, when its
cue first comes up (≤ 15 ms native release, ≤ 30 ms wasm for 60 s). The previous song is
unloaded.

## Design

### Crates and dependencies

```
jnr-app ──► jnr-audio ──► jnr-core      (cues: Event, Mode, Game queries)
   │            ▲
   └──► jnr-assets ── (build.rs, build-dep) ──► jnr-audio (parse + lint audio/)
xtask ──► jnr-audio (render, stats, lints, offline replay mix)
```

- **`jnr-audio`** (pure, wasm-safe; depends on `jnr-core`, `serde`, `toml`):
  `patch` (SFX and instrument params), `song` (song format and lints), `synth` (integer
  oscillators: pulse with duty, triangle, saw, 15-bit LFSR noise, sine from a 256-entry
  i16 table; ADSR in Q16; pitch slide and vibrato in cents through an integer 2^(c/1200)
  table; one-pole low-pass), `render` (patch/song → `Pcm { rate: 44_100, samples: Vec<i16> }`,
  mono), `wav` (`Pcm` → RIFF bytes and back for tests), `mix` (offline mixer for
  `replay --audio`) and `cues` (below). No floats, no `HashMap` iteration, no
  time/fs/threads. Parsers take `&str` plus a file name and return `file:line: message`
  errors.
- **`jnr-assets`**: `build.rs` also loads `audio/**` through `jnr_audio::Bank::load` and runs
  its lints (errors fail the build, warnings become `cargo:warning`). It embeds the sources
  as `&'static str` (a generated list, so a new file needs no hand-made list) and exposes
  `jnr_assets::audio_sources()`.
- **`jnr-app`** (`src/audio.rs`): the only code that touches `macroquad::audio`. See "App
  adapter" below.
- **Arch-lint updates** (`xtask/src/archlint.rs`): `DEP_RULES` gains
  `("jnr-audio", ["macroquad", "miniquad", "quad-snd", "rand", "getrandom"])`. The
  determinism source scan runs over `crates/jnr-audio/src` too (same rules as `jnr-core`).
  `jnr-audio` may depend on `jnr-core` and nothing else in the workspace.
  `jnr-render` must not reach `jnr-audio` (not needed).

### Sources (`audio/`)

```
audio/
├─ sfx/*.sfx            # one sound effect per file (id = file stem)
├─ jingles/*.sfx        # longer one-shots: stage intro, stage clear, game over, 1-up
├─ instruments/*.inst   # note-driven patches used by songs; kits map letters to patches
└─ music/*.song         # one loop per file (id = [meta] id = file stem)
```

`.sfx` and `.inst` are TOML with integer, unit-suffixed keys (no floats anywhere, so the
`toml` crate never yields `f64`):

```toml
# audio/sfx/jump.sfx: Resi jumps (a short rising square chirp)
[[layer]]                  # 1..=3 layers, mixed; each may start later (delay_ms)
wave = "pulse"             # pulse | triangle | saw | noise | sine
duty_pct = 50              # pulse only
freq_hz = 330              # start pitch (sfx only; instruments take it from the note)
slide_to_hz = 660          # optional: pitch at the end of the sound, glides in cents
attack_ms = 2
hold_ms = 30
decay_ms = 90              # to silence (release_ms/sustain_pct for instruments)
volume_pct = 70
vibrato = { depth_cents = 0, rate_hz = 0 }
arp = { every_ms = 0, semitones = [] }   # e.g. Oblate: { every_ms = 50, semitones = [0, 7] }
lowpass_hz = 8000          # optional
delay_ms = 0
```

`.inst` adds `range = ["e1", "f4"]` (lint) and, for kits, `[kit] K = "kick"`, `S = "snare"`,
`h = "hihat"` (letters → `.sfx`-style patches in the same file).

`.song` = TOML header and bar-structured note lines (the same "TOML + compact text" style as
`.level`):

```toml
# audio/music/salzburg.song
[meta]
id = "salzburg"
tempo_bpm = 132
meter = [3, 4]                 # Ländler; a bar = 3 beats
steps_per_beat = 4             # grid: 16ths, so a bar = 12 steps
key = "g major"                # lint: notes outside the key need a trailing `!`
intro = "stage_intro"          # optional jingle for the StageIntro screen
order = ["A", "A", "B", "A"]   # the loop, played from the start forever

[channels]                     # up to 6 voices; a chord token sounds up to 3 notes
melody = { instrument = "klarinette", volume_pct = 70 }
horn   = { instrument = "fluegelhorn", volume_pct = 50 }
bass   = { instrument = "tuba", volume_pct = 80 }
drums  = { instrument = "kit", volume_pct = 60 }

[patterns.A]
melody = "| d5/4 g5 b5 | a5/6 g5/2 c#5!/4 | ..."  # pitch+octave, /steps (default: the previous length)
horn   = "| r/4 [g4,b4]/4 [g4,b4] | r/4 [f#4,a4]/4 [f#4,a4] | ..."
bass   = "| g2/4 r/8 | d2/4 r/8 | ..."             # r = rest, _ = tie (extends the previous note)
drums  = "| K/4 h h | K/4 h h | ..."
```

Every pattern line in `[patterns.X]` must have the same number of bars.

### Rendering rules

- 44 100 Hz, mono, i16, hard clamp (counted, and the lints treat any clamp as an error).
  `frames_per_tick = 735` (44 100 / 60) exactly, so game frames map to sample offsets without
  rounding (`replay --audio`).
- **Loop alignment:** a song's rendered loop length is rounded to the nearest multiple of
  **4096 frames**. The difference is spread over its rows with Bresenham (a tempo change
  ≤ 0.15 % for a 30 s loop, not audible). This is the workaround for the quad-snd native
  loop bug. A unit test checks `len % 4096 == 0` for every song, and the reason goes into
  the decision record.
- Note releases are rendered **into** the loop: a note's tail that runs past the loop end is
  added back at the loop start, so the wrap has no click.
- Limits (lints): an SFX ≤ 1.5 s, a jingle ≤ 6 s, a song loop 20–90 s (memory budget).

### Cues (`jnr_audio::cues`, pure)

`Director` is fed once per simulation tick with `(&[Event], &Game)` (`mode()`,
`level().meta.music`, `is_paused`) and returns `Vec<Cue>`:
`PlaySfx(SfxId)`, `StartMusic(SongId)`, `StopMusic`, `MusicVolume(pct)` (pause ducks to 0;
quad-snd can't pause), `PlayJingle(Id)`.

- `fn sfx_for(Event) -> Option<SfxId>` is an **exhaustive match**, so a new `Event` variant
  doesn't compile until someone decides its sound (or `None`, with a comment).
- Mode → music: `StageIntro` → the song's `intro` jingle; `Playing` → `StartMusic(level
  music)`, only when the song changes (respawn restarts it); `Dying`/`GameOver`/`StageClear`
  → `StopMusic` plus the jingle; `Title`/`RouteMap`/`Ending` → their own songs.
- Voice policy, shared by the app and the offline mixer: the same SFX within 3 ticks is
  played once (no stacking on simultaneous `OblateCollected`). At most 4 SFX play at once,
  and a lower-priority SFX is dropped when 4 are already playing. The master headroom is
  −6 dB for SFX over music (a volume factor).
- Tests: every `Event::ALL` variant has a decision. Every id the cues can produce exists in
  the embedded bank. Every journey level's `[meta] music` exists (`levels lint` also checks
  this). The cue timeline of `tests/replays/testbed_route.replay` has 7 jump cues at the
  replay's jump frames.

### App adapter (`jnr-app/src/audio.rs`)

- **Startup** (`main()` before `Window::from_config`): install the quad-snd panic guard
  (sets `AUDIO_DEAD`, prints one line). `--mute` (native CLI) and the **M** key (both
  platforms, no text needed) turn sound off at runtime. With `--mute`, nothing is loaded or
  played. macroquad still creates its context, which can't be avoided while the feature is
  on.
- **Loading:** render all SFX at startup and queue them as pending loads. Each pending load
  is a pinned `load_sound_from_bytes` future polled once per frame with a no-op waker. After
  120 frames without a result, audio is switched off with one log line (the missing-audio.js
  case). Loading never blocks the game loop, and a cue for a sound that hasn't loaded yet is
  dropped. A song is rendered and queued when its `StartMusic` cue first comes up (during
  the 150-frame intro), and the previous song is dropped.
- **Calls:** before every macroquad audio call, check `AUDIO_DEAD`, mute and "loaded".
  Volumes are f32 only here (allowed in the app).
- **wasm:** `crates/jnr-app/build.rs` and `wasm-audio-imports.txt` (the 9 quad-snd symbols)
  for the link. `web/audio.js` is an unmodified copy (version and sha256 in `web/README.md`,
  licence file next to it). `index.html` loads it after `gl.js`, and `xtask wasm` copies it.

### Validation without ears

1. **`cargo xtask audio render <id>|--all`** writes to `$CARGO_TARGET_DIR/preview/audio/`:
   `<id>.wav`, `<id>_wave.png` (envelope view), `<id>_spec.png` (spectrogram, log
   frequency), songs also `<id>_roll.png` (a **piano roll** per channel with bar lines; a
   reviewer agent reads it like sheet music), and `<id>.json`:
   `duration_ms, peak_dbfs, rms_dbfs` (unweighted, "LUFS-ish"), `clipped_samples,
   dc_offset_pct, onset_ms` (first sample above −40 dBFS), `silent_tail_ms,
   spectral_centroid_hz, loop_frames, loop_aligned`. The analysis uses f64 and lives in
   xtask, not in `jnr-audio`.
2. **`cargo xtask audio lint`** (`check` step `audio-lint`; the build already rejects syntax
   errors):
   - Signal: no clipping. Peak ≤ −1 dBFS. |DC| < 0.5 %. SFX onset ≤ 10 ms (responsiveness).
     Silent tail ≤ 50 ms. Durations within the limits. Song loop RMS between −20 and
     −14 dBFS. Loop aligned.
   - Music from text: every bar sums to `meter × steps_per_beat`. Every note lies in its
     instrument's `range`. Notes outside the declared key need a `!`. Pattern lines have
     equal bar counts. `order` names only existing patterns. Channels ≤ 6, chords ≤ 3 notes.
     Warning: strong-beat consonance (on beat 1 of a bar, the melody against the sounding
     bass note is a unison, 3rd, 5th, 6th or octave in ≥ 70 % of bars). Warning: melody
     range > 2 octaves. Warning: a pattern repeated more than 4 times in a row.
   - Wiring: every `sfx_for`/mode cue id and every level `music` exists. No unused sources
     (warning).
3. **`cargo xtask audio golden [--accept <id>|--all]`** (`verify` step `audio-goldens`):
   `tests/golden/audio.toml` (text, one line per id: `jump = "fnv64:… frames=…"`). A
   mismatch writes the actual and expected stats and spectrograms to
   `reports/audio-golden/`. Goldens are accepted only after a **game-designer APPROVED**
   verdict on the rendered review set (the audio counterpart of the art-director rule;
   rubric in the new `sfx-authoring` skill).
4. **`cargo xtask replay <file> --audio`** writes `preview/replays/<name>.wav`,
   `<name>_spec.png` and `<name>_cues.txt` (`frame  cue`). It runs the replay, feeds the
   `Director` and mixes at `frame × 735` through `jnr_audio::mix`. This is the intended,
   frame-exact soundtrack of a run, for reviewers and for timing questions ("does the stomp
   sound land on the stomp frame?").
5. **`vnc-smoke`** (real audio): if `pulseaudio` is installed, xtask starts its own daemon
   with a null sink (unless one is running), records `jnr_null.monitor` with `parec` while
   the app plays `testbed_route.replay`, and checks that the capture isn't silent, has no
   clipping and contains ≥ 5 SFX onsets (spike: 7 of 7). Without Pulse the step passes with
   a note "audio not exercised", like the solver-cache note.
6. **`wasm-smoke`**: unchanged checks (they prove that audio never hangs the page), plus a
   unit test that `web/index.html` loads `audio.js` after `gl.js` and that `web/audio.js`
   matches the sha256 recorded in `web/README.md`. A real browser-audio check stays in MW
   (virtual time makes capture timing meaningless).

### Determinism

`jnr-core` is unchanged and stays pure. Audio never feeds back into the simulation: the app
reads events and modes after `step`. `jnr-audio` follows the core rules (integers only,
lint-enforced), so rendered PCM is bit-identical on every platform, and golden hashes are
meaningful. What the speakers do (quad-snd's mixing, quantisation, browser resampling) is
deliberately outside the contract.

## Proposed decisions

- **D-113** (recorded as **D-114**) Audio playback: macroquad `audio` (quad-snd 0.2.8) with runtime-rendered mono
  44.1 kHz WAVs. No custom streaming mixer. A song is a loop from sample 0, rendered to a
  multiple of 4096 frames (quad-snd native loop bug). Intros are separate jingles on the
  StageIntro screen.
- **D-114** (recorded as **D-115**) Audio is optional: the quad-snd panic guard, frame-budgeted loads (120 frames),
  `--mute`/M, and no cue ever blocks or fails the game.
- **D-115** (recorded as **D-116**) wasm link: `--allow-undefined-file` for exactly quad-snd's 9 imports (not a
  global `--allow-undefined`). `web/audio.js` is a pinned copy of quad-snd 0.2.8's
  `js/audio.js`.
- **D-116** (recorded as **D-117**) `jnr-audio`: integer-only synth under the core determinism lint. Sources in
  `audio/`, embedded and linted by `jnr-assets/build.rs`. PCM-hash goldens accepted after a
  game-designer verdict.
- **D-117** (recorded as **D-118**) Container: PulseAudio null sink for audio smoke tests (packages below).

## Task breakdown

Each task is about one focused session. "∥" marks work that can run in parallel with sound
authoring (sound-designer / game-designer).

| Task | What | Depends on | Parallel |
|---|---|---|---|
| **E54** | Platform: enable `audio`; wasm link fix (build.rs + imports list); `web/audio.js` (+ licence, README, `xtask wasm` copies it, index.html); `audio.rs` skeleton with the panic guard, frame-budgeted loader, `--mute`, M key; Dockerfile packages; xtask Pulse helper. | – | ∥ E55 |
| **E55** | `jnr-audio` crate: `.sfx`/`.inst` parser (`file:line` errors), integer synth, renderer, WAV writer; archlint rules; dev opt-level. | – | ∥ E54 |
| **E56** | `xtask audio render/lint/golden` for SFX (WAV, wave and spectrogram PNG, JSON stats, signal lints, `tests/golden/audio.toml`); `check` step `audio-lint`, `verify` step `audio-goldens`. | E55 | ∥ **SFX authoring starts here** |
| **E57** | `.song` format, renderer (loop alignment, wrapped tails), music lints, piano roll in `audio render`. | E55 (E56 for the tool hooks) | ∥ E56; **music authoring starts after E57** |
| **E58** | Embedding (`jnr-assets/build.rs`), `cues::Director` with voice policy, exhaustive `sfx_for`, level `music` ids in the 9 journey levels + `levels lint` check. | E55, E57 (songs can be stubs) | ∥ authoring |
| **E59** | App wiring: SFX at startup, songs per stage (lazy, unload), pause duck, jingles; `vnc-smoke` audio capture assertions. | E54, E58 | ∥ authoring |
| **E60** | `xtask replay <file> --audio` (offline mix, spectrogram, cue timeline). | E58 (E56 PNG helpers) | ∥ authoring |
| **E61** | Docs and agents: `sound-designer` agent (`audio/` only), `sfx-authoring` skill (formats, commands, review rubric), updates to architecture/validation/agentic-setup, decisions D-113…D-117. Can be drafted from this proposal right away and finalised after E57. | E55, E57 for the final format | ∥ everything |

Acceptance criteria per task:

- **E54:** `cargo xtask verify` green with the feature on. Tests: CLI `--mute` parse;
  panic-guard classifier (a quad-snd location on a non-main thread → audio off; anything
  else → default hook) as a pure function; wasm-imports list ⊇ the crate's `extern` block
  (xtask test that reads quad-snd's `web_snd.rs` from the registry, or a pinned list plus
  the version check from `Cargo.lock`); index.html/audio.js sha test. `vnc-smoke` green with
  and without Pulse. `wasm-smoke` green. No `Audio thread died` lines in the `vnc-smoke`
  output.
- **E55:** parse errors carry `file:line` (tests per error kind). Rendering the same patch
  twice gives the same hash. Pulse-wave pitch is correct (zero crossings ± 1). Envelope
  length is exact to the sample. A clamp is counted. WAV round-trips. Arch-lint fails on an
  `f32` in `jnr-audio` (test). `check` stays under budget.
- **E56:** `audio render --all` writes all 4 files per id. Every lint has a failing and a
  passing fixture. Goldens fail on a one-sample change and point to the report files.
  `verify` stays under 10 min.
- **E57:** fixtures for every music lint. A song's length is a multiple of 4096 frames
  (test for each song). A loop wrap has no discontinuity > 1 LSB beyond the waveform
  (test). Piano-roll PNG for a fixture song. Render time of a 90 s, 6-channel fixture
  < 50 ms in release (test-reported).
- **E58:** a broken `audio/` source fails `cargo build` with `file:line`. Exhaustive
  `sfx_for`. Tests "every Event decided", "every cue id exists", "every journey level has
  existing music". Director timeline test on `testbed_route`. Voice-policy unit tests.
- **E59:** the app plays SFX on events and per-stage music, unloads the previous song, and
  pause ducks the music. `vnc-smoke` with Pulse asserts non-silence, no clipping and ≥ 5 SFX
  onsets. With `--mute`, no audio calls are made (counter test). The wasm build still loads
  without audio.js (budget path; manual check documented). Native startup cost is logged.
- **E60:** `replay --audio` on `testbed_route` gives a WAV whose onsets match the cue
  timeline within one frame (test). It is documented in the `replay-testing` skill.
- **E61:** docs updated. The agent and skill exist and are listed in `agentic-setup.md`.
  The work-loop audio gate (game-designer verdict before `audio golden --accept`) is
  described.

## Container changes (for E54)

`.devcontainer/Dockerfile`, added to the apt list (installed and measured in the spike, then
removed again):

```
        pulseaudio \
        pulseaudio-utils \
        libasound2-plugins \
```

`pulseaudio` runs the null sink. `pulseaudio-utils` provides `pactl` and `parec`.
`libasound2-plugins` routes ALSA `default` to Pulse; without it quad-snd can't reach Pulse.
It pulls in libavcodec59 and ~30 other libraries (36 packages in total). `alsa-utils`
(`aplay -l`) is optional for diagnosis. No `post-create.sh` change: xtask starts its own
daemon on demand with
`pulseaudio --daemonize=yes --exit-idle-time=-1 -n --load="module-null-sink sink_name=jnr_null" --load=module-native-protocol-unix`
and `pactl set-default-sink jnr_null`. While Pulse isn't running, ALSA behaves as without
the packages (verified: no autospawn).

## Risks

1. **quad-snd 0.2.8 is old and quirky** (no `wasm_import_module`, the loop bug, fixed
   93 ms blocks, `println!` on errors). Our workarounds depend on its internals. The tests
   above pin the behaviour (sha of audio.js, the imports list, loop alignment). Fallback: a
   `[patch.crates-io]` vendored copy with the 3-line loop fix and `PCM_BUFFER_SIZE = 1024`.
2. **Native timing** (1–2 blocks of latency, 93 ms jitter) is audible only on a native
   desktop, which isn't a shipping target. `replay --audio` shows the intended timing.
3. **Memory** for long or many songs: capped by the loop-length lint and per-stage loading.
4. **Browser autoplay**: silence until the first key press. The title screen needs Enter
   anyway. Document it for MW.
5. **Judging music without ears**: lints catch mistakes but not taste. The piano roll,
   spectrogram, stats and the consonance and range heuristics give the reviewer something
   concrete. Expect iterations, and keep the first songs short (20–40 s loops).
6. **Disk**: the Pulse packages add some tens of MB to the image (the root filesystem
   showed no measurable change at 59 GB, 75 % used).

## As built (E55)

`crates/jnr-audio` (modules `patch`, `note`, `synth`, `tables`, `render`, `wav`, `error`); the
formats are documented in `docs/architecture.md` ("jnr-audio"). Deviations from the design
above:

- **Layers: 1..=8, not 1..=3.** `docs/audio-design.md` (D-113) asks for patches like the
  church bell (5 partials + a noise strike) and `boss_hit` (6 ticks + a whistle).
- **Added for D-113's SFX table** (each cheap and integer-only): `bandpass = { hz, to_hz,
  q_pct }` (Chamberlin state-variable filter, 2× oversampled, unity gain at the centre,
  the centre glides in cents over the sound; centre ≤ 10 kHz, Q 1..=20, the range where it
  stays stable), `highpass_hz` (one-pole, next to `lowpass_hz`), `tremolo = { depth_pct,
  rate_hz }` (the spinning axe, crawling), and `repeat = { count, every_ms }` (a clap's
  bursts, debris ticks; `.sfx` and kit patches only).
- **Instrument keys**: melodic layers use `detune_cents` and `slide_cents` + `slide_ms`
  instead of `freq_hz`/`slide_to_hz`, and an ADSR (`attack_ms`, `decay_ms`, `sustain_pct`,
  `release_ms`) instead of `hold_ms`/`decay_ms`-to-silence. Kits are
  `[kit] K = "kick"` plus `[[patch.kick.layer]]` (`.sfx` keys).
- **Required keys**: `wave`, `volume_pct`, and `freq_hz` in `.sfx` layers (no hidden default
  loudness). Everything else has a default (see the table in `architecture.md`).
- **Noise pitch**: the LFSR steps on every half period of `freq_hz` (so `freq_hz = 20000`
  ≈ white noise at 44.1 kHz). One LFSR mode (15-bit, taps 0 and 1); no "short/metallic" mode.
- **Pulse** is zero-mean and peak-normalised for every duty (high = (100−d)/max(d,100−d),
  low = −d/max(d,100−d) of full scale), so a 12 % pulse has no DC offset.
- **Pitch** updates every 32 samples (0.73 ms); envelopes, oscillators and filters run per
  sample. Envelopes are linear (no exponential decay curve yet).
- **Clamp** is symmetric (±32767); `-32768` counts as clamped. `Render { pcm, clamped }`.
- **Golden fingerprint**: `Pcm::fnv64()` (FNV-1a 64 over the rate and the LE samples), for
  E56's `fnv64:` lines.
- **No `jnr-core` dependency yet** (the arch-lint already allows exactly that one, for E58's
  cues). `check`'s wasm build now builds `-p jnr-app -p jnr-audio`.
- **Determinism**: the pinned-hash test's patch (every wave and feature) and a 60 s
  4-channel note sequence hashed identically from a native release build and a
  `wasm32-unknown-unknown` release build run in Node 24.
- **Render cost** (release, aarch64, 14 cores under a load average of ~10 from other
  agents): a 60 s sequence of 16ths at 140 bpm on 4 channels (2-layer lead with
  vibrato and low-pass, 2-layer tuba, a kit with a band-passed snare; ~24 M voice-samples)
  in **50 ms native, 66 ms wasm** (Node 24); ~2 ns per voice-sample, the band-pass ~8 ns.
  E57's "90 s, 6 channels < 50 ms" will need the song renderer to reuse identical
  pattern renders (see the notes for E57 below) or a lighter target.

Notes for E56/E57:

- E56: `render_sfx(&Sfx) -> Render` gives the PCM and the clamp count; `wav::write` the
  file; `Pcm::fnv64` the golden. `ms_to_samples` (floor of ms × 44.1) is the one rounding
  rule for every time.
- E57: build songs with `render::add_note(&mut mix, start, layers, note, gate, volume_pct)`
  and `render::add_sfx` for kit hits (`InstrumentKind::Kit { hits }`), then `to_pcm`. A note
  sounds `gate + release` samples, so the loop-wrap tail is `mix[loop_len..]` folded back
  onto the start. Identical (instrument, note, gate, volume) voices render identically, so a
  per-song cache of rendered notes or whole pattern instances is safe and the obvious speed-up.

## As built (E56)

`xtask/src/audio/` (`mod.rs` commands and sources, `stats.rs` f64 analysis and FFT, `plot.rs`
images, `lint.rs`, `targets.rs`, `golden.rs`); usage in `docs/validation.md` ("Audio"),
decision D-119.

- **Commands**: `cargo xtask audio render <id>|<file.sfx>|--all`, `audio lint [id]` (`check`
  step `audio-lint`), `audio golden [--accept <id>|--all]` and `audio golden --prune`
  (`verify` step `audio-goldens`, right after the screenshot `goldens`). The proposal's
  `--accept <id>|--all` is kept; `--prune` is new (it only removes lines, so it needs no
  verdict, and the guard hook blocks hand edits).
- **Outputs** (`$CARGO_TARGET_DIR/preview/audio/`): `<id>.wav`, `<id>.json`, `<id>_wave.png`,
  `<id>_spec.png`, and with `--all` the overview `sheet_spectrograms.png` (waveform above
  spectrogram per sound, labelled with `montage`, four per row, one shared time scale of
  1 ms per column up to 1 s). JSON keys as proposed, except `loop_frames`/`loop_aligned`
  (songs, E57) and plus `frames`, `first_sample`, `last_sample`, `pcm`. Silent head and tail
  use the proposal's −40 dBFS. The centroid is magnitude-weighted over Hann-windowed
  1024-point frames; naive (not band-limited) pulses alias, so a 440 Hz square measures
  ~6.1 kHz. Centroid bands in `audio/targets.toml` must be set from measured renders, not
  from the fundamental.
- **Targets** live in `audio/targets.toml` (class, optional `peak_dbfs`, `duration_ms`,
  `centroid_hz`, `placeholder`), not in the `.sfx` files (D-119): `jnr-audio` is unchanged.
- **Lints** (all errors): `clamp`, `ceiling` (−1 dBFS), `peak` (class band or
  `peak_dbfs` ± 1 dB), `rms` (jingles −17 ± 2), `dc` (< 0.5 %), `onset` (≤ 10 ms),
  `tail` (≤ 50 ms), `edges` (first and last sample within −60 dBFS: no click), `length`
  (SFX 20–1500 ms, jingles 200–10 000 ms; the proposal's 6 s jingle limit was raised for
  the design's 8 s `game_over`), `duration` (± 10 ms), `centroid`; plus wiring (target per
  sound and vice versa, unique ids, `jingle` class exactly for `audio/jingles/`). Fixtures:
  `xtask/fixtures/audio/<lint>_{pass,fail}.sfx` with their own `targets.toml`; each `_fail`
  fails only its lint (`clamp_fail` also `ceiling`).
- **Goldens**: `tests/golden/audio.toml` lines carry the hash plus frames, peak, RMS and
  centroid at acceptance (`jump = { pcm = "fnv64:…", frames = 6615, … }`), since the old PCM
  is not kept: a mismatch prints old → new stats and writes the actual review set and the
  stored line to `reports/audio-golden/`. There is no expected WAV or spectrogram (deviation
  from the proposal's "actual and expected spectrograms"). Every non-placeholder sound must
  have a golden; placeholders must not, and `--accept` refuses them and any sound that fails
  its lints. The goldens list is empty at the end of E56.
- **`.claude/hooks/guard-golden.sh`** already covered `tests/golden/`; it now names the audio
  rule (game-designer verdict) for `audio.toml`.

Notes for E57 and the sound-designer:

- E57: add `Kind::Song` (`audio/music/*.song`) in `xtask/src/audio/mod.rs`: its directory in
  `Kind::files`, the render arm in `render`, a target in `target_for` (songs have no
  `targets.toml` entry: add a `Class::Song` with peak ≤ −8 dBFS, RMS −20 ± 2 dBFS, length
  20–90 s to `lint.rs`, or read targets from the song's `[meta]`), the piano roll in
  `write_review`'s per-kind `match`, and `loop_frames`/`loop_aligned` in `stats::to_json`.
  `wiring` checks targets only for `.sfx` kinds. `plot::ms_per_column` already scales long
  sounds to ~1000 columns. Music lints that work on the parsed song (not the PCM) can go in
  a separate function called from `lint_all`.
- Sound-designer: every new `.sfx` needs a `[id]` table in `audio/targets.toml` (at least
  `class`; copy the SFX table's peak into `peak_dbfs` where it differs from the class, its
  duration into `duration_ms`). Review with `cargo xtask audio render --all` and the sheet,
  then ask the game-designer for a verdict before `audio golden --accept <id>`.
- E58: `jnr-assets/build.rs` should skip `audio/targets.toml` (review data, not a source).
