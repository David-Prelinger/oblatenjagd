//! Traffic (M6, E36; D-093, D-097; game-design.md → "Stage 7: Strobl"):
//! the Ampel spawner (its counter's conditions, the light cycle, the cars
//! it sends, max alive, the skip inside the player, parse validation), the
//! Auto (2.0 px/frame left through everything, roof bounce without damage
//! or points in every form, side hurt, removal off screen, axe Bounce,
//! walkers ignore it), the Ampel lane lint, `world_hash`, a reset on
//! respawn, and the Reisebus's `look = "postbus"` (parse, nothing but the
//! sprite changes).

use super::*;
use crate::enemies::{AmpelLight, ampel_light, reisebus_lane_problems, state};
use crate::hackl::AxeHit;
use crate::player::Power;
use crate::{BusLook, EnemyKind, Entity, EntityKind, Fx, SpawnKind};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"-" = "oneway"
"a" = { entity = "ampel" }
"b" = { entity = "ampel", cars = 2, phase = 110 }
"1" = { entity = "ampel", cars = 1, period = 300 }
"r" = { entity = "reisebus", stop = 4 }
"y" = { entity = "reisebus", stop = 4, look = "postbus" }
"w" = { entity = "nordic_walker" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const AMPEL: EntityKind = EntityKind::Ampel;
const AUTO: EntityKind = EntityKind::Enemy(EnemyKind::Auto);

/// The level text of a 14-row world `width` columns wide: ground in rows
/// 12–13 (feet at y 192), the player start at column 2 unless `marks`
/// places a `P`, plus `marks` as (row, col, char).
fn world_text(width: usize, marks: &[(usize, usize, char)]) -> String {
    let mut rows: Vec<Vec<char>> = (0..14)
        .map(|r| vec![if r >= 12 { '#' } else { '.' }; width])
        .collect();
    if !marks.iter().any(|m| m.2 == 'P') {
        rows[11][2] = 'P';
    }
    for &(r, c, ch) in marks {
        rows[r][c] = ch;
    }
    let grid: Vec<String> = rows.into_iter().map(|r| r.into_iter().collect()).collect();
    format!(
        "[meta]\nid = \"traffic\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    )
}

fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
    Level::parse(&world_text(width, marks)).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn ampel(g: &Game) -> Entity {
    *g.entities().iter().find(|e| e.kind == AMPEL).unwrap()
}

fn light(g: &Game) -> AmpelLight {
    ampel_light(&ampel(g), g.level(), g.tuning()).unwrap()
}

fn cars(g: &Game) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == AUTO)
        .copied()
        .collect()
}

