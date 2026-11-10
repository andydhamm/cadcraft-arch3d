//! Save-time proxies for stair plan symbols (phase 17): each line of a stair's plan symbol as an
//! LWPOLYLINE on the stair's layer (dashed ones with linetype `DASHED`) and its UP / DN label as
//! TEXT, so a drawing opened elsewhere still shows them. Written only on save, dropped on read
//! while the arch record is present (like the electrical proxies).

use cadcraft_arch::stair_symbol::{StairSymbol, stair_symbol};
use cadcraft_doc::arch::{ArchModel, MAX_STAIRS};
use cadcraft_dxf::Tag;

/// Marker in the `CADCRAFT` xdata of every stair proxy.
pub(crate) const STAIR_PROXY_MARKER: &str = "CADCRAFT-STAIR";
/// Most proxies written for stairs, so a pathological count can't flood a save.
const MAX_STAIR_PROXIES: usize = 100_000;

/// `(entity kind, layer, tags)` for every stair proxy: the symbol lines of each stair for the
/// model's `plan_level` as `LWPOLYLINE`s, and its label as `TEXT`. Capped at 100 000 entities.
pub(crate) fn proxy_stair_entities(m: &ArchModel) -> Vec<(String, String, Vec<Tag>)> {
    let mut out: Vec<(String, String, Vec<Tag>)> = Vec::new();
    for s in m.stairs.iter().take(MAX_STAIRS) {
        let Some(sym) = stair_symbol(m, s, m.plan_level) else { continue };
        let layer = if s.layer.is_empty() { "A-STAIR" } else { s.layer.as_str() };
        push_symbol(&mut out, layer, &sym);
    }
    out
}

fn push_symbol(out: &mut Vec<(String, String, Vec<Tag>)>, layer: &str, sym: &StairSymbol) {
    for line in &sym.lines {
        if out.len() >= MAX_STAIR_PROXIES {
            return;
        }
        let Ok(n) = i64::try_from(line.points.len()) else { continue };
        if line.points.len() < 2 || line.points.iter().any(|p| !p.is_finite()) {
            continue;
        }
        let closed = line.points.first() == line.points.last() && line.points.len() > 2;
        let keep = if closed { line.points.len() - 1 } else { line.points.len() };
        let mut tags = Vec::new();
        if line.dashed {
            tags.push(Tag::s(6, "DASHED"));
        }
        tags.push(Tag::i(90, if closed { n - 1 } else { n }));
        tags.push(Tag::i(70, i64::from(closed)));
        for p in line.points.iter().take(keep) {
            tags.push(Tag::f(10, p.x));
            tags.push(Tag::f(20, p.y));
        }
        out.push(("LWPOLYLINE".into(), layer.into(), marked(tags, line.role.name())));
    }
    if let Some(l) = &sym.label {
        if out.len() < MAX_STAIR_PROXIES && l.at.is_finite() && l.angle.is_finite() && l.height.is_finite() {
            let tags = vec![
                Tag::f(10, l.at.x),
                Tag::f(20, l.at.y),
                Tag::f(30, 0.0),
                Tag::f(40, l.height),
                Tag::s(1, &l.text),
                Tag::f(50, l.angle.to_degrees()),
                Tag::s(7, "Standard"),
            ];
            out.push(("TEXT".into(), layer.into(), marked(tags, "label")));
        }
    }
}

/// Append the stair-proxy xdata (marker and role) to `tags`.
fn marked(mut tags: Vec<Tag>, role: &str) -> Vec<Tag> {
    tags.push(Tag::s(1001, crate::dxf_ext::APP));
    tags.push(Tag::s(1000, STAIR_PROXY_MARKER));
    tags.push(Tag::s(1000, role));
    tags
}

/// True when any proxy of `m` is dashed (the save then needs a `DASHED` linetype).
pub(crate) fn needs_dashed(proxies: &[(String, String, Vec<Tag>)]) -> bool {
    proxies.iter().any(|(_, _, t)| t.iter().any(|t| t.code == 6 && t.str() == "DASHED"))
}

