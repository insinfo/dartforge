"""Inspect exact function symbols; static counts do not describe execution."""
from pathlib import Path
import re,json
archive=Path(__file__).resolve().parent;root=archive.parents[3];stats={}
for case,label in [('color','convertFormatValue'),('multiply','multiply')]:
 stats[case]={}
 for profile in ['off','on']:
  p=root/'target/primitive-sdk-native'/case/profile/'.df_tmp/program.ll';s=p.read_text()
  matches=[m for m in re.finditer(r'^define [^\n]+\{\n.*?^\}',s,re.M|re.S) if '..'+label+'(' in m.group(0).split('\n',1)[0]]
  assert len(matches)==1,(case,profile,len(matches));body=matches[0].group(0)
  (archive/case/(profile+'.function.ll')).write_text(body+'\n')
  stats[case][profile]={'signature':body.splitlines()[0],'dyn_op_calls':len(re.findall(r'call i64 @dartforge_dyn_op\(',body)),'selector_sites':len(re.findall(r'@df.selector\(',body)),'box_int_sites':len(re.findall(r'@df.box_int\(',body)),'scope':'Static LLVM sites; generic fallback remains. Not executed call counts.'}
(archive/'inspection.json').write_text(json.dumps(stats,indent=2)+'\n');print(json.dumps(stats,indent=2))
