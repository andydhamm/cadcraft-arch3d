//! Molding lines in the plan: each line's inset path on `A-FLOR-TRIM`, tagged with its handle.

use cadcraft_arch::molding_line::line_run_at;
use cadcraft_color::Color;
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{MOLDING_LAYER, molding_line_handle};

use crate::DisplayList;
use crate::arch::{arch_layer_skipped, layer_transparency, on_shown_level, push_polyline_with_transparency};

/// Append every free molding line to `list` when it belongs on the shown level.
pub(crate) fn push_molding_lines(d: &Drawing, list: &mut DisplayList, plotting: bool) {
    if arch_layer_skipped(d, MOLDING_LAYER, plotting) {
        return;
    }
    let layer = d.layer(MOLDING_LAYER).map(|l| l.color).unwrap_or(Color::Index(7));
    let ink = crate::ink(Color::ByLayer, layer, Color::Index(7));
    let transparency = layer_transparency(d, MOLDING_LAYER);
    for line in &d.arch.molding_lines {
        if !on_shown_level(d, line.level) {
            continue;
        }
        let Some(run) = line_run_at(line, d.arch.level(line.level).map_or(0.0, |level| level.elevation)) else {
            continue;
        };
        if run.path.len() < 2 || !run.path.iter().all(|p| p.is_finite()) {
            continue;
        }
        push_polyline_with_transparency(list, molding_line_handle(line.id), &ink, &run.path, transparency);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::ArchId;
    use cadcraft_doc::arch::{Level, MoldingLine};
    use cadcraft_geom::Vec2;

    #[test]
    fn path_drawn_on_a_flor_trim_on_its_level_only() {
        let mut d = Drawing::default();
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        d.arch.levels.push(Level { index: 1, elevation: 4.0, ..Default::default() });
        let profile = cadcraft_arch::molding_profile::builtin_profile("molding:chair-rail").unwrap();
        d.arch.molding_lines.push(MoldingLine {
            id: ArchId(1),
            level: 0,
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0)],
            elevation: 0.05,
            inset: 0.1,
            profile: profile.clone(),
            ..Default::default()
        });
        d.arch.plan_level = Some(0);
        let list = crate::arch::build_arch(&d, &crate::Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == molding_line_handle(ArchId(1))));

        let mut other = d.clone();
        other.arch.plan_level = Some(1);
        let list = crate::arch::build_arch(&other, &crate::Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == molding_line_handle(ArchId(1))));
    }

    #[test]
    fn hidden_when_the_layer_is_off() {
        let mut d = Drawing::default();
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        let profile = cadcraft_arch::molding_profile::builtin_profile("molding:chair-rail").unwrap();
        d.arch.molding_lines.push(MoldingLine {
            id: ArchId(2),
            level: 0,
            points: vec![Vec2::new(0.0, 0.0), Vec2::new(2.0, 0.0)],
            elevation: 0.05,
            inset: 0.1,
            profile,
            ..Default::default()
        });

        let list = crate::arch::build_arch(&d, &crate::Options::default(), false);
        assert!(list.prims.iter().any(|p| p.handle == molding_line_handle(ArchId(2))));

        let mut hidden = d.clone();
        hidden.ensure_layer(MOLDING_LAYER);
        if let Some(layer) = hidden.layers.iter_mut().find(|l| l.name == MOLDING_LAYER) {
            layer.on = false;
        }
        let list = crate::arch::build_arch(&hidden, &crate::Options::default(), false);
        assert!(!list.prims.iter().any(|p| p.handle == molding_line_handle(ArchId(2))));
    }
}
