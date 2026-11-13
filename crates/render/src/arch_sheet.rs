use cadcraft_doc::arch::SheetSource;
use cadcraft_doc::arch::sheet::{LINK_SHEET_VIEW, read_link};
use cadcraft_doc::{Drawing, Entity};

/// A clone of `d` set to the named plan view this sheet viewport shows, else `None`.
///
/// The cloned drawing keeps the active session's own view and level unchanged, but the viewport
/// path applies the plan view's layer state and plan-level rules to the copy before drawing it.
pub(crate) fn plan_view_drawing(d: &Drawing, e: &Entity) -> Option<Drawing> {
    let (role, sheet_name, number) = read_link(&e.common.xdata)?;
    if role != LINK_SHEET_VIEW {
        return None;
    }
    let sheet = d.arch.sheet(&sheet_name)?;
    let view = sheet.views.iter().find(|v| v.number == number)?;
    let SheetSource::Plan { view: name, level } = &view.source else {
        return None;
    };
    let plan = d.arch.plan_view(name)?;
    let mut local = d.clone();
    for layer in cadcraft_arch::plan::layers_in_use(&local.arch, plan.kind) {
        local.ensure_layer(layer.as_str());
    }
    if !plan.annotation.layer.is_empty() {
        local.ensure_layer(&plan.annotation.layer);
    }
    let view_level = plan.level.filter(|l| usize::try_from(*l).is_ok_and(|i| i < local.arch.levels.len()));
    let source_level = level.filter(|l| usize::try_from(*l).is_ok_and(|i| i < local.arch.levels.len()));
    if let Some(level) = source_level.or(view_level) {
        local.arch.plan_level = Some(level);
    }
    let _ = plan.layers.apply(&mut local.layers);
    local.arch.active_plan_view = Some(plan.name.clone());
    Some(local)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::arch::sheet::{LINK_SHEET_VIEW, SheetView, link_tags};
    use cadcraft_doc::arch::{PlanKind, PlanView, Sheet};
    use cadcraft_doc::{Entity, EntityKind, Handle, LayerState, Viewport};

    #[test]
    fn plan_viewport_hides_what_its_view_hides() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-WALL");
        d.ensure_layer("A-ROOM");
        d.arch.plan_views = vec![PlanView {
            name: "Demo".into(),
            kind: PlanKind::Floor,
            layers: LayerState {
                name: "Demo".into(),
                layers: Vec::new(),
                rules: vec![cadcraft_doc::LayerRule { pattern: "A-WALL".into(), on: Some(false), transparency: None }],
            },
            ..PlanView::default()
        }];
        d.arch.sheets.push(Sheet {
            name: "S1".into(),
            views: vec![SheetView { number: 1, source: SheetSource::Plan { view: "Demo".into(), level: None }, ..SheetView::default() }],
            ..Sheet::default()
        });
        let mut e = Entity::new(Handle(1), EntityKind::Viewport(Viewport::default()));
        e.common.xdata = link_tags(LINK_SHEET_VIEW, "S1", 1).into_iter().collect();
        let local = plan_view_drawing(&d, &e).expect("plan sheet viewport");
        assert!(local.layer("A-WALL").is_some_and(|layer| !layer.on));
    }

    #[test]
    fn plan_viewport_draws_its_level() {
        let mut d = Drawing::new_metric();
        d.arch.levels.push(cadcraft_doc::arch::Level { index: 1, elevation: 3.0, ..cadcraft_doc::arch::Level::default() });
        d.arch.plan_views = vec![PlanView { name: "Demo".into(), kind: PlanKind::Floor, level: Some(1), ..PlanView::default() }];
        d.arch.sheets.push(Sheet {
            name: "S1".into(),
            views: vec![SheetView { number: 1, source: SheetSource::Plan { view: "Demo".into(), level: None }, ..SheetView::default() }],
            ..Sheet::default()
        });
        let mut e = Entity::new(Handle(2), EntityKind::Viewport(Viewport::default()));
        e.common.xdata = link_tags(LINK_SHEET_VIEW, "S1", 1).into_iter().collect();
        let local = plan_view_drawing(&d, &e).expect("plan sheet viewport");
        assert_eq!(local.arch.plan_level, Some(1));
    }

    #[test]
    fn plan_viewport_annotates_with_its_view() {
        let mut d = Drawing::new_metric();
        d.ensure_layer("A-ANNO-NOTE");
        d.arch.plan_views = vec![PlanView {
            name: "Demo".into(),
            kind: PlanKind::Floor,
            annotation: cadcraft_doc::arch::AnnotationSet { layer: "A-ANNO-NOTE".into(), ..cadcraft_doc::arch::AnnotationSet::default() },
            ..PlanView::default()
        }];
        d.arch.sheets.push(Sheet {
            name: "S1".into(),
            views: vec![SheetView { number: 1, source: SheetSource::Plan { view: "Demo".into(), level: None }, ..SheetView::default() }],
            ..Sheet::default()
        });
        let mut e = Entity::new(Handle(3), EntityKind::Viewport(Viewport::default()));
        e.common.xdata = link_tags(LINK_SHEET_VIEW, "S1", 1).into_iter().collect();
        let local = plan_view_drawing(&d, &e).expect("plan sheet viewport");
        assert_eq!(local.arch.active_plan_view.as_deref(), Some("Demo"));
        assert!(local.layer("A-ANNO-NOTE").is_some());
    }

    #[test]
    fn other_viewports_are_unchanged() {
        let mut d = Drawing::new_metric();
        d.arch.active_plan_view = Some("Other".into());
        d.arch.plan_level = Some(0);
        d.arch.plan_views = vec![
            PlanView { name: "Other".into(), kind: PlanKind::Floor, ..PlanView::default() },
            PlanView { name: "Demo".into(), kind: PlanKind::Floor, ..PlanView::default() },
        ];
        d.arch.sheets.push(Sheet {
            name: "S1".into(),
            views: vec![SheetView { number: 1, source: SheetSource::Plan { view: "Other".into(), level: None }, ..SheetView::default() }],
            ..Sheet::default()
        });
        let mut e = Entity::new(Handle(4), EntityKind::Viewport(Viewport::default()));
        e.common.xdata = link_tags(LINK_SHEET_VIEW, "S1", 1).into_iter().collect();
        let local = plan_view_drawing(&d, &e).expect("plan sheet viewport");
        assert_eq!(d.arch.active_plan_view.as_deref(), Some("Other"));
        assert_eq!(local.arch.active_plan_view.as_deref(), Some("Other"));
    }

    #[test]
    fn a_dangling_link_draws_the_plain_model() {
        let mut d = Drawing::new_metric();
        d.arch.plan_views = vec![PlanView { name: "Demo".into(), kind: PlanKind::Floor, ..PlanView::default() }];
        d.arch.sheets.push(Sheet {
            name: "S1".into(),
            views: vec![SheetView { number: 1, source: SheetSource::Plan { view: "Missing".into(), level: None }, ..SheetView::default() }],
            ..Sheet::default()
        });
        let mut e = Entity::new(Handle(5), EntityKind::Viewport(Viewport::default()));
        e.common.xdata = link_tags(LINK_SHEET_VIEW, "S1", 1).into_iter().collect();
        assert!(plan_view_drawing(&d, &e).is_none());
    }
}
