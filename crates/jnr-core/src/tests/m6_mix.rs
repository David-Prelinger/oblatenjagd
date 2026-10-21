//! M6 integration (E41): the Stage 7 and Stage 8 kinds together (D-093 …
//! D-097): a shutter over a car lane, a Kurgast's chair in an Ampel lane,
//! an officer file meeting a lying Kurgast, the Postbus, a singer next to
//! a file, a note over a chair, officers and shutters on the one stage
//! clock after a respawn, the thrown Hackl against every new kind, and
//! `world_hash` over every new state (the solver's de-duplication).

use super::*;
use crate::enemies::{is_platform_now, lie_down, sabre_zone, state};
use crate::hackl::{AxeHit, axe};
use crate::platform::shutter;
use crate::player::Power;
use crate::{EnemyKind, Entity, EntityKind, Fx, Mode};

/// Legend of the test worlds (`.` air, `P` start).
const LEGEND: &str = r##"
"#" = "solid"
"a" = { entity = "ampel" }
"f" = { entity = "fensterladen", period = 480, open = 450 }
"c" = { entity = "fensterladen" }
"s" = { entity = "fensterladen", phase = 60 }
"k" = { entity = "kurgast" }
"o" = { entity = "offizier" }
"q" = { entity = "offizier", phase = 30 }
"y" = { entity = "reisebus", look = "postbus" }
"n" = { entity = "operettensaenger" }
"M" = { tile = "oblaten_box", contains = "mozartkugel" }
"P" = { entity = "player_start" }
"G" = { entity = "goal" }
"##;

const AMPEL: EntityKind = EntityKind::Ampel;
const AUTO: EntityKind = EntityKind::Enemy(EnemyKind::Auto);
const KURGAST: EntityKind = EntityKind::Enemy(EnemyKind::Kurgast);
const OFFIZIER: EntityKind = EntityKind::Enemy(EnemyKind::Offizier);
const BUS: EntityKind = EntityKind::Enemy(EnemyKind::Reisebus);
const SINGER: EntityKind = EntityKind::Enemy(EnemyKind::Operettensaenger);

/// The street's top (row 12), px: a car's lane is y 178..192.
const FLOOR: i32 = 192;

/// A 14-row world `width` columns wide: ground in rows 12–13 (top y 192),
/// the start at column 2 (row 11) unless `marks` has a `P`, plus `marks`
/// as (row, col, char) in [`LEGEND`] (`.` in rows 12–13 digs a hole).
fn world(width: usize, marks: &[(usize, usize, char)]) -> Level {
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
    let text = format!(
        "[meta]\nid = \"m6_mix_test\"\ntime_limit = 300\n[legend]\n{LEGEND}\n[map]\ngrid = '''\n{}\n'''\n",
        grid.join("\n")
    );
    Level::parse(&text).unwrap()
}

fn game(level: &Level) -> Game {
    Game::new(level, &tuning(), 1)
}

fn all(g: &Game, kind: EntityKind) -> Vec<Entity> {
    g.entities()
        .iter()
        .filter(|e| e.kind == kind && !e.removed)
        .copied()
        .collect()
}

