//! Electrical tools in the plan and the 3D pane (phase 15): the outlet and switch tools that
//! click a wall face, the Connection drag from a switch to lights, the grips that reshape a
//! connection's arcs, the Electrical View toggle, the plan's level picker and the light-set
//! chooser. Everything runs through `Session::execute` (`outlet`, `switch`, `connection`,
//! `connection.edit`, `view.electrical`, `layerstate.restore`, `level.show`, `lightset`); this
//! module only turns pointer positions into those commands' parameters.

use cadcraft_arch::connections::{bulged_polyline, device_plan, light_plan};
use cadcraft_doc::arch::{ElectricalKind, WallSide, connection_label, electrical_label, light_label, wall_label};
use cadcraft_doc::{ArchId, ArchModel};
use cadcraft_engine::Session;
use cadcraft_geom::Vec2;
use serde_json::{Value, json};

/// The layer state `view.electrical` saves the previous layer settings under.
pub const ELECTRICAL_STATE: &str = "Electrical";

/// How far (m) a pointer may be from a switch, a light or an arc grip to pick it.
const PICK_RADIUS: f64 = 0.3;
/// How far (m) beyond a wall's face a plan click still counts as on that face.
const FACE_SLACK: f64 = 0.15;

/// A tool armed in the plan or the 3D pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Click a wall face to place an outlet or switch of this kind.
    Device(ElectricalKind),
    /// Drag from a switch to lights.
    Connection,
}

/// One entry of Architecture ▸ Electrical tools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolInfo {
    /// The tool's id, `electrical.<kind>` or `electrical.connection`.
    pub id: &'static str,
    /// The label shown in the menu.
    pub label: &'static str,
    /// What the tool arms.
    pub tool: Tool,
}

/// The electrical tools: an outlet or switch per kind, then the Connection tool.
pub const TOOLS: &[ToolInfo] = &[
    ToolInfo { id: "electrical.duplex", label: "Duplex Outlet", tool: Tool::Device(ElectricalKind::Duplex) },
    ToolInfo { id: "electrical.gfci", label: "GFCI Outlet", tool: Tool::Device(ElectricalKind::Gfci) },
    ToolInfo { id: "electrical.240v", label: "240 V Outlet", tool: Tool::Device(ElectricalKind::Outlet240) },
    ToolInfo { id: "electrical.single-pole", label: "Single-Pole Switch", tool: Tool::Device(ElectricalKind::SinglePole) },
    ToolInfo { id: "electrical.three-way", label: "Three-Way Switch", tool: Tool::Device(ElectricalKind::ThreeWay) },
    ToolInfo { id: "electrical.dimmer", label: "Dimmer Switch", tool: Tool::Device(ElectricalKind::Dimmer) },
    ToolInfo { id: "electrical.connection", label: "Connection", tool: Tool::Connection },
];

/// The state of the electrical tools in the app.
#[derive(Clone, Debug, Default)]
pub struct ElectricalTools {
    /// The armed tool (`None`: clicks select).
    pub armed: Option<Tool>,
    /// The switch a Connection drag started on.
    pub drag_from: Option<ArchId>,
    /// Whether the Electrical View is on (so the toggle knows to restore the previous state).
    pub view_on: bool,
    /// The light set chosen for 3D renders (`None`: the lights as they are).
    pub light_set: Option<String>,
    /// What the last tool command reported.
    pub note: String,
    /// The arc grip being dragged.
    pub grip: Option<ArcGrip>,
}

/// The wall face a plan point is on: the wall, the side the point lies on and the offset along
/// the wall from its start. Walls of `level` only when it is given.
pub fn wall_face_at(model: &ArchModel, p: Vec2, level: Option<u32>) -> Option<(ArchId, WallSide, f64)> {
    let mut best: Option<(f64, ArchId, WallSide, f64)> = None;
    for w in model.walls.iter().filter(|w| level.is_none_or(|l| w.base_level == l)) {
        let d = w.end - w.start;
        let len = d.len();
        if !len.is_finite() || len < 1e-9 || !p.is_finite() {
            continue;
        }
        let dir = d * (1.0 / len);
        let along = (p - w.start).dot(dir);
        let across = (p - w.start).dot(dir.perp());
        if !(0.0..=len).contains(&along) || across.abs() > w.thickness / 2.0 + FACE_SLACK {
            continue;
        }
        if best.as_ref().is_none_or(|b| across.abs() < b.0) {
            best = Some((across.abs(), w.id, if across >= 0.0 { WallSide::Left } else { WallSide::Right }, along));
        }
    }
    best.map(|(_, id, side, along)| (id, side, along))
}

