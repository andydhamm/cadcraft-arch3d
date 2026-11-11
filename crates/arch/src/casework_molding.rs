//! Cabinet crown and light-rail runs (phase 18): the path a molding on a wall or tall cabinet
//! follows, along the face (or the box front where there is no door) and back at exposed ends.

use crate::molding_profile::{scaled, validate_profile};
use crate::molding_runs::MoldingRun;
use cadcraft_doc::arch::{ArchModel, Cabinet, CabinetKind, CabinetMolding, FaceKind, Fronts, MoldingKind, MoldingProfile};
use cadcraft_geom::Vec2;

/// How far the box front sits behind the door face line (the cabinet's depth).
const BOX_FRONT_SETBACK: f64 = 0.019;
/// Tolerance of the neighbour test: rotation, back line and gap.
const TOL: f64 = 1e-6;

/// The crown run of `c`, or `None` when it has none (a wall or tall cabinet with a crown that
/// has a valid profile and finite measurements).
pub fn crown_run(model: &ArchModel, c: &Cabinet) -> Option<MoldingRun> {
    molding_run(model, c, true)
}

/// The light-rail run of `c`, or `None` when it has none (see [`crown_run`]).
pub fn light_rail_run(model: &ArchModel, c: &Cabinet) -> Option<MoldingRun> {
    molding_run(model, c, false)
}

fn molding_of(c: &Cabinet, crown: bool) -> Option<&CabinetMolding> {
    if crown { c.crown.as_ref() } else { c.light_rail.as_ref() }
}

fn rotate(p: Vec2, deg: f64) -> Vec2 {
    let a = deg.to_radians();
    Vec2::new(p.x * a.cos() - p.y * a.sin(), p.x * a.sin() + p.y * a.cos())
}

fn level_elevation(model: &ArchModel, level: u32) -> f64 {
    model
        .levels
        .iter()
        .rev()
        .find(|l| l.index == level)
        .map(|l| l.elevation)
        .unwrap_or_else(|| model.levels.last().map(|l| l.elevation).unwrap_or(0.0))
}

fn finite(c: &Cabinet) -> bool {
    [c.position.x, c.position.y, c.rotation, c.width, c.depth, c.height, c.elevation].iter().all(|v| v.is_finite()) && c.width > 0.0 && c.depth > 0.0
}

/// Whether the face line is the doors' plane (`depth`) rather than the box front.
fn fronted(c: &Cabinet) -> bool {
    if c.filler {
        return true;
    }
    if c.fronts == Fronts::Open {
        return false;
    }
    match &c.face {
        Some(f) => f.rows.iter().any(|r| r.kind != FaceKind::Opening),
        None => true,
    }
}

/// Whether `n` continues `c`'s row at the end `sign` (-1 left, +1 right) with the same molding.
fn continues(c: &Cabinet, n: &Cabinet, sign: f64, crown: bool) -> bool {
    if n.id == c.id || n.level != c.level || !matches!(n.kind, CabinetKind::Wall | CabinetKind::Tall) || molding_of(n, crown).is_none() || !finite(n)
    {
        return false;
    }
    let turn = (n.rotation - c.rotation).rem_euclid(360.0);
    if turn > TOL && 360.0 - turn > TOL {
        return false;
    }
    // The neighbour's back centre in c's frame.
    let local = rotate(n.position - c.position, -c.rotation);
    if local.y.abs() > TOL {
        return false;
    }
    let (near, far) = (local.x - n.width / 2.0, local.x + n.width / 2.0);
    // Its edge facing `c` must meet c's end.
    if sign < 0.0 { (far + c.width / 2.0).abs() <= TOL } else { (near - c.width / 2.0).abs() <= TOL }
}

fn exposed(model: &ArchModel, c: &Cabinet, sign: f64, crown: bool) -> bool {
    !model.cabinets.iter().any(|n| continues(c, n, sign, crown))
}

