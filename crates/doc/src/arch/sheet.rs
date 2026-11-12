//! Phase 21: schedules, sheets, view notes and generated view extras.
//!
//! Data only: the commands that fill these live in the engine, the row maths in `cadcraft_arch`.

use super::{ArchId, ArchKind, ArchModel};
use crate::RawTag;
use serde::{Deserialize, Serialize};

/// Most schedules in a drawing.
pub const MAX_SCHEDULES: usize = 64;
/// Most columns in a schedule.
pub const MAX_SCHEDULE_COLUMNS: usize = 16;
/// Most rows a schedule table may have.
pub const MAX_SCHEDULE_ROWS: usize = 10_000;
/// Most sheets in a drawing.
pub const MAX_SHEETS: usize = 256;
/// Most views on one sheet.
pub const MAX_SHEET_VIEWS: usize = 32;
/// Longest sheet title or number text.
pub const MAX_SHEET_TEXT: usize = 128;
/// Most notes on one view.
pub const MAX_VIEW_NOTES: usize = 512;
/// Longest note text, in characters.
pub const MAX_NOTE_TEXT: usize = 256;
/// Largest absolute view coordinate, in metres.
pub const MAX_VIEW_COORD: f64 = 1e6;

/// Layer of schedule tables.
pub const LAYER_SCHEDULE: &str = "A-ANNO-SCHD";
/// Layer of title blocks.
pub const LAYER_TITLE: &str = "A-ANNO-TTLB";
/// Layer of callout symbols.
pub const LAYER_CALLOUT: &str = "A-ANNO-SYMB";
/// Layer of elevation material patterns.
pub const LAYER_ELEV_PATTERN: &str = "A-ELEV-PATT";
/// Layer of elevation door swings.
pub const LAYER_ELEV_SWING: &str = "A-ELEV-SWNG";
/// Layer of elevation tags.
pub const LAYER_ELEV_TAG: &str = "A-ELEV-IDEN";
/// Layer of door number tags.
pub const LAYER_DOOR_TAG: &str = "A-DOOR-IDEN";
/// Layer of window number tags.
pub const LAYER_WINDOW_TAG: &str = "A-GLAZ-IDEN";
/// Layer of cabinet number tags.
pub const LAYER_CASE_TAG: &str = "A-FLOR-CASE-IDEN";
/// Name of the title-block block.
pub const TITLE_BLOCK: &str = "ARCH-TITLE-BLOCK";

/// The XDATA application that links an entity to a schedule or sheet.
pub const LINK_APP: &str = "CADCRAFT_ARCH_LINK";
/// Role: a schedule's TABLE (key = schedule label).
pub const LINK_SCHEDULE: &str = "schedule";
/// Role: a sheet view's VIEWPORT (key = sheet name, number = view number).
pub const LINK_SHEET_VIEW: &str = "sheet-view";
/// Role: a sheet view's title.
pub const LINK_VIEW_TITLE: &str = "view-title";
/// Role: a sheet's title block (key = sheet name, number 0).
pub const LINK_TITLE: &str = "title-block";
/// Role: a sheet view's image.
pub const LINK_SHEET_IMAGE: &str = "sheet-image";

/// What a schedule lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScheduleKind {
    /// Doors.
    #[default]
    Door,
    /// Windows.
    Window,
    /// Cabinets.
    Cabinet,
    /// Rooms.
    Room,
}

impl ScheduleKind {
    /// The JSON spelling.
    pub fn key(self) -> &'static str {
        match self {
            ScheduleKind::Door => "door",
            ScheduleKind::Window => "window",
            ScheduleKind::Cabinet => "cabinet",
            ScheduleKind::Room => "room",
        }
    }

    /// Parse a JSON spelling.
    pub fn from_key(s: &str) -> Option<ScheduleKind> {
        match s {
            "door" => Some(ScheduleKind::Door),
            "window" => Some(ScheduleKind::Window),
            "cabinet" => Some(ScheduleKind::Cabinet),
            "room" => Some(ScheduleKind::Room),
            _ => None,
        }
    }
}

