//! Plan view tabs: a strip under the drawing tabs with one tab per open plan view.
//!
//! The strip reads `arch.open_plan_views`, `arch.active_plan_view` and `arch.plan_views` and acts
//! only through the `planview*` commands (`app.run`), so MCP and the CLI reach the same behaviour.
//! A click opens a view, the X closes it, "+" opens a closed view or makes a new one, and the tab's
//! right-click menu ([`crate::shortcut_menus::plan_tab_entries`]) duplicates, renames, deletes and
//! sets the view's room labels and reference level.

use egui::{Rect, Sense, Stroke, pos2, vec2};
use serde_json::{Value, json};

use crate::CadApp;
use crate::icons::{self, Icon};
use crate::theme::Tokens;

/// Height of the strip.
const STRIP_HEIGHT: f32 = 24.0;
/// Widest a tab gets; more tabs than fit share the room.
const TAB_WIDTH: f32 = 140.0;
/// Narrowest a tab gets before the strip clips.
const TAB_MIN_WIDTH: f32 = 48.0;
/// How many names are tried when looking for a free one.
const MAX_NAME_TRIES: usize = 1000;

/// Whether the strip is shown: a drawing is on screen and it has walls or open plan views.
pub fn strip_visible(app: &CadApp) -> bool {
    if app.ui.start_tab {
        return false;
    }
    let Ok(d) = app.session.doc() else { return false };
    !d.arch.walls.is_empty() || !d.arch.open_plan_views.is_empty()
}

fn taken(app: &CadApp, name: &str) -> bool {
    app.session.doc().is_ok_and(|d| d.arch.plan_views.iter().any(|v| v.name.trim().eq_ignore_ascii_case(name)))
}

/// The first free "View N" name (N from 1).
pub fn new_view_name(app: &CadApp) -> String {
    (1..=MAX_NAME_TRIES).map(|n| format!("View {n}")).find(|n| !taken(app, n)).unwrap_or_else(|| "View".to_string())
}

/// The first free "<name> copy", "<name> copy 2", ... name.
pub fn copy_name(app: &CadApp, name: &str) -> String {
    let base = format!("{} copy", name.trim());
    if !taken(app, &base) {
        return base;
    }
    (2..=MAX_NAME_TRIES).map(|n| format!("{base} {n}")).find(|n| !taken(app, n)).unwrap_or(base)
}

/// Run a command, echoing an error to the command line.
fn run(app: &mut CadApp, command: &str, params: Value) {
    if let Err(e) = app.run(command, params) {
        app.session.echo(e);
    }
}

/// Carry out a plan tab menu action (`op` is the part of `planview:<op>:<name>` before the name).
pub(crate) fn plan_action(app: &mut CadApp, op: &str, name: &str) {
    match op {
        "open" => run(app, "planview.open", json!({ "name": name })),
        "close" => run(app, "planview.close", json!({ "name": name })),
        "delete" => run(app, "planview.delete", json!({ "name": name })),
        "new" => {
            let fresh = new_view_name(app);
            run(app, "planview", json!({ "name": fresh }));
        }
        "duplicate" => {
            let copy = copy_name(app, name);
            run(app, "planview", json!({ "name": copy, "from": name }));
        }
        "rename" => app.ui.plan_rename = Some((name.to_string(), name.to_string())),
        "ref.none" => run(app, "planview.edit", json!({ "name": name, "reference": Value::Null })),
        _ => {
            if let Some(flag) = op.strip_prefix("label.") {
                let Some(current) = app.session.doc().ok().and_then(|d| d.arch.plan_view(name)).map(|v| v.labels) else { return };
                let on = match flag {
                    "name" => current.name,
                    "area" => current.area,
                    "dimensions" => current.dimensions,
                    "ceiling" => current.ceiling,
                    _ => return,
                };
                run(app, "planview.edit", json!({ "name": name, "labels": { flag: !on } }));
            } else if let Some(level) = op.strip_prefix("ref.").and_then(|l| l.parse::<u32>().ok()) {
                run(app, "planview.edit", json!({ "name": name, "reference": { "level": level } }));
            }
        }
    }
}

/// Finish the rename box: rename the view to `text` (errors echo to the command line).
pub(crate) fn commit_rename(app: &mut CadApp) {
    let Some((old, text)) = app.ui.plan_rename.take() else { return };
    if text.trim() != old.trim() {
        run(app, "planview.edit", json!({ "name": old, "rename": text.trim() }));
    }
}

