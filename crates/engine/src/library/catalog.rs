//! The library's catalogs as [`ItemRecord`]s: the built-in items (the catalogues the existing
//! commands already know), the embedded sample set, catalogs installed into the user dir, local
//! folders of `.glb` files and the user's own items.
//!
//! A catalog.json is `{"version": 1, "items": [...]}`. Each entry is read with serde into an
//! [`ItemRecord`] and checked; an entry that cannot be trusted (no licence, no source, no file, an
//! unknown category or placement, a hostile number or path) is skipped with its reason, never
//! repaired. The native file system code is behind `#[cfg(not(target_arch = "wasm32"))]`: the web
//! build has the built-in and sample items only.

use cadcraft_arch::library::{Category, ItemKind, ItemRecord, folder_for};
use cadcraft_doc::arch::{CabinetKind, Finish, FixtureKind, FrontStyle, LightMount, Placement, PlantForm};
use serde_json::Value;

/// The door styles the `door` command accepts (`crates/engine/src/cmd/arch/opening.rs`).
pub const DOOR_STYLES: [&str; 4] = ["hinged", "sliding", "pocket", "bifold"];

/// The window styles the `window` command accepts (`crates/engine/src/cmd/arch/opening.rs`).
pub const WINDOW_STYLES: [&str; 10] = ["single-hung", "double-hung", "slider", "casement", "picture", "bay", "box", "bow", "awning", "fixed"];

/// The cabinet kinds the `cabinet` command accepts, with their item names.
pub const CABINET_KINDS: [(&str, &str, CabinetKind); 3] =
    [("base", "Base cabinet", CabinetKind::Base), ("wall", "Wall cabinet", CabinetKind::Wall), ("tall", "Tall cabinet", CabinetKind::Tall)];

/// The phase-13 fixture kinds the `fixture` command accepts, with their item names.
pub const FIXTURE_KINDS: [(&str, &str, FixtureKind); 8] = [
    ("sink", "Sink", FixtureKind::Sink),
    ("lavatory", "Lavatory", FixtureKind::Lavatory),
    ("toilet", "Toilet", FixtureKind::Toilet),
    ("bathtub", "Bathtub", FixtureKind::Bathtub),
    ("shower", "Shower", FixtureKind::Shower),
    ("range", "Range", FixtureKind::Range),
    ("refrigerator", "Refrigerator", FixtureKind::Refrigerator),
    ("dishwasher", "Dishwasher", FixtureKind::Dishwasher),
];

/// Most entries one catalog.json may hold.
pub const MAX_CATALOG_ITEMS: usize = 20_000;
/// Largest catalog.json (and state.json, and user item file) read.
pub const MAX_CATALOG_BYTES: usize = 16 << 20;
/// Largest item file (a .glb or an image) read.
pub const MAX_ITEM_FILE_BYTES: usize = 256 << 20;
/// Most files one local folder walk visits.
pub const MAX_LOCAL_FILES: usize = 20_000;
/// Deepest folder a local walk descends into.
pub const MAX_LOCAL_DEPTH: usize = 32;
/// Most installed catalogs read.
pub const MAX_INSTALLED: usize = 256;
/// Most user item files read.
pub const MAX_USER_FILES: usize = 10_000;
/// Longest id, name, folder, style, keyword or licence (characters).
const MAX_SHORT: usize = 256;
/// Longest source, author, attribution or notes (characters).
const MAX_LONG: usize = 4096;
/// Most keywords one entry may carry.
const MAX_KEYWORDS: usize = 64;
/// Largest size, elevation or cut-out (m).
const MAX_METRES: f64 = 1000.0;

/// The licence, source and author of every built-in item.
const BUILTIN_LICENCE: &str = "CC0-1.0";
const BUILTIN_SOURCE: &str = "built in";
const BUILTIN_AUTHOR: &str = "CADCraft";

/// An entry a catalog did not take, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    /// The entry's file (its id when it names none).
    pub file: String,
    pub reason: String,
}

impl Skipped {
    pub fn new(file: impl Into<String>, reason: impl Into<String>) -> Self {
        Skipped { file: file.into(), reason: reason.into() }
    }

    /// `{"file", "reason"}`.
    pub fn json(&self) -> Value {
        serde_json::json!({ "file": self.file, "reason": self.reason })
    }
}

/// What `library.install` did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Installed {
    /// `installed:<name>`.
    pub catalog: String,
    /// The entries it took.
    pub items: usize,
    pub skipped: Vec<Skipped>,
}

/// `folder_for`, and for a compound symbol type such as `chair/stool` the folder of its first
/// part that has one.
pub fn default_folder(category: &str, symbol_type: &str) -> String {
    let folder = folder_for(category, symbol_type);
    if folder == "Other"
        && let Some(f) = symbol_type.split('/').map(|t| folder_for(category, t.trim())).find(|f| *f != "Other")
    {
        return f.to_string();
    }
    folder.to_string()
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

fn builtin(id: String, name: String, category: Category, folder: &str, kind: ItemKind, symbol_type: &str, keywords: &[&str]) -> ItemRecord {
    ItemRecord {
        id,
        name,
        category: category.name().to_string(),
        folder: folder.to_string(),
        kind,
        symbol_type: symbol_type.to_string(),
        keywords: keywords.iter().map(|k| (*k).to_string()).collect(),
        source: BUILTIN_SOURCE.to_string(),
        licence: BUILTIN_LICENCE.to_string(),
        author: BUILTIN_AUTHOR.to_string(),
        catalog: "builtin".to_string(),
        read_only: true,
        ..Default::default()
    }
}

fn form_words(form: PlantForm) -> [&'static str; 2] {
    match form {
        PlantForm::DeciduousTree => ["tree", "deciduous"],
        PlantForm::EvergreenTree => ["tree", "evergreen"],
        PlantForm::Shrub => ["shrub", "bush"],
        PlantForm::GroundCover => ["ground cover", "groundcover"],
    }
}

fn mount_name(m: LightMount) -> &'static str {
    match m {
        LightMount::Ceiling => Placement::Ceiling.name(),
        LightMount::Wall => Placement::Wall.name(),
        LightMount::Floor => Placement::Floor.name(),
    }
}