/// A schedule: a table of doors, windows, cabinets or rooms.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Schedule {
    /// Stable id.
    pub id: ArchId,
    /// Display name.
    pub name: String,
    /// What it lists.
    pub kind: ScheduleKind,
    /// Column keys, in order.
    pub columns: Vec<String>,
    /// Restrict to one level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    /// Insertion point of the TABLE in model space.
    pub at: [f64; 2],
}

/// `sc{n}`.
pub fn schedule_label(id: ArchId) -> String {
    format!("sc{}", id.0)
}

/// Parse `sc{n}` (canonical decimal: no sign, no leading zeros).
pub fn parse_schedule_label(s: &str) -> Option<ArchId> {
    let n = s.strip_prefix("sc")?;
    if n.is_empty() || !n.bytes().all(|b| b.is_ascii_digit()) || (n.len() > 1 && n.starts_with('0')) {
        return None;
    }
    n.parse::<u32>().ok().map(ArchId)
}

/// Where a sheet view's picture comes from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SheetSource {
    /// A named plan view.
    Plan {
        /// The plan view's name.
        #[serde(default)]
        view: String,
        /// Level override.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        level: Option<u32>,
    },
    /// An elevation, section or interior view.
    View {
        /// The view's id.
        #[serde(default)]
        id: ArchId,
    },
    /// A schedule table.
    Schedule {
        /// The schedule's id.
        #[serde(default)]
        id: ArchId,
    },
    /// A rendered camera image.
    Camera {
        /// The camera's name.
        #[serde(default)]
        name: String,
        /// Image width on the paper's scale, in metres of paper.
        #[serde(default)]
        width: f64,
        /// Image size in pixels.
        #[serde(default)]
        pixels: [u32; 2],
    },
}

impl Default for SheetSource {
    fn default() -> Self {
        SheetSource::Plan { view: String::new(), level: None }
    }
}

impl SheetSource {
    /// "plan", the view's kind key ("elevation" when the view is gone), "schedule" or "camera".
    pub fn kind_key(&self, model: &ArchModel) -> &'static str {
        match self {
            SheetSource::Plan { .. } => "plan",
            SheetSource::View { id } => model.view(*id).map_or("elevation", |v| v.kind.key()),
            SheetSource::Schedule { .. } => "schedule",
            SheetSource::Camera { .. } => "camera",
        }
    }

    /// The plan view's name, "v<n>", "sc<n>" or the camera name.
    pub fn name(&self) -> String {
        match self {
            SheetSource::Plan { view, .. } => view.clone(),
            SheetSource::View { id } => super::view_label(*id),
            SheetSource::Schedule { id } => schedule_label(*id),
            SheetSource::Camera { name, .. } => name.clone(),
        }
    }
}

/// One view placed on a sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetView {
    /// The view number on the sheet (>= 1).
    pub number: u32,
    /// What it shows.
    pub source: SheetSource,
    /// Scale denominator (50 for 1:50); 0 for a camera image.
    pub scale: f64,
    /// Viewport centre on the paper, mm.
    pub at: [f64; 2],
    /// Viewport width and height on the paper, mm.
    pub size: [f64; 2],
}

/// A sheet: one CADCraft layout with a title block and views.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Sheet {
    /// The layout's name.
    pub name: String,
    /// The sheet number text.
    pub number: String,
    /// The sheet title.
    pub title: String,
    /// Paper name as given ("ARCH D").
    pub size: String,
    /// Landscape orientation.
    pub landscape: bool,
    /// Views on the sheet.
    pub views: Vec<SheetView>,
}

impl Default for Sheet {
    fn default() -> Self {
        Sheet { name: String::new(), number: String::new(), title: String::new(), size: "ARCH D".into(), landscape: true, views: Vec::new() }
    }
}

/// Project-wide sheet-set text.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SheetSet {
    /// Project name.
    pub project: String,
    /// Issue date text.
    pub date: String,
}

impl SheetSet {
    /// Whether nothing is set.
    pub fn is_default(&self) -> bool {
        self.project.is_empty() && self.date.is_empty()
    }
}

/// What a view note is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NoteKind {
    /// Free text.
    #[default]
    Text,
    /// A marker with text.
    Marker,
    /// A measured dimension between two points.
    Dimension,
}

