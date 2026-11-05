#!/usr/bin/env python3
# Grade classes: every level is classed from the finished grade 0.3 m outside its exterior walls: above grade when its
# floor is at or above the grade all round, else basement. arch.inspect reports each level's class and the above-grade
# floor area (the rooms of floor levels classed above grade; basements and landings never count).
# needs: p15-doc-levels, p15-cmd-levels, p15-arch-spanning-walls, p15-arch-grade, p15-inspect
import os
exec(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), 'p15lib.inc')).read())

def house(lower=-1.2, main=1.5, entry=0.15):
    return [cmd('level.edit', {"level": 0, "name": "Lower", "elevation": lower}),
            cmd('level', {"name": "Main", "elevation": main}),
            cmd('level', {"name": "Entry", "elevation": entry, "kind": "landing"}),
            cmd('wall', {"points": [[0, 0], [10, 0]], "thickness": 0.2, "exterior": True, "level": "Lower", "topLevel": "Main"}),
            cmd('wall', {"points": [[10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Lower"}),
            cmd('wall', {"points": [[10, 0], [10, 8], [0, 8], [0, 0]], "thickness": 0.2, "height": 2.4, "exterior": True, "level": "Main"}),
            cmd('wall', {"points": [[3, 0], [3, 3], [6, 3], [6, 0]], "thickness": 0.1, "height": 2.4, "level": "Entry"}),
            cmd('roomdetect', {"level": "Lower"}), cmd('roomdetect', {"level": "Main"}), cmd('roomdetect', {"level": "Entry"})]

def slope(back):
    # z = -0.5 at y = -20 rising linearly to `back` at y = 28 (no pad: the finished grade is this surface)
    return cmd('terrain', {"boundary": [[-20, -20], [30, -20], [30, 28], [-20, 28]],
                           "points": [[-20, -20, -0.5], [30, -20, -0.5], [30, 28, back], [-20, 28, back]], "pad": False})

def classes(cmds):
    rc, rr, so, se = run(cmds + [INSPECT15])
    ins = inspected(rr)
    lv = {l['name']: l for l in rows_of(ins, 'levels')}
    return ins, lv

for label, extra, want in (("no terrain (grade 0)", [], {"Lower": "basement", "Main": "aboveGrade", "Entry": "aboveGrade"}),
                           ("a gentle slope", [slope(1.0)], {"Lower": "basement", "Main": "aboveGrade", "Entry": "aboveGrade"}),
                           ("a steep slope (1.56 at the back wall, 0.94 at the front)", [slope(3.0)], {"Lower": "basement", "Main": "basement", "Entry": "basement"})):
    ins, lv = classes(house() + extra)
    got = {n: lv[n].get('gradeClass') for n in want}
    if got != want:
        fail(f"{label}: classes {got}, expected {want}")
    main_rooms = [r for r in rows_of(ins, 'rooms') if r.get('level') == lv['Main'].get('index')]
    expect = sum(num(r['interiorArea'], 'interiorArea') for r in main_rooms) if want['Main'] == 'aboveGrade' else 0.0
    near(num(ins.get('aboveGradeArea'), 'aboveGradeArea'), expect, 1e-6, f"{label}: the above-grade area (Main's rooms only)")
    near(num(lv['Entry'].get('aboveGradeArea', 0.0), 'Entry aboveGradeArea'), 0.0, 1e-9, f"{label}: a landing adds no area")
    near(num(lv['Lower'].get('aboveGradeArea', 0.0), 'Lower aboveGradeArea'), 0.0, 1e-9, f"{label}: a basement adds no area")
    print(f"ok: {label}: {got}, above-grade area {ins['aboveGradeArea']:.2f} m²")
ins, lv = classes(house(lower=0.0, main=2.7, entry=1.35))
if lv['Lower']['gradeClass'] != 'aboveGrade':
    fail(f"a floor exactly at grade (0) is above grade: {lv['Lower']}")
print("ok: a floor exactly at grade is above grade")
print("PASS 07")
