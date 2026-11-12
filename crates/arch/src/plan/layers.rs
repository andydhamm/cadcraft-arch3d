//! The layers a plan can draw a model's objects on. A drawing read from a file (or one made by
//! commands that create no layer) may lack them, and a plan view's layer rules can only hide a
//! layer that exists, so opening a view creates every one of these first.

use cadcraft_doc::arch::{ArchModel, Placement, PlanKind};

use crate::casework::{cabinet_layer, fixture_layer};
use crate::site::north_arrow;

/// Most names returned.
const MAX_NAMES: usize = 4096;

/// Every layer the plan can draw `model`'s objects on, for a plan of `kind`: deduplicated without
/// regard to case (the first spelling is kept), sorted, at most 4096. Empty names are skipped.
pub fn layers_in_use(model: &ArchModel, kind: PlanKind) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        if !name.is_empty() {
            out.push(name.to_string());
        }
    };
    if !model.walls.is_empty() {
        add("A-WALL");
    }
    for o in &model.openings {
        add(&o.layer);
    }
    if !model.rooms.is_empty() {
        add("A-ROOM");
    }
    if !model.roofs.is_empty() {
        add("A-ROOF");
    }
    if !model.stairs.is_empty() {
        add("A-STAIR");
    }
    if !model.railings.is_empty() {
        add("A-RAIL");
    }
    if !model.trays.is_empty() || !model.soffits.is_empty() {
        add("A-CLNG");
    }
    for t in &model.trays {
        add(&t.layer);
    }
    for s in &model.soffits {
        add(&s.layer);
    }
    for f in &model.foundations {
        add(&f.layer);
    }
    if model.terrain.is_some() {
        for n in ["C-TOPO-BNDY", "C-TOPO-MINR", "C-TOPO-MAJR", "C-TOPO-TEXT", "C-TOPO-GRAD", "C-ROAD"] {
            add(n);
        }
    }
    if north_arrow(model).is_some() {
        add("A-ANNO-NRTH");
    }
    if !model.plants.is_empty() {
        add("L-PLNT");
    }
    if !model.lights.is_empty() {
        add("E-LITE");
    }
    for e in &model.electrical {
        add(e.kind.layer());
    }
    for c in &model.cabinets {
        add(cabinet_layer(c.kind));
    }
    for f in &model.fixtures {
        add(fixture_layer(f.kind));
    }
    for v in &model.views {
        add(&v.layer);
    }
    match kind {
        PlanKind::Roof if !model.roofs.is_empty() => {
            add("A-ROOF-OTLN");
            add("A-ROOF-ANNO");
        }
        PlanKind::ReflectedCeiling => {
            let ceiling_items = model.furnishings.iter().any(|f| f.placement == Placement::Ceiling);
            if !model.rooms.is_empty() || !model.trays.is_empty() || !model.soffits.is_empty() || ceiling_items {
                add("A-CLNG");
            }
            if !model.trays.is_empty() {
                add("A-CLNG-ANNO");
            }
        }
        _ => {}
    }
    out.sort_by_key(|n| n.to_ascii_lowercase());
    out.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    out.sort();
    out.truncate(MAX_NAMES);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{Plant, Room, Terrain, Tray, Wall};
    use cadcraft_geom::Vec2;

    fn has(v: &[String], n: &str) -> bool {
        v.iter().any(|x| x == n)
    }

    #[test]
    fn layers_in_use_follows_the_objects_present() {
        let mut m = ArchModel::default();
        assert!(layers_in_use(&m, PlanKind::Floor).is_empty());
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        m.rooms.push(Room::default());
        assert_eq!(layers_in_use(&m, PlanKind::Floor), vec!["A-ROOM".to_string(), "A-WALL".to_string()]);
        m.terrain = Some(Terrain::default());
        let l = layers_in_use(&m, PlanKind::Floor);
        for n in ["C-TOPO-BNDY", "C-TOPO-MINR", "C-TOPO-MAJR", "C-TOPO-TEXT", "C-TOPO-GRAD", "C-ROAD", "A-ANNO-NRTH"] {
            assert!(has(&l, n), "{n} in {l:?}");
        }
        m.plants.push(Plant::default());
        assert!(has(&layers_in_use(&m, PlanKind::Floor), "L-PLNT"));

        let mut m = ArchModel::default();
        m.trays.push(Tray::default());
        assert!(has(&layers_in_use(&m, PlanKind::Floor), "A-CLNG"));
        assert!(!has(&layers_in_use(&m, PlanKind::Floor), "A-CLNG-ANNO"));
        assert!(has(&layers_in_use(&m, PlanKind::ReflectedCeiling), "A-CLNG-ANNO"));

        let mut m = ArchModel::default();
        m.roofs.push(cadcraft_doc::arch::Roof::default());
        let roof = layers_in_use(&m, PlanKind::Roof);
        assert_eq!(roof, vec!["A-ROOF".to_string(), "A-ROOF-ANNO".to_string(), "A-ROOF-OTLN".to_string()]);
        assert_eq!(layers_in_use(&m, PlanKind::Floor), vec!["A-ROOF".to_string()]);

        // Case-insensitive de-duplication keeps the first spelling.
        let mut m = ArchModel::default();
        m.walls.push(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(4.0, 0.0), 0.2));
        m.trays.push(Tray { layer: "a-wall".into(), ..Tray::default() });
        let l = layers_in_use(&m, PlanKind::Floor);
        assert_eq!(l.iter().filter(|n| n.eq_ignore_ascii_case("a-wall")).count(), 1);
    }
}