impl NoteKind {
    /// The JSON spelling.
    pub fn key(self) -> &'static str {
        match self {
            NoteKind::Text => "text",
            NoteKind::Marker => "marker",
            NoteKind::Dimension => "dimension",
        }
    }
}

/// A corner of an object's bounds in a view.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Corner {
    /// Top left.
    TopLeft,
    /// Top right.
    TopRight,
    /// Bottom left.
    BottomLeft,
    /// Bottom right.
    BottomRight,
    /// The centre.
    #[default]
    Center,
}

impl Corner {
    /// The JSON spelling.
    pub fn key(self) -> &'static str {
        match self {
            Corner::TopLeft => "topLeft",
            Corner::TopRight => "topRight",
            Corner::BottomLeft => "bottomLeft",
            Corner::BottomRight => "bottomRight",
            Corner::Center => "center",
        }
    }

    /// Parse a JSON spelling.
    pub fn from_key(s: &str) -> Option<Corner> {
        match s {
            "topLeft" => Some(Corner::TopLeft),
            "topRight" => Some(Corner::TopRight),
            "bottomLeft" => Some(Corner::BottomLeft),
            "bottomRight" => Some(Corner::BottomRight),
            "center" => Some(Corner::Center),
            _ => None,
        }
    }
}

/// A note's attachment to an object's corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NoteAttach {
    /// The object's kind.
    pub kind: ArchKind,
    /// The object's id.
    pub object: ArchId,
    /// Which corner.
    pub corner: Corner,
}

fn is_zero_pt(p: &[f64; 2]) -> bool {
    p[0] == 0.0 && p[1] == 0.0
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// A note on an elevation, section or interior view.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ViewNote {
    /// Id within the view.
    pub id: u32,
    /// What it is.
    pub kind: NoteKind,
    /// The text.
    pub text: String,
    /// Position in view coordinates.
    pub at: [f64; 2],
    /// Second point (dimensions), view coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<[f64; 2]>,
    /// Attachment of `at`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attach: Option<NoteAttach>,
    /// Attachment of `b`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attach_b: Option<NoteAttach>,
    /// Added to an attached corner.
    #[serde(skip_serializing_if = "is_zero_pt")]
    pub offset: [f64; 2],
    /// Text height, metres.
    pub height: f64,
    /// Layer.
    pub layer: String,
    /// Whether the attachment was lost.
    #[serde(skip_serializing_if = "is_false")]
    pub detached: bool,
}

impl Default for ViewNote {
    fn default() -> Self {
        ViewNote {
            id: 0,
            kind: NoteKind::Text,
            text: String::new(),
            at: [0.0, 0.0],
            b: None,
            attach: None,
            attach_b: None,
            offset: [0.0, 0.0],
            height: 0.15,
            layer: "A-ANNO-NOTE".into(),
            detached: false,
        }
    }
}

/// `n{id}`.
pub fn note_label(id: u32) -> String {
    format!("n{id}")
}

/// A generated material pattern line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewPatternLine {
    /// Start (u, v).
    pub a: [f64; 2],
    /// End (u, v).
    pub b: [f64; 2],
    /// Owner kind.
    pub kind: ArchKind,
    /// Owner id.
    pub object: ArchId,
    /// Material name.
    pub material: String,
}

/// A generated text label.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ViewLabel {
    /// The text.
    pub text: String,
    /// Position (u, v).
    pub at: [f64; 2],
    /// Text height, metres.
    pub height: f64,
    /// Layer.
    pub layer: String,
    /// Owner kind.
    pub kind: ArchKind,
    /// Owner id.
    pub object: ArchId,
}

/// [`LINK_APP`] tags: `[1001 app, 1000 role, 1000 key, 1071 number]`.
pub fn link_tags(role: &str, key: &str, number: u32) -> Vec<RawTag> {
    vec![
        RawTag { code: 1001, value: LINK_APP.into() },
        RawTag { code: 1000, value: role.into() },
        RawTag { code: 1000, value: key.into() },
        RawTag { code: 1071, value: number.to_string() },
    ]
}

