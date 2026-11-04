//! The library (phase 14): one set of items from every catalog — built in, the embedded sample
//! set, installed catalogs, local folders of .glb files and the user's own items — with tags,
//! saved filters and the search history. [`Session::library`](crate::Session) holds it.
//!
//! The file readers (glTF, PNG/JPEG, the PNG encoder and the sample set) live in lower layers the
//! engine does not depend on; the apps install them with [`set_library_io`], as they install the
//! drawing readers with `cmd::file::set_io`. Without them the library lists the built-in items
//! only, and installing or adding a local catalog is refused.
//!
//! THE USER DIR is `CADCRAFT_LIBRARY_USER_DIR`, else `$XDG_DATA_HOME/cadcraft/library`, else
//! `~/.local/share/cadcraft/library` (native only; the web build keeps everything in memory). It
//! holds `state.json` (tags, saved filters, local catalogs), `installed/<name>/` (catalog.json and
//! its files), `user/<slug>.json`, `trash/<slug>.json` and `icons/<hash>.png`. Writes are atomic.
//! The crate's own unit tests never look at the real one: they pass a temporary folder to
//! [`Library::with_user_dir`].

pub mod catalog;
pub mod user;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::OnceLock;

use cadcraft_arch::library::{Category, Chip, ChipKind, FilterHistory, ItemKind, ItemRecord, Query};
use cadcraft_doc::arch::{ImageRgba, MAX_SYMBOL_TRIANGLES, SymbolMesh};
use serde::Serialize;
use serde_json::Value;

pub use catalog::{Installed, Skipped};

/// The library's file readers, installed by the app.
pub struct LibraryIo {
    /// A glTF 2.0 binary (.glb) as a symbol mesh.
    pub read_glb: fn(&[u8]) -> Result<SymbolMesh, String>,
    /// A PNG or JPEG, its longer side reduced to at most the given number of pixels.
    pub read_image: fn(&[u8], u32) -> Result<ImageRgba, String>,
    /// An image as PNG bytes (empty on failure).
    pub encode_png: fn(&ImageRgba) -> Vec<u8>,
    /// The embedded sample set: `catalog.json` and the files it names.
    pub sample: fn() -> &'static [(&'static str, &'static [u8])],
}

static IO: OnceLock<LibraryIo> = OnceLock::new();

/// Install the file readers (the first call wins).
pub fn set_library_io(io: LibraryIo) {
    let _ = IO.set(io);
}

/// The installed file readers, if any.
pub fn library_io() -> Option<&'static LibraryIo> {
    IO.get()
}

/// What a build without the readers says when one is needed.
pub const NO_READERS: &str = "the library's file readers are not available in this build";

/// The longest tag or saved filter name (characters).
pub const MAX_TAG: usize = 64;
/// The built-in tag every listing offers.
pub const FAVORITES: &str = "Favorites";
/// Most tags on one item.
const MAX_TAGS_PER_ITEM: usize = 64;
/// Most tagged items.
const MAX_TAGGED_ITEMS: usize = 100_000;
/// Most saved filters.
const MAX_FILTERS: usize = 256;
/// Most local catalogs.
#[cfg(not(target_arch = "wasm32"))]
const MAX_LOCAL_CATALOGS: usize = 64;
/// Longest search text (characters).
pub const MAX_QUERY_TEXT: usize = 1000;
/// Most chips in one query.
pub const MAX_CHIPS: usize = 32;
/// Longest chip value (characters).
const MAX_CHIP_VALUE: usize = 256;

