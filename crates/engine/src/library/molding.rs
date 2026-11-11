//! Resolving a molding profile by item id.

use cadcraft_arch::library::ItemKind;
use cadcraft_arch::molding_profile::{builtin_profile, validate_profile};
use cadcraft_doc::arch::MoldingProfile;

use super::Library;

/// The profile of molding item `item`: a built-in one, else a library item of kind Molding whose
/// `profile` validates.
pub fn profile_of(lib: &Library, item: &str) -> Result<MoldingProfile, String> {
    if let Some(p) = builtin_profile(item) {
        return Ok(p);
    }
    if let Some(r) = lib.item(item)
        && r.kind == ItemKind::Molding
        && let Some(points) = &r.profile
        && validate_profile(points).is_ok()
    {
        return Ok(MoldingProfile { item: item.to_string(), points: points.clone() });
    }
    let shown: String = item.chars().take(64).collect();
    Err(format!("`{shown}` is not a molding profile"))
}
