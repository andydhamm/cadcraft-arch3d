//! Schedule data and observables for phase 21 (p21-observables-2).
//!
//! The row/column model that `arch.inspect` reports for schedules.

use std::collections::{BTreeMap, BTreeSet};

use cadcraft_doc::arch::room_label;
use cadcraft_doc::arch::{
    ArchId, ArchKind, ArchModel, Cabinet, CabinetKind, Finish, Fronts, Level, MaterialRef, Opening, OpeningKind, Paint, Room, Schedule, ScheduleKind,
    Side, Surface, Swing, Wall,
};

/// One row of a schedule TABLE.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleRow {
    pub number: String,
    pub count: usize,
    pub members: Vec<String>,
    pub cells: Vec<String>,
}

fn mm(value: f64) -> Option<i64> {
    if !value.is_finite() || value < 0.0 || value > 1_000_000.0 {
        return None;
    }
    Some((value * 1e6).round() as i64)
}

fn door_kind_name(kind: OpeningKind) -> &'static str {
    match kind {
        OpeningKind::Door => "door",
        OpeningKind::GarageDoor => "garageDoor",
        OpeningKind::CasedOpening => "casedOpening",
        OpeningKind::Window => "window",
        _ => "door",
    }
}

fn cabinet_kind_name(kind: CabinetKind) -> &'static str {
    match kind {
        CabinetKind::Base => "base",
        CabinetKind::Wall => "wall",
        CabinetKind::Tall => "tall",
        _ => "base",
    }
}

fn fronts_name(fronts: Fronts) -> &'static str {
    match fronts {
        Fronts::Doors => "doors",
        Fronts::Drawers => "drawers",
        Fronts::DrawerOverDoor => "drawerOverDoor",
        Fronts::Open => "open",
        _ => "doors",
    }
}

fn swing_name(swing: Swing) -> &'static str {
    match swing {
        Swing::In => "in",
        Swing::Out => "out",
        _ => "in",
    }
}

fn level_name(model: &ArchModel, level: u32) -> String {
    model.levels.iter().find(|l| l.index == level).map_or(level.to_string(), |l| l.name.clone())
}

fn opening_prefix(kind: OpeningKind) -> Option<char> {
    match kind {
        OpeningKind::Door | OpeningKind::GarageDoor | OpeningKind::CasedOpening => Some('D'),
        OpeningKind::Window => Some('W'),
        _ => None,
    }
}

fn opening_identity(item: &Opening) -> Option<String> {
    match item.kind {
        OpeningKind::Door | OpeningKind::GarageDoor | OpeningKind::CasedOpening => {
            let w = mm(item.width)?;
            let h = mm(item.height)?;
            Some(format!("{}|{}|{}|{}", door_kind_name(item.kind), item.style.trim().to_ascii_lowercase(), w, h))
        }
        OpeningKind::Window => {
            let w = mm(item.width)?;
            let h = mm(item.height)?;
            let s = mm(item.sill)?;
            Some(format!("{}|{}|{}|{}", item.style.trim().to_ascii_lowercase(), w, h, s))
        }
        _ => None,
    }
}

fn cabinet_identity(item: &Cabinet) -> Option<String> {
    let width = mm(item.width)?;
    let depth = mm(item.depth)?;
    let height = mm(item.height)?;
    Some(format!("{}|{}|{}|{}|{}|{}|{}", cabinet_kind_name(item.kind), width, depth, height, fronts_name(item.fronts), item.doors, item.drawers,))
}

fn number_key(n: &str) -> u64 {
    let rest = n.strip_prefix('D').or_else(|| n.strip_prefix('W')).or_else(|| n.strip_prefix('C')).unwrap_or(n);
    rest.parse::<u64>().unwrap_or(u64::MAX)
}

