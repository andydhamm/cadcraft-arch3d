//! Automatic railings: the railings a stair or a deck should have now, and the regeneration that
//! replaces the host's previous automatic ones. Drawn railings and a stair's well guard are never
//! touched.

use cadcraft_doc::arch::{MAX_RAILINGS, Railing, RailingHost};
use cadcraft_doc::{ArchId, ArchModel};

/// What an automatic railing follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailHostRef {
    /// A stair (the railings on the sides of its flight).
    Stair(ArchId),
    /// A deck room.
    Deck(ArchId),
}

impl RailHostRef {
    fn id(self) -> ArchId {
        match self {
            RailHostRef::Stair(i) | RailHostRef::Deck(i) => i,
        }
    }

    fn matches(self, host: RailingHost) -> bool {
        match self {
            RailHostRef::Stair(_) => matches!(host, RailingHost::StairLeft | RailingHost::StairRight),
            RailHostRef::Deck(_) => host == RailingHost::Deck,
        }
    }
}

/// The railings the host should have now, ids unset.
// STUB (owner p17-railing-auto): returns none until the rules are built.
pub fn auto_railings(_model: &ArchModel, _host: RailHostRef) -> Vec<Railing> {
    Vec::new()
}

/// Remove every automatic railing of the host (`auto`, following the host, of the host's kind;
/// a stair's well guard and drawn railings stay), then add [`auto_railings`] up to
/// `MAX_RAILINGS`. Returns the new ids in order.
pub fn regenerate(model: &mut ArchModel, host: RailHostRef) -> Vec<ArchId> {
    let id = host.id();
    model.railings.retain(|r| !(r.auto && r.follows == Some(id) && host.matches(r.host)));
    let mut ids = Vec::new();
    for r in auto_railings(model, host) {
        if model.railings.len() >= MAX_RAILINGS {
            break;
        }
        ids.push(model.push_railing(r));
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rail(host: RailingHost, follows: u32, auto: bool) -> Railing {
        Railing { host, follows: Some(ArchId(follows)), auto, ..Railing::default() }
    }

    #[test]
    fn regenerate_replaces_only_the_hosts_auto_railings() {
        let mut m = ArchModel::default();
        m.push_railing(rail(RailingHost::StairLeft, 0, true));
        m.push_railing(rail(RailingHost::StairRight, 0, true));
        m.push_railing(rail(RailingHost::Well, 0, false));
        m.push_railing(rail(RailingHost::StairLeft, 1, true));
        m.push_railing(rail(RailingHost::Deck, 0, true));
        m.push_railing(Railing::default());
        // The stub makes none: only stair 0's two automatic railings go.
        let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
        assert!(ids.is_empty());
        assert_eq!(m.railings.len(), 4);
        assert!(m.railings.iter().any(|r| r.host == RailingHost::Well));
        assert!(m.railings.iter().any(|r| r.host == RailingHost::Deck && r.auto));
        assert!(m.railings.iter().any(|r| r.follows == Some(ArchId(1))));
        regenerate(&mut m, RailHostRef::Deck(ArchId(0)));
        assert_eq!(m.railings.len(), 3);
    }

    #[test]
    fn regenerate_respects_max_railings() {
        let mut m = ArchModel::default();
        for _ in 0..MAX_RAILINGS {
            m.push_railing(Railing::default());
        }
        let ids = regenerate(&mut m, RailHostRef::Stair(ArchId(0)));
        assert!(ids.is_empty());
        assert_eq!(m.railings.len(), MAX_RAILINGS);
    }
}
