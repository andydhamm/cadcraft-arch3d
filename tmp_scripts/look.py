import json, sys
d = json.load(open(sys.argv[1]))
print(list(d.keys()))
for r in d['rooms']:
    print(json.dumps(r))
print(json.dumps(d['walls'][0]))
print(json.dumps(d.get('lights'))[:300])
s = d['surfaces']
print(type(s), json.dumps(s)[:1500])
