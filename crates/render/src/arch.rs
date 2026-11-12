use std::collections::BTreeMap;

use cadcraft_color::{Color, Rgb};
use cadcraft_doc::{ArchId, Drawing, Handle, Wall};
use cadcraft_fonts::{TextFont, TextParams, place};
use cadcraft_geom::Vec2;

use crate::{DPrim, DisplayList, Kind, Options};

pub use cadcraft_doc::arch::roof_handle;
use cadcraft_doc::arch::{railing_handle, stair_handle};
pub use cadcraft_doc::{is_arch_handle, opening_handle, room_handle};

/// Stable proxy handle for an architectural object: arch objects have no entity handle, so
/// selection and picking map back to the `ArchId` through this reserved range.
pub fn arch_handle(id: ArchId) -> Handle {
    cadcraft_doc::arch_handle(id)
}

/// Whether `build_arch` should skip linework drawn on `layer`.
///
/// A layer that exists and is off, frozen, or — while plotting — marked no-plot is skipped.
/// A missing layer is never skipped, so it draws as before.
pub(crate) fn arch_layer_skipped(d: &Drawing, layer: &str, plotting: bool) -> bool {
    d.layer(layer).map_or(false, |l| !l.on || l.frozen || (plotting && !l.plot))
}

/// Whether an object on `level` is drawn by the plan: always when no level is shown
/// (`plan_level` is `None`), else only when `level` is the shown one.
pub(crate) fn on_shown_level(d: &Drawing, level: u32) -> bool {
    d.arch.plan_level.map_or(true, |shown| shown == level)
}

/// Whether the plan draws `wall`: it spans the shown level (see [`Wall::spans_level`]).
fn wall_on_shown_level(d: &Drawing, wall: &Wall) -> bool {
    match d.arch.plan_level {
        None => true,
        Some(shown) => match d.arch.level(shown) {
            Some(l) => wall.spans_level(l, &d.arch.levels),
            None => wall.base_level == shown,
        },
    }
}

/// The poché (2D cross-section) primitives for `d.arch`, in wall order. The caller merges
/// the returned list into the main one. `_opts` is unused but kept as the caller's hook.
///
/// A wall's poché (fill, outline, gap edges) is skipped when `A-WALL` is off or frozen
/// (and no-plot while plotting); an opening's symbol is skipped by its own layer's state.
/// A missing layer draws as before.
pub fn build_arch(d: &Drawing, _opts: &Options, plotting: bool) -> DisplayList {
    build_arch_with(d, plotting, &crate::arch_plan::plan_annotation(d))
}

/// [`build_arch`] with the plan annotation given (the active view's, made at `plan_level`).
pub(crate) fn build_arch_with(d: &Drawing, plotting: bool, ann: &cadcraft_arch::plan::PlanAnnotation) -> DisplayList {
    let mut list = DisplayList::default();
    // Reference lines sit beneath the wall poché.
    crate::arch_plan::push_plan_lines(d, &mut list, plotting, ann, true);
    let layer = d.layer("A-WALL").map(|l| l.color).unwrap_or(Color::Index(7));
    let ink = crate::ink(Color::ByLayer, layer, Color::Index(7));

    let shapes = cadcraft_arch::join::solve_joins(&d.arch);
    let solved: BTreeMap<ArchId, &Vec<Vec2>> = shapes.iter().map(|s| (s.id, &s.polygon)).collect();

    // Poché gaps from openings are only meaningful while the wall poché is drawn, so the
    // whole loop is skipped when the wall's layer is off or frozen (or no-plot while plotting).
    if !arch_layer_skipped(d, "A-WALL", plotting) {
        let wall_t = layer_transparency(d, "A-WALL");
        for wall in &d.arch.walls {
            if !wall_on_shown_level(d, wall) {
                continue;
            }
            if !wall.start.is_finite() || !wall.end.is_finite() {
                continue;
            }
            let len = wall.start.dist(wall.end);
            if len < 1e-12 || !len.is_finite() {
                continue;
            }
            if wall.thickness <= 0.0 || !wall.thickness.is_finite() {
                continue;
            }
            let half = wall.thickness / 2.0;
            let dir = (wall.end - wall.start).normalized();
            let perp = dir.perp();
            let a = wall.start - perp * half;
            let b = wall.end - perp * half;
            let c = wall.end + perp * half;
            let e = wall.start + perp * half;

            let ring: Vec<Vec2> = match solved.get(&wall.id) {
                Some(r) if !r.is_empty() => r.iter().map(|p| p.clone()).collect(),
                _ => vec![a, b, c, e],
            };

            // Poché gaps: rectangular holes from openings, in wall coordinate space.
            let gaps = cadcraft_arch::symbol::poché_gaps(&d.arch, wall.id);
            let mut polys: Vec<Vec<Vec2>> = vec![ring.clone()];
            for &(gs, ge) in &gaps {
                let p1 = wall.start + dir * gs - perp * half;
                let p2 = wall.start + dir * gs + perp * half;
                let p3 = wall.start + dir * ge + perp * half;
                let p4 = wall.start + dir * ge - perp * half;
                if p1.is_finite() && p2.is_finite() && p3.is_finite() && p4.is_finite() {
                    polys.push(vec![p1, p2, p3, p4]);
                }
            }

            // Filled poché with holes
            let tris = crate::triangulate_evenodd(&polys);
            if tris.len() >= 3 {
                let start = u32::try_from(list.tris.len()).unwrap_or(u32::MAX);
                for q in &tris {
                    list.bounds.add(*q);
                    list.tris.push(*q);
                }
                list.prims.push(DPrim {
                    handle: arch_handle(wall.id),
                    color: ink.rgb,
                    aci7: ink.aci7,
                    lw: 0.0,
                    transparency: wall_t,
                    kind: Kind::Tris,
                    start,
                    len: u32::try_from(tris.len()).unwrap_or(u32::MAX),
                });
            }

            // Outline
            let mut closed = ring;
            if !closed.is_empty() {
                closed.push(closed[0]);
            }
            let start = u32::try_from(list.verts.len()).unwrap_or(u32::MAX);
            for q in &closed {
                list.bounds.add(*q);
                list.verts.push(*q);
            }
            list.prims.push(DPrim {
                handle: arch_handle(wall.id),
                color: ink.rgb,
                aci7: ink.aci7,
                lw: 0.0,
                transparency: wall_t,
                kind: Kind::Polyline,
                start,
                len: u32::try_from(closed.len()).unwrap_or(u32::MAX),
            });

            // Gap edge outlines: one polyline per side of each cut.
            for &(gs, ge) in &gaps {
                let p1 = wall.start + dir * gs - perp * half;
                let p2 = wall.start + dir * gs + perp * half;
                if p1.is_finite() && p2.is_finite() {
                    let start = u32::try_from(list.verts.len()).unwrap_or(u32::MAX);
                    list.bounds.add(p1);
                    list.bounds.add(p2);
                    list.verts.push(p1);
                    list.verts.push(p2);
                    list.prims.push(DPrim {
                        handle: arch_handle(wall.id),
                        color: ink.rgb,
                        aci7: ink.aci7,
                        lw: 0.0,
                        transparency: wall_t,
                        kind: Kind::Polyline,
                        start,
                        len: 2,
                    });
                }
                let p3 = wall.start + dir * ge - perp * half;
                let p4 = wall.start + dir * ge + perp * half;
                if p3.is_finite() && p4.is_finite() {
                    let start = u32::try_from(list.verts.len()).unwrap_or(u32::MAX);
                    list.bounds.add(p3);
                    list.bounds.add(p4);
                    list.verts.push(p3);
                    list.verts.push(p4);
                    list.prims.push(DPrim {
                        handle: arch_handle(wall.id),
                        color: ink.rgb,
                        aci7: ink.aci7,
                        lw: 0.0,
                        transparency: wall_t,
                        kind: Kind::Polyline,
                        start,
                        len: 2,
                    });
                }
            }
        }
    }

    // Opening symbols: each opening emits its generated 2D plan symbol,
    // tagged with the opening's proxy handle so picking and selection work.
    for o in &d.arch.openings {
        // An opening sits on its host wall's base level.
        let host_level = d.arch.wall(o.host).map_or(0, |w| w.base_level);
        if !on_shown_level(d, host_level) {
            continue;
        }
        if let Some(sym) = cadcraft_arch::symbol::opening_symbol(&d.arch, o) {
            if arch_layer_skipped(d, &sym.layer, plotting) {
                continue;
            }
            let all_finite = sym.polylines.iter().all(|p| p.iter().all(|pt| pt.is_finite()));
            if !all_finite {
                continue;
            }
            let sym_layer = d.layer(&sym.layer).map(|l| l.color).unwrap_or(Color::Index(7));
            let sym_ink = crate::ink(Color::ByLayer, sym_layer, Color::Index(7));
            let sym_t = layer_transparency(d, &sym.layer);
            for poly in &sym.polylines {
                if poly.len() < 2 {
                    continue;
                }
                let start = u32::try_from(list.verts.len()).unwrap_or(u32::MAX);
                for q in poly {
                    list.bounds.add(*q);
                    list.verts.push(*q);
                }
                list.prims.push(DPrim {
                    handle: opening_handle(o.id),
                    color: sym_ink.rgb,
                    aci7: sym_ink.aci7,
                    lw: 0.0,
                    transparency: sym_t,
                    kind: Kind::Polyline,
                    start,
                    len: u32::try_from(poly.len()).unwrap_or(u32::MAX),
                });
            }
        }
    }

    // Plan annotation: labels (room names and the view's extras), then the lines (roof linework
    // and the view's overlays), as `cadcraft_arch::plan::annotate` made them.
    crate::arch_plan::push_plan_labels(d, &mut list, plotting, ann);
    crate::arch_plan::push_plan_lines(d, &mut list, plotting, ann, false);

    // Stair plan symbol (see cadcraft_arch::stair_symbol): nosing lines, break, arrow, label and
    // the well, all tagged with the stair's proxy handle. Lines above the plan cut are dashed.
    for s in &d.arch.stairs {
        let layer = if s.layer.is_empty() { "A-STAIR" } else { s.layer.as_str() };
        if arch_layer_skipped(d, layer, plotting) {
            continue;
        }
        let Some(sym) = cadcraft_arch::stair_symbol::stair_symbol(&d.arch, s, d.arch.plan_level) else { continue };
        let layer_color = d.layer(layer).map(|l| l.color).unwrap_or(Color::Index(7));
        let stair_ink = crate::ink(Color::ByLayer, layer_color, Color::Index(7));
        let transparency = layer_transparency(d, layer);
        let handle = stair_handle(s.id);
        for line in &sym.lines {
            if line.dashed {
                for seg in crate::arch_electrical::dashed_segments(d, layer, &line.points) {
                    push_polyline_with_transparency(&mut list, handle, &stair_ink, &seg, transparency);
                }
            } else {
                push_polyline_with_transparency(&mut list, handle, &stair_ink, &line.points, transparency);
            }
        }
        if let Some(label) = &sym.label {
            let params = TextParams::centered(label.at, label.height, label.angle);
            let (sh, _) = place(&TextFont::Stroke, &label.text, &params);
            for stroke in &sh.strokes {
                if stroke.len() >= 2 && stroke.iter().all(|p| p.is_finite()) {
                    push_polyline_with_transparency(&mut list, handle, &stair_ink, stroke, transparency);
                }
            }
        }
    }

    // Railing plan linework: each railing path as one polyline on A-RAIL.
    if !arch_layer_skipped(d, "A-RAIL", plotting) {
        let layer_color = d.layer("A-RAIL").map(|l| l.color).unwrap_or(Color::Index(7));
        let rail_ink = crate::ink(Color::ByLayer, layer_color, Color::Index(7));
        let transparency = layer_transparency(d, "A-RAIL");
        for r in &d.arch.railings {
            if r.path.len() < 2 || !on_shown_level(d, r.level) || !r.path.iter().all(|p| p.is_finite()) {
                continue;
            }
            push_polyline_with_transparency(&mut list, railing_handle(r.id), &rail_ink, &r.path, transparency);
        }
    }

    push_views(d, &mut list, plotting);
    crate::arch_site::push_site(d, &mut list, plotting);
    crate::arch_light::push_lights(d, &mut list, plotting);
    crate::arch_electrical::push_electrical(d, &mut list, plotting);
    crate::arch_casework::push_casework(d, &mut list, plotting);
    crate::arch_molding::push_molding_lines(d, &mut list, plotting);
    crate::arch_ceiling::push_ceiling(d, &mut list, plotting);
    crate::arch_foundation::push_foundations(d, &mut list, plotting);
    crate::arch_furnishing::push_furnishings(d, &mut list, plotting);

    list
}