/// The side of wall `wall` a face normal (facing the viewer) points to, and the offset of
/// `point` along the wall. `None` for a normal that is not across the wall.
pub fn face_of_click(model: &ArchModel, wall: ArchId, normal: [f64; 3], point: Vec2) -> Option<(WallSide, f64)> {
    let w = model.wall(wall)?;
    let d = w.end - w.start;
    let len = d.len();
    if !len.is_finite() || len < 1e-9 {
        return None;
    }
    let dir = d * (1.0 / len);
    let across = normal[0] * -dir.y + normal[1] * dir.x;
    let along_n = normal[0] * dir.x + normal[1] * dir.y;
    if !(across.abs() > along_n.abs() && across.abs() > normal[2].abs()) {
        return None;
    }
    Some((if across > 0.0 { WallSide::Left } else { WallSide::Right }, (point - w.start).dot(dir)))
}

fn side_key(side: WallSide) -> &'static str {
    match side {
        WallSide::Left => "left",
        WallSide::Right => "right",
    }
}

/// Run `outlet` or `switch` for `kind` on the face `side` of `wall` at `offset`.
pub fn place_device(
    session: &mut Session,
    kind: ElectricalKind,
    wall: ArchId,
    side: WallSide,
    offset: f64,
    level: Option<u32>,
) -> Result<Value, String> {
    let mut params = json!({ "kind": kind.key(), "wall": wall_label(wall), "offset": offset, "side": side_key(side) });
    if let Some(l) = level {
        params["level"] = json!(l);
    }
    let id = if kind.is_switch() { "switch" } else { "outlet" };
    session.execute(id, &params).map_err(|e| e.to_string())
}

/// A click at plan point `p` with a device tool armed.
pub fn plan_place(session: &mut Session, kind: ElectricalKind, p: Vec2) -> Result<Value, String> {
    let doc = session.doc().map_err(|e| e.to_string())?;
    let level = doc.arch.plan_level;
    let (wall, side, offset) = wall_face_at(&doc.arch, p, level).ok_or("Click a wall face")?;
    place_device(session, kind, wall, side, offset, level)
}

/// A click in the 3D pane (`click`) with a device tool armed: the face struck gives the side,
/// its projection the offset.
pub fn pane_place(session: &mut Session, kind: ElectricalKind, click: &crate::pane3d::Click) -> Result<Value, String> {
    if click.part != "wall" {
        return Err("Click a wall face".into());
    }
    let doc = session.doc().map_err(|e| e.to_string())?;
    let at = Vec2::new(click.point.x, click.point.y);
    let (side, offset) = face_of_click(&doc.arch, click.object, click.normal, at).ok_or("Click a wall face")?;
    place_device(session, kind, click.object, side, offset, None)
}

/// The switch under plan point `p`, if any: where a Connection drag may start.
pub fn switch_at(model: &ArchModel, p: Vec2) -> Option<ArchId> {
    model
        .electrical
        .iter()
        .filter(|d| d.kind.is_switch())
        .filter_map(|d| device_plan(model, d.id).map(|at| (at.dist(p), d.id)))
        .filter(|(dist, _)| *dist <= PICK_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// The light under plan point `p`, if any: where a Connection drag may end.
pub fn light_at(model: &ArchModel, p: Vec2) -> Option<ArchId> {
    model
        .lights
        .iter()
        .filter_map(|l| light_plan(model, l.id).map(|at| (at.dist(p), l.id)))
        .filter(|(dist, _)| *dist <= PICK_RADIUS)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, id)| id)
}

