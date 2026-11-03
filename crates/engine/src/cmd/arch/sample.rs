//! `arch.sample` — inject a deterministic sample arch model (benchmark fixture).
//!
//! Replaces `Drawing::arch.walls` with a seeded xorshift64* wall chain (same
//! bit pattern as `apps/cadcraft-cli/src/main.rs::Rng`, seed
//! `0x9E37_79B9_7F4A_7C15`). The chain is connected: wall `i` starts where
//! wall `i-1` ended, the heading turns a right angle every `4 + (rng % 3)`
//! walls, length in [3.0, 8.9) drawing units, thickness alternating 0.2/0.1,
//! height 2.6, ids `w1..wN` (handles `arch_handle(i)`).
//!
//! The optional `openings` parameter (default `0`, capped at `MAX_OPENINGS`)
//! deterministically seeds openings on top of the chain, one per wall, using
//! two waves so no separation conflict is ever possible: wave 0 seeds the
//! even 1-based walls and wave 1 the odd ones, each candidate at 40 % of the
//! centre-line length, choosing the hinged door (0.9 m × 2.1 m) when the
//! 1-based index is a multiple of 3 and the sliding window (1.2 m × 1.2 m,
//! sill 0.9 m) otherwise. Walls shorter than 2.0 m and any placement rejected
//! by [`cadcraft_arch::opening::validate_placement`] are skipped, so the
//! command never fails. Openings are pushed through `ArchModel::push_opening`,
//! so ids continue the shared counter after the wall ids. Same seed ⇒ same
//! output.
//!
//! This command *does* mutate the model and calls `touch_arch()`; it is
//! `.noundo()` only because it is a benchmark fixture, not a user-facing edit.

use serde_json::{Value, json};

use crate::Session;
use cadcraft_arch::opening::validate_placement;
use cadcraft_doc::arch::{ArchId, ArchModel, Level, MAX_OPENINGS, Opening, OpeningKind};

/// Seeded xorshift64*, copied bit-for-bit from the CLI's `Rng`.
struct Rng(u64);

impl Rng {
    /// Advance the state and return the next 64-bit word.
    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// The next [0.0, 1.0) value (same as the CLI's `next`).
    fn next(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Generate `n` connected walls. The chain is fully determined by `n` and the
/// fixed seed, so two fresh runs with the same `n` produce identical data.
fn generate_walls(n: usize) -> Vec<cadcraft_doc::Wall> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let mut walls: Vec<cadcraft_doc::Wall> = Vec::with_capacity(n);
    let mut pos = cadcraft_geom::Vec2::ZERO;
    let mut dir = 0usize; // 0:+x 1:+y 2:-x 3:-y (right turn = clockwise)
    // Right-angle turn every 4 + (rng % 3) walls (4..=6 walls per leg).
    let mut legs_left: usize = (4 + (rng.next_u64() % 3)) as usize;
    let mut k = 0usize; // 1-based wall index

    while walls.len() < n {
        // Length: 3.0 + (rng % 60) as f64 / 10.0 → [3.0, 8.9) in 0.1 steps.
        // Never zero with the seeded generator; if a degenerate value ever
        // comes back, skip it and retry with a fresh one, bounded retries.
        let mut length: f64 = 0.0;
        let mut tried = 0usize;
        while length <= 0.0 || !length.is_finite() || tried < 32 {
            length = 3.0 + ((rng.next() * 60.0).floor() as f64) / 10.0;
            tried += 1;
        }
        if !(length > 0.0) {
            length = 3.0;
        }

        let (dx, dy) = match dir {
            0 => (1.0, 0.0),
            1 => (0.0, 1.0),
            2 => (-1.0, 0.0),
            _ => (0.0, -1.0),
        };
        let end = pos + cadcraft_geom::Vec2::new(dx * length, dy * length);

        walls.push(cadcraft_doc::Wall {
            id: cadcraft_doc::ArchId((k + 1) as u32),
            start: pos,
            end,
            thickness: if (k + 1) % 2 == 1 { 0.2 } else { 0.1 },
            height: 2.6,
            ..cadcraft_doc::Wall::default()
        });

        pos = end;
        k += 1;

        legs_left = legs_left.saturating_sub(1);
        if legs_left == 0 {
            dir = (dir + 1) % 4;
            legs_left = (4 + (rng.next_u64() % 3)) as usize;
        }
    }

    walls
}

/// Minimum wall length (m) for a sample opening to be seeded on a wall.
const MIN_SAMPLE_WALL: f64 = 2.0;

/// One opening attempt to seed: kind, finished width/height, sill, style.
type OpeningSpec = (OpeningKind, f64, f64, f64, &'static str);

/// Deterministically seed up to `max` openings on the given model, one per
/// wall.
///
/// Two waves so no separation conflict is ever possible: wave 0 seeds the
/// even 1-based walls, wave 1 the odd ones (the other parity is skipped with
/// `continue`), and each candidate wall gets at most one opening. Every
/// generated wall is 3.0-8.9 m long and 2.6 m high, so a single 0.9×2.1 door
/// or 1.2×1.2 window (sill 0.9 m) at 40 % of the centre-line always passes
/// [`validate_placement`] (min segment 0.15 m, rough allowance 0.12 m),
/// meaning wave 0 alone yields `floor(n/2)` openings — 150 for 300 walls —
/// and wave 1 doubles the capacity to `n`. `max` is already clamped to
/// `MAX_OPENINGS` in `run_sample`. Returns the number actually placed; never
/// fails.
fn seed_openings(model: &mut ArchModel, max: usize) -> usize {
    const DOOR: OpeningSpec = (OpeningKind::Door, 0.9, 2.1, 0.0, "hinged");
    const WINDOW: OpeningSpec = (OpeningKind::Window, 1.2, 1.2, 0.9, "slider");

    let mut placed = 0usize;
    // Index-based loop: the only thing copied out is `wid` (a `Copy` ArchId)
    // and a plain `f64`, so no borrow of `model.walls` is live when
    // `model.push_opening(o)` mutates the model.
    for wave in 0..2 {
        for i in 0..model.walls.len() {
            if placed >= max {
                return placed;
            }
            // Wave 0: even 1-based walls; wave 1: odd ones.
            if (i + 1) % 2 != wave {
                continue;
            }
            let wid = model.walls[i].id;
            let length = model.walls[i].start.dist(model.walls[i].end);
            if !length.is_finite() || length < MIN_SAMPLE_WALL {
                continue;
            }
            let spec = if (i + 1) % 3 == 0 { DOOR } else { WINDOW };
            let (kind, width, height, sill, style) = spec;
            let offset = 0.4 * length;

            match validate_placement(model, wid, offset, width, sill, height, kind, style, None) {
                Ok(off) => {
                    let mut o = Opening::new(wid, off, width, height, kind);
                    o.sill = sill;
                    o.style = style.to_string();
                    model.push_opening(o);
                    placed += 1;
                }
                Err(_) => {
                    continue;
                }
            }
            if placed >= max {
                return placed;
            }
        }
    }
    placed
}

/// The four 0.2 m exterior walls of the 10×8 m phase-1 house on `level`,
/// centre lines (0,0)→(10,0)→(10,8)→(0,8)→(0,0).
fn house_exterior(level: u32) -> Vec<cadcraft_doc::Wall> {
    let w = |s: (f64, f64), e: (f64, f64), lvl: u32| -> cadcraft_doc::Wall {
        cadcraft_doc::Wall {
            start: cadcraft_geom::Vec2::new(s.0, s.1),
            end: cadcraft_geom::Vec2::new(e.0, e.1),
            base_level: lvl,
            ..cadcraft_doc::Wall::default()
        }
    };
    vec![w((0.0, 0.0), (10.0, 0.0), level), w((10.0, 0.0), (10.0, 8.0), level), w((10.0, 8.0), (0.0, 8.0), level), w((0.0, 8.0), (0.0, 0.0), level)]
}

/// The thickness-0 interior divider (5,0)→(5,8) on `level`.
fn central_divider(level: u32) -> cadcraft_doc::Wall {
    cadcraft_doc::Wall {
        start: cadcraft_geom::Vec2::new(5.0, 0.0),
        end: cadcraft_geom::Vec2::new(5.0, 8.0),
        base_level: level,
        exterior: false,
        wall_type: "Interior".into(),
        thickness: 0.0,
        divider: true,
        ..cadcraft_doc::Wall::default()
    }
}

fn push_benchmark_wall(
    model: &mut ArchModel,
    start: cadcraft_geom::Vec2,
    end: cadcraft_geom::Vec2,
    level: u32,
    exterior: bool,
    thickness: f64,
) -> ArchId {
    let mut w = cadcraft_doc::Wall::new(start, end, thickness);
    w.base_level = level;
    w.exterior = exterior;
    w.height = 2.7;
    w.wall_type = if exterior { "Exterior".into() } else { "Interior".into() };
    w.to_ceiling = true;
    w.id = model.next_wall_id();
    model.push_wall(w)
}

/// Grid lines of the benchmark house plan (centre lines): 9 bays of 2 m across
/// x and 6 of 2 m across y, an 18 × 12 m footprint.
const HOUSE_X: [f64; 10] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0, 14.0, 16.0, 18.0];
const HOUSE_Y: [f64; 7] = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0, 12.0];

