use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::{LayerRule, LayerState, MAX_LAYER_PATTERN, MAX_LAYER_RULES};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanKind {
    #[default]
    Floor,
    ReflectedCeiling,
    Roof,
}

impl PlanKind {
    pub fn key(self) -> &'static str {
        match self {
            PlanKind::Floor => "floor",
            PlanKind::ReflectedCeiling => "reflectedCeiling",
            PlanKind::Roof => "roof",
        }
    }

    pub fn from_key(s: &str) -> Option<Self> {
        match s {
            "floor" => Some(Self::Floor),
            "reflectedCeiling" | "reflected_ceiling" => Some(Self::ReflectedCeiling),
            "roof" => Some(Self::Roof),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AnnotationSet {
    pub layer: String,
    pub text_style: String,
    pub dim_style: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomLabelShow {
    pub name: bool,
    pub area: bool,
    pub dimensions: bool,
    pub ceiling: bool,
}

impl Default for RoomLabelShow {
    fn default() -> Self {
        Self { name: true, area: false, dimensions: false, ceiling: false }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceDisplay {
    pub level: u32,
    pub color: [u8; 3],
    pub transparency: u8,
}

impl Default for ReferenceDisplay {
    fn default() -> Self {
        Self { level: 0, color: [180, 180, 180], transparency: 50 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewZoom {
    pub center: [f64; 2],
    pub width: f64,
    pub height: f64,
}

impl Default for ViewZoom {
    fn default() -> Self {
        Self { center: [0.0, 0.0], width: 1.0, height: 1.0 }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PlanView {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
    pub kind: PlanKind,
    pub layers: LayerState,
    pub annotation: AnnotationSet,
    pub labels: RoomLabelShow,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<ReferenceDisplay>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom: Option<ViewZoom>,
}

impl Default for PlanView {
    fn default() -> Self {
        Self {
            name: String::new(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState { name: String::new(), layers: Vec::new(), rules: Vec::new() },
            annotation: AnnotationSet::default(),
            labels: RoomLabelShow::default(),
            reference: None,
            zoom: None,
        }
    }
}

pub const MAX_PLAN_VIEWS: usize = 64;
pub const MAX_PLAN_VIEW_NAME: usize = 64;

fn valid_layer_name(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 255 && !s.chars().any(|c| c.is_control() || "<>/\\\":;?*|,=`".contains(c))
}

fn valid_style_name(s: &str) -> bool {
    s.chars().count() <= 255 && !s.chars().any(|c| c.is_control())
}

pub fn validate_plan_view(v: &PlanView, level_count: usize) -> Result<(), String> {
    let trimmed = v.name.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_PLAN_VIEW_NAME || trimmed.chars().any(|c| c.is_control()) {
        return Err("plan view name must be 1..=64 chars without control characters".into());
    }
    if let Some(level) = v.level {
        if level as usize >= level_count {
            return Err(format!("plan view `{trimmed}` level `{level}` is out of range"));
        }
    }
    if v.layers.rules.len() > MAX_LAYER_RULES {
        return Err(format!("plan view `{trimmed}` has more than {MAX_LAYER_RULES} rules"));
    }
    for rule in &v.layers.rules {
        if rule.pattern.trim().is_empty() || rule.pattern.chars().count() > MAX_LAYER_PATTERN || rule.pattern.chars().any(|c| c.is_control()) {
            return Err(format!("plan view `{trimmed}` has a bad layer rule pattern"));
        }
        if let Some(t) = rule.transparency {
            if t > 90 {
                return Err(format!("plan view `{trimmed}` has transparency {t}, max 90"));
            }
        }
    }
    if !v.annotation.layer.is_empty() && !valid_layer_name(&v.annotation.layer) {
        return Err(format!("plan view `{trimmed}` has an invalid annotation layer"));
    }
    if !valid_style_name(&v.annotation.text_style) {
        return Err(format!("plan view `{trimmed}` has an invalid text style"));
    }
    if !valid_style_name(&v.annotation.dim_style) {
        return Err(format!("plan view `{trimmed}` has an invalid dimension style"));
    }
    if let Some(reference) = v.reference {
        if reference.level as usize >= level_count {
            return Err(format!("plan view `{trimmed}` reference level {} is out of range", reference.level));
        }
        if reference.transparency > 90 {
            return Err(format!("plan view `{trimmed}` reference transparency {} is above 90", reference.transparency));
        }
    }
    if let Some(zoom) = v.zoom {
        if !zoom.center.iter().all(|v| v.is_finite()) || !zoom.width.is_finite() || !zoom.height.is_finite() {
            return Err(format!("plan view `{trimmed}` zoom is not finite"));
        }
        if !(0.0 < zoom.width && zoom.width <= 1e9) || !(0.0 < zoom.height && zoom.height <= 1e9) {
            return Err(format!("plan view `{trimmed}` zoom width and height must be in (0, 1e9]"));
        }
    }
    if v.layers.layers.len() > 4096 {
        return Err(format!("plan view `{trimmed}` has more than 4096 saved layers"));
    }
    Ok(())
}

fn rule(pattern: &str, on: Option<bool>, transparency: Option<u8>) -> LayerRule {
    LayerRule { pattern: pattern.to_string(), on, transparency }
}

pub fn template_plan_views() -> Vec<PlanView> {
    vec![
        PlanView {
            name: "Working".into(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState { name: "Working".into(), layers: Vec::new(), rules: vec![rule("*", Some(true), Some(0))] },
            annotation: AnnotationSet { layer: "A-ANNO-NOTE".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: true, area: true, dimensions: false, ceiling: false },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Floor Plan".into(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState {
                name: "Floor Plan".into(),
                layers: Vec::new(),
                rules: vec![
                    rule("*", Some(true), Some(0)),
                    rule("E-*", Some(false), None),
                    rule("A-ROOF*", Some(false), None),
                    rule("C-*", Some(false), None),
                    rule("L-*", Some(false), None),
                ],
            },
            annotation: AnnotationSet { layer: "A-ANNO-NOTE".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: true, area: true, dimensions: true, ceiling: false },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Electrical".into(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState {
                name: "Electrical".into(),
                layers: Vec::new(),
                rules: vec![rule("*", None, Some(70)), rule("E-*", Some(true), Some(0))],
            },
            annotation: AnnotationSet { layer: "E-ANNO-NOTE".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: true, area: false, dimensions: false, ceiling: false },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Reflected Ceiling".into(),
            level: None,
            kind: PlanKind::ReflectedCeiling,
            layers: LayerState {
                name: "Reflected Ceiling".into(),
                layers: Vec::new(),
                rules: vec![
                    rule("*", Some(false), None),
                    rule("0", Some(true), Some(0)),
                    rule("A-WALL*", Some(true), Some(0)),
                    rule("A-GLAZ", Some(true), Some(0)),
                    rule("A-CLNG*", Some(true), Some(0)),
                    rule("E-LITE*", Some(true), Some(0)),
                    rule("A-ROOM*", Some(true), Some(0)),
                    rule("A-ANNO*", Some(true), Some(0)),
                    rule("A-ANNO-DIMS", Some(false), None),
                    rule("A-FLOR-CASE-UPPR", Some(true), Some(50)),
                ],
            },
            annotation: AnnotationSet { layer: "A-CLNG-ANNO".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: true, area: false, dimensions: false, ceiling: true },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Roof Plan".into(),
            level: None,
            kind: PlanKind::Roof,
            layers: LayerState {
                name: "Roof Plan".into(),
                layers: Vec::new(),
                rules: vec![
                    rule("*", Some(false), None),
                    rule("0", Some(true), Some(0)),
                    rule("A-ROOF*", Some(true), Some(0)),
                    rule("A-WALL", Some(true), Some(70)),
                    rule("A-ANNO*", Some(true), Some(0)),
                    rule("A-ANNO-DIMS", Some(false), None),
                ],
            },
            annotation: AnnotationSet { layer: "A-ROOF-ANNO".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Kitchen & Bath".into(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState {
                name: "Kitchen & Bath".into(),
                layers: Vec::new(),
                rules: vec![
                    rule("*", Some(true), Some(0)),
                    rule("I-FURN*", Some(true), Some(70)),
                    rule("E-*", Some(false), None),
                    rule("A-ROOF*", Some(false), None),
                    rule("C-*", Some(false), None),
                    rule("L-*", Some(false), None),
                ],
            },
            annotation: AnnotationSet { layer: "A-ANNO-NOTE".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: true, area: false, dimensions: true, ceiling: false },
            reference: None,
            zoom: None,
        },
        PlanView {
            name: "Site".into(),
            level: None,
            kind: PlanKind::Floor,
            layers: LayerState {
                name: "Site".into(),
                layers: Vec::new(),
                rules: vec![
                    rule("*", Some(false), None),
                    rule("0", Some(true), Some(0)),
                    rule("A-WALL", Some(true), Some(0)),
                    rule("A-ROOF*", Some(true), Some(0)),
                    rule("C-*", Some(true), Some(0)),
                    rule("L-*", Some(true), Some(0)),
                    rule("V-*", Some(true), Some(0)),
                    rule("A-ANNO*", Some(true), Some(0)),
                    rule("A-ANNO-DIMS", Some(false), None),
                ],
            },
            annotation: AnnotationSet { layer: "C-ANNO-NOTE".into(), text_style: String::new(), dim_style: String::new() },
            labels: RoomLabelShow { name: false, area: false, dimensions: false, ceiling: false },
            reference: None,
            zoom: None,
        },
    ]
}

#[allow(clippy::ptr_arg)]
pub fn is_template_plan_views(v: &Vec<PlanView>) -> bool {
    *v == template_plan_views()
}

pub fn repair_plan_views(model: &mut super::ArchModel) -> Vec<String> {
    let mut notes = Vec::new();
    let mut kept = Vec::new();
    let mut seen = HashSet::new();

    for mut view in std::mem::take(&mut model.plan_views) {
        let name = view.name.trim().to_string();
        if name.is_empty() || name.chars().count() > MAX_PLAN_VIEW_NAME || name.chars().any(|c| c.is_control()) {
            notes.push(format!("plan view dropped: invalid name `{name}`"));
            continue;
        }
        if let Some(level) = view.level {
            if level as usize >= model.levels.len() {
                notes.push(format!("plan view `{name}` level reset to `any`"));
                view.level = None;
            }
        }
        if let Some(reference) = &mut view.reference {
            if reference.level as usize >= model.levels.len() {
                notes.push(format!("plan view `{name}` reference level reset"));
                *reference = ReferenceDisplay::default();
            }
        }
        let key = name.to_ascii_lowercase();
        if !seen.insert(key) {
            notes.push(format!("plan view `{name}` dropped: duplicate name"));
            continue;
        }
        if validate_plan_view(&view, model.levels.len()).is_err() {
            notes.push(format!("plan view `{name}` dropped: invalid definition"));
            continue;
        }
        kept.push(view);
    }
    if kept.len() > MAX_PLAN_VIEWS {
        notes.push(format!("model has {} plan views, max {MAX_PLAN_VIEWS}", kept.len()));
        kept.truncate(MAX_PLAN_VIEWS);
    }
    model.plan_views = kept;

    let mut open = Vec::new();
    let mut seen_open = HashSet::new();
    for name in std::mem::take(&mut model.open_plan_views) {
        let name = name.trim();
        if name.is_empty() || !model.plan_view(name).is_some() {
            notes.push(format!("open plan view `{name}` dropped"));
            continue;
        }
        let key = name.to_ascii_lowercase();
        if !seen_open.insert(key) {
            notes.push(format!("open plan view `{name}` dropped: duplicate"));
            continue;
        }
        open.push(name.to_string());
    }
    model.open_plan_views = open;

    if let Some(active) = &model.active_plan_view {
        if model.plan_view(active).is_none() || !model.open_plan_views.iter().any(|n| n.eq_ignore_ascii_case(active)) {
            notes.push(format!("active plan view `{active}` cleared"));
            model.active_plan_view = None;
        }
    }
    notes
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn layer_rule_serde_round_trip_and_defaults() {
        let rule = LayerRule { pattern: "*".into(), on: Some(true), transparency: Some(0) };
        let json = serde_json::to_value(&rule).unwrap();
        assert_eq!(json, json!({ "pattern": "*", "on": true, "transparency": 0 }));
        let roundtrip: LayerRule = serde_json::from_value(json).unwrap();
        assert_eq!(roundtrip, rule);
        assert_eq!(LayerRule::default(), LayerRule { pattern: String::new(), on: None, transparency: None });
    }

    #[test]
    fn layer_state_without_rules_serializes_as_before() {
        let state = LayerState { name: "Saved".into(), layers: vec![crate::Layer::new("0")], rules: Vec::new() };
        let text = serde_json::to_string(&state).unwrap();
        assert!(!text.contains("\"rules\""), "rules should not serialize when empty");
        assert!(serde_json::from_str::<LayerState>(&text).unwrap().rules.is_empty());
    }

    #[test]
    fn seven_templates_in_order_with_their_rules_kinds_labels_and_annotation_layers() {
        let views = template_plan_views();
        assert_eq!(views.len(), 7);
        assert_eq!(
            views.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(),
            vec!["Working", "Floor Plan", "Electrical", "Reflected Ceiling", "Roof Plan", "Kitchen & Bath", "Site"]
        );
        assert_eq!(views[0].kind, PlanKind::Floor);
        assert_eq!(views[1].annotation.layer, "A-ANNO-NOTE");
        assert_eq!(views[3].labels.ceiling, true);
        assert_eq!(views[4].kind, PlanKind::Roof);
        assert_eq!(views[6].annotation.layer, "C-ANNO-NOTE");
        assert_eq!(views[0].layers.rules[0].pattern, "*");
        assert_eq!(views[1].layers.rules[3].pattern, "C-*");
    }

    #[test]
    fn template_views_are_not_serialized() {
        use crate::arch::ArchModel;
        let json = serde_json::to_value(ArchModel::default()).unwrap();
        assert!(json.get("plan_views").is_none(), "default model should omit the template view list");
    }

    #[test]
    fn deleted_templates_stay_deleted() {
        let mut model = super::super::ArchModel::default();
        model.plan_views.clear();
        let json = serde_json::to_value(&model).unwrap();
        assert_eq!(json["plan_views"], json!([]));
        assert!(serde_json::from_value::<super::super::ArchModel>(json).unwrap().plan_views.is_empty());
    }

    #[test]
    fn old_payload_loads_the_templates() {
        let json = json!({ "levels": [], "walls": [] });
        let model: super::super::ArchModel = serde_json::from_value(json).unwrap();
        assert_eq!(model.plan_views.len(), 7);
    }

    #[test]
    fn validate_plan_view_refuses_bad_views() {
        let good = PlanView::default();
        assert!(validate_plan_view(&good, 1).is_err());

        let mut bad_name = PlanView::default();
        bad_name.name = "x".repeat(65);
        assert!(validate_plan_view(&bad_name, 1).is_err());

        let mut bad_level = PlanView::default();
        bad_level.name = "Bad level".into();
        bad_level.level = Some(1);
        assert!(validate_plan_view(&bad_level, 1).is_err());

        let mut too_many_rules = PlanView::default();
        too_many_rules.name = "Rules".into();
        too_many_rules.layers.rules = vec![LayerRule::default(); MAX_LAYER_RULES + 1];
        assert!(validate_plan_view(&too_many_rules, 1).is_err());

        let mut bad_pattern = PlanView::default();
        bad_pattern.name = "Pattern".into();
        bad_pattern.layers.rules = vec![LayerRule { pattern: "x".repeat(MAX_LAYER_PATTERN + 1), on: Some(true), transparency: None }];
        assert!(validate_plan_view(&bad_pattern, 1).is_err());

        let mut bad_transparency = PlanView::default();
        bad_transparency.name = "Trans".into();
        bad_transparency.layers.rules = vec![LayerRule { pattern: "*".into(), on: Some(true), transparency: Some(91) }];
        assert!(validate_plan_view(&bad_transparency, 1).is_err());

        let mut bad_annotation = PlanView::default();
        bad_annotation.name = "Anno".into();
        bad_annotation.annotation.layer = "bad/name".into();
        assert!(validate_plan_view(&bad_annotation, 1).is_err());

        let mut bad_zoom = PlanView::default();
        bad_zoom.name = "Zoom".into();
        bad_zoom.zoom = Some(ViewZoom { center: [0.0, 0.0], width: 0.0, height: 1.0 });
        assert!(validate_plan_view(&bad_zoom, 1).is_err());

        let mut out_ref = PlanView::default();
        out_ref.name = "Ref".into();
        out_ref.reference = Some(ReferenceDisplay { level: 99, color: [0, 0, 0], transparency: 0 });
        assert!(validate_plan_view(&out_ref, 1).is_err());
    }

    #[test]
    fn model_validate_repairs_plan_views() {
        let mut model = super::super::ArchModel::default();
        model.plan_views.clear();
        model.plan_views.push(PlanView { name: "Alpha".into(), ..Default::default() });
        model.plan_views.push(PlanView { name: "alpha".into(), ..Default::default() });
        for i in 0..70 {
            model.plan_views.push(PlanView { name: format!("V{i}"), ..Default::default() });
        }
        model.open_plan_views = vec!["Alpha".into(), "Nope".into(), "Alpha".into()];
        model.active_plan_view = Some("Nope".into());
        model.levels.clear();
        let notes = super::super::ArchModel::validate(&mut model);
        assert!(!notes.is_empty());
        assert!(model.plan_views.len() <= MAX_PLAN_VIEWS);
        assert!(!model.open_plan_views.iter().any(|n| n.eq_ignore_ascii_case("Nope")));
        assert!(model.active_plan_view.is_none());
    }

    #[test]
    fn room_label_area_defaults_interior_and_is_skipped() {
        let model = super::super::Room::default();
        assert_eq!(model.label_area, crate::arch::LabelArea::Interior);
        let text = serde_json::to_string(&model).unwrap();
        assert!(!text.contains("labelArea"), "default room label area should be skipped");
        let roundtrip: super::super::Room = serde_json::from_str(&text).unwrap();
        assert_eq!(roundtrip.label_area, crate::arch::LabelArea::Interior);
    }
}
