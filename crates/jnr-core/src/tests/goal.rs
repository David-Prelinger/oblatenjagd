//! The Maibaum goal sequence (M1 task E4): height zones, slide, hop, walk
//! off, time-bonus tally, texts, „Fortsetzung folgt …", title.

use super::*;
use crate::game::maibaum_points;
use crate::{ClearPhase, EntityKind, Fx, Mode, Popup, Start};

/// Flat 30-column world (ground rows 12–13), the start at column 2 and a
/// goal at column 10 standing on the ground (base y = 192 px).
fn goal_world() -> Level {
    let air = ".".repeat(30);
    let start = format!("..P.......G{}", ".".repeat(19));
    let ground = "#".repeat(30);
    let mut rows: Vec<&str> = vec![&air; 11];
    rows.push(&start);
    rows.push(&ground);
    rows.push(&ground);
    level(&rows)
}

const BASE_Y: i32 = 12 * 16;
const POLE_X: i32 = 10 * 16 + 8;

/// Steps until the mode is `StageClear`; panics after `max` frames.
fn walk_into_goal(g: &mut Game, token: &str, max: u32) {
    for _ in 0..max {
        g.step(input(token));
        if g.mode() == Mode::StageClear {
            return;
        }
    }
    panic!("never touched the goal");
}

fn phase(g: &Game) -> ClearPhase {
    g.stage_clear().expect("in the goal sequence").phase
}

/// Steps while the phase stays `p`; returns how many steps it took to leave it.
fn frames_in(g: &mut Game, p: ClearPhase) -> u32 {
    let mut n = 0;
    while g.mode() == Mode::StageClear && phase(g) == p {
        g.step(input("-"));
        n += 1;
        assert!(n < 2000, "stuck in {p:?}");
    }
    n
}

#[test]
fn height_zones_up_to_the_wurstkranz_at_seven_tiles() {
    // 2/4/6 tiles, then the Wurstkranz at 7 tiles (D-038).
    let t = tuning().goal;
    let cases = [
        (0, 100, false),
        (31, 100, false),
        (32, 400, false),
        (63, 400, false),
        (64, 800, false),
        (95, 800, false),
        (96, 2000, false),
        (111, 2000, false),
        (112, 5000, true),
        (127, 5000, true),
        (500, 5000, true),
    ];
    for (h, points, wurst) in cases {
        assert_eq!(maibaum_points(h, &t), (points, wurst), "height {h}");
    }
}

#[test]
fn touching_on_the_ground_scores_the_lowest_zone() {
    let mut g = Game::new(&goal_world(), &tuning(), 1);
    walk_into_goal(&mut g, "R", 300);
    let c = *g.stage_clear().unwrap();
    assert_eq!(c.height_px, 0);
    assert_eq!(c.height_points, 100);
    assert_eq!(g.score(), 100);
    assert_eq!((c.pole_x, c.base_y), (POLE_X, BASE_Y));
    assert!(g.events().contains(&Event::LevelComplete));
    assert!(
        g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::Popup(Popup::Score(100)))
    );
    // Placed just left of the pole.
    let p = g.player();
    assert_eq!(
        (p.x + p.w).to_px(),
        POLE_X - tuning().goal.pole_offset,
        "hitbox right edge"
    );
}

#[test]
fn touching_high_up_gives_the_wurstkranz_popup() {
    let t = tuning();
    let mut g = Game::new(&goal_world(), &t, 1);
    // Put the player 9 tiles above the ground just left of the pole, moving right.
    g.player.x = Fx::from_px(POLE_X - 8 - 12);
    g.player.y = Fx::from_px(BASE_Y - 9 * 16) - g.player.h;
    g.player.vx = t.player.walk_speed;
    g.player.on_ground = false;
    walk_into_goal(&mut g, "R", 20);
    let c = *g.stage_clear().unwrap();
    assert!(c.height_px >= 128, "{}", c.height_px);
    assert_eq!(g.score(), 5000);
    let popups: Vec<EntityKind> = g
        .entities()
        .iter()
        .map(|e| e.kind)
        .filter(|k| matches!(k, EntityKind::Popup(_)))
        .collect();
    assert_eq!(
        popups,
        vec![
            EntityKind::Popup(Popup::Score(5000)),
            EntityKind::Popup(Popup::Wurstkranz)
        ]
    );
}

