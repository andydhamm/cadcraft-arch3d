use cadcraft_arch3d::openings::{GlassRect, grille_bars, has_glass, opening_meshes};
use cadcraft_doc::arch::{Grille, Opening, OpeningKind};
use cadcraft_doc::{ArchId, ArchModel, Wall};
use cadcraft_geom::Vec2;

fn wall_model() -> ArchModel {
    let mut model = ArchModel::default();
    model.walls.push(Wall {
        id: ArchId(1),
        start: Vec2::new(0.0, 0.0),
        end: Vec2::new(10.0, 0.0),
        thickness: 0.3,
        height: 2.5,
        ..Default::default()
    });
    model
}

fn window_opening() -> Opening {
    Opening {
        id: ArchId(100),
        host: ArchId(1),
        offset: 5.0,
        width: 1.2,
        height: 1.5,
        sill: 0.9,
        frame_width: 0.05,
        frame_depth: 0.1,
        casing_width: 0.05,
        casing_depth: 0.02,
        sill_depth: 0.05,
        swing_angle: 0.0,
        style: "window".to_string(),
        recess: 0.0,
        kind: OpeningKind::Window,
        ..Default::default()
    }
}

fn door_opening() -> Opening {
    Opening {
        id: ArchId(101),
        host: ArchId(1),
        offset: 5.0,
        width: 0.9,
        height: 2.1,
        sill: 0.0,
        frame_width: 0.05,
        frame_depth: 0.1,
        casing_width: 0.05,
        casing_depth: 0.02,
        sill_depth: 0.0,
        swing_angle: 0.0,
        style: "hinged".to_string(),
        recess: 0.0,
        kind: OpeningKind::Door,
        ..Default::default()
    }
}

fn helper_grille(across: u32, down: u32, top_only: bool) -> Opening {
    let mut o = window_opening();
    o.grille = Some(Grille { across, down, top_only, ..Default::default() });
    o
}

#[test]
fn grille_2x2_probes() {
    let o = helper_grille(2, 2, false);
    let rect = GlassRect::new(-0.55, 0.55, 0.95, 2.05);
    let bars = grille_bars(&o, &rect);
    assert!(!bars.is_empty());
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    assert!(vertical >= 1, "a 2x2 grille should include at least one vertical bar");
    assert!(horizontal >= 2, "a 2x2 grille splits the horizontal bar across the vertical bar");
}

#[test]
fn grille_bars_evenly_divide_the_glass() {
    let o = helper_grille(3, 3, false);
    let rect = GlassRect::new(-0.6, 0.6, 0.8, 2.2);
    let bars = grille_bars(&o, &rect);
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).collect::<Vec<_>>();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).collect::<Vec<_>>();
    assert_eq!(vertical.len(), 2, "3x3 grille should have two vertical bar positions");
    assert!(horizontal.len() >= 2, "a 3x3 grille should split at least two horizontal rows");
    let u_centres = vertical.iter().map(|b| (b[0] + b[1]) / 2.0).collect::<Vec<_>>();
    let z_centres = horizontal.iter().map(|b| (b[2] + b[3]) / 2.0).collect::<Vec<_>>();
    let unique_u = u_centres.iter().copied().fold(Vec::new(), |mut v, u| {
        if !v.iter().any(|existing| (existing - u).abs() < 1e-6) {
            v.push(u);
        }
        v
    });
    let unique_z = z_centres.iter().copied().fold(Vec::new(), |mut v, z| {
        if !v.iter().any(|existing| (existing - z).abs() < 1e-6) {
            v.push(z);
        }
        v
    });
    assert!(unique_u.len() >= 2, "the vertical bars should divide the glass into multiple columns");
    assert!(unique_z.len() >= 2, "the horizontal bars should divide the glass into multiple rows");
}