/// The user dir from the environment `var` gives: `CADCRAFT_LIBRARY_USER_DIR`, else
/// `$XDG_DATA_HOME/cadcraft/library` (an absolute one), else `$HOME/.local/share/cadcraft/library`
/// (on Windows `%APPDATA%\CADCraft\library` when there is no HOME). An empty variable is unset.
pub fn user_dir_from(var: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let set = |k: &str| var(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    set("CADCRAFT_LIBRARY_USER_DIR")
        .or_else(|| set("XDG_DATA_HOME").filter(|p| p.is_absolute()).map(|p| p.join("cadcraft").join("library")))
        .or_else(|| set("HOME").map(|h| h.join(".local").join("share").join("cadcraft").join("library")))
        .or_else(|| if cfg!(windows) { set("APPDATA").map(|a| a.join("CADCraft").join("library")) } else { None })
}

/// The user dir a new [`Library`] uses: from the process environment natively; none on the web
/// and in this crate's unit tests (which pass their own folder to [`Library::with_user_dir`]).
pub fn default_user_dir() -> Option<PathBuf> {
    if cfg!(any(test, target_arch = "wasm32")) { None } else { user_dir_from(|k| std::env::var_os(k)) }
}

/// A saved filter.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SavedFilter {
    pub name: String,
    pub query: Query,
}

/// A remembered local catalog.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct LocalCatalog {
    pub path: String,
    pub licence: String,
    pub category: String,
}

/// state.json: tags by item id, saved filters by name, local catalogs in the order added.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
struct State {
    tags: BTreeMap<String, Vec<String>>,
    filters: Vec<SavedFilter>,
    local: Vec<LocalCatalog>,
}

/// A tag or saved filter name: non-empty, at most [`MAX_TAG`] characters, no control characters
/// and no space at either end.
pub fn check_name(what: &str, s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err(format!("the {what} is empty"));
    }
    if s.chars().count() > MAX_TAG {
        return Err(format!("the {what} is longer than {MAX_TAG} characters"));
    }
    if s.chars().any(char::is_control) {
        return Err(format!("the {what} has control characters"));
    }
    if s.trim() != s {
        return Err(format!("the {what} starts or ends with a space"));
    }
    Ok(())
}

/// A query from its JSON parts: `text` a string (at most [`MAX_QUERY_TEXT`] characters), `chips`
/// a list (at most [`MAX_CHIPS`]) of `{kind, value}` objects with a known kind and a non-empty
/// string value. Anything else is refused.
pub fn parse_query(text: Option<&Value>, chips: Option<&Value>) -> Result<Query, String> {
    let text = match text {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(t)) if t.chars().count() <= MAX_QUERY_TEXT => t.clone(),
        Some(Value::String(_)) => return Err(format!("`text` is longer than {MAX_QUERY_TEXT} characters")),
        Some(_) => return Err("`text` must be a string".into()),
    };
    let mut out = Vec::new();
    match chips {
        None | Some(Value::Null) => {}
        Some(Value::Array(list)) => {
            if list.len() > MAX_CHIPS {
                return Err(format!("more than {MAX_CHIPS} chips"));
            }
            for c in list {
                let Some(obj) = c.as_object() else { return Err("each chip must be an object {kind, value}".into()) };
                let kind = obj.get("kind").and_then(Value::as_str).ok_or("a chip's `kind` must be a string")?;
                let kind =
                    ChipKind::parse(kind).ok_or_else(|| format!("unknown chip kind `{}`", kind.chars().take(MAX_CHIP_VALUE).collect::<String>()))?;
                let value = obj.get("value").and_then(Value::as_str).ok_or("a chip's `value` must be a string")?;
                if value.is_empty() {
                    return Err("a chip's `value` is empty".into());
                }
                if value.chars().count() > MAX_CHIP_VALUE {
                    return Err(format!("a chip's `value` is longer than {MAX_CHIP_VALUE} characters"));
                }
                out.push(Chip { kind, value: value.to_string(), ..Default::default() });
            }
        }
        Some(_) => return Err("`chips` must be a list".into()),
    }
    Ok(Query { text, chips: out, ..Default::default() })
}

