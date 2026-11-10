//! Phase 17 window tops: the rough opening is cut to the top's shape and the frame, glass, casing,
//! grille and header follow it. Round and segment tops sit in a flat wall; a roof top sits in the
//! gable wall of the phase-1 house.

use std::collections::BTreeMap;

use cadcraft_arch3d::openings::{glass_plane, opening_meshes};
use cadcraft_arch3d::{ArchId, Mesh, Scene3d, SceneCache};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{Grille, Level, Opening, OpeningKind, OpeningTop, Roof, RoofPlane, Wall};
use cadcraft_geom::{Vec2, Vec3};

/// One 10 m wall along x at y = 0, 0.2 m thick and 3 m tall.
fn flat_wall() -> ArchModel {
    let mut m = ArchModel::default();
    m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(10.0, 0.0), thickness: 0.2, height: 3.0, exterior: true, ..Default::default() });
    m
}

fn window(top: OpeningTop, width: f64, height: f64, sill: f64, rise: f64) -> Opening {
    let mut o = Opening::new(ArchId(0), 5.0, width, height, OpeningKind::Window);
    o.sill = sill;
    o.top = top;
    o.top_rise = rise;
    o
}

/// The part under the probe point, if any.
fn part(s: &Scene3d, x: f64, y: f64, z: f64) -> Option<String> {
    s.probe(Vec3::new(x, y, z)).map(|h| h.part)
}

fn is_wall(s: &Scene3d, x: f64, y: f64, z: f64) -> bool {
    part(s, x, y, z).as_deref() == Some("wall")
}

fn with(mut m: ArchModel, o: Opening) -> ArchModel {
    m.push_opening(o);
    m
}

/// The phase-1 gable house (6/12 gable roof over a 10 x 8 plan); w3 is the west gable wall.
fn gable_house() -> ArchModel {
    let mut model = ArchModel::default();
    model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    for (a, b) in [((0.0, 0.0), (10.0, 0.0)), ((10.0, 0.0), (10.0, 8.0)), ((10.0, 8.0), (0.0, 8.0)), ((0.0, 8.0), (0.0, 0.0))] {
        model.push_wall(Wall {
            start: Vec2::new(a.0, a.1),
            end: Vec2::new(b.0, b.1),
            thickness: 0.2,
            height: 2.4,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
    }
    let mut roof = Roof::default();
    roof.id = ArchId(100);
    roof.level = 0;
    roof.planes.push(RoofPlane {
        base_edge: [-0.1, -0.1, 10.1, -0.1],
        ring: vec![Vec2::new(-0.1, -0.1), Vec2::new(10.1, -0.1), Vec2::new(10.1, 4.0), Vec2::new(-0.1, 4.0)],
        slope: 0.5,
        eave_z: 2.4,
        thickness: 0.04,
        ..Default::default()
    });
    roof.planes.push(RoofPlane {
        base_edge: [-0.1, 8.1, 10.1, 8.1],
        ring: vec![Vec2::new(-0.1, 8.1), Vec2::new(-0.1, 4.0), Vec2::new(10.1, 4.0), Vec2::new(10.1, 8.1)],
        slope: 0.5,
        eave_z: 2.4,
        thickness: 0.04,
        ..Default::default()
    });
    model.roofs.push(roof);
    model
}

/// The roof window of the numbers: 1.0 wide at y 1.5 (offset 6.5) of w3, sill 1.2, its height
/// derived from the roof.
fn roof_window(model: &ArchModel, roof_offset: f64) -> Opening {
    let mut o = Opening::new(ArchId(3), 6.5, 1.0, 1.0, OpeningKind::Window);
    o.sill = 1.2;
    o.top = OpeningTop::Roof;
    o.roof_offset = roof_offset;
    let wall = model.wall(ArchId(3)).expect("w3");
    let (base, top) = cadcraft_arch3d::wall_span(model, wall).unwrap_or((0.0, wall.height));
    let roof_top = cadcraft_arch3d::walls::roof_top_polyline(model, wall, base, top);
    cadcraft_arch::opening_top::validate(&mut o, roof_top.as_deref()).expect("a gable wall under a roof");
    o
}

fn meshes_of(model: &ArchModel, o: &Opening, span: Option<(f64, f64)>) -> Vec<Mesh> {
    let walls: BTreeMap<ArchId, &Wall> = model.walls.iter().map(|w| (w.id, w)).collect();
    opening_meshes(&walls, o, span)
}

fn z_range(m: &Mesh) -> (f64, f64) {
    m.positions.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p[2] as f64), b.max(p[2] as f64)))
}

