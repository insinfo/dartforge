"""Inspect actual CLI callsite facts separately from final-size results."""
from pathlib import Path
import re,json,hashlib,difflib
ROOT=Path.cwd();OUT=ROOT/'target/selector-getter-validation';ARCHIVE=ROOT/'bench/results/2026-10-14-selector-getter-native'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
inputs={mode:(OUT/mode/'.df_tmp/program.ll') for mode in ['off','on']}
text={mode:path.read_text() for mode,path in inputs.items()}
def function(source,name):
    match=re.search(r'^define [^\n]*@'+re.escape(name)+r'\([^\n]*\).*?^}',source,re.M|re.S)
    assert match,name
    return match.group()+'\n'
classes={}
for cid,table in re.findall(r'@dartforge_object_new_t\(i64 (\d+), i64 \d+, ptr @([^\s)]+)\)',text['on']):
    if '.EncoderA' in table or '.EncoderB' in table or '.Owner' in table or '.OtherOwner' in table:classes[int(cid)]=table
name='df.getter_return_provenance$2edart..convertUnknown';bodies={mode:{'main':function(source,'dart_main'),'unknown':function(source,name)} for mode,source in text.items()}
for mode,body in bodies.items():
    for kind,source in body.items():(ARCHIVE/(mode+'.'+kind+'.ll')).write_text(source)
diff=''.join(difflib.unified_diff(text['off'].splitlines(True),text['on'].splitlines(True),fromfile='off/program.ll',tofile='on/program.ll'))
(ARCHIVE/'program-ir.diff').write_text(diff)
markers=lambda body:re.findall(r'; df.selector-exact ([\d,]+)\n\s*(%sf\d+ = call ptr @df\.selector[^\n]*)',body)
off=markers(bodies['off']['main']);on=markers(bodies['on']['main']);new=[(facts,call.strip()) for facts,call in on if (facts,call) not in off]
assert len(new)==3,new
assert set(new[0][0].split(','))=={'1','1025'}
assert all(set(facts.split(','))=={'1','1025','1026'} for facts,_ in new[1:])
assert '.EncoderA' in classes[1025] and '.EncoderB' in classes[1026]
assert bodies['off']['unknown']==bodies['on']['unknown']
assert '; df.selector-exact' not in bodies['on']['unknown']
assert len(re.findall(r'call ptr @df.selector',bodies['on']['unknown']))==2
assert all(re.search(r'%v'+str(value)+r' = phi i64',bodies['on']['main']) for value in [233,238])
why={mode:(OUT/mode/'retention.tsv') for mode in ['off','on']}
assert why['off'].read_bytes()==why['on'].read_bytes()
selected=[line for line in why['on'].read_text().splitlines() if any(name in line for name in ['.EncoderA.convert','.EncoderB.convert','.Owner.encoder','.OtherOwner.encoder','..convertUnknown'])]
(ARCHIVE/'selected-retention.tsv').write_text('\n'.join(selected)+'\n')
report={'producer':json.loads((OUT/'validation.json').read_text())['compiler_sha256'],'IR_inputs':{mode:{'path':str(path),'sha256':sha(path),'bytes':path.stat().st_size} for mode,path in inputs.items()},'physical_classes':classes,'new_downstream_convert_facts':new,'surviving_factory_phis':['%v233','%v238'],'unknown_formal':{'function':name,'body_equal':True,'dynamic_selector_calls':2,'physical_comments':0},'WHY_equal':True,'WHY_sha256':sha(why['on']),'binary_equal':sha(OUT/'off/program')==sha(OUT/'on/program'),'scope':'Actual CLI adds sound local convert receiver sets. Comments feed pruning; this mixed positive/negative fixture retains unrestricted convert calls, producing no final-size reduction. No cached SDK getter propagation or runtime-speed claim.'}
(ARCHIVE/'inspection.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps(report,indent=2))
