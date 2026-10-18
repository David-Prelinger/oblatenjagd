# web/

Host page for the wasm build of Oblatenjagd. Assemble and serve with
`cargo xtask wasm [--serve]`; check it in headless Chromium with
`cargo xtask wasm-smoke`.

- `index.html`: full-window canvas (`#glcanvas`, pixelated), loads `gl.js`
  and then `jnr-app.wasm`. Sets `<body data-wasm="loaded">` once the module
  is instantiated (smoke-test marker).
- `gl.js`: unmodified copy of `js/gl.js` from the **miniquad 0.4.11** crate
  (the version in `Cargo.lock`, pulled in by macroquad 0.4.16),
  https://github.com/not-fl3/miniquad, (c) 2019-2020 Fedor Logachev,
  licensed MIT OR Apache-2.0. sha256
  `3e40bcbd9645c264645fcf1d174b8adedc706a9bb45aa72d271d37755005fa15`.
  When miniquad is upgraded, copy the new crate's `js/gl.js` here
  (`~/.cargo/registry/src/*/miniquad-<version>/js/gl.js`, or
  `$CARGO_HOME/registry/...`).