#[test]
fn sequence_timing_slide_hop_walk_tally_texts_title() {
    let t = tuning();
    let mut g = Game::new(&goal_world(), &t, 1);
    // A jump right before the pole: touched in the air.
    while (g.player().x + g.player().w).to_px() < POLE_X - 48 {
        g.step(input("R"));
    }
    walk_into_goal(&mut g, "RJ", 200);
    let c = *g.stage_clear().unwrap();
    assert!(c.height_px > 0, "touched in the air");
    let feet = g.player().y + g.player().h;
    let time = g.time_left();
    let score = g.score();
    let enemies_frozen_hash = g.world_hash();

    // Slide: 2 px per frame down to the base.
    let to_slide = Fx::from_px(BASE_Y) - feet;
    let expected = (to_slide.raw() + t.goal.slide_speed.raw() - 1) / t.goal.slide_speed.raw();
    let mut n = 0;
    let mut last = feet;
    while phase(&g) == ClearPhase::Slide {
        g.step(input("RJ"));
        n += 1;
        let now = g.player().y + g.player().h;
        assert!(now - last <= t.goal.slide_speed, "at most 2 px per frame");
        last = now;
    }
    assert_eq!(n, expected.max(1), "slide frames");
    assert_eq!(g.time_left(), time, "timer stopped");
    // Hop: lands on the base right of the pole.
    assert!(g.player().x.to_px() >= POLE_X + t.goal.pole_offset);
    frames_in(&mut g, ClearPhase::Hop);
    assert_eq!(phase(&g), ClearPhase::Walk);
    assert_eq!((g.player().y + g.player().h).to_px(), BASE_Y);
    assert!(g.player().on_ground);
    // Walk: at walk speed until off screen; the camera stays.
    let cam = g.camera();
    let x0 = g.player().x;
    g.step(input("-"));
    assert_eq!(g.player().x - x0, t.player.walk_speed);
    frames_in(&mut g, ClearPhase::Walk);
    assert_eq!(g.camera(), cam);
    assert!(g.player().x.to_px() >= cam.x + 320);
    assert_eq!(g.time_left(), time, "no tally before she is gone");
    assert_eq!(g.world_hash(), enemies_frozen_hash);
    // Tally: one second per frame, 50 points each.
    assert_eq!(phase(&g), ClearPhase::Tally);
    assert_eq!(frames_in(&mut g, ClearPhase::Tally), u32::from(time));
    assert_eq!(g.time_left(), 0);
    assert_eq!(g.score(), score + 50 * u32::from(time));
    assert_eq!(g.stage_clear().unwrap().bonus, 50 * u32::from(time));
    // Texts, then „Fortsetzung folgt …", then the title screen.
    assert_eq!(
        frames_in(&mut g, ClearPhase::Clear),
        u32::from(t.goal.clear_frames)
    );
    assert_eq!(
        frames_in(&mut g, ClearPhase::ToBeContinued),
        u32::from(t.goal.to_be_continued_frames)
    );
    assert_eq!(
        g.mode(),
        Mode::Title {
            frames: 0,
            from_ending: false
        }
    );
    assert!(g.stage_clear().is_none());
    assert_eq!(*g.player(), *Game::new(&goal_world(), &t, 1).player());
    // START from the title starts the stage again (direct flow: at once).
    g.step(input("S"));
    assert_eq!(g.mode(), Mode::Playing);
    assert_eq!(g.score(), 0);
}

#[test]
fn popups_float_but_enemies_freeze_during_the_sequence() {
    let mut g = Game::new(&goal_world(), &tuning(), 1);
    walk_into_goal(&mut g, "R", 300);
    let popup = |g: &Game| {
        g.entities()
            .iter()
            .find(|e| matches!(e.kind, EntityKind::Popup(_)))
            .map(|e| e.timer)
    };
    assert_eq!(popup(&g), Some(0));
    g.step(input("-"));
    assert_eq!(popup(&g), Some(1));
    hold(&mut g, "-", 40);
    assert_eq!(popup(&g), None, "gone after its 30 frames");
}

