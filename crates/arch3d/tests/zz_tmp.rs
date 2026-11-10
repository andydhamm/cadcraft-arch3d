use cadcraft_arch3d::Scene3d;
use cadcraft_doc::arch::{Level, Stair};
use cadcraft_doc::{ArchModel, Wall};
use cadcraft_geom::Vec2;

#[test]
fn zz() {
    let mut m = ArchModel::default();
    m.levels.push(Level { index: 0, elevation: 0.0, ceiling_height: 2.4, ..Default::default() });
    m.levels.push(Level { index: 1, elevation: 2.7, ceiling_height: 2.4, ..Default::default() });
    m.push_wall(Wall { start: Vec2::new(0.0, 1.0), end: Vec2::new(6.0, 1.0), thickness: 0.1, height: 2.7, ..Default::default() });
    m.push_stair(Stair {
        start: Vec2::new(1.0, 1.0),
        width: 0.9,
        from_level: 0,
        to_level: 1,
        rise: 2.7,
        risers: 16,
        riser_height: 0.16875,
        treads: 15,
        tread_depth: 0.254,
        run: 3.81,
        ..Default::default()
    });
    let s = Scene3d::build(&m);
    for mesh in s.meshes.iter().filter(|x| x.material == "wall") {
        for t in mesh.indices.chunks_exact(3) {
            let p: Vec<[f32; 3]> = t.iter().map(|&i| mesh.positions[i as usize]).collect();
            if p.iter().any(|q| q[0] > 4.4 && q[0] < 4.6) || (p.iter().any(|q| q[0] < 4.5) && p.iter().any(|q| q[0] > 4.5)) {
                eprintln!("TRI {:?}", p);
            }
        }
    }
}