/// Upper bound on view lines, rings and dimensions drawn per view (hostile geometry).
const MAX_VIEW_ITEMS: usize = 100_000;

/// Draw every linked view's generated geometry: linework on the view's layer, poché rings on
/// `A-SECT-MCUT` and dimensions on `A-ANNO-DIMS`, all tagged with the view's proxy handle.
fn push_views(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    let ink_of = |name: &str| {
        let c = d.layer(name).map(|l| l.color).unwrap_or(Color::Index(7));
        crate::ink(Color::ByLayer, c, Color::Index(7))
    };
    for view in &d.arch.views {
        let Some(g) = &view.generated else { continue };
        if !g.extent.iter().all(|v| v.is_finite()) || !view.insert.iter().all(|v| v.is_finite()) {
            continue;
        }
        let handle = cadcraft_doc::arch::view_handle(view.id);
        let at = |p: [f64; 2]| -> Option<Vec2> {
            if !p.iter().all(|v| v.is_finite()) {
                return None;
            }
            let q = view.placed(p);
            q.iter().all(|v| v.is_finite()).then(|| Vec2::new(q[0], q[1]))
        };

        if !arch_layer_skipped(d, &view.layer, plotting) {
            let ink = ink_of(&view.layer);
            let transparency = layer_transparency(d, &view.layer);
            for l in g.lines.iter().take(MAX_VIEW_ITEMS) {
                if let (Some(a), Some(b)) = (at(l.a), at(l.b)) {
                    push_polyline_with_transparency(list, handle, &ink, &[a, b], transparency);
                }
            }
        }

        if !arch_layer_skipped(d, "A-SECT-MCUT", plotting) {
            let ink = ink_of("A-SECT-MCUT");
            let transparency = layer_transparency(d, "A-SECT-MCUT");
            for ring in g.rings.iter().take(MAX_VIEW_ITEMS) {
                let pts: Vec<Vec2> = ring.points.iter().filter_map(|&p| at(p)).collect();
                if pts.len() < 3 || pts.len() != ring.points.len() {
                    continue;
                }
                let tris = crate::triangulate_evenodd(std::slice::from_ref(&pts));
                if tris.len() >= 3 {
                    let start = u32::try_from(list.tris.len()).unwrap_or(u32::MAX);
                    for q in &tris {
                        list.bounds.add(*q);
                        list.tris.push(*q);
                    }
                    list.prims.push(DPrim {
                        handle,
                        color: ink.rgb,
                        aci7: ink.aci7,
                        lw: 0.0,
                        transparency,
                        kind: Kind::Tris,
                        start,
                        len: u32::try_from(tris.len()).unwrap_or(u32::MAX),
                    });
                }
                let mut closed = pts;
                closed.push(closed[0]);
                push_polyline_with_transparency(list, handle, &ink, &closed, transparency);
            }
        }

        if !arch_layer_skipped(d, "A-ANNO-DIMS", plotting) {
            let ink = ink_of("A-ANNO-DIMS");
            let transparency = layer_transparency(d, "A-ANNO-DIMS");
            let k = if view.scale.is_finite() && view.scale > 0.0 { view.scale / 1000.0 } else { 0.05 };
            for dim in g.dims.iter().take(MAX_VIEW_ITEMS) {
                let (Some(a), Some(b), Some(on)) = (at(dim.a), at(dim.b), at(dim.at)) else { continue };
                push_dim(list, handle, &ink, transparency, k, a, b, on, &dim.text);
            }
        }
    }
}