fn first(g: &Game, kind: EntityKind) -> Entity {
    all(g, kind)
        .first()
        .copied()
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

fn index(g: &Game, kind: EntityKind) -> usize {
    g.entities().iter().position(|e| e.kind == kind).unwrap()
}

/// Steps once with `token` and the player kept invulnerable.
fn safe(g: &mut Game, token: &str) -> Vec<Event> {
    g.player.invulnerable = u8::MAX;
    g.step(input(token)).to_vec()
}

/// Puts the player standing on `e`'s top with her centre on its centre.
fn stand_on(g: &mut Game, e: Entity) {
    g.player.x = e.x + e.w.div_int(2) - g.player.w.div_int(2);
    g.player.y = e.y - g.player.h;
    g.player.vx = Fx::ZERO;
    g.player.vy = Fx::ZERO;
    g.player.on_ground = true;
}

fn feet(g: &Game) -> Fx {
    g.player().y + g.player().h
}

/// A car (as an Ampel sends it) with its hitbox at (`x`, `y`) px.
fn car_at(x: Fx, y: i32) -> Entity {
    let [w, h] = tuning().enemies.auto_hitbox;
    let mut c = Entity::new(AUTO, x, Fx::from_px(y), Fx::from_px(w), Fx::from_px(h));
    c.dir = -1;
    c
}

/// Lays the first Kurgast down in his deck chair at once.
fn lay_down(g: &mut Game) -> Entity {
    let i = index(g, KURGAST);
    lie_down(&mut g.entities[i], &tuning());
    g.entities[i]
}

// ------------------------------------------------- a shutter over a lane

/// A shutter in the lane's own row (body y 176..184, the car 178..192):
/// cars drive through the open body (they ignore platforms) and beneath a
/// rider standing on it (her feet at 176, the roof at 178), unhurt.
#[test]
fn cars_drive_through_a_shutter_in_their_lane_beneath_its_rider() {
    let mut g = game(&world(40, &[(11, 14, 'a'), (11, 8, 'f')]));
    hold(&mut g, "-", 1);
    let s = first(&g, EntityKind::Fensterladen);
    stand_on(&mut g, s);
    let at = (g.player().x, feet(&g));
    let mut crossed = 0;
    let mut last: Vec<Fx> = Vec::new();
    for _ in 0..300 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert_eq!((g.player().x, feet(&g)), at, "she stays on the shutter");
        assert!(g.player().on_ground);
        let sh = first(&g, EntityKind::Fensterladen);
        assert_eq!((sh.x, sh.y, sh.state), (s.x, s.y, shutter::OPEN));
        let xs: Vec<Fx> = all(&g, AUTO).iter().map(|c| c.x).collect();
        // Every car drives on at 2.0 px/frame through the shutter (or was
        // just sent, 2 px into the spawner's cell).
        for x in &xs {
            let sent = *x == Fx::from_px(14 * 16 + tuning().enemies.auto_spawn_dx);
            assert!(sent || last.contains(&(*x + tuning().enemies.auto_speed)));
        }
        crossed += all(&g, AUTO)
            .iter()
            .filter(|c| c.x <= s.x && c.x + tuning().enemies.auto_speed > s.x)
            .count();
        last = xs;
    }
    assert_eq!(crossed, 3, "the green's three cars crossed under her");
    assert_eq!(g.score(), 0);
}

/// A shutter two tiles over the lane closes under her while a car drives
/// beneath: she drops onto its roof and bounces without damage or points.
#[test]
fn a_closing_shutter_over_a_lane_drops_her_onto_a_car_roof_for_a_bounce() {
    let t = tuning();
    for power in [Power::Normal, Power::Big] {
        // `c`: period 240, open 150; the body's top at y 144.
        let mut g = game(&world(40, &[(9, 8, 'c')]));
        g.player.set_power(power, &t.player);
        g.player.transform = 0;
        hold(&mut g, "-", 140);
        let s = first(&g, EntityKind::Fensterladen);
        stand_on(&mut g, s);
        hold(&mut g, "-", 10);
        assert_eq!(first(&g, EntityKind::Fensterladen).state, shutter::CLOSED);
        // Without a car she falls n frames until her feet reach the roof.
        let mut probe = g.clone();
        let mut n = 0;
        while feet(&probe) < Fx::from_px(FLOOR - 14) {
            hold(&mut probe, "-", 1);
            n += 1;
        }
        // A car whose centre is under hers when she gets there.
        let cx = g.player().x + g.player().w.div_int(2);
        let half = Fx::from_px(t.enemies.auto_hitbox[0]).div_int(2);
        g.entities.push(car_at(
            cx - half + t.enemies.auto_speed.mul_int(n),
            FLOOR - 14,
        ));
        let events = hold(&mut g, "-", n as u32 + 1);
        assert!(events.contains(&Event::Stomped), "{power:?}: {events:?}");
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert!(g.player().vy < Fx::ZERO, "{power:?}: bounced up");
        assert_eq!((g.player().power, g.score()), (power, 0));
        assert_eq!(all(&g, AUTO).len(), 1, "the car drives on");
    }
}

