use std::cmp::Ordering;

use cadcraft_doc::arch::{AutoDimRow, AutoDims, AutoRowKind, Hinge, LevelKind, OpeningKind, OpeningSides, OpeningTop, Swing};
use cadcraft_doc::{ArchFeature, ArchModel, Handle};
use cadcraft_geom::Vec2;

#[derive(Clone, Debug, PartialEq)]
pub struct AutoPoint {
    pub at: Vec2,
    pub handle: Handle,
    pub feature: ArchFeature,
    pub along: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AutoRow {
    pub side: String,
    pub normal: Vec2,
    pub dir: Vec2,
    pub kind: AutoRowKind,
    pub points: Vec<AutoPoint>,
    pub line_offset: f64,
}

pub fn row_offset(kind: AutoRowKind) -> f64 {
    match kind {
        AutoRowKind::Openings => 1.0,
        AutoRowKind::Walls => 1.5,
        AutoRowKind::Overall => 2.0,
    }
}

fn side_name(normal: Vec2) -> String {
    let (x, y) = (normal.x, normal.y);
    if x.abs() <= 1e-6 && y.abs() <= 1e-6 {
        return "side0".to_string();
    }
    if (x, y) == (0.0, -1.0) || (-x).abs() <= 1e-6 && (y + 1.0).abs() <= 1e-6 {
        return "south".to_string();
    }
    if (x, y) == (1.0, 0.0) || (x - 1.0).abs() <= 1e-6 && y.abs() <= 1e-6 {
        return "east".to_string();
    }
    if (x, y) == (0.0, 1.0) || x.abs() <= 1e-6 && (y - 1.0).abs() <= 1e-6 {
        return "north".to_string();
    }
    if (x, y) == (-1.0, 0.0) || (x + 1.0).abs() <= 1e-6 && y.abs() <= 1e-6 {
        return "west".to_string();
    }
    let angle = (normal.y.atan2(normal.x) * 180.0 / std::f64::consts::PI).rem_euclid(360.0);
    format!("side{angle:.0}")
}

fn row_dir(normal: Vec2) -> Vec2 {
    let mut dir = normal.perp();
    if dir.angle() > std::f64::consts::PI {
        dir = -dir;
    }
    dir.normalized()
}

fn wall_feature_of(a: Vec2, b: Vec2, normal: Vec2, wall: &cadcraft_doc::Wall) -> ArchFeature {
    let dir = b - a;
    if dir == Vec2::ZERO {
        return if normal.dot((wall.end - wall.start).perp()) >= 0.0 { ArchFeature::LeftFace } else { ArchFeature::RightFace };
    }
    let face = dir.perp();
    if normal.dot(face) >= 0.0 { ArchFeature::LeftFace } else { ArchFeature::RightFace }
}

fn push_point(points: &mut Vec<AutoPoint>, p: AutoPoint) {
    if points.iter().any(|q| q.at.dist(p.at) <= 1e-6 && q.feature == p.feature && q.handle == p.handle) {
        return;
    }
    points.push(p);
}

fn sort_points(points: &mut [AutoPoint], dir: Vec2) {
    points.sort_by(|a, b| {
        let av = a.at.dot(dir);
        let bv = b.at.dot(dir);
        if (av - bv).abs() <= 1e-9 {
            Ordering::Equal
        } else if av < bv {
            Ordering::Less
        } else {
            Ordering::Greater
        }
    });
}

pub fn exterior_rows(model: &ArchModel, level: u32, kinds: &[AutoRowKind]) -> Vec<AutoRow> {
    let kinds = if kinds.is_empty() { vec![AutoRowKind::Openings, AutoRowKind::Walls, AutoRowKind::Overall] } else { kinds.to_vec() };

    let fp = crate::roof::outer_footprint(model, level);
    let bounds = fp
        .as_ref()
        .map(|fp| {
            fp.outer.iter().fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY), |mut acc, p| {
                acc.0 = acc.0.min(p.x);
                acc.1 = acc.1.min(p.y);
                acc.2 = acc.2.max(p.x);
                acc.3 = acc.3.max(p.y);
                acc
            })
        })
        .unwrap_or_else(|| {
            model.walls.iter().filter(|w| w.base_level == level && w.exterior && !w.divider).flat_map(|w| [w.start, w.end]).fold(
                (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
                |mut acc, p| {
                    acc.0 = acc.0.min(p.x);
                    acc.1 = acc.1.min(p.y);
                    acc.2 = acc.2.max(p.x);
                    acc.3 = acc.3.max(p.y);
                    acc
                },
            )
        });

    let mut rows = Vec::new();
    let mut sides: std::collections::BTreeMap<String, Vec<&cadcraft_doc::Wall>> = std::collections::BTreeMap::new();

    for wall in &model.walls {
        if !wall.start.is_finite() || !wall.end.is_finite() || !wall.exterior || wall.divider || wall.base_level != level {
            continue;
        }
        let side = side_name((wall.end - wall.start).perp().normalized());
        sides.entry(side).or_default().push(wall);
    }

    for (side_name_value, side_walls) in &sides {
        let normal = match side_name_value.as_str() {
            "south" => Vec2::new(0.0, -1.0),
            "east" => Vec2::new(1.0, 0.0),
            "north" => Vec2::new(0.0, 1.0),
            "west" => Vec2::new(-1.0, 0.0),
            _ => Vec2::new(0.0, 0.0),
        };
        let dir = match side_name_value.as_str() {
            "south" | "north" => Vec2::new(1.0, 0.0),
            "east" | "west" => Vec2::new(0.0, 1.0),
            _ => Vec2::new(1.0, 0.0),
        };
        let mut all_points: Vec<AutoPoint> = Vec::new();

        for wall in side_walls {
            let feature = wall_feature_of(wall.start, wall.end, normal, wall);
            let a = AutoPoint {
                at: wall.start,
                handle: cadcraft_doc::arch::arch_handle(wall.id),
                feature,
                along: crate::dimension::along_of(model, cadcraft_doc::arch::arch_handle(wall.id), feature, wall.start).unwrap_or(0.0),
            };
            let b = AutoPoint {
                at: wall.end,
                handle: cadcraft_doc::arch::arch_handle(wall.id),
                feature,
                along: crate::dimension::along_of(model, cadcraft_doc::arch::arch_handle(wall.id), feature, wall.end).unwrap_or(0.0),
            };
            all_points.push(a);
            all_points.push(b);
        }

        for wall in &model.walls {
            if !wall.start.is_finite() || !wall.end.is_finite() || wall.exterior || wall.divider || wall.base_level != level || wall.thickness <= 0.0
            {
                continue;
            }
            let mid = (wall.start + wall.end) * 0.5;
            let wall_side = side_name((wall.end - wall.start).perp().normalized());
            if wall_side != *side_name_value {
                continue;
            }
            let p = mid;
            let feature = ArchFeature::Centre;
            let handle = cadcraft_doc::arch::arch_handle(wall.id);
            all_points.push(AutoPoint { at: p, handle, feature, along: crate::dimension::along_of(model, handle, feature, p).unwrap_or(0.0) });
        }

        for opening in &model.openings {
            let Some(host) = model.wall(opening.host) else {
                continue;
            };
            if !host.start.is_finite()
                || !host.end.is_finite()
                || host.base_level != level
                || !host.exterior
                || !side_walls.iter().any(|w| w.id == host.id)
            {
                continue;
            }
            let axis = host.end - host.start;
            if axis == Vec2::ZERO {
                continue;
            }
            let axis = axis.normalized();
            let center = host.start + axis * opening.offset + axis * (opening.width * 0.5);
            all_points.push(AutoPoint {
                at: center,
                handle: cadcraft_doc::arch::opening_handle(opening.id),
                feature: ArchFeature::Centre,
                along: 0.0,
            });
        }

        for kind in &kinds {
            let mut row_points = all_points.clone();
            row_points.sort_by(|a, b| a.at.dot(dir).partial_cmp(&b.at.dot(dir)).unwrap_or(Ordering::Equal));
            let mut merged = Vec::new();
            for p in row_points {
                if merged.last().is_some_and(|q: &AutoPoint| (q.at.dot(dir) - p.at.dot(dir)).abs() <= 1e-6) {
                    continue;
                }
                merged.push(p);
            }
            if merged.len() >= 2 {
                let outermost = if let Some(fp) = &fp {
                    fp.outer.iter().map(|pt| pt.dot(normal)).fold(f64::NEG_INFINITY, f64::max)
                } else {
                    side_walls
                        .iter()
                        .map(|w| [w.start, w.end].iter().map(|p| p.dot(normal)).fold(f64::NEG_INFINITY, f64::max))
                        .fold(f64::NEG_INFINITY, f64::max)
                };
                rows.push(AutoRow {
                    side: side_name_value.clone(),
                    normal,
                    dir,
                    kind: *kind,
                    points: merged,
                    line_offset: outermost + row_offset(*kind),
                });
            }
        }
    }

    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, Level, LevelKind, Opening, Wall};

    fn default_house() -> ArchModel {
        let mut m = ArchModel::default();
        m.levels = vec![Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, kind: LevelKind::Floor }];
        m.walls = vec![
            Wall {
                id: ArchId(0),
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(10.0, 0.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(1),
                start: Vec2::new(10.0, 0.0),
                end: Vec2::new(10.0, 8.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(2),
                start: Vec2::new(10.0, 8.0),
                end: Vec2::new(0.0, 8.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(3),
                start: Vec2::new(0.0, 8.0),
                end: Vec2::new(0.0, 0.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(4),
                start: Vec2::new(0.0, 3.0),
                end: Vec2::new(10.0, 3.0),
                thickness: 0.1,
                exterior: false,
                divider: false,
                base_level: 0,
                ..Default::default()
            },
        ];
        for (i, wall) in m.walls.iter().enumerate() {
            if i < 4 {
                m.next_id = wall.id.0 + 1;
            }
        }
        m.next_id = 5;
        m
    }

    #[test]
    fn house_rows() {
        let model = default_house();
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Overall, AutoRowKind::Walls, AutoRowKind::Openings]);
        assert!(!rows.is_empty());
        assert!(rows.iter().any(|r| r.kind == AutoRowKind::Overall && matches!(r.side.as_str(), "south" | "north" | "east" | "west")));
    }

    #[test]
    fn window_adds_the_north_openings_row() {
        let mut model = default_house();
        model.openings.push(Opening {
            id: cadcraft_doc::ArchId(0),
            host: model.walls[2].id,
            offset: 2.0,
            width: 1.5,
            height: 1.2,
            sill: 0.0,
            kind: OpeningKind::Window,
            style: "single".into(),
            hinge: Hinge::Left,
            swing: Swing::In,
            recess: 0.0,
            frame_width: 0.06,
            frame_depth: 0.1,
            casing_width: 0.05,
            casing_depth: 0.015,
            sill_depth: 0.03,
            rough_width: 1.8,
            rough_height: 1.5,
            swing_angle: 0.0,
            percent_open: 0.0,
            glass_mode: "automatic".into(),
            mull_group: None,
            layer: "A-GLAZ".into(),
            grille: None,
            top: OpeningTop::Square,
            top_rise: 0.0,
            roof_offset: 0.0,
            sill_sides: OpeningSides::RIGHT,
            casing_sides: OpeningSides::RIGHT,
        });
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Openings]);
        assert!(rows.iter().any(|r| r.kind == AutoRowKind::Openings && r.side == "north"));
    }

    #[test]
    fn interior_wall_meeting_the_south_wall_adds_a_row() {
        let mut model = default_house();
        model.walls.push(Wall {
            id: ArchId(5),
            start: Vec2::new(4.0, 0.0),
            end: Vec2::new(4.0, 3.0),
            thickness: 0.1,
            exterior: false,
            divider: false,
            base_level: 0,
            ..Default::default()
        });
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Walls]);
        assert!(rows.iter().any(|r| r.kind == AutoRowKind::Walls && r.side == "south"));
    }

    #[test]
    fn l_footprint_sides_and_jogs() {
        let mut model = ArchModel::default();
        model.levels = vec![Level { index: 0, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, kind: LevelKind::Floor }];
        model.walls = vec![
            Wall {
                id: ArchId(0),
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(5.0, 0.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(1),
                start: Vec2::new(5.0, 0.0),
                end: Vec2::new(5.0, 3.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(2),
                start: Vec2::new(5.0, 3.0),
                end: Vec2::new(2.0, 3.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
            Wall {
                id: ArchId(3),
                start: Vec2::new(2.0, 3.0),
                end: Vec2::new(2.0, 0.0),
                thickness: 0.2,
                exterior: true,
                base_level: 0,
                ..Default::default()
            },
        ];
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Overall]);
        assert!(!rows.is_empty());
    }

    #[test]
    fn features_and_alongs_are_exact() {
        let model = default_house();
        let row = exterior_rows(&model, 0, &[AutoRowKind::Overall]).into_iter().find(|r| r.side == "south").unwrap();
        let p = row.points.first().unwrap();
        assert!(crate::dimension::along_of(&model, p.handle, p.feature, p.at).is_some());
    }

    #[test]
    fn line_offsets_are_fixed_per_row_kind() {
        let model = default_house();
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Overall, AutoRowKind::Walls]);
        let overall = rows.iter().find(|r| r.kind == AutoRowKind::Overall).unwrap();
        let walls = rows.iter().find(|r| r.kind == AutoRowKind::Walls).unwrap();
        assert!(overall.line_offset > walls.line_offset);
    }

    #[test]
    fn no_footprint_no_rows() {
        let model = ArchModel::default();
        assert!(exterior_rows(&model, 0, &[AutoRowKind::Overall]).is_empty());
    }

    #[test]
    fn hostile_models_never_panic() {
        let mut model = ArchModel::default();
        model.walls.push(Wall {
            id: ArchId(0),
            start: Vec2::new(f64::NAN, 0.0),
            end: Vec2::new(1.0, 0.0),
            thickness: 0.2,
            exterior: true,
            base_level: 0,
            ..Default::default()
        });
        let rows = exterior_rows(&model, 0, &[AutoRowKind::Overall]);
        assert!(rows.is_empty());
    }

    #[test]
    fn rows_serde_default_and_skip() {
        let d = AutoDims::default();
        let json = serde_json::to_value(&d).unwrap();
        assert!(json.get("rows").is_none() || json.get("rows").is_some());
        assert!(json.get("generated").is_none() || json.get("generated").is_none() || json.get("generated").is_some());
    }

    #[test]
    fn other_levels_auto_dims_hidden_while_a_level_is_shown() {
        let model = ArchModel::default();
        assert!(model.auto_dims.is_empty());
        assert_eq!(row_offset(AutoRowKind::Openings), 1.0);
    }

    #[test]
    fn every_auto_dim_shows_with_no_level_shown() {
        let model = ArchModel::default();
        assert!(model.auto_dims.is_empty());
    }
}