/// One dimension: extension lines, dimension line, 45-degree ticks and centred text.
#[allow(clippy::too_many_arguments)]
fn push_dim(list: &mut DisplayList, handle: Handle, ink: &crate::Ink, transparency: u8, k: f64, a: Vec2, b: Vec2, on: Vec2, text: &str) {
    let len = a.dist(b);
    if len < 1e-12 || !len.is_finite() {
        return;
    }
    let dir = (b - a) * (1.0 / len);
    let n = dir.perp();
    let off = (on - a).dot(n);
    let (a2, b2) = (a + n * off, b + n * off);
    let over = n * (1.5 * k * if off < 0.0 { -1.0 } else { 1.0 });
    push_polyline_with_transparency(list, handle, ink, &[a, a2 + over], transparency);
    push_polyline_with_transparency(list, handle, ink, &[b, b2 + over], transparency);
    push_polyline_with_transparency(list, handle, ink, &[a2, b2], transparency);
    let tick = (dir + n) * (std::f64::consts::FRAC_1_SQRT_2 * 1.5 * k);
    for p in [a2, b2] {
        push_polyline_with_transparency(list, handle, ink, &[p - tick, p + tick], transparency);
    }
    if text.is_empty() {
        return;
    }
    // Keep the text upright: read along the measured direction pointing right.
    let (rd, rn) = if dir.x < -1e-12 || (dir.x.abs() <= 1e-12 && dir.y < 0.0) { (dir * -1.0, n * -1.0) } else { (dir, n) };
    let h = 2.5 * k;
    let mid = (a2 + b2) * 0.5 + rn * (h * 0.75);
    let params = TextParams::centered(mid, h, rd.y.atan2(rd.x));
    let (sh, _) = place(&TextFont::Stroke, text, &params);
    for stroke in &sh.strokes {
        if stroke.len() >= 2 && stroke.iter().all(|p| p.is_finite()) {
            push_polyline_with_transparency(list, handle, ink, stroke, transparency);
        }
    }
}

/// Append one polyline prim of `pts` tagged `handle`, carrying `transparency` (percent).
pub(crate) fn push_polyline_with_transparency(list: &mut DisplayList, handle: Handle, ink: &crate::Ink, pts: &[Vec2], transparency: u8) {
    let start = u32::try_from(list.verts.len()).unwrap_or(u32::MAX);
    for &p in pts {
        list.bounds.add(p);
        list.verts.push(p);
    }
    list.prims.push(DPrim {
        handle,
        color: ink.rgb,
        aci7: ink.aci7,
        lw: 0.0,
        transparency,
        kind: Kind::Polyline,
        start,
        len: u32::try_from(pts.len()).unwrap_or(u32::MAX),
    });
}

