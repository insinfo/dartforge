import json,sys
grp,pat=sys.argv[1],sys.argv[2]
lo=int(sys.argv[3]) if len(sys.argv)>3 else 0
hi=int(sys.argv[4]) if len(sys.argv)>4 else 10**9
for l in open(f'E:/MyRustProjects/dartforge/corpus/diagnosticos/{grp}/oraculo.jsonl',encoding='utf8'):
    if pat in l:
        d=json.loads(l)
        if lo<=d['line']<=hi:
            print(f"  {d['code']} off={d['offset']} len={d['length']} {d['line']}:{d['column']} {d['problemMessage']}")
