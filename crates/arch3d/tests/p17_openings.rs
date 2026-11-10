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
    o.grille = Some(Grille { across, down, top_only });
    o
}

#[test]
fn grille_2x2_probes() {
    let o = helper_grille(2, 2, false);
    let rect = GlassRect { u0: -0.55, u1: 0.55, z0: 0.95, z1: 2.05 };
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
    let rect = GlassRect { u0: -0.6, u1: 0.6, z0: 0.8, z1: 2.2 };
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
    let rect = GlassRect { u0: -0.55, u1: 0.55, z0: 0.9, z1: 2.1 };
    let bars = grille_bars(&o, &rect);
    assert!(!bars.is_empty(), "top-only grilles should still create bars");
    assert!(bars.iter().all(|b| b[2] >= 1.5 - 0.02), "top-only bars should stay in the upper half of the glass");
    assert!(bars.iter().all(|b| b[3] <= 2.1 + 1e-6), "top-only bars must stay within the upper glass half");
}

#[test]
fn one_by_one_has_no_bars() {
    let o = helper_grille(1, 1, false);
    let rect = GlassRect { u0: -0.5, u1: 0.5, z0: 0.0, z1: 1.0 };
    assert!(grille_bars(&o, &rect).is_empty());
}

#[test]
fn bars_never_overlap_inside_their_mesh() {
    let o = helper_grille(4, 3, false);
    let rect = GlassRect { u0: -0.8, u1: 0.8, z0: 0.7, z1: 2.3 };
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
    o.grille = Some(Grille { across: 2, down: 2, top_only: false });
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
    o.grille = Some(Grille { across: 2, down: 2, top_only: false });
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
