//! Dimensions of an interior elevation: overall height/width and each cabinet run.

use crate::Scene3d;
use crate::hlr::OrthoView;
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{ArchView, Cabinet, CabinetKind, DimKind, ViewDim};
use cadcraft_geom::Vec2;

// Cabinet front edges within 1 mm are considered part of the same run for automatic
// dimensioning. This keeps adjacent cabinets in a run from being split by a small gap.
const EPS_RUN: f64 = 1e-3;

fn project_u(point: [f64; 2], look: [f64; 2]) -> f64 {
    point[0] * look[1] - point[1] * look[0]
}

fn floor_elevation(model: &ArchModel, view: &ArchView) -> f64 {
    if let Some(level) = view.level {
        return model
            .levels
            .iter()
            .rev()
            .find(|l| l.index == level)
            .map(|l| l.elevation)
            .unwrap_or_else(|| model.levels.last().map(|l| l.elevation).unwrap_or(0.0));
    }
    if let Some(room) = view.room.and_then(|id| model.rooms.iter().find(|r| r.id == id)) {
        return model
            .levels
            .iter()
            .rev()
            .find(|l| l.index == room.level)
            .map(|l| l.elevation)
            .unwrap_or_else(|| model.levels.last().map(|l| l.elevation).unwrap_or(0.0));
    }
    model.levels.last().map(|l| l.elevation).unwrap_or(0.0)
}

fn cabinet_visible(scene: &Scene3d, cabinet: &Cabinet) -> bool {
    scene.meshes.iter().any(|mesh| mesh.object == cabinet.id && mesh.material.contains("cabinet"))
}

fn cabinet_interval(cabinet: &Cabinet, look: [f64; 2]) -> Option<(f64, f64)> {
    if !cabinet.width.is_finite()
        || !cabinet.depth.is_finite()
        || !cabinet.position.x.is_finite()
        || !cabinet.position.y.is_finite()
        || !cabinet.rotation.is_finite()
    {
        return None;
    }
    let rotation = cabinet.rotation.to_radians();
    let front = Vec2::new(0.0, 1.0).rotate(rotation);
    let width_axis = Vec2::new(1.0, 0.0).rotate(rotation);
    if front.x * look[0] + front.y * look[1] >= -0.7 {
        return None;
    }
    let left = cabinet.position + width_axis * (cabinet.width / 2.0) + front * cabinet.depth;
    let right = cabinet.position - width_axis * (cabinet.width / 2.0) + front * cabinet.depth;
    let a = project_u([left.x, left.y], look);
    let b = project_u([right.x, right.y], look);
    Some((a.min(b), a.max(b)))
}

fn push_width_dim(dims: &mut Vec<ViewDim>, u0: f64, u1: f64, v: f64, value: f64) {
    let mid = (u0 + u1) / 2.0;
    dims.push(ViewDim {
        kind: DimKind::Width,
        a: [u0, v],
        b: [u1, v],
        at: [mid, v],
        value,
        text: format!("{:.0}", value * 1000.0),
        ..Default::default()
    });
}