/// A closet in the top-right corner of a bay: `CLOSET_W` along x, `CLOSET_D`
/// along y. Its two partitions meet in an L and end on the middle of the bay's
/// top wall and on the bay's right wall (T joins on the walls' sides).
const CLOSET_W: f64 = 1.0;
const CLOSET_D: f64 = 0.6;

/// Bays (column, row) that get a closet: a checkerboard of the 8 × 4 bays whose
/// top and right walls are partitions, so a closet never touches the ring.
fn house_closet_bays() -> impl Iterator<Item = (usize, usize)> {
    (0..4usize).flat_map(|row| (0..8usize).map(move |col| (col, row))).filter(|(col, row)| (col + row) % 2 == 0)
}

/// Build the phase-13 benchmark house fixture: two identical levels on an
/// 18 × 12 m footprint. Per level: the exterior ring split into 30 segments
/// where the partitions meet it (T joins), a 2 m grid of partitions split at
/// every crossing (X joins: four ends at one point), 16 closets whose two walls
/// meet in an L and end on other walls' sides (T joins), and the ring's four
/// corners (L joins): 155 walls per level, 310 in all. 60 windows (one per ring
/// segment) and 100 doors on partitions that no closet ends on; rooms come from
/// `detect_rooms`; Level 2 gets the automatic hip roof `roofauto` would make.
/// The bench edit pulls the end of the Level 1 partition (8,8)→(8,10) 0.3 m
/// back along itself; `arch.wall.stretch` takes the X join at (8,10) with it.
fn build_benchmark_house() -> crate::Result<(ArchModel, Value)> {
    use cadcraft_geom::Vec2;
    let err = |m: String| crate::EngineError::Other(format!("benchmark house: {m}"));
    let mut model = ArchModel::default();
    model.levels = vec![
        Level { index: 1, name: "Level 1".into(), elevation: 0.0, ceiling_height: 2.4, structure: 0.3, ..Default::default() },
        Level { index: 2, name: "Level 2".into(), elevation: 2.7, ceiling_height: 2.4, structure: 0.3, ..Default::default() },
    ];
    let (nx, ny) = (HOUSE_X.len() - 1, HOUSE_Y.len() - 1);
    let (xmax, ymax) = (HOUSE_X[nx], HOUSE_Y[ny]);
    let mut hosts = std::collections::BTreeSet::new();
    let mut walls_by_level: Vec<(Vec<ArchId>, Vec<ArchId>)> = Vec::new();
    for level in 1u32..=2 {
        let mut ring = Vec::new();
        for i in 0..nx {
            ring.push(push_benchmark_wall(&mut model, Vec2::new(HOUSE_X[i], 0.0), Vec2::new(HOUSE_X[i + 1], 0.0), level, true, 0.2));
        }
        for j in 0..ny {
            ring.push(push_benchmark_wall(&mut model, Vec2::new(xmax, HOUSE_Y[j]), Vec2::new(xmax, HOUSE_Y[j + 1]), level, true, 0.2));
        }
        for i in (0..nx).rev() {
            ring.push(push_benchmark_wall(&mut model, Vec2::new(HOUSE_X[i + 1], ymax), Vec2::new(HOUSE_X[i], ymax), level, true, 0.2));
        }
        for j in (0..ny).rev() {
            ring.push(push_benchmark_wall(&mut model, Vec2::new(0.0, HOUSE_Y[j + 1]), Vec2::new(0.0, HOUSE_Y[j]), level, true, 0.2));
        }
        // Partitions: x-line i between bays i-1 and i, segment j spans row j; y-line j likewise.
        let mut x_seg = std::collections::BTreeMap::new();
        let mut y_seg = std::collections::BTreeMap::new();
        let mut partitions = Vec::new();
        for i in 1..nx {
            for j in 0..ny {
                let id = push_benchmark_wall(&mut model, Vec2::new(HOUSE_X[i], HOUSE_Y[j]), Vec2::new(HOUSE_X[i], HOUSE_Y[j + 1]), level, false, 0.12);
                x_seg.insert((i, j), id);
                partitions.push(id);
            }
        }
        for j in 1..ny {
            for i in 0..nx {
                let id = push_benchmark_wall(&mut model, Vec2::new(HOUSE_X[i], HOUSE_Y[j]), Vec2::new(HOUSE_X[i + 1], HOUSE_Y[j]), level, false, 0.12);
                y_seg.insert((i, j), id);
                partitions.push(id);
            }
        }
        for (col, row) in house_closet_bays() {
            let (right, top) = (HOUSE_X[col + 1], HOUSE_Y[row + 1]);
            let corner = Vec2::new(right - CLOSET_W, top - CLOSET_D);
            push_benchmark_wall(&mut model, Vec2::new(right - CLOSET_W, top), corner, level, false, 0.12);
            push_benchmark_wall(&mut model, corner, Vec2::new(right, top - CLOSET_D), level, false, 0.12);
            hosts.extend(y_seg.get(&(col, row + 1)).copied());
            hosts.extend(x_seg.get(&(col + 1, row)).copied());
        }
        walls_by_level.push((ring, partitions));
    }

    // One window centred on every ring segment, then doors centred on the
    // partitions no closet ends on, 50 per level.
    const DOORS_PER_LEVEL: usize = 50;
    for (ring, partitions) in &walls_by_level {
        for &id in ring {
            let len = model.wall(id).map(|w| w.start.dist(w.end)).ok_or_else(|| err(format!("ring wall {id} missing")))?;
            validate_placement(&model, id, len / 2.0, 1.2, 0.9, 1.2, OpeningKind::Window, "slider", None)
                .map_err(|e| err(format!("window on {id}: {}", e.0)))?;
            let mut o = Opening::new(id, len / 2.0, 1.2, 1.2, OpeningKind::Window);
            o.sill = 0.9;
            o.style = "slider".to_string();
            model.push_opening(o);
        }
        for &id in partitions.iter().filter(|id| !hosts.contains(*id)).take(DOORS_PER_LEVEL) {
            let len = model.wall(id).map(|w| w.start.dist(w.end)).ok_or_else(|| err(format!("partition {id} missing")))?;
            validate_placement(&model, id, len / 2.0, 0.9, 0.0, 2.1, OpeningKind::Door, "hinged", None)
                .map_err(|e| err(format!("door on {id}: {}", e.0)))?;
            let mut o = Opening::new(id, len / 2.0, 0.9, 2.1, OpeningKind::Door);
            o.style = "hinged".to_string();
            o.sill = 0.0;
            model.push_opening(o);
        }
    }

    for room in cadcraft_arch::room::detect_rooms(&model) {
        model.push_room(room);
    }

    let auto = cadcraft_doc::Roof { level: 2, pitch_rise: 6.0, pitch_run: 12.0, overhang: 0.5, thickness: 0.2, fascia: true, soffit: true, ..Default::default() };
    let roof = cadcraft_arch::roof::build_roof(&model, 2, &auto).map_err(|e| err(format!("roof: {e:?}")))?;
    model.push_roof(roof);

    let (from, to) = (Vec2::new(8.0, 10.0), Vec2::new(8.0, 9.7));
    let bench_wall = model
        .walls
        .iter()
        .find(|w| w.base_level == 1 && !w.exterior && w.start == Vec2::new(8.0, 8.0) && w.end == from)
        .ok_or_else(|| err("no Level 1 partition (8,8)→(8,10) for the bench edit".into()))?;
    let bench = json!({
        "wall": format!("w{}", bench_wall.id.0),
        "end": "end",
        "from": [from.x, from.y],
        "to": [to.x, to.y],
    });

    Ok((model, bench))
}

