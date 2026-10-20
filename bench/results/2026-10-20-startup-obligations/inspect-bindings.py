from pathlib import Path
import json,re,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');source=root/'target/rpo-native-cache/sdk/9734c45007553b208736fb763b877f68/allocation-inventory.json'
d=json.loads(source.read_text());inventory=d['inventory'];functions=inventory['functions'];globals={(g['module'],g['symbol']):g for g in inventory['globals']};keys={}
for function in functions:keys.setdefault(function['key'],[]).append(function)
rows=[]
for function in functions:
 if not function['symbol'].startswith('df.register.'):continue
 for call in function['calls']:
  if call['symbol']!='dartforge_register_helper':continue
  args=call['arguments'];initializer=globals[(function['module'],args[0].removeprefix('ptr @'))]['initializer'];match=re.search(r'] c"(.*)"$',initializer);assert match
  name=match.group(1);assert '\\' not in name and len(name.encode())==int(args[1].removeprefix('i64 '))
  symbol=args[2].removeprefix('ptr @');targets=keys.get(function['module']+'#'+symbol,keys.get(symbol,[]));assert len(targets)==1;target=targets[0]
  rows.append(dict(module=function['module'],startup=function['key'],name=name,target=target['key'],abi=target['abi'],transitive_classes=target['transitive_classes'],unknown_obligations=target['unknown_obligations']))
assert len(rows)==41 and len({r['name'] for r in rows})==41 and all(r['transitive_classes'] is None for r in rows)
record=dict(scope='Observations only; helper runtime invocation ABI, values and allocation closure are not certified',source=str(source.relative_to(root)),source_sha256=hashlib.sha256(source.read_bytes()).hexdigest(),cache_key=d['cache_key'],compiler=d['compiler'],helpers=rows)
Path('/tmp/forge-startup-diagnostics-runtime/helper-observations.json').write_text(json.dumps(record,indent=2)+'\n')
print('41 distinct helper targets; all allocation closures unknown')