/// The dimensions of an interior view whose lines span `extent`. Owner: p21-interior-dims.
pub(crate) fn interior_dims(model: &ArchModel, view: &ArchView, sc: &Scene3d, _ov: &OrthoView, extent: [f64; 4]) -> Vec<ViewDim> {
    if !view.dimensions {
        return Vec::new();
    }

    let [u0, _, u1, v1] = extent;
    let floor = floor_elevation(model, view);
    let ceiling = v1.max(floor);
    let mut dims = Vec::new();
    let overall_height = ceiling - floor;
    if overall_height.is_finite() && overall_height > 0.0 {
        let at = [u0 - super::DIM_OFFSET, (floor + ceiling) / 2.0];
        dims.push(ViewDim {
            kind: DimKind::Height,
            a: [u0, floor],
            b: [u0, ceiling],
            at,
            value: overall_height,
            text: format!("{:.0}", overall_height * 1000.0),
            ..Default::default()
        });
    }

    let overall_width = (u1 - u0).max(0.0);
    if overall_width.is_finite() && overall_width > 0.0 {
        let width_v = floor - 2.0 * super::DIM_OFFSET;
        push_width_dim(&mut dims, u0, u1, width_v, overall_width);
    }

    let mut lower: Vec<(f64, f64, f64, &Cabinet)> = Vec::new();
    let mut upper: Vec<(f64, f64, f64, &Cabinet)> = Vec::new();
    for cabinet in &model.cabinets {
        if !cabinet_visible(sc, cabinet) {
            continue;
        }
        let Some((left, right)) = cabinet_interval(cabinet, view.look) else {
            continue;
        };
        let width = cabinet.width;
        if !width.is_finite() || width <= 0.0 {
            continue;
        }
        if !cabinet.height.is_finite() || !cabinet.elevation.is_finite() {
            continue;
        }
        let entry = (left.min(right), left.max(right), width, cabinet);
        match cabinet.kind {
            CabinetKind::Base | CabinetKind::Tall => lower.push(entry),
            CabinetKind::Wall => upper.push(entry),
        }
    }

    for (is_lower, band) in [(true, &mut lower), (false, &mut upper)] {
        band.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        let mut run: Vec<(f64, f64, f64, &Cabinet)> = Vec::new();
        let mut run_end = f64::NEG_INFINITY;
        let flush = |run: &mut Vec<(f64, f64, f64, &Cabinet)>, dims: &mut Vec<ViewDim>| {
            if run.is_empty() {
                return;
            }
            let run_left = run.first().map(|item| item.0).unwrap_or(f64::NAN);
            let run_right = run.last().map(|item| item.1).unwrap_or(f64::NAN);
            let band_top = run.iter().map(|item| item.3.elevation + item.3.height).fold(f64::NEG_INFINITY, f64::max);
            let base_y = if is_lower { floor - super::DIM_OFFSET } else { band_top + super::DIM_OFFSET };
            for item in run.iter() {
                push_width_dim(dims, item.0, item.1, base_y, item.2);
            }
            if run.len() > 1 {
                let total = run.iter().map(|item| item.2).sum::<f64>();
                let total_y = if is_lower { base_y - super::DIM_OFFSET } else { base_y + super::DIM_OFFSET };
                push_width_dim(dims, run_left, run_right, total_y, total);
            }
            run.clear();
        };
        for item in band.drain(..) {
            if run.is_empty() {
                run.push(item);
                run_end = item.1;
                continue;
            }
            if item.0 <= run_end + EPS_RUN {
                run.push(item);
                run_end = run_end.max(item.1);
                continue;
            }
            flush(&mut run, &mut dims);
            run.push(item);
            run_end = item.1;
        }
        flush(&mut run, &mut dims);
    }

    dims.truncate(1000);
    dims
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Cabinet, CabinetKind, Level, ViewKind};
    use cadcraft_geom::Vec2;

    fn with_scene(cabinet: &Cabinet) -> Scene3d {
        Scene3d { meshes: vec![crate::Mesh::new("cabinet-front", cabinet.id)], ..Default::default() }
    }

    #[test]
    fn one_cabinet_gives_its_width() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6), "dims: {dims:?}");
    }

    #[test]
    fn a_run_gives_each_cabinet_and_the_total() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut a = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 4.8),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        let mut b = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.35),
            rotation: 90.0,
            width: 0.5,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        a.id = model.push_cabinet(a.clone());
        b.id = model.push_cabinet(b.clone());
        let scene = Scene3d { meshes: vec![crate::Mesh::new("cabinet-front", a.id), crate::Mesh::new("cabinet-front", b.id)], ..Default::default() };
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        let widths: Vec<_> = dims.iter().filter(|d| d.kind == DimKind::Width && d.value > 0.0).collect();
        assert!(widths.iter().any(|d| (d.value - 0.6).abs() < 1e-6));
        assert!(widths.iter().any(|d| (d.value - 0.5).abs() < 1e-6));
        assert!(widths.iter().any(|d| (d.value - 1.1).abs() < 1e-6));
    }

    #[test]
    fn wall_cabinets_dimension_above() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Wall,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.7,
            elevation: 1.4,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6 && d.at[1] > 1.4));
    }

    #[test]
    fn overall_height_is_floor_to_ceiling() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let scene = Scene3d::default();
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(dims.iter().any(|d| d.kind == DimKind::Height && (d.value - 2.4).abs() < 1e-6));
    }

    #[test]
    fn cabinets_not_facing_the_view_are_ignored() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 0.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(!dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6));
    }

    #[test]
    fn no_dims_when_dimensions_off() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let mut view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: false, ..Default::default() };
        view.dimensions = false;
        let scene = with_scene(&cabinet);
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(dims.is_empty());
    }

    #[test]
    fn cabinets_with_fifty_mm_gap_are_separate_runs() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut a = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 4.8),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        let mut b = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.4),
            rotation: 90.0,
            width: 0.5,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        a.id = model.push_cabinet(a.clone());
        b.id = model.push_cabinet(b.clone());
        let scene = Scene3d { meshes: vec![crate::Mesh::new("cabinet-front", a.id), crate::Mesh::new("cabinet-front", b.id)], ..Default::default() };
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        let widths: Vec<_> = dims.iter().filter(|d| d.kind == DimKind::Width && d.value > 0.0).collect();
        assert!(widths.iter().any(|d| (d.value - 0.6).abs() < 1e-6), "missing 0.6: {dims:?}");
        assert!(widths.iter().any(|d| (d.value - 0.5).abs() < 1e-6), "missing 0.5: {dims:?}");
        assert!(!widths.iter().any(|d| (d.value - 1.1).abs() < 1e-6), "unexpected total 1.1: {dims:?}");
    }

    #[test]
    fn cabinets_with_half_mm_gap_form_one_run() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut a = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 4.8),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        let mut b = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.3495),
            rotation: 90.0,
            width: 0.5,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        a.id = model.push_cabinet(a.clone());
        b.id = model.push_cabinet(b.clone());
        let scene = Scene3d { meshes: vec![crate::Mesh::new("cabinet-front", a.id), crate::Mesh::new("cabinet-front", b.id)], ..Default::default() };
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        let widths: Vec<_> = dims.iter().filter(|d| d.kind == DimKind::Width && d.value > 0.0).collect();
        assert!(widths.iter().any(|d| (d.value - 0.6).abs() < 1e-6), "missing 0.6: {dims:?}");
        assert!(widths.iter().any(|d| (d.value - 0.5).abs() < 1e-6), "missing 0.5: {dims:?}");
        assert!(widths.iter().any(|d| (d.value - 1.1).abs() < 1e-6), "missing total 1.1: {dims:?}");
    }

    #[test]
    fn nan_rotation_cabinet_is_ignored() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: f64::NAN,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(
            !dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6),
            "cabinet with NaN rotation should be ignored: {dims:?}"
        );
    }

    #[test]
    fn nan_height_cabinet_is_ignored() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: f64::NAN,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(
            !dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6),
            "cabinet with NaN height should be ignored: {dims:?}"
        );
    }

    #[test]
    fn huge_coordinates_produce_correct_dims() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut cabinet = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(1e15, 1e15),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        cabinet.id = model.push_cabinet(cabinet.clone());
        let scene = with_scene(&cabinet);
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        assert!(dims.iter().any(|d| d.kind == DimKind::Width && (d.value - 0.6).abs() < 1e-6), "dims: {dims:?}");
    }

    #[test]
    fn coincident_cabinets_form_a_run() {
        let mut model = ArchModel::default();
        model.levels.clear();
        model.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
        let mut a = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.6,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        let mut b = Cabinet {
            kind: CabinetKind::Base,
            position: Vec2::new(0.0, 5.0),
            rotation: 90.0,
            width: 0.5,
            depth: 0.3,
            height: 0.8,
            elevation: 0.0,
            ..Default::default()
        };
        a.id = model.push_cabinet(a.clone());
        b.id = model.push_cabinet(b.clone());
        let scene = Scene3d { meshes: vec![crate::Mesh::new("cabinet-front", a.id), crate::Mesh::new("cabinet-front", b.id)], ..Default::default() };
        let view = ArchView { kind: ViewKind::Interior, look: [1.0, 0.0], level: Some(0), dimensions: true, ..Default::default() };
        let dims = interior_dims(&model, &view, &scene, &crate::hlr::OrthoView::elevation(view.look).unwrap(), [0.0, 0.0, 1.0, 2.4]);
        let widths: Vec<_> = dims.iter().filter(|d| d.kind == DimKind::Width && d.value > 0.0).collect();
        assert!(widths.iter().any(|d| (d.value - 0.6).abs() < 1e-6), "missing 0.6: {dims:?}");
        assert!(widths.iter().any(|d| (d.value - 0.5).abs() < 1e-6), "missing 0.5: {dims:?}");
        assert!(widths.iter().any(|d| (d.value - 1.1).abs() < 1e-6), "missing total 1.1: {dims:?}");
    }
}
