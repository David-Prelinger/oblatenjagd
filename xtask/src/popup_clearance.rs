//! E53 (D-106/D-107 "As built (E53)"): the popups that keep their place
//! (`jnr_core::game::PopupPlace`, the Oberkellner's hit labels and the
//! `goal_stollen` pair) never cover Resi and never reach into the HUD, on
//! any frame. The replay format can't check rectangles, so this replays
//! the files (the `qa_m5_zaunerstollen_clear_of_hud` pattern) and measures
//! what the renderer really draws: each such popup alone into an empty
//! framebuffer with the real font, its drawn pixels against her drawn look
//! (16 × 24 / 32 at her feet, the look shown this frame) and against screen row
//! `jnr_core::game::HUD_CLEAR_Y`. Every Stage 9 boss and finale replay,
//! plus random play branched off the three boss wins around each hit.

use std::fs;
use std::path::{Path, PathBuf};

use jnr_core::game::{DODGE_SPEED_PX, HUD_CLEAR_Y, Mode, PopupPlace};
use jnr_core::{EntityKind, Game, InputFrame, Popup, Replay, Texts};
use jnr_render::Font;
use jnr_render::Framebuffer;

/// Not a palette colour: what the empty framebuffer is filled with.
const EMPTY: u32 = 0x0102_0304;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// What the run saw: frames with a boss label / a Stollen popup up.
#[derive(Default)]
struct Seen {
    labels: u32,
    stollen: u32,
    /// Frames on which the labels jumped (moved more than
    /// `DODGE_SPEED_PX` between two frames they were both up): the
    /// last-resort rule when she comes at them faster than they slide.
    jumps: u32,
    /// The labels' text popup position (whole px) on the last frame.
    last: Option<(i32, i32)>,
}

struct Checker {
    font: Font,
    texts: Texts,
    fb: Framebuffer,
}

impl Checker {
    fn new() -> Checker {
        let texts = fs::read_to_string(root().join("content/text/de.toml")).unwrap();
        Checker {
            font: Font::from_atlas(jnr_assets::atlas()),
            texts: Texts::parse(&texts).unwrap(),
            fb: Framebuffer::new(),
        }
    }

    /// The screen box `(left, top, right, bottom)` (exclusive) of what
    /// popup `e` draws, if anything.
    fn drawn_box(&mut self, g: &Game, e: &jnr_core::Entity) -> Option<(i32, i32, i32, i32)> {
        let EntityKind::Popup(p) = e.kind else {
            return None;
        };
        self.fb.clear(EMPTY);
        jnr_render::entities::draw_popup(
            g,
            jnr_assets::atlas(),
            &self.font,
            &self.texts,
            &mut self.fb,
            g.camera(),
            e,
            p,
        );
        let mut b: Option<(i32, i32, i32, i32)> = None;
        for y in 0..jnr_core::SCREEN_H {
            for x in 0..jnr_core::SCREEN_W {
                if self.fb.get(x, y) != Some(EMPTY) {
                    b = Some(match b {
                        None => (x, y, x + 1, y + 1),
                        Some((l, t, r, bt)) => (l.min(x), t.min(y), r.max(x + 1), bt.max(y + 1)),
                    });
                }
            }
        }
        b
    }