fn positive(v: f64) -> Option<f64> {
    (v.is_finite() && v > 0.0).then_some(v)
}

/// Every built-in item: the doors and windows, plants, light fixtures, finishes, cabinets,
/// fixtures, front styles and backdrops the existing commands already offer.
pub fn builtin_items() -> Vec<ItemRecord> {
    let mut v = Vec::new();
    for style in DOOR_STYLES {
        let mut r = builtin(
            format!("door:{style}"),
            format!("{} door", capitalised(style)),
            Category::DoorsAndWindows,
            "Doors",
            ItemKind::Door,
            "door",
            &["door", style],
        );
        r.style = Some(style.to_string());
        r.size = Some([0.9, 0.2, 2.1]);
        v.push(r);
    }
    for style in WINDOW_STYLES {
        let mut r = builtin(
            format!("window:{style}"),
            format!("{} window", capitalised(style)),
            Category::DoorsAndWindows,
            "Windows",
            ItemKind::Window,
            "window",
            &["window", style],
        );
        r.style = Some(style.to_string());
        r.size = Some([1.2, 0.2, 1.2]);
        v.push(r);
    }
    for sp in cadcraft_arch::plant::catalogue() {
        let words = form_words(sp.form);
        let mut r = builtin(
            format!("plant:{}", sp.name),
            sp.name.clone(),
            Category::Plants,
            folder_for(Category::Plants.name(), "plant"),
            ItemKind::Plant,
            "plant",
            &["plant", words[0], words[1]],
        );
        if let (Some(spread), Some(height)) = (positive(sp.mature_spread), positive(sp.mature_height)) {
            r.size = Some([spread, spread, height]);
        }
        v.push(r);
    }
    for t in cadcraft_arch::light::catalogue() {
        let mut r = builtin(
            format!("lightfixture:{}", t.name),
            t.name.to_string(),
            Category::LightingAndElectrical,
            folder_for(Category::LightingAndElectrical.name(), "light"),
            ItemKind::LightFixture,
            "light",
            &["light", "fixture"],
        );
        r.placement = Some(mount_name(t.mount).to_string());
        v.push(r);
    }
    for f in Finish::ALL {
        let uses: &[&str] = match f {
            Finish::Siding | Finish::Brick => &["finish", "wall"],
            Finish::Shingles => &["finish", "roof"],
            Finish::Wood => &["finish", "wall", "roof", "floor"],
        };
        v.push(builtin(format!("finish:{}", f.name()), capitalised(f.name()), Category::Materials, "Finishes", ItemKind::Finish, "finish", uses));
    }
    for (kind, name, k) in CABINET_KINDS {
        let c = cadcraft_arch::casework::cabinet_defaults(k);
        let mut r = builtin(
            format!("cabinet:{kind}"),
            name.to_string(),
            Category::CabinetParts,
            "Cabinets",
            ItemKind::Cabinet,
            "cabinet",
            &["cabinet", kind],
        );
        r.size = Some([c.width, c.depth, c.height]);
        r.elevation = positive(c.elevation);
        v.push(r);
    }
    for (kind, name, k) in FIXTURE_KINDS {
        let f = cadcraft_arch::casework::fixture_defaults(k);
        let mut r = builtin(
            format!("fixture:{kind}"),
            name.to_string(),
            Category::AppliancesAndPlumbing,
            "Parametric",
            ItemKind::Fixture,
            kind,
            &["fixture", kind],
        );
        r.size = Some([f.width, f.depth, f.height]);
        r.elevation = positive(f.elevation);
        v.push(r);
    }
    for part in ["door", "drawer"] {
        for style in FrontStyle::ALL {
            let mut r = builtin(
                format!("frontstyle:{part}-{}", style.name()),
                format!("{} {part}", capitalised(style.name())),
                Category::CabinetParts,
                "Front styles",
                ItemKind::FrontStyle,
                "front",
                &["front", part, style.name()],
            );
            r.style = Some(style.name().to_string());
            v.push(r);
        }
    }
    for (id, name) in [("colour", "Colour"), ("gradient", "Gradient")] {
        v.push(builtin(
            format!("backdrop:{id}"),
            name.to_string(),
            Category::Backdrops,
            folder_for(Category::Backdrops.name(), "backdrop"),
            ItemKind::Backdrop,
            "backdrop",
            &["backdrop", "background", id],
        ));
    }
    v
}