/// Run `arch.sample` with `{"house": true}` — the deterministic 300-wall,
/// two-level benchmark house with room detection, a generated Level-2 hip roof,
/// and a named interior-wall stretch bench edit.
fn run_house(s: &mut Session, _params: &Value) -> crate::Result<Value> {
    let (model, bench) = build_benchmark_house()?;
    let d = s.doc_mut()?;
    d.ensure_layer("A-WALL");
    d.ensure_layer("A-DOOR");
    d.ensure_layer("A-GLAZ");
    d.ensure_layer("A-ROOF");
    d.arch = model;
    d.touch_arch();

    Ok(json!({
        "walls": d.arch.walls.len(),
        "openings": d.arch.openings.len(),
        "levels": d.arch.levels.len(),
        "rooms": d.arch.rooms.len(),
        "roofs": d.arch.roofs.len(),
        "revision": d.arch_revision,
        "bench": bench,
    }))
}

/// Run `arch.sample` with `{"house2": true}` — three-storey demo: basement
/// (elevation −2.7) + Level 1 (elevation 0) + Level 2 (elevation 2.7),
/// exterior walls on all three levels, central divider on the basement and
/// Level 2, `roomdetect` on all three levels → 5 rooms.
fn run_house2(s: &mut Session, _params: &Value) -> crate::Result<Value> {
    let mut m = cadcraft_doc::ArchModel::default();
    m.levels = vec![
        cadcraft_doc::Level { index: 0, name: "Basement".into(), elevation: -2.7, ..cadcraft_doc::Level::default() },
        cadcraft_doc::Level { index: 1, name: "Level 1".into(), ..cadcraft_doc::Level::default() },
        cadcraft_doc::Level { index: 2, name: "Level 2".into(), elevation: 2.7, ..cadcraft_doc::Level::default() },
    ];
    for w in house_exterior(0) {
        m.push_wall(w);
    }
    m.push_wall(central_divider(0));
    for w in house_exterior(1) {
        m.push_wall(w);
    }
    for w in house_exterior(2) {
        m.push_wall(w);
    }
    m.push_wall(central_divider(2));
    let rooms = cadcraft_arch::room::detect_rooms(&m);
    let placed = rooms.into_iter().map(|r| m.push_room(r)).count();
    let d = s.doc_mut()?;
    d.ensure_layer("A-WALL");
    d.ensure_layer("A-DOOR");
    d.ensure_layer("A-GLAZ");
    d.arch = m;
    d.touch_arch();
    Ok(json!({
        "walls": d.arch.walls.len(),
        "levels": d.arch.levels.len(),
        "rooms": placed,
        "revision": d.arch_revision
    }))
}

