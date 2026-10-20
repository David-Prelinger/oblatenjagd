//! The journey (D-042/D-043): the stations in route order with their place
//! on the route map, the [`Route`] of parsed levels a run plays, and the
//! state of the route-map screen between two stages ([`RouteMap`]).
//!
//! A station is playable when the run's [`Route`] has a level for it. Stage
//! `i` of a route is always station `i` (the standard route is the levels
//! whose `meta.id` equals the station ids, in order, up to the first
//! station without one). Test routes (`route = a b` in a replay) put any
//! levels on the first stations, so a two-level test route plays as
//! Salzburg → St. Gilgen on the map.
//!
//! The core never loads files: the app and xtask parse the levels they have
//! and hand them over ([`select`]); `Game::new_run` takes the route.
//!
//! Map geometry is in screen pixels (320×180). Paths are polylines walked
//! along the major axis of each segment (like the platforms): a segment
//! from `a` to `b` is `max(|dx|, |dy|)` px long and the point `t` px along
//! it is `a + (b - a) * t / len` (integer division). Everything is integer
//! math, so the walk is deterministic.

use std::sync::Arc;

use crate::ParseError;
use crate::level::Level;

/// Where a station's name label sits relative to its dot (the renderer
/// keeps every label at least 4 px inside the screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabelSide {
    /// Left edge right of the dot, vertically centred on it.
    Right,
    /// Right edge left of the dot, vertically centred on it.
    Left,
    /// Centred above the dot.
    Above,
    /// Centred below the dot.
    Below,
}

/// A label slot: a fixed top edge and a right limit (each optional).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelSlot {
    /// Fixed top edge of the label text (screen y).
    pub top: Option<i32>,
    /// The label's right edge stays at or left of this x.
    pub right: Option<i32>,
}

/// One station of the journey.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Station {
    /// Station id = the level id of its stage (`salzburg`) and the text key
    /// suffix of its label (`map.station.<id>`).
    pub id: &'static str,
    /// The station dot on the map, screen px.
    pub dot: (i32, i32),
    /// Where the name label goes.
    pub label: LabelSide,
    /// Fixed top edge of the label text (screen y), overriding the side's.
    pub label_top: Option<i32>,
    /// The label's right edge stays at or left of this x.
    pub label_right: Option<i32>,
    /// An alternate slot for the label, used on a map screen where a big
    /// Resi (Zuckerschock or Hackl) would cover the primary one
    /// (`label_top`, `label_right`) at that screen's start or end dot
    /// (art-director, M3; chosen by the renderer once per screen).
    pub label_alt: Option<LabelSlot>,
    /// End stations (ids) of the map screens that use `label_alt` for
    /// every form of Resi, not only a big one (D-072: she waits on that
    /// dot right next to this label).
    pub label_alt_ends: &'static [&'static str],
    /// Waypoints of the route from the previous station's dot to this one
    /// (both dots excluded); empty for the first station.
    pub path_in: &'static [(i32, i32)],
}