#[test]
fn top_part_only() {
    let o = helper_grille(2, 2, true);
    let rect = GlassRect::new(-0.45, 0.45, 0.96, 2.04);
    let bars = grille_bars(&o, &rect);
    assert!(!bars.is_empty(), "top-only grilles should still create bars");

    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).collect::<Vec<_>>();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).collect::<Vec<_>>();
    assert_eq!(vertical.len(), 1, "2x2 top-only should have one central vertical bar");
    assert_eq!(horizontal.len(), 2, "2x2 top-only should split the single horizontal bar into two pieces");
    assert!(((vertical[0][0] + vertical[0][1]) / 2.0).abs() < 1e-9, "top-part vertical bar should be centered on the window centre");
    assert!((vertical[0][2] - 1.5).abs() < 1e-9, "top-part vertical bar should start at the upper-half boundary");
    let z_centres = horizontal.iter().map(|b| (b[2] + b[3]) / 2.0).collect::<Vec<_>>();
    assert_eq!(z_centres.len(), 2);
    assert!((z_centres[0] - 1.77).abs() < 1e-3, "horizontal top bars should be centred in the upper half");
    assert!((z_centres[1] - 1.77).abs() < 1e-3);
}

#[test]
fn top_part_grille_divides_the_upper_half_evenly() {
    let o = helper_grille(2, 2, true);
    let rect = GlassRect::new(-0.45, 0.45, 0.96, 2.04);
    let bars = grille_bars(&o, &rect);
    assert!(!bars.is_empty(), "top-only grilles should still create bars");

    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).collect::<Vec<_>>();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).collect::<Vec<_>>();
    assert_eq!(vertical.len(), 1, "2x2 top-only should have one central vertical bar");
    assert_eq!(horizontal.len(), 2, "2x2 top-only should split the single horizontal bar into two pieces");
    assert!(((vertical[0][0] + vertical[0][1]) / 2.0).abs() < 1e-9, "top-part vertical bar should be centered on the window centre");
    assert!((vertical[0][2] - 1.5).abs() < 1e-9, "top-part vertical bar should start at the upper-half boundary");
    let z_centres = horizontal.iter().map(|b| (b[2] + b[3]) / 2.0).collect::<Vec<_>>();
    assert_eq!(z_centres.len(), 2);
    assert!((z_centres[0] - 1.77).abs() < 1e-3, "horizontal top bars should be centred in the upper half");
    assert!((z_centres[1] - 1.77).abs() < 1e-3);
}

#[test]
fn top_part_grille_leaves_the_lower_half_clear() {
    let o = helper_grille(2, 2, true);
    let rect = GlassRect::new(-0.55, 0.55, 0.9, 2.1);
    let bars = grille_bars(&o, &rect);
    assert!(bars.iter().all(|b| b[2] >= 1.5 - 1e-9), "top-only bars must stay in the upper half");
    assert!(bars.iter().all(|b| b[3] <= 2.1 + 1e-9), "top-only bars must stay inside the glass");
}

#[test]
fn one_by_three_grille_has_two_horizontal_bars() {
    let o = helper_grille(1, 3, false);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    assert_eq!(vertical, 0, "a 1x3 grille has no vertical bars");
    assert_eq!(horizontal, 2, "a 1x3 grille has exactly two horizontal bars");
}

#[test]
fn three_by_one_grille_has_two_vertical_bars() {
    let o = helper_grille(3, 1, false);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    assert_eq!(vertical, 2, "a 3x1 grille has exactly two vertical bars");
    assert_eq!(horizontal, 0, "a 3x1 grille has no horizontal bars");
}

#[test]
fn one_by_one_has_no_bars() {
    let o = helper_grille(1, 1, false);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    assert!(grille_bars(&o, &rect).is_empty());
}

#[test]
fn bars_never_overlap_inside_their_mesh() {
    let o = helper_grille(4, 3, false);
    let rect = GlassRect::new(-0.8, 0.8, 0.7, 2.3);
    let bars = grille_bars(&o, &rect);
    for i in 0..bars.len() {
        for j in (i + 1)..bars.len() {
            let a = bars[i];
            let b = bars[j];
            let overlap_u = a[0].max(b[0]) < a[1].min(b[1]);
            let overlap_z = a[2].max(b[2]) < a[3].min(b[3]);
            assert!(!(overlap_u && overlap_z), "bars overlap: {:?} vs {:?}", a, b);
        }
    }
}

#[test]
fn grille_precedes_glass_in_the_opening_meshes() {
    let model = wall_model();
    let mut o = window_opening();
    o.grille = Some(Grille { across: 2, down: 2, top_only: false, ..Default::default() });
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    let grille_idx = meshes.iter().position(|m| m.material.contains("grille")).unwrap();
    let glass_idx = meshes.iter().position(|m| m.material.contains("glass")).unwrap();
    assert!(grille_idx < glass_idx, "grille should be returned before glass");
}

