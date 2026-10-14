from pathlib import Path
import re,json,collections,hashlib,shutil
root=Path.cwd();out=root/'target/enum-pattern-identity/format-final';p=out/'validation.json';shutil.copyfile(p,out/'validation-before-selector-inspection.json');d=json.load(open(p))
for row in d['cases']:
 label='on' if row['enabled'] else 'off';s=Path(row['IR_path']).read_text();body=(out/label/'convertFormatValue.ll').read_text();names={}
 for m in re.finditer(r'^@(df\.seln\.\d+) = .*?c"([^"]*)"',s,re.M):names[m[1]]=re.sub(r'\\([\dA-Fa-f]{2})',lambda m:chr(int(m[1],16)),m[2])
 calls=re.findall(r'\bcall[^\n]*@df\.selector\([^\n]*ptr @(df\.seln\.\d+)',body);assert calls;counts=collections.Counter(names[n] for n in calls);row['function_selector_calls']=len(calls);row['selector_calls_by_name']=dict(counts);row['IR_stats_scope']='Static @df.selector callsites in exact generated function; names decoded from matched df.seln constants in same IR. Not execution counts or cost.';print(label,len(calls),counts)
rows={r['enabled']:r for r in d['cases']};d['selector_calls_removed']=rows[False]['function_selector_calls']-rows[True]['function_selector_calls'];d['equality_selector_calls_removed']=rows[False]['selector_calls_by_name'].get('c:==',0)-rows[True]['selector_calls_by_name'].get('c:==',0);d['selector_inspection_correction']='First IR inspection searched runtime dartforge_selector, but generated callsites use inline df.selector. Original preliminary ledger preserved; final counts use actual emitted helper and selector-name globals, no recompilation.';p.write_text(json.dumps(d,indent=2)+'\n')