/// The journey in order (`docs/game-design.md` → "Route map": starting
/// layout; dots and labels may move a few px to fit the art).
pub const STATIONS: [Station; 9] = [
    Station {
        id: "salzburg",
        dot: (28, 36),
        label: LabelSide::Right,
        label_top: None,
        label_right: None,
        label_alt: None,
        label_alt_ends: &[],
        path_in: &[],
    },
    Station {
        id: "st_gilgen",
        dot: (100, 70),
        label: LabelSide::Left,
        label_top: None,
        label_right: None,
        label_alt: None,
        label_alt_ends: &[],
        path_in: &[(48, 40), (68, 52), (88, 62)],
    },
    Station {
        id: "brunnwinkl",
        dot: (124, 58),
        // Above, raised clear of Resi's head (art-director, M2).
        // Above-left, close to the dot and clear of Resi (art-director, M2).
        // Alternate (M3): rows 20–28 when a big Resi stands on the
        // St. Gilgen or Brunnwinkl dot of that map screen.
        label: LabelSide::Left,
        label_top: Some(43),
        label_right: Some(114),
        label_alt: Some(LabelSlot {
            top: Some(20),
            right: Some(114),
        }),
        label_alt_ends: &[],
        path_in: &[(108, 64), (116, 58)],
    },
    Station {
        id: "falkenstein",
        dot: (152, 68),
        // Clear of the St. Wolfgang marker (art-director, M2). Alternate
        // (M3): ending at x 170 when a big Resi stands on St. Wolfgang (the
        // teaser), so the outline (x 170) keeps a 1 px gap to her 16×32 box
        // (x 172..) and never touches it (QA, M3 task E19). M4 (D-072):
        // the screen ending on St. Wolfgang uses it for every form (Resi
        // waits there now).
        label: LabelSide::Below,
        label_top: None,
        label_right: Some(172),
        label_alt: Some(LabelSlot {
            top: None,
            right: Some(170),
        }),
        label_alt_ends: &["st_wolfgang"],
        path_in: &[(136, 58), (144, 62)],
    },
    Station {
        id: "st_wolfgang",
        dot: (180, 84),
        // Alternate (M4, D-072 + addendum): raised to top y 59 on the
        // screens ending on Wolfgangsee or Strobl, for every form (Resi on
        // either dot would cover the primary slot; y 69 would touch „Café
        // Zauner").
        label: LabelSide::Right,
        label_top: None,
        label_right: None,
        label_alt: Some(LabelSlot {
            top: Some(59),
            right: None,
        }),
        label_alt_ends: &["wolfgangsee", "strobl"],
        path_in: &[(164, 74), (172, 80)],
    },
    Station {
        id: "wolfgangsee",
        dot: (196, 108),
        label: LabelSide::Below,
        label_top: None,
        label_right: None,
        label_alt: None,
        label_alt_ends: &[],
        path_in: &[(186, 94)],
    },
    Station {
        id: "strobl",
        dot: (232, 118),
        label: LabelSide::Right,
        label_top: None,
        label_right: None,
        label_alt: None,
        label_alt_ends: &[],
        path_in: &[(212, 116)],
    },
    Station {
        id: "bad_ischl",
        dot: (276, 98),
        // Above, not below as in the starting layout: below it would touch
        // the Strobl label. Alternate (M6, D-098): left of the dot, level
        // with it (top 94, right-aligned to x 266), on the screen ending on
        // Bad Ischl, for every form (Resi waits on the dot and covers the
        // primary slot; it also clears the map's church spire).
        label: LabelSide::Above,
        label_top: None,
        label_right: None,
        label_alt: Some(LabelSlot {
            top: Some(94),
            right: Some(266),
        }),
        label_alt_ends: &["bad_ischl"],
        path_in: &[(252, 112), (264, 104)],
    },
    Station {
        id: "cafe_zauner",
        dot: (300, 76),
        // Left of the dot and the cake, clear of Resi (art-director, M2).
        // Alternate (M6, D-098): raised to top 58 (same right edge) on the
        // screen ending on Bad Ischl, for every form (Resi on that dot
        // touches the primary's bottom rows).
        label: LabelSide::Left,
        label_top: Some(69),
        label_right: Some(290),
        label_alt: Some(LabelSlot {
            top: Some(58),
            right: Some(290),
        }),
        label_alt_ends: &["bad_ischl"],
        path_in: &[(288, 88)],
    },
];

/// Index of the station with this id.
pub fn station_index(id: &str) -> Option<usize> {
    STATIONS.iter().position(|s| s.id == id)
}

/// The levels a run plays: stage `i` is station `i` ([`STATIONS`]).
/// At least one level, at most one per station.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    stages: Vec<Arc<Level>>,
}

impl Route {
    /// A route of these levels in this order (stage `i` = station `i`).
    pub fn new(levels: Vec<Level>) -> Result<Route, ParseError> {
        if levels.is_empty() {
            return Err(ParseError::new("a route needs at least one level"));
        }
        if levels.len() > STATIONS.len() {
            return Err(ParseError::new(format!(
                "a route has at most {} levels (one per station), got {}",
                STATIONS.len(),
                levels.len()
            )));
        }
        Ok(Route {
            stages: levels.into_iter().map(Arc::new).collect(),
        })
    }

    /// The real journey: for each station in order the level with that id,
    /// up to the first station without one. `None` without a `salzburg` level.
    pub fn standard(levels: &[Level]) -> Option<Route> {
        let stages: Vec<Arc<Level>> = STATIONS
            .iter()
            .map_while(|s| levels.iter().find(|l| l.meta.id == s.id))
            .map(|l| Arc::new(l.clone()))
            .collect();
        (!stages.is_empty()).then_some(Route { stages })
    }

    /// Number of playable stages.
    pub fn len(&self) -> usize {
        self.stages.len()
    }

    /// Always false (a route has at least one stage).
    pub fn is_empty(&self) -> bool {
        self.stages.is_empty()
    }

    /// The level of stage `i`, if that station is playable.
    pub fn level(&self, i: usize) -> Option<&Arc<Level>> {
        self.stages.get(i)
    }

    /// The stage of the level with this id.
    pub fn stage_of(&self, id: &str) -> Option<usize> {
        self.stages.iter().position(|l| l.meta.id == id)
    }

