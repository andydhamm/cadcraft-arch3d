//! Fronts of a cabinet with a face layout (phase 18): the face box, row heights, items across a
//! row, and the fronts themselves.

use crate::casework_layout::{FrontInfo, FrontKind, Hinge, hosted_cutout};
use cadcraft_doc::ArchModel;
use cadcraft_doc::arch::{
    Cabinet, CabinetKind, Construction, DOOR_PAIR_WIDTH, FACE_REVEAL, FaceKind, FaceLayout, FaceRow, FrontMount, Fronts, MAX_ACROSS,
    MAX_FACE_APPLIANCES_TALL, MAX_FACE_ROWS,
};

const GAP: f64 = 0.003;
/// Width of a face-frame member (m).
const FRAME: f64 = 0.038;
/// Thickness of the box's front edge (m).
const EDGE: f64 = 0.019;
/// Shortest a row may be (m).
const MIN_ROW: f64 = 0.05;
const EPS: f64 = 1e-9;

/// The rectangle the fronts of a cabinet divide up, in its frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceBox {
    /// Left edge along the width.
    pub u0: f64,
    /// Right edge along the width.
    pub u1: f64,
    /// Bottom, above the level's floor (the cabinet's elevation included).
    pub v0: f64,
    /// Top, above the level's floor.
    pub v1: f64,
    /// The reveal between rows.
    pub separator: f64,
}

/// The face layout a cabinet without an explicit one has, as a layout.
pub fn default_face(c: &Cabinet) -> FaceLayout {
    let across = if c.width.is_finite() && c.width > DOOR_PAIR_WIDTH { c.doors.max(2) } else { c.doors };
    let door = FaceRow { kind: FaceKind::Door, across, ..FaceRow::default() };
    let drawers = c.drawers.max(1) as usize;
    let drawer = |height: Option<f64>| FaceRow { kind: FaceKind::Drawer, height, ..FaceRow::default() };
    let rows = match c.fronts {
        Fronts::Doors => vec![door],
        Fronts::Drawers => (0..drawers).map(|_| drawer(None)).collect(),
        Fronts::DrawerOverDoor => (0..drawers).map(|_| drawer(Some(0.15))).chain(std::iter::once(door)).collect(),
        Fronts::Open => vec![FaceRow { kind: FaceKind::Opening, ..FaceRow::default() }],
    };
    FaceLayout { rows, ..FaceLayout::default() }
}

/// The face box of `c`: what its face layout divides into rows and items.
pub fn face_box(c: &Cabinet) -> FaceBox {
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let (v0, v1) = (c.elevation + toe, c.elevation + c.height);
    let half = c.width / 2.0;
    let (shrink, separator) = match (c.mount, c.construction) {
        (FrontMount::Overlay, _) => (0.0, FACE_REVEAL),
        (FrontMount::Inset, Construction::Frameless) => (EDGE + GAP, FACE_REVEAL),
        (FrontMount::Inset, Construction::Framed) => (FRAME + GAP, FRAME + 2.0 * GAP),
    };
    // An overlay face is full height and inset from the sides by one gap.
    let side = if shrink == 0.0 { GAP } else { shrink };
    FaceBox { u0: -half + side, u1: half - side, v0: v0 + shrink, v1: v1 - shrink, separator }
}

/// The height of every row of `face` on `c`, top to bottom; `None` for an invalid layout.
pub fn row_heights(c: &Cabinet, face: &FaceLayout) -> Option<Vec<f64>> {
    let n = face.rows.len();
    if n == 0 || n > MAX_FACE_ROWS {
        return None;
    }
    let b = face_box(c);
    let h = b.v1 - b.v0;
    if !(h.is_finite() && b.separator.is_finite()) {
        return None;
    }
    let mut fixed = 0.0;
    let mut shared = 0usize;
    for r in &face.rows {
        match r.height {
            Some(v) if v.is_finite() && v >= MIN_ROW => fixed += v,
            Some(_) => return None,
            None => shared += 1,
        }
    }
    let rest = h - fixed - (n as f64 - 1.0) * b.separator;
    if !rest.is_finite() {
        return None;
    }
    let each = if shared == 0 {
        if rest.abs() > EPS {
            return None;
        }
        0.0
    } else {
        if rest < 0.0 {
            return None;
        }
        let each = rest / shared as f64;
        if each < MIN_ROW {
            return None;
        }
        each
    };
    Some(face.rows.iter().map(|r| r.height.unwrap_or(each)).collect())
}