/// A Connection drag released at `p`: connect `from` to the light there. With `shift` the light
/// is added to the connection the switch already has; without, a new connection is made.
pub fn connection_release(session: &mut Session, from: ArchId, p: Vec2, shift: bool) -> Result<Value, String> {
    let doc = session.doc().map_err(|e| e.to_string())?;
    let light = light_at(&doc.arch, p).ok_or("Release on a light")?;
    let existing = doc.arch.connections.iter().find(|c| c.from == from).map(|c| (c.id, c.to.clone()));
    match existing {
        Some((id, mut to)) if shift => {
            if !to.contains(&light) {
                to.push(light);
            }
            let to: Vec<String> = to.into_iter().map(light_label).collect();
            session.execute("connection.edit", &json!({ "id": connection_label(id), "to": to })).map_err(|e| e.to_string())
        }
        _ => session.execute("connection", &json!({ "from": electrical_label(from), "to": [light_label(light)] })).map_err(|e| e.to_string()),
    }
}

/// A grip at the midpoint of one of a connection's arcs.
#[derive(Clone, Debug, PartialEq)]
pub struct ArcGrip {
    /// The connection.
    pub connection: ArchId,
    /// The grip's plan position.
    pub at: Vec2,
    /// The arc's chord start (the switch).
    pub from: Vec2,
    /// The arc's chord end (the light).
    pub to: Vec2,
}

/// The grips of every connection, one per arc, in id order.
pub fn arc_grips(model: &ArchModel) -> Vec<ArcGrip> {
    let mut out = Vec::new();
    for c in &model.connections {
        let Some(from) = device_plan(model, c.from) else { continue };
        let mut to = c.to.clone();
        to.sort();
        for light in to.into_iter().filter_map(|l| light_plan(model, l)) {
            let line = bulged_polyline(from, light, c.bulge);
            let at = line.get(line.len() / 2).copied().unwrap_or_else(|| from.mid(light));
            out.push(ArcGrip { connection: c.id, at, from, to: light });
        }
    }
    out
}

/// The grip under plan point `p`.
pub fn grip_at(model: &ArchModel, p: Vec2) -> Option<ArcGrip> {
    arc_grips(model).into_iter().filter(|g| g.at.dist(p) <= PICK_RADIUS).min_by(|a, b| a.at.dist(p).total_cmp(&b.at.dist(p)))
}

/// The bulge that puts the arc `grip` belongs to through plan point `p`: the sagitta over half
/// the chord, positive to the left of switch→light, within -1..1.
pub fn bulge_through(grip: &ArcGrip, p: Vec2) -> Option<f64> {
    let chord = grip.to - grip.from;
    let half = chord.len() / 2.0;
    if !half.is_finite() || half < 1e-9 || !p.is_finite() {
        return None;
    }
    let dir = chord * (1.0 / chord.len());
    let sagitta = (p - grip.from.mid(grip.to)).dot(dir.perp());
    Some((sagitta / half).clamp(-1.0, 1.0))
}

/// A grip dragged to plan point `p`: `connection.edit {bulge}`.
pub fn grip_drag(session: &mut Session, grip: &ArcGrip, p: Vec2) -> Result<Value, String> {
    let bulge = bulge_through(grip, p).ok_or("That arc cannot be reshaped")?;
    session.execute("connection.edit", &json!({ "id": connection_label(grip.connection), "bulge": bulge })).map_err(|e| e.to_string())
}

/// Turn the Electrical View on, or off by restoring the layer state it saved.
pub fn toggle_view(session: &mut Session, tools: &mut ElectricalTools) -> Result<Value, String> {
    let r = if tools.view_on {
        session.execute("layerstate.restore", &json!({ "name": ELECTRICAL_STATE }))
    } else {
        session.execute("view.electrical", &json!({}))
    }
    .map_err(|e| e.to_string())?;
    tools.view_on = !tools.view_on;
    Ok(r)
}

