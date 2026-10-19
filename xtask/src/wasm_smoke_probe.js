// Liveness probe for `cargo xtask wasm-smoke` (never shipped: the smoke
// check serves it only in its own copy of index.html, `smoke.html`).
//
// Wraps the game's per-frame export: after each game frame it reads the
// title prompt's band of the canvas back with gl.readPixels (same task as
// the drawing, so it works without preserveDrawingBuffer) and counts the
// frames on which that band changed. The blinking prompt (30 frames on,
// 30 off) changes it every 30 frames; a frozen frame or a stopped game loop
// never (the first frames may change it once while the title appears). The
// counts go into the DOM (`data-smoke-frames`, `data-smoke-prompt-changes`)
// for --dump-dom. PROMPT_TOP / PROMPT_BOTTOM (the band as fractions of the
// canvas height) are prepended by xtask.
miniquad_add_plugin({
  register_plugin: function (importObject) {},
  on_init: function () {
    var game = wasm_exports;
    var frames = 0;
    var last = null;
    var changes = 0;
    var frame = function () {
      game.frame();
      frames += 1;
      var top = Math.floor(gl.canvas.height * PROMPT_TOP);
      var bottom = Math.ceil(gl.canvas.height * PROMPT_BOTTOM);
      var w = gl.canvas.width;
      var h = bottom - top;
      var px = new Uint8Array(w * h * 4);
      // GL rows count from the bottom.
      gl.readPixels(0, gl.canvas.height - bottom, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px);
      var hash = 0;
      for (var i = 0; i < px.length; i++) {
        hash = (Math.imul(hash, 31) + px[i]) | 0;
      }
      if (last !== null && hash !== last) {
        changes += 1;
      }
      last = hash;
      document.body.dataset.smokeFrames = frames;
      document.body.dataset.smokePromptChanges = changes;
    };
    wasm_exports = Object.assign({}, game, { frame: frame });
    // Headless Chromium with a virtual-time budget runs requestAnimationFrame
    // only a few times; timers do follow virtual time, so drive the frames
    // with one (the game's clock follows virtual time too).
    setInterval(frame, 1000 / 60);
  },
  name: "jnr_smoke_probe",
  version: 1,
});