#[test]
fn round_top_hole_probes() {
    // 0.9 x 1.5 at sill 0.6: spring 1.65, rough arc radius 0.51, rough top 2.16.
    let s = Scene3d::build(&with(flat_wall(), window(OpeningTop::Round, 0.9, 1.5, 0.6, 0.0)));
    // Beside the arch above the spring the wall is solid; a square hole would be open.
    assert!(is_wall(&s, 5.48, 0.0, 2.1), "wall at the corner a square hole would cut");
    assert!(is_wall(&s, 4.52, 0.0, 2.1));
    // Inside the rough arch at du 0.45 (rough top there 1.89) it is open.
    assert!(!is_wall(&s, 5.45, 0.0, 1.8));
    assert!(!is_wall(&s, 5.0, 0.0, 2.14));
    // Above the rough top the wall is solid.
    assert!(is_wall(&s, 5.3, 0.0, 2.3));
    assert!(is_wall(&s, 5.45, 0.0, 1.95));
}

#[test]
fn segment_hole_probes() {
    // 0.9 x 1.2 at sill 0.9, rise 0.15: rough top 2.16, 1.9792853 at the sides.
    let s = Scene3d::build(&with(flat_wall(), window(OpeningTop::Segment, 0.9, 1.2, 0.9, 0.15)));
    assert!(is_wall(&s, 5.5, 0.0, 2.05), "wall above the segment's side");
    assert!(!is_wall(&s, 5.5, 0.0, 1.9), "open under the side's rough top");
    assert!(is_wall(&s, 5.4, 0.0, 2.2));
}

#[test]
fn roof_top_hole_probes() {
    let m = gable_house();
    let o = roof_window(&m, 0.15);
    assert!((o.height - 1.73).abs() < 1e-9, "height {}", o.height);
    let s = Scene3d::build(&with(m, o));
    // Rough top 3.05 at y 1.5 and 2.825 at y 1.05 (window spans y 0.94..2.06).
    assert!(!is_wall(&s, 0.0, 1.5, 3.03));
    assert!(is_wall(&s, 0.0, 1.5, 3.1));
    assert!(!is_wall(&s, 0.0, 1.05, 2.8));
    assert!(is_wall(&s, 0.0, 1.05, 2.9));
    // Outside the rough span the wall is solid at the same height.
    assert!(is_wall(&s, 0.0, 2.2, 2.7));
}

#[test]
fn shaped_frame_glass_and_casing_follow_the_top() {
    let model = with(flat_wall(), window(OpeningTop::Round, 0.9, 1.5, 0.6, 0.0));
    let o = model.openings[0].clone();
    let ms = meshes_of(&model, &o, None);
    for name in ["window-frame", "window-glass", "window-casing"] {
        assert!(ms.iter().any(|m| m.material == name && m.triangle_count() > 0), "{name}");
    }
    // The frame reaches the finished top (sill + height = 2.1) and not the square head's reach beyond.
    let (_, frame_top) = z_range(ms.iter().find(|m| m.material == "window-frame").expect("frame"));
    assert!((frame_top - 2.1).abs() < 1e-3, "frame top {frame_top}");
    // The glass top is the finished top lowered by the frame width.
    let (glass_bottom, glass_top) = z_range(ms.iter().find(|m| m.material == "window-glass").expect("glass"));
    assert!((glass_top - (2.1 - o.frame_width)).abs() < 1e-3, "glass top {glass_top}");
    assert!((glass_bottom - (0.6 + o.frame_width)).abs() < 1e-6);
    // The casing head rises above the finished top by the casing width.
    let (_, casing_top) = z_range(ms.iter().find(|m| m.material == "window-casing").expect("casing"));
    assert!((casing_top - (2.1 + o.casing_width)).abs() < 1e-3, "casing top {casing_top}");
    // Every mesh is closed.
    for m in &ms {
        assert!(m.is_manifold() || m.material.ends_with("-sill"), "{} is manifold", m.material);
    }
    // The glass is narrower than the square window's at the top: its corners are cut.
    let glass = ms.iter().find(|m| m.material == "window-glass").expect("glass");
    let near_top = glass.positions.iter().filter(|p| p[2] as f64 > 2.0).map(|p| (p[0] as f64 - 5.0).abs()).fold(0.0, f64::max);
    assert!(near_top < 0.3, "the glass narrows towards the top: {near_top}");
}

