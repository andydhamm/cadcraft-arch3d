//! Architecture command group.
use crate::cmd::CommandSpec;

pub use cadcraft_doc::arch::{MAX_WALLS, MIN_WALL_LENGTH, MIN_WALL_THICKNESS};

/// Default exterior wall thickness (m).
pub const DEFAULT_EXT_THICKNESS: f64 = 0.2;

/// Default interior wall thickness (m).
pub const DEFAULT_INT_THICKNESS: f64 = 0.1;

/// Default wall height when the model has no level (the derived floor-to-floor of the default level).
pub const DEFAULT_WALL_HEIGHT: f64 = 2.7;

pub mod archdetect;
pub mod backdrop;
pub mod build3d;
pub(crate) mod cabinet_opts;
pub mod casework;
pub mod connections;
mod countertop_inspect;
pub mod deck;
mod deck_inspect;
pub(crate) mod defaults;
pub mod electrical;
pub(crate) mod electrical_inspect;
pub mod electrical_view;
pub mod foundation;
pub mod furnish;
pub mod illuminance;
pub mod inspect;
mod inspect_detail;
#[cfg(test)]
mod inspect_detail_tests;
pub mod level;
pub mod library;
pub mod library_icon;
pub mod library_user;
pub(crate) mod light_inspect;
pub mod lightfixture;
pub mod lightset;
pub mod look;
pub mod material;
pub mod opening;
pub mod outlets_auto;
pub mod paint;
pub mod pick;
pub mod plant;
pub mod railing;
pub mod render;
pub mod roofauto;
pub mod roofplane;
pub mod room;
pub mod roomdetect;
pub mod sample;
pub mod sheet;
pub mod site;
pub(crate) mod site_inspect;
pub mod soffit;
pub mod stair;
pub mod stretch;
pub mod sun;
pub mod sunstudy;
pub mod terrain;
pub mod terrain_modify;
pub mod terrain_query;
pub mod tray;
pub(crate) mod units;
pub mod view;
pub mod views;
pub mod wall;

/// Default layered wall band thicknesses for the given wall side.
///
/// The values are the *natural* band thicknesses in drawing units:
/// exterior = siding 0.02, sheathing 0.015, stud 0.09, drywall 0.013 (sum 0.138)
/// interior = drywall 0.013, stud 0.09, drywall 0.013 (sum 0.116)
///
/// The caller must scale these proportionally so the bands sum exactly to the
/// requested wall thickness (see `crates/arch/src/wall.rs::wall_layer_polygons`).
pub(crate) fn wall_layers(exterior: bool) -> Vec<(String, f64)> {
    if exterior {
        vec![("siding".to_string(), 0.02), ("sheathing".to_string(), 0.015), ("stud".to_string(), 0.09), ("drywall".to_string(), 0.013)]
    } else {
        vec![("drywall".to_string(), 0.013), ("stud".to_string(), 0.09), ("drywall".to_string(), 0.013)]
    }
}

/// Command specs for the architecture module.
pub fn specs() -> Vec<CommandSpec> {
    let mut v = Vec::new();
    v.extend(wall::specs());
    v.extend(opening::specs());
    v.extend(room::specs());
    v.extend(library::specs());
    v.extend(roomdetect::specs());
    v.extend(inspect::specs());
    v.extend(pick::specs());
    v.extend(level::specs());
    v.extend(stair::specs());
    v.extend(library_icon::specs());
    v.extend(build3d::specs());
    v.extend(sample::specs());
    v.extend(stretch::specs());
    v.extend(furnish::specs());
    v.extend(view::specs());
    v.extend(electrical::specs());
    v.extend(outlets_auto::specs());
    v.extend(electrical_view::specs());
    v.extend(views::specs());
    v.extend(roofauto::specs());
    v.extend(deck::specs());
    v.extend(library_user::specs());
    v.extend(railing::specs());
    v.extend(sun::specs());
    v.extend(sheet::specs());
    v.extend(paint::specs());
    v.extend(archdetect::specs());
    v.extend(site::specs());
    v.extend(terrain::specs());
    v.extend(backdrop::specs());
    v.extend(look::specs());
    v.extend(terrain_modify::specs());
    v.extend(terrain_query::specs());
    v.extend(plant::specs());
    v.extend(defaults::specs());
    v.extend(lightfixture::specs());
    v.extend(lightset::specs());
    v.extend(connections::specs());
    v.extend(casework::specs());
    v.extend(material::specs());
    v.extend(illuminance::specs());
    v.extend(render::specs());
    v.extend(sunstudy::specs());
    v.extend(tray::specs());
    v.extend(foundation::specs());
    v.extend(soffit::specs());
    v
}
