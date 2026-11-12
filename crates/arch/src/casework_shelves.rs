//! Open-cabinet shelf geometry and counts.

use cadcraft_doc::arch::{Cabinet, Fronts, MAX_SHELVES};

const SHELF_THICKNESS: f64 = 0.019;
const EPS: f64 = 1e-6;

pub fn shelf_count(c: &Cabinet) -> u32 {
    if let Some(n) = c.shelves {
        return n.min(MAX_SHELVES);
    }
    if c.fronts != Fronts::Open {
        return 0;
    }
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let interior = c.height - toe - 2.0 * SHELF_THICKNESS;
    if !interior.is_finite() || interior <= 0.0 {
        return 1;
    }
    let n = (interior / 0.25).floor() as u32;
    n.clamp(1, 6)
}

pub fn shelf_levels(c: &Cabinet) -> Vec<(f64, f64)> {
    if c.fronts != Fronts::Open {
        return Vec::new();
    }
    let n = shelf_count(c);
    if n == 0 {
        return Vec::new();
    }
    let toe = if c.toe_kick { 0.1 } else { 0.0 };
    let interior = c.height - toe - 2.0 * SHELF_THICKNESS;
    let gap = (interior - n as f64 * SHELF_THICKNESS) / (n as f64 + 1.0);
    let base = c.elevation + toe + SHELF_THICKNESS;
    let mut out = Vec::new();
    for k in 1..=n {
        let bottom = base + (k as f64 * gap) + ((k - 1) as f64 * SHELF_THICKNESS);
        let top = bottom + SHELF_THICKNESS;
        out.push((bottom, top));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Cabinet, CabinetKind, Fronts};
    use cadcraft_geom::Vec2;

    fn wall_cabinet(width: f64) -> Cabinet {
        Cabinet {
            id: cadcraft_doc::ArchId(0),
            kind: CabinetKind::Wall,
            level: 0,
            position: Vec2::ZERO,
            rotation: 0.0,
            width,
            depth: 0.305,
            height: 0.762,
            elevation: 1.372,
            fronts: Fronts::Open,
            ..Cabinet::default()
        }
    }

    #[test]
    fn auto_count_floor_interior_over_0_25() {
        let c = wall_cabinet(0.762);
        assert_eq!(shelf_count(&c), 2);
    }

    #[test]
    fn given_shelf_count() {
        let mut c = wall_cabinet(0.762);
        c.shelves = Some(4);
        assert_eq!(shelf_count(&c), 4);
    }

    #[test]
    fn shelf_levels_evenly_spaced() {
        let c = wall_cabinet(0.762);
        let levels = shelf_levels(&c);
        assert_eq!(levels.len(), 2);
        let gaps = [levels[0].1 - levels[0].0, levels[1].1 - levels[1].0];
        assert!((gaps[0] - 0.019).abs() < 1e-9);
        assert!((gaps[1] - 0.019).abs() < 1e-9);
    }

    #[test]
    fn only_open_cabinets_have_shelves() {
        let mut c = wall_cabinet(0.762);
        c.fronts = Fronts::Doors;
        assert_eq!(shelf_count(&c), 0);
        assert!(shelf_levels(&c).is_empty());
    }
}