#[test]
fn glazed_doors_take_grilles_too() {
    let mut o = door_opening();
    o.style = "glass".to_string();
    o.grille = Some(Grille { across: 2, down: 2, top_only: false, ..Default::default() });
    assert!(has_glass(&o));
    let model = wall_model();
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(meshes.iter().any(|m| m.material.contains("grille")), "door grille is missing");
}

#[test]
fn fixed_door_is_glass_in_a_leaf_frame() {
    let mut o = door_opening();
    o.style = "fixed".to_string();
    assert!(has_glass(&o));
    let model = wall_model();
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(meshes.iter().any(|m| m.material.contains("glass")), "fixed door should have glass");
    assert!(meshes.iter().any(|m| m.material.contains("leaf")), "fixed door should have a leaf frame");
}

#[test]
fn sill_and_casing_sides() {
    let model = wall_model();
    let mut o = window_opening();
    o.casing_width = 0.05;
    o.casing_depth = 0.02;
    o.sill_depth = 0.04;
    o.casing_sides = cadcraft_doc::arch::OpeningSides::BOTH;
    o.sill_sides = cadcraft_doc::arch::OpeningSides::BOTH;
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    let casing = meshes.iter().filter(|m| m.material.contains("casing")).count();
    let sill = meshes.iter().filter(|m| m.material.contains("sill")).count();
    assert_eq!(casing, 2, "both casing sides should create 2 casing meshes");
    assert_eq!(sill, 2, "both sill sides should create 2 sill meshes");
}

#[test]
fn default_window_meshes_unchanged() {
    let model = wall_model();
    let o = window_opening();
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(meshes.iter().any(|m| m.material == "window-casing"), "default right-side casing should keep the legacy material name");
    assert!(meshes.iter().any(|m| m.material == "window-sill"), "default right-side sill should keep the legacy material name");
    assert!(
        !meshes.iter().any(|m| m.material.contains("casing-left") || m.material.contains("casing-right")),
        "default should not add mirrored side materials"
    );
    assert!(
        !meshes.iter().any(|m| m.material.contains("sill-left") || m.material.contains("sill-right")),
        "default should not add mirrored side materials"
    );
}

#[test]
fn grille_bars_handle_zero_sized_glass() {
    let o = helper_grille(2, 2, false);
    let rect = GlassRect::new(0.0, 0.0, 0.0, 0.0);
    let bars = grille_bars(&o, &rect);
    for b in &bars {
        assert!(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite());
    }
}

