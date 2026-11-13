//! Openings, doors and windows (phase 12).

use super::Ctx;
use super::guid::global_id;
use crate::step::P;
use cadcraft_arch3d::Mesh;
use cadcraft_doc::arch::{ArchId, OpeningKind};

/// How far (m) the void box overshoots each face of its wall.
const OVERSHOOT: f64 = 0.002;

/// Corner indices of the box's six faces (corner = along + 2·across + 4·up), outward-wound quads.
const FACES: [[u32; 4]; 6] = [[1, 3, 7, 5], [0, 4, 6, 2], [2, 6, 7, 3], [0, 1, 5, 4], [4, 5, 7, 6], [0, 2, 3, 1]];

/// A closed box with 8 corners and 12 triangles: `origin` plus `0..1` of `u`, `v`, `w` (a
/// right-handed frame).
fn box_mesh(object: ArchId, origin: [f64; 3], u: [f64; 3], v: [f64; 3], w: [f64; 3]) -> Mesh {
    let mut mesh = Mesh::new("opening", object);
    for i in 0..8u32 {
        let (a, b, c) = ((i & 1) as f64, ((i >> 1) & 1) as f64, ((i >> 2) & 1) as f64);
        let p = |k: usize| {
            let at = |axis: &[f64; 3]| axis.get(k).copied().unwrap_or(0.0);
            (origin.get(k).copied().unwrap_or(0.0) + a * at(&u) + b * at(&v) + c * at(&w)) as f32
        };
        mesh.positions.push([p(0), p(1), p(2)]);
        mesh.normals.push([0.0, 0.0, 1.0]);
        mesh.uvs.push([0.0, 0.0]);
    }
    for q in FACES {
        mesh.indices.extend([q[0], q[1], q[2], q[0], q[2], q[3]]);
    }
    mesh
}

