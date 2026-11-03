//! IFC4 export (phase 12): ISO 16739 (`FILE_SCHEMA(('IFC4'))`) as ISO 10303-21 text from our own
//! writer ([`crate::step`]). The spatial structure is `IfcProject` → `IfcSite` → `IfcBuilding` →
//! one `IfcBuildingStorey` per level (placed at its elevation). Each element is a product whose
//! body is the 3D view's geometry ([`geometry::body`]), contained in its level's storey; ids come
//! from [`guid::global_id`], so two exports of one model are byte-identical given one timestamp
//! (`SOURCE_DATE_EPOCH`). The elements are written by one module each, in this order: walls,
//! openings (which need the walls), slabs, roofs, stairs, spaces, site, lights.

mod casework;
pub mod geometry;
pub mod guid;
mod lights;
mod openings;
mod roofs;
mod site;
mod slabs;
mod spaces;
mod stairs;
mod walls;

use crate::step::{P, Ref, StepHeader, StepWriter, iso_timestamp, source_date_epoch};
use cadcraft_arch3d::{Mesh, Scene3d};
use cadcraft_doc::Drawing;
use cadcraft_doc::arch::{ArchModel, Level};
use geometry::Pieces;
use guid::global_id;
use std::collections::BTreeMap;

/// What an IFC export needs besides the model.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct IfcOptions {
    /// Seconds since 1970-01-01 for the header's time stamp.
    pub timestamp: i64,
    /// The file name written in the header (`FILE_NAME`), e.g. `house.ifc`.
    pub file_name: String,
}