/// The entries of a catalog.json: `{"version": 1, "items": [...]}` with at most
/// [`MAX_CATALOG_ITEMS`] entries. Anything else is refused.
pub fn catalog_entries(bytes: &[u8]) -> Result<Vec<Value>, String> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(format!("catalog.json is larger than {} MiB", MAX_CATALOG_BYTES >> 20));
    }
    let v: Value = serde_json::from_slice(bytes).map_err(|e| format!("catalog.json is not valid JSON: {e}"))?;
    let Some(obj) = v.as_object() else { return Err("catalog.json must be an object".into()) };
    if obj.get("version").and_then(Value::as_u64) != Some(1) {
        return Err("catalog.json must have \"version\": 1".into());
    }
    let Some(items) = obj.get("items").and_then(Value::as_array) else { return Err("catalog.json must have an \"items\" list".into()) };
    if items.len() > MAX_CATALOG_ITEMS {
        return Err(format!("catalog.json has more than {MAX_CATALOG_ITEMS} items"));
    }
    Ok(items.clone())
}

/// What a skipped entry is called: its file, else its id, else its position.
pub fn entry_label(v: &Value, index: usize) -> String {
    let named = |k: &str| v.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).map(|s| s.chars().take(MAX_SHORT).collect::<String>());
    named("file").or_else(|| named("id")).unwrap_or_else(|| format!("item {}", index + 1))
}

fn short(what: &str, s: &str, required: bool) -> Result<(), String> {
    if required && s.trim().is_empty() {
        return Err(format!("no {what}"));
    }
    if s.chars().count() > MAX_SHORT {
        return Err(format!("the {what} is longer than {MAX_SHORT} characters"));
    }
    if s.chars().any(char::is_control) {
        return Err(format!("the {what} has control characters"));
    }
    Ok(())
}

fn long(what: &str, s: &str) -> Result<(), String> {
    if s.chars().count() > MAX_LONG {
        return Err(format!("the {what} is longer than {MAX_LONG} characters"));
    }
    Ok(())
}

fn metres(what: &str, v: f64, zero_ok: bool) -> Result<(), String> {
    let ok = v.is_finite() && v.abs() <= MAX_METRES && (v > 0.0 || (zero_ok && v >= 0.0));
    if ok { Ok(()) } else { Err(format!("the {what} {v} is out of range")) }
}

/// Whether `file` is a plain relative path inside its catalog's folder: `/`-separated names, none
/// empty, `.` or `..`, no drive or backslash, no control characters.
pub fn safe_relative(file: &str) -> bool {
    !file.is_empty()
        && file.chars().count() <= MAX_SHORT
        && !file.contains('\\')
        && !file.contains(':')
        && file.split('/').all(|c| !c.is_empty() && c != "." && c != ".." && !c.chars().any(char::is_control))
}

/// The lower-case extension of a file name.
fn extension(file: &str) -> String {
    file.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default()
}

/// Ids the generated catalogs own: a catalog.json may not use them.
const RESERVED_PREFIXES: [&str; 2] = ["local:", "user:"];

/// One catalog.json entry as an item of `catalog`, checked: the kind follows the file (a `.glb` is
/// a symbol, an image a material or a backdrop by category), the folder defaults to
/// [`default_folder`], the tags come from state.json (any in the entry are dropped).
pub fn entry_record(v: &Value, catalog: &str) -> Result<ItemRecord, String> {
    if !v.is_object() {
        return Err("the entry is not an object".into());
    }
    let mut r: ItemRecord = serde_json::from_value(v.clone()).map_err(|e| format!("the entry is unreadable: {e}"))?;
    short("id", &r.id, true)?;
    if r.id.trim() != r.id {
        return Err("the id starts or ends with a space".into());
    }
    if let Some(p) = RESERVED_PREFIXES.iter().find(|p| r.id.starts_with(**p)) {
        return Err(format!("ids starting with `{p}` are reserved"));
    }
    short("name", &r.name, true)?;
    match Category::parse(&r.category) {
        None => return Err(format!("unknown category `{}`", r.category.chars().take(MAX_SHORT).collect::<String>())),
        Some(Category::User) => return Err("only your own items go in the User catalog".into()),
        Some(_) => {}
    }
    short("licence", &r.licence, true)?;
    if r.source.trim().is_empty() {
        return Err("no source".into());
    }
    long("source", &r.source)?;
    long("author", &r.author)?;
    long("attribution", &r.attribution)?;
    long("notes", &r.notes)?;
    short("folder", &r.folder, false)?;
    short("symbol type", &r.symbol_type, false)?;
    if let Some(s) = &r.style {
        short("style", s, false)?;
    }
    if r.keywords.len() > MAX_KEYWORDS {
        return Err(format!("more than {MAX_KEYWORDS} keywords"));
    }
    for k in &r.keywords {
        short("keyword", k, false)?;
    }
    if let Some(p) = &r.placement
        && Placement::parse(p).is_none()
    {
        return Err(format!("unknown placement `{}`", p.chars().take(MAX_SHORT).collect::<String>()));
    }
    if let Some(size) = r.size {
        for v in size {
            metres("size", v, false)?;
        }
    }
    if let Some(e) = r.elevation {
        metres("elevation", e, true)?;
    }
    if let Some(c) = r.cutout {
        for v in c {
            metres("cut-out", v, false)?;
        }
    }
    let file = r.file.clone().unwrap_or_default();
    if file.is_empty() {
        return Err("no file".into());
    }
    if !safe_relative(&file) {
        return Err("the file must be a plain relative path inside the catalog's folder".into());
    }
    r.kind = match (extension(&file).as_str(), Category::parse(&r.category)) {
        ("glb", _) => ItemKind::Symbol,
        ("png" | "jpg" | "jpeg", Some(Category::Materials)) => ItemKind::Material,
        ("png" | "jpg" | "jpeg", Some(Category::Backdrops)) => ItemKind::Backdrop,
        ("png" | "jpg" | "jpeg", _) => return Err("an image item belongs in Materials or Backdrops".into()),
        (ext, _) => return Err(format!("unsupported file type `.{}`", ext.chars().take(16).collect::<String>())),
    };
    if r.folder.is_empty() {
        r.folder = default_folder(&r.category, &r.symbol_type);
    }
    r.tags.clear();
    r.catalog = catalog.to_string();
    r.read_only = true;
    Ok(r)
}

