//! The Library side panel: the nine category tiles, a folder tree, the results as a grid of icons
//! or a list, a search box with its chips, Back and Forward, saved filters, the Favorites star and
//! the details of the selected item. A thin view over the engine's `library`, `library.item`,
//! `library.icon`, `library.tag` and `library.filter` commands: it never reaches into the
//! library itself, so the native and web builds behave alike.

use std::collections::BTreeMap;

use egui::RichText;
use serde_json::{Value, json};

use crate::CadApp;

/// The tag the star toggles.
const FAVORITES: &str = "Favorites";
/// The chip kinds, as the `library` command names them.
const CHIP_KINDS: [&str; 7] = ["category", "folder", "symbolType", "placement", "source", "style", "tag"];
/// Icons requested per frame at most, so a long result list fills in over a few frames.
const ICONS_PER_FRAME: usize = 8;
/// Icon textures kept; the cache is emptied when it grows past this.
const MAX_TEXTURES: usize = 512;
/// Pixel size of the tile and result icons.
const ICON_PX: u64 = 64;

/// Icon textures by request key, with the icon's hash (a texture handle has no `Debug`).
#[derive(Clone, Default)]
struct Textures(BTreeMap<String, (String, egui::TextureHandle)>);

impl std::fmt::Debug for Textures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Textures({})", self.0.len())
    }
}

impl std::ops::Deref for Textures {
    type Target = BTreeMap<String, (String, egui::TextureHandle)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Textures {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The panel's state, kept in [`crate::UiState`] (never persisted).
#[derive(Clone, Debug, Default)]
pub struct State {
    /// The last `library` reply.
    reply: Option<Value>,
    /// The search box text.
    text: String,
    /// The chips of the query shown, as `(kind, value)`.
    chips: Vec<(String, String)>,
    /// The selected item and its `library.item` record.
    selected: Option<String>,
    detail: Option<Value>,
    /// Results as a list rather than a grid.
    list_mode: bool,
    /// The name typed for Save Filter, the saved filters' names.
    filter_name: String,
    filters: Vec<String>,
    filters_loaded: bool,
    /// The chip menu's kind and value.
    chip_kind: usize,
    chip_value: String,
    /// The last error, if any.
    note: String,
    /// Icon textures by request key (`category:<name>` or `item:<id>`) with the icon's hash.
    textures: Textures,
    /// Requests that failed, not repeated.
    no_icon: std::collections::BTreeSet<String>,
}

/// What a click asks for, run once the panel is drawn.
enum Act {
    Search,
    AddChip(String, String),
    RemoveChip(usize),
    Category(String),
    Folder(String),
    Back,
    Forward,
    Select(String),
    Star(String, bool),
    SaveFilter,
    ApplyFilter(String),
    DeleteFilter(String),
}

fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn list<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    v.get(key).and_then(Value::as_array).map_or(&[], Vec::as_slice)
}

/// Take the query a reply carries as the panel's text and chips.
fn adopt(st: &mut State, reply: Value) {
    if let Some(q) = reply.get("query") {
        st.text = str_of(q, "text").to_owned();
        st.chips = list(q, "chips").iter().map(|c| (str_of(c, "kind").to_owned(), str_of(c, "value").to_owned())).collect();
    }
    st.reply = Some(reply);
    st.note.clear();
}

/// Run `library` with the panel's text and chips.
fn search(app: &mut CadApp, st: &mut State) {
    let chips: Vec<Value> = st.chips.iter().map(|(k, v)| json!({"kind": k, "value": v})).collect();
    match app.run("library", json!({"text": st.text, "chips": chips})) {
        Ok(r) => adopt(st, r),
        Err(e) => st.note = e,
    }
}

fn step(app: &mut CadApp, st: &mut State, key: &str) {
    match app.run("library", json!({ key: true })) {
        Ok(r) => adopt(st, r),
        Err(e) => st.note = e,
    }
}

fn refresh_filters(app: &mut CadApp, st: &mut State) {
    if let Ok(r) = app.run("library.filter", json!({})) {
        st.filters = list(&r, "filters").iter().map(|f| str_of(f, "name").to_owned()).collect();
        st.filters_loaded = true;
    }
}

fn select(app: &mut CadApp, st: &mut State, id: &str) {
    match app.run("library.item", json!({"id": id})) {
        Ok(r) => {
            st.selected = Some(id.to_owned());
            st.detail = r.get("item").cloned();
        }
        Err(e) => st.note = e,
    }
}

/// The texture for an icon request, asking the engine for it the first time.
fn icon(app: &mut CadApp, st: &mut State, ctx: &egui::Context, key: &str, budget: &mut usize) -> Option<egui::TextureHandle> {
    if let Some((_, tex)) = st.textures.get(key) {
        return Some(tex.clone());
    }
    if st.no_icon.contains(key) || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let params = match key.split_once(':') {
        Some(("category", name)) => json!({"category": name, "size": ICON_PX}),
        Some(("item", id)) => json!({"item": id, "size": ICON_PX}),
        _ => return None,
    };
    let decoded = app.run("library.icon", params).ok().and_then(|r| {
        let hash = str_of(&r, "hash").to_owned();
        let png = cadcraft_engine::cmd::file::base64_decode(r.get("png")?.as_str()?)?;
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).ok()?.to_rgba8();
        Some((hash, img))
    });
    let Some((hash, img)) = decoded else {
        st.no_icon.insert(key.to_owned());
        return None;
    };
    // Same hash as another request: share its texture.
    if let Some((_, tex)) = st.textures.values().find(|(h, _)| *h == hash) {
        let tex = tex.clone();
        st.textures.insert(key.to_owned(), (hash, tex.clone()));
        return Some(tex);
    }
    if st.textures.len() >= MAX_TEXTURES {
        st.textures.clear();
    }
    let size = [img.width() as usize, img.height() as usize];
    let tex = ctx.load_texture(format!("library-{key}"), egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()), egui::TextureOptions::LINEAR);
    st.textures.insert(key.to_owned(), (hash, tex.clone()));
    Some(tex)
}