/// Read only from a native user dir (the web build keeps the state in memory).
#[cfg(not(target_arch = "wasm32"))]
impl State {
    /// state.json, checked entry by entry: whatever is malformed is dropped, the rest kept.
    fn from_json(v: &Value) -> State {
        let mut st = State::default();
        if let Some(tags) = v.get("tags").and_then(Value::as_object) {
            for (item, list) in tags.iter().take(MAX_TAGGED_ITEMS) {
                if item.is_empty() || item.chars().count() > 256 {
                    continue;
                }
                let mut set = BTreeSet::new();
                for t in list.as_array().map(Vec::as_slice).unwrap_or_default() {
                    if let Some(t) = t.as_str().filter(|t| check_name("tag", t).is_ok()) {
                        set.insert(t.to_string());
                    }
                }
                if !set.is_empty() {
                    st.tags.insert(item.clone(), set.into_iter().take(MAX_TAGS_PER_ITEM).collect());
                }
            }
        }
        if let Some(filters) = v.get("filters").and_then(Value::as_array) {
            for f in filters {
                let Some(name) = f.get("name").and_then(Value::as_str).filter(|n| check_name("filter name", n).is_ok()) else { continue };
                let Some(q) = f.get("query") else { continue };
                let Ok(query) = parse_query(q.get("text"), q.get("chips")) else { continue };
                if st.filters.len() < MAX_FILTERS && !st.filters.iter().any(|s| s.name == name) {
                    st.filters.push(SavedFilter { name: name.to_string(), query });
                }
            }
            st.filters.sort_by(|a, b| a.name.cmp(&b.name));
        }
        if let Some(local) = v.get("local").and_then(Value::as_array) {
            for l in local {
                let path = l.get("path").and_then(Value::as_str).filter(|p| !p.is_empty());
                let licence = l.get("licence").and_then(Value::as_str).filter(|p| !p.trim().is_empty());
                let category = l.get("category").and_then(Value::as_str).unwrap_or(Category::Furniture.name());
                let ok = Category::parse(category).is_some_and(|c| c != Category::User);
                if let (Some(path), Some(licence), true) = (path, licence, ok)
                    && st.local.len() < MAX_LOCAL_CATALOGS
                    && !st.local.iter().any(|x| x.path == path)
                {
                    st.local.push(LocalCatalog { path: path.to_string(), licence: licence.to_string(), category: category.to_string() });
                }
            }
        }
        st
    }
}

/// Every catalog's items, with tags, saved filters and the search history.
pub struct Library {
    user_dir: Option<PathBuf>,
    state: State,
    /// Every catalog merged, sorted by id (an id taken by an earlier catalog drops a later one:
    /// built in, sample, installed, local, user).
    items: Vec<ItemRecord>,
    sample: Vec<ItemRecord>,
    installed: Vec<ItemRecord>,
    /// Each remembered local catalog's items, by its path.
    locals: Vec<(String, Vec<ItemRecord>)>,
    /// The user dir's user items.
    user: Vec<ItemRecord>,
    /// User items added in memory ([`Library::add_user_item`]; the only store on the web).
    added: Vec<ItemRecord>,
    symbols: BTreeMap<String, SymbolMesh>,
    images: BTreeMap<String, ImageRgba>,
    /// Whether the readers were installed when the catalogs were last read.
    io_seen: bool,
    /// The search history `library` pushes and Back and Forward step.
    pub history: FilterHistory,
}

impl Default for Library {
    fn default() -> Self {
        Library::new()
    }
}

impl Library {
    /// The library with the [`default_user_dir`].
    pub fn new() -> Library {
        Library::with_user_dir(default_user_dir())
    }

    /// The library with its user dir at `dir` (`None`: nothing is read or written, everything
    /// stays in memory). Reads every catalog.
    pub fn with_user_dir(dir: Option<PathBuf>) -> Library {
        let mut lib = Library {
            user_dir: dir,
            state: State::default(),
            items: Vec::new(),
            sample: Vec::new(),
            installed: Vec::new(),
            locals: Vec::new(),
            user: Vec::new(),
            added: Vec::new(),
            symbols: BTreeMap::new(),
            images: BTreeMap::new(),
            io_seen: false,
            history: FilterHistory::default(),
        };
        lib.reload();
        lib
    }

    /// Every item of every catalog, sorted by id.
    pub fn items(&self) -> &[ItemRecord] {
        &self.items
    }

    /// The item `id`.
    pub fn item(&self, id: &str) -> Option<&ItemRecord> {
        self.items.binary_search_by(|r| r.id.as_str().cmp(id)).ok().and_then(|i| self.items.get(i))
    }

