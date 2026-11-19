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
        return Ok(MoldingProfile { item: item.to_string(), points: points.clone(), ..Default::default() });
    }
    let shown: String = item.chars().take(64).collect();
    Err(format!("`{shown}` is not a molding profile"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::testing::install_io;
    use crate::temp_dir::TempDir;

    #[test]
    fn profile_of_finds_builtin_profiles() {
        install_io();
        let dir = TempDir::new("molding-profile-builtin");
        let lib = Library::with_user_dir(Some(dir.to_path_buf()));
        for (id, _) in cadcraft_arch::molding_profile::BUILTIN {
            let p = profile_of(&lib, id).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert_eq!(p.item, id);
            assert!(!p.points.is_empty());
            assert!(cadcraft_arch::molding_profile::validate_profile(&p.points).is_ok());
        }
    }

    #[test]
    fn profile_of_finds_library_molding_items() {
        install_io();
        let dir = TempDir::new("molding-profile-item");
        let mut lib = Library::with_user_dir(Some(dir.to_path_buf()));
        let profile = cadcraft_arch::molding_profile::builtin_profile("molding:base").unwrap().points;
        let rec = cadcraft_arch::library::ItemRecord {
            id: "user:test-molding".into(),
            name: "Test Molding".into(),
            kind: cadcraft_arch::library::ItemKind::Molding,
            profile: Some(profile.clone()),
            ..Default::default()
        };
        lib.add_user_item(rec);
        let p = profile_of(&lib, "user:test-molding").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(p.item, "user:test-molding");
        assert_eq!(p.points, profile);
    }

    #[test]
    fn profile_of_refuses_non_molding_and_invalid() {
        install_io();
        let dir = TempDir::new("molding-profile-refuse");
        let mut lib = Library::with_user_dir(Some(dir.to_path_buf()));
        // A door is not a molding
        assert!(profile_of(&lib, "door:hinged").is_err());
        // An unknown item
        assert!(profile_of(&lib, "no-such-item").is_err());
        // A library item with no profile
        let rec = cadcraft_arch::library::ItemRecord {
            id: "user:no-profile".into(),
            name: "No Profile".into(),
            kind: cadcraft_arch::library::ItemKind::Molding,
            ..Default::default()
        };
        lib.add_user_item(rec);
        assert!(profile_of(&lib, "user:no-profile").is_err());
        // A library item with an invalid (clockwise) profile
        let rec = cadcraft_arch::library::ItemRecord {
            id: "user:clockwise".into(),
            name: "Clockwise".into(),
            kind: cadcraft_arch::library::ItemKind::Molding,
            profile: Some(vec![[0.0, 0.0], [0.0, 0.07], [0.02, 0.05], [0.02, 0.0]]),
            ..Default::default()
        };
        lib.add_user_item(rec);
        assert!(profile_of(&lib, "user:clockwise").is_err());
    }
}