/// A button with an optional icon above its caption; `caption` is also its accessible name.
fn tile(ui: &mut egui::Ui, tex: Option<&egui::TextureHandle>, caption: &str, selected: bool, width: f32) -> egui::Response {
    let t = crate::theme::Tokens::get();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 84.0), egui::Sense::click());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), caption));
    if ui.is_rect_visible(rect) {
        let visuals = ui.style().interact_selectable(&response, selected);
        ui.painter().rect(rect, 3.0, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
        if let Some(tex) = tex {
            let img = egui::Rect::from_center_size(egui::pos2(rect.center().x, rect.min.y + 24.0), egui::vec2(40.0, 40.0));
            ui.painter().image(tex.id(), img, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        let galley = ui.painter().layout(caption.to_owned(), crate::theme::body(), t.text, width - 6.0);
        let pos = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.min.y + 46.0);
        ui.painter().with_clip_rect(rect).galley(pos, galley, t.text);
    }
    response
}

fn size_text(item: &Value) -> String {
    match item.get("size").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<_>>()) {
        Some(s) if s.len() == 3 => format!("{:.2} × {:.2} × {:.2} m", s[0], s[1], s[2]),
        _ => String::new(),
    }
}

fn is_favorite(item: &Value) -> bool {
    list(item, "tags").iter().any(|t| t.as_str() == Some(FAVORITES))
}

