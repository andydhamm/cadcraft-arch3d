//! The user catalog's items in the drawing (p14-cmd-user-catalog fills this in).

use cadcraft_arch::library::ItemRecord;
use cadcraft_geom::Vec2;
use serde_json::Value;

use crate::{EngineError, Result, Session};

/// Place the user item `item` (a group of furnishings, cabinets and fixtures) at `at`, turned by
/// `rotation` degrees: `library.place` for an item of kind group.
pub(crate) fn place_group(_s: &mut Session, _item: &ItemRecord, _at: Vec2, _rotation: f64) -> Result<Value> {
    Err(EngineError::Other("user items are placed by p14-cmd-user-catalog".into()))
}