/// How many items stand across `row` on `c`; 0 when the row's `across` is out of range.
pub fn row_across(c: &Cabinet, row: &FaceRow) -> u32 {
    match row.across {
        0 if row.kind == FaceKind::Door => {
            if c.width > DOOR_PAIR_WIDTH + EPS {
                2
            } else {
                1
            }
        }
        0 => 1,
        n if n <= MAX_ACROSS => n,
        _ => 0,
    }
}

/// The fronts of `c` laid out by its face layout, in the order arch3d builds them.
pub fn face_fronts(model: &ArchModel, c: &Cabinet, face: &FaceLayout) -> Vec<FrontInfo> {
    let Some(heights) = row_heights(c, face) else { return Vec::new() };
    let b = face_box(c);
    let mut cutout = hosted_cutout(model, c).is_some();
    let mut out = Vec::new();
    let mut top = b.v1;
    for (row, h) in face.rows.iter().zip(heights) {
        let n = row_across(c, row);
        if n == 0 {
            return Vec::new();
        }
        let (v0, v1) = (top - h, top);
        top = v0 - b.separator;
        let mut kind = match row.kind {
            FaceKind::Door => FrontKind::Door,
            FaceKind::Drawer => FrontKind::Drawer,
            FaceKind::False => FrontKind::False,
            FaceKind::Opening => FrontKind::Opening,
            FaceKind::Appliance => FrontKind::Appliance,
        };
        if kind == FrontKind::Drawer && cutout {
            kind = FrontKind::False;
            cutout = false;
        }
        let w = (b.u1 - b.u0 - (n as f64 - 1.0) * GAP) / n as f64;
        for i in 0..n as usize {
            let x0 = b.u0 + i as f64 * (w + GAP);
            let x1 = x0 + w;
            let hinge = (kind == FrontKind::Door).then(|| if n == 1 || i % 2 == 0 { Hinge::Left } else { Hinge::Right });
            let pull = if c.pull.is_none() {
                None
            } else {
                match kind {
                    FrontKind::False | FrontKind::Opening | FrontKind::Appliance => None,
                    FrontKind::Drawer => Some([(x0 + x1) / 2.0, (v0 + v1) / 2.0]),
                    FrontKind::Door => {
                        let u = if hinge == Some(Hinge::Right) { x0 + 0.05 } else { x1 - 0.05 };
                        let v = if c.kind == CabinetKind::Wall { v0 + 0.075 } else { v1 - 0.075 };
                        Some([u, v])
                    }
                }
            };
            out.push(FrontInfo { kind, rect: [x0, v0, x1, v1], hinge, pull, ..FrontInfo::default() });
        }
    }
    out
}

/// Set `row` to an appliance item in the cabinet face. The appliance occupies the full width of a
/// single front and keeps a shared door or drawer row under it if that was the last shared row left.
pub fn place_appliance(c: &Cabinet, row: usize, item: &str, size: [f64; 3]) -> Result<FaceLayout, String> {
    if !(size.iter().all(|v| v.is_finite() && *v > 0.0)) {
        return Err("an appliance size must be finite and above zero".to_string());
    }
    if c.width < size[0] - 0.01 {
        let kind = match c.kind {
            CabinetKind::Base => "base",
            CabinetKind::Wall => "wall",
            CabinetKind::Tall => "tall",
        };
        return Err(format!("a {} cabinet is narrower than the appliance ({} m vs {} m)", kind, c.width, size[0]));
    }
    let mut face = c.face.clone().unwrap_or_else(|| default_face(c));
    if row >= face.rows.len() {
        return Err(format!("`row` {row} is past the last row ({})", face.rows.len()));
    }
    let max_appliances = match c.kind {
        CabinetKind::Base | CabinetKind::Wall => 1,
        CabinetKind::Tall => MAX_FACE_APPLIANCES_TALL,
    };
    let count = face.rows.iter().filter(|r| r.kind == FaceKind::Appliance).count();
    if count >= max_appliances {
        let kind = match c.kind {
            CabinetKind::Base => "base",
            CabinetKind::Wall => "wall",
            CabinetKind::Tall => "tall",
        };
        return Err(format!("{max_appliances} appliance rows is the limit for a {} cabinet", kind));
    }
    let old = face.rows[row].clone();
    let split = old.kind != FaceKind::Appliance
        && old.height.is_none()
        && face.rows.iter().filter(|r| r.kind != FaceKind::Appliance && r.height.is_none()).count() == 1;
    face.rows[row] = FaceRow { kind: FaceKind::Appliance, height: Some(size[2]), across: 1, appliance: Some(item.to_string()) };
    if split {
        face.rows.insert(row + 1, FaceRow { kind: old.kind, height: None, across: 0, appliance: None });
    }
    if row_heights(c, &face).is_none() {
        return Err("the appliance does not fit the cabinet face".to_string());
    }
    Ok(face)
}