/// Replace this app's link group in `xdata`, keeping every other app's groups.
pub fn set_link(xdata: &mut Vec<RawTag>, role: &str, key: &str, number: u32) {
    let mut out: Vec<RawTag> = Vec::with_capacity(xdata.len() + 4);
    let mut skipping = false;
    for t in xdata.drain(..) {
        if t.code == 1001 {
            skipping = t.value == LINK_APP;
        }
        if !skipping {
            out.push(t);
        }
    }
    out.extend(link_tags(role, key, number));
    *xdata = out;
}

/// The (role, key, number) of the [`LINK_APP`] group; `None` when absent or malformed.
pub fn read_link(xdata: &[RawTag]) -> Option<(String, String, u32)> {
    let start = xdata.iter().position(|t| t.code == 1001 && t.value == LINK_APP)?;
    let mut strings = Vec::new();
    let mut number = None;
    for t in xdata.iter().skip(start + 1) {
        match t.code {
            1001 => break,
            1000 => strings.push(t.value.clone()),
            1071 | 1070 => number = t.value.trim().parse::<u32>().ok(),
            _ => {}
        }
    }
    if strings.len() != 2 {
        return None;
    }
    let key = strings.pop()?;
    let role = strings.pop()?;
    Some((role, key, number?))
}

fn plain_text_ok(s: &str, max: usize) -> bool {
    let t = s.trim();
    !t.is_empty() && t.chars().count() <= max && !t.chars().any(|c| c.is_control())
}

/// Check a schedule.
pub fn validate_schedule(s: &Schedule, level_count: usize) -> Result<(), String> {
    if !plain_text_ok(&s.name, 64) {
        return Err("schedule name must be 1..=64 characters without control characters".into());
    }
    if s.columns.is_empty() || s.columns.len() > MAX_SCHEDULE_COLUMNS {
        return Err(format!("schedule columns must number 1..={MAX_SCHEDULE_COLUMNS}"));
    }
    for (i, c) in s.columns.iter().enumerate() {
        if s.columns[..i].contains(c) {
            return Err(format!("schedule column `{c}` is repeated"));
        }
    }
    if let Some(l) = s.level {
        if l as usize >= level_count {
            return Err(format!("schedule level {l} does not exist"));
        }
    }
    if !s.at.iter().all(|v| v.is_finite() && v.abs() <= MAX_VIEW_COORD) {
        return Err("schedule at must be finite and within 1e6".into());
    }
    Ok(())
}

