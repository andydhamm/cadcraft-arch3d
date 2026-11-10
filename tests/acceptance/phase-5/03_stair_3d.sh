#!/usr/bin/env bash
# The stair is a probeable solid built from parts (phase 17: treads, risers and stringers, centred on `start`).
# Rewritten in phase 17: the phase-5 flight was one solid ramp extended to one side of `start`; now (2.1,1.0,0.16) is
# inside tread 1 (x 1.975..2.254, z 0.14375..0.16875), (2.01,1.0,0.07) inside riser 1, (2.6,0.51,0.45) inside the right
# stringer (y 0.5..0.538), and the air under the flight (2.6,1.0,0.3), above it (2.1,1.0,1.0), beyond its top
# (5.7,1.0,2.6) and beyond y 1.5 (2.1,1.9,0.16: the old one-sided flight reached y 2.0) is empty. 16 risers of 0.16875.
set -uo pipefail
b=$(cargo run -q -p cadcraft-cli -- run --metric \
  --cmd 'wall {"points":[[0,0],[10,0],[10,8],[0,8],[0,0]],"thickness":0.2,"height":2.7,"exterior":true}' \
  --cmd 'level {"above":0}' \
  --cmd 'stair {"start":[2,1],"direction":0,"width":1.0,"fromLevel":0,"toLevel":1}' \
  --cmd 'build3d {"probe":[[2.1,1.0,0.16],[2.01,1.0,0.07],[2.6,0.51,0.45],[2.1,1.0,1.0],[5.7,1.0,2.6],[2.6,1.0,0.3],[2.1,1.9,0.16]]}' 2>/dev/null | tail -n 1) || { echo "FAIL: stair build crashed"; exit 1; }
python3 - "$b" <<'PY' || exit 1
import json,sys
d=json.loads(sys.argv[1]); P=d["probe"]
assert d.get("stairs",0)==1, f"build3d must count the stair: {d.get('stairs')}"
assert P[0] and P[0]["part"]=="tread", f"inside tread 1 must be tread: {P[0]}"
assert P[1] and P[1]["part"]=="riser", f"inside riser 1 must be riser: {P[1]}"
assert P[2] and P[2]["part"]=="stringer", f"inside the right stringer must be stringer: {P[2]}"
assert P[3] is None, f"the air above step 1 is empty: {P[3]}"
assert P[4] is None, f"nothing beyond the flight top: {P[4]}"
assert P[5] is None, f"under the flight is air (the old solid ramp is gone): {P[5]}"
assert P[6] is None, f"the flight is centred on start (nothing beyond y 1.5): {P[6]}"
parts=d.get("parts",{})
assert parts.get("tread")==15 and parts.get("riser")==16, f"15 treads and 16 risers: {parts}"
print("ok: stair is probeable, part by part")
PY
echo "PASS 03"