/// Restore a cabinet-face appliance row to its shared door row.
pub fn remove_appliance(c: &Cabinet, row: usize) -> Result<FaceLayout, String> {
    let mut face = c.face.clone().unwrap_or_else(|| default_face(c));
    if row >= face.rows.len() {
        return Err(format!("`row` {row} is past the last row ({})", face.rows.len()));
    }
    let old = face.rows[row].clone();
    if old.kind != FaceKind::Appliance {
        return Err(format!("row {row} is not an appliance row"));
    }
    face.rows[row] = FaceRow { kind: FaceKind::Door, height: None, across: 0, appliance: None };
    if row_heights(c, &face).is_none() {
        return Err("the row does not fit the cabinet face".to_string());
    }
    Ok(face)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Furnishing, FurnishingHost};

    fn base() -> Cabinet {
        Cabinet {
            kind: CabinetKind::Base,
            width: 0.6,
            depth: 0.6,
            height: 0.876,
            fronts: Fronts::Doors,
            doors: 1,
            drawers: 1,
            toe_kick: true,
            ..Default::default()
        }
    }

    fn row(kind: FaceKind, height: Option<f64>, across: u32) -> FaceRow {
        FaceRow { kind, height, across, ..FaceRow::default() }
    }

    #[test]
    fn wall_oven_on_the_upper_door_row_is_0_737() {
        let c = Cabinet { kind: CabinetKind::Tall, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        let face = default_face(&c);
        let put = place_appliance(&c, 0, "appliance:wall-oven", [0.762, 0.6, 0.737]).unwrap();
        let h = row_heights(&c, &put).unwrap();
        assert!((h[0] - 0.737).abs() < 1e-6, "oven row height is 0.737: {h:?}");
        assert_eq!(put.rows[0].kind, FaceKind::Appliance);
        assert_eq!(put.rows[0].appliance.as_deref(), Some("appliance:wall-oven"));
        assert_eq!(put.rows[1].kind, FaceKind::Door);
    }

    #[test]
    fn appliance_on_the_last_shared_row_splits_it() {
        let mut c = Cabinet { kind: CabinetKind::Tall, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        c.face = Some(FaceLayout { rows: vec![row(FaceKind::Appliance, Some(0.737), 1), row(FaceKind::Door, None, 0)], ..FaceLayout::default() });
        let put = place_appliance(&c, 1, "appliance:microwave-drawer", [0.762, 0.6, 0.381]).unwrap();
        assert_eq!(put.rows.len(), 3);
        assert_eq!(put.rows[0].kind, FaceKind::Appliance);
        assert_eq!(put.rows[1].kind, FaceKind::Appliance);
        assert_eq!(put.rows[2].kind, FaceKind::Door);
        assert_eq!(put.rows[1].height, Some(0.381));
    }

    #[test]
    fn tall_takes_two_base_and_wall_one() {
        let c = Cabinet { kind: CabinetKind::Tall, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        assert!(place_appliance(&c, 0, "appliance:wall-oven", [0.762, 0.6, 0.737]).is_ok());
        let c2 = Cabinet { kind: CabinetKind::Base, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        assert!(place_appliance(&c2, 0, "appliance:microwave-drawer", [0.762, 0.6, 0.381]).is_ok());
        let c3 = Cabinet { kind: CabinetKind::Wall, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        assert!(place_appliance(&c3, 0, "appliance:warming-drawer", [0.762, 0.6, 0.254]).is_ok());
    }

    #[test]
    fn narrower_cabinet_refused() {
        let c = Cabinet { kind: CabinetKind::Base, width: 0.6, height: 2.034, fronts: Fronts::Doors, doors: 1, ..Default::default() };
        let err = place_appliance(&c, 0, "appliance:wall-oven", [0.762, 0.6, 0.737]).unwrap_err();
        assert!(err.contains("narrower"));
    }

    #[test]
    fn removing_an_appliance_restores_a_shared_door_row() {
        let mut c = Cabinet { kind: CabinetKind::Base, width: 0.762, height: 2.034, fronts: Fronts::Doors, doors: 2, ..Default::default() };
        let put = place_appliance(&c, 0, "appliance:wall-oven", [0.762, 0.6, 0.737]).unwrap();
        c.face = Some(put.clone());
        let back = remove_appliance(&c, 0).unwrap();
        assert_eq!(back.rows[0].kind, FaceKind::Door);
        assert_eq!(back.rows[0].appliance, None);
        assert!(!row_heights(&c, &back).unwrap().is_empty());
    }

    #[test]
    fn three_equal_drawers_sum_with_reveals() {
        let c = base();
        let face = FaceLayout { rows: vec![row(FaceKind::Drawer, None, 0); 3], ..FaceLayout::default() };
        let h = row_heights(&c, &face).unwrap();
        assert!((h[0] - h[1]).abs() < 1e-9 && (h[1] - h[2]).abs() < 1e-9);
        assert!((h.iter().sum::<f64>() + 2.0 * FACE_REVEAL - 0.776).abs() < 1e-9);
        let b = face_box(&c);
        assert!((b.v1 - b.v0 - 0.776).abs() < 1e-12 && b.separator == FACE_REVEAL);
    }

    #[test]
    fn fixed_rows_then_shared() {
        let c = base();
        let face = FaceLayout {
            rows: vec![row(FaceKind::Drawer, Some(0.15), 0), row(FaceKind::Door, None, 0), row(FaceKind::Drawer, None, 0)],
            ..FaceLayout::default()
        };
        let h = row_heights(&c, &face).unwrap();
        assert_eq!(h[0], 0.15);
        assert!((h[1] - h[2]).abs() < 1e-12);
        assert!((h.iter().sum::<f64>() + 2.0 * FACE_REVEAL - 0.776).abs() < 1e-9);
        // Only fixed rows must fill the face exactly.
        let short = FaceLayout { rows: vec![row(FaceKind::Door, Some(0.5), 0)], ..FaceLayout::default() };
        assert!(row_heights(&c, &short).is_none());
        let exact = FaceLayout { rows: vec![row(FaceKind::Door, Some(0.776), 0)], ..FaceLayout::default() };
        assert!(row_heights(&c, &exact).is_some());
    }

    #[test]
    fn across_auto_pairs_above_0_686() {
        let mut c = base();
        let door = row(FaceKind::Door, None, 0);
        c.width = 0.686;
        assert_eq!(row_across(&c, &door), 1);
        c.width = 0.762;
        assert_eq!(row_across(&c, &door), 2);
        assert_eq!(row_across(&c, &row(FaceKind::Drawer, None, 0)), 1);
        assert_eq!(row_across(&c, &row(FaceKind::Drawer, None, 3)), 3);
        assert_eq!(row_across(&c, &row(FaceKind::Drawer, None, 5)), 0);
    }

    #[test]
    fn forced_single_and_double() {
        let mut c = base();
        c.width = 0.762;
        let m = ArchModel::default();
        let one = FaceLayout { rows: vec![row(FaceKind::Door, None, 1)], ..FaceLayout::default() };
        assert_eq!(face_fronts(&m, &c, &one).len(), 1);
        c.width = 0.6;
        let two = FaceLayout { rows: vec![row(FaceKind::Door, None, 2)], ..FaceLayout::default() };
        let f = face_fronts(&m, &c, &two);
        assert_eq!(f.len(), 2);
        assert_eq!((f[0].hinge, f[1].hinge), (Some(Hinge::Left), Some(Hinge::Right)));
    }

    #[test]
    fn inset_frameless_and_framed_openings() {
        let mut c = base();
        c.mount = FrontMount::Inset;
        let b = face_box(&c);
        assert!((b.v1 - b.v0 - (0.776 - 2.0 * 0.022)).abs() < 1e-12 && b.separator == FACE_REVEAL);
        assert!((b.u0 + 0.3 - 0.022).abs() < 1e-12);
        c.construction = Construction::Framed;
        let b = face_box(&c);
        assert!((b.v1 - b.v0 - 0.694).abs() < 1e-12);
        assert!((b.separator - 0.044).abs() < 1e-12);
        assert!((b.u0 + 0.3 - 0.041).abs() < 1e-12);
        c.mount = FrontMount::Overlay;
        let b = face_box(&c);
        assert!((b.u1 - 0.297).abs() < 1e-12 && b.separator == FACE_REVEAL);
    }

    #[test]
    fn hinge_and_pull_rules_match_the_legacy_layout() {
        let m = ArchModel::default();
        let mut c = base();
        c.width = 0.9;
        c.pull = Some("p".into());
        let f = face_fronts(&m, &c, &default_face(&c));
        assert_eq!(f.len(), 2);
        let [_, _, u1, v1] = f[0].rect;
        assert_eq!(f[0].hinge, Some(Hinge::Left));
        let p = f[0].pull.unwrap();
        assert!((p[0] - (u1 - 0.05)).abs() < 1e-12 && (p[1] - (v1 - 0.075)).abs() < 1e-12);
        let p = f[1].pull.unwrap();
        assert!((p[0] - (f[1].rect[0] + 0.05)).abs() < 1e-12);
        c.kind = CabinetKind::Wall;
        let f = face_fronts(&m, &c, &default_face(&c));
        assert!((f[0].pull.unwrap()[1] - (f[0].rect[1] + 0.075)).abs() < 1e-12);
        c.fronts = Fronts::Drawers;
        c.drawers = 2;
        let f = face_fronts(&m, &c, &default_face(&c));
        let p = f[0].pull.unwrap();
        assert!((p[1] - (f[0].rect[1] + f[0].rect[3]) / 2.0).abs() < 1e-12 && p[0].abs() < 1e-12);
    }

    #[test]
    fn hosted_sink_turns_the_top_drawer_row_false() {
        let mut c = base();
        c.id = ArchId(0);
        c.fronts = Fronts::Drawers;
        c.drawers = 2;
        let m = ArchModel {
            furnishings: vec![Furnishing { cutout: Some([0.4, 0.3]), host: FurnishingHost::Cabinet { cabinet: ArchId(0) }, ..Default::default() }],
            cabinets: vec![c.clone()],
            ..Default::default()
        };
        let f = face_fronts(&m, &c, &default_face(&c));
        assert_eq!((f[0].kind, f[1].kind), (FrontKind::False, FrontKind::Drawer));
    }

    #[test]
    fn default_face_from_legacy_fields() {
        let mut c = base();
        c.doors = 2;
        assert_eq!(default_face(&c).rows, vec![row(FaceKind::Door, None, 2)]);
        c.doors = 0;
        assert_eq!(default_face(&c).rows[0].across, 0);
        c.fronts = Fronts::Drawers;
        c.drawers = 0;
        assert_eq!(default_face(&c).rows.len(), 1);
        c.fronts = Fronts::DrawerOverDoor;
        c.drawers = 2;
        let f = default_face(&c);
        assert_eq!(f.rows.len(), 3);
        assert_eq!((f.rows[0].height, f.rows[2].kind), (Some(0.15), FaceKind::Door));
        c.fronts = Fronts::Open;
        assert_eq!(default_face(&c).rows[0].kind, FaceKind::Opening);
    }

    #[test]
    fn hostile_rows_give_no_fronts() {
        let c = base();
        let m = ArchModel::default();
        let bad = [
            FaceLayout::default(),
            FaceLayout { rows: vec![row(FaceKind::Door, None, 0); 9], ..FaceLayout::default() },
            FaceLayout { rows: vec![row(FaceKind::Door, Some(f64::NAN), 0)], ..FaceLayout::default() },
            FaceLayout { rows: vec![row(FaceKind::Door, Some(0.01), 0)], ..FaceLayout::default() },
            FaceLayout { rows: vec![row(FaceKind::Door, Some(2.0), 0), row(FaceKind::Drawer, None, 0)], ..FaceLayout::default() },
            FaceLayout { rows: vec![row(FaceKind::Door, None, 7)], ..FaceLayout::default() },
            FaceLayout { rows: vec![row(FaceKind::Door, Some(f64::INFINITY), 0)], ..FaceLayout::default() },
        ];
        for f in &bad {
            assert!(face_fronts(&m, &c, f).is_empty());
        }
        let mut c = base();
        c.height = f64::NAN;
        assert!(face_fronts(&m, &c, &default_face(&c)).is_empty());
    }
}