    /// The level ids in stage order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.stages.iter().map(|l| l.meta.id.as_str())
    }
}

/// The route a run starting at level `start` follows, and the start stage
/// (D-047): with `explicit` level ids (a replay's `route = …`) exactly those
/// levels in that order (`start` must be one of them); otherwise the
/// standard route if `start` is on it; otherwise `None` (a lone level, e.g.
/// the test levels: no route map, M1 ending).
pub fn select(
    levels: &[Level],
    start: &str,
    explicit: &[String],
) -> Result<Option<(Route, usize)>, ParseError> {
    if explicit.is_empty() {
        return Ok(Route::standard(levels).and_then(|r| r.stage_of(start).map(|stage| (r, stage))));
    }
    let picked = explicit
        .iter()
        .map(|id| {
            levels
                .iter()
                .find(|l| l.meta.id == *id)
                .cloned()
                .ok_or_else(|| ParseError::new(format!("route: no level with id `{id}`")))
        })
        .collect::<Result<Vec<Level>, ParseError>>()?;
    let route = Route::new(picked)?;
    let stage = route.stage_of(start).ok_or_else(|| {
        ParseError::new(format!(
            "route: the start level `{start}` is not on the route"
        ))
    })?;
    Ok(Some((route, stage)))
}

// ------------------------------------------------------------ map paths

/// The polyline from station `from` to the next one (both dots included);
/// `None` for the last station.
pub fn path(from: usize) -> Option<Vec<(i32, i32)>> {
    let a = STATIONS.get(from)?;
    let b = STATIONS.get(from + 1)?;
    let mut points = vec![a.dot];
    points.extend_from_slice(b.path_in);
    points.push(b.dot);
    Some(points)
}

/// Length of a segment along its major axis.
fn segment_len(a: (i32, i32), b: (i32, i32)) -> i32 {
    (b.0 - a.0).abs().max((b.1 - a.1).abs())
}

/// Length in px of the path from station `from` to the next (0 for the last).
pub fn path_len(from: usize) -> i32 {
    path(from).map_or(0, |p| p.windows(2).map(|w| segment_len(w[0], w[1])).sum())
}

/// The point `d` px along a polyline (clamped to its ends) and whether the
/// segment there runs leftwards.
pub fn point_along(points: &[(i32, i32)], d: i32) -> ((i32, i32), bool) {
    let mut left = d.max(0);
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = segment_len(a, b);
        if left <= len && len > 0 {
            let p = (
                a.0 + (b.0 - a.0) * left / len,
                a.1 + (b.1 - a.1) * left / len,
            );
            return (p, b.0 < a.0);
        }
        left -= len;
    }
    let last = points.last().copied().unwrap_or((0, 0));
    let leftwards = points.len() >= 2 && last.0 < points[points.len() - 2].0;
    (last, leftwards)
}

// -------------------------------------------------------- the map screen

/// The phases of the route map (D-043), in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPhase {
    /// Resi idle on the cleared station (its marker turns `done` at
    /// `map.done_frame`).
    Arrive,
    /// Resi walks to the next station.
    Walk,
    /// Resi on the next station, the START prompt blinks; START or
    /// `map.wait_frames` → the next stage's intro.
    Wait,
    /// The next station has no level: „Fortsetzung folgt …" for
    /// `map.teaser_frames` (START ignored), then the title.
    Teaser,
}

/// The state of the route map (`Game::route_map`, while `Mode::RouteMap`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RouteMap {
    /// The current phase.
    pub phase: MapPhase,
    /// Frames spent in the current phase (0 on the frame it began).
    pub timer: u16,
    /// The station just cleared; Resi walks to `from + 1`.
    pub from: u8,
    /// Px walked along the path so far.
    pub walked: u16,
    /// Whether the next station has no level (the walk ends in the teaser).
    pub teaser: bool,
}

impl RouteMap {
    /// Resi's feet on the map and whether she faces left.
    pub fn resi(&self) -> ((i32, i32), bool) {
        let from = usize::from(self.from);
        match path(from) {
            Some(p) => point_along(&p, i32::from(self.walked)),
            None => (STATIONS[from.min(STATIONS.len() - 1)].dot, false),
        }
    }

    /// The next station (Resi's destination).
    pub fn next(&self) -> usize {
        usize::from(self.from) + 1
    }