/// The schedule number for each member, keyed by (kind, id).
pub fn numbers(model: &ArchModel) -> BTreeMap<(ArchKind, ArchId), String> {
    let mut by_prefix: BTreeMap<char, BTreeMap<String, Vec<ArchId>>> = BTreeMap::new();
    for opening in &model.openings {
        let Some(prefix) = opening_prefix(opening.kind) else { continue };
        let Some(identity) = opening_identity(opening) else { continue };
        by_prefix.entry(prefix).or_default().entry(identity).or_default().push(opening.id);
    }
    for cabinet in &model.cabinets {
        let Some(identity) = cabinet_identity(cabinet) else { continue };
        by_prefix.entry('C').or_default().entry(identity).or_default().push(cabinet.id);
    }

    let mut out = BTreeMap::new();
    for prefix in ['D', 'W', 'C'] {
        let Some(groups) = by_prefix.get(&prefix) else { continue };
        let mut entries: Vec<(ArchId, Vec<ArchId>)> = groups
            .values()
            .map(|ids| {
                let mut ids = ids.clone();
                ids.sort_unstable();
                (ids[0], ids)
            })
            .collect();
        entries.sort_by_key(|(lowest, _)| *lowest);
        for (index, (_, ids)) in entries.into_iter().enumerate() {
            let number = format!("{prefix}{:02}", index + 1);
            let kind = if prefix == 'C' { ArchKind::Cabinet } else { ArchKind::Opening };
            for id in ids {
                out.insert((kind, id), number.clone());
            }
        }
    }
    out
}