impl IfcOptions {
    /// `SOURCE_DATE_EPOCH` when set, else the current time (0 in the web build, which has no clock).
    pub fn from_env(file_name: &str) -> IfcOptions {
        IfcOptions { timestamp: source_date_epoch().unwrap_or_else(now), file_name: file_name.to_string() }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

#[cfg(target_arch = "wasm32")]
fn now() -> i64 {
    0
}

/// The drawing's arch model as an IFC4 file.
pub fn write_drawing(d: &Drawing, opts: &IfcOptions) -> String {
    write_model(&d.arch, &Scene3d::build(&d.arch), opts)
}

/// `model` (with `scene`, its 3D build) as an IFC4 file.
pub fn write_model(model: &ArchModel, scene: &Scene3d, opts: &IfcOptions) -> String {
    let mut ctx = Ctx::new(model, scene);
    walls::emit(&mut ctx);
    openings::emit(&mut ctx);
    slabs::emit(&mut ctx);
    roofs::emit(&mut ctx);
    stairs::emit(&mut ctx);
    spaces::emit(&mut ctx);
    site::emit(&mut ctx);
    lights::emit(&mut ctx);
    casework::emit(&mut ctx);
    ctx.finish(opts)
}

/// A storey of the export: the level it stands for and its entities.
#[allow(dead_code)]
pub(crate) struct Storey {
    /// The level's `index`.
    pub(crate) level: u32,
    /// The level's elevation (m): the storey's placement and its products' z origin.
    pub(crate) elevation: f64,
    pub(crate) entity: Ref,
    pub(crate) placement: Ref,
}

/// The export in progress: the writer, the model, the scene's meshes by object, the spatial
/// structure, and what each storey and the site contain.
#[allow(dead_code)]
pub(crate) struct Ctx<'a> {
    pub(crate) w: StepWriter,
    pub(crate) model: &'a ArchModel,
    pub(crate) pieces: Pieces<'a>,
    /// The `Body` representation subcontext every shape is in.
    pub(crate) body_context: Ref,
    pub(crate) site: Ref,
    pub(crate) site_placement: Ref,
    pub(crate) building: Ref,
    /// One per level in index order (one for a default level when the model has none).
    pub(crate) storeys: Vec<Storey>,
    /// Each wall's `IfcWall`, by `ArchId` number: written by `walls`, read by `openings`.
    pub(crate) walls: BTreeMap<u32, Ref>,
    contained: BTreeMap<usize, Vec<Ref>>,
    site_contained: Vec<Ref>,
    spaces: BTreeMap<usize, Vec<Ref>>,
}

// The element modules (filled by the phase-12 segments) call every helper here.
#[allow(dead_code)]
impl<'a> Ctx<'a> {
    fn new(model: &'a ArchModel, scene: &'a Scene3d) -> Ctx<'a> {
        let mut w = StepWriter::new();
        let origin = w.add("IfcCartesianPoint", vec![P::reals(&[0.0, 0.0, 0.0])]);
        let z = w.add("IfcDirection", vec![P::reals(&[0.0, 0.0, 1.0])]);
        let x = w.add("IfcDirection", vec![P::reals(&[1.0, 0.0, 0.0])]);
        let world = w.add("IfcAxis2Placement3D", vec![P::Ref(origin), P::Ref(z), P::Ref(x)]);
        // True north in the drawing's XY: the site's `true_north` degrees counter-clockwise from +Y.
        let tn = model.site.as_ref().map_or(0.0, |s| s.true_north).to_radians();
        let north = w.add("IfcDirection", vec![P::reals(&[-tn.sin(), tn.cos()])]);
        let context =
            w.add("IfcGeometricRepresentationContext", vec![P::Null, P::str("Model"), P::Int(3), P::Real(1.0e-5), P::Ref(world), P::Ref(north)]);
        let body_context = w.add(
            "IfcGeometricRepresentationSubContext",
            vec![
                P::str("Body"),
                P::str("Model"),
                P::Derived,
                P::Derived,
                P::Derived,
                P::Derived,
                P::Ref(context),
                P::Null,
                P::Enum("MODEL_VIEW"),
                P::Null,
            ],
        );
        let units: Vec<Ref> = [("LENGTHUNIT", "METRE"), ("AREAUNIT", "SQUARE_METRE"), ("VOLUMEUNIT", "CUBIC_METRE"), ("PLANEANGLEUNIT", "RADIAN")]
            .iter()
            .map(|(kind, name)| w.add("IfcSIUnit", vec![P::Derived, P::Enum(kind), P::Null, P::Enum(name)]))
            .collect();
        let unit_assignment = w.add("IfcUnitAssignment", vec![P::refs(&units)]);
        let project = w.add(
            "IfcProject",
            vec![
                P::str(global_id("project")),
                P::Null,
                P::str("CADCraft project"),
                P::Null,
                P::Null,
                P::Null,
                P::Null,
                P::refs(&[context]),
                P::Ref(unit_assignment),
            ],
        );
        let site_placement = w.add("IfcLocalPlacement", vec![P::Null, P::Ref(world)]);
        let [lat, lon, elev] = site::references(model);
        let site = w.add(
            "IfcSite",
            vec![
                P::str(global_id("site")),
                P::Null,
                P::str("Site"),
                P::Null,
                P::Null,
                P::Ref(site_placement),
                P::Null,
                P::Null,
                P::Enum("ELEMENT"),
                lat,
                lon,
                elev,
                P::Null,
                P::Null,
            ],
        );
        let building_placement = w.add("IfcLocalPlacement", vec![P::Ref(site_placement), P::Ref(world)]);
        let building = w.add(
            "IfcBuilding",
            vec![
                P::str(global_id("building")),
                P::Null,
                P::str("Building"),
                P::Null,
                P::Null,
                P::Ref(building_placement),
                P::Null,
                P::Null,
                P::Enum("ELEMENT"),
                P::Null,
                P::Null,
                P::Null,
            ],
        );
        let mut levels: Vec<Level> = model.levels.clone();
        levels.sort_by_key(|l| l.index);
        if levels.is_empty() {
            levels.push(Level::default());
        }
        let mut storeys = Vec::with_capacity(levels.len());
        for l in &levels {
            let at = w.add("IfcCartesianPoint", vec![P::reals(&[0.0, 0.0, l.elevation])]);
            let axes = w.add("IfcAxis2Placement3D", vec![P::Ref(at), P::Ref(z), P::Ref(x)]);
            let placement = w.add("IfcLocalPlacement", vec![P::Ref(building_placement), P::Ref(axes)]);
            let name = if l.name.trim().is_empty() { format!("Level {}", l.index) } else { l.name.clone() };
            let entity = w.add(
                "IfcBuildingStorey",
                vec![
                    P::str(global_id(&format!("storey/{}", l.index))),
                    P::Null,
                    P::Str(name),
                    P::Null,
                    P::Null,
                    P::Ref(placement),
                    P::Null,
                    P::Null,
                    P::Enum("ELEMENT"),
                    P::Real(l.elevation),
                ],
            );
            storeys.push(Storey { level: l.index, elevation: l.elevation, entity, placement });
        }
        let mut ctx = Ctx {
            w,
            model,
            pieces: Pieces::new(scene),
            body_context,
            site,
            site_placement,
            building,
            storeys,
            walls: BTreeMap::new(),
            contained: BTreeMap::new(),
            site_contained: Vec::new(),
            spaces: BTreeMap::new(),
        };
        ctx.rel("IfcRelAggregates", "rel/aggregates/project", vec![P::Ref(project), P::refs(&[site])]);
        ctx.rel("IfcRelAggregates", "rel/aggregates/site", vec![P::Ref(site), P::refs(&[building])]);
        let storey_refs: Vec<Ref> = ctx.storeys.iter().map(|s| s.entity).collect();
        ctx.rel("IfcRelAggregates", "rel/aggregates/building", vec![P::Ref(building), P::refs(&storey_refs)]);
        ctx
    }