/// Check a sheet.
pub fn validate_sheet(s: &Sheet, level_count: usize) -> Result<(), String> {
    let n = s.name.chars().count();
    if s.name.trim().is_empty() || n > 255 || s.name.chars().any(|c| c.is_control() || "<>/\\\":;?*|,=`".contains(c)) {
        return Err("sheet name must be 1..=255 characters without control characters or any of < > / \\ \" : ; ? * | , = `".into());
    }
    let nn = s.number.chars().count();
    if s.number.trim().is_empty() || nn > 32 || s.number.chars().any(|c| c.is_control()) {
        return Err("sheet number must be 1..=32 characters without control characters".into());
    }
    if s.title.chars().count() > MAX_SHEET_TEXT || s.title.chars().any(|c| c.is_control()) {
        return Err(format!("sheet title must be at most {MAX_SHEET_TEXT} characters without control characters"));
    }
    if s.views.len() > MAX_SHEET_VIEWS {
        return Err(format!("a sheet holds at most {MAX_SHEET_VIEWS} views"));
    }
    for (i, v) in s.views.iter().enumerate() {
        if v.number < 1 {
            return Err("view numbers start at 1".into());
        }
        if s.views[..i].iter().any(|o| o.number == v.number) {
            return Err(format!("view number {} is repeated", v.number));
        }
        if !v.at.iter().chain(v.size.iter()).all(|x| x.is_finite() && x.abs() <= 1e5) {
            return Err(format!("view {}: at and size must be finite and within 1e5", v.number));
        }
        match &v.source {
            SheetSource::Camera { width, pixels, .. } => {
                if !(width.is_finite() && *width > 0.0 && *width <= 2000.0) {
                    return Err(format!("view {}: camera width must be in (0, 2000]", v.number));
                }
                if pixels.iter().any(|p| !(16..=4096).contains(p)) {
                    return Err(format!("view {}: camera pixels must be 16..=4096", v.number));
                }
            }
            other => {
                if !(v.scale.is_finite() && (1.0..=10000.0).contains(&v.scale)) {
                    return Err(format!("view {}: scale must be in [1, 10000]", v.number));
                }
                if let SheetSource::Plan { level: Some(l), .. } = other {
                    if *l as usize >= level_count {
                        return Err(format!("view {}: level {l} does not exist", v.number));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Check a view note.
pub fn validate_note(n: &super::ViewNote) -> Result<(), String> {
    let label = note_label(n.id);
    if n.text.is_empty() || n.text.chars().count() > MAX_NOTE_TEXT || n.text.chars().any(|c| c.is_control()) {
        return Err(format!("{label}: text must be 1..={MAX_NOTE_TEXT} characters without control characters"));
    }
    let ok = |p: &[f64; 2]| p.iter().all(|v| v.is_finite() && v.abs() <= MAX_VIEW_COORD);
    if !ok(&n.at) || n.b.as_ref().is_some_and(|b| !ok(b)) || !ok(&n.offset) {
        return Err(format!("{label}: at, b and offset must be finite and within 1e6"));
    }
    if !(n.height.is_finite() && n.height > 0.0 && n.height <= 10.0) {
        return Err(format!("{label}: height must be in (0, 10]"));
    }
    if n.layer.trim().is_empty() {
        return Err(format!("{label}: layer must not be empty"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_and_sheet_data_round_trip_and_skip_when_empty() {
        let plain = serde_json::to_value(ArchModel::default()).unwrap();
        for k in ["schedules", "nextScheduleId", "sheets", "sheetSet", "next_schedule_id", "sheet_set"] {
            assert!(plain.get(k).is_none(), "{k} must be skipped");
        }
        let mut m = ArchModel::default();
        m.schedules.push(Schedule {
            id: ArchId(2),
            name: "Doors".into(),
            columns: vec!["number".into()],
            level: Some(0),
            at: [1.0, 2.0],
            ..Default::default()
        });
        m.next_schedule_id = 3;
        m.sheet_set = SheetSet { project: "P".into(), date: "D".into() };
        m.sheets.push(Sheet {
            name: "A101".into(),
            number: "A1.01".into(),
            views: vec![
                SheetView { number: 1, source: SheetSource::Plan { view: "Floor".into(), level: None }, scale: 50.0, ..Default::default() },
                SheetView { number: 2, source: SheetSource::View { id: ArchId(1) }, scale: 25.0, ..Default::default() },
                SheetView { number: 3, source: SheetSource::Schedule { id: ArchId(2) }, scale: 1.0, ..Default::default() },
                SheetView { number: 4, source: SheetSource::Camera { name: "c".into(), width: 100.0, pixels: [64, 64] }, ..Default::default() },
            ],
            ..Default::default()
        });
        let back: ArchModel = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        assert_eq!(back.schedules, m.schedules);
        assert_eq!(back.sheets, m.sheets);
        assert_eq!(back.sheet_set, m.sheet_set);
        assert_eq!(back.next_schedule_id, 3);
        let n = ViewNote { id: 1, text: "x".into(), b: Some([1.0, 1.0]), detached: true, ..Default::default() };
        let back: ViewNote = serde_json::from_str(&serde_json::to_string(&n).unwrap()).unwrap();
        assert_eq!(back, n);
        let j = serde_json::to_value(ViewNote::default()).unwrap();
        for k in ["b", "attach", "attachB", "offset", "detached"] {
            assert!(j.get(k).is_none(), "{k}");
        }
        assert_eq!(schedule_label(ArchId(4)), "sc4");
        assert_eq!(parse_schedule_label("sc4"), Some(ArchId(4)));
        assert_eq!(parse_schedule_label("sc04"), None);
        assert_eq!(parse_schedule_label("sc+4"), None);
        assert_eq!(note_label(7), "n7");
    }

    fn sched() -> Schedule {
        Schedule { name: "S".into(), columns: vec!["number".into()], ..Default::default() }
    }

    fn sheet() -> Sheet {
        Sheet { name: "A1".into(), number: "1".into(), ..Default::default() }
    }

    #[test]
    fn validate_schedule_and_sheet_refuse_bad_values() {
        assert!(validate_schedule(&sched(), 1).is_ok());
        assert!(validate_schedule(&Schedule { name: " ".into(), ..sched() }, 1).is_err());
        assert!(validate_schedule(&Schedule { name: "x".repeat(65), ..sched() }, 1).is_err());
        assert!(validate_schedule(&Schedule { columns: (0..17).map(|i| format!("c{i}")).collect(), ..sched() }, 1).is_err());
        assert!(validate_schedule(&Schedule { columns: vec!["a".into(), "a".into()], ..sched() }, 1).is_err());
        assert!(validate_schedule(&Schedule { level: Some(1), ..sched() }, 1).is_err());
        assert!(validate_schedule(&Schedule { at: [f64::NAN, 0.0], ..sched() }, 1).is_err());
        assert!(validate_sheet(&sheet(), 1).is_ok());
        assert!(validate_sheet(&Sheet { name: "".into(), ..sheet() }, 1).is_err());
        assert!(validate_sheet(&Sheet { name: "a/b".into(), ..sheet() }, 1).is_err());
        assert!(validate_sheet(&Sheet { name: "x".repeat(256), ..sheet() }, 1).is_err());
        assert!(validate_sheet(&Sheet { number: "n".repeat(33), ..sheet() }, 1).is_err());
        assert!(validate_sheet(&Sheet { title: "t".repeat(129), ..sheet() }, 1).is_err());
        let v = |number: u32, scale: f64| SheetView { number, scale, ..Default::default() };
        let with = |views: Vec<SheetView>| Sheet { views, ..sheet() };
        assert!(validate_sheet(&with(vec![v(1, 50.0)]), 1).is_ok());
        assert!(validate_sheet(&with(vec![v(1, 0.0)]), 1).is_err());
        assert!(validate_sheet(&with(vec![v(1, 20000.0)]), 1).is_err());
        assert!(validate_sheet(&with(vec![v(1, 50.0), v(1, 50.0)]), 1).is_err());
        assert!(validate_sheet(&with(vec![v(0, 50.0)]), 1).is_err());
        let nan = SheetView { at: [f64::NAN, 0.0], ..v(1, 50.0) };
        assert!(validate_sheet(&with(vec![nan]), 1).is_err());
        let cam = |w: f64, px: [u32; 2]| SheetView {
            number: 1,
            source: SheetSource::Camera { name: "c".into(), width: w, pixels: px },
            ..Default::default()
        };
        assert!(validate_sheet(&with(vec![cam(100.0, [64, 64])]), 1).is_ok());
        assert!(validate_sheet(&with(vec![cam(0.0, [64, 64])]), 1).is_err());
        assert!(validate_sheet(&with(vec![cam(100.0, [8, 64])]), 1).is_err());
        let plan = SheetView { source: SheetSource::Plan { view: "p".into(), level: Some(3) }, ..v(1, 50.0) };
        assert!(validate_sheet(&with(vec![plan]), 1).is_err());
        assert!(validate_sheet(&with((1..=33).map(|i| v(i, 50.0)).collect()), 1).is_err());
    }

    #[test]
    fn link_tags_read_back() {
        let t = link_tags(LINK_SHEET_VIEW, "Plans", 2);
        assert_eq!(t[0].code, 1001);
        assert_eq!(read_link(&t), Some(("sheet-view".to_string(), "Plans".to_string(), 2)));
        let mut x = vec![RawTag { code: 1001, value: "OTHER".into() }, RawTag { code: 1000, value: "keep".into() }];
        set_link(&mut x, LINK_SCHEDULE, "sc0", 0);
        set_link(&mut x, LINK_TITLE, "A1", 0);
        assert_eq!(x.iter().filter(|t| t.code == 1001).count(), 2);
        assert_eq!(x[1].value, "keep");
        assert_eq!(read_link(&x), Some(("title-block".to_string(), "A1".to_string(), 0)));
        assert_eq!(read_link(&[]), None);
        assert_eq!(read_link(&[RawTag { code: 1001, value: LINK_APP.into() }]), None);
    }
}