/// The rows of the level picker: the label and the `level.show` parameter, "All levels" first.
pub fn level_choices(model: &ArchModel) -> Vec<(String, Value)> {
    let mut rows = vec![("All levels".to_string(), json!("all"))];
    let mut levels: Vec<_> = model.levels.iter().collect();
    levels.sort_by_key(|l| l.index);
    for l in levels {
        let label = if l.name.is_empty() { format!("Level {}", l.index) } else { l.name.clone() };
        rows.push((label, json!(l.index)));
    }
    rows
}

/// The row of [`level_choices`] showing now (0 = all levels).
pub fn level_selected(model: &ArchModel) -> usize {
    let rows = level_choices(model);
    model.plan_level.and_then(|i| rows.iter().position(|(_, v)| *v == json!(i))).unwrap_or(0)
}

/// Show one level (or all) in the plan: `level.show`.
pub fn show_level(session: &mut Session, level: &Value) -> Result<Value, String> {
    session.execute("level.show", &json!({ "level": level })).map_err(|e| e.to_string())
}

/// The names of the drawing's light sets, for the chooser.
pub fn light_set_names(session: &mut Session) -> Vec<String> {
    let Ok(v) = session.execute("lightset", &json!({ "list": true })) else { return Vec::new() };
    v.get("lightSets")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|s| s.get("name").and_then(Value::as_str).map(str::to_owned)).collect())
        .unwrap_or_default()
}

/// `render` parameters with the chosen light set added (`lightSet`) when there is one.
pub fn with_light_set(mut params: Value, tools: &ElectricalTools) -> Value {
    if let (Some(name), Some(o)) = (&tools.light_set, params.as_object_mut()) {
        o.insert("lightSet".into(), json!(name));
    }
    params
}

/// The light-set chooser: "Lights as they are" and each saved set.
pub fn light_set_chooser(ui: &mut egui::Ui, session: &mut Session, tools: &mut ElectricalTools) {
    let names = light_set_names(session);
    if names.is_empty() {
        tools.light_set = None;
        return;
    }
    if tools.light_set.as_ref().is_some_and(|n| !names.contains(n)) {
        tools.light_set = None;
    }
    let shown = tools.light_set.clone().unwrap_or_else(|| crate::i18n::t("Lights as they are").to_owned());
    let caption = ui.label(crate::i18n::t("Light set"));
    let combo = egui::ComboBox::from_id_salt("light-set").selected_text(shown).show_ui(ui, |ui| {
        ui.selectable_value(&mut tools.light_set, None, crate::i18n::t("Lights as they are"));
        for n in names {
            ui.selectable_value(&mut tools.light_set, Some(n.clone()), n);
        }
    });
    combo.response.labelled_by(caption.id);
}

/// The strip over the plan: the Electrical tools menu, the Electrical View toggle and the level
/// picker. Every action goes through [`crate::CadApp::run`].
pub fn plan_toolbar(app: &mut crate::CadApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        let rows = app.session.doc().map(|d| (level_choices(&d.arch), level_selected(&d.arch))).unwrap_or_default();
        if rows.0.len() > 1 {
            let mut chosen = rows.1;
            let text = rows.0.get(rows.1).map(|r| r.0.clone()).unwrap_or_default();
            let caption = ui.label(crate::i18n::t("Level"));
            let combo = egui::ComboBox::from_id_salt("level-picker").selected_text(text).show_ui(ui, |ui| {
                for (i, (label, _)) in rows.0.iter().enumerate() {
                    ui.selectable_value(&mut chosen, i, label.as_str());
                }
            });
            combo.response.labelled_by(caption.id);
            if chosen != rows.1
                && let Some((_, v)) = rows.0.get(chosen)
            {
                let _ = app.run("level.show", json!({ "level": v }));
            }
        }
        ui.menu_button(crate::i18n::t("Electrical"), |ui| {
            for info in TOOLS {
                let on = app.electrical.armed == Some(info.tool);
                if ui.selectable_label(on, crate::i18n::t(info.label)).clicked() {
                    app.electrical.armed = if on { None } else { Some(info.tool) };
                    ui.close();
                }
            }
        });
        let mut view = app.electrical.view_on;
        if ui.checkbox(&mut view, crate::i18n::t("Electrical View")).changed() {
            let mut tools = std::mem::take(&mut app.electrical);
            if let Err(e) = toggle_view(&mut app.session, &mut tools) {
                app.session.echo(e);
            }
            app.electrical = tools;
        }
    });
}