/// What the strip's widgets asked for this frame.
enum Pick {
    Open(String),
    Close(String),
    Menu(&'static str, String),
}

/// The plan view strip, drawn under the drawing tabs. Nothing is drawn when [`strip_visible`] is
/// false.
pub fn plan_tabs(app: &mut CadApp, ui: &mut egui::Ui) {
    if !strip_visible(app) {
        app.ui.plan_rename = None;
        return;
    }
    let t = Tokens::get();
    let (open, active) = match app.session.doc() {
        Ok(d) => (d.arch.open_plan_views.clone(), d.arch.active_plan_view.clone()),
        Err(_) => return,
    };
    let closed: Vec<String> = app
        .session
        .doc()
        .map(|d| d.arch.plan_views.iter().filter(|v| !open.iter().any(|o| o.eq_ignore_ascii_case(&v.name))).map(|v| v.name.clone()).collect())
        .unwrap_or_default();
    let mut picks: Vec<Pick> = Vec::new();
    egui::Panel::top("cc_plan_tabs").exact_size(STRIP_HEIGHT).frame(egui::Frame::NONE.fill(t.chrome_dark)).show(ui, |ui| {
        let r = ui.max_rect();
        let room = (r.width() - 40.0).max(0.0);
        let width = (room / open.len().max(1) as f32).clamp(TAB_MIN_WIDTH, TAB_WIDTH);
        let mut x = r.left() + 4.0;
        for (i, name) in open.iter().enumerate() {
            let is_active = active.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(name));
            let id = ui.scope_id().with(("plan_tab", i));
            let tr = Rect::from_min_size(pos2(x, r.top()), vec2(width, r.height()));
            let resp = ui.interact(tr, id, Sense::click());
            let hovered = ui.rect_contains_pointer(tr);
            let fill = if is_active {
                t.tab_active
            } else if hovered {
                t.chrome
            } else {
                t.chrome_dark
            };
            ui.painter().rect_filled(tr, 0.0, fill);
            let color = if is_active { t.text } else { t.text_dim };
            let text_w = (width - 8.0 - 24.0).max(0.0);
            let (galley, cut) = crate::chrome::elided(ui.painter(), name, crate::theme::body(), color, text_w);
            ui.painter().galley(pos2(tr.left() + 8.0, tr.center().y - galley.size().y / 2.0), galley, color);
            let resp = if cut { resp.on_hover_text(name.clone()) } else { resp };
            if is_active || hovered {
                let cr = Rect::from_center_size(pos2(tr.right() - 11.0, tr.center().y), vec2(12.0, 12.0));
                let cresp = ui.interact(cr, id.with("x"), Sense::click());
                icons::paint(ui.painter(), cr, Icon::Close, false);
                icons::describe_control(ui, &cresp, &format!("Close {name}"), None, false);
                if cresp.clicked() {
                    picks.push(Pick::Close(name.clone()));
                }
            }
            icons::describe_control(ui, &resp, name, Some(is_active), true);
            ui.painter().vline(tr.right(), tr.y_range(), Stroke::new(1.0, t.border));
            if resp.clicked() {
                picks.push(Pick::Open(name.clone()));
            }
            crate::shortcut_menus::attach(app, &resp, |app| crate::shortcut_menus::plan_tab_entries(app, name));
            x += width;
        }
        // "+": the views that are not open, then a new one.
        let plus = Rect::from_min_size(pos2(x + 6.0, r.top() + 3.0), vec2(18.0, 18.0));
        let presp = ui.interact(plus, ui.scope_id().with("plan_tab+"), Sense::click());
        icons::paint(ui.painter(), plus.shrink(2.0), Icon::Plus, false);
        icons::describe_control(ui, &presp, "Plan View Tabs", None, false);
        egui::Popup::menu(&presp).show(|ui| {
            for name in &closed {
                if ui.button(name).clicked() {
                    picks.push(Pick::Open(name.clone()));
                    ui.close();
                }
            }
            if !closed.is_empty() {
                ui.separator();
            }
            if ui.button(crate::tl!("New Plan View")).clicked() {
                picks.push(Pick::Menu("new", String::new()));
                ui.close();
            }
        });
        presp.on_hover_text(crate::tl!("Plan View Tabs"));
    });
    for pick in picks {
        match pick {
            Pick::Open(name) => plan_action(app, "open", &name),
            Pick::Close(name) => plan_action(app, "close", &name),
            Pick::Menu(op, name) => plan_action(app, op, &name),
        }
    }
    rename_box(app, ui.ctx());
}