    /// The storey (its slot in [`Ctx::storeys`]) of level `index`: the storey of that level, else
    /// the last one, as the 3D build places an object whose level is gone.
    pub(crate) fn storey_for(&self, index: u32) -> usize {
        self.storeys.iter().position(|s| s.level == index).unwrap_or(self.storeys.len().saturating_sub(1))
    }

    /// A placement in storey `slot` at its origin (products' coordinates are the storey's).
    pub(crate) fn placement_in_storey(&mut self, slot: usize) -> Ref {
        let parent = self.storeys.get(slot).map_or(self.site_placement, |s| s.placement);
        let axes = self.world_axes();
        self.w.add("IfcLocalPlacement", vec![P::Ref(parent), P::Ref(axes)])
    }

    /// A placement in the site at its origin (the site's coordinates are the model's).
    pub(crate) fn placement_in_site(&mut self) -> Ref {
        let axes = self.world_axes();
        self.w.add("IfcLocalPlacement", vec![P::Ref(self.site_placement), P::Ref(axes)])
    }

    fn world_axes(&mut self) -> Ref {
        let origin = self.w.add("IfcCartesianPoint", vec![P::reals(&[0.0, 0.0, 0.0])]);
        self.w.add("IfcAxis2Placement3D", vec![P::Ref(origin), P::Null, P::Null])
    }

    /// The body of `meshes` with z measured from `z0` ([`geometry::body`]).
    pub(crate) fn body(&mut self, meshes: &[&Mesh], z0: f64) -> Option<Ref> {
        geometry::body(&mut self.w, self.body_context, meshes, z0)
    }

    /// Write an element of storey `slot`: `entity(GlobalId of key, $, name, $, $, placement, body,
    /// …tail)` where `tail` is everything after `IfcProduct`'s seven attributes (for `IfcElement`s
    /// the `Tag` first), and contain it in the storey.
    pub(crate) fn element(&mut self, entity: &str, key: &str, name: &str, slot: usize, meshes: &[&Mesh], tail: Vec<P>) -> Ref {
        let placement = self.placement_in_storey(slot);
        let z0 = self.storeys.get(slot).map_or(0.0, |s| s.elevation);
        let body = self.body(meshes, z0).map_or(P::Null, P::Ref);
        let mut params = vec![P::str(global_id(key)), P::Null, P::str(name), P::Null, P::Null, P::Ref(placement), body];
        params.extend(tail);
        let r = self.w.add(entity, params);
        self.contain(slot, r);
        r
    }

