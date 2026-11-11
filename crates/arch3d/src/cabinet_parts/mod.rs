//! The parts of a cabinet's meshes that phase 18 owners fill: one file per owner, each a stub that
//! builds nothing until its segment replaces it. `casework::cabinet_meshes_in` calls them.

pub mod appliance;
pub mod corner;
pub mod counter;
pub mod filler;
pub mod frame;
pub mod molding;
pub mod shelves;