/// A Kurgast-chair-jumper: from his chair a held jump goes up through an
/// open shutter 3 tiles above and lands on it; when it closes she drops
/// back onto the chair (a one-way top), unhurt, and his rest pauses again
/// while she stands on him.
#[test]
fn a_closing_shutter_drops_a_chair_jumper_back_onto_the_chair() {
    // `c` (period 240, open 150) at (8, 8): body x 128..160, top y 128;
    // the Kurgast at column 8: chair x 128..144, top y 176.
    let mut g = game(&world(40, &[(11, 8, 'k'), (8, 8, 'c')]));
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    stand_on(&mut g, chair);
    g.player.invulnerable = 0;
    hold(&mut g, "-", 10);
    assert_eq!(first(&g, KURGAST).count, 0, "paused under her");
    let events = hold(&mut g, "J", 20);
    assert!(events.contains(&Event::Jumped));
    until(&mut g, "-", 60, |g| g.player().on_ground);
    let s = first(&g, EntityKind::Fensterladen);
    assert_eq!(feet(&g), s.y, "on the shutter");
    assert!(g.stage_clock() < 150);
    // His rest runs while she is up there; the shutter closes at 150.
    let events = until(&mut g, "-", 200, |g| {
        g.player().on_ground && feet(g) == first(g, KURGAST).y
    });
    assert!(g.stage_clock() > 150);
    assert!(events.contains(&Event::Landed), "back on the chair");
    assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
    let k = first(&g, KURGAST);
    assert_eq!((k.x, k.state), (chair.x, state::LIE));
    assert!(k.count > 0, "he rested while she was on the shutter");
    let events = hold(&mut g, "-", 30);
    assert!(!events.contains(&Event::Hurt));
    assert_eq!(first(&g, KURGAST).count, k.count, "paused again");
}

// --------------------------------------------- a deck chair in a lane

/// A Kurgast's chair in an Ampel lane: the cars pass through it (they
/// ignore enemies), he stays in it, and a Resi standing on it rides out
/// the whole green above the roofs (feet 176, roof 178), unhurt, his rest
/// paused under her.
#[test]
fn cars_pass_a_kurgasts_chair_in_their_lane_and_she_rides_out_the_green_on_it() {
    let mut g = game(&world(40, &[(11, 14, 'a'), (11, 8, 'k')]));
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    assert_eq!(chair.y, Fx::from_px(FLOOR - 16));
    stand_on(&mut g, chair);
    g.player.invulnerable = 0;
    let mut crossed = 0;
    for _ in 0..300 {
        let events = g.step(input("-")).to_vec();
        assert!(!events.contains(&Event::Hurt) && !events.contains(&Event::Died));
        assert_eq!(feet(&g), chair.y, "on the chair");
        let k = first(&g, KURGAST);
        assert_eq!(
            (k.x, k.y, k.state, k.count),
            (chair.x, chair.y, state::LIE, 0)
        );
        crossed += all(&g, AUTO)
            .iter()
            .filter(|c| c.hitbox().overlaps(&k.hitbox()))
            .count();
    }
    assert!(crossed > 0, "the cars drove through the chair");
    assert_eq!(g.score(), 0);
}

/// A walking Kurgast ignores the cars that drive through him: he neither
/// turns nor is hurt (cars are never „on foot").
#[test]
fn a_walking_kurgast_ignores_the_cars() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 12, 'k')]));
    safe(&mut g, "-");
    let k = first(&g, KURGAST);
    assert_eq!(k.dir, -1, "towards her");
    g.entities.push(car_at(k.x + Fx::from_px(40), FLOOR - 14));
    let mut met = false;
    for _ in 0..60 {
        safe(&mut g, "-");
        let k = first(&g, KURGAST);
        assert_eq!((k.dir, k.state), (-1, state::WALK));
        met |= all(&g, AUTO)
            .first()
            .is_some_and(|c| c.hitbox().overlaps(&k.hitbox()));
    }
    assert!(met, "the car drove through him");
    assert_eq!(
        first(&g, KURGAST).w,
        Fx::from_px(t.enemies.kurgast_hitbox[0])
    );
}

/// A note (the Kurkapelle's) flies on through a deck chair; the chair
/// stays.
#[test]
fn a_note_passes_a_kurgasts_chair() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 10, 'k')]));
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    let [w, h] = t.enemies.note_hitbox;
    let mut n = Entity::at_feet(
        EntityKind::Note,
        chair.x + chair.w + Fx::from_px(8),
        chair.y + Fx::from_px(8 + h / 2),
        [w, h],
    );
    n.vx = -t.enemies.note_speed;
    n.dir = -1;
    n.base_x = n.x;
    g.entities.push(n);
    let mut inside = false;
    for _ in 0..40 {
        safe(&mut g, "-");
        let k = first(&g, KURGAST);
        let note = first(&g, EntityKind::Note);
        inside |= note.hitbox().overlaps(&k.hitbox());
        assert!(is_platform_now(&k));
        assert_eq!((k.x, k.state), (chair.x, state::LIE));
    }
    assert!(inside, "the note flew through the chair");
    assert!(first(&g, EntityKind::Note).right() < chair.x, "and on");
}