/// For each opening whose host wall is in `ctx.walls`: an `IfcOpeningElement` (`.OPENING.`,
/// key `opening/<n>`) whose body is the void it cuts through its wall, an `IfcRelVoidsElement`
/// (`rel/voids/<n>`, the wall → the opening element), the `IfcDoor` or `IfcWindow` (key `door/<n>`
/// or `window/<n>`, body from `ctx.pieces.get("opening", id)`, `OverallHeight` and `OverallWidth`
/// from the opening) and an `IfcRelFillsElement` (`rel/fills/<n>`, the opening element → the door or
/// window), all in the host wall's storey.
pub(super) fn emit(ctx: &mut Ctx) {
    let model = ctx.model;
    for o in &model.openings {
        let Some(&wall_ref) = ctx.walls.get(&o.host.0) else { continue };
        let Some(wall) = model.walls.iter().find(|w| w.id == o.host) else { continue };
        let finite = o.width.is_finite() && o.height.is_finite() && o.sill.is_finite() && o.offset.is_finite();
        if !finite || o.width <= 0.0 || o.height <= 0.0 {
            continue;
        }
        let (dx, dy) = (wall.end.x - wall.start.x, wall.end.y - wall.start.y);
        let len = dx.hypot(dy);
        if !len.is_finite() || len <= 0.0 || !wall.thickness.is_finite() {
            continue;
        }
        let n = o.id.0;
        let slot = ctx.storey_for(wall.base_level);
        let (ux, uy) = (dx / len, dy / len);
        let (nx, ny) = (-uy, ux);
        let half = wall.thickness.abs() / 2.0 + OVERSHOOT;
        let a = o.offset - o.width / 2.0;
        let origin = [wall.start.x + ux * a - nx * half, wall.start.y + uy * a - ny * half, o.sill];
        let mesh = box_mesh(o.id, origin, [ux * o.width, uy * o.width, 0.0], [nx * 2.0 * half, ny * 2.0 * half, 0.0], [0.0, 0.0, o.height]);

        let placement = ctx.placement_in_storey(slot);
        let body = ctx.body(&[&mesh], 0.0).map_or(P::Null, P::Ref);
        let (opening_name, predefined) =
            if o.kind == OpeningKind::Niche { (format!("niche-o{n}"), "RECESS") } else { (format!("opening-o{n}"), "OPENING") };
        let void = ctx.w.add(
            "IfcOpeningElement",
            vec![
                P::str(global_id(&format!("opening/{n}"))),
                P::Null,
                P::str(&opening_name),
                P::Null,
                P::Null,
                P::Ref(placement),
                body,
                P::Null,
                P::Enum(predefined),
            ],
        );
        ctx.rel("IfcRelVoidsElement", &format!("rel/voids/{n}"), vec![P::Ref(wall_ref), P::Ref(void)]);

        if o.kind == OpeningKind::Niche {
            continue;
        }

        let meshes = ctx.pieces.get("opening", o.id).to_vec();
        let (entity, key, name, kind) = if o.kind == OpeningKind::Window {
            ("IfcWindow", format!("window/{n}"), format!("window-o{n}"), "WINDOW")
        } else {
            ("IfcDoor", format!("door/{n}"), format!("door-o{n}"), "DOOR")
        };
        let tail = vec![P::Null, P::Real(o.height), P::Real(o.width), P::Enum(kind), P::Enum("NOTDEFINED"), P::Null];
        let filling = ctx.element(entity, &key, &name, slot, &meshes, tail);
        ctx.rel("IfcRelFillsElement", &format!("rel/fills/{n}"), vec![P::Ref(void), P::Ref(filling)]);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{IfcOptions, write_model};
    use cadcraft_arch3d::Scene3d;
    use cadcraft_doc::arch::{ArchId, ArchModel, Opening, OpeningKind, Wall};
    use cadcraft_geom::Vec2;

    fn model(kinds: &[OpeningKind]) -> ArchModel {
        let mut m = ArchModel::default();
        let host = m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(8.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() });
        for (i, kind) in kinds.iter().enumerate() {
            let window = *kind == OpeningKind::Window;
            m.push_opening(Opening {
                host,
                offset: 1.5 + 2.0 * i as f64,
                width: if window { 1.2 } else { 0.9 },
                height: if window { 1.1 } else { 2.1 },
                sill: if window { 0.9 } else { 0.0 },
                kind: *kind,
                ..Opening::default()
            });
        }
        m
    }

    fn export(m: &ArchModel) -> String {
        write_model(m, &Scene3d::build(m), &IfcOptions::default())
    }

    #[test]
    fn openings_void_and_fill_their_walls() {
        let text = export(&model(&[OpeningKind::Door, OpeningKind::Window]));
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 2, "{text}");
        assert_eq!(text.matches("=IFCRELVOIDSELEMENT(").count(), 2);
        assert_eq!(text.matches("=IFCRELFILLSELEMENT(").count(), 2);
        assert!(text.contains(",'opening-o0',$,$,#") && text.contains(",$,.OPENING.);"), "{text}");
        assert_eq!(text.matches("=IFCDOOR(").count(), 1);
        assert_eq!(text.matches("=IFCWINDOW(").count(), 1);
    }

    #[test]
    fn doors_and_windows_carry_their_size() {
        let text = export(&model(&[OpeningKind::Door, OpeningKind::Window, OpeningKind::GarageDoor]));
        assert!(text.contains("'door-o0'") && text.contains("'window-o1'") && text.contains("'door-o2'"), "{text}");
        assert!(text.contains(",$,2.1,0.9,.DOOR.,.NOTDEFINED.,$);"), "{text}");
        assert!(text.contains(",$,1.1,1.2,.WINDOW.,.NOTDEFINED.,$);"), "{text}");
        assert!(!text.contains(".GATE."));
    }

    #[test]
    fn niche_is_a_recess_not_a_door() {
        let mut m = model(&[OpeningKind::Niche]);
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELVOIDSELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCDOOR(").count(), 0, "niche is not a door");
        assert_eq!(text.matches("=IFCWINDOW(").count(), 0, "niche is not a window");
        assert_eq!(text.matches("=IFCRELFILLSELEMENT(").count(), 0, "niche does not fill an opening");
        assert!(text.contains(".RECESS."), "recess predefined type");
    }

    #[test]
    fn an_opening_on_a_missing_wall_is_skipped() {
        let mut m = model(&[OpeningKind::Door]);
        m.push_opening(Opening { host: ArchId(999), ..Opening::default() });
        m.push_opening(Opening { host: ArchId(0), width: f64::NAN, ..Opening::default() });
        m.push_opening(Opening { host: ArchId(0), height: 0.0, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELFILLSELEMENT(").count(), 1);
    }

    #[test]
    fn a_wall_along_y_axis_gets_a_correct_void() {
        let mut m = ArchModel::default();
        let host = m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(0.0, 8.0), thickness: 0.2, height: 2.4, ..Wall::default() });
        m.push_opening(Opening { host, offset: 3.0, width: 1.0, height: 2.1, sill: 0.0, kind: OpeningKind::Door, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELVOIDSELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELFILLSELEMENT(").count(), 1, "{text}");
    }

    #[test]
    fn a_garage_door_is_an_ifc_door() {
        let text = export(&model(&[OpeningKind::GarageDoor]));
        assert_eq!(text.matches("=IFCDOOR(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCWINDOW(").count(), 0, "{text}");
        assert!(text.contains(",$,2.1,0.9,.DOOR.,.NOTDEFINED.,$);"), "{text}");
    }

    #[test]
    fn an_opening_wider_than_its_wall_still_voids() {
        let mut m = ArchModel::default();
        let host = m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(8.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() });
        m.push_opening(Opening { host, offset: 4.0, width: 10.0, height: 2.1, sill: 0.0, kind: OpeningKind::Door, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
        assert_eq!(text.matches("=IFCRELVOIDSELEMENT(").count(), 1, "{text}");
    }

    #[test]
    fn a_negative_sill_is_allowed() {
        let mut m = ArchModel::default();
        let host = m.push_wall(Wall { start: Vec2::new(0.0, 0.0), end: Vec2::new(8.0, 0.0), thickness: 0.2, height: 2.4, ..Wall::default() });
        m.push_opening(Opening { host, offset: 4.0, width: 1.0, height: 1.0, sill: -0.5, kind: OpeningKind::Window, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
        assert!(text.contains(",$,1.,1.,.WINDOW.,.NOTDEFINED.,$);"), "{text}");
    }

    #[test]
    fn a_zero_width_opening_is_skipped() {
        let mut m = model(&[OpeningKind::Door]);
        m.push_opening(Opening { host: ArchId(0), width: 0.0, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
    }

    #[test]
    fn a_nan_offset_is_skipped() {
        let mut m = model(&[OpeningKind::Door]);
        m.push_opening(Opening { host: ArchId(0), offset: f64::NAN, ..Opening::default() });
        let text = export(&m);
        assert_eq!(text.matches("=IFCOPENINGELEMENT(").count(), 1, "{text}");
    }
}