#[test]
fn grille_bars_are_clipped_to_a_round_top() {
    let mut o = window(OpeningTop::Round, 0.9, 1.5, 0.6, 0.0);
    o.grille = Some(Grille { across: 3, down: 4, top_only: false });
    let model = with(flat_wall(), o.clone());
    let ms = meshes_of(&model, &o, None);
    let bars = ms.iter().find(|m| m.material == "window-grille").expect("grille bars");
    let glass = ms.iter().find(|m| m.material == "window-glass").expect("glass");
    // Every bar vertex lies inside the glass outline's circle region: no bar pokes out of the arch.
    let (cx, cz, r) = (5.0, 1.65, 0.45 - o.frame_width);
    for p in &bars.positions {
        let (x, z) = (p[0] as f64, p[2] as f64);
        if z > cz {
            let d = ((x - cx).powi(2) + (z - cz).powi(2)).sqrt();
            assert!(d <= r + 1e-4, "bar vertex outside the arch: ({x}, {z}) at {d}");
        }
    }
    let (_, gt) = z_range(glass);
    let (_, bt) = z_range(bars);
    assert!(bt <= gt + 1e-4);
    // The bars cut by the arch are shorter than a rectangle's.
    let square = window(OpeningTop::Square, 0.9, 1.5, 0.6, 0.0);
    let mut sq = square;
    sq.grille = o.grille;
    let sm = meshes_of(&with(flat_wall(), sq.clone()), &sq, None);
    let (_, st) = z_range(sm.iter().find(|m| m.material == "window-grille").expect("bars"));
    assert!(bt < st);
}

#[test]
fn header_sits_on_the_rough_round_top_and_roof_tops_get_none() {
    let model = with(flat_wall(), window(OpeningTop::Round, 0.9, 1.5, 0.6, 0.0));
    let o = model.openings[0].clone();
    let ms = meshes_of(&model, &o, None);
    let (h0, h1) = z_range(ms.iter().find(|m| m.material == "window-header").expect("header"));
    assert!((h0 - 2.16).abs() < 1e-4, "header bottom {h0}");
    assert!((h1 - 2.36).abs() < 1e-4, "header top {h1}");
    // A square window's header sits on the finished head, as before.
    let sq = window(OpeningTop::Square, 0.9, 1.5, 0.6, 0.0);
    let sm = meshes_of(&with(flat_wall(), sq.clone()), &sq, None);
    let (s0, _) = z_range(sm.iter().find(|m| m.material == "window-header").expect("header"));
    assert!((s0 - 2.1).abs() < 1e-4);
    // A roof top gets no header.
    let m = gable_house();
    let o = roof_window(&m, 0.15);
    let m = with(m, o);
    let scene = Scene3d::build(&m);
    assert!(scene.meshes.iter().any(|x| x.material == "window-glass"));
    assert!(!scene.meshes.iter().any(|x| x.material == "window-header"));
}

