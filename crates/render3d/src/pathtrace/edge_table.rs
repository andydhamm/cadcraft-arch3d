//! The edge table (phase 22): per triangle, the neighbours across each edge, so `pt_round_edge`
//! (`shaders/pt/edges.wgsl`) can bend a shading normal near a crease. [`fill`] appends the table to
//! the scene's aux buffer and records its offset and triangle count in `GpuFrame::photo` (y, z)
//! with `PHOTO_EDGES`; p22-r3d-edges writes it. Until then it does nothing.

use super::PathOptions;
use super::scene_gpu::{GpuFrame, GpuScene};

/// Build the edge table when `opts.edges` asks for one. A no-op for now: the frame and the aux
/// buffer stay as they are.
pub(super) fn fill(_scene: &mut GpuScene, _frame: &mut GpuFrame, _opts: &PathOptions) {}
