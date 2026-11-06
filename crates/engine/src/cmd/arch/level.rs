//! LEVEL command: add a level above/below another, or edit a level's heights.
use crate::cmd::{CommandSpec, Session, bad, f64_or};
use cadcraft_doc::arch::{Level, LevelKind};
use serde_json::{Value, json};

fn ensure_ground(s: &mut Session) {
    if s.doc().is_ok_and(|d| d.arch.levels.is_empty())
        && let Ok(d) = s.doc_mut()
    {
        d.arch.ensure_ground_level();
    }
}

fn kind_key(k: LevelKind) -> &'static str {
    match k {
        LevelKind::Floor => "floor",
        LevelKind::Landing => "landing",
    }
}

/// Find a level by name (string) or index (number). Returns the position in the levels vec.
fn find_level(arch: &cadcraft_doc::arch::ArchModel, ref_val: &Value) -> Result<usize, crate::EngineError> {
    if let Some(n) = ref_val.as_str() {
        if let Some(i) = n.parse::<u64>().ok() {
            if i < arch.levels.len() as u64 {
                return Ok(i as usize);
            }
        }
        if let Some((i, _)) = arch.levels.iter().enumerate().find(|(_, l)| l.name.eq_ignore_ascii_case(n)) {
            return Ok(i);
        }
        return Err(crate::EngineError::Other(format!("level: unknown level '{n}'")));
    }
    if let Some(i) = ref_val.as_u64() {
        if i < arch.levels.len() as u64 {
            return Ok(i as usize);
        }
        return Err(crate::EngineError::Other(format!("level: unknown level index {i}")));
    }
    Err(crate::EngineError::Other("level: invalid level reference".into()))
}

/// True if `name` is an auto-generated level name (`Level <n>`, case-insensitive).
fn is_auto_level_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let lower = lower.trim();
    if !lower.starts_with("level") {
        return false;
    }
    let rest = &lower["level".len()..].trim();
    !rest.is_empty() && rest.parse::<i64>().is_ok()
}

/// Derives an auto-generated level name based on a neighbour's name or a fallback index.
///
/// If `neighbour` looks like `Level N`, the new name starts at `Level N+1`.
/// Otherwise it starts at `Level {fallback}`.
/// Collisions with existing names in `levels` are resolved by incrementing the number.
fn auto_level_name(levels: &[cadcraft_doc::arch::Level], neighbour: &str, fallback: usize) -> String {
    let mut n: i64 = if is_auto_level_name(neighbour) {
        let lower = neighbour.to_ascii_lowercase();
        let lower = lower.trim();
        let rest = &lower["level".len()..].trim();
        rest.parse::<i64>().unwrap_or(fallback as i64) + 1
    } else {
        fallback as i64
    };

    let mut name = format!("Level {n}");
    let mut count = 0;
    while levels.iter().any(|l| l.name.eq_ignore_ascii_case(&name)) {
        n += 1;
        name = format!("Level {n}");
        count += 1;
        if count > levels.len() + 2 {
            break;
        }
    }
    name
}

fn parse_level_kind(value: &Value) -> Result<LevelKind, crate::EngineError> {
    match value {
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "floor" => Ok(LevelKind::Floor),
            "landing" => Ok(LevelKind::Landing),
            _ => Err(bad("level", "level: `kind` must be \"floor\" or \"landing\"")),
        },
        _ => Err(bad("level", "level: `kind` must be \"floor\" or \"landing\"")),
    }
}

fn parse_finite_range(value: &Value, min: f64, max: f64, err: &str) -> Result<f64, crate::EngineError> {
    let n = value.as_f64().filter(|v| v.is_finite()).ok_or_else(|| bad("level", err))?;
    if !(min..=max).contains(&n) {
        return Err(bad("level", err));
    }
    Ok(n)
}

fn level_reply(level: &cadcraft_doc::arch::Level) -> Value {
    json!({
        "index": level.index,
        "name": level.name,
        "elevation": level.elevation,
        "kind": kind_key(level.kind),
    })
}