// ------------------------------------------------ officers and others

/// An officer file marching at a lying Kurgast turns at his chair (it is
/// „on foot"), the rear man turns at the front man, and the file stays in
/// step; the chair stays.
#[test]
fn an_officer_file_turns_at_a_lying_kurgast() {
    let mut g = game(&world(40, &[(11, 12, 'k'), (11, 16, 'o'), (11, 17, 'o')]));
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    assert!(all(&g, OFFIZIER).iter().all(|o| o.active && o.dir == -1));
    let mut turned = [false, false];
    for _ in 0..280 {
        safe(&mut g, "-");
        let file = all(&g, OFFIZIER);
        assert_eq!(file.len(), 2);
        assert_eq!(file[0].count, file[1].count, "in step");
        for (i, o) in file.iter().enumerate() {
            assert!(o.x > chair.x, "never walks through the chair");
            turned[i] |= o.dir == 1;
        }
        let k = first(&g, KURGAST);
        assert_eq!((k.x, k.state), (chair.x, state::LIE));
    }
    assert_eq!(turned, [true, true]);
}

/// A walking Kurgast and an officer turn at each other.
#[test]
fn a_walking_kurgast_and_an_officer_turn_at_each_other() {
    // Both walk towards her at column 12 from either side and meet.
    let mut g = game(&world(40, &[(11, 9, 'k'), (11, 12, 'P'), (11, 15, 'o')]));
    safe(&mut g, "-");
    assert_eq!((first(&g, KURGAST).dir, first(&g, OFFIZIER).dir), (1, -1));
    let mut turned = (false, false);
    for _ in 0..240 {
        safe(&mut g, "-");
        let (o, k) = (first(&g, OFFIZIER), first(&g, KURGAST));
        assert!(k.x < o.x, "they never walk through each other");
        turned.0 |= k.dir == -1;
        turned.1 |= o.dir == 1;
    }
    assert_eq!(turned, (true, true));
}

/// A singer next to a file: the officers turn at him (he is „on foot")
/// and he stays where he sings.
#[test]
fn officers_turn_at_a_singer_beside_them() {
    let mut g = game(&world(40, &[(11, 14, 'n'), (11, 17, 'o'), (11, 18, 'o')]));
    safe(&mut g, "-");
    let singer = first(&g, SINGER);
    let mut turned = [false, false];
    for _ in 0..280 {
        safe(&mut g, "-");
        for (i, o) in all(&g, OFFIZIER).iter().enumerate() {
            assert!(o.x > singer.x, "never walks through the singer");
            turned[i] |= o.dir == 1;
        }
        assert_eq!(first(&g, SINGER).x, singer.x);
    }
    assert_eq!(turned, [true, true]);
}

/// The Postbus (a Reisebus) drives through a lying Kurgast; the chair
/// stays. A parked bus is „on foot", so an officer turns at it.
#[test]
fn the_postbus_drives_through_a_chair_and_officers_turn_at_it_parked() {
    let mut g = game(&world(
        40,
        &[(11, 12, 'k'), (11, 24, 'y'), (11, 2, '.'), (11, 18, 'P')],
    ));
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    let mut through = false;
    for _ in 0..300 {
        safe(&mut g, "-");
        let k = first(&g, KURGAST);
        assert_eq!(k.x, chair.x);
        assert!(is_platform_now(&k), "in his chair");
        if let Some(b) = all(&g, BUS).first() {
            through |= b.hitbox().overlaps(&k.hitbox());
            assert_eq!(b.kind, BUS);
        }
    }
    assert!(through, "the bus drove through the chair");
    // Parked: an officer marching left at it from its right turns. She
    // stays at the start, beyond the bus's trigger (161 px).
    let mut g = game(&world(40, &[(11, 14, 'y'), (11, 19, 'o')]));
    safe(&mut g, "-");
    let bus = first(&g, BUS);
    assert_eq!(first(&g, OFFIZIER).dir, -1);
    let mut turned = false;
    for _ in 0..240 {
        safe(&mut g, "-");
        let o = first(&g, OFFIZIER);
        assert_eq!(first(&g, BUS).state, state::PARKED);
        assert!(o.x > bus.right() - Fx::from_px(1), "never into the bus");
        turned |= o.dir == 1;
    }
    assert!(turned);
}