    /// Checks one frame; `what` names it in a failure.
    fn frame(&mut self, g: &Game, what: &str, seen: &mut Seen) {
        if !matches!(g.mode(), Mode::Playing | Mode::StageClear) {
            return;
        }
        let cam = g.camera();
        let p = g.player();
        let (fx, fy) = p.feet();
        let (fx, fy) = (fx.to_px() - cam.x, fy.to_px() - cam.y);
        // As drawn this frame (a transformation flickers between looks).
        let shown = jnr_render::scene::player_shown_power(p, &g.tuning().player);
        let h = if shown.is_big() { 32 } else { 24 };
        let look = (fx - 8, fy - h, fx + 8, fy);
        let (mut label, mut stollen) = (false, false);
        for e in g.entities() {
            let EntityKind::Popup(kind) = e.kind else {
                continue;
            };
            let place = match e.state {
                s if s == PopupPlace::ClearOfResi as u8 => PopupPlace::ClearOfResi,
                s if s == PopupPlace::ClearOfHud as u8 => PopupPlace::ClearOfHud,
                _ => {
                    assert!(
                        !matches!(kind, Popup::Unerhoert | Popup::BitteSehr | Popup::Stollen),
                        "{what} frame {}: {kind:?} untagged",
                        g.frame()
                    );
                    continue;
                }
            };
            match place {
                PopupPlace::ClearOfResi => label = true,
                PopupPlace::ClearOfHud => stollen = true,
            }
            let Some(b) = self.drawn_box(g, e) else {
                continue;
            };
            assert!(
                b.1 >= HUD_CLEAR_Y,
                "{what} frame {}: {kind:?} drawn from screen row {} (HUD up to {})",
                g.frame(),
                b.1,
                HUD_CLEAR_Y - 1
            );
            let covers = b.0 < look.2 && look.0 < b.2 && b.1 < look.3 && look.1 < b.3;
            assert!(
                !covers,
                "{what} frame {}: {kind:?} drawn at {b:?} covers Resi's look {look:?}",
                g.frame()
            );
        }
        let text = g.entities().iter().find(|e| {
            e.state == PopupPlace::ClearOfResi as u8
                && matches!(
                    e.kind,
                    EntityKind::Popup(Popup::Unerhoert | Popup::BitteSehr)
                )
        });
        let now = text.map(|e| (e.x.to_px(), e.y.to_px()));
        if let (Some((x0, y0)), Some((x1, y1))) = (seen.last, now)
            && ((x1 - x0).abs() > DODGE_SPEED_PX || y0 - y1 > DODGE_SPEED_PX)
        {
            seen.jumps += 1;
        }
        seen.last = now;
        seen.labels += u32::from(label);
        seen.stollen += u32::from(stollen);
    }
}

fn load(name: &str) -> (Replay, Game) {
    let text = fs::read_to_string(root().join("tests/replays").join(name)).unwrap();
    let replay = Replay::parse(&text).unwrap();
    let levels = crate::content::all_levels(&root())
        .unwrap()
        .into_iter()
        .map(|(_, l)| l.unwrap())
        .collect::<Vec<_>>();
    let tuning = crate::content::tuning(&root()).unwrap();
    let game = replay.new_game(&levels, &tuning).unwrap();
    (replay, game)
}

/// Every Stage 9 boss and finale replay: the arena fight, the finale and
/// the routes that end there (the QA bug replays included).
fn stage_9_replays() -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(root().join("tests/replays"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.ends_with(".replay")
                && (n.starts_with("cafe_zauner_")
                    || n.starts_with("m7_boss_")
                    || n.starts_with("m7_finale_")
                    || n.starts_with("m7_cafe_")
                    || n.starts_with("flow_m7_")
                    || n.starts_with("qa_m7")
                    || n.contains("_to_cafe_zauner"))
        })
        .collect();
    names.sort();
    names
}

#[test]
fn stage_9_popups_never_cover_resi_or_reach_the_hud() {
    let mut checker = Checker::new();
    let names = stage_9_replays();
    for must in [
        "qa_m7a_boss_label_covers_resi.replay",
        "qa_m7a_stollen_popup_in_hud_big.replay",
        "cafe_zauner_boss_win_normal.replay",
        "cafe_zauner_boss_win_big.replay",
        "cafe_zauner_boss_win_hackl.replay",
        "cafe_zauner_tour.replay",
        "qa_m7_accept_full_route.replay",
    ] {
        assert!(names.iter().any(|n| n == must), "{must} is checked");
    }
    for name in &names {
        let (replay, game) = load(name);
        let mut seen = Seen::default();
        replay.run_game(game, |g| checker.frame(g, name, &mut seen));
        assert_eq!(seen.jumps, 0, "{name}: the labels slide, they never jump");
        // The bug replays show what they were about.
        if name.starts_with("qa_m7a_boss_label") || name.contains("boss_win") {
            assert!(seen.labels > 20, "{name}: labels up {} frames", seen.labels);
        }
        if name.starts_with("qa_m7a_stollen") {
            assert!(
                seen.stollen > 20,
                "{name}: Stollen up {} frames",
                seen.stollen
            );
        }
    }
}

