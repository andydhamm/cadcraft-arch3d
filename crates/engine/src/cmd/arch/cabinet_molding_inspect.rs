use cadcraft_arch::casework_molding::{crown_run, light_rail_run};
use cadcraft_arch::molding_runs::run_length;
use cadcraft_doc::arch::{ArchModel, CabinetMolding};
use serde_json::{Map as JsonMap, Value, json};

pub(crate) fn inspect_cabinet_moldings(model: &ArchModel, out: &mut JsonMap<String, Value>) {
    let Some(Value::Array(rows)) = out.get_mut("cabinets") else { return };
    for (row, c) in rows.iter_mut().zip(model.cabinets.iter()) {
        let Some(obj) = row.as_object_mut() else { continue };
        obj.insert("crown".to_string(), serialize_molding(c.crown.as_ref(), crown_run(model, c).as_ref()));
        obj.insert("lightRail".to_string(), serialize_molding(c.light_rail.as_ref(), light_rail_run(model, c).as_ref()));
    }
}

fn serialize_molding(m: Option<&CabinetMolding>, run: Option<&cadcraft_arch::molding_runs::MoldingRun>) -> Value {
    let Some(m) = m else { return Value::Null };
    let Some(run) = run else { return Value::Null };
    let size = match m.size {
        Some([w, h]) => vec![w, h],
        None => {
            let [w, h] = run.profile.size();
            vec![w, h]
        }
    };
    let path: Vec<Value> = run.path.iter().map(|p| json!([p.x, p.y])).collect();
    json!({
        "profile": m.profile.item,
        "size": size,
        "length": run_length(run),
        "z": run.z.first().copied().unwrap_or(0.0),
        "path": path,
    })
}