/// A chair is no refuge from a bus: the bus is 28 px tall, a Resi on the
/// 16 px chair is hit from the side.
#[test]
fn a_chair_is_no_refuge_from_a_bus() {
    let t = tuning();
    let mut g = game(&world(40, &[(11, 10, 'k'), (11, 18, 'y')]));
    g.player.set_power(Power::Big, &t.player);
    g.player.transform = 0;
    safe(&mut g, "-");
    let chair = lay_down(&mut g);
    stand_on(&mut g, chair);
    g.player.invulnerable = 0;
    let events = until(&mut g, "-", 400, |g| g.player().power != Power::Big);
    assert!(events.contains(&Event::Hurt), "{events:?}");
}

// ----------------------------------------- one stage clock, a respawn

/// Officers, shutters, an Ampel, a Kurgast and the Postbus all start over
/// after a respawn (game-design "New mechanics", the checkpoint rows: as
/// Normal, the `M`-box fresh, the Postbus parked again, the Ampel back to
/// its start, the Kurgast walking, the drill restarted): right after it
/// the world equals a fresh game's, and
/// both run identically from there; the shutters and the drill follow the
/// one restarted clock.
#[test]
fn a_respawn_restarts_shutters_drill_ampel_kurgast_and_postbus_on_one_clock() {
    let marks = [
        (9, 14, 's'),
        (11, 16, 'q'),
        (11, 12, 'a'),
        (11, 8, 'k'),
        (11, 10, 'y'),
        (8, 4, 'M'),
        (12, 6, '.'),
        (13, 6, '.'),
    ];
    let lvl = world(40, &marks);
    let t = tuning();
    let fresh = game(&lvl);
    let mut g = game(&lvl);
    // Bump the `M`-box at column 4 (row 8) from below.
    g.player.x = Fx::from_px(4 * 16 + 2);
    let events = hold(&mut g, "J", 12);
    assert!(events.contains(&Event::BlockBumped), "{events:?}");
    assert_ne!(g.tile(4, 8), fresh.tile(4, 8), "the box is empty");
    while g.stage_clock() < 150 {
        safe(&mut g, "-");
    }
    let i = index(&g, KURGAST);
    lie_down(&mut g.entities[i], &t);
    assert!(!all(&g, AUTO).is_empty(), "the Ampel sent its cars");
    assert_ne!(first(&g, BUS).state, state::PARKED, "the bus left");
    assert_eq!(g.stage_clock(), 150);
    // Into the pit at column 6.
    until(&mut g, "R", 200, |g| matches!(g.mode(), Mode::Dying { .. }));
    let events = until(&mut g, "-", 300, |g| g.mode() == Mode::Playing);
    assert!(events.contains(&Event::Respawned));
    assert_eq!(g.stage_clock(), 0);
    assert_eq!(g.entities, fresh.entities, "the world starts over");
    assert_eq!(g.tile(4, 8), fresh.tile(4, 8), "the `M`-box is fresh");
    assert_eq!(g.player().power, Power::Normal);
    let mut fresh = fresh;
    for k in 0..300 {
        let a = safe(&mut g, "-");
        let b = safe(&mut fresh, "-");
        assert_eq!(a, b, "frame {k}");
        assert_eq!(g.entities, fresh.entities, "frame {k}");
        assert_eq!(g.world_hash(), fresh.world_hash());
        let clock = g.stage_clock();
        let s = first(&g, EntityKind::Fensterladen);
        assert_eq!(u32::from(s.count), (clock + 60) % 240, "the shutter");
        // An officer takes his drill frame from the clock at the start of
        // the frame (`UpdateCtx::stage_clock`), a shutter its frame right
        // after the tick: one frame apart, always the same way.
        let o = first(&g, OFFIZIER);
        if o.active {
            assert_eq!(u32::from(o.count), (clock - 1 + 30) % 240, "the drill");
        }
    }
    assert_eq!(first(&g, KURGAST).state, state::WALK, "walking again");
}

// ------------------------------------------------- the thrown Hackl