/// The rows of schedule `s`.
pub fn rows(model: &ArchModel, s: &Schedule) -> Vec<ScheduleRow> {
    let numbers = numbers(model);
    let mut rows = Vec::new();
    match s.kind {
        ScheduleKind::Door => {
            let mut by_number: BTreeMap<String, Vec<ArchId>> = BTreeMap::new();
            for opening in &model.openings {
                if !matches!(opening.kind, OpeningKind::Door | OpeningKind::GarageDoor | OpeningKind::CasedOpening) {
                    continue;
                }
                let wall = match model.wall(opening.host) {
                    Some(w) => w,
                    None => continue,
                };
                if s.level.is_some_and(|level| wall.base_level != level) {
                    continue;
                }
                if let Some(num) = numbers.get(&(ArchKind::Opening, opening.id)).cloned() {
                    by_number.entry(num).or_default().push(opening.id);
                }
            }
            let mut keys: Vec<String> = by_number.keys().cloned().collect();
            keys.sort_by(|a, b| number_key(a).cmp(&number_key(b)));
            for num in keys {
                let mut members = by_number[&num].clone();
                members.sort_unstable();
                let first = members.first().and_then(|id| model.openings.iter().find(|o| o.id == *id)).cloned();
                let count = members.len();
                let mut cells = Vec::new();
                for col in &s.columns {
                    match col.as_str() {
                        "number" => cells.push(num.clone()),
                        "count" => cells.push(count.to_string()),
                        "width" => cells.push(first.as_ref().map_or(String::new(), |o| format!("{:.3}", o.width))),
                        "height" => cells.push(first.as_ref().map_or(String::new(), |o| format!("{:.3}", o.height))),
                        "style" => cells.push(first.as_ref().map_or(String::new(), |o| o.style.clone())),
                        "kind" => cells.push(first.as_ref().map_or(String::new(), |o| door_kind_name(o.kind).to_string())),
                        "swing" => cells.push(first.as_ref().map_or(String::new(), |o| swing_name(o.swing).to_string())),
                        "level" => {
                            cells.push(first.as_ref().and_then(|o| model.wall(o.host)).map_or_else(String::new, |w| level_name(model, w.base_level)))
                        }
                        _ => cells.push(String::new()),
                    }
                }
                rows.push(ScheduleRow { number: num.clone(), count, members: members.iter().map(|id| format!("o{}", id.0)).collect(), cells });
            }
        }
        ScheduleKind::Window => {
            let mut by_number: BTreeMap<String, Vec<ArchId>> = BTreeMap::new();
            for opening in &model.openings {
                if opening.kind != OpeningKind::Window {
                    continue;
                }
                let wall = match model.wall(opening.host) {
                    Some(w) => w,
                    None => continue,
                };
                if s.level.is_some_and(|level| wall.base_level != level) {
                    continue;
                }
                if let Some(num) = numbers.get(&(ArchKind::Opening, opening.id)).cloned() {
                    by_number.entry(num).or_default().push(opening.id);
                }
            }
            let mut keys: Vec<String> = by_number.keys().cloned().collect();
            keys.sort_by(|a, b| number_key(a).cmp(&number_key(b)));
            for num in keys {
                let mut members = by_number[&num].clone();
                members.sort_unstable();
                let first = members.first().and_then(|id| model.openings.iter().find(|o| o.id == *id)).cloned();
                let count = members.len();
                let mut cells = Vec::new();
                for col in &s.columns {
                    match col.as_str() {
                        "number" => cells.push(num.clone()),
                        "count" => cells.push(count.to_string()),
                        "width" => cells.push(first.as_ref().map_or(String::new(), |o| format!("{:.3}", o.width))),
                        "height" => cells.push(first.as_ref().map_or(String::new(), |o| format!("{:.3}", o.height))),
                        "sill" => cells.push(first.as_ref().map_or(String::new(), |o| format!("{:.3}", o.sill))),
                        "style" => cells.push(first.as_ref().map_or(String::new(), |o| o.style.clone())),
                        "level" => {
                            cells.push(first.as_ref().and_then(|o| model.wall(o.host)).map_or_else(String::new, |w| level_name(model, w.base_level)))
                        }
                        _ => cells.push(String::new()),
                    }
                }
                rows.push(ScheduleRow { number: num.clone(), count, members: members.iter().map(|id| format!("o{}", id.0)).collect(), cells });
            }
        }
        ScheduleKind::Cabinet => {
            let mut by_number: BTreeMap<String, Vec<ArchId>> = BTreeMap::new();
            for cabinet in &model.cabinets {
                if s.level.is_some_and(|level| cabinet.level != level) {
                    continue;
                }
                if let Some(num) = numbers.get(&(ArchKind::Cabinet, cabinet.id)).cloned() {
                    by_number.entry(num).or_default().push(cabinet.id);
                }
            }
            let mut keys: Vec<String> = by_number.keys().cloned().collect();
            keys.sort_by(|a, b| number_key(a).cmp(&number_key(b)));
            for num in keys {
                let mut members = by_number[&num].clone();
                members.sort_unstable();
                let first = members.first().and_then(|id| model.cabinets.iter().find(|c| c.id == *id)).cloned();
                let count = members.len();
                let mut cells = Vec::new();
                for col in &s.columns {
                    match col.as_str() {
                        "number" => cells.push(num.clone()),
                        "count" => cells.push(count.to_string()),
                        "kind" => cells.push(first.as_ref().map_or(String::new(), |c| cabinet_kind_name(c.kind).to_string())),
                        "width" => cells.push(first.as_ref().map_or(String::new(), |c| format!("{:.3}", c.width))),
                        "depth" => cells.push(first.as_ref().map_or(String::new(), |c| format!("{:.3}", c.depth))),
                        "height" => cells.push(first.as_ref().map_or(String::new(), |c| format!("{:.3}", c.height))),
                        "fronts" => cells.push(first.as_ref().map_or(String::new(), |c| fronts_name(c.fronts).to_string())),
                        "doors" => cells.push(first.as_ref().map_or(String::new(), |c| c.doors.to_string())),
                        "drawers" => cells.push(first.as_ref().map_or(String::new(), |c| c.drawers.to_string())),
                        "level" => cells.push(first.as_ref().map_or_else(String::new, |c| level_name(model, c.level))),
                        _ => cells.push(String::new()),
                    }
                }
                rows.push(ScheduleRow { number: num.clone(), count, members: members.iter().map(|id| format!("cb{}", id.0)).collect(), cells });
            }
        }
        ScheduleKind::Room => {
            let surfaces = crate::paint::surfaces(model);
            let mut room_rows = Vec::new();
            for room in &model.rooms {
                if s.level.is_some_and(|level| room.level != level) {
                    continue;
                }
                let mut walls = BTreeSet::new();
                for info in &surfaces {
                    if info.room == Some(room.id) && matches!(info.surface, Surface::WallSide { .. }) {
                        walls.insert(material_name(&info.material));
                    }
                }
                let mut cells = Vec::new();
                for col in &s.columns {
                    match col.as_str() {
                        "name" => cells.push(room.name.clone()),
                        "number" => cells.push(room_label(room.id)),
                        "level" => cells.push(level_name(model, room.level)),
                        "floor" => cells.push(material_name(&crate::paint::effective_material(model, &Surface::RoomFloor(room.id)))),
                        "walls" => cells.push(walls.iter().cloned().collect::<Vec<_>>().join(", ")),
                        "ceiling" => cells.push(material_name(&crate::paint::effective_material(model, &Surface::RoomCeiling(room.id)))),
                        "area" => cells.push(format!("{:.2}", room.interior_area)),
                        "ceilingHeight" => cells.push(crate::plan::ceiling::room_ceiling_text(model, room).unwrap_or_default()),
                        _ => cells.push(String::new()),
                    }
                }
                room_rows.push(ScheduleRow { number: room_label(room.id), count: 1, members: vec![room_label(room.id)], cells });
            }
            room_rows.sort_by_key(|row| row.number.clone());
            rows.extend(room_rows);
        }
    }
    rows.truncate(cadcraft_doc::arch::MAX_SCHEDULE_ROWS);
    rows
}

