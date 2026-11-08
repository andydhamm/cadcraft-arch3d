//! Interior soffits in the plan (phase 16; owner: p16-soffits): each soffit's outline, dashed on
//! its layer (`A-CLNG`), tagged with its handle.

use cadcraft_doc::Drawing;

use crate::DisplayList;

/// Append every interior soffit's dashed plan outline to `list`.
pub(crate) fn push_ceiling(_d: &Drawing, _list: &mut DisplayList, _plotting: bool) {}