fn validate_duplicate_name(levels: &[cadcraft_doc::arch::Level], name: &str, skip_idx: Option<usize>) -> Result<(), crate::EngineError> {
    if levels.iter().enumerate().any(|(idx, other)| skip_idx != Some(idx) && other.name.eq_ignore_ascii_case(name)) {
        return Err(bad("level", format!("level: level name '{name}' already exists")));
    }
    Ok(())
}

/// Run the `level` command.
///
/// JSON: `{above: "Level 1" | index}` — add a level above.
/// `{below: "Level 1" | index}` — add a level below (renumbers walls).
/// `{index: n, ceiling?, structure?}` or `{name: "Level 1", ceiling?, structure?}` — edit.
pub fn run_level(s: &mut Session, p: &Value) -> Result<Value, crate::EngineError> {
    let above = p.get("above");
    let below = p.get("below");
    let index = p.get("index");
    let name = p.get("name");
    let elevation = p.get("elevation");

    if above.is_none() && below.is_none() && index.is_none() && name.is_none() && elevation.is_none() {
        return Err(crate::EngineError::Other("level: specify above, below, index or name".into()));
    }

    ensure_ground(s);

    if above.is_none() && below.is_none() && elevation.is_some() {
        let doc = s.doc()?;
        let arch = &doc.arch;
        let free_elev =
            elevation.and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| bad("level", "level: `elevation` must be a finite number"))?;
        if let Some(name_val) = name.and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
            validate_duplicate_name(&arch.levels, name_val, None)?;
        }
        if let Some(v) = p.get("ceiling") {
            parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let kind = p.get("kind").map(parse_level_kind).transpose()?.unwrap_or_default();
        let mut new_level = Level {
            index: arch.levels.len() as u32,
            name: name
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| auto_level_name(&arch.levels, "", arch.levels.len() + 1)),
            elevation: free_elev,
            kind,
            ..Default::default()
        };
        if let Some(v) = p.get("ceiling") {
            new_level.ceiling_height = parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            new_level.structure = parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let mut doc = s.doc_mut()?;
        doc.arch.levels.push(new_level);
        doc.touch_arch();
        return Ok(level_reply(doc.arch.levels.last().unwrap()));
    }

    if let Some(above_ref) = above {
        if elevation.is_some() {
            return Err(bad("level", "level: `elevation` cannot be given with `above` or `below`"));
        }
        let doc = s.doc()?;
        let arch = &doc.arch;
        let i = find_level(arch, above_ref)?;
        let base = &arch.levels[i];
        let new_elev = base.elevation + base.floor_to_floor();
        let name_val = name.and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
        if let Some(name_val) = name_val {
            validate_duplicate_name(&arch.levels, name_val, None)?;
        }
        if let Some(v) = p.get("ceiling") {
            parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let kind = p.get("kind").map(parse_level_kind).transpose()?.unwrap_or_default();
        let mut new_level = Level {
            index: arch.levels.len() as u32,
            name: name_val.map(str::to_string).unwrap_or_else(|| auto_level_name(&arch.levels, &base.name, arch.levels.len() + 1)),
            elevation: new_elev,
            kind,
            ..Default::default()
        };
        if let Some(v) = p.get("ceiling") {
            new_level.ceiling_height = parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            new_level.structure = parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let mut doc = s.doc_mut()?;
        doc.arch.levels.push(new_level);
        doc.touch_arch();
        return Ok(level_reply(doc.arch.levels.last().unwrap()));
    }

    if let Some(below_ref) = below {
        if elevation.is_some() {
            return Err(bad("level", "level: `elevation` cannot be given with `above` or `below`"));
        }
        let doc = s.doc()?;
        let arch = &doc.arch;
        let i = find_level(arch, below_ref)?;
        let base = &arch.levels[i];
        let new_elev = base.elevation - base.floor_to_floor();
        let name_val = name.and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
        if let Some(name_val) = name_val {
            validate_duplicate_name(&arch.levels, name_val, None)?;
        }
        if let Some(v) = p.get("ceiling") {
            parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let kind = p.get("kind").map(parse_level_kind).transpose()?.unwrap_or_default();
        let mut new_level = Level {
            index: i as u32,
            name: name_val
                .map(str::to_string)
                .unwrap_or_else(|| if i == 0 { "Basement".to_string() } else { auto_level_name(&arch.levels, &base.name, i + 1) }),
            elevation: new_elev,
            kind,
            ..Default::default()
        };
        if let Some(v) = p.get("ceiling") {
            new_level.ceiling_height = parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
        }
        if let Some(v) = p.get("structure") {
            new_level.structure = parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
        }
        let mut doc = s.doc_mut()?;
        let arch = &mut doc.arch;
        arch.levels.insert(i, new_level);
        for level in arch.levels.iter_mut().skip(i + 1) {
            level.index += 1;
        }
        for wall in arch.walls.iter_mut() {
            if wall.base_level >= i as u32 {
                wall.base_level = wall.base_level.saturating_add(1);
            }
        }
        let reply = level_reply(&doc.arch.levels[i]);
        doc.touch_arch();
        return Ok(reply);
    }

    let doc = s.doc()?;
    let arch = &doc.arch;
    let target_index = if let Some(n) = name {
        find_level(arch, n)?
    } else if let Some(i) = index.and_then(Value::as_u64) {
        if i < arch.levels.len() as u64 {
            i as usize
        } else {
            return Err(crate::EngineError::Other(format!("level: unknown level index {i}")));
        }
    } else {
        return Err(crate::EngineError::Other("level: no level specified".into()));
    };

    if let Some(name_val) = name.and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
        validate_duplicate_name(&arch.levels, name_val, Some(target_index))?;
    }
    if let Some(v) = p.get("ceiling") {
        parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
    }
    if let Some(v) = p.get("structure") {
        parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
    }
    if let Some(v) = p.get("elevation") {
        let _ = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: `elevation` must be a finite number"))?;
    }
    if let Some(v) = p.get("kind") {
        parse_level_kind(v)?;
    }

    let mut doc = s.doc_mut()?;
    let level = &mut doc.arch.levels[target_index];
    if let Some(name_val) = name.and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()) {
        level.name = name_val.to_string();
    }
    if let Some(v) = p.get("elevation") {
        level.elevation = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: `elevation` must be a finite number"))?;
    }
    if let Some(v) = p.get("ceiling") {
        level.ceiling_height = parse_finite_range(v, 1.8, 6.0, "level: ceiling must be between 1.8 and 6 m")?;
    }
    if let Some(v) = p.get("structure") {
        level.structure = parse_finite_range(v, 0.05, 1.5, "level: structure must be between 0.05 and 1.5 m")?;
    }
    if let Some(v) = p.get("kind") {
        level.kind = parse_level_kind(v)?;
    }
    let reply = level_reply(&doc.arch.levels[target_index]);
    doc.touch_arch();
    Ok(reply)
}