/// The small modal asking for a plan view's new name.
fn rename_box(app: &mut CadApp, ctx: &egui::Context) {
    let Some((_, mut text)) = app.ui.plan_rename.clone() else { return };
    let (mut ok, mut cancel) = (false, false);
    crate::modal::instant(ctx, egui::Id::unique("cc_plan_rename")).show(ctx, |ui| {
        ui.set_width(260.0);
        ui.heading(crate::tl!("Rename..."));
        let edit = ui.add(egui::TextEdit::singleline(&mut text).desired_width(f32::INFINITY));
        edit.request_focus();
        if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            ok = true;
        }
        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            cancel = true;
        }
        ui.horizontal(|ui| {
            ok |= ui.button(crate::tl!("OK")).clicked();
            cancel |= ui.button(crate::tl!("Cancel")).clicked();
        });
    });
    if let Some(slot) = app.ui.plan_rename.as_mut() {
        slot.1 = text;
    }
    if ok {
        commit_rename(app);
    } else if cancel {
        app.ui.plan_rename = None;
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::menus::Entry;
    use crate::shortcut_menus::{choose, plan_tab_entries};
    use cadcraft_doc::arch::{Level, Wall};
    use cadcraft_engine::Session;
    use cadcraft_geom::Vec2;
    use egui::accesskit::Role;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// An app on a drawing with one wall, two levels and the open views `views`.
    fn app_with(views: &[&str]) -> CadApp {
        let mut app = CadApp::new(Session::default(), crate::Services::default());
        app.run("new", json!({})).unwrap();
        app.ui.start_tab = false;
        {
            let d = app.session.doc_mut().unwrap();
            d.arch.levels = vec![
                Level { index: 0, name: "Ground".into(), ..Level::default() },
                Level { index: 1, name: "Upper".into(), elevation: 3.0, ..Level::default() },
            ];
            d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0), 0.2));
        }
        for v in views {
            app.run("planview", json!({ "name": v })).unwrap();
        }
        app
    }

    fn open_views(app: &CadApp) -> Vec<String> {
        app.session.doc().unwrap().arch.open_plan_views.clone()
    }

    fn active(app: &CadApp) -> Option<String> {
        app.session.doc().unwrap().arch.active_plan_view.clone()
    }

    fn harness(app: CadApp) -> Harness<'static, CadApp> {
        let mut h = Harness::builder().with_size(vec2(900.0, 300.0)).build_ui_state(
            |ui, app: &mut CadApp| {
                plan_tabs(app, ui);
            },
            app,
        );
        h.run_steps(3);
        h
    }

    fn choose_id(app: &mut CadApp, id: &str) {
        choose(app, &egui::Context::default(), id);
    }

    fn labels(groups: &[Vec<Entry>]) -> Vec<String> {
        groups
            .iter()
            .flatten()
            .map(|e| match e {
                Entry::Item { label, .. } | Entry::Sub { label, .. } => label.clone(),
            })
            .collect()
    }

    #[test]
    fn strip_lists_the_open_views_in_order() {
        let h = harness(app_with(&["Alpha", "Beta", "Gamma"]));
        let tabs = h.query_all_by_role(Role::Tab).count();
        assert_eq!(tabs, 3);
        assert_eq!(open_views(h.state()), vec!["Alpha", "Beta", "Gamma"]);
        for name in ["Alpha", "Beta", "Gamma"] {
            assert!(h.query_by_role_and_label(Role::Tab, name).is_some(), "{name} has a tab");
        }
        assert_eq!(active(h.state()).as_deref(), Some("Gamma"));
    }

    #[test]
    fn clicking_a_tab_runs_planview_open() {
        let mut h = harness(app_with(&["Alpha", "Beta"]));
        assert_eq!(active(h.state()).as_deref(), Some("Beta"));
        h.get_by_role_and_label(Role::Tab, "Alpha").click();
        h.run_steps(3);
        assert_eq!(active(h.state()).as_deref(), Some("Alpha"));
        assert_eq!(open_views(h.state()), vec!["Alpha", "Beta"]);
    }

    #[test]
    fn close_runs_planview_close() {
        let mut h = harness(app_with(&["Alpha", "Beta"]));
        h.get_by_label("Close Beta").click();
        h.run_steps(3);
        assert_eq!(open_views(h.state()), vec!["Alpha"]);
        assert!(h.state().session.doc().unwrap().arch.plan_view("Beta").is_some(), "closing keeps the view");
    }

    #[test]
    fn new_and_duplicate_pick_a_free_name() {
        let mut app = app_with(&[]);
        app.run("planview.open", json!({ "name": "Floor Plan" })).unwrap();
        choose_id(&mut app, "planview:new:");
        assert!(app.session.doc().unwrap().arch.plan_view("View 1").is_some());
        choose_id(&mut app, "planview:new:");
        assert!(app.session.doc().unwrap().arch.plan_view("View 2").is_some());
        choose_id(&mut app, "planview:duplicate:Floor Plan");
        choose_id(&mut app, "planview:duplicate:Floor Plan");
        let d = app.session.doc().unwrap();
        assert!(d.arch.plan_view("Floor Plan copy").is_some());
        assert!(d.arch.plan_view("Floor Plan copy 2").is_some());
        assert_eq!(copy_name(&app, "Floor Plan"), "Floor Plan copy 3");
        let fresh = app_with(&[]);
        assert_eq!(new_view_name(&fresh), "View 1");
    }

    #[test]
    fn rename_and_delete_run_their_commands() {
        let mut app = app_with(&["Alpha", "Beta"]);
        choose_id(&mut app, "planview:rename:Alpha");
        assert_eq!(app.ui.plan_rename, Some(("Alpha".to_string(), "Alpha".to_string())));
        if let Some(slot) = app.ui.plan_rename.as_mut() {
            slot.1 = "Main".into();
        }
        commit_rename(&mut app);
        assert!(app.ui.plan_rename.is_none());
        assert_eq!(open_views(&app), vec!["Main", "Beta"]);
        // A taken name is refused and echoed.
        choose_id(&mut app, "planview:rename:Main");
        if let Some(slot) = app.ui.plan_rename.as_mut() {
            slot.1 = "Beta".into();
        }
        let logged = app.session.log.len();
        commit_rename(&mut app);
        assert!(app.session.log.len() > logged, "the error is echoed");
        assert_eq!(open_views(&app), vec!["Main", "Beta"]);
        choose_id(&mut app, "planview:delete:Beta");
        assert!(app.session.doc().unwrap().arch.plan_view("Beta").is_none());
        assert_eq!(open_views(&app), vec!["Main"]);
    }

    #[test]
    fn label_toggles_run_planview_edit() {
        let mut app = app_with(&["Alpha"]);
        let shown = labels(&plan_tab_entries(&app, "Alpha"));
        assert!(shown.iter().any(|l| l.contains("Room Names")) && shown.iter().any(|l| l.contains("Ceiling Heights")));
        let flags = |app: &CadApp| {
            let l = app.session.doc().unwrap().arch.plan_view("Alpha").unwrap().labels;
            (l.name, l.area, l.dimensions, l.ceiling)
        };
        let before = flags(&app);
        choose_id(&mut app, "planview:label.area:Alpha");
        choose_id(&mut app, "planview:label.ceiling:Alpha");
        assert_eq!(flags(&app), (before.0, !before.1, before.2, !before.3));
        choose_id(&mut app, "planview:label.name:Alpha");
        choose_id(&mut app, "planview:label.dimensions:Alpha");
        assert_eq!(flags(&app), (!before.0, !before.1, !before.2, !before.3));
    }

    #[test]
    fn reference_menu_sets_and_clears_the_reference() {
        let mut app = app_with(&["Alpha"]);
        app.run("planview.edit", json!({ "name": "Alpha", "level": "Ground" })).unwrap();
        let groups = plan_tab_entries(&app, "Alpha");
        let sub = groups.iter().flatten().find_map(|e| match e {
            Entry::Sub { label, children } if label == "Reference Level" => Some(children),
            _ => None,
        });
        let children = labels(&[sub.unwrap().clone()]);
        assert!(children.iter().any(|l| l.contains("None")));
        assert!(children.iter().any(|l| l.contains("Upper")));
        assert!(!children.iter().any(|l| l.contains("Ground")), "the view's own level is left out");
        let reference = |app: &CadApp| app.session.doc().unwrap().arch.plan_view("Alpha").unwrap().reference.map(|r| r.level);
        choose_id(&mut app, "planview:ref.1:Alpha");
        assert_eq!(reference(&app), Some(1));
        choose_id(&mut app, "planview:ref.none:Alpha");
        assert_eq!(reference(&app), None);
    }

    #[test]
    fn strip_hidden_without_walls_or_tabs() {
        let mut app = app_with(&[]);
        assert!(strip_visible(&app), "a wall shows the strip");
        app.session.doc_mut().unwrap().arch.walls.clear();
        assert!(!strip_visible(&app));
        app.run("planview", json!({ "name": "Alpha" })).unwrap();
        assert!(strip_visible(&app), "an open view shows the strip");
        app.run("planview.close", json!({ "name": "Alpha" })).unwrap();
        assert!(!strip_visible(&app));
        let h = harness(app);
        assert_eq!(h.query_all_by_role(Role::Tab).count(), 0);
    }
}