    pub(crate) fn hash(&self, h: &mut crate::hash::Fnv) {
        h.u8(match self.phase {
            MapPhase::Arrive => 0,
            MapPhase::Walk => 1,
            MapPhase::Wait => 2,
            MapPhase::Teaser => 3,
        });
        h.u16(self.timer);
        h.u8(self.from);
        h.u16(self.walked);
        h.bool(self.teaser);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lvl(id: &str) -> Level {
        Level::parse(&format!(
            "[meta]\nid = \"{id}\"\ntime_limit = 100\n[legend]\n\"#\" = \"solid\"\n\
             \"P\" = {{ entity = \"player_start\" }}\n\"G\" = {{ entity = \"goal\" }}\n\
             [map]\ngrid = '''\n.P..G\n#####\n'''\n"
        ))
        .unwrap()
    }

    #[test]
    fn stations_are_the_journey_in_order() {
        let ids: Vec<&str> = STATIONS.iter().map(|s| s.id).collect();
        assert_eq!(
            ids,
            [
                "salzburg",
                "st_gilgen",
                "brunnwinkl",
                "falkenstein",
                "st_wolfgang",
                "wolfgangsee",
                "strobl",
                "bad_ischl",
                "cafe_zauner"
            ]
        );
        assert_eq!(station_index("brunnwinkl"), Some(2));
        assert_eq!(station_index("testbed"), None);
        // On screen, clear of the panel band (y >= 136).
        for s in &STATIONS {
            for &(x, y) in s.path_in.iter().chain([&s.dot]) {
                assert!((4..316).contains(&x) && (4..136).contains(&y), "{}", s.id);
            }
        }
    }

    #[test]
    fn every_walk_takes_20_to_110_frames() {
        // Design: "≈ 30–110" px at 1 px/frame; the short hops along the
        // lake shore are a little shorter.
        for from in 0..STATIONS.len() - 1 {
            let len = path_len(from);
            assert!((20..=110).contains(&len), "{from}: {len}");
        }
        assert_eq!(path_len(STATIONS.len() - 1), 0);
        assert_eq!(path(8), None);
    }

    #[test]
    fn point_along_walks_the_major_axis() {
        let p = [(0, 0), (10, 5), (10, 15)];
        assert_eq!(point_along(&p, 0), ((0, 0), false));
        assert_eq!(point_along(&p, 4), ((4, 2), false));
        assert_eq!(point_along(&p, 10), ((10, 5), false));
        assert_eq!(point_along(&p, 12), ((10, 7), false));
        assert_eq!(point_along(&p, 99), ((10, 15), false), "clamped");
        let back = [(20, 0), (0, 0)];
        assert_eq!(point_along(&back, 5), ((15, 0), true), "leftwards");
        assert_eq!(point_along(&back, 50), ((0, 0), true));
    }

    #[test]
    fn standard_route_stops_at_the_first_missing_station() {
        let levels = [lvl("testbed"), lvl("brunnwinkl"), lvl("salzburg")];
        let r = Route::standard(&levels).unwrap();
        assert_eq!(
            r.ids().collect::<Vec<_>>(),
            ["salzburg"],
            "no st_gilgen yet"
        );
        let levels = [lvl("st_gilgen"), lvl("salzburg"), lvl("brunnwinkl")];
        let r = Route::standard(&levels).unwrap();
        assert_eq!(
            r.ids().collect::<Vec<_>>(),
            ["salzburg", "st_gilgen", "brunnwinkl"]
        );
        assert_eq!(r.stage_of("brunnwinkl"), Some(2));
        assert!(Route::standard(&[lvl("testbed")]).is_none());
    }

    #[test]
    fn select_picks_explicit_standard_or_none() {
        let levels = [lvl("salzburg"), lvl("a"), lvl("b"), lvl("testbed")];
        let (r, s) = select(&levels, "salzburg", &[]).unwrap().unwrap();
        assert_eq!((r.len(), s), (1, 0));
        assert_eq!(select(&levels, "testbed", &[]).unwrap(), None);
        let ab = ["a".to_owned(), "b".to_owned()];
        let (r, s) = select(&levels, "b", &ab).unwrap().unwrap();
        assert_eq!((r.ids().collect::<Vec<_>>(), s), (vec!["a", "b"], 1));
        assert!(select(&levels, "testbed", &ab).is_err(), "start not on it");
        let missing = ["a".to_owned(), "zz".to_owned()];
        assert!(select(&levels, "a", &missing).is_err());
        assert!(Route::new(vec![]).is_err());
        assert!(Route::new(vec![lvl("a"); 10]).is_err());
    }

    #[test]
    fn resi_walks_from_the_cleared_station_to_the_next() {
        let mut m = RouteMap {
            phase: MapPhase::Walk,
            timer: 0,
            from: 0,
            walked: 0,
            teaser: false,
        };
        assert_eq!(m.resi(), (STATIONS[0].dot, false));
        m.walked = path_len(0) as u16;
        assert_eq!(m.resi().0, STATIONS[1].dot);
        assert_eq!(m.next(), 1);
    }
}