/// Command specs for the level module.
pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec::new("level", "Level", run_level)
            .menu(&["Architecture", "Level"])
            .params("{above?: name|index, below?: name|index, index?: n, name?: s, elevation?: m, ceiling?: f, structure?: f, kind?: floor|landing}"),
        CommandSpec::new("level.edit", "Edit Level", run_level_edit)
            .menu(&["Architecture", "Edit Level"])
            .params("{level: name|index, elevation?, ceiling?, structure?, name?, kind?}"),
        CommandSpec::new("level.show", "Show Level", run_level_show).menu(&["Architecture", "Show Level"]).params("{level: name|index|\"all\"}"),
    ]
}

fn run_level_edit(s: &mut Session, p: &Value) -> Result<Value, crate::EngineError> {
    let level_ref = p.get("level").ok_or_else(|| crate::EngineError::Other("level.edit: `level` is required".into()))?;
    ensure_ground(s);

    let doc = s.doc()?;
    let arch = &doc.arch;
    let idx = find_level(arch, level_ref)?;

    let new_name = p.get("name").and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty());
    if let Some(name_val) = new_name {
        if arch.levels.iter().enumerate().any(|(other_idx, other)| other_idx != idx && other.name.eq_ignore_ascii_case(name_val)) {
            return Err(bad("level", format!("level: level name '{name_val}' already exists")));
        }
    }
    if let Some(v) = p.get("elevation") {
        let _ = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: `elevation` must be a finite number"))?;
    }
    if let Some(v) = p.get("ceiling") {
        let c = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: ceiling must be between 1.8 and 6 m"))?;
        if !(1.8..=6.0).contains(&c) {
            return Err(bad("level", "level: ceiling must be between 1.8 and 6 m"));
        }
    }
    if let Some(v) = p.get("structure") {
        let s = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: structure must be between 0.05 and 1.5 m"))?;
        if !(0.05..=1.5).contains(&s) {
            return Err(bad("level", "level: structure must be between 0.05 and 1.5 m"));
        }
    }
    if let Some(v) = p.get("kind") {
        parse_level_kind(v)?;
    }

    let mut doc = s.doc_mut()?;
    let level = &mut doc.arch.levels[idx];
    if let Some(name_val) = new_name {
        level.name = name_val.to_string();
    }
    if let Some(v) = p.get("elevation") {
        level.elevation = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: `elevation` must be a finite number"))?;
    }
    if let Some(v) = p.get("ceiling") {
        let c = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: ceiling must be between 1.8 and 6 m"))?;
        if !(1.8..=6.0).contains(&c) {
            return Err(bad("level", "level: ceiling must be between 1.8 and 6 m"));
        }
        level.ceiling_height = c;
    }
    if let Some(v) = p.get("structure") {
        let s = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad("level", "level: structure must be between 0.05 and 1.5 m"))?;
        if !(0.05..=1.5).contains(&s) {
            return Err(bad("level", "level: structure must be between 0.05 and 1.5 m"));
        }
        level.structure = s;
    }
    if let Some(v) = p.get("kind") {
        level.kind = parse_level_kind(v)?;
    }
    let reply = level_reply(&doc.arch.levels[idx]);
    doc.touch_arch();
    Ok(reply)
}