fn categories(app: &mut CadApp, st: &mut State, ui: &mut egui::Ui, act: &mut Option<Act>, budget: &mut usize) {
    let Some(reply) = st.reply.clone() else { return };
    let active = st.chips.iter().find(|(k, _)| k == "category").map(|(_, v)| v.clone());
    let width = ((ui.available_width() - 32.0) / 3.0).max(60.0);
    egui::Grid::new("library-categories").num_columns(3).spacing([4.0, 4.0]).show(ui, |ui| {
        for (i, c) in list(&reply, "categories").iter().enumerate() {
            let name = str_of(c, "name");
            let tex = icon(app, st, ui.ctx(), &format!("category:{name}"), budget);
            let count = c.get("items").and_then(Value::as_u64).unwrap_or(0);
            let r = tile(ui, tex.as_ref(), name, active.as_deref() == Some(name), width).on_hover_text(format!("{count}"));
            if r.clicked() {
                *act = Some(Act::Category(name.to_owned()));
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
}

fn folders(ui: &mut egui::Ui, st: &State, act: &mut Option<Act>) {
    let Some(reply) = st.reply.as_ref() else { return };
    let folders = list(reply, "folders");
    if folders.is_empty() {
        return;
    }
    let active = st.chips.iter().find(|(k, _)| k == "folder").map(|(_, v)| v.as_str());
    ui.add_space(4.0);
    ui.label(RichText::new(crate::tl!("Folders")).strong());
    for f in folders {
        let name = str_of(f, "name");
        let n = f.get("items").and_then(Value::as_u64).unwrap_or(0);
        if ui.selectable_label(active == Some(name), format!("{name} ({n})")).clicked() {
            *act = Some(Act::Folder(name.to_owned()));
        }
    }
}

fn results(app: &mut CadApp, st: &mut State, ui: &mut egui::Ui, act: &mut Option<Act>, budget: &mut usize) {
    let t = crate::theme::Tokens::get();
    let Some(reply) = st.reply.clone() else { return };
    let items = list(&reply, "items");
    if items.is_empty() {
        ui.label(RichText::new(crate::tl!("No items match")).color(t.text_dim));
        return;
    }
    if st.list_mode {
        for item in items {
            let id = str_of(item, "id");
            ui.horizontal(|ui| {
                star(ui, item, act);
                let on = st.selected.as_deref() == Some(id);
                if ui.selectable_label(on, str_of(item, "name")).clicked() {
                    *act = Some(Act::Select(id.to_owned()));
                }
                ui.label(RichText::new(str_of(item, "category")).color(t.text_dim));
                ui.label(RichText::new(size_text(item)).color(t.text_dim));
            });
        }
        return;
    }
    let width = ((ui.available_width() - 32.0) / 3.0).max(60.0);
    ui.horizontal_wrapped(|ui| {
        for item in items {
            let id = str_of(item, "id");
            let tex = icon(app, st, ui.ctx(), &format!("item:{id}"), budget);
            ui.vertical(|ui| {
                let on = st.selected.as_deref() == Some(id);
                if tile(ui, tex.as_ref(), str_of(item, "name"), on, width).clicked() {
                    *act = Some(Act::Select(id.to_owned()));
                }
                star(ui, item, act);
            });
        }
    });
}

fn star(ui: &mut egui::Ui, item: &Value, act: &mut Option<Act>) {
    let on = is_favorite(item);
    let r = ui.add(egui::Button::new(if on { "★" } else { "☆" }).frame(false)).on_hover_text(crate::tl!("Favorites"));
    if r.clicked() {
        *act = Some(Act::Star(str_of(item, "id").to_owned(), !on));
    }
}

fn details(st: &State, ui: &mut egui::Ui, act: &mut Option<Act>) {
    let t = crate::theme::Tokens::get();
    let Some(item) = st.detail.as_ref() else { return };
    ui.separator();
    ui.label(RichText::new(crate::tl!("Details")).strong());
    ui.heading(str_of(item, "name"));
    let row = |ui: &mut egui::Ui, label: &str, value: &str| {
        if !value.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{label}:")).color(t.text_dim));
                ui.label(value);
            });
        }
    };
    row(ui, crate::tl!("Category"), str_of(item, "category"));
    row(ui, crate::tl!("Folder"), str_of(item, "folder"));
    row(ui, crate::tl!("Size"), &size_text(item));
    row(ui, crate::tl!("Source"), str_of(item, "source"));
    row(ui, crate::tl!("Licence"), str_of(item, "licence"));
    row(ui, crate::tl!("Author"), str_of(item, "author"));
    row(ui, crate::tl!("Attribution"), str_of(item, "attribution"));
    if str_of(item, "catalog") == "user" {
        row(ui, crate::tl!("Notes"), str_of(item, "notes"));
    }
    // Chips this item offers.
    let mut offers: Vec<(&str, String)> = Vec::new();
    for (kind, key) in [("symbolType", "symbolType"), ("placement", "placement"), ("style", "style")] {
        let v = str_of(item, key);
        if !v.is_empty() {
            offers.push((kind, v.to_owned()));
        }
    }
    let catalog = str_of(item, "catalog");
    if !catalog.is_empty() {
        offers.push(("source", catalog.split(':').next().unwrap_or(catalog).to_owned()));
    }
    offers.extend(list(item, "tags").iter().filter_map(Value::as_str).map(|tag| ("tag", tag.to_owned())));
    ui.label(RichText::new(crate::tl!("Tags")).color(t.text_dim));
    ui.horizontal_wrapped(|ui| {
        for (kind, value) in offers {
            if ui.small_button(format!("+ {kind}: {value}")).clicked() {
                *act = Some(Act::AddChip(kind.to_owned(), value));
            }
        }
    });
}

/// The panel body.
pub fn section(app: &mut CadApp, ui: &mut egui::Ui) {
    let t = crate::theme::Tokens::get();
    let mut st = std::mem::take(&mut app.ui.library);
    if st.reply.is_none() {
        search(app, &mut st);
    }
    if !st.filters_loaded {
        refresh_filters(app, &mut st);
    }
    let mut act: Option<Act> = None;
    let mut budget = ICONS_PER_FRAME;
    let (can_back, can_forward) = st
        .reply
        .as_ref()
        .map_or((false, false), |r| (r.get("back").and_then(Value::as_bool) == Some(true), r.get("forward").and_then(Value::as_bool) == Some(true)));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        if ui.add_enabled(can_back, egui::Button::new(crate::tl!("Back"))).clicked() {
            act = Some(Act::Back);
        }
        if ui.add_enabled(can_forward, egui::Button::new(crate::tl!("Forward"))).clicked() {
            act = Some(Act::Forward);
        }
        let label = if st.list_mode { crate::tl!("Grid") } else { crate::tl!("List") };
        if ui.button(label).clicked() {
            st.list_mode = !st.list_mode;
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        let edit = ui.add(egui::TextEdit::singleline(&mut st.text).hint_text(crate::tl!("Search")).desired_width(ui.available_width() - 8.0));
        let applied = st.reply.as_ref().and_then(|r| r.get("query")).map(|q| str_of(q, "text").to_owned()).unwrap_or_default();
        if edit.lost_focus() && st.text != applied {
            act = Some(Act::Search);
        }
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0);
        for (i, (kind, value)) in st.chips.iter().enumerate() {
            if ui.small_button(format!("{kind}: {value} ×")).clicked() {
                act = Some(Act::RemoveChip(i));
            }
        }
        ui.menu_button("+", |ui| {
            egui::ComboBox::from_id_salt("library-chip-kind").selected_text(CHIP_KINDS[st.chip_kind.min(CHIP_KINDS.len() - 1)]).show_ui(ui, |ui| {
                for (i, k) in CHIP_KINDS.iter().enumerate() {
                    ui.selectable_value(&mut st.chip_kind, i, *k);
                }
            });
            ui.text_edit_singleline(&mut st.chip_value);
            if ui.button(crate::tl!("Add")).clicked() && !st.chip_value.trim().is_empty() {
                let kind = CHIP_KINDS[st.chip_kind.min(CHIP_KINDS.len() - 1)].to_owned();
                act = Some(Act::AddChip(kind, st.chip_value.trim().to_owned()));
                st.chip_value.clear();
                ui.close();
            }
        });
    });
    if !st.note.is_empty() {
        ui.label(RichText::new(st.note.as_str()).color(t.text_dim));
    }
    ui.add_space(4.0);
    categories(app, &mut st, ui, &mut act, &mut budget);
    folders(ui, &st, &mut act);
    ui.add_space(4.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(crate::tl!("Filters")).strong());
        for name in st.filters.clone() {
            if ui.small_button(&name).clicked() {
                act = Some(Act::ApplyFilter(name.clone()));
            }
            if ui.small_button(format!("× {name}")).on_hover_text(crate::tl!("Delete")).clicked() {
                act = Some(Act::DeleteFilter(name));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.add_space(8.0);
        ui.add(egui::TextEdit::singleline(&mut st.filter_name).desired_width(120.0).id_salt("library-filter-name"));
        if ui.add_enabled(!st.filter_name.trim().is_empty(), egui::Button::new(crate::tl!("Save"))).clicked() {
            act = Some(Act::SaveFilter);
        }
    });
    ui.separator();
    results(app, &mut st, ui, &mut act, &mut budget);
    details(&st, ui, &mut act);
    if budget < ICONS_PER_FRAME || st.reply.as_ref().is_some_and(|r| !list(r, "items").is_empty() && budget == 0) {
        ui.ctx().request_repaint();
    }

    match act {
        Some(Act::Search) => search(app, &mut st),
        Some(Act::AddChip(kind, value)) => {
            if !st.chips.iter().any(|c| c.0 == kind && c.1 == value) {
                st.chips.push((kind, value));
            }
            search(app, &mut st);
        }
        Some(Act::RemoveChip(i)) => {
            if i < st.chips.len() {
                st.chips.remove(i);
            }
            search(app, &mut st);
        }
        Some(Act::Category(name)) => {
            st.chips.retain(|(k, _)| k != "category" && k != "folder");
            st.chips.push(("category".into(), name));
            search(app, &mut st);
        }
        Some(Act::Folder(name)) => {
            st.chips.retain(|(k, _)| k != "folder");
            st.chips.push(("folder".into(), name));
            search(app, &mut st);
        }
        Some(Act::Back) => step(app, &mut st, "back"),
        Some(Act::Forward) => step(app, &mut st, "forward"),
        Some(Act::Select(id)) => select(app, &mut st, &id),
        Some(Act::Star(id, on)) => match app.run("library.tag", json!({"item": id, "tag": FAVORITES, "on": on})) {
            Ok(_) => {
                search_keep(app, &mut st);
                if st.selected.as_deref() == Some(id.as_str()) {
                    select(app, &mut st, &id);
                }
            }
            Err(e) => st.note = e,
        },
        Some(Act::SaveFilter) => {
            let name = st.filter_name.trim().to_owned();
            match app.run("library.filter", json!({"save": name})) {
                Ok(_) => {
                    st.filter_name.clear();
                    refresh_filters(app, &mut st);
                }
                Err(e) => st.note = e,
            }
        }
        Some(Act::ApplyFilter(name)) => match app.run("library.filter", json!({"apply": name})) {
            Ok(r) => adopt(&mut st, r),
            Err(e) => st.note = e,
        },
        Some(Act::DeleteFilter(name)) => match app.run("library.filter", json!({"delete": name})) {
            Ok(_) => refresh_filters(app, &mut st),
            Err(e) => st.note = e,
        },
        None => {}
    }
    app.ui.library = st;
}

