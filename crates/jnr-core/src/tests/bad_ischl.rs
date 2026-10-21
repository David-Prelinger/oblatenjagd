//! Stage 8 Bad Ischl as a whole (D-103: the street-level camera top clamp).

use super::tuning;
use crate::camera::{SCREEN_H, SCREEN_W};
use crate::entity::{EnemyKind, EntityKind};
use crate::physics::Aabb;
use crate::{Game, Level};

const SINGER: EntityKind = EntityKind::Enemy(EnemyKind::Operettensaenger);

/// Bad Ischl's `camera_top` (D-103).
const TOP: i32 = 76;
/// The street's top (row 15): at screen y ≤ 164 whenever y ≥ 76.
const STREET: i32 = 240;
/// B2's pier top (rows 7-10 at col 237): the leap's take-off ground.
const PIER: i32 = 112;
/// The 2-high bench at cols 249-250 (row 13), the leap's landing.
const BENCH: (i32, i32) = (249 * 16, 208);
/// Singer #2 stands at col 245.
const SINGER_2_COL: i32 = 245;

macro_rules! replay_text {
    ($name:literal) => {
        include_str!(concat!("../../../../tests/replays/", $name, ".replay"))
    };
}

/// Stage 8 and a Stage 8 replay's text, ready to play.
fn bad_ischl(replay: &str) -> (Game, crate::Replay) {
    let level = Level::parse(include_str!(
        "../../../../content/levels/08_bad_ischl.level"
    ))
    .unwrap();
    let replay = crate::Replay::parse(replay).unwrap();
    let g = replay.new_game(&[level], &tuning()).unwrap();
    (g, replay)
}

/// Plays `text` and checks the clamp on every frame (and on the start
/// framing); returns per frame the camera, her hitbox and whether she
/// stands, plus singer #2's hitbox (once it is in the entity list).
fn play(name: &str, text: &str) -> Vec<(u32, crate::Camera, Aabb, bool, Option<Aabb>)> {
    let (mut g, replay) = bad_ischl(text);
    assert!(
        g.camera().y >= TOP,
        "{name}: start camera y {}",
        g.camera().y
    );
    let mut frames = Vec::new();
    for i in replay.frames() {
        g.step(i);
        let c = g.camera();
        assert!(c.y >= TOP, "{name} frame {}: camera y {}", g.frame(), c.y);
        assert!(STREET - c.y <= 164, "{name} frame {}", g.frame());
        let singer = g
            .entities()
            .iter()
            .find(|e| e.kind == SINGER && e.x.to_px() / 16 == SINGER_2_COL)
            .map(|e| e.hitbox());
        frames.push((
            g.frame(),
            c,
            g.player().hitbox(),
            g.player().on_ground,
            singer,
        ));
    }
    frames
}