fn resolve_level_show(arch: &cadcraft_doc::arch::ArchModel, value: &Value) -> Result<Option<usize>, crate::EngineError> {
    match value {
        Value::String(n) if n.eq_ignore_ascii_case("all") => Ok(None),
        Value::String(n) => {
            if let Ok(i) = n.parse::<u64>() {
                if i < arch.levels.len() as u64 {
                    return Ok(Some(i as usize));
                }
                return Err(crate::EngineError::Other(format!("level.show: no level index {i}")));
            }
            arch.levels
                .iter()
                .position(|l| l.name.eq_ignore_ascii_case(n))
                .ok_or_else(|| crate::EngineError::Other(format!("level.show: no level named '{n}'")))
                .map(Some)
        }
        Value::Number(n) => {
            let i = n.as_u64().ok_or_else(|| crate::EngineError::Other("level.show: level index must be a whole number".to_string()))?;
            if i < arch.levels.len() as u64 {
                Ok(Some(i as usize))
            } else {
                Err(crate::EngineError::Other(format!("level.show: no level index {i}")))
            }
        }
        _ => Err(crate::EngineError::Other("level.show: `level` must be a name, index or \"all\"".into())),
    }
}

pub fn run_level_show(s: &mut Session, p: &Value) -> Result<Value, crate::EngineError> {
    let level = p.get("level").ok_or_else(|| crate::EngineError::Other("level.show: `level` is required".to_string()))?;
    let doc = s.doc_mut()?;
    let index = resolve_level_show(&doc.arch, level)?;
    doc.arch.plan_level = index.map(|i| i as u32);
    Ok(serde_json::json!({ "shown": index.map(|i| i as u64) }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use cadcraft_doc::arch::{Level, Wall};
    use cadcraft_geom::Vec2;
    use serde_json::json;

    fn setup_session() -> Session {
        Session::new()
    }

    #[test]
    fn level_above_uses_the_derived_floor_to_floor() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert!((arch.levels[1].elevation - 2.7).abs() < 1e-9);
    }

    #[test]
    fn level_above_takes_the_default_ceiling_and_structure() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let doc = s.doc().unwrap();
        let added = &doc.arch.levels[1];
        assert!((added.ceiling_height - 2.4).abs() < 1e-9);
        assert!((added.structure - 0.3).abs() < 1e-9);
    }

    #[test]
    fn level_below_renumbers_walls() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, ..Default::default() });
            doc.arch.walls.push(Wall {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(10.0, 0.0),
                base_level: 1,
                to_ceiling: true,
                ..Default::default()
            });
        }
        let result = s.execute("level", &json!({"below": "Level 2"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.walls[0].base_level, 2);
    }

    #[test]
    fn level_rejects_an_unknown_neighbour() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"above": "Level 99"}));
        assert!(result.is_err());
    }

    #[test]
    fn level_rejects_a_non_positive_ceiling() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"name": "Level 1", "ceiling": -1.0}));
        assert!(result.is_err());
    }

    #[test]
    fn level_below_uses_the_requested_name() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let _ = s.execute("level", &json!({"below": "Level 1", "name": "Basement"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[0].name, "Basement");
        assert_eq!(arch.levels[1].name, "Level 1");
    }

    #[test]
    fn level_above_names_the_new_level_by_position() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let _ = s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[1].name, "Level 2");
    }

    #[test]
    fn level_above_names_the_new_level_after_a_basement() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        // Add a basement below Level 1
        s.execute("level", &json!({"name": "Basement", "below": "Level 1", "ceiling": 2.4, "structure": 0.3})).unwrap();
        // Add a level above Level 1
        s.execute("level", &json!({"above": "Level 1"})).unwrap();

        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        assert_eq!(arch.levels[0].name, "Basement");
        assert_eq!(arch.levels[1].name, "Level 1");
        assert_eq!(arch.levels[2].name, "Level 2");

        // Check elevations: Basement (-2.7), Level 1 (0.0), Level 2 (2.7)
        assert!((arch.levels[0].elevation - (-2.7)).abs() < 1e-9);
        assert!((arch.levels[1].elevation - 0.0).abs() < 1e-9);
        assert!((arch.levels[2].elevation - 2.7).abs() < 1e-9);
    }

    #[test]
    fn level_below_the_bottom_level_defaults_to_basement() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        // Add a level below Level 1 without specifying a name
        s.execute("level", &json!({"below": "Level 1"})).unwrap();

        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[0].name, "Basement");
        assert_eq!(arch.levels[1].name, "Level 1");
    }

    #[test]
    fn level_auto_names_never_collide() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { name: "Level 2".into(), elevation: 2.7, ..Default::default() });
        }
        // Add a level above Level 1. Auto name would try "Level 2", which collides.
        // It should increment to "Level 3".
        s.execute("level", &json!({"above": "Level 1"})).unwrap();

        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        assert_eq!(arch.levels[0].name, "Level 1");
        assert_eq!(arch.levels[1].name, "Level 2");
        assert_eq!(arch.levels[2].name, "Level 3");
    }

    #[test]
    fn level_auto_name_i32_max_neighbour() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 2147483647".into(), ..Default::default() });
        }
        s.execute("level", &json!({"above": "Level 2147483647"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[1].name, "Level 2147483648");
    }

    #[test]
    fn level_auto_name_non_auto_neighbour() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Ground".into(), ..Default::default() });
        }
        s.execute("level", &json!({"above": "Ground"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        // Neighbour "Ground" is not auto-named, so fallback = levels.len() = 2
        assert_eq!(arch.levels[1].name, "Level 2");
    }

    #[test]
    fn level_auto_name_all_names_collide() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { name: "Level 2".into(), elevation: 2.7, ..Default::default() });
            doc.arch.levels.push(Level { name: "Level 3".into(), elevation: 5.4, ..Default::default() });
        }
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 4);
        assert_eq!(arch.levels[3].name, "Level 4");
    }

    #[test]
    fn level_find_by_name_that_looks_like_index() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { name: "Level 2".into(), elevation: 2.7, ..Default::default() });
        }
        // "1" as a string: try index 1 first (Level 2), not name "1"
        s.execute("level", &json!({"above": "1"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        // Above Level 2 (index 1), so new level has elevation 2.7 + 2.7 = 5.4
        assert!((arch.levels[2].elevation - 5.4).abs() < 1e-9);
    }

    #[test]
    fn level_edit_with_both_index_and_name() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { name: "Level 2".into(), elevation: 2.7, ..Default::default() });
        }
        // Both index 0 and name "Level 2" — name should win
        s.execute("level", &json!({"index": 0, "name": "Level 2", "ceiling": 3.0})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        // Level 2 (index 1) should have ceiling 3.0, Level 1 should be unchanged
        assert!((arch.levels[0].ceiling_height - 2.4).abs() < 1e-9);
        assert!((arch.levels[1].ceiling_height - 3.0).abs() < 1e-9);
    }

    #[test]
    fn level_rejects_nan_ceiling() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from("Level 1"));
        map.insert("ceiling".to_string(), Value::from(f64::NAN));
        let result = s.execute("level", &Value::Object(map));
        assert!(result.is_err());
    }

    #[test]
    fn is_auto_level_name_edge_cases() {
        assert!(!is_auto_level_name(""));
        assert!(!is_auto_level_name("Level"));
        assert!(is_auto_level_name("Level 0"));
        assert!(is_auto_level_name("Level 1"));
        assert!(is_auto_level_name("LEVEL 1"));
        assert!(is_auto_level_name("Level  1"));
        assert!(is_auto_level_name("Level -1"));
        assert!(!is_auto_level_name("Level 1.5"));
        assert!(!is_auto_level_name("Level 1abc"));
        assert!(!is_auto_level_name("Floor 1"));
    }

    #[test]
    fn level_above_uses_the_requested_name() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let _ = s.execute("level", &json!({"above": "Level 1", "name": "Second Floor"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[1].name, "Second Floor");
    }

    #[test]
    fn level_edit_rejects_nan_structure() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from("Level 1"));
        map.insert("structure".to_string(), Value::from(f64::NAN));
        let result = s.execute("level", &Value::Object(map));
        assert!(result.is_err());
    }

    #[test]
    fn level_edit_rejects_non_positive_structure() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"name": "Level 1", "structure": 0.0}));
        assert!(result.is_err());
    }

    #[test]
    fn level_below_with_explicit_ceiling_and_structure() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let _ = s.execute("level", &json!({"below": "Level 1", "ceiling": 2.0, "structure": 0.2})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert!((arch.levels[0].ceiling_height - 2.0).abs() < 1e-9);
        assert!((arch.levels[0].structure - 0.2).abs() < 1e-9);
    }

    #[test]
    fn level_no_args_errors() {
        let mut s = setup_session();
        let result = s.execute("level", &json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn level_edit_unknown_index_errors() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"index": 99, "ceiling": 3.0}));
        assert!(result.is_err());
    }

    #[test]
    fn level_edit_unknown_name_errors() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let result = s.execute("level", &json!({"name": "Level 99", "ceiling": 3.0}));
        assert!(result.is_err());
    }

    #[test]
    fn level_above_by_index() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, ..Default::default() });
        }
        s.execute("level", &json!({"above": 0})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        // Above Level 1 (index 0), so elevation = 0.0 + 2.7 = 2.7
        assert!((arch.levels[1].elevation - 2.7).abs() < 1e-9);
    }

    #[test]
    fn level_below_by_index() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, ..Default::default() });
        }
        s.execute("level", &json!({"below": 1})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        // Below Level 2 (index 1), so elevation = 2.7 - 2.7 = 0.0
        assert!((arch.levels[1].elevation - 0.0).abs() < 1e-9);
    }

    #[test]
    fn level_rejects_infinite_ceiling() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from("Level 1"));
        map.insert("ceiling".to_string(), Value::from(f64::INFINITY));
        let result = s.execute("level", &Value::Object(map));
        assert!(result.is_err());
    }

    #[test]
    fn level_rejects_infinite_structure() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::from("Level 1"));
        map.insert("structure".to_string(), Value::from(f64::INFINITY));
        let result = s.execute("level", &Value::Object(map));
        assert!(result.is_err());
    }

    #[test]
    fn level_edit_no_changes_succeeds() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        // Edit with just the name, no ceiling or structure — should succeed as a no-op
        let result = s.execute("level", &json!({"name": "Level 1"}));
        assert!(result.is_ok());
    }

    #[test]
    fn level_below_multiple_times() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
        }
        s.execute("level", &json!({"below": 0})).unwrap();
        s.execute("level", &json!({"below": 0})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 3);
        // Elevations: -5.4, -2.7, 0.0
        assert!((arch.levels[0].elevation - (-5.4)).abs() < 1e-9);
        assert!((arch.levels[1].elevation - (-2.7)).abs() < 1e-9);
        assert!((arch.levels[2].elevation - 0.0).abs() < 1e-9);
    }

    #[test]
    fn level_above_custom_height_level() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            // Level with custom ceiling 3.0 and structure 0.5 -> floor_to_floor = 3.5
            doc.arch.levels.push(Level { name: "Level 1".into(), ceiling_height: 3.0, structure: 0.5, ..Default::default() });
        }
        s.execute("level", &json!({"above": "Level 1"})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        // New level elevation should be 0.0 + 3.5 = 3.5
        assert!((arch.levels[1].elevation - 3.5).abs() < 1e-9);
    }

    #[test]
    fn level_below_renumbers_multiple_walls() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Level 2".into(), elevation: 2.7, ..Default::default() });
            // Wall on level 0
            doc.arch.walls.push(Wall {
                start: Vec2::new(0.0, 0.0),
                end: Vec2::new(10.0, 0.0),
                base_level: 0,
                to_ceiling: true,
                ..Default::default()
            });
            // Wall on level 1
            doc.arch.walls.push(Wall {
                start: Vec2::new(0.0, 10.0),
                end: Vec2::new(10.0, 10.0),
                base_level: 1,
                to_ceiling: true,
                ..Default::default()
            });
        }
        s.execute("level", &json!({"below": 0})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        // Both walls should be incremented
        assert_eq!(arch.walls[0].base_level, 1);
        assert_eq!(arch.walls[1].base_level, 2);
    }

    #[test]
    fn level_above_empty_levels_errors() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
        }
        let result = s.execute("level", &json!({"above": 0}));
        assert!(result.is_ok());
        assert_eq!(s.doc().unwrap().arch.levels.len(), 2);
    }

    #[test]
    fn level_below_empty_levels_errors() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
        }
        let result = s.execute("level", &json!({"below": 0}));
        assert!(result.is_ok());
        assert_eq!(s.doc().unwrap().arch.levels.len(), 2);
    }

    #[test]
    fn level_show_sets_the_plan_level() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Upper".into(), elevation: 2.7, ..Default::default() });
        }

        let result = s.execute("level.show", &json!({ "level": "Upper" })).unwrap();
        assert_eq!(result["shown"], 1);
        assert_eq!(s.doc().unwrap().arch.plan_level, Some(1));

        s.execute("level.show", &json!({ "level": "all" })).unwrap();
        assert_eq!(s.doc().unwrap().arch.plan_level, None);
    }

    #[test]
    fn level_takes_a_free_elevation() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ..Default::default() });
        }

        let result = s.execute("level", &json!({"name":"Entry","elevation":1.35,"kind":"landing"})).unwrap();
        assert_eq!(result["index"], 1);
        assert_eq!(result["name"], "Entry");
        assert_eq!(result["elevation"], 1.35);
        assert_eq!(result["kind"], "landing");

        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[1].index, 1);
        assert_eq!(arch.levels[1].name, "Entry");
        assert!((arch.levels[1].elevation - 1.35).abs() < 1e-9);
        assert_eq!(arch.levels[1].kind, cadcraft_doc::arch::LevelKind::Landing);

        s.execute("level", &json!({"name":"Entry 2","elevation":1.35})).unwrap();
        let err = s.execute("level", &json!({"name":"Entry","elevation":1.35}));
        assert!(err.is_err());
    }

    #[test]
    fn level_edit_moves_a_level() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Entry".into(), elevation: 1.35, ..Default::default() });
        }

        s.execute("level.edit", &json!({"level":"Entry","elevation":1.4})).unwrap();
        let doc = s.doc().unwrap();
        assert!((doc.arch.levels[1].elevation - 1.4).abs() < 1e-9);

        s.execute("level.edit", &json!({"level":0,"name":"Lower"})).unwrap();
        let doc = s.doc().unwrap();
        assert_eq!(doc.arch.levels[0].name, "Lower");

        s.execute("level.edit", &json!({"level":"Lower","name":"Lower"})).unwrap();
        let err = s.execute("level.edit", &json!({"level":"Lower","name":"Entry"}));
        assert!(err.is_err());
    }

    #[test]
    fn a_new_drawing_has_level_zero() {
        let mut s = Session::new();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
        }
        assert!(s.doc().unwrap().arch.levels.is_empty());
        s.execute("level", &json!({"name":"Main","elevation":2.7})).unwrap();
        let doc = s.doc().unwrap();
        let arch = &doc.arch;
        assert_eq!(arch.levels.len(), 2);
        assert_eq!(arch.levels[0].index, 0);
        assert_eq!(arch.levels[0].name, "Level 1");
        assert!((arch.levels[0].elevation - 0.0).abs() < 1e-9);
        assert_eq!(arch.levels[1].index, 1);
        assert_eq!(arch.levels[1].name, "Main");
        assert!((arch.levels[1].elevation - 2.7).abs() < 1e-9);
    }

    #[test]
    fn level_edit_refuses_bad_values_without_an_undo_step() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ..Default::default() });
        }

        for params in [
            json!({"level":9,"elevation":1.0}),
            json!({"level":0,"ceiling":-2.0}),
            json!({"level":0,"ceiling":7.0}),
            json!({"level":0,"structure":0.0}),
            json!({"level":0,"kind":"attic"}),
            json!({"level":0,"elevation":"1.5"}),
        ] {
            let before = s.state().unwrap().undo.len();
            let result = s.execute("level.edit", &params);
            assert!(result.is_err());
            assert_eq!(s.state().unwrap().undo.len(), before);
        }
    }

    #[test]
    fn level_free_elevation_is_refused_with_above() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), elevation: 0.0, ..Default::default() });
        }

        let result = s.execute("level", &json!({"above":0,"elevation":5.0}));
        assert!(result.is_err());
        let doc = s.doc().unwrap();
        assert_eq!(doc.arch.levels.len(), 1);
    }

    #[test]
    fn a_missing_level_is_refused() {
        let mut s = setup_session();
        {
            let doc = s.doc_mut().unwrap();
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Level 1".into(), ..Default::default() });
        }

        let result = s.execute("level.show", &json!({ "level": "Attic" }));
        assert!(result.is_err());
    }
}