/// Steps once with the player kept invulnerable.
fn safe_step(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// Puts the player's hitbox centre at `cx` px, standing still.
fn place(g: &mut Game, cx: i32) {
    g.player.x = Fx::from_px(cx) - g.player.w.div_int(2);
    g.player.vx = Fx::ZERO;
}

/// A car (as an Ampel sends it) with its hitbox at (`x`, `y`) px.
fn car_at(x: i32, y: i32) -> Entity {
    let [w, h] = tuning().enemies.auto_hitbox;
    let mut c = Entity::new(
        AUTO,
        Fx::from_px(x),
        Fx::from_px(y),
        Fx::from_px(w),
        Fx::from_px(h),
    );
    c.dir = -1;
    c
}

// ------------------------------------------------------------ parsing

#[test]
fn an_ampel_parses_with_its_defaults_and_rules() {
    let spawn = |field: &str| {
        let text = world_text(30, &[(11, 20, 'x')]).replace(
            "[legend]\n",
            &format!("[legend]\n\"x\" = {{ entity = \"ampel\"{field} }}\n"),
        );
        Level::parse(&text).map(|l| {
            let s = l.entities.iter().find(|s| s.kind == SpawnKind::Ampel);
            let s = s.unwrap();
            (s.period, s.cars, s.phase, s.range)
        })
    };
    assert_eq!(spawn(""), Ok((400, 3, 0, 12)), "defaults (D-093)");
    assert_eq!(
        spawn(", period = 600, cars = 4, phase = 190, range = 16"),
        Ok((600, 4, 190, 16))
    );
    assert_eq!(
        spawn(", period = 300, cars = 1, range = 6"),
        Ok((300, 1, 0, 6))
    );
    // The minimum period: 30 + cars × 40 + 64 + 30 + 120.
    assert_eq!(crate::level::ampel_min_period(3), 364);
    assert_eq!(spawn(", period = 404, cars = 4"), Ok((404, 4, 0, 12)));
    let e = spawn(", period = 403, cars = 4").unwrap_err().to_string();
    assert!(e.contains("needs a period of at least 404 frames"), "{e}");
    for (bad, field) in [
        (", period = 299", "period"),
        (", period = 601", "period"),
        (", cars = 0", "cars"),
        (", cars = 5", "cars"),
        (", range = 5", "range"),
        (", range = 17", "range"),
        (", phase = -1", "phase"),
    ] {
        let e = spawn(bad).unwrap_err().to_string();
        assert!(
            e.contains(&format!("field \"{field}\" must be")),
            "{bad}: {e}"
        );
    }
    // The phase is 0 or after the green (30 + cars × 40), below the period.
    assert_eq!(spawn(", phase = 150"), Ok((400, 3, 150, 12)));
    assert_eq!(spawn(", phase = 399"), Ok((400, 3, 399, 12)));
    assert_eq!(spawn(", cars = 1, phase = 70"), Ok((400, 1, 70, 12)));
    for bad in [", phase = 1", ", phase = 149", ", phase = 400"] {
        let e = spawn(bad).unwrap_err().to_string();
        assert!(e.contains("phase must be 0 or 150..=399"), "{bad}: {e}");
    }
    for (entity, field) in [
        ("{ entity = \"ampel\", stop = 2 }", "stop"),
        ("{ entity = \"ampel\", look = \"postbus\" }", "look"),
        ("{ entity = \"ampel\", len = 3 }", "len"),
        ("{ entity = \"souvenirstand\", cars = 2 }", "cars"),
        ("{ entity = \"fiaker\", look = \"postbus\" }", "look"),
    ] {
        let text =
            world_text(30, &[]).replace("[legend]\n", &format!("[legend]\n\"x\" = {entity}\n"));
        let e = Level::parse(&text).unwrap_err().to_string();
        assert!(
            e.contains(&format!("field \"{field}\" is not allowed")),
            "{entity}: {e}"
        );
    }
}

#[test]
fn the_postbus_look_parses_on_a_reisebus() {
    let look = |field: &str| {
        let text = world_text(30, &[(11, 20, 'x')]).replace(
            "[legend]\n",
            &format!("[legend]\n\"x\" = {{ entity = \"reisebus\"{field} }}\n"),
        );
        Level::parse(&text).map(|l| {
            let s = l.entities.iter().find(|s| s.kind == SpawnKind::Reisebus);
            s.unwrap().look
        })
    };
    assert_eq!(look(""), Ok(BusLook::Reisebus), "the default");
    assert_eq!(look(", look = \"reisebus\""), Ok(BusLook::Reisebus));
    assert_eq!(look(", look = \"postbus\", stop = 4"), Ok(BusLook::Postbus));
    let e = look(", look = \"oebb\"").unwrap_err().to_string();
    assert!(e.contains("unknown look \"oebb\""), "{e}");
}

/// D-097: the look changes nothing but the sprite: a Postbus and a plain
/// bus in the same level run hash-identical through the trigger, the drive,
/// the halt and on.
#[test]
fn a_postbus_behaves_and_hashes_like_a_plain_bus() {
    let mut plain = game(&world(60, &[(11, 14, 'r')]));
    let mut post = game(&world(60, &[(11, 14, 'y')]));
    assert_eq!(post.level().entities.last().unwrap().look, BusLook::Postbus);
    let mut states = Vec::new();
    for f in 0..400 {
        let token = if f < 60 { "R" } else { "-" };
        safe_step(&mut plain, token);
        safe_step(&mut post, token);
        assert_eq!(plain.state_hash(), post.state_hash(), "frame {f}");
        assert_eq!(plain.world_hash(), post.world_hash(), "frame {f}");
        let bus = post
            .entities()
            .iter()
            .find(|e| e.kind == EntityKind::Enemy(EnemyKind::Reisebus));
        states.extend(bus.map(|b| b.state));
    }
    for st in [
        state::TELEGRAPH,
        state::DRIVE,
        state::PHOTO_STOP,
        state::RESTART,
    ] {
        assert!(states.contains(&st), "it went through state {st}");
    }
}

// -------------------------------------------------------------- Ampel

#[test]
fn the_ampel_is_its_cell_and_harmless() {
    let g = game(&world(40, &[(11, 12, 'a')]));
    let a = ampel(&g);
    assert_eq!(
        (a.x, a.y, a.w, a.h),
        (
            Fx::from_px(192),
            Fx::from_px(176),
            Fx::from_px(16),
            Fx::from_px(16)
        )
    );
    assert_eq!(a.state, state::WAITING);
    assert_eq!(light(&g), AmpelLight::Red, "red until it first counts");
    assert_eq!(a.axe_hit(), AxeHit::Pass);
    assert!(a.pose(g.tuning()).is_none());
    // Standing inside its cell does nothing.
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 200);
    let events = hold(&mut g, "-", 10);
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
}