#[test]
fn roof_edit_rebuilds_the_wall_and_the_window() {
    let m = gable_house();
    let o = roof_window(&m, 0.15);
    let before = with(m, o);
    let mut after = before.clone();
    for r in &mut after.roofs {
        for p in &mut r.planes {
            p.slope = 0.6;
        }
    }
    let (mut cache, _) = SceneCache::new(&before);
    assert!(is_wall(cache.scene(), 0.0, 1.5, 3.19));
    cache.update(&after);
    let full = Scene3d::build(&after);
    assert_eq!(cache.scene().digest(), full.digest(), "incremental equals full after a re-pitch");
    // The steeper roof lifts the hole and the window with it: 2.4 + 0.6 * 1.6 - 0.15 = 3.21.
    assert!(!is_wall(&full, 0.0, 1.5, 3.19));
    let glass_top = full.meshes.iter().filter(|x| x.material == "window-glass").map(|x| z_range(x).1).fold(0.0, f64::max);
    let old_top = Scene3d::build(&before).meshes.iter().filter(|x| x.material == "window-glass").map(|x| z_range(x).1).fold(0.0, f64::max);
    assert!(glass_top > old_top + 0.1, "glass follows the steeper roof: {glass_top} vs {old_top}");
}

#[test]
fn square_windows_unchanged() {
    let a = with(flat_wall(), window(OpeningTop::Square, 0.9, 1.5, 0.6, 0.0));
    let ms = meshes_of(&a, &a.openings[0], None);
    let (_, ft) = z_range(ms.iter().find(|m| m.material == "window-frame").expect("frame"));
    assert!((ft - 2.1).abs() < 1e-6);
    let (gb, gt) = z_range(ms.iter().find(|m| m.material == "window-glass").expect("glass"));
    let fw = a.openings[0].frame_width;
    assert!((gb - (0.6 + fw)).abs() < 1e-6 && (gt - (2.1 - fw)).abs() < 1e-6);
    let s = Scene3d::build(&a);
    // The square hole is a rectangle: open at its corner.
    assert!(!is_wall(&s, 5.5, 0.0, 2.1));
    assert!(is_wall(&s, 5.7, 0.0, 2.1));
}

#[test]
fn shaped_parts_on_an_upper_level_wall_sit_at_base_plus_sill() {
    let model = with(flat_wall(), window(OpeningTop::Round, 0.9, 1.5, 0.6, 0.0));
    let o = model.openings[0].clone();
    let ground = meshes_of(&model, &o, None);
    let upper = meshes_of(&model, &o, Some((2.7, 5.7)));
    for name in ["window-frame", "window-glass", "window-casing"] {
        let (a0, a1) = z_range(ground.iter().find(|m| m.material == name).expect(name));
        let (b0, b1) = z_range(upper.iter().find(|m| m.material == name).expect(name));
        assert!((b0 - a0 - 2.7).abs() < 1e-4 && (b1 - a1 - 2.7).abs() < 1e-4, "{name}: {b0}..{b1} vs {a0}..{a1}");
    }
    let (g0, _) = z_range(upper.iter().find(|m| m.material == "window-glass").expect("glass"));
    assert!((g0 - (2.7 + 0.6 + o.frame_width)).abs() < 1e-4);
}

#[test]
fn glass_plane_for_shaped_tops() {
    for (top, rise, h, sill) in [(OpeningTop::Round, 0.0, 1.5, 0.6), (OpeningTop::Segment, 0.15, 1.2, 0.9)] {
        let model = with(flat_wall(), window(top, 0.9, h, sill, rise));
        let g = glass_plane(&model, &model.openings[0]).expect("glass");
        let fw = model.openings[0].frame_width;
        assert!((g.bottom - (sill + fw)).abs() < 1e-6, "{top:?} bottom {}", g.bottom);
        assert!((g.top - (sill + h - fw)).abs() < 1e-3, "{top:?} top {}", g.top);
        assert!((g.centre.z - (g.bottom + g.top) / 2.0).abs() < 1e-9);
    }
    let m = gable_house();
    let o = roof_window(&m, 0.15);
    let m = with(m, o);
    let g = glass_plane(&m, &m.openings[0]).expect("glass");
    // Finished top at the centre is 3.05 - 0.12 = 2.93; the glass is a frame width under the
    // sloping top (measured square to the 6/12 slope).
    let drop = m.openings[0].frame_width * (1.0f64 + 0.25).sqrt();
    assert!((g.top - (2.93 - drop)).abs() < 1e-6, "top {}", g.top);
}