/// Puts a resting (flying, no speed) axe centred on `target` into `g` and
/// steps once; returns the events of that step.
fn axe_on(g: &mut Game, target: crate::physics::Aabb) -> Vec<Event> {
    let [w, h] = tuning().hackl.axe_hitbox;
    let mut a = Entity::at_feet(
        EntityKind::ThrownHackl,
        target.x + target.w.div_int(2),
        target.y + target.h.div_int(2) + Fx::from_px(h).div_int(2),
        [w, h],
    );
    a.state = axe::FLY;
    g.entities.push(a);
    safe(g, "-")
}

/// D-097's classification, in play: an axe passes the Ampel, a shutter
/// (open or closed), a lying Kurgast and a presented sabre; bounces off a
/// car and the Postbus; defeats a walking Kurgast and an officer.
#[test]
fn an_axe_meets_every_new_kind_as_d097_classifies_it() {
    let t = tuning();
    let new = |marks: &[(usize, usize, char)]| {
        let mut g = game(&world(40, marks));
        safe(&mut g, "-");
        g
    };
    // Pass: the Ampel's cell.
    let mut g = new(&[(11, 12, 'a')]);
    let a = first(&g, AMPEL);
    assert_eq!(a.axe_hit(), AxeHit::Pass);
    assert!(!axe_on(&mut g, a.hitbox()).contains(&Event::AxeHit));
    assert_eq!(all(&g, AMPEL).len(), 1);
    // Pass: a shutter, open and closed.
    for frames in [0, 160] {
        let mut g = new(&[(9, 12, 'c')]);
        hold(&mut g, "-", frames);
        let s = first(&g, EntityKind::Fensterladen);
        assert_eq!(s.state == shutter::OPEN, frames == 0);
        assert_eq!(s.axe_hit(), AxeHit::Pass);
        assert!(!axe_on(&mut g, s.hitbox()).contains(&Event::AxeHit));
        assert_eq!(first(&g, EntityKind::Fensterladen).state, s.state);
    }
    // Pass: a lying Kurgast.
    let mut g = new(&[(11, 12, 'k')]);
    let chair = lay_down(&mut g);
    assert_eq!(chair.axe_hit(), AxeHit::Pass);
    assert!(!axe_on(&mut g, chair.hitbox()).contains(&Event::AxeHit));
    assert_eq!(first(&g, KURGAST).state, state::LIE);
    // Defeat: a walking Kurgast (100).
    let mut g = new(&[(11, 12, 'k')]);
    let k = first(&g, KURGAST);
    assert_eq!(k.axe_hit(), AxeHit::Defeat);
    assert!(axe_on(&mut g, k.hitbox()).contains(&Event::AxeHit));
    assert!(all(&g, KURGAST).is_empty());
    assert_eq!(g.score(), 100);
    // Defeat: an officer in every drill state (200); the sabre zone
    // passes.
    for st in [state::WALK, state::ATTENTION, state::PRESENT] {
        let mut g = new(&[(11, 12, 'o')]);
        while first(&g, OFFIZIER).state != st {
            safe(&mut g, "-");
        }
        let o = first(&g, OFFIZIER);
        assert_eq!(o.axe_hit(), AxeHit::Defeat);
        if st == state::PRESENT {
            let z = sabre_zone(&o, &t).unwrap();
            let blade = crate::physics::Aabb {
                y: z.y,
                h: Fx::from_px(t.hackl.axe_hitbox[1]),
                ..z
            };
            let mut above = g.clone();
            assert!(!axe_on(&mut above, blade).contains(&Event::AxeHit));
            assert_eq!(all(&above, OFFIZIER).len(), 1, "the blade lets it pass");
        }
        assert!(axe_on(&mut g, o.hitbox()).contains(&Event::AxeHit));
        assert!(all(&g, OFFIZIER).is_empty(), "{st}");
        assert_eq!(g.score(), 200, "{st}");
    }
    // Bounce: a car and the (parked) Postbus.
    let mut g = new(&[]);
    g.entities.push(car_at(Fx::from_px(200), FLOOR - 14));
    let c = first(&g, AUTO);
    assert_eq!(c.axe_hit(), AxeHit::Bounce);
    assert!(axe_on(&mut g, c.hitbox()).contains(&Event::AxeHit));
    assert_eq!(all(&g, AUTO).len(), 1);
    let mut g = new(&[(11, 14, 'y')]);
    let b = first(&g, BUS);
    assert_eq!(b.axe_hit(), AxeHit::Bounce);
    assert!(axe_on(&mut g, b.hitbox()).contains(&Event::AxeHit));
    assert_eq!(all(&g, BUS).len(), 1);
    assert_eq!(g.score(), 0);
}