#[test]
fn the_counter_runs_only_left_of_it_in_range_and_fully_on_screen() {
    // Right of its centre (200): it waits.
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 201);
    hold(&mut g, "-", 5);
    assert_eq!(ampel(&g).state, state::WAITING, "right of it");
    place(&mut g, 200);
    hold(&mut g, "-", 3);
    assert_eq!(ampel(&g).state, state::WAITING, "level with its centre");
    // 12 tiles (192 px) is in range, 193 px is not.
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 7);
    hold(&mut g, "-", 3);
    assert_eq!(g.camera().x, 0);
    assert_eq!(ampel(&g).state, state::WAITING, "193 px");
    place(&mut g, 8);
    g.step(input("-"));
    let a = ampel(&g);
    assert_eq!(
        (a.state, a.count),
        (state::COUNTING, 0),
        "starts at its phase"
    );
    assert_eq!(light(&g), AmpelLight::RedAmber);
    // Its cell at x 320..336 is off screen at camera x 0, then fully on
    // screen once the camera reaches 16.
    let mut g = game(&world(60, &[(11, 20, 'a')]));
    place(&mut g, 144);
    hold(&mut g, "-", 3);
    assert_eq!(g.camera().x, 0);
    assert_eq!(ampel(&g).state, state::WAITING, "not fully on screen");
    for cx in 145..=162 {
        place(&mut g, cx);
        let on_screen = g.camera().x >= 16;
        g.step(input("-"));
        assert_eq!(
            ampel(&g).state == state::COUNTING,
            on_screen,
            "camera at {}",
            g.camera().x
        );
    }
    assert_eq!(ampel(&g).state, state::COUNTING);
}

#[test]
fn it_pauses_and_keeps_its_light_while_she_is_out_of_range() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    hold(&mut g, "-", 40);
    let (count, shown) = (ampel(&g).count, light(&g));
    assert_eq!((count, shown), (39, AmpelLight::Green));
    // Right of it: paused.
    place(&mut g, 230);
    for _ in 0..30 {
        safe_step(&mut g, "-");
        assert_eq!((ampel(&g).count, light(&g)), (count, shown));
    }
    // Back in range it goes on from there.
    place(&mut g, 40);
    safe_step(&mut g, "-");
    assert_eq!(ampel(&g).count, count + 1);
}

/// D-093's cycle with 3 cars and period 400: red and amber 0–29, green
/// 30–149 with a car at 30, 70 and 110, four green blinks 150–213 (8 on, 8
/// off), amber 214–243, red 244–399, and again from 0.
#[test]
fn a_cycle_telegraphs_sends_its_cars_blinks_four_times_and_turns_red() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    let mut lights = Vec::new();
    let mut spawns = Vec::new();
    for f in 0..=430u16 {
        let before = cars(&g).len();
        safe_step(&mut g, "-");
        assert_eq!(ampel(&g).count, f % 400);
        lights.push(light(&g));
        if cars(&g).len() > before {
            spawns.push(f);
            let c = *cars(&g).last().unwrap();
            // 2 px into the spawner cell, on the road, already driving.
            assert_eq!((c.x, c.y), (Fx::from_px(194), Fx::from_px(178)));
            assert_eq!((c.w, c.h), (Fx::from_px(28), Fx::from_px(14)));
            assert_eq!((c.base_x, c.base_y), (c.x, c.y));
            assert!(c.facing_left() && c.active);
        }
    }
    assert_eq!(spawns, [30, 70, 110, 430]);
    let looks: Vec<u16> = cars(&g).iter().map(|c| c.count).collect();
    assert!(
        looks.ends_with(&[0]),
        "the look restarts with each green: {looks:?}"
    );
    let expect = |f: usize| match f % 400 {
        0..=29 => AmpelLight::RedAmber,
        30..=149 => AmpelLight::Green,
        150..=213 => AmpelLight::GreenBlink {
            on: (((f % 400) - 150) / 8).is_multiple_of(2),
        },
        214..=243 => AmpelLight::Amber,
        _ => AmpelLight::Red,
    };
    for (f, l) in lights.iter().enumerate() {
        assert_eq!(*l, expect(f), "frame {f}");
    }
    let blinks = lights[150..214]
        .windows(2)
        .filter(|w| w[0] == AmpelLight::GreenBlink { on: false } && w[1] != w[0])
        .count();
    assert_eq!(blinks + 1, 4, "four green blinks");
    assert_eq!(
        lights.iter().filter(|&&l| l == AmpelLight::Red).count(),
        156,
        "156 frames of red with 3 cars"
    );
}

