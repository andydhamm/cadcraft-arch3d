//! The `decks` rows of `arch.inspect` with `decks: true` (phase 17).
//!
//! One row per Deck room. The frame fields keep their row shape; the frame itself is filled in by
//! the deck-frame work.

use cadcraft_doc::arch::{ArchModel, RoomType, room_label};
use serde_json::{Map as JsonMap, Value, json};

/// Add `decks` to `out` when `p` asks for it. Read-only.
pub(crate) fn inspect_decks(model: &ArchModel, p: &Value, out: &mut JsonMap<String, Value>) {
    if p.get("decks").and_then(Value::as_bool) != Some(true) {
        return;
    }
    let rows: Vec<Value> = model
        .rooms
        .iter()
        .filter(|r| r.room_type == RoomType::Deck)
        .map(|r| {
            let elevation = model.level(r.level).map_or(0.0, |l| l.elevation);
            json!({
                "id": room_label(r.id),
                "level": r.level,
                "top": elevation + r.deck_height.unwrap_or(0.0),
                "boardDirection": r.deck_boards.unwrap_or(0.0),
                "ledger": null,
                "joists": [],
                "rims": [],
                "beams": [],
                "posts": [],
                "postGrades": [],
            })
        })
        .collect();
    out.insert("decks".to_string(), Value::Array(rows));
}
