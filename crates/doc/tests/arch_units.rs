use cadcraft_doc::*;
use cadcraft_geom::Vec2;

#[test]
fn touching_a_non_empty_arch_model_makes_the_drawing_metres() {
    let mut d = Drawing::new_metric();
    assert_eq!(d.header.i64("INSUNITS", 0), 4);
    d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), 0.2));
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6);
    assert_eq!(d.header.i64("MEASUREMENT", 0), 1);
}

#[test]
fn an_empty_arch_model_leaves_insunits_alone() {
    let mut d = Drawing::new_metric();
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 4);
}

#[test]
fn touch_arch_is_idempotent() {
    let mut d = Drawing::new_metric();
    d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), 0.2));
    d.touch_arch();
    d.touch_arch();
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6);
    assert_eq!(d.header.i64("MEASUREMENT", 0), 1);
}

#[test]
fn touching_with_an_opening_makes_the_drawing_metres() {
    let mut d = Drawing::new_metric();
    assert_eq!(d.header.i64("INSUNITS", 0), 4);
    d.arch.openings.push(Opening { id: ArchId(0), ..Default::default() });
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6);
    assert_eq!(d.header.i64("MEASUREMENT", 0), 1);
}

#[test]
fn touching_after_emptying_leaves_insunits_at_six() {
    let mut d = Drawing::new_metric();
    d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), 0.2));
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6);
    d.arch.walls.clear();
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6, "emptying the model must not revert units");
}

#[test]
fn touching_an_imperial_drawing_with_arch_sets_metres() {
    let mut d = Drawing::new_imperial();
    assert_eq!(d.header.i64("INSUNITS", 0), 1);
    d.arch.push_wall(Wall::new(Vec2::new(0.0, 0.0), Vec2::new(5.0, 0.0), 0.2));
    d.touch_arch();
    assert_eq!(d.header.i64("INSUNITS", 0), 6);
    assert_eq!(d.header.i64("MEASUREMENT", 0), 1);
}