#[test]
fn title_flow_ends_on_the_title_screen_too() {
    let t = tuning();
    let mut g = Game::new_at(&goal_world(), &t, 1, Start::Title);
    g.step(input("S"));
    hold(&mut g, "-", u32::from(t.game.intro_frames));
    assert_eq!(g.mode(), Mode::Playing);
    walk_into_goal(&mut g, "R", 300);
    for _ in 0..5000 {
        g.step(input("-"));
        if matches!(g.mode(), Mode::Title { .. }) {
            return;
        }
    }
    panic!("never back on the title screen");
}

/// One frame of [`moving_frames`]: the phase after the step, the camera and
/// the player hitbox in screen px.
struct CamFrame {
    phase: ClearPhase,
    camera: crate::Camera,
    left: i32,
    top: i32,
    bottom: i32,
}

/// Steps through slide, hop and walk, recording every frame.
fn moving_frames(g: &mut Game) -> Vec<CamFrame> {
    let mut frames = Vec::new();
    while matches!(
        phase(g),
        ClearPhase::Slide | ClearPhase::Hop | ClearPhase::Walk
    ) {
        g.step(input("-"));
        let c = g.camera();
        let p = g.player();
        let top = p.y.to_px() - c.y;
        frames.push(CamFrame {
            phase: phase(g),
            camera: c,
            left: p.x.to_px() - c.x,
            top,
            bottom: top + p.h.to_px(),
        });
        assert!(frames.len() < 2000, "stuck");
    }
    frames
}

#[test]
fn low_touch_keeps_the_camera_still() {
    let mut g = Game::new(&goal_world(), &tuning(), 1);
    walk_into_goal(&mut g, "R", 300);
    let at_touch = g.camera();
    for f in moving_frames(&mut g) {
        assert_eq!(f.camera, at_touch, "the camera does not move (D-040)");
    }
}

#[test]
fn high_touch_camera_follows_the_slide_down_to_the_ground_framing() {
    // The ground framing: the play camera after walking into the pole.
    let mut low = Game::new(&goal_world(), &tuning(), 1);
    walk_into_goal(&mut low, "R", 300);
    let ground = low.camera();

    let t = tuning();
    let mut g = Game::new(&goal_world(), &t, 1);
    // 9 tiles up just left of the pole, moving right, the camera at the
    // top as after a high jump.
    g.player.x = Fx::from_px(POLE_X - 8 - 12);
    g.player.y = Fx::from_px(BASE_Y - 9 * 16) - g.player.h;
    g.player.vx = t.player.walk_speed;
    g.player.on_ground = false;
    g.camera.y = 0;
    walk_into_goal(&mut g, "R", 20);
    assert!(
        g.stage_clear().unwrap().height_px >= 112,
        "a Wurstkranz touch"
    );
    let at_touch = g.camera();
    assert!(at_touch.y < ground.y, "starts above the ground framing");

    let frames = moving_frames(&mut g);
    let mut prev_y = at_touch.y;
    for (i, f) in frames.iter().enumerate() {
        assert_eq!(f.camera.x, at_touch.x, "x stays (frame {i})");
        assert!(f.camera.y >= prev_y, "only follows down (frame {i})");
        prev_y = f.camera.y;
        assert!(
            f.top >= 0 && f.bottom <= crate::SCREEN_H,
            "on screen vertically, frame {i}: {}..{}",
            f.top,
            f.bottom
        );
    }
    let slide_end = frames
        .iter()
        .position(|f| f.phase != ClearPhase::Slide)
        .expect("leaves the slide");
    assert_eq!(
        frames[slide_end].camera.y, ground.y,
        "the slide ends at the ground framing"
    );
    // Hop and walk on that framing, on screen until she leaves to the right.
    for f in &frames[slide_end..] {
        assert_eq!(f.camera.y, ground.y);
    }
    let walk_on_screen = frames[slide_end..]
        .iter()
        .filter(|f| f.phase == ClearPhase::Walk && f.left < crate::SCREEN_W)
        .count();
    assert!(walk_on_screen > 0);
    assert!(
        frames.last().unwrap().left >= crate::SCREEN_W,
        "left to the right"
    );
}