    /// Contain `product` in storey `slot` (one `IfcRelContainedInSpatialStructure` per storey).
    pub(crate) fn contain(&mut self, slot: usize, product: Ref) {
        self.contained.entry(slot).or_default().push(product);
    }

    /// Contain `product` in the site (geographic elements).
    pub(crate) fn contain_in_site(&mut self, product: Ref) {
        self.site_contained.push(product);
    }

    /// Aggregate the `IfcSpace` `space` in storey `slot` (spaces decompose a storey).
    pub(crate) fn aggregate_space(&mut self, slot: usize, space: Ref) {
        self.spaces.entry(slot).or_default().push(space);
    }

    /// Write a relationship `entity(GlobalId of key, $, $, $, …rest)`.
    pub(crate) fn rel(&mut self, entity: &str, key: &str, rest: Vec<P>) -> Ref {
        let mut params = vec![P::str(global_id(key)), P::Null, P::Null, P::Null];
        params.extend(rest);
        self.w.add(entity, params)
    }

    /// A property set `name` of single values on `related`: `IfcPropertySingleValue`s, the
    /// `IfcPropertySet` (id `pset/<name>/<owner>`) and its `IfcRelDefinesByProperties`
    /// (`rel/props/<name>/<owner>`). `owner` names the object, e.g. `wall/3`.
    pub(crate) fn pset(&mut self, name: &str, owner: &str, props: Vec<(&str, P)>, related: &[Ref]) -> Ref {
        let values: Vec<Ref> = props.into_iter().map(|(n, v)| self.w.add("IfcPropertySingleValue", vec![P::str(n), P::Null, v, P::Null])).collect();
        let set =
            self.w.add("IfcPropertySet", vec![P::str(global_id(&format!("pset/{name}/{owner}"))), P::Null, P::str(name), P::Null, P::refs(&values)]);
        self.rel("IfcRelDefinesByProperties", &format!("rel/props/{name}/{owner}"), vec![P::refs(related), P::Ref(set)]);
        set
    }