/// The full column list for a schedule kind.
pub fn columns(kind: ScheduleKind) -> &'static [&'static str] {
    match kind {
        ScheduleKind::Door => &["number", "count", "width", "height", "style", "kind", "swing", "level"],
        ScheduleKind::Window => &["number", "count", "width", "height", "sill", "style", "level"],
        ScheduleKind::Cabinet => &["number", "count", "kind", "width", "depth", "height", "fronts", "doors", "drawers", "level"],
        ScheduleKind::Room => &["name", "number", "level", "floor", "walls", "ceiling", "area", "ceilingHeight"],
    }
}

/// The default (minimum) column list for a schedule kind.
pub fn default_columns(kind: ScheduleKind) -> &'static [&'static str] {
    match kind {
        ScheduleKind::Door => &["number", "count", "width", "height", "style"],
        ScheduleKind::Window => &["number", "count", "width", "height", "sill", "style"],
        ScheduleKind::Cabinet => &["number", "count", "kind", "width", "depth", "height", "fronts"],
        ScheduleKind::Room => &["name", "floor", "walls", "ceiling", "area"],
    }
}

/// The human title for a column key; `""` for an unknown key.
pub fn column_title(key: &str) -> &'static str {
    match key {
        "number" => "No.",
        "count" => "Count",
        "width" => "Width",
        "height" => "Height",
        "sill" => "Sill",
        "style" => "Style",
        "kind" => "Kind",
        "swing" => "Swing",
        "level" => "Level",
        "depth" => "Depth",
        "fronts" => "Fronts",
        "doors" => "Doors",
        "drawers" => "Drawers",
        "name" => "Name",
        "floor" => "Floor",
        "walls" => "Walls",
        "ceiling" => "Ceiling",
        "area" => "Area",
        "ceilingHeight" => "Ceiling Height",
        _ => "",
    }
}

/// Validate a schedule's column list: 1..=16, each allowed for the kind, no duplicates.
/// The error names the bad column.
pub fn validate_columns(kind: ScheduleKind, cols: &[String]) -> Result<(), String> {
    if cols.len() < 1 || cols.len() > 16 {
        return Err(format!("columns must be 1..=16, got {}", cols.len()));
    }
    let allowed = columns(kind);
    let mut seen: Vec<&str> = Vec::with_capacity(cols.len());
    for col in cols {
        if !allowed.contains(&col.as_str()) {
            return Err(format!("column `{}` is not allowed for a {} schedule", col, kind.key()));
        }
        if seen.contains(&col.as_str()) {
            return Err(format!("column `{}` is duplicated", col));
        }
        seen.push(col.as_str());
    }
    Ok(())
}