    /// Mutable access to the item `id` in the merged item list.
    pub fn item_mut(&mut self, id: &str) -> Option<&mut ItemRecord> {
        let idx = self.items.binary_search_by(|r| r.id.as_str().cmp(id)).ok()?;
        self.items.get_mut(idx)
    }

    /// The user dir, if there is one.
    pub fn user_dir(&self) -> Option<PathBuf> {
        self.user_dir.clone()
    }

    /// Re-read state.json and the sample, installed, local and user catalogs (the symbol and image
    /// caches are emptied: their files may have changed).
    pub fn reload(&mut self) {
        let io = library_io();
        self.io_seen = io.is_some();
        self.state = self.read_state();
        self.sample = io.map(|io| catalog::sample_items((io.sample)())).unwrap_or_default();
        self.read_dir_catalogs(io);
        self.symbols.clear();
        self.images.clear();
        self.rebuild();
    }

    /// Re-read the catalogs if the readers were installed since they were last read (an app that
    /// made its session before installing them still gets the sample set).
    pub fn sync_io(&mut self) {
        if library_io().is_some() != self.io_seen {
            self.reload();
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_state(&self) -> State {
        let Some(dir) = &self.user_dir else { return State::default() };
        let path = dir.join("state.json");
        if std::fs::symlink_metadata(&path).is_err() {
            return State::default();
        }
        match crate::capped_read::read_capped(&path, catalog::MAX_CATALOG_BYTES)
            .map_err(|e| e.to_string())
            .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string()))
        {
            Ok(v) => State::from_json(&v),
            Err(e) => {
                log::warn!("library: {}: {e}", path.display());
                State::default()
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn read_state(&self) -> State {
        self.state.clone()
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn read_dir_catalogs(&mut self, io: Option<&LibraryIo>) {
        let Some(dir) = self.user_dir.clone() else {
            self.installed.clear();
            self.user.clear();
            self.locals.clear();
            return;
        };
        self.installed = catalog::installed_items(&dir);
        self.user = catalog::user_items(&dir);
        self.locals = match io {
            Some(io) => self
                .state
                .local
                .iter()
                .map(|l| {
                    let category = Category::parse(&l.category).unwrap_or(Category::Furniture);
                    let items = catalog::local_items(io, std::path::Path::new(&l.path), &l.licence, category).map(|(items, _)| items);
                    (
                        l.path.clone(),
                        items.unwrap_or_else(|e| {
                            log::warn!("library: local catalog {}: {e}", l.path);
                            Vec::new()
                        }),
                    )
                })
                .collect(),
            None => Vec::new(),
        };
    }

    #[cfg(target_arch = "wasm32")]
    fn read_dir_catalogs(&mut self, _io: Option<&LibraryIo>) {}

    /// Merge the catalogs into `items`: earlier catalogs keep a contested id, tags come from
    /// state.json, the result is sorted by id.
    fn rebuild(&mut self) {
        let mut seen = BTreeSet::new();
        let mut items = Vec::new();
        let locals = self.locals.iter().flat_map(|(_, items)| items.iter());
        for r in catalog::builtin_items()
            .into_iter()
            .chain(self.sample.iter().cloned())
            .chain(self.installed.iter().cloned())
            .chain(locals.cloned())
            .chain(self.user.iter().cloned())
            .chain(self.added.iter().cloned())
        {
            if seen.insert(r.id.clone()) {
                items.push(r);
            }
        }
        for r in &mut items {
            r.tags = self.state.tags.get(&r.id).cloned().unwrap_or_default();
        }
        items.sort_by(|a, b| a.id.cmp(&b.id));
        self.items = items;
    }

    /// Add (or replace, by id) a user item kept in memory: catalog `user`, editable.
    pub fn add_user_item(&mut self, mut rec: ItemRecord) {
        rec.catalog = "user".to_string();
        rec.read_only = false;
        self.added.retain(|r| r.id != rec.id);
        self.added.push(rec);
        self.rebuild();
    }

    /// Remove a user item kept in memory; whether there was one.
    pub fn remove_user_item(&mut self, id: &str) -> bool {
        let before = self.added.len();
        self.added.retain(|r| r.id != id);
        let removed = self.added.len() != before;
        if removed {
            self.rebuild();
        }
        removed
    }

    /// The item `id`'s file from its catalog. An item without a licence never loads.
    pub fn file_bytes(&self, id: &str) -> Result<Vec<u8>, String> {
        let r = self.item(id).ok_or_else(|| format!("no library item `{id}`"))?;
        if r.licence.trim().is_empty() {
            return Err(format!("`{id}` has no licence, so it never loads"));
        }
        let file = r.file.as_deref().filter(|f| !f.is_empty());
        match r.catalog.as_str() {
            "builtin" => Err(format!("`{id}` is built in: it has no file")),
            "sample" => {
                let io = library_io().ok_or(NO_READERS)?;
                let file = file.ok_or_else(|| format!("`{id}` has no file"))?;
                (io.sample)().iter().find(|(name, _)| *name == file).map(|(_, b)| b.to_vec()).ok_or_else(|| format!("the sample set has no {file}"))
            }
            other => self.dir_file(r, other, file),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn dir_file(&self, r: &ItemRecord, catalog_name: &str, file: Option<&str>) -> Result<Vec<u8>, String> {
        let id = &r.id;
        if catalog_name == "local" {
            return catalog::read_item_file(std::path::Path::new(&r.source));
        }
        if catalog_name == "user" && self.user_dir.is_none() {
            if file.is_none() {
                return Ok(Vec::new());
            }
        }
        let dir = self.user_dir.as_ref().ok_or_else(|| format!("`{id}`'s catalog has no folder"))?;
        let file = file.filter(|f| catalog::safe_relative(f)).ok_or_else(|| format!("`{id}` has no file"))?;
        if let Some(name) = catalog_name.strip_prefix("installed:") {
            return catalog::read_item_file(&catalog::join_relative(&dir.join("installed").join(name), file));
        }
        if catalog_name == "user" {
            return catalog::read_item_file(&catalog::join_relative(&dir.join("user"), file));
        }
        Err(format!("`{id}` is in an unknown catalog"))
    }

    #[cfg(target_arch = "wasm32")]
    fn dir_file(&self, r: &ItemRecord, _catalog_name: &str, _file: Option<&str>) -> Result<Vec<u8>, String> {
        Err(format!("`{}`'s file is not available in the web build", r.id))
    }

    /// The symbol mesh of item `id` (kind symbol), read once and cached by id.
    pub fn symbol(&mut self, id: &str) -> Result<SymbolMesh, String> {
        self.sync_io();
        if let Some(m) = self.symbols.get(id) {
            return Ok(m.clone());
        }
        let r = self.item(id).ok_or_else(|| format!("no library item `{id}`"))?;
        if r.kind != ItemKind::Symbol {
            return Err(format!("`{id}` is not a 3D symbol"));
        }
        let io = library_io().ok_or(NO_READERS)?;
        let bytes = self.file_bytes(id)?;
        let mesh = (io.read_glb)(&bytes)?;
        if mesh.triangles() > MAX_SYMBOL_TRIANGLES {
            return Err(format!("`{id}` has more than {MAX_SYMBOL_TRIANGLES} triangles"));
        }
        self.symbols.insert(id.to_string(), mesh.clone());
        Ok(mesh)
    }

    /// The image of item `id` (materials at most 256 px, backdrops 1024 px), read once and cached.
    pub fn image(&mut self, id: &str) -> Result<ImageRgba, String> {
        self.sync_io();
        if let Some(i) = self.images.get(id) {
            return Ok(i.clone());
        }
        let r = self.item(id).ok_or_else(|| format!("no library item `{id}`"))?;
        let side = match r.kind {
            ItemKind::Material => 256,
            ItemKind::Backdrop if r.file.is_some() => 1024,
            _ => return Err(format!("`{id}` is not an image")),
        };
        let io = library_io().ok_or(NO_READERS)?;
        let bytes = self.file_bytes(id)?;
        let img = (io.read_image)(&bytes, side)?;
        if !img.is_valid() {
            return Err(format!("`{id}`'s image is not valid"));
        }
        self.images.insert(id.to_string(), img.clone());
        Ok(img)
    }

    /// Every tag in use, [`FAVORITES`] first (always offered), then by name.
    pub fn tags(&self) -> Vec<String> {
        let used: BTreeSet<&String> = self.state.tags.values().flatten().filter(|t| t.as_str() != FAVORITES).collect();
        std::iter::once(FAVORITES.to_string()).chain(used.into_iter().cloned()).collect()
    }

    /// Add (`on`) or remove a tag on item `item`, saved in state.json; the item's tags after.
    pub fn set_tag(&mut self, item: &str, tag: &str, on: bool) -> Result<Vec<String>, String> {
        if self.item(item).is_none() {
            return Err(format!("no library item `{item}`"));
        }
        check_name("tag", tag)?;
        let mut st = self.state.clone();
        let list = st.tags.entry(item.to_string()).or_default();
        let mut set: BTreeSet<String> = list.drain(..).collect();
        if on {
            set.insert(tag.to_string());
            if set.len() > MAX_TAGS_PER_ITEM {
                return Err(format!("an item takes at most {MAX_TAGS_PER_ITEM} tags"));
            }
        } else {
            set.remove(tag);
        }
        let after: Vec<String> = set.into_iter().collect();
        if after.is_empty() {
            st.tags.remove(item);
        } else {
            *list = after.clone();
        }
        if st.tags.len() > MAX_TAGGED_ITEMS {
            return Err(format!("at most {MAX_TAGGED_ITEMS} items can be tagged"));
        }
        self.write_state(st)?;
        self.rebuild();
        Ok(after)
    }

    /// The saved filters, by name.
    pub fn filters(&self) -> &[SavedFilter] {
        &self.state.filters
    }

    /// The saved filter `name`'s query.
    pub fn filter(&self, name: &str) -> Option<&Query> {
        self.state.filters.iter().find(|f| f.name == name).map(|f| &f.query)
    }

    /// Save `query` under `name` (replacing a filter of that name), in state.json.
    pub fn save_filter(&mut self, name: &str, query: Query) -> Result<(), String> {
        check_name("filter name", name)?;
        let mut st = self.state.clone();
        st.filters.retain(|f| f.name != name);
        if st.filters.len() >= MAX_FILTERS {
            return Err(format!("at most {MAX_FILTERS} filters can be saved"));
        }
        st.filters.push(SavedFilter { name: name.to_string(), query });
        st.filters.sort_by(|a, b| a.name.cmp(&b.name));
        self.write_state(st)
    }

    /// Delete the saved filter `name`.
    pub fn delete_filter(&mut self, name: &str) -> Result<(), String> {
        if self.filter(name).is_none() {
            return Err(format!("no saved filter `{name}`"));
        }
        let mut st = self.state.clone();
        st.filters.retain(|f| f.name != name);
        self.write_state(st)
    }

    /// The remembered local catalogs.
    pub fn local_catalogs(&self) -> &[LocalCatalog] {
        &self.state.local
    }

    /// Make `st` the state, writing state.json first (atomically) when there is a user dir.
    fn write_state(&mut self, st: State) -> Result<(), String> {
        self.persist(&st)?;
        self.state = st;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn persist(&self, st: &State) -> Result<(), String> {
        let Some(dir) = &self.user_dir else { return Ok(()) };
        let text = serde_json::to_vec_pretty(st).map_err(|e| e.to_string())?;
        let path = dir.join("state.json");
        crate::atomic_write::write_atomic_making_folders(&path, &text)
            .map_err(|e| format!("cannot save the library state to {}: {e}", path.display()))
    }

    #[cfg(target_arch = "wasm32")]
    fn persist(&self, _st: &State) -> Result<(), String> {
        Ok(())
    }

    /// Whether `id` belongs to an item of a catalog other than `catalog`.
    #[cfg(not(target_arch = "wasm32"))]
    fn taken_elsewhere(&self, id: &str, catalog: &str) -> bool {
        self.item(id).is_some_and(|r| r.catalog != catalog)
    }

    /// `library.install`: copy the catalog in folder `from` into the user dir and reload.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn install(&mut self, from: &std::path::Path) -> Result<Installed, String> {
        let io = library_io().ok_or(NO_READERS)?;
        let dir = self.user_dir.clone().ok_or("there is no library folder (set CADCRAFT_LIBRARY_USER_DIR)")?;
        let done = catalog::install(io, &dir, from, &|id, cat| self.taken_elsewhere(id, cat))?;
        self.reload();
        Ok(done)
    }

    /// `library.local`: read the .glb files under `path` as the local catalog, remember it in
    /// state.json; the number of items it added and the files it skipped.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn add_local(&mut self, path: &std::path::Path, licence: &str, category: Category) -> Result<(usize, Vec<Skipped>), String> {
        let io = library_io().ok_or(NO_READERS)?;
        let meta = std::fs::metadata(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        if !meta.is_dir() {
            return Err(format!("{} is not a folder", path.display()));
        }
        let root = std::fs::canonicalize(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
        let key = root.to_str().ok_or("the folder's path is not UTF-8")?.to_string();
        let (found, mut skipped) = catalog::local_items(io, &root, licence, category)?;
        // Ids other catalogs (and other local folders) hold are skipped.
        let mut items = Vec::new();
        for r in found {
            let other_local = self.locals.iter().any(|(p, list)| *p != key && list.iter().any(|x| x.id == r.id));
            if other_local || self.item(&r.id).is_some_and(|x| x.catalog != "local") {
                skipped.push(Skipped::new(r.file.clone().unwrap_or_default(), format!("the id `{}` is already in the library", r.id)));
            } else {
                items.push(r);
            }
        }
        let mut st = self.state.clone();
        let entry = LocalCatalog { path: key.clone(), licence: licence.to_string(), category: category.name().to_string() };
        if let Some(l) = st.local.iter_mut().find(|l| l.path == key) {
            *l = entry;
        } else if st.local.len() >= MAX_LOCAL_CATALOGS {
            return Err(format!("at most {MAX_LOCAL_CATALOGS} local catalogs"));
        } else {
            st.local.push(entry);
        }
        self.write_state(st)?;
        let n = items.len();
        match self.locals.iter_mut().find(|(p, _)| *p == key) {
            Some((_, list)) => *list = items,
            None => self.locals.push((key, items)),
        }
        self.symbols.retain(|id, _| !id.starts_with("local:"));
        self.rebuild();
        Ok((n, skipped))
    }

    #[cfg(target_arch = "wasm32")]
    pub fn install(&mut self, _from: &std::path::Path) -> Result<Installed, String> {
        Err("installing a catalog needs a file system: not available in the web build".into())
    }

    #[cfg(target_arch = "wasm32")]
    pub fn add_local(&mut self, _path: &std::path::Path, _licence: &str, _category: Category) -> Result<(usize, Vec<Skipped>), String> {
        Err("a local catalog needs a file system: not available in the web build".into())
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! The library readers every engine unit test installs. A `OnceLock` keeps the first
    //! [`set_library_io`], so tests never install another: they call [`install_io`].

    use super::*;
    use cadcraft_doc::arch::{SymbolPart, fnv1a64_hex};

    /// Install the test readers: the real sample set, image decoder and PNG encoder, and a fake
    /// glTF reader ([`fake_glb`]).
    pub(crate) fn install_io() {
        set_library_io(LibraryIo {
            read_glb: fake_glb,
            read_image: cadcraft_io::image_read::read_image,
            encode_png: |img| cadcraft_io::png::encode_rgba8(img.width, img.height, &img.rgba).unwrap_or_default(),
            sample: cadcraft_io::library::sample_files,
        });
    }

    /// A box from `glTF` followed by its three sizes as text (`glTF 0.5 0.4 0.9`; a unit cube when
    /// they are missing, as in the sample set's real files); anything not starting `glTF` is
    /// refused.
    pub(crate) fn fake_glb(bytes: &[u8]) -> Result<SymbolMesh, String> {
        let rest = bytes.strip_prefix(b"glTF").ok_or("not a glTF binary file (no glTF header)")?;
        let dims: Vec<f32> = std::str::from_utf8(rest).unwrap_or_default().split_whitespace().filter_map(|t| t.parse().ok()).collect();
        let [x, y, z] = match dims.as_slice() {
            [x, y, z] => [*x, *y, *z],
            _ => [1.0, 1.0, 1.0],
        };
        let (hx, hy) = (x / 2.0, y / 2.0);
        let positions = vec![[-hx, -hy, 0.0], [hx, -hy, 0.0], [hx, hy, 0.0], [-hx, hy, 0.0], [-hx, -hy, z], [hx, -hy, z], [hx, hy, z], [-hx, hy, z]];
        let indices = vec![0, 2, 1, 0, 3, 2, 4, 5, 6, 4, 6, 7, 0, 1, 5, 0, 5, 4, 1, 2, 6, 1, 6, 5, 2, 3, 7, 2, 7, 6, 3, 0, 4, 3, 4, 7];
        let part = SymbolPart {
            name: "box".into(),
            normals: vec![[0.0, 0.0, 1.0]; positions.len()],
            positions,
            indices,
            base_color: [0.8, 0.8, 0.8, 1.0],
            roughness: 0.5,
            ..Default::default()
        };
        Ok(SymbolMesh { key: fnv1a64_hex(bytes), parts: vec![part], ..Default::default() })
    }

    /// The bytes [`fake_glb`] reads as a box of `size`.
    pub(crate) fn glb(size: [f64; 3]) -> Vec<u8> {
        format!("glTF {} {} {}", size[0], size[1], size[2]).into_bytes()
    }

    /// A session with the test readers whose library keeps its files in `dir`.
    pub(crate) fn session(dir: &std::path::Path) -> crate::Session {
        install_io();
        let mut s = crate::Session::new();
        s.library = Library::with_user_dir(Some(dir.to_path_buf()));
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<OsString> + 'a {
        move |k| vars.iter().find(|(n, _)| *n == k).map(|(_, v)| OsString::from(v))
    }

    #[test]
    fn the_user_dir_follows_the_environment() {
        let p = |v: &[(&str, &str)]| user_dir_from(env(v));
        assert_eq!(p(&[("CADCRAFT_LIBRARY_USER_DIR", "/lib"), ("XDG_DATA_HOME", "/x"), ("HOME", "/h")]), Some(PathBuf::from("/lib")));
        assert_eq!(p(&[("CADCRAFT_LIBRARY_USER_DIR", ""), ("XDG_DATA_HOME", "/x"), ("HOME", "/h")]), Some(PathBuf::from("/x/cadcraft/library")));
        assert_eq!(p(&[("XDG_DATA_HOME", "rel"), ("HOME", "/h")]), Some(PathBuf::from("/h/.local/share/cadcraft/library")));
        if !cfg!(windows) {
            assert_eq!(p(&[]), None);
        }
        // This crate's tests never use the real one.
        assert_eq!(default_user_dir(), None);
    }

    #[test]
    fn hostile_state_entries_are_dropped() {
        let v = serde_json::json!({
            "tags": {"a": ["Favorites", "", " x", 5, "ok"], "": ["y"], "b": "nope"},
            "filters": [{"name": "good", "query": {"text": "chair", "chips": [{"kind": "tag", "value": "Favorites"}]}},
                        {"name": "bad", "query": {"chips": [{"kind": "colour", "value": "x"}]}},
                        {"name": "", "query": {}}],
            "local": [{"path": "/x", "licence": ""}, {"path": "/y", "licence": "CC0-1.0", "category": "Spaceships"},
                      {"path": "/z", "licence": "CC0-1.0"}],
        });
        let st = State::from_json(&v);
        assert_eq!(st.tags.get("a"), Some(&vec!["Favorites".to_string(), "ok".to_string()]));
        assert_eq!(st.tags.len(), 1);
        assert_eq!(st.filters.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["good"]);
        assert_eq!(st.local, [LocalCatalog { path: "/z".into(), licence: "CC0-1.0".into(), category: "Furniture".into() }]);
    }
}