#[test]
fn the_cars_of_a_green_get_their_looks_by_spawn_index() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    for _ in 0..112 {
        safe_step(&mut g, "-");
    }
    let looks: Vec<u16> = cars(&g).iter().map(|c| c.count).collect();
    assert_eq!(looks, [0, 1, 2]);
}

#[test]
fn a_phase_starts_the_first_approach_after_the_green() {
    // `b`: 2 cars, phase 110 (the first blink frame), period 400.
    let mut g = game(&world(40, &[(11, 12, 'b')]));
    place(&mut g, 40);
    safe_step(&mut g, "-");
    assert_eq!(ampel(&g).count, 110);
    assert_eq!(light(&g), AmpelLight::GreenBlink { on: true });
    let mut first = None;
    for n in 1..400 {
        safe_step(&mut g, "-");
        if first.is_none() && !cars(&g).is_empty() {
            first = Some(n);
        }
    }
    // 290 frames to the period's end, then the telegraph: car at f = 30.
    assert_eq!(first, Some(320));
}

#[test]
fn at_most_four_of_its_cars_are_alive() {
    let t = tuning();
    assert_eq!(t.enemies.ampel_max_alive, 4);
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    for _ in 0..31 {
        safe_step(&mut g, "-");
    }
    assert_eq!(cars(&g).len(), 1);
    // Three more of its cars (same spawn point), standing still for the
    // test: 4 alive.
    let first = cars(&g)[0];
    for _ in 0..3 {
        g.entities.push(first);
    }
    let still = |g: &mut Game| {
        for e in g.entities.iter_mut().filter(|e| e.kind == AUTO) {
            e.x = first.x - Fx::from_px(40);
        }
    };
    for _ in 31..71 {
        still(&mut g);
        safe_step(&mut g, "-");
    }
    assert_eq!(ampel(&g).count, 70);
    assert_eq!(cars(&g).len(), 4, "the second car is skipped");
    // A car of another Ampel doesn't count.
    let i = g.entities.iter().position(|e| e.kind == AUTO).unwrap();
    g.entities[i].base_x -= Fx::from_px(16);
    for _ in 71..111 {
        still(&mut g);
        safe_step(&mut g, "-");
    }
    assert_eq!(cars(&g).len(), 5, "3 of its own alive: the third car comes");
}

#[test]
fn a_car_that_would_spawn_inside_her_is_skipped() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    // Her centre 197 is left of the spawner's centre (200), her hitbox
    // (191..203) overlaps the car's spawn box (194..222).
    place(&mut g, 197);
    for _ in 0..31 {
        safe_step(&mut g, "-");
        place(&mut g, 197);
    }
    assert_eq!(ampel(&g).count, 30);
    assert!(cars(&g).is_empty(), "no car appears inside her");
    // One tile further left the next car comes.
    place(&mut g, 176);
    for _ in 31..71 {
        safe_step(&mut g, "-");
    }
    assert_eq!(cars(&g).len(), 1);
}

#[test]
fn ampel_and_cars_reset_with_the_level() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    let events = until(&mut g, "-", 400, |g| g.deaths() > 0);
    assert!(events.contains(&Event::Died), "run over as Normal");
    let events = until(&mut g, "-", 600, |g| g.mode() == crate::Mode::Playing);
    assert!(events.contains(&Event::Respawned), "{events:?}");
    assert!(cars(&g).is_empty(), "the cars are gone");
    let a = ampel(&g);
    assert!(a.count <= 1, "the cycle starts over: {}", a.count);
}

// --------------------------------------------------------------- Auto

