//! Cabinet option hook (phase 18, owner p18-cabinet-moldings): parsing and applying this group of
//! `cabinet` / `cabinet.edit` keys.

use crate::{Result, Session};
use cadcraft_doc::arch::Cabinet;
use serde_json::Value;

/// The parsed options of this group.
#[derive(Clone, Debug, Default)]
pub(crate) struct Opts {}

/// Parse this group's keys of `p`.
#[allow(unused_variables)]
pub(crate) fn parse(cmd: &str, s: &Session, p: &Value) -> Result<Opts> {
    Ok(Opts::default())
}

impl Opts {
    /// Apply the options to the cabinet being created or edited.
    #[allow(unused_variables)]
    pub(crate) fn apply(&self, cmd: &str, c: &mut Cabinet) -> Result<()> {
        Ok(())
    }
}