/// Draw the arc grips over the plan and run a drag of one: press on a grip picks it up, the
/// release reshapes the arc with `connection.edit {bulge}`. `true` when a press was taken.
pub fn plan_grips(app: &mut crate::CadApp, painter: &egui::Painter, xf: &crate::canvas::Xf, press: Option<Vec2>, release: Option<Vec2>) -> bool {
    let grips = app.session.doc().map(|d| arc_grips(&d.arch)).unwrap_or_default();
    for g in &grips {
        let c = xf.to_screen(g.at);
        let r = egui::Rect::from_center_size(c, egui::vec2(7.0, 7.0));
        painter.rect_filled(r, 1.0, egui::Color32::from_rgb(90, 160, 255));
    }
    let mut taken = false;
    if let Some(p) = press
        && app.electrical.armed.is_none()
    {
        app.electrical.grip = app.session.doc().ok().and_then(|d| grip_at(&d.arch, p));
        taken = app.electrical.grip.is_some();
    }
    if let Some(p) = release
        && let Some(g) = app.electrical.grip.take()
        && let Err(e) = grip_drag(&mut app.session, &g, p)
    {
        app.session.echo(e);
    }
    taken
}

/// Handle the plan pointer for the armed tool; `true` when the event was used.
/// `press`/`release` are plan points; `shift` adds lights to a connection.
pub fn plan_pointer(app: &mut crate::CadApp, press: Option<Vec2>, release: Option<Vec2>, shift: bool) -> bool {
    let Some(tool) = app.electrical.armed else {
        return false;
    };
    let result = match tool {
        Tool::Device(kind) => match press {
            Some(p) => Some(plan_place(&mut app.session, kind, p)),
            None => None,
        },
        Tool::Connection => {
            if let Some(p) = press {
                app.electrical.drag_from = app.session.doc().ok().and_then(|d| switch_at(&d.arch, p));
            }
            match (release, app.electrical.drag_from) {
                (Some(p), Some(from)) => {
                    app.electrical.drag_from = None;
                    Some(connection_release(&mut app.session, from, p, shift))
                }
                _ => None,
            }
        }
    };
    match result {
        Some(Ok(_)) => app.electrical.note.clear(),
        Some(Err(e)) => {
            app.electrical.note = e.clone();
            app.session.echo(e);
        }
        None => return press.is_some() || release.is_some(),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_doc::Wall;
    use cadcraft_doc::arch::{ElectricalDevice, Level, LightFixture};

    fn session_with_wall() -> Session {
        let mut s = Session::empty();
        s.new_drawing(false);
        if let Ok(doc) = s.doc_mut() {
            doc.arch.push_wall(Wall::new(Vec2::ZERO, Vec2::new(6.0, 0.0), 0.2));
        }
        s
    }

    #[test]
    fn electrical_tools_are_registered() {
        let kinds: Vec<_> = TOOLS.iter().filter_map(|t| if let Tool::Device(k) = t.tool { Some(k) } else { None }).collect();
        for k in [
            ElectricalKind::Duplex,
            ElectricalKind::Gfci,
            ElectricalKind::Outlet240,
            ElectricalKind::SinglePole,
            ElectricalKind::ThreeWay,
            ElectricalKind::Dimmer,
        ] {
            assert!(kinds.contains(&k), "no tool for {}", k.key());
        }
        assert!(TOOLS.iter().any(|t| t.tool == Tool::Connection));
        let mut ids: Vec<_> = TOOLS.iter().map(|t| t.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), TOOLS.len(), "tool ids are unique");

        // A click on a wall face runs the command for the kind: the side comes from the face.
        let mut s = session_with_wall();
        let r = plan_place(&mut s, ElectricalKind::Gfci, Vec2::new(2.0, -0.1)).expect("outlet");
        assert_eq!(r["side"], "right");
        assert_eq!(r["layer"], "E-POWR");
        let r = plan_place(&mut s, ElectricalKind::Dimmer, Vec2::new(3.0, 0.1)).expect("switch");
        assert_eq!(r["side"], "left");
        assert_eq!(r["layer"], "E-LITE");
        assert!(plan_place(&mut s, ElectricalKind::Duplex, Vec2::new(3.0, 5.0)).is_err(), "off the walls is refused");
    }

    #[test]
    fn the_level_picker_lists_levels() {
        let mut s = session_with_wall();
        if let Ok(doc) = s.doc_mut() {
            doc.arch.levels.clear();
            doc.arch.levels.push(Level { index: 0, name: "Ground".into(), ..Default::default() });
            doc.arch.levels.push(Level { index: 1, name: "Upper".into(), elevation: 2.7, ..Default::default() });
        }
        let model = s.doc().expect("doc").arch.clone();
        let rows = level_choices(&model);
        let labels: Vec<_> = rows.iter().map(|r| r.0.as_str()).collect();
        assert_eq!(labels, ["All levels", "Ground", "Upper"]);
        assert_eq!(level_selected(&model), 0);
        let r = show_level(&mut s, &rows[2].1).expect("level.show");
        assert_eq!(r["shown"], 1);
        let model = s.doc().expect("doc").arch.clone();
        assert_eq!(level_selected(&model), 2);
        show_level(&mut s, &rows[0].1).expect("all");
        assert_eq!(s.doc().expect("doc").arch.plan_level, None);
    }

    #[test]
    fn connection_drag_runs_connection() {
        let mut s = session_with_wall();
        let (switch, lights) = {
            let doc = s.doc_mut().expect("doc");
            let wall = doc.arch.walls[0].id;
            let switch = doc.arch.push_electrical(ElectricalDevice {
                wall,
                kind: ElectricalKind::SinglePole,
                side: WallSide::Left,
                offset: 1.0,
                height: 1.2,
                ..Default::default()
            });
            let mut lights = Vec::new();
            for i in 0..2u32 {
                doc.arch.lights.push(LightFixture::new(ArchId(i), "Recessed pot light", Vec2::new(2.0 + f64::from(i), 1.5)));
                lights.push(ArchId(i));
            }
            (switch, lights)
        };
        let doc = s.doc().expect("doc");
        let from = device_plan(&doc.arch, switch).expect("placed");
        assert_eq!(switch_at(&doc.arch, from), Some(switch));
        assert_eq!(switch_at(&doc.arch, Vec2::new(5.0, 5.0)), None);
        assert!(connection_release(&mut s, switch, Vec2::new(9.0, 9.0), false).is_err(), "no light there");

        let r = connection_release(&mut s, switch, Vec2::new(2.0, 1.5), false).expect("connection");
        assert_eq!(r["from"], electrical_label(switch));
        // Shift adds the second light to the same connection.
        connection_release(&mut s, switch, Vec2::new(3.0, 1.5), true).expect("edit");
        let doc = s.doc().expect("doc");
        assert_eq!(doc.arch.connections.len(), 1);
        assert_eq!(doc.arch.connections[0].to, lights);

        // The grip at an arc's midpoint reshapes it.
        let grips = arc_grips(&doc.arch);
        assert_eq!(grips.len(), 2);
        let g = grips[0].clone();
        let bowed = g.at + (g.to - g.from).normalized().perp() * 0.4;
        grip_drag(&mut s, &g, bowed).expect("connection.edit");
        let b = s.doc().expect("doc").arch.connections[0].bulge;
        assert!(b > 0.0 && b <= 1.0, "bulge {b}");
    }

    #[test]
    fn the_electrical_view_toggles_back() {
        let mut s = session_with_wall();
        let mut tools = ElectricalTools::default();
        toggle_view(&mut s, &mut tools).expect("on");
        assert!(tools.view_on);
        toggle_view(&mut s, &mut tools).expect("off");
        assert!(!tools.view_on);
        let p = with_light_set(json!({ "out": "a.png" }), &ElectricalTools { light_set: Some("Evening".into()), ..Default::default() });
        assert_eq!(p["lightSet"], "Evening");
    }
}