#[test]
fn bad_ischl_street_stays_on_screen() {
    // D-103: in Bad Ischl (`camera_top = 76`) the camera's y is never above
    // 76 on any frame of the solver route, the tour and QA's relaxed run
    // (start, checkpoint and goal sequence included; the drill replay adds
    // a respawn), so the street's top is at screen y <= 164. Without the
    // clamp the solver's leap from B2's pier over singer #2 took the camera
    // to ~0 and hid the street.
    play("bad_ischl_tour", replay_text!("bad_ischl_tour"));
    play(
        "qa_m6_offiziere_drill_after_respawn",
        replay_text!("qa_m6_offiziere_drill_after_respawn"),
    );
    play(
        "qa_m6_bad_ischl_relaxed",
        replay_text!("qa_m6_bad_ischl_relaxed"),
    );
    let solver = play("bad_ischl_solver", replay_text!("bad_ischl_solver"));
    // The leap's take-off: the first airborne frame after standing on the
    // pier.
    let take_off = solver
        .windows(2)
        .position(|w| w[0].3 && !w[1].3 && w[0].2.bottom().to_px() == PIER)
        .map(|i| i + 1)
        .expect("the solver leaps from the pier");
    let (frame, c, _, _, singer) = solver[take_off];
    assert_eq!(frame, 1520, "the solver's leap (E42 route)");
    let singer = singer.expect("singer #2 is active at the take-off");
    let on_screen_y = |top: i32, bottom: i32| top >= c.y && bottom <= c.y + SCREEN_H;
    assert!(
        on_screen_y(singer.y.to_px(), singer.bottom().to_px()),
        "singer #2 {singer:?} vs camera {c:?}"
    );
    assert!(
        singer.x.to_px() >= c.x && singer.right().to_px() <= c.x + SCREEN_W,
        "singer #2 horizontally on screen: {singer:?} vs camera {c:?}"
    );
    assert!(
        BENCH.1 - c.y <= 164,
        "bench top at screen {}",
        BENCH.1 - c.y
    );
    // The bench's left edge comes into view within 6 frames of the leap.
    let (_, c6, ..) = solver[take_off + 6];
    assert!(BENCH.0 < c6.x + SCREEN_W, "bench at {} vs {c6:?}", BENCH.0);
}

/// A 40 x 20 side level (320 px tall, bottom clamp 140): a ledge at row 4
/// (cols 0-5) with the start on it, the street at rows 18-19, plus `meta`.
fn ledge_level(meta: &str) -> Level {
    let mut rows = vec![".".repeat(40); 20];
    rows[3] = format!("..P{}", ".".repeat(37));
    rows[4] = format!("{}{}", "#".repeat(6), ".".repeat(34));
    rows[18] = "#".repeat(40);
    rows[19] = "#".repeat(40);
    Level::parse(&format!(
        "[meta]\nid = \"camera_top_test\"\ntime_limit = 300\n{meta}\n[legend]\n\"#\" = \"solid\"\n\"P\" = {{ entity = \"player_start\" }}\n[map]\ngrid = '''\n{}\n'''\n",
        rows.join("\n")
    ))
    .unwrap()
}

#[test]
fn camera_top_clamps_the_start_and_every_frame_and_nothing_else() {
    // D-103 in a small level: high on the ledge the side camera starts at
    // y 0; with `camera_top = 100` at 100. Walking off the ledge, falling
    // to the street and jumping there, the camera's y is the plain one
    // clamped to >= 100 on every frame, and the player moves exactly the
    // same (only the camera's y differs). The key is in the static hash
    // when present: `camera_top = 0` clamps nothing, yet hashes apart.
    let tuning = tuning();
    let mut a = Game::new(&ledge_level(""), &tuning, 1);
    let mut b = Game::new(&ledge_level("camera_top = 100"), &tuning, 1);
    assert_eq!(a.camera().y, 0);
    assert_eq!(b.camera().y, 100);
    let mut clamped = 0;
    for (token, frames) in [
        ("R", 60),
        ("-", 60),
        ("J", 20),
        ("-", 40),
        ("RJ", 30),
        ("-", 60),
    ] {
        let i = super::input(token);
        for _ in 0..frames {
            a.step(i);
            b.step(i);
            let f = a.frame();
            assert_eq!(a.player().hitbox(), b.player().hitbox(), "frame {f}");
            assert_eq!(a.camera().x, b.camera().x, "frame {f}");
            assert_eq!(b.camera().y, a.camera().y.max(100), "frame {f}");
            clamped += usize::from(a.camera().y < 100);
        }
    }
    assert!(clamped > 0, "the plain camera went above 100");
    assert_eq!(a.player().hitbox().bottom().to_px(), 288, "on the street");
    let zero = Game::new(&ledge_level("camera_top = 0"), &tuning, 1);
    let plain = Game::new(&ledge_level(""), &tuning, 1);
    assert_eq!(zero.camera(), plain.camera());
    assert_ne!(zero.state_hash(), plain.state_hash());
}