    fn finish(mut self, opts: &IfcOptions) -> String {
        for (slot, spaces) in std::mem::take(&mut self.spaces) {
            if let Some(s) = self.storeys.get(slot) {
                let (level, storey) = (s.level, s.entity);
                self.rel("IfcRelAggregates", &format!("rel/aggregates/storey/{level}"), vec![P::Ref(storey), P::refs(&spaces)]);
            }
        }
        for (slot, products) in std::mem::take(&mut self.contained) {
            if let Some(s) = self.storeys.get(slot) {
                let (level, storey) = (s.level, s.entity);
                self.rel("IfcRelContainedInSpatialStructure", &format!("rel/contains/storey/{level}"), vec![P::refs(&products), P::Ref(storey)]);
            }
        }
        let site_products = std::mem::take(&mut self.site_contained);
        if !site_products.is_empty() {
            let site = self.site;
            self.rel("IfcRelContainedInSpatialStructure", "rel/contains/site", vec![P::refs(&site_products), P::Ref(site)]);
        }
        let header = StepHeader {
            description: vec!["ViewDefinition [ReferenceView_V1.2]".into()],
            name: opts.file_name.clone(),
            time_stamp: iso_timestamp(opts.timestamp),
            author: vec![String::new()],
            organization: vec![String::new()],
            preprocessor_version: "CADCraft arch3d".into(),
            originating_system: "CADCraft".into(),
            schema: vec!["IFC4".into()],
            ..StepHeader::default()
        };
        self.w.finish(&header)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn export(model: &ArchModel) -> String {
        write_model(model, &Scene3d::build(model), &IfcOptions { timestamp: 1_793_556_000, file_name: "t.ifc".into() })
    }

    /// (entity name, raw parameter text) by instance number, from the DATA section.
    fn instances(text: &str) -> BTreeMap<u32, (String, String)> {
        let mut out = BTreeMap::new();
        for line in text.lines().filter(|l| l.starts_with('#')) {
            let (n, rest) = line[1..].split_once('=').unwrap_or_default();
            let (name, params) = rest.split_once('(').unwrap_or_default();
            out.insert(n.parse().unwrap_or(0), (name.to_string(), params.trim_end_matches(");").to_string()));
        }
        out
    }

    fn refs_in(params: &str) -> Vec<u32> {
        let mut out = Vec::new();
        let b = params.as_bytes();
        let mut i = 0;
        let mut quoted = false;
        while i < b.len() {
            if b[i] == b'\'' {
                quoted = !quoted;
            } else if !quoted && b[i] == b'#' {
                let start = i + 1;
                i = start;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
                out.push(params[start..i].parse().unwrap_or(0));
                continue;
            }
            i += 1;
        }
        out
    }

    fn count(inst: &BTreeMap<u32, (String, String)>, name: &str) -> usize {
        inst.values().filter(|(n, _)| n == name).count()
    }

    #[test]
    fn an_empty_model_is_a_project_site_building_and_one_storey() {
        let text = export(&ArchModel::default());
        assert!(text.contains("FILE_SCHEMA(('IFC4'));"), "{text}");
        assert!(text.contains("FILE_NAME('t.ifc','2026-11-01T18:00:00',"), "{text}");
        let inst = instances(&text);
        for (name, n) in
            [("IFCPROJECT", 1), ("IFCSITE", 1), ("IFCBUILDING", 1), ("IFCBUILDINGSTOREY", 1), ("IFCRELAGGREGATES", 3), ("IFCUNITASSIGNMENT", 1)]
        {
            assert_eq!(count(&inst, name), n, "{name}");
        }
        assert_eq!(count(&inst, "IFCRELCONTAINEDINSPATIALSTRUCTURE"), 0, "nothing to contain");
    }

    #[test]
    fn every_reference_is_defined_and_every_id_is_unique() {
        let mut model = ArchModel::default();
        model.levels = vec![Level { index: 0, name: "Ground".into(), ..Level::default() }, Level { index: 1, elevation: 2.7, ..Level::default() }];
        let text = export(&model);
        let inst = instances(&text);
        let defined: BTreeSet<u32> = inst.keys().copied().collect();
        for (n, (name, params)) in &inst {
            for r in refs_in(params) {
                assert!(defined.contains(&r), "#{n} {name} refers to #{r}, which is not defined");
            }
        }
        let ids: Vec<&str> = inst.values().filter_map(|(_, p)| p.strip_prefix('\'').and_then(|p| p.get(..22))).collect();
        assert_eq!(ids.len(), ids.iter().collect::<BTreeSet<_>>().len(), "GlobalIds repeat");
        assert!(text.contains("IFCBUILDINGSTOREY('") && text.contains(",'Ground',$,$,"), "{text}");
        assert!(text.contains(",'Level 1',$,$,") && text.contains(",.ELEMENT.,2.7);"), "the second storey is named for its index and at 2.7: {text}");
    }

    #[test]
    fn two_exports_of_one_model_are_byte_identical() {
        let mut model = ArchModel::default();
        model.levels = vec![Level::default()];
        assert_eq!(export(&model), export(&model));
    }

    #[test]
    fn a_level_without_a_storey_falls_back_to_the_last() {
        let mut model = ArchModel::default();
        model.levels = vec![Level { index: 0, ..Level::default() }, Level { index: 3, elevation: 3.0, ..Level::default() }];
        let scene = Scene3d::default();
        let ctx = Ctx::new(&model, &scene);
        assert_eq!((ctx.storey_for(0), ctx.storey_for(3), ctx.storey_for(7)), (0, 1, 1));
    }
}
