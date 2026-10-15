from pathlib import Path
import hashlib,json,re
root=Path(__file__).resolve().parents[3]
out=Path(__file__).resolve().parent
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
source_refs={
 'crates/emit_native/src/pruning.rs':[(27,38),(1496,1505),(1533,1565),(1644,1660)],
 'crates/emit_native/src/llvm/selectors.rs':[(207,226),(242,260)],
 'crates/emit_native/src/pruning/receiver_provenance/registry.rs':[(1,75)],
 'crates/emit_native/src/pruning/receiver_provenance.rs':[(18,30),(130,171)],
}
for file,ranges in source_refs.items():
 p=root/file;lines=p.read_text().splitlines();content='\n\n'.join('\n'.join(f'{i+1}: {lines[i]}' for i in range(start-1,min(end,len(lines)))) for start,end in ranges)
 (out/(p.stem+'-'+sha(p)[:12]+'-excerpts.txt')).write_text(content+'\n')
historical=root/'bench/results/2026-10-14-current-native-status/prepared.json'
h=json.loads(historical.read_text());hello={}
for value in h.values():
 if isinstance(value,dict):
  for name,artifact in value.items():
   if name.endswith('/hello'):hello[name]=artifact
modes={}
for mode in ['off','on']:
 map_path=root/f'target/constant-specialization-file-explorer-{mode}/program.map';binary=map_path.parent/'program'
 d=json.loads((out/f'current-{mode}-text.json').read_text())
 modes[mode]={'binary_sha256':sha(binary),'binary_bytes':binary.stat().st_size,'map_sha256':sha(map_path),'text_bytes':d['text_bytes'],'categories':d['category_bytes'],'largest_libraries':d['largest_libraries']}
d={'current_file_explorer':modes,'historical_hello':{'manifest':str(historical),'manifest_sha256':sha(historical),'compiler_sha256':h['compiler_sha256'],'artifacts':hello,'limitation':'Existing hello artifacts use the earlier 50c1a7a7 compiler; no hello compilation was performed with current c363 producer.'},'attribution_correction':'The older runtime-and-native-support bucket included canonical Dart getters and LLVM machine outlines. Current corrected OFF counts separate 784632B of canonical getters and 1442144B of machine outlines, leaving a residual support bucket of1199488B. This residual is not guaranteed pure Rust; original source ownership of outlines requires further debug/link provenance.','source_hashes':{file:sha(root/file) for file in source_refs},'retention_chain':[
 'hir_summary creates dartforge_entry refs to every method table, every table function, every class toString symbol and vtable entry. This is unconditional class activation, not an allocation RTA.',
 'The emitter also registers every program method table at initialization, so simply deleting pruning roots without changing emitted registrations would be inconsistent.',
 'Graph roots Object/call protocol selectors for all reached tables. Globally called unrestricted selectors then retain corresponding pairs in every reached table; receiver cones/exact class facts can narrow individual calls, but cannot make the unconditional table roots disappear.',
 'Known constant-CID object_new calls already create table-reference edges. They provide a starting mechanism for allocation-based class activation, but runtime/foreign allocation paths require an independent completeness proof.',
 'Class/library bytes establish linked retention, not unreachability or executed cost. Image decoder support may be reachable through actual preview operations; do not assume all decoder code is dead.'
], 'next_general_size_step':{
 'target':'Conservative allocated-class plus selector reachability for fixed tracing AOT images, coordinated HIR summary and emitter registration.',
 'initial_implementation':'Build a closed allocation inventory from actual constant-CID object creation and table references in reachable bodies, SDK summary/certified foreign return allocations and explicit runtime-created classes. Couple class activation to the function/selector fixed point. Keep every class when completeness is unknown; remove unconditional registration only for independently proven unallocated tables.',
 'preserve':['RTI/ancestry and class names needed by type operations even when instance method tables are absent','inherited method implementations of every activated class','callback, entry-point, helper and tearoff bodies and their reachable allocations','runtime-created exception, stacktrace, closure, collection and FFI compound classes','actual canonical constants whose initialization can allocate instances'],
 'deny_or_widen':['JIT/reload and non-fixed images','class registry mutation or ambiguous/missing ancestry','reflection, dynamic constructor invocation or dynamic symbol resolution with unknown targets','native Handle/Ref returns, callbacks or aliases with unproved class creation/exposure','unknown class IDs passed to allocation helpers and uncertified foreign allocation paths'],
 'existing_gates':['DARTFORGE_FACTORY_RECEIVER_PROVENANCE default on: physical allocation/factory return facts, not a complete allocation-universe proof','registry::closed detects explicit mutation names and resolvable native aliases; it does not alone certify arbitrary NativeCall behavior','imported allocation certificates reconcile artifact/class/source identities; unknown records remain conservative','ROOT_SELECTORS remain protocol roots only for activated tables; do not remove protocol support globally'],
 'minimal_tests':['unused class table and all its protocol methods disappear while allocated class remains','same selector on two unrelated classes retains only allocated receiver class','allocation in an inherited method activates the returned class transitively','tearoff/callback/entry-point/global lazy initialization keeps its body and allocation table','runtime-created classes and imported SDK allocation return tables stay live','unknown native/reflection/class-ID paths disable narrowing rather than producing missing dispatch','GUI navigation/back/close and image previews exercise preserved class tables','compare unchanged native output and full gate before any size claim'],
 'limitation':'This is a design proposal, not implemented or a quantified saving. Imported/runtime allocation completeness is the major proof obligation.'
}}
(out/'diagnosis.json').write_text(json.dumps(d,indent=2)+'\n')
manifest={p.name:sha(p) for p in out.iterdir() if p.is_file() and p.name!='hashes.json'}
(out/'hashes.json').write_text(json.dumps(manifest,indent=2)+'\n')
print('archived',len(manifest),'hashed files')