/// Run `arch.sample`: inject a deterministic wall chain into the drawing.
///
/// `walls` defaults to 64, clamped to `1..=MAX_WALLS`; `openings` defaults to
/// 0 and is clamped to `MAX_OPENINGS`. This replaces the existing walls
/// *and* openings, calls `touch_arch()`; it is `.noundo()` because it is a
/// benchmark fixture.
///
/// `{"house": true}` and `{"house2": true}` override the wall chain with a
/// fixed demo (see `run_house` / `run_house2`).
fn run_sample(s: &mut Session, params: &Value) -> crate::Result<Value> {
    if let Some(true) = params.get("house2").and_then(|v| v.as_bool()) {
        return run_house2(s, params);
    }
    if let Some(true) = params.get("house").and_then(|v| v.as_bool()) {
        return run_house(s, params);
    }
    let n = params.get("walls").and_then(|v| v.as_u64()).unwrap_or(64).clamp(1, crate::cmd::arch::MAX_WALLS as u64) as usize;
    let count = params.get("openings").and_then(|v| v.as_u64()).unwrap_or(0).clamp(0, MAX_OPENINGS as u64) as usize;

    let d = s.doc_mut()?;
    let mut model = ArchModel::default();
    model.levels.clear();
    model.walls = generate_walls(n);
    // The generated chain hardcodes ids 1..=n; keep the shared counter in
    // step so `push_opening` hands out ids that continue after the walls.
    model.next_id = n as u32;
    model.openings.clear();
    model.rooms.clear();
    model.roofs.clear();
    d.arch = model;
    let placed = seed_openings(&mut d.arch, count);
    d.ensure_layer("A-WALL");
    d.ensure_layer("A-DOOR");
    d.ensure_layer("A-GLAZ");
    d.touch_arch();

    Ok(json!({ "walls": n, "openings": placed, "revision": d.arch_revision }))
}

