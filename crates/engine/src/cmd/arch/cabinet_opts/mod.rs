//! Phase-18 `cabinet` / `cabinet.edit` options, one sub-module per group. `parse` runs before the
//! drawing is borrowed; `apply` runs on each cabinet before it is validated.

pub(crate) mod backsplash;
pub(crate) mod corner;
pub(crate) mod fronts;
pub(crate) mod molding;
pub(crate) mod shelves;

use crate::{Result, Session};
use cadcraft_doc::arch::Cabinet;
use serde_json::Value;

/// Every phase-18 option group of one call.
#[derive(Clone, Debug, Default)]
pub(crate) struct CabinetOpts {
    pub fronts: fronts::Opts,
    pub corner: corner::Opts,
    pub shelves: shelves::Opts,
    pub molding: molding::Opts,
    pub backsplash: backsplash::Opts,
}

#[cfg(test)]
thread_local! {
    /// Test probe: how many times `parse` and `apply` ran on this thread.
    pub(crate) static CALLS: std::cell::Cell<(u32, u32)> = const { std::cell::Cell::new((0, 0)) };
}

/// Parse every group's keys of `p`; an error refuses the whole call.
pub(crate) fn parse(cmd: &str, s: &Session, p: &Value) -> Result<CabinetOpts> {
    #[cfg(test)]
    CALLS.with(|c| c.set((c.get().0 + 1, c.get().1)));
    Ok(CabinetOpts {
        fronts: fronts::parse(cmd, s, p)?,
        corner: corner::parse(cmd, s, p)?,
        shelves: shelves::parse(cmd, s, p)?,
        molding: molding::parse(cmd, s, p)?,
        backsplash: backsplash::parse(cmd, s, p)?,
    })
}

impl CabinetOpts {
    /// Apply every group to `c`, in order.
    pub(crate) fn apply(&self, cmd: &str, c: &mut Cabinet) -> Result<()> {
        #[cfg(test)]
        CALLS.with(|k| k.set((k.get().0, k.get().1 + 1)));
        self.fronts.apply(cmd, c)?;
        self.corner.apply(cmd, c)?;
        self.shelves.apply(cmd, c)?;
        self.molding.apply(cmd, c)?;
        self.backsplash.apply(cmd, c)
    }
}
