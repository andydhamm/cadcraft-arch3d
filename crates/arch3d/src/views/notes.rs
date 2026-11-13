//! View notes: attaching them to the generated geometry, so annotations survive regeneration.

use cadcraft_doc::arch::{Corner, MAX_VIEW_NOTES, NoteAttach, NoteKind, ViewGeometry, ViewNote};

/// The point of `a`'s object in `g` (its line bounds' corner), or `None` when the object has no line in the view.
pub fn attach_point(g: &ViewGeometry, a: &NoteAttach) -> Option<[f64; 2]> {
    let mut b: Option<[f64; 4]> = None;
    for l in g.lines.iter().filter(|l| l.kind == a.kind && l.object == a.object) {
        for p in [l.a, l.b] {
            if !p[0].is_finite() || !p[1].is_finite() {
                continue;
            }
            b = Some(match b {
                None => [p[0], p[1], p[0], p[1]],
                Some(r) => [r[0].min(p[0]), r[1].min(p[1]), r[2].max(p[0]), r[3].max(p[1])],
            });
        }
    }
    let [u0, v0, u1, v1] = b?;
    Some(match a.corner {
        Corner::TopLeft => [u0, v1],
        Corner::TopRight => [u1, v1],
        Corner::BottomLeft => [u0, v0],
        Corner::BottomRight => [u1, v0],
        Corner::Center => [(u0 + u1) / 2.0, (v0 + v1) / 2.0],
    })
}

/// The notes of a view resolved against its freshly generated geometry (positions, detached flags). Free notes are
/// returned unchanged; an attached note whose object has no line keeps its last position and is marked detached.
pub fn resolve(notes: &[ViewNote], g: &ViewGeometry) -> Vec<ViewNote> {
    notes
        .iter()
        .take(MAX_VIEW_NOTES)
        .map(|n| {
            let mut n = n.clone();
            let mut lost = false;
            // A dimension's offset is its dimension line's offset, not a shift of the attached corners.
            let off = if n.kind == NoteKind::Dimension { [0.0, 0.0] } else { n.offset };
            if let Some(a) = n.attach {
                match attach_point(g, &a) {
                    Some(c) => n.at = [c[0] + off[0], c[1] + off[1]],
                    None => lost = true,
                }
            }
            if let Some(a) = n.attach_b {
                match attach_point(g, &a) {
                    Some(c) => n.b = Some([c[0] + off[0], c[1] + off[1]]),
                    None => lost = true,
                }
            }
            if n.attach.is_some() || n.attach_b.is_some() {
                n.detached = lost;
            }
            n
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::{ArchId, ArchKind, ViewLine};

    fn geom(x: f64) -> ViewGeometry {
        let line = |a: [f64; 2], b: [f64; 2]| ViewLine { a, b, kind: ArchKind::Cabinet, object: ArchId(0), ..Default::default() };
        ViewGeometry { lines: vec![line([x, 0.0], [x + 0.6, 0.0]), line([x + 0.6, 0.0], [x + 0.6, 0.9])], ..Default::default() }
    }

    fn attach(corner: Corner) -> Option<NoteAttach> {
        Some(NoteAttach { kind: ArchKind::Cabinet, object: ArchId(0), corner, ..Default::default() })
    }

    #[test]
    fn free_notes_keep_their_coordinates() {
        let n = ViewNote { id: 0, text: "t".into(), at: [-6.5, 1.5], ..Default::default() };
        assert_eq!(resolve(&[n.clone()], &geom(3.0)), vec![n]);
    }

    #[test]
    fn attached_note_follows_its_corner() {
        let n = ViewNote { text: "t".into(), attach: attach(Corner::TopLeft), offset: [0.0, 0.1], ..Default::default() };
        let r = resolve(&[n.clone()], &geom(1.0));
        assert_eq!(r[0].at, [1.0, 1.0]);
        let r = resolve(&[n], &geom(2.0));
        assert_eq!(r[0].at, [2.0, 1.0]);
        assert!(!r[0].detached);
    }

    #[test]
    fn dimension_between_two_attached_corners() {
        let n = ViewNote {
            kind: NoteKind::Dimension,
            text: "d".into(),
            attach: attach(Corner::BottomLeft),
            attach_b: attach(Corner::BottomRight),
            b: Some([0.0, 0.0]),
            ..Default::default()
        };
        let r = resolve(&[n], &geom(1.5));
        assert_eq!(r[0].at, [1.5, 0.0]);
        assert_eq!(r[0].b, Some([2.1, 0.0]));
    }

    #[test]
    fn missing_object_marks_detached() {
        let n = ViewNote { text: "t".into(), at: [4.0, 5.0], attach: attach(Corner::Center), ..Default::default() };
        let r = resolve(&[n], &ViewGeometry::default());
        assert!(r[0].detached);
        assert_eq!(r[0].at, [4.0, 5.0]);
    }
}