#[test]
fn grille_bars_handle_nan_in_glass() {
    let o = helper_grille(2, 2, false);
    let rect = GlassRect::new(f64::NAN, 1.0, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    for b in &bars {
        let _ = b[0];
    }
}

#[test]
fn grille_bars_handle_huge_glass() {
    let o = helper_grille(2, 2, false);
    let rect = GlassRect::new(0.0, 1e10, 0.0, 1e10);
    let bars = grille_bars(&o, &rect);
    assert!(!bars.is_empty());
    for b in &bars {
        assert!(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite());
    }
}

#[test]
fn grille_bars_zero_across_or_down() {
    let o = helper_grille(0, 0, false);
    let rect = GlassRect::new(0.0, 1.0, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    assert!(bars.is_empty(), "0x0 grille should produce no bars");
}

#[test]
fn grille_bars_clamp_to_max_lites() {
    let o = helper_grille(1000, 1000, false);
    let rect = GlassRect::new(0.0, 1.0, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    for b in &bars {
        assert!(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite());
    }
}

#[test]
fn top_only_one_by_two_has_one_horizontal_bar() {
    let o = helper_grille(1, 2, true);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    assert_eq!(vertical, 0, "1x2 top-only should have no vertical bars");
    assert_eq!(horizontal, 1, "1x2 top-only should have one horizontal bar");
    let z_centre = bars[0][2] + (bars[0][3] - bars[0][2]) / 2.0;
    assert!((z_centre - 0.75).abs() < 1e-9, "bar should be at z=0.75, got {}", z_centre);
}

#[test]
fn top_only_two_by_one_has_one_vertical_bar_in_upper_half() {
    let o = helper_grille(2, 1, true);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    assert_eq!(horizontal, 0, "2x1 top-only should have no horizontal bars");
    assert_eq!(vertical, 1, "2x1 top-only should have one vertical bar");
    assert!((bars[0][2] - 0.5).abs() < 1e-9, "bar should start at z=0.5, got {}", bars[0][2]);
    assert!((bars[0][3] - 1.0).abs() < 1e-9, "bar should end at z=1.0, got {}", bars[0][3]);
}

#[test]
fn grille_bars_respect_glass_bounds() {
    let o = helper_grille(3, 3, false);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    for b in &bars {
        assert!(b[0] >= rect.u0 - 0.011, "bar left edge outside glass: {}", b[0]);
        assert!(b[1] <= rect.u1 + 0.011, "bar right edge outside glass: {}", b[1]);
        assert!(b[2] >= rect.z0 - 0.011, "bar bottom edge outside glass: {}", b[2]);
        assert!(b[3] <= rect.z1 + 0.011, "bar top edge outside glass: {}", b[3]);
    }
}

#[test]
fn grille_with_down_one_has_no_horizontal_bars() {
    let o = helper_grille(3, 1, false);
    let rect = GlassRect::new(-0.5, 0.5, 0.0, 1.0);
    let bars = grille_bars(&o, &rect);
    let horizontal = bars.iter().filter(|b| (b[3] - b[2]).abs() < 0.03).count();
    let vertical = bars.iter().filter(|b| (b[1] - b[0]).abs() < 0.03).count();
    assert_eq!(horizontal, 0, "down=1 should have no horizontal bars");
    assert_eq!(vertical, 2, "across=3 should have 2 vertical bars");
}

#[test]
fn fixed_door_with_grille() {
    let mut o = door_opening();
    o.style = "fixed".to_string();
    o.grille = Some(Grille { across: 2, down: 2, top_only: false, ..Default::default() });
    let model = wall_model();
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(meshes.iter().any(|m| m.material.contains("glass")), "fixed door with grille should have glass");
    assert!(meshes.iter().any(|m| m.material.contains("grille")), "fixed door with grille should have grille");
    assert!(meshes.iter().any(|m| m.material.contains("leaf")), "fixed door with grille should have leaf frame");
}

#[test]
fn sill_sides_none_produces_no_sill() {
    let model = wall_model();
    let mut o = window_opening();
    o.sill_depth = 0.04;
    o.sill_sides = cadcraft_doc::arch::OpeningSides::NONE;
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(!meshes.iter().any(|m| m.material.contains("sill")), "sill_sides NONE should produce no sill");
}

#[test]
fn casing_sides_none_produces_no_casing() {
    let model = wall_model();
    let mut o = window_opening();
    o.casing_width = 0.05;
    o.casing_sides = cadcraft_doc::arch::OpeningSides::NONE;
    let walls = [(ArchId(1), &model.walls[0])].into_iter().collect();
    let meshes = opening_meshes(&walls, &o, None);
    assert!(!meshes.iter().any(|m| m.material.contains("casing")), "casing_sides NONE should produce no casing");
}

#[test]
fn grille_bars_finite_for_all_inputs() {
    let o = helper_grille(4, 3, false);
    let cases = vec![
        GlassRect::new(0.0, 1.0, 0.0, 1.0),
        GlassRect::new(f64::NAN, 1.0, 0.0, 1.0),
        GlassRect::new(0.0, f64::NAN, 0.0, 1.0),
        GlassRect::new(0.0, 1.0, f64::NAN, 1.0),
        GlassRect::new(0.0, 1.0, 0.0, f64::NAN),
        GlassRect::new(f64::INFINITY, 1.0, 0.0, 1.0),
        GlassRect::new(0.0, f64::INFINITY, 0.0, 1.0),
        GlassRect::new(0.0, 1.0, f64::INFINITY, 1.0),
        GlassRect::new(0.0, 1.0, 0.0, f64::INFINITY),
        GlassRect::new(-f64::INFINITY, 1.0, 0.0, 1.0),
        GlassRect::new(0.0, -f64::INFINITY, 0.0, 1.0),
        GlassRect::new(0.0, 1.0, -f64::INFINITY, 1.0),
        GlassRect::new(0.0, 1.0, 0.0, -f64::INFINITY),
    ];
    for rect in cases {
        let bars = grille_bars(&o, &rect);
        for b in &bars {
            assert!(b[0].is_finite() && b[1].is_finite() && b[2].is_finite() && b[3].is_finite(), "non-finite bar for rect {:?}", rect);
        }
    }
}