fn molding_run(model: &ArchModel, c: &Cabinet, crown: bool) -> Option<MoldingRun> {
    if !matches!(c.kind, CabinetKind::Wall | CabinetKind::Tall) || !finite(c) {
        return None;
    }
    let m = molding_of(c, crown)?;
    let profile: MoldingProfile = match m.size {
        Some(size) => scaled(&m.profile, size).ok()?,
        None => m.profile.clone(),
    };
    validate_profile(&profile.points).ok()?;
    let h = profile.size()[1];
    let y_f = if fronted(c) { c.depth } else { c.depth - BOX_FRONT_SETBACK };
    let hw = c.width / 2.0;
    let mut local = Vec::with_capacity(4);
    if exposed(model, c, -1.0, crown) {
        local.push(Vec2::new(-hw, 0.0));
    }
    local.push(Vec2::new(-hw, y_f));
    local.push(Vec2::new(hw, y_f));
    if exposed(model, c, 1.0, crown) {
        local.push(Vec2::new(hw, 0.0));
    }
    let path: Vec<Vec2> = local.iter().map(|p| c.position + rotate(*p, c.rotation)).collect();
    let base = level_elevation(model, c.level) + c.elevation;
    let z0 = if crown { base + c.height } else { base - h };
    if !z0.is_finite() || path.iter().any(|p| !p.is_finite()) {
        return None;
    }
    Some(MoldingRun {
        kind: if crown { MoldingKind::Crown } else { MoldingKind::Chair },
        index: 0,
        z: vec![z0; path.len()],
        path,
        profile,
        closed: false,
        side: 1.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::molding_profile::builtin_profile;
    use crate::molding_runs::run_length;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{FaceLayout, FaceRow};

    fn crown() -> Option<CabinetMolding> {
        Some(CabinetMolding { profile: builtin_profile("molding:crown").unwrap_or_default(), size: None })
    }

    fn rail() -> Option<CabinetMolding> {
        Some(CabinetMolding { profile: builtin_profile("molding:casing").unwrap_or_default(), size: None })
    }

    fn wall_cab(id: u32, x: f64) -> Cabinet {
        Cabinet {
            id: ArchId(id),
            kind: CabinetKind::Wall,
            position: Vec2::new(x, 0.0),
            width: 0.6,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            ..Default::default()
        }
    }

    fn model(cabs: Vec<Cabinet>) -> ArchModel {
        ArchModel { cabinets: cabs, ..Default::default() }
    }

    #[test]
    fn lone_wall_cabinet_crown_path() {
        let mut c = wall_cab(0, 0.9);
        c.crown = crown();
        let m = model(vec![c.clone()]);
        let r = crown_run(&m, &c).unwrap();
        assert!((run_length(&r) - 1.21).abs() < 1e-9);
        assert_eq!(r.path.len(), 4);
        assert!((r.z[0] - 2.134).abs() < 1e-9);
        assert_eq!(r.side, 1.0);
        assert!(!r.closed);
        assert!(light_rail_run(&m, &c).is_none());
    }

    #[test]
    fn neighbour_with_crown_drops_the_shared_return() {
        let (mut a, mut b) = (wall_cab(0, 0.9), wall_cab(1, 1.5));
        a.crown = crown();
        let m = model(vec![a.clone(), b.clone()]);
        assert!((run_length(&crown_run(&m, &a).unwrap()) - 1.21).abs() < 1e-9);
        b.crown = crown();
        let m = model(vec![a.clone(), b.clone()]);
        assert!((run_length(&crown_run(&m, &a).unwrap()) - 0.905).abs() < 1e-9);
        assert!((run_length(&crown_run(&m, &b).unwrap()) - 0.905).abs() < 1e-9);
        // A light rail on the neighbour does not count for the crown.
        b.crown = None;
        b.light_rail = rail();
        let m = model(vec![a.clone(), b]);
        assert!((run_length(&crown_run(&m, &a).unwrap()) - 1.21).abs() < 1e-9);
    }

    #[test]
    fn open_cabinet_follows_the_box_front() {
        let mut c = wall_cab(0, 0.0);
        c.crown = crown();
        c.fronts = Fronts::Open;
        let m = model(vec![c.clone()]);
        let r = crown_run(&m, &c).unwrap();
        assert!((r.path[1].y - 0.286).abs() < 1e-9);
        // A face layout of only openings is a box front too; a door row is the face.
        c.fronts = Fronts::Doors;
        c.face = Some(FaceLayout { rows: vec![FaceRow { kind: FaceKind::Opening, ..Default::default() }] });
        assert!((crown_run(&m, &c).unwrap().path[1].y - 0.286).abs() < 1e-9);
        c.face = Some(FaceLayout { rows: vec![FaceRow { kind: FaceKind::Door, ..Default::default() }] });
        assert!((crown_run(&m, &c).unwrap().path[1].y - 0.305).abs() < 1e-9);
        // A filler counts as fronted.
        c.face = None;
        c.fronts = Fronts::Open;
        c.filler = true;
        assert!((crown_run(&m, &c).unwrap().path[1].y - 0.305).abs() < 1e-9);
    }

    #[test]
    fn light_rail_hangs_below() {
        let mut c = wall_cab(0, 0.0);
        c.light_rail = rail();
        let m = model(vec![c.clone()]);
        let r = light_rail_run(&m, &c).unwrap();
        assert!((r.z[0] - (1.372 - 0.064)).abs() < 1e-9);
        assert!(crown_run(&m, &c).is_none());
    }

    #[test]
    fn size_override_scales() {
        let mut c = wall_cab(0, 0.0);
        c.crown = crown();
        if let Some(m) = c.crown.as_mut() {
            m.size = Some([0.05, 0.05]);
        }
        let m = model(vec![c.clone()]);
        let r = crown_run(&m, &c).unwrap();
        let s = r.profile.size();
        assert!((s[0] - 0.05).abs() < 1e-9 && (s[1] - 0.05).abs() < 1e-9);
        // A hostile size gives no run.
        if let Some(m) = c.crown.as_mut() {
            m.size = Some([f64::NAN, 0.05]);
        }
        assert!(crown_run(&model(vec![c.clone()]), &c).is_none());
    }

    #[test]
    fn base_cabinets_get_none() {
        let mut c = wall_cab(0, 0.0);
        c.kind = CabinetKind::Base;
        c.crown = crown();
        c.light_rail = rail();
        let m = model(vec![c.clone()]);
        assert!(crown_run(&m, &c).is_none() && light_rail_run(&m, &c).is_none());
        c.kind = CabinetKind::Tall;
        assert!(crown_run(&model(vec![c.clone()]), &c).is_some());
    }

    #[test]
    fn rotated_cabinet_paths() {
        let mut c = wall_cab(0, 0.0);
        c.rotation = 90.0;
        c.position = Vec2::new(2.0, 1.0);
        c.crown = crown();
        let m = model(vec![c.clone()]);
        let r = crown_run(&m, &c).unwrap();
        // The width axis points +y and the front points -x: the face line is x = 2 - 0.305.
        assert!((r.path[0].x - 2.0).abs() < 1e-9 && (r.path[0].y - 0.7).abs() < 1e-9);
        assert!((r.path[1].x - 1.695).abs() < 1e-9 && (r.path[1].y - 0.7).abs() < 1e-9);
        assert!((r.path[2].y - 1.3).abs() < 1e-9);
        // A neighbour along the rotated row shares the return.
        let mut n = wall_cab(1, 0.0);
        n.rotation = 90.0;
        n.position = Vec2::new(2.0, 1.6);
        n.crown = crown();
        let m = model(vec![c.clone(), n]);
        assert!((run_length(&crown_run(&m, &c).unwrap()) - 0.905).abs() < 1e-9);
    }

    #[test]
    fn hostile_cabinet_gives_none() {
        let mut c = wall_cab(0, 0.0);
        c.crown = crown();
        let edits: [fn(&mut Cabinet); 6] = [
            |c| c.width = f64::NAN,
            |c| c.depth = f64::INFINITY,
            |c| c.height = f64::NAN,
            |c| c.position = Vec2::new(f64::NAN, 0.0),
            |c| c.rotation = f64::NAN,
            |c| c.width = 0.0,
        ];
        for f in edits {
            let mut h = c.clone();
            f(&mut h);
            assert!(crown_run(&model(vec![h.clone()]), &h).is_none());
        }
        let mut bad = c.clone();
        bad.crown = Some(CabinetMolding::default());
        assert!(crown_run(&model(vec![bad.clone()]), &bad).is_none());
    }
}