#[test]
fn a_car_drives_left_at_2_through_walls_one_ways_and_enemies() {
    let t = tuning();
    assert_eq!(t.enemies.auto_speed, Fx(512), "2.0 px/frame");
    // A wall and a one-way tile in its lane; a Nordic-Walker walking into
    // it.
    let mut g = game(&world(40, &[(11, 8, '#'), (10, 8, '#'), (11, 5, '-')]));
    g.entities.push(car_at(240, 178));
    let mut w = Entity::at_feet(
        EntityKind::Enemy(EnemyKind::NordicWalker),
        Fx::from_px(220),
        Fx::from_px(192),
        t.enemies.nordic_walker_hitbox,
    );
    w.dir = 1;
    w.timer = 5;
    g.entities.push(w);
    place(&mut g, 20);
    let mut x = Fx::from_px(240);
    for _ in 0..90 {
        safe_step(&mut g, "-");
        let c = cars(&g)[0];
        x -= Fx(512);
        assert_eq!((c.x, c.y, c.state), (x, Fx::from_px(178), 0));
        assert_eq!(c.pose(g.tuning()), Some(crate::Pose::Drive));
    }
    let walker = g
        .entities()
        .iter()
        .find(|e| e.kind == EntityKind::Enemy(EnemyKind::NordicWalker))
        .unwrap();
    assert_eq!(walker.dir, 1, "walkers ignore it");
    assert!(!EnemyKind::Auto.sinks_in_swim_water());
}

#[test]
fn a_car_is_removed_once_its_right_edge_is_left_of_the_screen() {
    let mut g = game(&world(40, &[]));
    g.entities.push(car_at(-24, 178));
    safe_step(&mut g, "-");
    assert_eq!(g.camera().x, 0);
    assert_eq!(cars(&g)[0].right(), Fx::from_px(2), "still 2 px on screen");
    safe_step(&mut g, "-");
    assert!(cars(&g).is_empty(), "gone");
}

#[test]
fn its_roof_bounces_without_damage_or_points_in_every_form() {
    let t = tuning();
    for power in [Power::Normal, Power::Big, Power::Hackl] {
        let mut g = game(&world(40, &[]));
        g.player.set_power(power, &t.player);
        g.player.transform = 0;
        g.entities.push(car_at(150, 178));
        let c = cars(&g)[0];
        g.player.x = c.x + c.w.div_int(2) - g.player.w.div_int(2);
        g.player.y = c.y - Fx::from_px(2) - g.player.h;
        g.player.vx = Fx::ZERO;
        g.player.vy = Fx::from_px(3);
        g.player.on_ground = false;
        let events = g.step(input("-")).to_vec();
        assert!(events.contains(&Event::Stomped), "{power:?}: {events:?}");
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert_eq!(g.score(), 0, "{power:?}: no points");
        assert!(g.player().vy < Fx::ZERO, "{power:?}: bounced up");
        assert_eq!(
            g.player().y + g.player().h,
            cars(&g)[0].y,
            "feet on its roof"
        );
        assert_eq!(cars(&g)[0].kind, AUTO, "not defeated");
        assert_eq!(g.player().power, power);
        for _ in 0..20 {
            let events = g.step(input("-")).to_vec();
            assert!(!events.contains(&Event::Hurt), "{power:?}: {events:?}");
        }
    }
}

/// Across two roofs of one green (80+ px apart): a held bounce off the
/// first lands on the second; no damage, no points.
#[test]
fn she_bounces_across_two_car_roofs() {
    let mut g = game(&world(40, &[]));
    g.entities.push(car_at(150, 178));
    g.entities.push(car_at(240, 178));
    let c = cars(&g)[0];
    g.player.x = c.x + c.w.div_int(2) - g.player.w.div_int(2);
    g.player.y = c.y - Fx::from_px(2) - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::from_px(3);
    g.player.on_ground = false;
    let events = hold(&mut g, "J", 70);
    let stomps = events.iter().filter(|e| **e == Event::Stomped).count();
    assert_eq!(stomps, 2, "{events:?}");
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    assert_eq!(g.score(), 0);
}