/// Re-run the shown query without it counting as a new search when it is unchanged (the
/// engine's history ignores a repeat of the current entry).
fn search_keep(app: &mut CadApp, st: &mut State) {
    search(app, st);
}

#[cfg(test)]
mod tests {
    use super::*;
    use cadcraft_engine::Session;
    use egui_kittest::{Harness, kittest::Queryable};

    fn app() -> CadApp {
        // The apps install the library's readers; so does the test.
        cadcraft_engine::library::set_library_io(cadcraft_engine::library::LibraryIo {
            read_glb: cadcraft_io::gltf_read::read_glb,
            read_image: cadcraft_io::image_read::read_image,
            encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
            sample: cadcraft_io::library::sample_files,
        });
        let mut session = Session::new();
        // Everything in memory: the test never touches a real user dir.
        session.library = cadcraft_engine::library::Library::with_user_dir(None);
        CadApp::new(session, crate::Services::default())
    }

    fn harness() -> Harness<'static, CadApp> {
        let mut h = Harness::builder().with_size(egui::vec2(420.0, 1400.0)).build_ui_state(
            |ui, app: &mut CadApp| {
                egui::ScrollArea::vertical().show(ui, |ui| section(app, ui));
            },
            app(),
        );
        h.run_steps(3);
        h
    }

    fn total(h: &Harness<'_, CadApp>) -> u64 {
        h.state().ui.library.reply.as_ref().and_then(|r| r.get("total")).and_then(Value::as_u64).unwrap_or(u64::MAX)
    }

    fn search_for(h: &mut Harness<'_, CadApp>, text: &str) {
        // The search box is the first text input.
        if let Some(edit) = h.get_all_by_role(egui::accesskit::Role::TextInput).next() {
            edit.click();
        }
        h.run_steps(2);
        if let Some(edit) = h.get_all_by_role(egui::accesskit::Role::TextInput).next() {
            edit.type_text(text);
        }
        h.run_steps(2);
        h.key_press(egui::Key::Enter);
        h.run_steps(3);
    }

    #[test]
    fn library_panel_shows_nine_categories() {
        let h = harness();
        for c in cadcraft_arch::library::Category::ALL {
            assert!(h.query_by_label(c.name()).is_some(), "no tile for {}", c.name());
        }
        assert!(total(&h) > 0);
    }

    #[test]
    fn library_panel_search_and_chips() {
        let mut h = harness();
        let all = total(&h);
        search_for(&mut h, "chair");
        let chairs = total(&h);
        assert!(chairs > 0 && chairs < all, "chair: {chairs} of {all}");
        // The chip comes from an item's details: select a chair, add its symbol type.
        let first = list(h.state().ui.library.reply.as_ref().unwrap_or(&Value::Null), "items").first().cloned().unwrap_or_default();
        let id = str_of(&first, "id").to_owned();
        h.get_by_label(str_of(&first, "name")).click();
        h.run_steps(3);
        assert_eq!(h.state().ui.library.selected.as_deref(), Some(id.as_str()));
        let symbol = str_of(&first, "symbolType").to_owned();
        h.get_by_label(&format!("+ symbolType: {symbol}")).click();
        h.run_steps(3);
        let with_chip = total(&h);
        assert!(with_chip > 0 && with_chip <= chairs);
        assert!(list(h.state().ui.library.reply.as_ref().unwrap_or(&Value::Null), "items").iter().all(|i| str_of(i, "symbolType") == symbol));
        h.get_by_label(&format!("symbolType: {symbol} ×")).click();
        h.run_steps(3);
        assert!(h.state().ui.library.chips.is_empty());
        assert_eq!(total(&h), chairs, "removing the chip widens the results again");
        // Nothing matches: a valid answer.
        search_for(&mut h, "zzzzqq");
        assert_eq!(total(&h), 0);
        assert!(h.query_by_label("No items match").is_some());
    }

    #[test]
    fn library_panel_back_and_forward() {
        let mut h = harness();
        search_for(&mut h, "chair");
        let chairs = total(&h);
        h.get_by_label("Plants").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.library.chips, vec![("category".to_owned(), "Plants".to_owned())]);
        let plants = total(&h);
        h.get_by_label("Back").click();
        h.run_steps(3);
        assert_eq!(total(&h), chairs);
        assert!(h.state().ui.library.chips.is_empty());
        assert_eq!(h.state().ui.library.text, "chair");
        h.get_by_label("Forward").click();
        h.run_steps(3);
        assert_eq!(total(&h), plants);
    }

    #[test]
    fn library_panel_details() {
        let mut h = harness();
        h.get_by_label("Doors and windows").click();
        h.run_steps(3);
        assert_eq!(h.state().ui.library.chips.len(), 1, "the category chip is set: {}", h.state().ui.library.note);
        let first = list(h.state().ui.library.reply.as_ref().unwrap_or(&Value::Null), "items").first().cloned().unwrap_or_default();
        h.get_by_label(str_of(&first, "name")).click();
        h.run_steps(3);
        assert!(h.query_by_label("Details").is_some());
        assert!(h.query_by_label("CC0-1.0").is_some(), "the licence is shown");
        assert!(h.query_by_label("built in").is_some(), "the source is shown: {:?}", h.state().ui.library.detail);
        assert!(h.query_by_label("CADCraft").is_some(), "the author is shown");
        // The Favorites star toggles through library.tag.
        let id = str_of(&first, "id").to_owned();
        let _ = h.state_mut().run("library.tag", json!({"item": id, "tag": FAVORITES, "on": true}));
        h.run_steps(2);
    }
}