/// The sample set's items (catalog `sample`): the entries of its catalog.json whose files the set
/// holds.
pub fn sample_items(files: &[(&str, &[u8])]) -> Vec<ItemRecord> {
    let Some((_, bytes)) = files.iter().find(|(name, _)| *name == "catalog.json") else {
        log::warn!("library: the sample set has no catalog.json");
        return Vec::new();
    };
    let entries = match catalog_entries(bytes) {
        Ok(e) => e,
        Err(e) => {
            log::warn!("library: the sample catalog: {e}");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for (i, v) in entries.iter().enumerate() {
        match entry_record(v, "sample") {
            Ok(r) if files.iter().any(|(name, _)| Some(*name) == r.file.as_deref()) => out.push(r),
            Ok(_) => log::warn!("library: sample {}: the file is not in the set", entry_label(v, i)),
            Err(e) => log::warn!("library: sample {}: {e}", entry_label(v, i)),
        }
    }
    out
}

/// A user item file (`user/<slug>.json` = `{"record": ItemRecord, ...}`) as an item of catalog
/// `user`, editable.
pub fn user_record(v: &Value) -> Result<ItemRecord, String> {
    let rec = v.get("record").ok_or("no \"record\"")?;
    let mut r: ItemRecord = serde_json::from_value(rec.clone()).map_err(|e| format!("the record is unreadable: {e}"))?;
    short("id", &r.id, true)?;
    short("name", &r.name, true)?;
    r.tags.clear();
    r.catalog = "user".to_string();
    r.read_only = false;
    Ok(r)
}

/// The installed catalog name for a folder name: every character outside `[A-Za-z0-9._-]` becomes
/// `-` and leading dots go (a dot-name is the installer's own scratch folder).
pub fn sanitised_name(name: &str) -> Result<String, String> {
    let s: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' }).collect();
    let s = s.trim_start_matches('.').to_string();
    if s.is_empty() {
        return Err("the catalog folder's name has no usable characters".into());
    }
    if s.chars().count() > 64 {
        return Err("the catalog folder's name is longer than 64 characters".into());
    }
    Ok(s)
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::{
        Category, Installed, ItemKind, ItemRecord, MAX_CATALOG_BYTES, MAX_INSTALLED, MAX_ITEM_FILE_BYTES, MAX_LOCAL_DEPTH, MAX_LOCAL_FILES,
        MAX_USER_FILES, Placement, Skipped, Value, catalog_entries, entry_label, entry_record, extension, sanitised_name, user_record,
    };
    use crate::capped_read::read_capped;
    use crate::library::LibraryIo;
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Unique scratch names within this process.
    static SEQ: AtomicU64 = AtomicU64::new(0);

    /// The names in `dir`, sorted, with whether each is a real folder or a regular file (symlinks
    /// are neither: they are not followed).
    fn listing(dir: &Path) -> std::io::Result<Vec<(String, bool, bool)>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let Ok(name) = entry.file_name().into_string() else {
                out.push((String::new(), false, false));
                continue;
            };
            let ft = entry.file_type()?;
            out.push((name, ft.is_dir(), ft.is_file()));
        }
        out.sort();
        Ok(out)
    }

    /// Whether `path` is a regular file (not a symlink).
    fn regular_file(path: &Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
    }

    /// `relative` (a [`safe_relative`] path) under `dir`.
    pub fn join_relative(dir: &Path, relative: &str) -> PathBuf {
        relative.split('/').fold(dir.to_path_buf(), |p, c| p.join(c))
    }

    /// The items of every catalog installed in `<user dir>/installed/`, by folder name. Entries
    /// whose file is missing are dropped. The .glb files were read when they were installed, so
    /// they are not read again here.
    pub fn installed_items(user_dir: &Path) -> Vec<ItemRecord> {
        let root = user_dir.join("installed");
        let Ok(names) = listing(&root) else { return Vec::new() };
        let mut out = Vec::new();
        for (name, _, _) in names.into_iter().filter(|(n, d, _)| *d && !n.is_empty() && !n.starts_with('.')).take(MAX_INSTALLED) {
            let dir = root.join(&name);
            let catalog = format!("installed:{name}");
            let entries = match read_capped(&dir.join("catalog.json"), MAX_CATALOG_BYTES).map_err(|e| e.to_string()).and_then(|b| catalog_entries(&b))
            {
                Ok(e) => e,
                Err(e) => {
                    log::warn!("library: {catalog}: {e}");
                    continue;
                }
            };
            for (i, v) in entries.iter().enumerate() {
                match entry_record(v, &catalog) {
                    Ok(r) if r.file.as_deref().is_some_and(|f| regular_file(&join_relative(&dir, f))) => out.push(r),
                    Ok(_) => log::warn!("library: {catalog} {}: the file is missing", entry_label(v, i)),
                    Err(e) => log::warn!("library: {catalog} {}: {e}", entry_label(v, i)),
                }
            }
        }
        out
    }

    /// The user's items, `<user dir>/user/*.json`, sorted by file name.
    pub fn user_items(user_dir: &Path) -> Vec<ItemRecord> {
        let root = user_dir.join("user");
        let Ok(names) = listing(&root) else { return Vec::new() };
        let mut out = Vec::new();
        for (name, _, _) in names.into_iter().filter(|(n, _, f)| *f && n.ends_with(".json")).take(MAX_USER_FILES) {
            let parsed = read_capped(&root.join(&name), MAX_CATALOG_BYTES)
                .map_err(|e| e.to_string())
                .and_then(|b| serde_json::from_slice::<Value>(&b).map_err(|e| e.to_string()))
                .and_then(|v| user_record(&v));
            match parsed {
                Ok(r) => out.push(r),
                Err(e) => log::warn!("library: user item {name}: {e}"),
            }
        }
        out
    }

    /// The bytes of a catalog file, at most [`MAX_ITEM_FILE_BYTES`].
    pub fn read_item_file(path: &Path) -> Result<Vec<u8>, String> {
        read_capped(path, MAX_ITEM_FILE_BYTES).map_err(|e| format!("cannot read {}: {e}", path.display()))
    }

    /// Install the catalog in folder `from` into `<user dir>/installed/<name>/`, replacing an
    /// earlier install of that name. `taken` holds the ids of every other catalog. Each entry is
    /// checked as [`entry_record`] does, its file must be a regular file in the folder, and a .glb
    /// must pass `read_glb` (an image `read_image`); the rest are skipped with their reasons. The
    /// copy is written to a scratch folder first and swapped in by renames.
    pub fn install(io: &LibraryIo, user_dir: &Path, from: &Path, taken: &dyn Fn(&str, &str) -> bool) -> Result<Installed, String> {
        let meta = std::fs::metadata(from).map_err(|e| format!("cannot open {}: {e}", from.display()))?;
        if !meta.is_dir() {
            return Err(format!("{} is not a folder", from.display()));
        }
        let from = std::fs::canonicalize(from).map_err(|e| format!("cannot open {}: {e}", from.display()))?;
        let name = from.file_name().and_then(|n| n.to_str()).ok_or("the catalog folder has no usable name")?;
        let name = sanitised_name(name)?;
        let catalog = format!("installed:{name}");
        let catalog_path = from.join("catalog.json");
        if !regular_file(&catalog_path) {
            return Err(format!("{} holds no catalog.json", from.display()));
        }
        let bytes = read_capped(&catalog_path, MAX_CATALOG_BYTES).map_err(|e| format!("cannot read catalog.json: {e}"))?;
        let entries = catalog_entries(&bytes)?;

        let mut skipped = Vec::new();
        let mut accepted: Vec<(Value, String, Vec<u8>)> = Vec::new();
        let mut ids = BTreeSet::new();
        for (i, v) in entries.iter().enumerate() {
            let label = entry_label(v, i);
            let checked = entry_record(v, &catalog).and_then(|r| {
                if taken(&r.id, &catalog) || !ids.insert(r.id.clone()) {
                    return Err(format!("the id `{}` is already in the library", r.id));
                }
                let file = r.file.clone().unwrap_or_default();
                let path = join_relative(&from, &file);
                if !regular_file(&path) {
                    return Err("the file is not in the folder".into());
                }
                let data = read_item_file(&path)?;
                match r.kind {
                    ItemKind::Symbol => {
                        let mesh = (io.read_glb)(&data)?;
                        if mesh.triangles() > cadcraft_doc::arch::MAX_SYMBOL_TRIANGLES {
                            return Err(format!("more than {} triangles", cadcraft_doc::arch::MAX_SYMBOL_TRIANGLES));
                        }
                    }
                    _ => {
                        (io.read_image)(&data, 1024)?;
                    }
                }
                Ok((file, data))
            });
            match checked {
                Ok((file, data)) => accepted.push((v.clone(), file, data)),
                Err(reason) => skipped.push(Skipped::new(label, reason)),
            }
        }

        let root = user_dir.join("installed");
        std::fs::create_dir_all(&root).map_err(|e| format!("cannot make {}: {e}", root.display()))?;
        let seq = SEQ.fetch_add(1, Ordering::Relaxed);
        let staging = root.join(format!(".{name}.partial-{}-{seq}", std::process::id()));
        let _ = std::fs::remove_dir_all(&staging);
        let written = write_catalog(&staging, &accepted);
        if let Err(e) = written {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        let target = root.join(&name);
        let old = root.join(format!(".{name}.old-{}-{seq}", std::process::id()));
        let had_old = std::fs::symlink_metadata(&target).is_ok();
        if had_old && let Err(e) = std::fs::rename(&target, &old) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("cannot replace {}: {e}", target.display()));
        }
        if let Err(e) = std::fs::rename(&staging, &target) {
            if had_old {
                let _ = std::fs::rename(&old, &target);
            }
            let _ = std::fs::remove_dir_all(&staging);
            return Err(format!("cannot install into {}: {e}", target.display()));
        }
        if had_old {
            let _ = std::fs::remove_dir_all(&old);
        }
        Ok(Installed { catalog, items: accepted.len(), skipped })
    }

    /// The accepted entries' files and a catalog.json listing just them, in a fresh folder.
    fn write_catalog(dir: &Path, accepted: &[(Value, String, Vec<u8>)]) -> Result<(), String> {
        std::fs::create_dir(dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
        for (_, file, data) in accepted {
            let path = join_relative(dir, file);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| format!("cannot make {}: {e}", parent.display()))?;
            }
            std::fs::write(&path, data).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        }
        let items: Vec<Value> = accepted.iter().map(|(v, _, _)| v.clone()).collect();
        let catalog = serde_json::json!({ "version": 1, "items": items });
        let text = serde_json::to_vec_pretty(&catalog).map_err(|e| e.to_string())?;
        std::fs::write(dir.join("catalog.json"), text).map_err(|e| format!("cannot write catalog.json: {e}"))
    }

    /// The .glb files under `root` as items of catalog `local` (see `library.local`): a sorted walk
    /// that never follows a symlink and refuses a folder of more than [`MAX_LOCAL_FILES`] files.
    /// Each file goes through `read_glb`; the ones it refuses are skipped with its reason.
    pub fn local_items(io: &LibraryIo, root: &Path, licence: &str, category: Category) -> Result<(Vec<ItemRecord>, Vec<Skipped>), String> {
        let mut files = Vec::new();
        let mut skipped = Vec::new();
        let mut visited = 0usize;
        walk(root, &mut Vec::new(), &mut visited, &mut files, &mut skipped)?;
        let mut items = Vec::new();
        for rel in files {
            let parts: Vec<&str> = rel.split('/').collect();
            let path = join_relative(root, &rel);
            let mesh = match read_item_file(&path).and_then(|b| (io.read_glb)(&b)) {
                Ok(m) => m,
                Err(e) => {
                    skipped.push(Skipped::new(rel.clone(), e));
                    continue;
                }
            };
            let file_name = parts.last().copied().unwrap_or_default();
            let stem = file_name.get(..file_name.len().saturating_sub(4)).unwrap_or(file_name).to_string();
            let id_path = rel.get(..rel.len().saturating_sub(4)).unwrap_or(&rel).to_string();
            let folders = parts.get(..parts.len().saturating_sub(1)).unwrap_or(&[]);
            let first = folders.first().and_then(|f| Category::parse(f)).filter(|c| *c != Category::User);
            let extents = mesh.extents();
            let mut r = ItemRecord {
                id: format!("local:{id_path}"),
                name: stem,
                category: first.unwrap_or(category).name().to_string(),
                folder: folders.last().map(|f| (*f).to_string()).unwrap_or_default(),
                kind: ItemKind::Symbol,
                placement: Some(Placement::Floor.name().to_string()),
                licence: licence.to_string(),
                source: path.to_string_lossy().into_owned(),
                catalog: "local".to_string(),
                read_only: true,
                file: Some(rel.clone()),
                ..Default::default()
            };
            if extents.iter().all(|v| v.is_finite() && *v >= 0.0) {
                r.size = Some(extents);
            }
            items.push(r);
        }
        Ok((items, skipped))
    }

    fn walk(dir: &Path, rel: &mut Vec<String>, visited: &mut usize, files: &mut Vec<String>, skipped: &mut Vec<Skipped>) -> Result<(), String> {
        if rel.len() > MAX_LOCAL_DEPTH {
            skipped.push(Skipped::new(rel.join("/"), format!("deeper than {MAX_LOCAL_DEPTH} folders")));
            return Ok(());
        }
        let names = listing(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for (name, is_dir, is_file) in names {
            *visited += 1;
            if *visited > MAX_LOCAL_FILES {
                return Err(format!("the folder holds more than {MAX_LOCAL_FILES} files"));
            }
            if name.is_empty() {
                skipped.push(Skipped::new(rel.join("/"), "a file name is not UTF-8"));
                continue;
            }
            if is_dir {
                rel.push(name.clone());
                let r = walk(&dir.join(&name), rel, visited, files, skipped);
                rel.pop();
                r?;
            } else if is_file && extension(&name) == "glb" {
                let mut p = rel.clone();
                p.push(name);
                files.push(p.join("/"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Session;
    use serde_json::json;

    fn ids(items: &[ItemRecord], prefix: &str) -> Vec<String> {
        items.iter().filter(|r| r.id.starts_with(prefix)).map(|r| r.id.clone()).collect()
    }

    #[test]
    fn builtin_items_cover_the_existing_catalogues() {
        let items = builtin_items();
        built_in_and_licensed(&items);
        doors_and_windows(&items);
        plants_and_lights(&items);
        finishes(&items);
        casework(&items);
        fronts_and_backdrops(&items);
    }

    /// Unique ids, all built in, read-only and licensed.
    fn built_in_and_licensed(items: &[ItemRecord]) {
        let mut seen = std::collections::BTreeSet::new();
        for r in items {
            assert!(seen.insert(r.id.clone()), "duplicate {}", r.id);
            assert_eq!((r.catalog.as_str(), r.read_only), ("builtin", true), "{}", r.id);
            assert_eq!((r.source.as_str(), r.licence.as_str(), r.author.as_str()), ("built in", "CC0-1.0", "CADCraft"), "{}", r.id);
            assert!(Category::parse(&r.category).is_some_and(|c| c != Category::User), "{}: {}", r.id, r.category);
            assert!(!r.name.is_empty() && !r.folder.is_empty(), "{}", r.id);
        }
    }

    /// Every door and window style, each accepted by its command.
    fn doors_and_windows(items: &[ItemRecord]) {
        assert_eq!(ids(items, "door:"), DOOR_STYLES.map(|s| format!("door:{s}")));
        assert_eq!(ids(items, "window:"), WINDOW_STYLES.map(|s| format!("window:{s}")));
        for r in items.iter().filter(|r| r.id.starts_with("door:") || r.id.starts_with("window:")) {
            assert_eq!(r.category, "Doors and windows");
            let door = r.id.starts_with("door:");
            assert_eq!(r.kind, if door { ItemKind::Door } else { ItemKind::Window });
            assert_eq!(r.size, Some(if door { [0.9, 0.2, 2.1] } else { [1.2, 0.2, 1.2] }), "{}", r.id);
        }
        for (cmd, styles) in [("door", DOOR_STYLES.as_slice()), ("window", WINDOW_STYLES.as_slice())] {
            for style in styles {
                let mut s = Session::new();
                s.execute("wall", &json!({"points": [[0.0, 0.0], [10.0, 0.0]]})).expect("wall");
                let r = s.execute(cmd, &json!({"wall": "w0", "offset": 5.0, "style": style}));
                assert!(r.is_ok(), "{cmd} refuses the built-in style {style}: {r:?}");
            }
        }
    }

    /// One item per plant species and light fixture type, each known to its command.
    fn plants_and_lights(items: &[ItemRecord]) {
        let plants: Vec<String> = cadcraft_arch::plant::catalogue().iter().map(|p| format!("plant:{}", p.name)).collect();
        let mut got = ids(items, "plant:");
        let mut want = plants.clone();
        got.sort();
        want.sort();
        assert_eq!(got, want);
        assert!(items.iter().filter(|r| r.id.starts_with("plant:")).all(|r| r.category == "Plants" && r.kind == ItemKind::Plant));
        let lights: Vec<String> = cadcraft_arch::light::catalogue().iter().map(|t| format!("lightfixture:{}", t.name)).collect();
        assert_eq!(ids(items, "lightfixture:"), lights);
        assert!(
            items
                .iter()
                .filter(|r| r.id.starts_with("lightfixture:"))
                .all(|r| r.category == "Lighting and electrical" && r.kind == ItemKind::LightFixture)
        );
        for r in items.iter().filter(|r| r.id.starts_with("plant:")) {
            assert!(cadcraft_arch::plant::find_species(&cadcraft_doc::ArchModel::default(), &r.name).is_some(), "the plant command knows {}", r.name);
        }
        for r in items.iter().filter(|r| r.id.starts_with("lightfixture:")) {
            assert!(cadcraft_arch::light::find_type(&r.name).is_some(), "the lightfixture command knows {}", r.name);
            assert!(r.placement.as_deref().is_some_and(|p| Placement::parse(p).is_some()), "{}", r.id);
        }
    }

    /// The four finishes, with their names.
    fn finishes(items: &[ItemRecord]) {
        let finishes: Vec<(String, String)> = items.iter().filter(|r| r.id.starts_with("finish:")).map(|r| (r.id.clone(), r.name.clone())).collect();
        assert_eq!(
            finishes,
            [("finish:siding", "Siding"), ("finish:shingles", "Shingles"), ("finish:brick", "Brick"), ("finish:wood", "Wood")]
                .map(|(a, b)| (a.to_string(), b.to_string()))
        );
        assert!(items.iter().filter(|r| r.id.starts_with("finish:")).all(|r| r.category == "Materials" && r.kind == ItemKind::Finish));
        for f in Finish::ALL {
            assert!(Finish::parse(f.name()).is_some());
        }
    }

    /// Cabinets and fixtures, each accepted by its command.
    fn casework(items: &[ItemRecord]) {
        assert_eq!(ids(items, "cabinet:"), ["cabinet:base", "cabinet:wall", "cabinet:tall"]);
        assert!(
            items
                .iter()
                .filter(|r| r.id.starts_with("cabinet:"))
                .all(|r| r.category == "Cabinet parts" && r.kind == ItemKind::Cabinet && r.folder == "Cabinets")
        );
        assert_eq!(ids(items, "fixture:"), FIXTURE_KINDS.map(|(k, _, _)| format!("fixture:{k}")));
        assert!(
            items
                .iter()
                .filter(|r| r.id.starts_with("fixture:"))
                .all(|r| r.category == "Appliances and plumbing fixtures" && r.kind == ItemKind::Fixture && r.folder == "Parametric")
        );
        for (kind, _, _) in CABINET_KINDS {
            let mut s = Session::new();
            assert!(s.execute("cabinet", &json!({"kind": kind, "at": [0.0, 0.0]})).is_ok(), "cabinet {kind}");
        }
        for (kind, _, _) in FIXTURE_KINDS {
            let mut s = Session::new();
            assert!(s.execute("fixture", &json!({"kind": kind, "at": [0.0, 0.0]})).is_ok(), "fixture {kind}");
        }
    }

    /// Door and drawer fronts of every style, and the two backdrops.
    fn fronts_and_backdrops(items: &[ItemRecord]) {
        assert_eq!(
            ids(items, "frontstyle:"),
            ["door-slab", "door-shaker", "door-raised-panel", "drawer-slab", "drawer-shaker", "drawer-raised-panel"]
                .map(|s| format!("frontstyle:{s}"))
        );
        for r in items.iter().filter(|r| r.id.starts_with("frontstyle:")) {
            assert_eq!((r.category.as_str(), r.kind, r.folder.as_str()), ("Cabinet parts", ItemKind::FrontStyle, "Front styles"));
            let style = r.style.as_deref().unwrap_or_default();
            assert!(FrontStyle::parse(style).is_some() && r.id.ends_with(style), "{}", r.id);
        }

        // Backdrops.
        assert_eq!(ids(items, "backdrop:"), ["backdrop:colour", "backdrop:gradient"]);
        assert!(items.iter().filter(|r| r.id.starts_with("backdrop:")).all(|r| r.category == "Backdrops" && r.kind == ItemKind::Backdrop));
    }

    #[test]
    fn sample_items_come_from_the_catalog() {
        use crate::library::testing;
        let dir = crate::temp_dir::TempDir::new("library-sample");
        // A tag saved earlier comes back on the sample item.
        dir.file("state.json", r#"{"tags": {"armchair": ["Favorites", "living room"]}}"#);
        testing::install_io();
        let mut lib = crate::library::Library::with_user_dir(Some(dir.to_path_buf()));

        let files = cadcraft_io::library::sample_files();
        let entries = catalog_entries(files[0].1).expect("the sample catalog parses");
        let sample: Vec<&ItemRecord> = lib.items().iter().filter(|r| r.catalog == "sample").collect();
        assert_eq!(sample.len(), entries.len(), "every sample entry is an item");
        for v in &entries {
            let id = v["id"].as_str().unwrap_or_default();
            let r = lib.item(id).unwrap_or_else(|| panic!("no sample item {id}"));
            assert_eq!(r.catalog, "sample");
            assert!(r.read_only);
            assert_eq!(r.name, v["name"].as_str().unwrap_or_default());
            assert_eq!(r.category, v["category"].as_str().unwrap_or_default());
            assert_eq!(r.licence, v["licence"].as_str().unwrap_or_default());
            assert_eq!(r.source, v["source"].as_str().unwrap_or_default());
            assert_eq!(r.folder, default_folder(&r.category, &r.symbol_type));
            let file = v["file"].as_str().unwrap_or_default();
            let want = if file.ends_with(".glb") {
                ItemKind::Symbol
            } else if r.category == "Backdrops" {
                ItemKind::Backdrop
            } else {
                ItemKind::Material
            };
            assert_eq!(r.kind, want, "{id}");
            let bytes = files.iter().find(|f| f.0 == file).map(|f| f.1.to_vec());
            assert_eq!(lib.file_bytes(id).ok(), bytes, "{id}'s file");
        }
        // Folders from folder_for, compound symbol types by their first part.
        assert_eq!(lib.item("armchair").map(|r| r.folder.as_str()), Some("Chairs and stools"));
        assert_eq!(lib.item("own:pull-knob-32").map(|r| r.folder.as_str()), Some("Pulls"));
        assert_eq!(lib.item("material-oak").map(|r| (r.kind, r.folder.as_str())), Some((ItemKind::Material, "Images")));
        assert_eq!(lib.item("own:backdrop-sky").map(|r| r.kind), Some(ItemKind::Backdrop));
        // Tags from state.json.
        assert_eq!(lib.item("armchair").map(|r| r.tags.clone()), Some(vec!["Favorites".to_string(), "living room".to_string()]));

        // Symbols and images are read through the readers and cached.
        let mesh = lib.symbol("armchair").expect("symbol");
        assert_eq!(mesh.key, cadcraft_doc::arch::fnv1a64_hex(lib.file_bytes("armchair").unwrap_or_default().as_slice()));
        assert_eq!(lib.symbol("armchair").ok(), Some(mesh));
        assert!(lib.symbol("material-oak").is_err(), "a material is not a symbol");
        let oak = lib.image("material-oak").expect("image");
        assert!(oak.is_valid() && oak.width.max(oak.height) <= 256);
        let sky = lib.image("own:backdrop-sky").expect("backdrop");
        assert!(sky.is_valid() && sky.width.max(sky.height) <= 1024);
        assert!(lib.image("armchair").is_err());
        assert!(lib.file_bytes("door:hinged").is_err(), "a built-in item has no file");
        assert!(lib.file_bytes("nothing").is_err());
    }

    #[test]
    fn hostile_entries_are_skipped_with_a_reason() {
        let ok = json!({"id": "a", "name": "A", "category": "Furniture", "licence": "CC0-1.0", "source": "me", "file": "a.glb"});
        assert!(entry_record(&ok, "x").is_ok());
        let bad = [
            ("licence", json!("")),
            ("source", json!(" ")),
            ("file", json!("../a.glb")),
            ("file", json!("/etc/passwd.glb")),
            ("file", json!("C:/a.glb")),
            ("file", json!("a.exe")),
            ("category", json!("User catalog")),
            ("category", json!("Spaceships")),
            ("size", json!([1, 1e300, 1])),
            ("size", json!([1, -1, 1])),
            ("size", json!([1, "x", 1])),
            ("elevation", json!(1e9)),
            ("placement", json!("orbit")),
            ("id", json!("local:a")),
            ("id", json!(" a")),
            ("name", json!("")),
        ];
        for (k, v) in bad {
            let mut e = ok.clone();
            e[k] = v.clone();
            assert!(entry_record(&e, "x").is_err(), "{k} = {v} is taken");
        }
        let mut image = ok.clone();
        image["file"] = json!("a.png");
        assert!(entry_record(&image, "x").is_err(), "an image in Furniture");
        image["category"] = json!("Materials");
        assert_eq!(entry_record(&image, "x").map(|r| r.kind), Ok(ItemKind::Material));
        for bad in ["{}", "[]", r#"{"version": 2, "items": []}"#, r#"{"version": 1}"#, "nope"] {
            assert!(catalog_entries(bad.as_bytes()).is_err(), "{bad}");
        }
        assert_eq!(sanitised_name("My Catalog!").as_deref(), Ok("My-Catalog-"));
        assert_eq!(sanitised_name("..hidden").as_deref(), Ok("hidden"));
        assert!(sanitised_name("...").is_err());
    }
}