/// The display name of a material key.
pub fn material_name(key: &str) -> String {
    if let Some(rest) = key.strip_prefix("finish:") {
        return rest.to_string();
    }
    if let Some(rest) = key.strip_prefix("colour:") {
        return rest.to_string();
    }
    if let Some(rest) = key.strip_prefix("item:") {
        return rest.to_string();
    }
    if let Some(rest) = key.strip_prefix("default:") {
        return "default".to_string();
    }
    key.to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::arch::ScheduleKind;

    #[test]
    fn schedule_columns_and_titles() {
        assert_eq!(columns(ScheduleKind::Door), &["number", "count", "width", "height", "style", "kind", "swing", "level"]);
        assert_eq!(default_columns(ScheduleKind::Door), &["number", "count", "width", "height", "style"]);
        assert_eq!(columns(ScheduleKind::Window), &["number", "count", "width", "height", "sill", "style", "level"]);
        assert_eq!(default_columns(ScheduleKind::Window), &["number", "count", "width", "height", "sill", "style"]);
        assert_eq!(columns(ScheduleKind::Cabinet), &["number", "count", "kind", "width", "depth", "height", "fronts", "doors", "drawers", "level",]);
        assert_eq!(default_columns(ScheduleKind::Cabinet), &["number", "count", "kind", "width", "depth", "height", "fronts"]);
        assert_eq!(columns(ScheduleKind::Room), &["name", "number", "level", "floor", "walls", "ceiling", "area", "ceilingHeight"]);
        assert_eq!(default_columns(ScheduleKind::Room), &["name", "floor", "walls", "ceiling", "area"]);

        assert_eq!(column_title("number"), "No.");
        assert_eq!(column_title("count"), "Count");
        assert_eq!(column_title("width"), "Width");
        assert_eq!(column_title("height"), "Height");
        assert_eq!(column_title("sill"), "Sill");
        assert_eq!(column_title("style"), "Style");
        assert_eq!(column_title("kind"), "Kind");
        assert_eq!(column_title("swing"), "Swing");
        assert_eq!(column_title("level"), "Level");
        assert_eq!(column_title("depth"), "Depth");
        assert_eq!(column_title("fronts"), "Fronts");
        assert_eq!(column_title("doors"), "Doors");
        assert_eq!(column_title("drawers"), "Drawers");
        assert_eq!(column_title("name"), "Name");
        assert_eq!(column_title("floor"), "Floor");
        assert_eq!(column_title("walls"), "Walls");
        assert_eq!(column_title("ceiling"), "Ceiling");
        assert_eq!(column_title("area"), "Area");
        assert_eq!(column_title("ceilingHeight"), "Ceiling Height");
        assert_eq!(column_title("bogus"), "");

        // validate_columns: ok lists, empty, unknown column, duplicate, too many.
        assert!(validate_columns(ScheduleKind::Door, &["number".into()]).is_ok());
        assert!(validate_columns(ScheduleKind::Door, &["number".into(), "count".into()]).is_ok());
        assert!(validate_columns(ScheduleKind::Door, &columns(ScheduleKind::Door).iter().map(|c| c.to_string()).collect::<Vec<_>>()).is_ok());
        assert!(validate_columns(ScheduleKind::Door, &[]).is_err());
        assert!(validate_columns(ScheduleKind::Door, &["bogus".into()]).is_err());
        assert!(validate_columns(ScheduleKind::Door, &["number".into(), "number".into()]).is_err());
        // 17 columns (the 10 unique cabinet keys plus 7 repeats) is too many.
        let too_many: Vec<String> = columns(ScheduleKind::Cabinet)
            .iter()
            .chain(columns(ScheduleKind::Cabinet).iter())
            .chain(columns(ScheduleKind::Door).iter())
            .map(|c| c.to_string())
            .take(17)
            .collect();
        assert!(validate_columns(ScheduleKind::Cabinet, &too_many).is_err());
    }

    #[test]
    fn material_names() {
        assert_eq!(material_name("finish:wood"), "wood");
        assert_eq!(material_name("colour:#c04020"), "#c04020");
        assert_eq!(material_name("item:abc"), "abc");
        assert_eq!(material_name("default:wood"), "default");
        assert_eq!(material_name("plain"), "plain");
        assert_eq!(material_name(""), "");
    }

    fn opening_with_id(model: &mut ArchModel, id: u32, host: u32, width: f64, height: f64, sill: f64, kind: OpeningKind, style: &str) -> ArchId {
        let id = ArchId(id);
        model.openings.push(Opening { id, host: ArchId(host), width, height, sill, kind, style: style.to_string(), ..Opening::default() });
        id
    }

    fn cabinet_with_id(
        model: &mut ArchModel,
        id: u32,
        level: u32,
        kind: CabinetKind,
        width: f64,
        depth: f64,
        height: f64,
        fronts: Fronts,
        doors: u32,
        drawers: u32,
    ) -> ArchId {
        let id = ArchId(id);
        model.cabinets.push(Cabinet { id, kind, level, width, depth, height, fronts, doors, drawers, ..Cabinet::default() });
        id
    }

    #[test]
    fn four_and_one_windows_give_w01_x4_and_w02_x1() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        for i in 0..5 {
            let width = if i == 4 { 1.2 } else { 0.9 };
            let height = if i == 4 { 1.2 } else { 1.2 };
            let sill = 0.9;
            opening_with_id(&mut model, i as u32, 1, width, height, sill, OpeningKind::Window, "sliding");
        }
        let n = numbers(&model);
        assert_eq!(n.get(&(ArchKind::Opening, ArchId(0))), Some(&"W01".to_string()));
        assert_eq!(n.get(&(ArchKind::Opening, ArchId(4))), Some(&"W02".to_string()));
        let schedule = Schedule { kind: ScheduleKind::Window, columns: vec!["number".into(), "count".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].number, "W01");
        assert_eq!(rows[0].count, 4);
        assert_eq!(rows[1].number, "W02");
        assert_eq!(rows[1].count, 1);
    }

    #[test]
    fn resizing_one_regroups_three_and_two() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        for i in 0..5 {
            let width = if i < 3 { 0.9 } else { 1.2 };
            opening_with_id(&mut model, i as u32, 1, width, 1.2, 0.9, OpeningKind::Window, "sliding");
        }
        let schedule = Schedule { kind: ScheduleKind::Window, columns: vec!["number".into(), "count".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows.iter().map(|r| (r.number.clone(), r.count)).collect::<Vec<_>>(), vec![("W01".into(), 3), ("W02".into(), 2)]);
    }

    #[test]
    fn groups_number_by_lowest_id() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        opening_with_id(&mut model, 20, 1, 0.9, 1.2, 0.9, OpeningKind::Window, "sliding");
        opening_with_id(&mut model, 1, 1, 0.9, 1.2, 0.9, OpeningKind::Window, "sliding");
        opening_with_id(&mut model, 9, 1, 1.2, 1.2, 0.9, OpeningKind::Window, "sliding");
        let numbers = numbers(&model);
        let matching: Vec<_> = model.openings.iter().map(|o| (o.id.0, numbers.get(&(ArchKind::Opening, o.id)).cloned().unwrap())).collect();
        assert_eq!(matching[0].1, "W01");
        assert_eq!(matching[1].1, "W01");
    }

    #[test]
    fn doors_and_windows_number_separately() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        opening_with_id(&mut model, 0, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        opening_with_id(&mut model, 1, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        opening_with_id(&mut model, 2, 1, 0.9, 1.2, 0.9, OpeningKind::Window, "sliding");
        let numbers = numbers(&model);
        assert_eq!(numbers.get(&(ArchKind::Opening, ArchId(0))), Some(&"D01".to_string()));
        assert_eq!(numbers.get(&(ArchKind::Opening, ArchId(2))), Some(&"W01".to_string()));
    }

    #[test]
    fn garage_doors_and_cased_openings_are_doors() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        opening_with_id(&mut model, 0, 1, 2.4, 2.1, 0.0, OpeningKind::GarageDoor, "garage");
        opening_with_id(&mut model, 1, 1, 0.9, 2.1, 0.0, OpeningKind::CasedOpening, "cased");
        let numbers = numbers(&model);
        assert_eq!(numbers.get(&(ArchKind::Opening, ArchId(0))), Some(&"D01".to_string()));
        assert_eq!(numbers.get(&(ArchKind::Opening, ArchId(1))), Some(&"D02".to_string()));
    }

    #[test]
    fn cabinets_group_by_kind_size_fronts() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        let c0 = cabinet_with_id(&mut model, 0, 0, CabinetKind::Base, 0.6, 0.6, 0.8, Fronts::Doors, 2, 0);
        let _ = cabinet_with_id(&mut model, 1, 0, CabinetKind::Base, 0.6, 0.6, 0.8, Fronts::Doors, 2, 0);
        let c2 = cabinet_with_id(&mut model, 2, 0, CabinetKind::Base, 0.9, 0.6, 0.8, Fronts::Doors, 2, 0);
        let numbers = numbers(&model);
        assert_eq!(numbers.get(&(ArchKind::Cabinet, c0)), Some(&"C01".to_string()));
        assert_eq!(numbers.get(&(ArchKind::Cabinet, c2)), Some(&"C02".to_string()));
    }

    #[test]
    fn rows_follow_the_chosen_columns() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        let o0 = opening_with_id(&mut model, 0, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        let o1 = opening_with_id(&mut model, 1, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        let schedule = Schedule {
            kind: ScheduleKind::Door,
            columns: vec!["number".into(), "count".into(), "width".into(), "height".into()],
            ..Schedule::default()
        };
        let rows = rows(&model, &schedule);
        assert_eq!(rows[0].cells, vec!["D01".to_string(), "2".to_string(), "0.900".to_string(), "2.100".to_string()]);
        assert_eq!(rows[0].members, vec![format!("o{}", o0.0), format!("o{}", o1.0)]);
    }

    #[test]
    fn lengths_have_three_decimals() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        let _ = opening_with_id(&mut model, 0, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        let schedule = Schedule { kind: ScheduleKind::Door, columns: vec!["width".into(), "height".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows[0].cells, vec!["0.900".to_string(), "2.100".to_string()]);
    }

    #[test]
    fn level_filter_keeps_that_level() {
        let mut model = ArchModel::default();
        model.levels =
            vec![Level { index: 0, name: "Ground".into(), ..Level::default() }, Level { index: 1, name: "First".into(), ..Level::default() }];
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        model.walls.push(Wall { id: ArchId(2), base_level: 1, ..Wall::default() });
        opening_with_id(&mut model, 0, 1, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        opening_with_id(&mut model, 1, 2, 0.9, 2.1, 0.0, OpeningKind::Door, "hinged");
        let schedule = Schedule { kind: ScheduleKind::Door, columns: vec!["number".into(), "count".into()], level: Some(1), ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].count, 1);
    }

    #[test]
    fn room_rows_read_effective_materials() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        let mut room =
            Room { id: ArchId(0), name: "Study".into(), level: 0, interior_area: 28.37, floor_finish: Some(Finish::Wood), ..Room::default() };
        room.polygon = vec![
            cadcraft_geom::Vec2::new(-2.0, -2.0),
            cadcraft_geom::Vec2::new(2.0, -2.0),
            cadcraft_geom::Vec2::new(2.0, 2.0),
            cadcraft_geom::Vec2::new(-2.0, 2.0),
        ];
        model.rooms.push(room.clone());
        let wall_id = ArchId(1);
        model.walls.push(Wall {
            id: wall_id,
            start: cadcraft_geom::Vec2::new(-2.0, 0.0),
            end: cadcraft_geom::Vec2::new(2.0, 0.0),
            thickness: 0.2,
            base_level: 0,
            ..Wall::default()
        });
        let surface = Surface::WallSide { wall: wall_id, side: Side::Left };
        model.paints.insert(surface.key(), Paint { material: Some(MaterialRef::Colour([0xc0, 0x40, 0x20])), ..Default::default() });
        let schedule =
            Schedule { kind: ScheduleKind::Room, columns: vec!["name".into(), "floor".into(), "walls".into(), "area".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows[0].cells[0], "Study");
        assert_eq!(rows[0].cells[1], "wood");
        assert!(rows[0].cells[2].contains("#c04020"));
        assert_eq!(rows[0].cells[3], "28.37");
    }

    #[test]
    fn room_rows_area_and_names() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.rooms.push(Room { id: ArchId(2), name: "Master".into(), level: 0, interior_area: 30.5, ..Room::default() });
        let schedule = Schedule { kind: ScheduleKind::Room, columns: vec!["name".into(), "number".into(), "area".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows[0].cells, vec!["Master".to_string(), "r2".to_string(), "30.50".to_string()]);
        assert_eq!(rows[0].number, "r2");
    }

    #[test]
    fn non_finite_items_are_skipped() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        model.walls.push(Wall { id: ArchId(1), base_level: 0, ..Wall::default() });
        opening_with_id(&mut model, 0, 1, f64::NAN, 2.1, 0.0, OpeningKind::Door, "hinged");
        let numbers = numbers(&model);
        assert!(numbers.is_empty());
    }

    #[test]
    fn rows_are_capped() {
        let mut model = ArchModel::default();
        model.levels.push(Level { index: 0, name: "Ground".into(), ..Level::default() });
        for i in 0..(cadcraft_doc::arch::MAX_SCHEDULE_ROWS + 5) {
            model.rooms.push(Room { id: ArchId(i as u32), name: format!("R{i}"), level: 0, interior_area: 10.0, ..Room::default() });
        }
        let schedule = Schedule { kind: ScheduleKind::Room, columns: vec!["name".into()], ..Schedule::default() };
        let rows = rows(&model, &schedule);
        assert_eq!(rows.len(), cadcraft_doc::arch::MAX_SCHEDULE_ROWS);
    }
}
