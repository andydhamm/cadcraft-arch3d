//! Footings in the plan (phase 16; owner: p16-foundation-stem): dashed on `S-FNDN` in the plan of
//! the level they support, tagged with their foundation's handle.

use cadcraft_arch::foundation::{footing_outline, foundation_walls};
use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{FOUNDATION_LAYER, foundation_handle};

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, on_shown_level, push_polyline_with_transparency};
use crate::arch_electrical::dashed_segments;

/// Append every foundation's dashed footing outlines to `list`: for each foundation on a shown
/// level, the closed outline of the footing under each of its walls, on the foundation's layer
/// (skipped when that layer is off, frozen, or not plotted while plotting).
pub(crate) fn push_foundations(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    for f in &d.arch.foundations {
        let layer = if f.layer.is_empty() { FOUNDATION_LAYER } else { f.layer.as_str() };
        if !on_shown_level(d, f.level) || arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let ink = crate::ink(Color::ByLayer, d.layer(layer).map_or(Color::Index(8), |l| l.color), Color::Index(8));
        let transparency = layer_transparency(d, layer);
        for w in foundation_walls(&d.arch, f.level) {
            let ring = footing_outline(&w);
            if !ring.iter().all(|p| p.is_finite()) || cadcraft_arch::join::polygon_area(&ring).abs() < 1e-12 {
                continue;
            }
            let mut closed = ring.to_vec();
            closed.push(ring[0]);
            for dash in dashed_segments(d, layer, &closed) {
                if dash.len() >= 2 {
                    push_polyline_with_transparency(list, foundation_handle(f.id), &ink, &dash, transparency);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::{Options, build_arch};
    use cadcraft_doc::arch::{Foundation, Level, Room, Wall};
    use cadcraft_doc::{ArchId, Handle};
    use cadcraft_geom::Vec2;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Vec2> {
        vec![Vec2::new(x0, y0), Vec2::new(x1, y0), Vec2::new(x1, y1), Vec2::new(x0, y1)]
    }

    /// The phase-1 house's ring and rooms on level 0, a studio ring on level 1 (on grade at 1.35),
    /// a foundation under each level, and layer S-FNDN.
    fn drawing() -> Drawing {
        let mut d = Drawing::default();
        d.ensure_layer(FOUNDATION_LAYER);
        d.arch.levels.push(Level { index: 1, name: "Studio".into(), elevation: 1.35, ..Default::default() });
        for (level, (x0, y0, x1, y1)) in [(0, (0.0, 0.0, 10.0, 8.0)), (1, (20.0, 0.0, 24.0, 4.0))] {
            for (a, b) in [((x0, y0), (x1, y0)), ((x1, y0), (x1, y1)), ((x1, y1), (x0, y1)), ((x0, y1), (x0, y0))] {
                d.arch.push_wall(Wall {
                    start: Vec2::new(a.0, a.1),
                    end: Vec2::new(b.0, b.1),
                    thickness: 0.2,
                    height: 2.4,
                    exterior: true,
                    base_level: level,
                    ..Default::default()
                });
            }
        }
        d.arch.push_room(Room { level: 0, polygon: rect(0.1, 0.1, 9.9, 7.9), ..Default::default() });
        d.arch.push_foundation(Foundation { level: 0, ..Default::default() });
        d.arch.push_foundation(Foundation { level: 1, ..Default::default() });
        d
    }

    fn prims_of(list: &DisplayList, h: Handle) -> Vec<Vec<Vec2>> {
        list.prims.iter().filter(|p| p.handle == h).map(|p| list.verts[p.start as usize..(p.start + p.len) as usize].to_vec()).collect()
    }

    fn bbox(polys: &[Vec<Vec2>]) -> (Vec2, Vec2) {
        polys.iter().flatten().fold((Vec2::new(f64::MAX, f64::MAX), Vec2::new(f64::MIN, f64::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)))
    }

    #[test]
    fn footings_dashed_on_s_fndn_on_the_founded_level_only() {
        let mut d = drawing();
        let (fd0, fd1) = (foundation_handle(ArchId(0)), foundation_handle(ArchId(1)));
        let mut list = DisplayList::default();
        push_foundations(&d, &mut list, false);
        let house = prims_of(&list, fd0);
        // Four closed outlines, each broken into dashes.
        assert!(house.len() > 8, "dashed: {}", house.len());
        let (lo, hi) = bbox(&house);
        assert!((lo.x + 0.3).abs() < 1e-9 && (lo.y + 0.3).abs() < 1e-9 && (hi.x - 10.3).abs() < 1e-9 && (hi.y - 8.3).abs() < 1e-9, "{lo:?} {hi:?}");
        // Every dash lies on a footing outline's edge (a line x or y = -0.3, 0.3, 7.7, 8.3, 9.7 or 10.3).
        let on_edge = |v: f64| [-0.3, 0.3, 7.7, 8.3, 9.7, 10.3].iter().any(|e| (v - e).abs() < 1e-9);
        for dash in &house {
            assert!(
                dash.windows(2).all(|s| (on_edge(s[0].x) && (s[0].x - s[1].x).abs() < 1e-9) || (on_edge(s[0].y) && (s[0].y - s[1].y).abs() < 1e-9)),
                "{dash:?}"
            );
        }
        let (lo, hi) = bbox(&prims_of(&list, fd1));
        assert!((lo.x - 19.7).abs() < 1e-9 && (hi.x - 24.3).abs() < 1e-9, "{lo:?} {hi:?}");
        // Through the plan builder too.
        assert!(!prims_of(&build_arch(&d, &Options::default(), false), fd0).is_empty());

        // Only the shown level's foundation.
        d.arch.plan_level = Some(1);
        let mut list = DisplayList::default();
        push_foundations(&d, &mut list, false);
        assert!(prims_of(&list, fd0).is_empty() && !prims_of(&list, fd1).is_empty());
        d.arch.plan_level = None;

        // Off, frozen or not plotted: nothing.
        for change in [|l: &mut cadcraft_doc::Layer| l.on = false, |l: &mut cadcraft_doc::Layer| l.frozen = true] {
            let mut hidden = d.clone();
            change(hidden.layer_mut(FOUNDATION_LAYER).unwrap());
            let mut list = DisplayList::default();
            push_foundations(&hidden, &mut list, false);
            assert!(list.prims.is_empty());
        }
        let mut noplot = d.clone();
        noplot.layer_mut(FOUNDATION_LAYER).unwrap().plot = false;
        let mut list = DisplayList::default();
        push_foundations(&noplot, &mut list, true);
        assert!(list.prims.is_empty());
        push_foundations(&noplot, &mut list, false);
        assert!(!list.prims.is_empty());

        // The layer's transparency and colour carry over.
        let mut tinted = d.clone();
        let l = tinted.layer_mut(FOUNDATION_LAYER).unwrap();
        l.transparency = 40;
        l.color = Color::Index(3);
        let mut list = DisplayList::default();
        push_foundations(&tinted, &mut list, false);
        let mut plain = DisplayList::default();
        push_foundations(&d, &mut plain, false);
        assert!(list.prims.iter().all(|p| p.transparency == 40));
        assert_ne!(list.prims[0].color, plain.prims[0].color);
    }

    #[test]
    fn no_foundation_or_hostile_walls_draw_nothing() {
        let mut d = drawing();
        d.arch.foundations.clear();
        let mut list = DisplayList::default();
        push_foundations(&d, &mut list, false);
        assert!(list.prims.is_empty());
        // Hostile walls under a foundation never panic; the finite ones still draw.
        let mut d = drawing();
        d.arch.walls[0].start = Vec2::new(f64::NAN, 0.0);
        d.arch.walls[1].thickness = f64::INFINITY;
        d.arch.push_wall(Wall { start: Vec2::new(1e300, 0.0), end: Vec2::new(-1e300, 0.0), thickness: 0.2, exterior: true, ..Default::default() });
        d.arch.push_foundation(Foundation { level: 99, ..Default::default() });
        let mut list = DisplayList::default();
        push_foundations(&d, &mut list, false);
        assert!(list.verts.iter().all(|p| p.is_finite()));
    }
}
