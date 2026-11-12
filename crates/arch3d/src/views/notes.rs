//! View notes: attaching them to the generated geometry. Owner: p21-view-notes (stub until then).

use cadcraft_doc::arch::{ViewGeometry, ViewNote};

/// The notes of a view resolved against its freshly generated geometry (positions, detached flags). Owner:
/// p21-view-notes.
pub fn resolve(notes: &[ViewNote], _g: &ViewGeometry) -> Vec<ViewNote> {
    notes.to_vec()
}