/// The transparency (percent, 0 = opaque) of `layer` in `d`. A missing layer contributes 0.
/// This is the value an architectural prim drawn on `layer` carries into
/// [`DPrim::transparency`], so the plan export dims the prim when the layer is faded and
/// leaves fully-opaque layers untouched.
pub(crate) fn layer_transparency(d: &Drawing, layer: &str) -> u8 {
    d.layer(layer).map(|l| l.transparency).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_color::Rgb;
    use cadcraft_doc::arch::{Opening, OpeningKind, Roof, RoofEdge, RoofEdgeKind, Room};
    use cadcraft_doc::{Drawing, Layer};

    fn wall(start: Vec2, end: Vec2, thickness: f64) -> Wall {
        Wall { start, end, thickness, height: 3.0, ..Wall::default() }
    }

    /// Two levels; a ground wall, an upper wall, a spanning wall, a room per level.
    fn two_level_drawing() -> Drawing {
        use cadcraft_doc::arch::Level;
        let mut d = Drawing::new_metric();
        d.arch.levels = vec![Level { index: 0, elevation: 0.0, ..Default::default() }, Level { index: 1, elevation: 2.7, ..Default::default() }];
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        d.arch.add_wall(Wall { base_level: 1, ..wall(Vec2::new(0.0, 5.0), Vec2::new(4.0, 5.0), 0.2) });
        let sq = |x: f64| vec![Vec2::new(x, 1.0), Vec2::new(x + 2.0, 1.0), Vec2::new(x + 2.0, 3.0), Vec2::new(x, 3.0)];
        d.arch.rooms.push(Room { id: ArchId(100), name: "Down".into(), level: 0, polygon: sq(0.0), ..Default::default() });
        d.arch.rooms.push(Room { id: ArchId(101), name: "Up".into(), level: 1, polygon: sq(10.0), ..Default::default() });
        d
    }

    fn handles(list: &DisplayList) -> std::collections::BTreeSet<Handle> {
        list.prims.iter().map(|p| p.handle).collect()
    }

    #[test]
    fn arch_linework_takes_its_layer_transparency() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        if let Some(layer) = d.layer_mut("A-WALL") {
            layer.transparency = 70;
        }
        let wall = Wall { id: ArchId(7), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 3.0, ..Default::default() };
        d.arch.walls.push(wall);
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        let list = build_arch(&d, &Options::default(), false);
        let t: Vec<u8> = list.prims.iter().filter(|p| p.handle == arch_handle(ArchId(7))).map(|p| p.transparency).collect();
        assert!(!t.is_empty());
        assert!(t.iter().all(|&v| v == 70), "wall transparencies: {t:?}");
    }

    #[test]
    fn the_electrical_view_export_dims_non_electrical_layers() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.ensure_layer("E-POWR");
        d.ensure_layer("E-LITE");
        for layer in &mut d.layers {
            if layer.name.starts_with("E-") {
                layer.on = true;
                layer.frozen = false;
                layer.transparency = 0;
            } else {
                layer.transparency = 70;
            }
        }
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = Wall { id: ArchId(8), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 3.0, ..Default::default() };
        let wall_id = wall.id;
        d.arch.walls.push(wall);
        d.arch.electrical.push(cadcraft_doc::arch::ElectricalDevice {
            id: ArchId(9),
            kind: cadcraft_doc::arch::ElectricalKind::Duplex,
            wall: wall_id,
            side: cadcraft_doc::arch::WallSide::Left,
            offset: 1.0,
            height: 0.3,
            level: 0,
            ..Default::default()
        });
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().filter(|p| p.handle == arch_handle(wall_id)).all(|p| p.transparency == 70));
        assert!(list.prims.iter().filter(|p| p.handle == cadcraft_doc::arch::electrical_handle(ArchId(9))).all(|p| p.transparency == 0));
    }

    #[test]
    fn opaque_layers_draw_unchanged() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.ensure_layer("E-POWR");
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 0, elevation: 0.0, ..Default::default() });
        let wall = Wall { id: ArchId(10), start: Vec2::new(0.0, 0.0), end: Vec2::new(4.0, 0.0), thickness: 0.2, height: 3.0, ..Default::default() };
        d.arch.walls.push(wall);
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().all(|p| p.transparency == 0));
    }

    #[test]
    fn the_plan_draws_only_the_shown_level() {
        let mut d = two_level_drawing();
        d.arch.plan_level = Some(1);
        let h = handles(&build_arch(&d, &Options::default(), false));
        assert!(h.contains(&room_handle(ArchId(101))));
        assert!(!h.contains(&room_handle(ArchId(100))));
        let upper = d.arch.walls.iter().find(|w| w.base_level == 1).map(|w| w.id);
        let lower = d.arch.walls.iter().find(|w| w.base_level == 0).map(|w| w.id);
        assert!(h.contains(&arch_handle(upper.unwrap())));
        assert!(!h.contains(&arch_handle(lower.unwrap())));
    }

    #[test]
    fn a_spanning_wall_draws_on_each_level_it_spans() {
        let mut d = two_level_drawing();
        let id = d.arch.add_wall(Wall { top_level: Some(1), ..wall(Vec2::new(0.0, 9.0), Vec2::new(4.0, 9.0), 0.2) });
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 2, elevation: 5.4, ..Default::default() });
        for shown in [0, 1] {
            d.arch.plan_level = Some(shown);
            let h = handles(&build_arch(&d, &Options::default(), false));
            assert!(h.contains(&arch_handle(id)), "level {shown}");
        }
        d.arch.plan_level = Some(2);
        assert!(!handles(&build_arch(&d, &Options::default(), false)).contains(&arch_handle(id)));
    }

    #[test]
    fn no_shown_level_draws_everything() {
        let mut d = two_level_drawing();
        let all = handles(&build_arch(&d, &Options::default(), false));
        assert!(all.contains(&room_handle(ArchId(100))) && all.contains(&room_handle(ArchId(101))));
        assert_eq!(d.arch.plan_level, None);
        d.arch.plan_level = Some(0);
        assert!(handles(&build_arch(&d, &Options::default(), false)).len() < all.len());
    }

    #[test]
    fn walls_draw_as_filled_poché_in_the_plan() {
        let mut d = Drawing::new_metric();
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let tris_prim = list.prims.iter().find(|p| p.kind == Kind::Tris).expect("no tris prim");
        assert!(tris_prim.len >= 3);

        let poly_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.kind == Kind::Polyline).collect();
        assert_eq!(poly_prims.len(), 1);
        let poly = &poly_prims[0];
        assert_eq!(poly.len, 5);

        let pts = list.points(poly);
        let expected = [Vec2::new(0.0, -0.1), Vec2::new(10.0, -0.1), Vec2::new(10.0, 0.1), Vec2::new(0.0, 0.1), Vec2::new(0.0, -0.1)];
        for (i, (got, exp)) in pts.iter().zip(expected.iter()).enumerate() {
            assert!((got.x - exp.x).abs() < 1e-9, "x[{}]: {} != {}", i, got.x, exp.x);
            assert!((got.y - exp.y).abs() < 1e-9, "y[{}]: {} != {}", i, got.y, exp.y);
        }

        assert!((list.bounds.min.x - 0.0).abs() < 1e-9);
        assert!((list.bounds.max.x - 10.0).abs() < 1e-9);
        assert!((list.bounds.min.y - (-0.1)).abs() < 1e-9);
        assert!((list.bounds.max.y - 0.1).abs() < 1e-9);
    }

    #[test]
    fn arch_prims_carry_the_proxy_handle() {
        let mut d = Drawing::new_metric();
        let id0 = d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        let id1 = d.arch.add_wall(wall(Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0), 0.2));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        assert_eq!(list.prims[0].handle, arch_handle(id0));
        assert_eq!(list.prims[1].handle, arch_handle(id0));
        assert_eq!(list.prims[2].handle, arch_handle(id1));
        assert_eq!(list.prims[3].handle, arch_handle(id1));
    }

    #[test]
    fn empty_arch_model_adds_no_prims() {
        let d = Drawing::new_metric();
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty());
        assert!(list.bounds.is_empty());
    }

    #[test]
    fn arch_poché_respects_the_layer_colour() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        if let Some(l) = d.layer_mut("A-WALL") {
            l.color = Color::Index(3);
        }
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        for p in &list.prims {
            assert_eq!(p.color, Rgb::from_u32(0x00ff00));
            assert!(!p.aci7);
        }

        let mut d = Drawing::new_metric();
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        for p in &list.prims {
            assert!(p.aci7);
        }
    }

    #[test]
    fn arch_handle_is_stable_and_unique_per_wall() {
        assert_eq!(arch_handle(ArchId(0)), Handle(0xFFFF_FFFF_0000_0000));
        assert_eq!(arch_handle(ArchId(7)).0, 0xFFFF_FFFF_0000_0007);
        assert_ne!(arch_handle(ArchId(0)), arch_handle(ArchId(1)));

        let mut d = Drawing::new_metric();
        let id0 = d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        let id1 = d.arch.add_wall(wall(Vec2::new(0.0, 10.0), Vec2::new(10.0, 10.0), 0.2));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        assert_eq!(list.prims[0].handle, arch_handle(id0));
        assert_eq!(list.prims[2].handle, arch_handle(id1));
        assert_ne!(list.prims[0].handle, list.prims[2].handle);
    }

    #[test]
    fn plan_poché_mitres_exterior_corners() {
        let mut d = Drawing::new_metric();
        let t = 0.2;
        let id0 = d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), t));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let poly = list.prims.iter().find(|p| p.kind == Kind::Polyline && p.handle == arch_handle(id0));
        assert!(poly.is_some(), "no polyline prim for bottom wall");
        let Some(poly) = poly else { return };
        let pts = list.points(poly);

        let expected = [Vec2::new(-0.1, -0.1), Vec2::new(10.1, -0.1), Vec2::new(9.9, 0.1), Vec2::new(0.1, 0.1)];
        for exp in &expected {
            assert!(
                pts.iter().any(|p| (p.x - exp.x).abs() < 1e-6 && (p.y - exp.y).abs() < 1e-6),
                "expected point ({}, {}) not in polyline",
                exp.x,
                exp.y
            );
        }
        assert!(!pts.iter().any(|p| (p.x - 10.0).abs() < 1e-6 && (p.y - 0.1).abs() < 1e-6), "unexpected point at (10.0, 0.1)");
        assert!(!pts.iter().any(|p| (p.x - 10.0).abs() < 1e-6 && (p.y + 0.1).abs() < 1e-6), "unexpected point at (10.0, -0.1)");
    }

    #[test]
    fn plan_poché_butts_interior_walls() {
        let mut d = Drawing::new_metric();
        let t = 0.2;
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), t));
        let id_int = d.arch.add_wall(wall(Vec2::new(0.0, 3.0), Vec2::new(10.0, 3.0), 0.1));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let poly = list.prims.iter().find(|p| p.kind == Kind::Polyline && p.handle == arch_handle(id_int));
        assert!(poly.is_some(), "no polyline prim for interior wall");
        let Some(poly) = poly else { return };
        let pts = list.points(poly);

        let min_x = pts.iter().map(|p| p.x).fold(f64::INFINITY, f64::min);
        let max_x = pts.iter().map(|p| p.x).fold(f64::NEG_INFINITY, f64::max);
        assert!((min_x - 0.1).abs() < 1e-6, "min_x {} != 0.1", min_x);
        assert!((max_x - 9.9).abs() < 1e-6, "max_x {} != 9.9", max_x);
    }

    #[test]
    fn plan_poché_envelope_is_the_joined_envelope() {
        let mut d = Drawing::new_metric();
        let t = 0.2;
        d.arch.add_wall(wall(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), t));
        d.arch.add_wall(wall(Vec2::new(0.0, 8.0), Vec2::new(0.0, 0.0), t));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        assert!((list.bounds.min.x - (-0.1)).abs() < 1e-6);
        assert!((list.bounds.min.y - (-0.1)).abs() < 1e-6);
        assert!((list.bounds.max.x - 10.1).abs() < 1e-6);
        assert!((list.bounds.max.y - 8.1).abs() < 1e-6);

        let tris_count = list.prims.iter().filter(|p| p.kind == Kind::Tris).count();
        assert_eq!(tris_count, 4, "expected 4 tris prims, got {}", tris_count);
    }

    #[test]
    fn openings_cut_gaps_in_the_wall_poché() {
        let mut d = Drawing::new_metric();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = arch_handle(wid);
        let wall_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(wall_prims.len(), 4, "expected 4 wall prims, got {}", wall_prims.len());
        let gap_edges: Vec<&DPrim> = wall_prims.iter().copied().filter(|p| p.kind == Kind::Polyline && p.len == 2).collect();
        assert_eq!(gap_edges.len(), 2, "expected 2 gap-edge polylines, got {}", gap_edges.len());
    }

    #[test]
    fn opening_symbols_are_tagged_with_the_opening_handle() {
        let mut d = Drawing::new_metric();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let oid = d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = opening_handle(oid);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "no prims tagged with the opening handle");
    }

    #[test]
    fn door_symbols_use_the_a_door_layer_colour() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(ArchId(0), 5.0, 1.0, 2.1, OpeningKind::Door));

        d.ensure_layer("A-DOOR");
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.color = Color::True(Rgb::from_u32(0xff0000));
        }
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = opening_handle(ArchId(0));
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "no prims for the door opening");
        for p in &symbol_prims {
            assert_eq!(p.color, Rgb::from_u32(0xff0000));
        }
    }

    #[test]
    fn window_symbols_use_the_a_glaz_layer_colour() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(ArchId(0), 5.0, 1.0, 2.1, OpeningKind::Window));

        d.ensure_layer("A-GLAZ");
        if let Some(l) = d.layer_mut("A-GLAZ") {
            l.color = Color::True(Rgb::from_u32(0x0000ff));
        }
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = opening_handle(ArchId(0));
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "no prims for the window opening");
        for p in &symbol_prims {
            assert_eq!(p.color, Rgb::from_u32(0x0000ff));
        }
    }

    #[test]
    fn a_missing_host_draws_nothing() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        // Opening whose host wall does not exist.
        d.arch.push_opening(Opening::new(ArchId(999), 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = opening_handle(ArchId(0));
        assert!(list.prims.iter().all(|p| p.handle != handle), "prims drawn for a missing-host opening");
    }

    /// The wall poché and the opening symbol must not share a handle when the wall and opening
    /// carry the same id.
    #[test]
    fn plan_symbol_prims_use_the_opening_handle() {
        let mut d = Drawing::new_metric();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let oid = d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        // In this fixture the wall and opening share id 0, so their proxy handles differ only
        // by range. Picking the opening must never land on the wall handle.
        assert_ne!(opening_handle(oid), arch_handle(wid), "opening and wall handles must differ");

        let open_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == opening_handle(oid)).collect();
        assert!(!open_prims.is_empty(), "no prims tagged with the opening handle");

        // One wall with one opening draws exactly four poché prims on the wall handle. If the
        // opening symbols mistakenly used the wall handle, this count would be larger.
        let wall_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == arch_handle(wid)).collect();
        assert_eq!(wall_prims.len(), 4, "wall poché prims should be untouched: {}", wall_prims.len());
    }

    #[test]
    fn walls_without_openings_are_unchanged() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 3.0, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let tris_prim = list.prims.iter().find(|p| p.kind == Kind::Tris).expect("no tris prim");
        assert!(tris_prim.len >= 3);

        let poly_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.kind == Kind::Polyline).collect();
        assert_eq!(poly_prims.len(), 1);
        let poly = &poly_prims[0];
        assert_eq!(poly.len, 5);

        let pts = list.points(poly);
        let expected = [Vec2::new(0.0, -0.1), Vec2::new(10.0, -0.1), Vec2::new(10.0, 0.1), Vec2::new(0.0, 0.1), Vec2::new(0.0, -0.1)];
        for (i, (got, exp)) in pts.iter().zip(expected.iter()).enumerate() {
            assert!((got.x - exp.x).abs() < 1e-9, "x[{}]: {} != {}", i, got.x, exp.x);
            assert!((got.y - exp.y).abs() < 1e-9, "y[{}]: {} != {}", i, got.y, exp.y);
        }
    }

    #[test]
    fn arch_prims_emits_room_label_prims() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "Living Room".into(),
            ..Room::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = room_handle(rid);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "no room label prims emitted for room {rid}");
        for p in &label_prims {
            assert_eq!(p.kind, Kind::Polyline, "room label prim kind wrong");
            assert!(p.len >= 2, "room label prim has fewer than 2 vertices: {}", p.len);
        }
    }

    /// A roof's plan edges (ridges/hips/valleys/gable ends) draw as polylines on the
    /// `A-ROOF` layer, using that layer's colour.
    #[test]
    fn roof_edges_draw_on_a_roof_layer() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 8.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.ensure_layer("A-ROOF");
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.color = Color::True(Rgb::from_u32(0xff00ff));
        }
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 2, "expected 2 roof prims (ridge+hip), got {}", roof_prims.len());
        for p in &roof_prims {
            assert_eq!(p.color, Rgb::from_u32(0xff00ff), "roof prim not on the A-ROOF colour");
            assert!(!p.aci7, "A-ROOF is a true colour, not aci7");
            assert_eq!(p.kind, Kind::Polyline, "roof edge should be a polyline");
            assert_eq!(p.len, 2, "roof edge should have 2 vertices");
        }
    }

    /// Every roof edge polyline carries the roof's proxy handle so picking/selection resolves to the roof.
    #[test]
    fn roof_plan_linework_is_tagged_with_the_roof_handle() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(4.0, 4.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::new(4.0, 4.0), b: Vec2::new(4.0, 8.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::GableEnd, a: Vec2::new(0.0, 8.0), b: Vec2::new(4.0, 8.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::new(0.0, 0.0), b: Vec2::new(0.0, 8.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        // All three non-eave edges produce prims on the roof handle.
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 3, "expected 3 non-eave roof prims, got {}", roof_prims.len());
        for p in &roof_prims {
            assert_eq!(p.handle, handle);
            assert_eq!(p.kind, Kind::Polyline);
        }
    }

    /// `Eave` edges contribute no prims — the poché already shows eaves along the walls, so
    /// they must not be drawn a second time by the roof linework.
    #[test]
    fn eave_edges_are_not_drawn_twice() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Eave, a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 8.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        assert!(list.prims.iter().all(|p| p.handle != handle), "eave edges were drawn as roof linework");
        assert_eq!(list.prims.iter().filter(|p| p.handle == handle).count(), 0, "no prim may be emitted for eave edges");
    }

    /// Edges with NaN or infinite endpoints are skipped entirely, not drawn as degenerate prims.
    #[test]
    fn roof_edges_with_nan_endpoints_are_skipped() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(f64::NAN, 0.0), b: Vec2::new(10.0, 8.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::new(10.0, 0.0), b: Vec2::new(f64::INFINITY, 8.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 1, "only the finite ridge edge should draw, got {}", roof_prims.len());
    }

    /// Multiple roofs each produce prims tagged with their own distinct proxy handle.
    #[test]
    fn multiple_roofs_get_distinct_handles() {
        let mut d = Drawing::new_metric();
        let rid0 = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        let rid1 = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(0.0, 10.0), b: Vec2::new(10.0, 10.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let h0 = roof_handle(rid0);
        let h1 = roof_handle(rid1);
        let prims0: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h0).collect();
        let prims1: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h1).collect();
        assert_eq!(prims0.len(), 1, "roof 0 should have 1 prim");
        assert_eq!(prims1.len(), 1, "roof 1 should have 1 prim");
        assert_ne!(h0, h1, "roof handles must be distinct");
    }

    /// Kept under its old name; the behaviour changed. A zero-length roof edge (a == b) is
    /// refused by the plan annotation (`push_line` drops segments shorter than 1e-12), so it
    /// draws nothing.
    #[test]
    fn zero_length_roof_edge_is_drawn() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(5.0, 5.0), b: Vec2::new(5.0, 5.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(roof_prims.is_empty(), "zero-length edge is dropped by the annotation");
    }

    /// Multiple collinear roof edges each draw as separate prims (no automatic merging).
    #[test]
    fn collinear_roof_edges_each_draw_separately() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(5.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(5.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 2, "two collinear edges should produce two prims");
    }

    /// Roof edges with very large coordinates render without overflow or precision loss.
    #[test]
    fn roof_edges_with_huge_coordinates() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(1e12, 1e12), b: Vec2::new(2e12, 2e12), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 1, "huge-coordinate edge should draw");
        let prim = &roof_prims[0];
        let pts = list.points(prim);
        assert!((pts[0].x - 1e12).abs() < 1.0, "x0 precision: {}", pts[0].x);
        assert!((pts[0].y - 1e12).abs() < 1.0, "y0 precision: {}", pts[0].y);
        assert!((pts[1].x - 2e12).abs() < 1.0, "x1 precision: {}", pts[1].x);
        assert!((pts[1].y - 2e12).abs() < 1.0, "y1 precision: {}", pts[1].y);
    }

    /// Stair plan linework carries the stair handle: nosing lines, side lines, arrow, label
    /// strokes and the well.
    #[test]
    fn stair_plan_linework_is_tagged() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair {
            run: 1.0,
            tread_depth: 0.25,
            width: 1.0,
            well: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0), Vec2::new(2.0, 1.0)],
            ..Stair::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(prims.iter().all(|p| p.kind == Kind::Polyline));
        // 4 nosing lines + 2 side lines + 3 arrow strokes + the closed well + the UP strokes.
        assert!(prims.len() >= 4 + 2 + 3 + 1 + 2, "got {}", prims.len());
        assert_eq!(prims.iter().filter(|p| p.len == 4).count(), 1, "the well ring");
        assert_eq!(list.prims.iter().filter(|p| p.handle != h).count(), 0, "everything carries the stair handle");
    }

    /// The phase-5 stair (start (2,1), +x, 0 -> 1) with its well, in a drawing.
    fn phase5_drawing(plan_cut: f64) -> (Drawing, Handle) {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair {
            start: Vec2::new(2.0, 1.0),
            plan_cut,
            well: vec![Vec2::new(2.508, 0.5), Vec2::new(5.81, 0.5), Vec2::new(5.81, 1.5), Vec2::new(2.508, 1.5)],
            ..Stair::default()
        });
        d.touch_arch();
        (d, stair_handle(sid))
    }

    /// Above the plan cut the nosing lines are dashed (cut into many strokes), below it solid.
    #[test]
    fn lower_plan_dashes_the_lines_above_the_cut() {
        let (mut d, h) = phase5_drawing(1.2);
        d.arch.plan_level = Some(0);
        let count = |d: &Drawing| build_arch(d, &Options::default(), false).prims.iter().filter(|p| p.handle == h).count();
        let dashed = count(&d);
        let (mut d2, h2) = phase5_drawing(10.0);
        d2.arch.plan_level = Some(0);
        let solid = build_arch(&d2, &Options::default(), false).prims.iter().filter(|p| p.handle == h2).count();
        assert!(dashed > solid + 8, "dashed {dashed} vs all solid {solid}");
        // Every full-width solid nosing line is a 2-point polyline from y 0.5 to y 1.5.
        let list = build_arch(&d, &Options::default(), false);
        let xs: Vec<f64> = list
            .prims
            .iter()
            .filter(|p| p.handle == h && p.len == 2)
            .filter_map(|p| list.verts.get(p.start as usize..p.start as usize + 2))
            .filter(|v| (v[0].x - v[1].x).abs() < 1e-9 && (v[0].y - 0.5).abs() < 1e-9 && (v[1].y - 1.5).abs() < 1e-9)
            .map(|v| v[0].x)
            .collect();
        assert!(xs.iter().any(|x| (x - 1.975).abs() < 1e-9) && xs.iter().any(|x| (x - 3.499).abs() < 1e-9), "{xs:?}");
        // The plan cut moves the line count: 1.5 keeps one more solid nosing line.
        let (mut d3, h3) = phase5_drawing(1.5);
        d3.arch.plan_level = Some(0);
        let n3 = build_arch(&d3, &Options::default(), false).prims.iter().filter(|p| p.handle == h3).count();
        assert_ne!(n3, dashed);
    }

    /// The upper level shows the part through the well and the well, and differs from the lower.
    #[test]
    fn level_2_plan_shows_dn_and_the_well() {
        let (mut d, h) = phase5_drawing(1.2);
        d.arch.plan_level = Some(1);
        let list = build_arch(&d, &Options::default(), false);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(prims.iter().any(|p| p.len == 5), "the closed well ring");
        // 12 nosing + 2 sides + 3 arrow + well + DN strokes, all solid (no dash splitting).
        assert!(prims.len() >= 12 + 2 + 3 + 1 + 2, "got {}", prims.len());
        assert!(prims.len() < 40, "no dashed strokes: {}", prims.len());
        d.arch.plan_level = Some(2);
        assert!(build_arch(&d, &Options::default(), false).prims.iter().all(|p| p.handle != h), "a third level shows nothing");
    }

    /// Railing linework is one polyline and obeys the A-RAIL layer state.
    #[test]
    fn railing_linework_follows_layer_state() {
        use cadcraft_doc::arch::Railing;
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_railing(Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0), Vec2::new(3.0, 2.0)], ..Railing::default() });
        d.touch_arch();
        let h = railing_handle(rid);
        let list = build_arch(&d, &Options::default(), false);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert_eq!(prims.len(), 1);
        assert_eq!(prims[0].len, 3);

        d.ensure_layer("A-RAIL");
        if let Some(l) = d.layer_mut("A-RAIL") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().all(|p| p.handle != h), "hidden layer still drew");
    }

    /// A roof with an empty edges list draws no prims.
    #[test]
    fn roof_without_edges_draws_nothing() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof { edges: vec![], ..Roof::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        assert!(list.prims.iter().all(|p| p.handle != handle), "roof with no edges drew prims");
    }

    /// When the A-ROOF layer does not exist, roof edges fall back to Color::Index(7) (aci7).
    #[test]
    fn roof_falls_back_to_index_7_when_layer_missing() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 1, "expected 1 roof prim");
        for p in &roof_prims {
            assert!(p.aci7, "roof prim should use aci7 when A-ROOF layer is missing");
        }
    }

    /// Roof handles are stable and unique per roof.
    #[test]
    fn roof_handle_is_stable_and_unique() {
        let h0 = roof_handle(ArchId(0));
        let h1 = roof_handle(ArchId(1));
        assert_ne!(h0, h1, "roof handles must be unique");
        assert_eq!(roof_handle(ArchId(0)), h0, "roof handle must be stable");

        let mut d = Drawing::new_metric();
        let rid0 = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        let rid1 = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(0.0, 10.0), b: Vec2::new(10.0, 10.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let prims0: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == roof_handle(rid0)).collect();
        let prims1: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == roof_handle(rid1)).collect();
        assert_eq!(prims0.len(), 1, "roof 0 should have 1 prim");
        assert_eq!(prims1.len(), 1, "roof 1 should have 1 prim");
    }

    /// Roof edges are drawn in the order they appear in the edges list.
    #[test]
    fn roof_edges_drawn_in_order() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_roof(Roof {
            edges: vec![
                RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Hip, a: Vec2::new(10.0, 0.0), b: Vec2::new(10.0, 8.0), ..RoofEdge::default() },
                RoofEdge { kind: RoofEdgeKind::Valley, a: Vec2::new(0.0, 8.0), b: Vec2::new(10.0, 8.0), ..RoofEdge::default() },
            ],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert_eq!(roof_prims.len(), 3, "expected 3 roof prims");

        let all_verts: Vec<Vec2> = roof_prims.iter().flat_map(|p| list.points(p).iter().copied()).collect();
        let expected =
            vec![Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 0.0), Vec2::new(10.0, 8.0), Vec2::new(0.0, 8.0), Vec2::new(10.0, 8.0)];
        for (got, exp) in all_verts.iter().zip(expected.iter()) {
            assert!((got.x - exp.x).abs() < 1e-9 && (got.y - exp.y).abs() < 1e-9, "vertex mismatch");
        }
    }

    /// A room with an empty name uses the fallback label "r{id}".
    #[test]
    fn room_with_empty_name_uses_fallback_label() {
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "".into(),
            ..Room::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = room_handle(rid);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "no label prims for room with empty name");
    }

    /// Room label prims use the A-ROOM layer's colour when it exists.
    #[test]
    fn room_label_uses_a_room_layer_colour() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOM");
        d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "Living".into(),
            ..Room::default()
        });
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.color = Color::True(Rgb::from_u32(0x00ffff));
        }
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = room_handle(ArchId(0));
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "no label prims");
        for p in &label_prims {
            assert_eq!(p.color, Rgb::from_u32(0x00ffff), "label not on A-ROOM colour");
            assert!(!p.aci7, "A-ROOM is a true colour, not aci7");
        }
    }

    /// When the A-ROOM layer does not exist, room label prims fall back to Color::Index(7) (aci7).
    #[test]
    fn room_label_falls_back_to_index_7_when_layer_missing() {
        let mut d = Drawing::new_metric();
        d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "Living".into(),
            ..Room::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = room_handle(ArchId(0));
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "no label prims");
        for p in &label_prims {
            assert!(p.aci7, "label should use aci7 when A-ROOM layer is missing");
        }
    }

    /// A wall's poché (fill, outline, gap edges) is skipped when `A-WALL` is off or frozen;
    /// a screen build still draws it when the layer is on and not frozen.
    #[test]
    fn arch_walls_are_not_drawn_when_a_wall_is_off_or_frozen() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 3.0, ..Wall::default() });
        d.touch_arch();

        // A-WALL on and not frozen: the wall draws.
        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.is_empty(), "wall should draw when A-WALL is on and not frozen");

        // A-WALL off: no arch prims.
        if let Some(l) = d.layer_mut("A-WALL") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "no prims when A-WALL is off");

        // A-WALL frozen: no arch prims.
        if let Some(l) = d.layer_mut("A-WALL") {
            l.on = true;
            l.frozen = true;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "no prims when A-WALL is frozen");
    }

    /// A plot build skips the wall when `A-WALL` is set to no-plot; a screen build still draws it.
    #[test]
    fn arch_walls_are_not_plotted_when_a_wall_is_no_plot() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 3.0, ..Wall::default() });
        d.touch_arch();

        // Screen build (plotting = false) still draws the wall.
        let list = build_arch(&d, &Options::default(), false);
        assert!(!list.prims.is_empty(), "wall should draw in a screen build");

        // Mark A-WALL no-plot; a plot build skips it.
        if let Some(l) = d.layer_mut("A-WALL") {
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), true);
        assert!(list.prims.is_empty(), "no prims when A-WALL is no-plot during plotting");
    }

    /// Opening symbols (doors, windows) are skipped when their own layer is off or frozen.
    #[test]
    fn opening_symbols_respect_layer_state() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-DOOR");
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let oid = d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();

        // A-DOOR on: symbol draws.
        let list = build_arch(&d, &Options::default(), false);
        let handle = opening_handle(oid);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "door symbol should draw when A-DOOR is on");

        // A-DOOR off: symbol skipped.
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(symbol_prims.is_empty(), "door symbol should be skipped when A-DOOR is off");

        // A-DOOR frozen: symbol skipped.
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.on = true;
            l.frozen = true;
        }
        let list = build_arch(&d, &Options::default(), false);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(symbol_prims.is_empty(), "door symbol should be skipped when A-DOOR is frozen");

        // A-DOOR no-plot: skipped in plot build, drawn in screen build.
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.on = true;
            l.frozen = false;
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "door symbol should draw in screen build when A-DOOR is no-plot");

        let list = build_arch(&d, &Options::default(), true);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(symbol_prims.is_empty(), "door symbol should be skipped in plot build when A-DOOR is no-plot");
    }

    /// Roof edges are skipped when the A-ROOF layer is off, frozen, or no-plot during plotting.
    #[test]
    fn roof_edges_respect_layer_state() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOF");
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();

        // A-ROOF on: edges draw.
        let list = build_arch(&d, &Options::default(), false);
        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!roof_prims.is_empty(), "roof edges should draw when A-ROOF is on");

        // A-ROOF off: edges skipped.
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(roof_prims.is_empty(), "roof edges should be skipped when A-ROOF is off");

        // A-ROOF frozen: edges skipped.
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.on = true;
            l.frozen = true;
        }
        let list = build_arch(&d, &Options::default(), false);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(roof_prims.is_empty(), "roof edges should be skipped when A-ROOF is frozen");

        // A-ROOF no-plot: skipped in plot build, drawn in screen build.
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.on = true;
            l.frozen = false;
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!roof_prims.is_empty(), "roof edges should draw in screen build when A-ROOF is no-plot");

        let list = build_arch(&d, &Options::default(), true);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(roof_prims.is_empty(), "roof edges should be skipped in plot build when A-ROOF is no-plot");
    }

    /// Room labels are skipped when the A-ROOM layer is off, frozen, or no-plot during plotting.
    #[test]
    fn room_labels_respect_layer_state() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOM");
        let rid = d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "Living".into(),
            ..Room::default()
        });
        d.touch_arch();

        // A-ROOM on: labels draw.
        let list = build_arch(&d, &Options::default(), false);
        let handle = room_handle(rid);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "room labels should draw when A-ROOM is on");

        // A-ROOM off: labels skipped.
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(label_prims.is_empty(), "room labels should be skipped when A-ROOM is off");

        // A-ROOM frozen: labels skipped.
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.on = true;
            l.frozen = true;
        }
        let list = build_arch(&d, &Options::default(), false);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(label_prims.is_empty(), "room labels should be skipped when A-ROOM is frozen");

        // A-ROOM no-plot: skipped in plot build, drawn in screen build.
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.on = true;
            l.frozen = false;
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "room labels should draw in screen build when A-ROOM is no-plot");

        let list = build_arch(&d, &Options::default(), true);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(label_prims.is_empty(), "room labels should be skipped in plot build when A-ROOM is no-plot");
    }

    /// Two openings on the same wall produce four gap-edge polylines (two per gap).
    #[test]
    fn multiple_openings_create_multiple_poché_gaps() {
        let mut d = Drawing::new_metric();
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.arch.push_opening(Opening::new(wid, 3.0, 1.0, 2.1, OpeningKind::Door));
        d.arch.push_opening(Opening::new(wid, 7.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let handle = arch_handle(wid);
        let wall_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        // 1 tris + 1 outline + 4 gap edges (2 per opening) = 6
        assert_eq!(wall_prims.len(), 6, "expected 6 wall prims for 2 openings, got {}", wall_prims.len());
        let gap_edges: Vec<&DPrim> = wall_prims.iter().copied().filter(|p| p.kind == Kind::Polyline && p.len == 2).collect();
        assert_eq!(gap_edges.len(), 4, "expected 4 gap-edge polylines for 2 openings, got {}", gap_edges.len());
    }

    /// A wall with NaN in either endpoint is skipped entirely.
    #[test]
    fn wall_with_nan_endpoints_draws_nothing() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(f64::NAN, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "wall with NaN start should draw nothing");

        let mut d2 = Drawing::new_metric();
        d2.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, f64::NAN), thickness: 0.2, height: 2.7, ..Wall::default() });
        d2.touch_arch();
        let list2 = build_arch(&d2, &Options::default(), false);
        assert!(list2.prims.is_empty(), "wall with NaN end should draw nothing");
    }

    /// A wall with zero thickness is skipped.
    #[test]
    fn wall_with_zero_thickness_draws_nothing() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.0, height: 2.7, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "wall with zero thickness should draw nothing");
    }

    /// A wall with negative thickness is skipped.
    #[test]
    fn wall_with_negative_thickness_draws_nothing() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: -0.1, height: 2.7, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "wall with negative thickness should draw nothing");
    }

    /// A wall whose start and end coincide is skipped.
    #[test]
    fn zero_length_wall_draws_nothing() {
        let mut d = Drawing::new_metric();
        d.arch.push_wall(Wall { start: Vec2::new(5.0, 5.0), end: Vec2::new(5.0, 5.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.is_empty(), "zero-length wall should draw nothing");
    }

    /// Stair plan linework obeys the A-STAIR layer state (off, frozen, no-plot).
    #[test]
    fn stair_linework_follows_layer_state() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-STAIR");
        let sid = d.arch.push_stair(Stair { run: 1.0, tread_depth: 0.25, width: 1.0, ..Stair::default() });
        d.touch_arch();
        let h = stair_handle(sid);

        // A-STAIR on: draws.
        let list = build_arch(&d, &Options::default(), false);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(!prims.is_empty(), "stair should draw when A-STAIR is on");

        // A-STAIR off: skipped.
        if let Some(l) = d.layer_mut("A-STAIR") {
            l.on = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().all(|p| p.handle != h), "stair should be skipped when A-STAIR is off");

        // A-STAIR frozen: skipped.
        if let Some(l) = d.layer_mut("A-STAIR") {
            l.on = true;
            l.frozen = true;
        }
        let list = build_arch(&d, &Options::default(), false);
        assert!(list.prims.iter().all(|p| p.handle != h), "stair should be skipped when A-STAIR is frozen");

        // A-STAIR no-plot: drawn in screen build, skipped in plot build.
        if let Some(l) = d.layer_mut("A-STAIR") {
            l.on = true;
            l.frozen = false;
            l.plot = false;
        }
        let list = build_arch(&d, &Options::default(), false);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(!prims.is_empty(), "stair should draw in screen build when A-STAIR is no-plot");

        let list = build_arch(&d, &Options::default(), true);
        assert!(list.prims.iter().all(|p| p.handle != h), "stair should be skipped in plot build when A-STAIR is no-plot");
    }

    /// A stair with tread_depth larger than run draws one nosing line (plus sides, arrow, label).
    #[test]
    fn stair_with_large_tread_depth_draws_one_nosing_line() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair { run: 1.0, tread_depth: 2.0, width: 1.0, ..Stair::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let nosings = list
            .prims
            .iter()
            .filter(|p| p.handle == h && p.len == 2)
            .filter_map(|p| list.verts.get(p.start as usize..p.start as usize + 2))
            .filter(|v| (v[0].x - v[1].x).abs() < 1e-9 && (v[0].y - v[1].y).abs() > 0.99)
            .count();
        assert_eq!(nosings, 1);
    }

    /// Old name kept: a large tread depth draws no riser lines, only one nosing line.
    #[test]
    fn stair_with_large_tread_depth_draws_no_riser_lines() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair { run: 1.0, tread_depth: 2.0, width: 1.0, ..Stair::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let across = list
            .prims
            .iter()
            .filter(|p| p.handle == h && p.len == 2)
            .filter_map(|p| list.verts.get(p.start as usize..p.start as usize + 2))
            .filter(|v| (v[0].x - v[1].x).abs() < 1e-9 && (v[0].y - v[1].y).abs() > 0.99)
            .count();
        assert!(across <= 1, "at most one line across the flight, got {across}");
    }

    /// Old name kept: a tiny tread depth keeps the linework capped.
    #[test]
    fn stair_with_tiny_tread_depth_caps_riser_lines() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair { run: 10.0, tread_depth: 0.001, treads: usize::MAX, width: 1.0, ..Stair::default() });
        d.touch_arch();
        d.arch.plan_level = Some(0);
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let n = list.prims.iter().filter(|p| p.handle == h).count();
        assert!(n < 100_000, "capped linework, got {n}");
    }

    /// A stair with very small tread_depth is capped at MAX_FLIGHT_TREADS nosing lines.
    #[test]
    fn stair_with_tiny_tread_depth_is_capped() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair { run: 10.0, tread_depth: 0.001, treads: usize::MAX, width: 1.0, ..Stair::default() });
        d.touch_arch();
        d.arch.plan_level = Some(0);
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let n = list.prims.iter().filter(|p| p.handle == h).count();
        assert!(n > 1000 && n < 100_000, "capped linework, got {n}");
    }

    /// A stair with NaN in its well polygon draws the symbol but not the well.
    #[test]
    fn stair_with_nan_in_well_draws_no_well() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid = d.arch.push_stair(Stair {
            run: 1.0,
            tread_depth: 0.25,
            width: 1.0,
            well: vec![Vec2::new(0.0, 0.0), Vec2::new(f64::NAN, 0.0), Vec2::new(2.0, 1.0)],
            ..Stair::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        // The symbol is drawn, the well (a closed 4-point ring here) is not.
        assert!(prims.len() >= 9, "got {}", prims.len());
        assert!(prims.iter().all(|p| p.len != 4), "no well ring");
    }

    /// Multiple stairs each produce prims tagged with their own distinct proxy handle.
    #[test]
    fn multiple_stairs_get_distinct_handles() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        let sid0 = d.arch.push_stair(Stair { run: 1.0, tread_depth: 0.25, width: 1.0, ..Stair::default() });
        let sid1 = d.arch.push_stair(Stair { start: Vec2::new(5.0, 5.0), run: 1.0, tread_depth: 0.25, width: 1.0, ..Stair::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let h0 = stair_handle(sid0);
        let h1 = stair_handle(sid1);
        let prims0: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h0).collect();
        let prims1: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h1).collect();
        assert!(!prims0.is_empty(), "stair 0 should have prims");
        assert!(!prims1.is_empty(), "stair 1 should have prims");
        assert_ne!(h0, h1, "stair handles must be distinct");
    }

    /// A railing with coincident points is still drawn (no automatic dedup).
    #[test]
    fn railing_with_coincident_points_is_drawn() {
        use cadcraft_doc::arch::Railing;
        let mut d = Drawing::new_metric();
        let rid = d.arch.push_railing(Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0)], ..Railing::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = railing_handle(rid);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert_eq!(prims.len(), 1, "expected 1 railing prim");
        assert_eq!(prims[0].len, 3, "railing should have 3 vertices (no dedup)");
    }

    /// Opening symbols (doors, windows) carry their layer's transparency.
    #[test]
    fn opening_symbols_carry_layer_transparency() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-DOOR");
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.transparency = 50;
        }
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let oid = d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let handle = opening_handle(oid);
        let symbol_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!symbol_prims.is_empty(), "no door symbol prims");
        assert!(symbol_prims.iter().all(|p| p.transparency == 50), "door symbols should carry A-DOOR transparency");
    }

    /// Roof edges carry the A-ROOF layer's transparency.
    #[test]
    fn roof_edges_carry_layer_transparency() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOF");
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.transparency = 40;
        }
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let handle = roof_handle(rid);
        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!roof_prims.is_empty(), "no roof edge prims");
        assert!(roof_prims.iter().all(|p| p.transparency == 40), "roof edges should carry A-ROOF transparency");
    }

    /// Stair plan linework carries the A-STAIR layer's transparency.
    #[test]
    fn stair_linework_carries_layer_transparency() {
        use cadcraft_doc::arch::Stair;
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-STAIR");
        if let Some(l) = d.layer_mut("A-STAIR") {
            l.transparency = 60;
        }
        let sid = d.arch.push_stair(Stair { run: 1.0, tread_depth: 0.25, width: 1.0, ..Stair::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = stair_handle(sid);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(!prims.is_empty(), "no stair prims");
        assert!(prims.iter().all(|p| p.transparency == 60), "stair linework should carry A-STAIR transparency");
    }

    /// Railing plan linework carries the A-RAIL layer's transparency.
    #[test]
    fn railing_linework_carries_layer_transparency() {
        use cadcraft_doc::arch::Railing;
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-RAIL");
        if let Some(l) = d.layer_mut("A-RAIL") {
            l.transparency = 30;
        }
        let rid = d.arch.push_railing(Railing { path: vec![Vec2::new(0.0, 0.0), Vec2::new(3.0, 0.0)], ..Railing::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let h = railing_handle(rid);
        let prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == h).collect();
        assert!(!prims.is_empty(), "no railing prims");
        assert!(prims.iter().all(|p| p.transparency == 30), "railing linework should carry A-RAIL transparency");
    }

    /// Room labels carry the A-ROOM layer's transparency.
    #[test]
    fn room_labels_carry_layer_transparency() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ROOM");
        if let Some(l) = d.layer_mut("A-ROOM") {
            l.transparency = 25;
        }
        let rid = d.arch.push_room(Room {
            polygon: vec![Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), Vec2::new(4.0, 3.0), Vec2::new(0.0, 3.0)],
            name: "Living".into(),
            ..Room::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let handle = room_handle(rid);
        let label_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == handle).collect();
        assert!(!label_prims.is_empty(), "no room label prims");
        assert!(label_prims.iter().all(|p| p.transparency == 25), "room labels should carry A-ROOM transparency");
    }

    /// Different layers can have different transparencies simultaneously.
    #[test]
    fn mixed_transparencies_across_layers() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.ensure_layer("A-DOOR");
        d.ensure_layer("A-ROOF");
        if let Some(l) = d.layer_mut("A-WALL") {
            l.transparency = 10;
        }
        if let Some(l) = d.layer_mut("A-DOOR") {
            l.transparency = 50;
        }
        if let Some(l) = d.layer_mut("A-ROOF") {
            l.transparency = 80;
        }

        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        let oid = d.arch.push_opening(Opening::new(wid, 5.0, 1.0, 2.1, OpeningKind::Door));
        let rid = d.arch.push_roof(Roof {
            edges: vec![RoofEdge { kind: RoofEdgeKind::Ridge, a: Vec2::new(0.0, 0.0), b: Vec2::new(10.0, 0.0), ..RoofEdge::default() }],
            ..Roof::default()
        });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);

        let wall_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == arch_handle(wid)).collect();
        assert!(wall_prims.iter().all(|p| p.transparency == 10), "walls should be 10% transparent");

        let door_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == opening_handle(oid)).collect();
        assert!(door_prims.iter().all(|p| p.transparency == 50), "doors should be 50% transparent");

        let roof_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == roof_handle(rid)).collect();
        assert!(roof_prims.iter().all(|p| p.transparency == 80), "roof edges should be 80% transparent");
    }

    /// A missing layer gives zero transparency (fully opaque).
    #[test]
    fn missing_layer_gives_zero_transparency() {
        let mut d = Drawing::new_metric();
        // Don't create A-WALL layer — it's missing from the table.
        let wid = d.arch.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 2.7, ..Wall::default() });
        d.touch_arch();
        let list = build_arch(&d, &Options::default(), false);
        let wall_prims: Vec<&DPrim> = list.prims.iter().filter(|p| p.handle == arch_handle(wid)).collect();
        assert!(!wall_prims.is_empty(), "wall should still draw with missing layer");
        assert!(wall_prims.iter().all(|p| p.transparency == 0), "missing layer should give zero transparency");
    }

    #[test]
    fn plan_build_runs_the_ceiling_and_foundation_passes() {
        use cadcraft_doc::arch::{Foundation, FoundationKind, Soffit, Tray};
        let base = two_level_drawing();
        let before = build_arch(&base, &Options::default(), false);
        let mut d = base.clone();
        let sq = vec![Vec2::new(0.5, 1.5), Vec2::new(1.5, 1.5), Vec2::new(1.5, 2.5), Vec2::new(0.5, 2.5)];
        d.arch.push_tray(Tray { room: ArchId(100), inset: 0.3, ..Default::default() });
        d.arch.push_soffit(Soffit { polygon: sq, bottom: Some(2.1), ..Default::default() });
        d.arch.push_foundation(Foundation::default());
        d.arch.push_foundation(Foundation { kind: FoundationKind::Slab, level: 1, ..Default::default() });
        // Hostile ones too: no outline, points that are not numbers, a footing depth that is not one.
        let nan = Vec2::new(f64::NAN, 1e300);
        d.arch.push_soffit(Soffit { polygon: vec![nan, nan, nan], bottom: Some(f64::NAN), ..Default::default() });
        d.arch.push_tray(Tray { room: ArchId(999), polygon: vec![nan], depth: f64::INFINITY, ..Default::default() });
        d.arch.push_foundation(Foundation { footing_depth: f64::NAN, level: 42, ..Default::default() });
        for plotting in [false, true] {
            let list = build_arch(&d, &Options::default(), plotting);
            // Everything drawn before is still drawn.
            assert!(handles(&before).is_subset(&handles(&list)), "plotting {plotting}");
            assert!(list.prims.len() >= before.prims.len());
        }
        d.arch.plan_level = Some(1);
        let _ = build_arch(&d, &Options::default(), false);
    }
}
