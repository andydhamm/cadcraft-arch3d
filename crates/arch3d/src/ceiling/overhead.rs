//! Ceiling clipping by rooms above (phase 16): the floor above caps a piece, and an Open Below
//! room above opens the room's ceiling and lets the upper room's own ceiling show through.

use cadcraft_arch::ceiling::overhead::{Overhead, overhead_in};
use cadcraft_geom::Vec2;

use super::CeilingIn;
use crate::planar::{Piece, PlaneZ, centroid, split_by_level, split_by_rings};

/// Clip a room's rough ceiling against the rooms above it.
pub(crate) fn apply(i: &CeilingIn, pieces: Vec<Piece>) -> Vec<Piece> {
    if i.above.is_empty() {
        return pieces;
    }
    let rings: Vec<Vec<Vec2>> = i.above.iter().flat_map(|room| std::iter::once(room.polygon.clone()).chain(room.holes.iter().cloned())).collect();
    let mut out = Vec::with_capacity(pieces.len());
    for piece in pieces {
        let cells = split_by_rings(&piece.outer, &piece.holes, &rings);
        if cells.is_empty() {
            out.push(piece);
            continue;
        }
        let mut kept = Vec::new();
        let mut changed = false;
        for (cell, _) in cells {
            let Some(c) = centroid(&cell) else {
                kept.push(Piece::new(cell, Vec::new(), piece.plane));
                continue;
            };
            match overhead_in(&i.above, c) {
                Overhead::Clear => kept.push(Piece::new(cell, Vec::new(), piece.plane)),
                Overhead::Floor { underside, .. } => {
                    if piece.plane.is_flat() {
                        let z = piece.plane.c.min(underside);
                        if (z - piece.plane.c).abs() > 1e-12 {
                            kept.push(Piece::new(cell, Vec::new(), PlaneZ::flat(z)));
                            changed = true;
                        } else {
                            kept.push(Piece::new(cell, Vec::new(), piece.plane));
                        }
                    } else {
                        let (below, above) = split_by_level(&cell, piece.plane, underside);
                        if above.is_empty() {
                            kept.push(Piece::new(cell, Vec::new(), piece.plane));
                        } else {
                            if !below.is_empty() {
                                kept.push(Piece::new(below, Vec::new(), piece.plane));
                            }
                            kept.push(Piece::new(above, Vec::new(), PlaneZ::flat(underside)));
                            changed = true;
                        }
                    }
                }
                Overhead::Open { .. } => {
                    changed = true;
                }
            }
        }
        if changed {
            out.extend(kept);
        } else {
            out.push(piece);
        }
    }
    out
}
