use cadcraft_arch::library::ItemRecord;
use cadcraft_geom::Vec2;
use serde_json::Value;

use crate::{Result, Session};

pub(crate) fn place_group(_s: &mut Session, _item: &ItemRecord, _at: Vec2, _rotation: f64) -> Result<Value> {
    Err(crate::cmd::bad("library.place", "user items are placed by p14-cmd-user-catalog"))
}