// ------------------------------------------------------ world_hash

/// `world_hash` (the solver's de-duplication key) changes with every new
/// M6 state: the stage clock (while an officer is in the entity list: an
/// officer not yet woken starts his drill at it), the Ampel's counter and
/// state, a car's position and look, a shutter's frame and state, the
/// Kurgast's rest counter and state, an officer's drill frame and state.
#[test]
fn world_hash_sees_every_m6_state() {
    let marks = [
        (9, 6, 's'),
        (11, 8, 'k'),
        (11, 10, 'a'),
        (11, 16, 'o'),
        (11, 70, 'q'),
        (11, 30, 'y'),
    ];
    let mut g = game(&world(80, &marks));
    for _ in 0..40 {
        safe(&mut g, "-");
    }
    assert!(!all(&g, AUTO).is_empty());
    let late = all(&g, OFFIZIER)[1];
    assert!(!late.active, "the far officer is not awake yet");
    lay_down(&mut g);
    let base = g.world_hash();
    let mut other = g.clone();
    other.stage_clock += 1;
    assert_ne!(other.world_hash(), base, "the stage clock");
    type Change = (&'static str, EntityKind, fn(&mut Entity));
    let changes: [Change; 12] = [
        ("ampel counter", AMPEL, |e| e.count += 1),
        ("ampel state", AMPEL, |e| e.state = state::WAITING),
        ("car position", AUTO, |e| e.x -= Fx::from_px(4)),
        ("car look", AUTO, |e| e.count += 1),
        ("shutter frame", EntityKind::Fensterladen, |e| e.count += 1),
        ("shutter state", EntityKind::Fensterladen, |e| e.state ^= 1),
        ("kurgast rest", KURGAST, |e| e.count += 1),
        ("kurgast wake", KURGAST, |e| e.state = state::WAKE),
        ("drill frame", OFFIZIER, |e| e.count += 1),
        ("drill state", OFFIZIER, |e| e.state = state::PRESENT),
        ("bus state", BUS, |e| e.state = state::DRIVE),
        ("bus position", BUS, |e| e.x -= Fx::from_px(4)),
    ];
    for (what, kind, change) in changes {
        let mut other = g.clone();
        let i = index(&other, kind);
        change(&mut other.entities[i]);
        assert_ne!(other.world_hash(), base, "{what}");
    }
    // Without an officer the clock is not part of it (a shutter's frame is
    // its `count`; the other levels' search is unchanged).
    let mut g = game(&world(40, &[(11, 8, 'k'), (11, 10, 'a'), (9, 14, 's')]));
    hold(&mut g, "-", 5);
    let base = g.world_hash();
    g.stage_clock += 1;
    assert_eq!(g.world_hash(), base);
}

/// Why the clock is in it: two games at the same frame, one of them froze
/// for a power-up (the clock stopped) and then lost it again, hold the
/// same entities but an officer who is not awake yet will start his drill
/// at different frames; the hash keeps them apart.
#[test]
fn a_freeze_moves_the_drill_of_an_officer_not_yet_awake() {
    let lvl = world(80, &[(11, 70, 'o')]);
    let mut a = game(&lvl);
    let mut b = game(&lvl);
    hold(&mut a, "-", 1);
    hold(&mut b, "-", 1);
    b.set_player_power(Power::Big);
    while b.player.transform > 0 {
        hold(&mut b, "-", 1);
        hold(&mut a, "-", 1);
    }
    b.player.set_power(Power::Normal, &tuning().player);
    assert_eq!(a.frame(), b.frame());
    assert_ne!(a.stage_clock(), b.stage_clock());
    assert_eq!(a.entities, b.entities, "nothing else differs");
    assert_ne!(a.world_hash(), b.world_hash());
    // And so it is: brought to him, he marches in both games at different
    // drill frames.
    for g in [&mut a, &mut b] {
        g.player.x = Fx::from_px(1000);
        hold(g, "-", 120);
    }
    let (oa, ob) = (first(&a, OFFIZIER), first(&b, OFFIZIER));
    assert!(oa.active && ob.active);
    assert_ne!(oa.count, ob.count);
}