/// A small deterministic generator for the random play (xorshift).
struct Xorshift(u64);

impl Xorshift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// Random play around every hit of the three boss wins: the game is
/// branched 0–24 frames before each hit's labels appear and played on with
/// random held inputs (walk, run, jump, throw, in held runs of 1–12
/// frames) for 90 frames, so she steps, bounces and drifts into the labels
/// from every side; the labels must keep clear of her on every frame.
#[test]
fn random_play_around_the_boss_hits_keeps_the_labels_clear_of_resi() {
    const BRANCHES_BEFORE: [u32; 5] = [0, 4, 9, 16, 24];
    const SEEDS: u64 = 6;
    const FRAMES: u32 = 90;
    let moves = [
        InputFrame::NONE,
        InputFrame::LEFT,
        InputFrame::RIGHT,
        InputFrame::JUMP,
        InputFrame::LEFT.with(InputFrame::JUMP),
        InputFrame::RIGHT.with(InputFrame::JUMP),
        InputFrame::LEFT.with(InputFrame::RUN),
        InputFrame::RIGHT.with(InputFrame::RUN),
        InputFrame::RIGHT
            .with(InputFrame::RUN)
            .with(InputFrame::JUMP),
        InputFrame::LEFT
            .with(InputFrame::RUN)
            .with(InputFrame::JUMP),
        InputFrame::RUN,
    ];
    let mut checker = Checker::new();
    let (mut label_frames, mut jumps, mut runs) = (0, 0, 0);
    for name in [
        "cafe_zauner_boss_win_normal.replay",
        "cafe_zauner_boss_win_big.replay",
        "cafe_zauner_boss_win_hackl.replay",
    ] {
        let (replay, game) = load(name);
        // The frames the labels appear (one per hit).
        let mut hits = Vec::new();
        let mut up = false;
        replay.run_game(game.clone(), |g| {
            let now = g.entities().iter().any(|e| {
                matches!(e.kind, EntityKind::Popup(_)) && e.state == PopupPlace::ClearOfResi as u8
            });
            if now && !up {
                hits.push(g.frame());
            }
            up = now;
        });
        assert_eq!(hits.len(), 3, "{name}: three hits");
        let inputs: Vec<InputFrame> = replay.frames().collect();
        for &hit in &hits {
            for before in BRANCHES_BEFORE {
                let mut g = game.clone();
                for &i in &inputs[..hit.saturating_sub(before) as usize] {
                    g.step(i);
                }
                for seed in 0..SEEDS {
                    let mut rng = Xorshift(0x9E37_79B9_7F4A_7C15 ^ (u64::from(hit) << 8) ^ seed);
                    let mut b = g.clone();
                    let (mut held, mut left) = (InputFrame::NONE, 0);
                    let what = format!("{name} hit {hit} branch -{before} seed {seed}");
                    let mut seen = Seen::default();
                    for _ in 0..FRAMES {
                        if left == 0 {
                            held = moves[(rng.next() % moves.len() as u64) as usize];
                            left = 1 + rng.next() % 12;
                        }
                        left -= 1;
                        b.step(held);
                        checker.frame(&b, &what, &mut seen);
                    }
                    label_frames += seen.labels;
                    jumps += u32::from(seen.jumps > 0);
                    runs += 1;
                }
            }
        }
    }
    assert!(
        label_frames > 500,
        "labels up in {label_frames} random frames"
    );
    // The slide keeps ahead of her almost always (5 of 270 runs jump as
    // built); a jump is the rare case of her dropping onto them from the
    // room side after a Hackl axe hit.
    assert!(
        jumps * 25 <= runs,
        "the labels jumped in {jumps} of {runs} random runs"
    );
}
