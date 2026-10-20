from pathlib import Path
import subprocess,json,hashlib
root=Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=Path('/tmp/forge-stack-storage');base=root/'crates/emit_native/src/pruning/allocation_inventory/closure';text=base.with_suffix('.rs').read_text();getters=(base/'table_getters.rs').read_text()
def part(source,start,end):return source[source.index(start):source.index(end,source.index(start))]
quoted=part(text,'pub(super) fn quoted_end','/// Split an outer');arguments=part(text,'fn arguments(','fn uncomment(');uncomment=part(text,'fn uncomment(','/// Locate body');signature=part(text,'fn signature(','/// Check observational');primitive=part(text,'fn primitive(','/// Skip a quoted');canonical=part(getters,'pub(in crate::pruning) fn canonical_symbol','fn table(').replace('pub(in crate::pruning)','pub(super)')
source='''#![allow(dead_code)]
mod proof {
 struct Abi { result: String, parameters: Vec<String> }
'''+quoted+arguments+uncomment+signature+primitive+'mod table_getters {\n'+canonical+'}\n'+f'#[path="{base / "read_only.rs"}"] mod read_only;\n#[path="{base / "stack_storage.rs"}"] mod stack_storage;\n'+''' pub fn inspect(text: &str) -> (usize,usize,usize,usize) {
 let mut lines=text.lines();let(mut functions,mut with_alloca,mut stores,mut geps)=(0,0,0,0);
 while let Some(header)=lines.next() {
  if !header.starts_with("define ") || !header.ends_with('{') {continue;}
  let mut body=Vec::new();for line in lines.by_ref(){if line.trim()=="}" {break;}body.push(line);}
  let Some((_,_,parameters))=signature(header) else {continue;};functions+=1;
  if body.iter().any(|line|line.contains("alloca")){with_alloca+=1;}
  for index in stack_storage::observe(&body,&parameters){
   let line=uncomment(body[index]).unwrap().trim();
   if line.starts_with("store "){stores+=1;}else{geps+=1;}
  }
 }
 (functions,with_alloca,stores,geps)
 }
}
fn main(){for path in std::env::args().skip(1){let text=std::fs::read_to_string(&path).unwrap();let(a,b,c,d)=proof::inspect(&text);println!("{a} {b} {c} {d} {path}");}}
'''
(out/'inspect-instructions.rs').write_text(source);cmd=['rustc','--edition','2024','-O',str(out/'inspect-instructions.rs'),'-o',str(out/'inspect-instructions')];p=subprocess.run(cmd,capture_output=True,text=True);(out/'instruction-build.json').write_text(json.dumps(dict(command=cmd,exit=p.returncode,stdout=p.stdout,stderr=p.stderr),indent=2)+'\n');assert p.returncode==0,p.stderr
record=json.loads((out/'runtime/sdk-comparison.json').read_text());sdk=root/record['source'];files=sorted(sdk.parent.glob('*.ll'));cmd=[str(out/'inspect-instructions')]+[str(p) for p in files];p=subprocess.run(cmd,capture_output=True,text=True);assert p.returncode==0,p.stderr;rows=[]
for line in p.stdout.splitlines():
 a,b,c,d,path=line.split(' ',4);f=Path(path);rows.append(dict(path=str(f.relative_to(root)),sha256=hashlib.sha256(f.read_bytes()).hexdigest(),functions=int(a),functions_with_alloca=int(b),local_stores=int(c),local_geps=int(d)))
result=dict(command=cmd,exit=p.returncode,stderr=p.stderr,modules=rows,total={key:sum(row[key] for row in rows) for key in ['functions','functions_with_alloca','local_stores','local_geps']});(out/'instruction-observations.json').write_text(json.dumps(result,indent=2)+'\n');print(result['total'])
