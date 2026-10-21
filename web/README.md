# web/

Host page for the wasm build of Oblatenjagd. Assemble and serve with
`cargo xtask wasm [--serve]`; check it in headless Chromium with
`cargo xtask wasm-smoke`.

- `index.html`: full-window canvas (`#glcanvas`, pixelated), loads `gl.js`,
  then `audio.js`, then `jnr-app.wasm`. Sets `<body data-wasm="loaded">` once the module
  is instantiated (smoke-test marker).
- `gl.js`: unmodified copy of `js/gl.js` from the **miniquad 0.4.11** crate
  (the version in `Cargo.lock`, pulled in by macroquad 0.4.16),
  https://github.com/not-fl3/miniquad, (c) 2019-2020 Fedor Logachev,
  licensed MIT OR Apache-2.0; we use it under MIT, the licence text is in
  `gl.js.LICENSE-MIT` (copied from the crate, shipped next to `gl.js` in
  the assembled web dir). sha256
  `3e40bcbd9645c264645fcf1d174b8adedc706a9bb45aa72d271d37755005fa15`.
  When miniquad is upgraded, copy the new crate's `js/gl.js` here
  (`~/.cargo/registry/src/*/miniquad-<version>/js/gl.js`, or
  `$CARGO_HOME/registry/...`).
- `audio.js`: unmodified copy of `js/audio.js` from the **quad-snd 0.2.8**
  crate (the version in `Cargo.lock`, pulled in by macroquad 0.4.16's
  `audio` feature), https://github.com/not-fl3/quad-snd, (c) Fedor
  Logachev, licensed MIT OR Apache-2.0 (`MIT/Apache-2.0` in its
  Cargo.toml); we use it under MIT. The crate ships no licence file, so
  `audio.js.LICENSE-MIT` is the same MIT text as `gl.js.LICENSE-MIT` (same
  author), shipped next to `audio.js`. sha256
  `764a9bc5634f36d9b508fe2ef6b32befb356b6c797238a8e5cf0d1c881def522`.
  It registers the miniquad plugin `macroquad_audio` (version 1) and
  provides the 9 `audio_*` imports listed in
  `crates/jnr-app/wasm-audio-imports.txt` (D-116). Without it the page
  still runs: gl.js stubs the imports, and the app's frame-budgeted loader
  gives up on audio. When quad-snd is upgraded, copy the new crate's
  `js/audio.js` here, update the sha256 and re-check the imports list
  (`cargo test -p xtask audio` fails until both match).
