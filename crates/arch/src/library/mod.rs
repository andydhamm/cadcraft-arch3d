//! The library module provides the data structures and logic for the CADCraft library.
//!
//! It handles:
//! - `record`: The `ItemRecord` structure and category/kind definitions.
//! - `search`: Query matching and filtering logic.
//! - `history`: Navigation history for library queries.
//! - `fits`: Context-aware query generation for specific targets.

pub mod fits;
pub mod history;
pub mod place;
pub mod record;
pub mod search;

pub use fits::query_for;
pub use history::FilterHistory;
pub use place::{PlaceOutcome, PlaceRequest, place};
pub use record::{Category, ItemKind, ItemRecord, folder_for, ifc_class_for, layer_for};
pub use search::{Chip, ChipKind, Query, folders, matches, search};