/// True when an entity's `CADCRAFT` xdata marks it as a stair proxy.
pub(crate) fn is_stair_proxy(tags: &[Tag]) -> bool {
    crate::dxf_ext::xdata(tags, crate::dxf_ext::APP).iter().any(|t| t.code == 1000 && t.str().trim() == STAIR_PROXY_MARKER)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use cadcraft_doc::Drawing;
    use cadcraft_doc::arch::{Level, Stair};
    use cadcraft_geom::Vec2;

    fn drawing(level: Option<u32>) -> Drawing {
        let mut d = Drawing::new_metric();
        d.arch.levels.push(Level { index: 0, elevation: 0.0, ..Default::default() });
        d.arch.levels.push(Level { index: 1, elevation: 2.7, ..Default::default() });
        d.arch.push_stair(Stair {
            start: Vec2::new(2.0, 1.0),
            well: vec![Vec2::new(2.508, 0.5), Vec2::new(5.81, 0.5), Vec2::new(5.81, 1.5), Vec2::new(2.508, 1.5)],
            ..Stair::default()
        });
        d.arch.plan_level = level;
        d.touch_arch();
        d
    }

    fn role(tags: &[Tag]) -> String {
        let x = crate::dxf_ext::xdata(tags, crate::dxf_ext::APP);
        x.iter().filter(|t| t.code == 1000).map(|t| t.str().to_string()).nth(1).unwrap_or_default()
    }

    fn dashed(tags: &[Tag]) -> bool {
        tags.iter().any(|t| t.code == 6 && t.str() == "DASHED")
    }

    #[test]
    fn proxies_carry_roles_and_linetypes() {
        let p = proxy_stair_entities(&drawing(Some(0)).arch);
        assert!(!p.is_empty());
        assert!(p.iter().all(|(_, l, t)| l == "A-STAIR" && is_stair_proxy(t)));
        let nosing = |d: bool| p.iter().filter(|(k, _, t)| k == "LWPOLYLINE" && role(t) == "nosing" && dashed(t) == d).count();
        assert_eq!((nosing(false), nosing(true)), (7, 8));
        assert_eq!(p.iter().filter(|(_, _, t)| role(t) == "break").count(), 1);
        let text: Vec<_> = p.iter().filter(|(k, _, _)| k == "TEXT").collect();
        assert_eq!(text.len(), 1);
        assert!(text[0].2.iter().any(|t| t.code == 1 && t.str() == "UP"));
        assert_eq!(role(&text[0].2), "label");
        assert!(needs_dashed(&p));
        // A plain save (all levels) adds the closed well.
        let all = proxy_stair_entities(&drawing(None).arch);
        let well = all.iter().find(|(_, _, t)| role(t) == "well").unwrap();
        assert!(well.2.iter().any(|t| t.code == 70 && t.i64() == 1));
        assert_eq!(well.2.iter().filter(|t| t.code == 10).count(), 4);
    }

    #[test]
    fn upper_level_export_has_dn_and_no_dashed_lines() {
        let p = proxy_stair_entities(&drawing(Some(1)).arch);
        assert!(!needs_dashed(&p));
        assert!(p.iter().any(|(k, _, t)| k == "TEXT" && t.iter().any(|t| t.code == 1 && t.str() == "DN")));
        assert!(!p.iter().any(|(_, _, t)| t.iter().any(|t| t.code == 1 && t.str() == "UP")));
        assert!(proxy_stair_entities(&drawing(Some(2)).arch).is_empty());
    }

    #[test]
    fn proxies_are_dropped_on_read_while_the_arch_record_is_present_and_not_duplicated_on_resave() {
        let d = drawing(None);
        let text = crate::write_dxf(&d);
        let n = proxy_stair_entities(&d.arch).len();
        assert_eq!(text.matches(STAIR_PROXY_MARKER).count(), n);
        let back = crate::read_dxf(text.as_bytes()).unwrap();
        assert_eq!(back.arch.stairs.len(), 1);
        assert_eq!(back.model.len(), d.model.len(), "no stair proxy came back as an entity");
        let again = crate::write_dxf(&back);
        assert_eq!(again.matches(STAIR_PROXY_MARKER).count(), n);
    }

    #[test]
    fn dashed_linetype_is_written_when_missing() {
        let mut d = drawing(Some(0));
        d.linetypes.retain(|l| !l.name.eq_ignore_ascii_case("DASHED"));
        let text = crate::write_dxf(&d);
        assert!(text.contains("DASHED"));
        // The drawing itself is unchanged by the save.
        assert!(d.linetypes.iter().all(|l| !l.name.eq_ignore_ascii_case("DASHED")));
        // The LTYPE table names it (group 2), besides the entities' group 6.
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        assert!(lines.windows(2).any(|w| w[0] == "2" && w[1] == "DASHED"));
    }

    #[test]
    fn hostile_stair_writes_no_proxy() {
        let mut d = drawing(None);
        d.arch.stairs[0].width = f64::NAN;
        assert!(proxy_stair_entities(&d.arch).is_empty());
        let mut d = drawing(None);
        d.arch.stairs[0].well[1] = Vec2::new(f64::NAN, 0.0);
        assert!(proxy_stair_entities(&d.arch).iter().all(|(_, _, t)| role(t) != "well"));
    }

    #[test]
    fn proxies_are_capped() {
        let mut d = drawing(None);
        let s = d.arch.stairs[0].clone();
        for _ in 0..1600 {
            d.arch.push_stair(s.clone());
        }
        assert!(proxy_stair_entities(&d.arch).len() <= MAX_STAIR_PROXIES);
        let mut m = ArchModel::default();
        let mut big = s.clone();
        big.run = 10.0;
        big.tread_depth = 0.001;
        big.treads = usize::MAX;
        for _ in 0..60 {
            m.push_stair(big.clone());
        }
        assert!(proxy_stair_entities(&m).len() <= MAX_STAIR_PROXIES);
    }
}