#[test]
fn a_side_hit_hurts() {
    let t = tuning();
    let mut g = game(&world(40, &[]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    g.entities.push(car_at(150, 178));
    // Standing on the ground, the car's front (left) 3 px into her.
    g.player.x = Fx::from_px(150 + 3) - g.player.w;
    g.player.vx = Fx::ZERO;
    let events = g.step(input("-")).to_vec();
    assert!(events.contains(&Event::Hurt), "{events:?}");
    assert_eq!(g.player().power, Power::Normal);
    assert_eq!(g.score(), 0);
    assert!(!EnemyKind::Auto.stompable() && !EnemyKind::Auto.hurts_from_above());
}

#[test]
fn the_hackl_bounces_off_a_car() {
    let t = tuning();
    let mut c = car_at(100, 178);
    assert_eq!(c.axe_hit(), AxeHit::Bounce);
    let before = c;
    assert_eq!(crate::enemies::hit_by_projectile(&mut c, &t), None);
    assert_eq!(c, before, "nothing happens to it");
    // In play: an axe thrown at a car 3 tiles ahead bounces off it.
    let mut g = game(&world(40, &[]));
    g.player.set_power(Power::Hackl, &t.player);
    g.player.transform = 0;
    g.entities.push(car_at(220, 178));
    let mut events = safe_step(&mut g, "B");
    for _ in 0..60 {
        events.extend(safe_step(&mut g, "-"));
    }
    assert!(events.contains(&Event::HacklThrown), "{events:?}");
    assert!(events.contains(&Event::AxeHit), "{events:?}");
    assert!(
        !g.entities()
            .iter()
            .any(|e| e.kind == EntityKind::ThrownHackl),
        "bounced off"
    );
    assert_eq!(cars(&g).len(), 1);
    assert_eq!(g.score(), 0);
}

// ------------------------------------------------------- hashes, lints

#[test]
fn world_hash_covers_the_ampel_counter_and_the_cars() {
    let mut g = game(&world(40, &[(11, 12, 'a')]));
    place(&mut g, 40);
    for _ in 0..35 {
        safe_step(&mut g, "-");
    }
    let base = g.world_hash();
    let i = g.entities.iter().position(|e| e.kind == AMPEL).unwrap();
    let mut other = g.clone();
    other.entities[i].count += 1;
    assert_ne!(other.world_hash(), base, "the counter");
    let mut other = g.clone();
    other.entities[i].state = state::WAITING;
    assert_ne!(other.world_hash(), base, "the counter's state");
    let mut other = g.clone();
    let c = other.entities.iter().position(|e| e.kind == AUTO).unwrap();
    other.entities[c].x -= Fx::from_px(8);
    assert_ne!(other.world_hash(), base, "a car");
    let mut other = g.clone();
    other.entities.retain(|e| e.kind != AUTO);
    assert_ne!(other.world_hash(), base, "the cars alive");
}

#[test]
fn the_lane_lint_covers_an_ampels_cars() {
    let t = tuning();
    let lint = |marks: &[(usize, usize, char)]| reisebus_lane_problems(&world(60, marks), &t);
    // Ampel at column 30: its cars' lane is row 11, columns 6–31.
    assert!(lint(&[(11, 30, 'a')]).is_empty());
    // Outside it: 25 tiles left, a row above, beyond the poke cell.
    assert!(lint(&[(11, 30, 'a'), (11, 5, '#'), (10, 20, '-'), (11, 32, '#')]).is_empty());
    // Closer to the left edge than 24 tiles: up to the edge.
    assert!(lint(&[(11, 10, 'a')]).is_empty());
    let p = lint(&[(11, 30, 'a'), (11, 6, '-'), (11, 12, '#')]);
    assert_eq!(p.len(), 1, "one problem per Ampel: {p:?}");
    assert!(
        p[0].starts_with(
            "ampel at row 12, column 31: its lane (rows 12-12, columns 7-32) has a solid or one-way tile at row 12, column 7 (D-093"
        ),
        "{p:?}"
    );
    // The cell its cars poke into counts.
    let p = lint(&[(11, 30, 'a'), (11, 31, '#')]);
    assert!(p[0].contains("row 12, column 32"), "{p:?}");
}

#[test]
fn a_one_car_ampel_with_the_shortest_period_has_two_seconds_of_red() {
    // `1`: 1 car, period 300 (minimum 284).
    let mut g = game(&world(40, &[(11, 12, '1')]));
    place(&mut g, 40);
    let mut red = 0;
    for _ in 0..300 {
        safe_step(&mut g, "-");
        red += usize::from(light(&g) == AmpelLight::Red);
    }
    assert_eq!(red, 300 - 30 - 40 - 64 - 30);
    assert!(red >= 120);
}
