//! Schedule data and observables for phase 21 (p21-observables-2).
//!
//! The row/column model that `arch.inspect` reports for schedules. `columns`,
//! `default_columns`, `column_title`, `validate_columns` and `material_name` are
//! implemented here; `numbers` and `rows` are stubs owned by p21-schedule-model
//! (the lead's check 01 only needs the empty lists).

use std::collections::BTreeMap;

use cadcraft_doc::arch::{ArchId, ArchKind, ArchModel, Schedule, ScheduleKind};

/// One row of a schedule TABLE.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ScheduleRow {
    /// The schedule number, e.g. `"D01"`.
    pub number: String,
    /// How many members share this number.
    pub count: usize,
    /// The member labels (e.g. `"o0"`, `"o1"`).
    pub members: Vec<String>,
    /// The cell values, one per column.
    pub cells: Vec<String>,
}

/// The schedule number for each member, keyed by (kind, id).
///
/// Stub owned by p21-schedule-model.
pub fn numbers(model: &ArchModel) -> BTreeMap<(ArchKind, ArchId), String> {
    let mut out: BTreeMap<(ArchKind, ArchId), String> = BTreeMap::new();
    out
}

/// The rows of schedule `s`.
///
/// Stub owned by p21-schedule-model.
pub fn rows(_model: &ArchModel, _s: &Schedule) -> Vec<ScheduleRow> {
    Vec::new()
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
}
