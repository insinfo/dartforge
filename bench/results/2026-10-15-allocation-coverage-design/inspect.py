#!/usr/bin/env python3
"""Read current published summaries; report blockers, never certify allocation closure."""
import collections, hashlib, json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[3]
CACHE=ROOT/'target/jit-readiness-build/native_cache/sdk/ec56edf3cb71e2dfaf5afadc4d1fd950'
effects={}
for line in (ROOT/'crates/runtime/effects.tsv').read_text().splitlines():
    if line and not line.startswith('#'):
        name,*flags=line.split('\t');effects[name]=tuple(map(int,flags))
counts=collections.Counter();references=collections.Counter();by_module=[];hashes={};nodes=[]
for path in sorted(CACHE.glob('*.poda')):
    names=[];definitions=[];states={};selectors=set()
    for line in path.read_text().splitlines():
        fields=line.split('\t');tag=fields[0]
        if tag=='N':names.append(fields[1])
        elif tag=='D':definitions.append((int(fields[1]),list(map(int,fields[2].split())) if fields[2] else [],fields[3] if len(fields)>3 else ''))
        elif tag=='I':states[int(fields[1])]=fields[2]
        elif tag in ('C','E'):selectors.add(int(fields[1]))
    local=collections.Counter()
    for node,refs,dynamic in definitions:
        state=states.get(node,'missing');counts[state]+=1;local[state]+=1
        name=names[node]
        if state=='unknown':
            references.update(set(names[r] for r in refs))
            if not refs:counts['unknown_without_symbol_references']+=1
            if dynamic or node in selectors:counts['unknown_with_selector_edges']+=1
            # This spelling category is descriptive, not an object/allocation proof.
            if '#.str.' in name:counts['unknown_native_string_name_candidates']+=1
        nodes.append((name,state,[names[r] for r in refs],bool(dynamic or node in selectors)))
    by_module.append({'module':path.stem,'definition_nodes':len(definitions),'states':dict(local)})
    hashes[str(path.relative_to(ROOT))]=hashlib.sha256(path.read_bytes()).hexdigest()
rows=[]
for name,count in references.most_common(45):
    flags=effects.get(name)
    rows.append({'reference':name,'unknown_definition_nodes_referencing_it':count,
                 'runtime_effects_collects_throws_runs_dart':flags,
                 'scope':'summary references include addresses and data; not necessarily direct calls'})
exports=json.loads((CACHE/'allocation-returns.json').read_text())
result={'cache':str(CACHE.relative_to(ROOT)), 'schema':8,'scope':'Exact published summary nodes and reference incidence; not executable cost or complete function opcode auditing.',
        'counts':dict(counts),'modules':by_module,'most_frequent_unknown_references':rows,
        'return_certificate_exports':len(exports['exports']),
        'runtime_effect_catalog_rows':len(effects),
        'runtime_catalog_no_collection_no_dart_rows':sum(not c and not d for c,t,d in effects.values()),
        'actual_final_llvm_available_in_this_cache':bool(list(CACHE.glob('*.ll'))),
        'summary_hashes_sha256':hashes}
out=Path(__file__).with_name('coverage.json');out.write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:result[k] for k in ('counts','return_certificate_exports','actual_final_llvm_available_in_this_cache')},indent=2))
print(json.dumps(rows[:8],indent=2))