/// Command spec for `arch.sample`.
pub fn specs() -> Vec<crate::cmd::CommandSpec> {
    vec![
        crate::cmd::CommandSpec::new("arch.sample", "Arch Sample", run_sample)
            .menu(&[])
            .noundo()
            .params("{walls?: N, openings?: N, house?: bool, house2?: bool}"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    /// A session with a fresh empty drawing.
    fn fresh_session() -> Session {
        let mut s = Session::empty();
        s.new_drawing(false);
        s
    }

    #[test]
    fn arch_sample_is_deterministic_and_capped() {
        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        let p = json!({ "walls": 2000 });
        let r1 = s1.execute("arch.sample", &p).unwrap();
        let r2 = s2.execute("arch.sample", &p).unwrap();

        assert_eq!(r1["walls"].as_u64(), Some(2000));
        assert_eq!(r2["walls"].as_u64(), Some(2000));

        // Byte-identical wall data on fresh documents.
        for (a, b) in s1.doc().unwrap().arch.walls.iter().zip(s2.doc().unwrap().arch.walls.iter()) {
            assert_eq!(a, b);
        }

        // Revision was bumped.
        assert!(s1.doc().unwrap().arch_revision > 0);

        // N is capped at MAX_WALLS.
        let mut big = fresh_session();
        let r3 = big.execute("arch.sample", &json!({ "walls": 99999 })).unwrap();
        assert_eq!(r3["walls"].as_u64(), Some(crate::cmd::arch::MAX_WALLS as u64));
        assert_eq!(big.doc().unwrap().arch.walls.len(), crate::cmd::arch::MAX_WALLS);
    }

    #[test]
    fn arch_sample_clamps_a_hostile_wall_count() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "walls": 999999999999i64 })).unwrap();
        assert_eq!(r["walls"].as_u64(), Some(crate::cmd::arch::MAX_WALLS as u64));
    }

    #[test]
    fn arch_sample_walls_are_connected() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 64 })).unwrap();
        let walls = &s.doc().unwrap().arch.walls;
        for i in 0..walls.len().saturating_sub(1) {
            assert_eq!(walls[i].end, walls[i + 1].start, "wall {} end != wall {} start", i, i + 1);
        }
    }

    /// The seed is the only input: two fresh sessions with the same
    /// `walls`/`openings` params must produce byte-identical openings
    /// (ids, hosts, offsets, dimensions, styles, kinds) in the same order.
    ///
    /// `openings` is set to the wave-0 capacity (`floor(N_WALLS / 2)` = 16),
    /// so exactly the even 1-based walls get seeded and wave 1 never runs:
    /// 16 openings, one per even wall 2,4,…,32, wall 1 empty.
    #[test]
    fn arch_sample_places_deterministic_openings() {
        const N_WALLS: usize = 32;
        const N_OPENINGS: usize = 16;
        let p = json!({ "walls": N_WALLS, "openings": N_OPENINGS });

        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        let r1 = s1.execute("arch.sample", &p).unwrap();
        let r2 = s2.execute("arch.sample", &p).unwrap();
        assert_eq!(r1, r2);

        let o1 = s1.doc().unwrap().arch.openings.clone();
        let o2 = s2.doc().unwrap().arch.openings.clone();
        assert_eq!(o1, o2);

        // 16 openings (even walls 2..=32, one each), ids in push order.
        assert_eq!(o1.len(), 16, "expected 16 seeded openings");
        assert!(o1.iter().all(|o| o.id.0 < N_OPENINGS as u32), "opening ids must be within the opening counter range");
        assert_eq!(o1.iter().map(|o| o.host.0).collect::<Vec<u32>>(), (2..=N_WALLS).step_by(2).map(|i| i as u32).collect::<Vec<u32>>());

        let d = s1.doc().unwrap();
        // Every opening sits at 40 % of its host wall, in the door/window
        // styles the command seeds.
        for o in &o1 {
            let w = d.arch.wall(o.host).expect("opening host missing");
            let len = w.start.dist(w.end);
            assert!((o.offset - 0.4 * len).abs() < 1e-9, "offset of {:?} not 40 % of its wall", o.id);
            if o.style.contains("hinged") {
                assert_eq!(o.kind, OpeningKind::Door);
                assert_eq!((o.width, o.height, o.sill), (0.9, 2.1, 0.0));
            } else {
                assert_eq!(o.style, "slider");
                assert_eq!(o.kind, OpeningKind::Window);
                assert_eq!((o.width, o.height, o.sill), (1.2, 1.2, 0.9));
            }
        }
        // 1st wall (odd): no opening; 2nd: exactly the window; 6th: exactly
        // the door (1-based index 6 is a multiple of 3).
        assert!(d.arch.openings_on(ArchId(1)).is_empty());
        assert_eq!(d.arch.openings_on(ArchId(2)).len(), 1);
        assert_eq!(d.arch.openings_on(ArchId(6)).len(), 1);
    }

    /// `arch.sample {"walls":300,"openings":150}` places exactly the requested
    /// 150 openings: one per wall, every host distinct, and every placement
    /// still legal against the final model.
    #[test]
    fn arch_sample_places_the_requested_opening_count() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "walls": 300, "openings": 150 })).unwrap();
        assert_eq!(r["openings"].as_u64(), Some(150));
        let d = s.doc().unwrap();
        assert_eq!(d.arch.openings.len(), 150);

        // One per wall: every host is distinct.
        let mut seen: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for o in &d.arch.openings {
            assert!(seen.insert(o.host.0), "duplicate host wall for opening {:?}", o.id);
        }

        // Every placement is still legal on the final model.
        for o in &d.arch.openings {
            assert!(
                validate_placement(&d.arch, o.host, o.offset, o.width, o.sill, o.height, o.kind, &o.style, Some(o.id)).is_ok(),
                "opening {:?} fails re-validation on the final model",
                o.id
            );
        }
    }

    /// Opening ids are exactly `ArchId(0)..ArchId(149)` in push order for a
    /// full 300-wall / 150-opening sample — this is what makes `o10`
    /// resolvable by `arch.opening.move`.
    #[test]
    fn arch_sample_opening_ids_are_sequential() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 300, "openings": 150 })).unwrap();
        let ids: Vec<ArchId> = s.doc().unwrap().arch.openings.iter().map(|o| o.id).collect();
        assert_eq!(ids, (0..150).map(ArchId).collect::<Vec<ArchId>>());
    }

    /// `arch.opening.move {"opening":"o10","offset":1.0}` on a 300-wall sample
    /// succeeds: with one opening per wall and every wall at least 3.0 m long,
    /// offset 1.0 always validates. This is exactly the edit
    /// `tests/acceptance/phase-2/09_opening_rebuild_bench.sh` runs.
    #[test]
    fn arch_sample_opening_move_to_one_metre_is_legal() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 300, "openings": 150 })).unwrap();
        s.execute("arch.opening.move", &json!({ "opening": "o10", "offset": 1.0 })).unwrap();
    }

    /// Requesting more openings than walls places at most one per wall and
    /// neither fails nor panics.
    #[test]
    fn arch_sample_more_openings_than_walls_is_capped_by_capacity() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "walls": 8, "openings": 100 })).unwrap();
        assert_eq!(r["walls"].as_u64(), Some(8));
        let placed = r["openings"].as_u64().unwrap();
        assert!(placed <= 8, "placed {placed} openings on 8 walls");
        assert_eq!(s.doc().unwrap().arch.openings.len(), placed as usize);
    }

    /// Every seeded opening must still pass placement validation against the
    /// final model (rough/finished dimensions, wall height, segment and
    /// separation limits), i.e. the fixture is a set of legal placements.
    #[test]
    fn arch_sample_openings_are_valid_placements() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 64, "openings": 16 })).unwrap();
        let d = s.doc().unwrap();
        for o in &d.arch.openings {
            assert!(d.arch.wall(o.host).is_some(), "host wall missing for {:?}", o.id);
            assert!(
                validate_placement(&d.arch, o.host, o.offset, o.width, o.sill, o.height, o.kind, &o.style, Some(o.id),).is_ok(),
                "opening {:?} fails re-validation on the final model",
                o.id
            );
        }
    }

    /// Without the `openings` parameter (or with 0) the model must contain no
    /// openings, exactly as the command behaved before this parameter.
    #[test]
    fn arch_sample_without_openings_is_unchanged() {
        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        s1.execute("arch.sample", &json!({})).unwrap();
        s2.execute("arch.sample", &json!({ "walls": 24, "openings": 0 })).unwrap();

        assert!(s1.doc().unwrap().arch.openings.is_empty());
        assert_eq!(s1.doc().unwrap().arch.walls.len(), 64);
        assert!(s2.doc().unwrap().arch.openings.is_empty());
        assert_eq!(s2.doc().unwrap().arch.walls.len(), 24);
    }

    /// `arch.sample {"house":true}` creates the A-WALL layer on first use (exactly once).
    #[test]
    fn arch_sample_creates_wall_layer() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        assert!(d.layers.iter().any(|l| l.name == "A-WALL"), "A-WALL not created by arch.sample");
        assert_eq!(d.layers.iter().filter(|l| l.name == "A-WALL").count(), 1, "A-WALL created more than once");
    }

    /// `{"house":true}` must bump the arch revision from 0.
    #[test]
    fn sample_house_revision_bumped() {
        let mut s = fresh_session();
        assert_eq!(s.doc().unwrap().arch_revision, 0);
        s.execute("arch.sample", &json!({ "house": true })).unwrap();
        assert!(s.doc().unwrap().arch_revision > 0);
    }

    /// `{"house2":true}` must bump the arch revision from 0.
    #[test]
    fn sample_house2_revision_bumped() {
        let mut s = fresh_session();
        assert_eq!(s.doc().unwrap().arch_revision, 0);
        s.execute("arch.sample", &json!({ "house2": true })).unwrap();
        assert!(s.doc().unwrap().arch_revision > 0);
    }

    /// When `openings` exceeds `floor(walls/2)`, wave 1 must run and seed the
    /// odd 1-based walls so that every wall gets an opening.
    #[test]
    fn arch_sample_both_waves_run_when_max_exceeds_half() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "walls": 4, "openings": 4 })).unwrap();
        assert_eq!(r["openings"].as_u64(), Some(4));
        let d = s.doc().unwrap();
        assert_eq!(d.arch.openings.len(), 4);
        // One opening on each of the 4 walls (wave 0: walls 2,4; wave 1: walls 1,3).
        for i in 1..=4 {
            assert_eq!(d.arch.openings_on(ArchId(i)).len(), 1, "wall {} should have exactly one opening", i);
        }
    }

    /// The wall chain must be bit-identical whether or not openings are seeded
    /// (other tests and goldens depend on `generate_walls` being deterministic
    /// and independent of the openings pass).
    #[test]
    fn arch_sample_walls_identical_with_and_without_openings() {
        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        s1.execute("arch.sample", &json!({ "walls": 32 })).unwrap();
        s2.execute("arch.sample", &json!({ "walls": 32, "openings": 16 })).unwrap();
        let w1 = s1.doc().unwrap().arch.walls.clone();
        let w2 = s2.doc().unwrap().arch.walls.clone();
        assert_eq!(w1, w2);
    }

    /// Hostile `openings` values (null, string, float, negative) must all be
    /// treated as 0 and never fail the command.
    #[test]
    fn arch_sample_hostile_openings_params() {
        let mut s1 = fresh_session();
        s1.execute("arch.sample", &json!({ "walls": 8, "openings": null })).unwrap();
        assert!(s1.doc().unwrap().arch.openings.is_empty());

        let mut s2 = fresh_session();
        s2.execute("arch.sample", &json!({ "walls": 8, "openings": "abc" })).unwrap();
        assert!(s2.doc().unwrap().arch.openings.is_empty());

        let mut s3 = fresh_session();
        s3.execute("arch.sample", &json!({ "walls": 8, "openings": 3.5 })).unwrap();
        assert!(s3.doc().unwrap().arch.openings.is_empty());

        let mut s4 = fresh_session();
        s4.execute("arch.sample", &json!({ "walls": 8, "openings": -5 })).unwrap();
        assert!(s4.doc().unwrap().arch.openings.is_empty());
    }

    /// Requesting more openings than there are walls places at most one per
    /// wall (the capacity limit), clamped by `MAX_OPENINGS`.
    #[test]
    fn arch_sample_openings_clamped_to_wall_capacity() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "walls": 100, "openings": 999999 })).unwrap();
        let placed = r["openings"].as_u64().unwrap();
        assert_eq!(placed, 100);
        assert_eq!(s.doc().unwrap().arch.openings.len(), 100);
    }

    /// A single wall is valid: positive finite length, positive thickness,
    /// height 2.6.
    #[test]
    fn arch_sample_single_wall_is_valid() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 1 })).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.walls.len(), 1);
        let w = &d.arch.walls[0];
        let len = w.start.dist(w.end);
        assert!(len > 0.0, "wall length must be positive");
        assert!(len.is_finite(), "wall length must be finite");
        assert!(w.thickness > 0.0, "wall thickness must be positive");
        assert_eq!(w.height, 2.6, "wall height must be 2.6");
    }

    /// Zero walls is clamped to one.
    #[test]
    fn arch_sample_zero_walls_clamped_to_one() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 0 })).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.walls.len(), 1, "0 walls should be clamped to 1");
    }

    /// All generated wall lengths are in [3.0, 9.0) and finite.
    #[test]
    fn arch_sample_wall_lengths_in_range() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 64 })).unwrap();
        let d = s.doc().unwrap();
        for w in &d.arch.walls {
            let len = w.start.dist(w.end);
            assert!(len >= 3.0 && len < 9.0, "wall length {} out of range [3.0, 9.0)", len);
            assert!(len.is_finite(), "wall length must be finite");
        }
    }

    /// Wall thickness alternates 0.2 (odd walls) / 0.1 (even walls).
    #[test]
    fn arch_sample_wall_thickness_alternates() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 10 })).unwrap();
        let d = s.doc().unwrap();
        for (i, w) in d.arch.walls.iter().enumerate() {
            let expected = if (i + 1) % 2 == 1 { 0.2 } else { 0.1 };
            assert_eq!(w.thickness, expected, "wall {} thickness should be {}", i + 1, expected);
        }
    }

    /// Walls at 1-based indices 3, 6, 9 (multiples of 3) get hinged doors.
    #[test]
    fn arch_sample_door_on_multiple_of_three() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 9, "openings": 9 })).unwrap();
        let d = s.doc().unwrap();
        for wall_id in [3, 6, 9] {
            let openings = d.arch.openings_on(ArchId(wall_id));
            assert_eq!(openings.len(), 1, "wall {} should have one opening", wall_id);
            assert_eq!(openings[0].kind, OpeningKind::Door, "wall {} should have a door", wall_id);
            assert_eq!(openings[0].style, "hinged", "wall {} door should be hinged", wall_id);
        }
    }

    /// Walls at 1-based indices 1, 2, 4, 5, 7, 8 (not multiples of 3) get
    /// slider windows.
    #[test]
    fn arch_sample_window_on_non_multiple_of_three() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 9, "openings": 9 })).unwrap();
        let d = s.doc().unwrap();
        for wall_id in [1, 2, 4, 5, 7, 8] {
            let openings = d.arch.openings_on(ArchId(wall_id));
            assert_eq!(openings.len(), 1, "wall {} should have one opening", wall_id);
            assert_eq!(openings[0].kind, OpeningKind::Window, "wall {} should have a window", wall_id);
            assert_eq!(openings[0].style, "slider", "wall {} window should be slider", wall_id);
        }
    }

    /// The wall chain changes direction at least once (right-angle turns).
    #[test]
    fn arch_sample_wall_chain_changes_direction() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "walls": 20 })).unwrap();
        let d = s.doc().unwrap();
        let walls = &d.arch.walls;
        let mut changed = false;
        for i in 0..walls.len().saturating_sub(2) {
            let dx1 = walls[i + 1].start.x - walls[i].start.x;
            let dy1 = walls[i + 1].start.y - walls[i].start.y;
            let dx2 = walls[i + 2].start.x - walls[i + 1].start.x;
            let dy2 = walls[i + 2].start.y - walls[i + 1].start.y;
            let cross = dx1 * dy2 - dy1 * dx2;
            if cross.abs() > 1e-9 {
                changed = true;
                break;
            }
        }
        assert!(changed, "wall chain should change direction");
    }

    #[test]
    fn house_sample_has_300_walls_on_two_levels() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        let walls = r["walls"].as_u64().unwrap() as usize;
        assert!((300..=320).contains(&walls), "expected 300 to 320 walls, got {walls}");
        assert_eq!(d.arch.walls.len(), walls);
        assert_eq!(r["levels"].as_u64(), Some(2));
        assert_eq!(d.arch.levels.iter().map(|l| (l.index, l.elevation)).collect::<Vec<_>>(), vec![(1, 0.0), (2, 2.7)]);
        for level in 1..=2 {
            let on = d.arch.walls.iter().filter(|w| w.base_level == level).count();
            assert!(on >= 120, "expected at least 120 walls on level {level}, got {on}");
        }
        assert!(d.arch.walls.iter().any(|w| w.exterior) && d.arch.walls.iter().any(|w| !w.exterior));
    }

    /// L, T and X joins per level from the walls' centre lines, counted the way
    /// the phase-13 acceptance check counts them: ends within 1e-4 form a
    /// cluster; 2 non-collinear ends are an L, 3 a T, 4 or more an X; a lone end
    /// on another wall's side is a T; two walls crossing in their interiors an X.
    fn classify_joins(walls: &[cadcraft_doc::Wall]) -> (usize, usize, usize) {
        let key = |p: cadcraft_geom::Vec2| ((p.x / 1e-4).round() as i64, (p.y / 1e-4).round() as i64);
        let (mut l, mut t, mut x) = (0, 0, 0);
        let mut levels: Vec<u32> = walls.iter().map(|w| w.base_level).collect();
        levels.sort_unstable();
        levels.dedup();
        for level in levels {
            let ws: Vec<_> = walls.iter().filter(|w| w.base_level == level).collect();
            let mut clusters = std::collections::BTreeMap::<(i64, i64), Vec<(cadcraft_geom::Vec2, cadcraft_geom::Vec2)>>::new();
            for w in &ws {
                clusters.entry(key(w.start)).or_default().push((w.start, w.end));
                clusters.entry(key(w.end)).or_default().push((w.end, w.start));
            }
            for ends in clusters.values() {
                match ends.len() {
                    2 => {
                        let (u, v) = (ends[0].1 - ends[0].0, ends[1].1 - ends[1].0);
                        if (u.x * v.y - u.y * v.x).abs() / (u.len() * v.len()) > 0.5 {
                            l += 1;
                        }
                    }
                    3 => t += 1,
                    n if n >= 4 => x += 1,
                    _ => {}
                }
            }
            for w in &ws {
                for p in [w.start, w.end] {
                    if clusters.get(&key(p)).is_some_and(|c| c.len() > 1) {
                        continue;
                    }
                    let on_side = ws.iter().any(|o| {
                        let d = o.end - o.start;
                        let ll = d.x * d.x + d.y * d.y;
                        let k = ((p.x - o.start.x) * d.x + (p.y - o.start.y) * d.y) / ll.max(1e-300);
                        !std::ptr::eq(*o, *w) && ll > 0.0 && 0.01 < k && k < 0.99 && (o.start + d * k).dist(p) <= o.thickness / 2.0 + 1e-6
                    });
                    t += usize::from(on_side);
                }
            }
            for (i, a) in ws.iter().enumerate() {
                for b in &ws[i + 1..] {
                    let (r, q) = (a.end - a.start, b.end - b.start);
                    let den = r.x * q.y - r.y * q.x;
                    if den.abs() < 1e-12 {
                        continue;
                    }
                    let e = b.start - a.start;
                    let (ta, tb) = ((e.x * q.y - e.y * q.x) / den, (e.x * r.y - e.y * r.x) / den);
                    x += usize::from(0.01 < ta && ta < 0.99 && 0.01 < tb && tb < 0.99);
                }
            }
        }
        (l, t, x)
    }

    #[test]
    fn house_sample_has_l_t_and_x_joins() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        let (l, t, x) = classify_joins(&d.arch.walls);
        assert!(l >= 8 && t >= 8 && x >= 8, "expected at least 8 L, 8 T and 8 X joins, saw L={l} T={t} X={x}");
        // The ring's corners are L joins on each level, so every corner has exactly two wall ends.
        for level in 1..=2 {
            let ends = |p: cadcraft_geom::Vec2| {
                d.arch.walls.iter().filter(|w| w.base_level == level).filter(|w| w.start == p || w.end == p).count()
            };
            for c in [(0.0, 0.0), (18.0, 0.0), (18.0, 12.0), (0.0, 12.0)] {
                assert_eq!(ends(cadcraft_geom::Vec2::new(c.0, c.1)), 2, "corner {c:?} on level {level}");
            }
        }
    }

    #[test]
    fn house_sample_openings_are_valid() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let openings = r["openings"].as_u64().unwrap();
        let d = s.doc().unwrap();
        let host = |o: &Opening| d.arch.wall(o.host).expect("opening host missing");
        let windows = d.arch.openings.iter().filter(|o| o.kind == OpeningKind::Window && host(o).exterior).count();
        let doors = d.arch.openings.iter().filter(|o| o.kind == OpeningKind::Door && !host(o).exterior).count();
        assert!((150..=170).contains(&openings), "expected 150 to 170 openings, got {openings}");
        assert_eq!(d.arch.openings.len(), openings as usize);
        assert!(windows >= 40, "expected at least 40 windows in exterior walls, got {windows}");
        assert!(doors >= 40, "expected at least 40 doors in partitions, got {doors}");
        for opening in &d.arch.openings {
            assert!(
                validate_placement(
                    &d.arch,
                    opening.host,
                    opening.offset,
                    opening.width,
                    opening.sill,
                    opening.height,
                    opening.kind,
                    &opening.style,
                    Some(opening.id)
                )
                .is_ok(),
                "invalid opening {:?}",
                opening.id
            );
            assert!((1..=2).contains(&host(opening).base_level), "house openings should be on level 1 or 2");
        }
    }

    #[test]
    fn house_sample_has_rooms_on_both_levels_and_a_hip_roof() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        for level in 1..=2 {
            let rooms = d.arch.rooms.iter().filter(|room| room.level == level).count();
            assert!(rooms >= 15, "expected at least 15 detected rooms on level {level}, got {rooms}");
        }
        assert_eq!(r["rooms"].as_u64(), Some(d.arch.rooms.len() as u64));
        assert_eq!(d.arch.roofs.len(), 1, "expected one automatic roof");
        let roof = &d.arch.roofs[0];
        assert_eq!(roof.level, 2, "roof should be on Level 2");
        assert_eq!(r["roofs"].as_u64(), Some(1));
        assert_eq!((roof.pitch_rise, roof.pitch_run, roof.overhang), (6.0, 12.0, 0.5));
        assert!(!roof.manual && !roof.hand_edited);
        assert!(roof.planes.len() >= 4, "a hip roof has at least 4 planes, got {}", roof.planes.len());
        assert!(roof.planes.iter().all(|p| p.slope.is_finite() && p.slope > 0.1), "no vertical (gable) plane: {:?}",
            roof.planes.iter().map(|p| p.slope).collect::<Vec<_>>());
    }

    #[test]
    fn house_sample_bench_stretch_round_trips() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let bench = r["bench"].as_object().unwrap();
        let wall = bench["wall"].as_str().unwrap();
        let end = bench["end"].as_str().unwrap();
        let from = [bench["from"][0].as_f64().unwrap(), bench["from"][1].as_f64().unwrap()];
        let to = [bench["to"][0].as_f64().unwrap(), bench["to"][1].as_f64().unwrap()];

        s.execute("arch.wall.stretch", &json!({ "wall": wall, "end": end, "to": to })).unwrap();
        s.execute("arch.wall.stretch", &json!({ "wall": wall, "end": end, "to": from })).unwrap();
    }

    #[test]
    fn house_sample_is_deterministic() {
        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        let r1 = s1.execute("arch.sample", &json!({ "house": true })).unwrap();
        let r2 = s2.execute("arch.sample", &json!({ "house": true })).unwrap();
        assert_eq!(r1, r2);
        assert_eq!(s1.doc().unwrap().arch.walls, s2.doc().unwrap().arch.walls);
        assert_eq!(s1.doc().unwrap().arch.openings, s2.doc().unwrap().arch.openings);
        assert_eq!(s1.doc().unwrap().arch.rooms, s2.doc().unwrap().arch.rooms);
    }

    #[test]
    fn sample_house2_has_two_levels_and_five_rooms() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house2": true })).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.levels.len(), 3, "house2 should have 3 levels");
        assert!(d.arch.rooms.len() >= 5, "expected at least 5 rooms, got {}", d.arch.rooms.len());
        assert_eq!(r["levels"].as_u64(), Some(3));
    }

    #[test]
    fn sample_house2_hostile_input() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house2": "bogus" })).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.arch.levels.len(), 0);
        assert!(d.arch.walls.len() > 0);
        assert_eq!(d.arch.walls.len(), 64);
    }

    #[test]
    fn sample_house2_no_openings() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house2": true })).unwrap();
        assert!(s.doc().unwrap().arch.openings.is_empty());
    }

    #[test]
    fn sample_house2_room_areas() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house2": true })).unwrap();
        let d = s.doc().unwrap();
        assert!(!d.arch.rooms.is_empty());
        assert!(d.arch.rooms.iter().all(|room| room.interior_area > 0.0));
    }

    #[test]
    fn sample_house2_wall_positions() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house2": true })).unwrap();
        let d = s.doc().unwrap();
        assert!(d.arch.walls.iter().all(|w| w.start.is_finite() && w.end.is_finite()));
        assert!(d.arch.walls.iter().any(|w| w.start.x == 0.0 || w.end.x == 0.0));
    }

    #[test]
    fn sample_house_hostile_input() {
        let mut s = fresh_session();
        let r = s.execute("arch.sample", &json!({ "house": "bogus", "walls": -5, "openings": "oops" })).unwrap();
        assert_eq!(r["walls"].as_u64(), Some(64));
        assert_eq!(r["openings"].as_u64(), Some(0));
    }

    #[test]
    fn sample_house_is_unchanged() {
        let mut s1 = fresh_session();
        let mut s2 = fresh_session();
        s1.execute("arch.sample", &json!({})).unwrap();
        s2.execute("arch.sample", &json!({ "walls": 64, "openings": 0 })).unwrap();
        assert_eq!(s1.doc().unwrap().arch.walls.len(), 64);
        assert_eq!(s2.doc().unwrap().arch.walls.len(), 64);
        assert!(s1.doc().unwrap().arch.openings.is_empty());
        assert!(s2.doc().unwrap().arch.openings.is_empty());
    }

    #[test]
    fn sample_house_no_openings() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house": false, "walls": 32, "openings": 0 })).unwrap();
        assert!(s.doc().unwrap().arch.openings.is_empty());
    }

    #[test]
    fn sample_house_room_area() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        assert!(!d.arch.rooms.is_empty());
        assert!(d.arch.rooms.iter().any(|room| room.interior_area > 0.0));
    }

    #[test]
    fn sample_house_wall_positions() {
        let mut s = fresh_session();
        s.execute("arch.sample", &json!({ "house": true })).unwrap();
        let d = s.doc().unwrap();
        assert!(d.arch.walls.iter().all(|w| w.start.is_finite() && w.end.is_finite()));
        assert!(d.arch.walls.iter().any(|w| w.start.x >= 0.0 && w.start.y >= 0.0));
    }
}
