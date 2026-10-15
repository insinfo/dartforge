from pathlib import Path
import hashlib,json,re
root=Path(__file__).resolve().parents[4]
work=root/'target/constant-specialization-color-performance-v2'
out=Path(__file__).resolve().parent
summary={'scope':'Pre-LTO static IR sites and final ARM64 link-map intervals. Counts are not executed call frequencies.'}
for mode in ['off','on']:
    ir=work/mode/'.df_tmp/program.ll'; data=ir.read_bytes(); text=data.decode()
    selectors={int(i):re.sub(r'\\([0-9A-Fa-f]{2})',lambda m:chr(int(m[1],16)),v) for i,v in re.findall(r'@df.seln.(\d+) = .* c"(.*)"',text)}
    bodies={}
    for block in re.split(r'(?=^define )',text,flags=re.M):
        if not block.startswith('define '):continue
        header=block.splitlines()[0]
        if not any(name in header for name in ['..convertPixels(', '..convertFormatValue(', '@df.specialized.']):continue
        body=block.split('\n}',1)[0]+'\n}\n'; symbol=re.search(r'@([^ (]+)',header)[1]
        label='caller' if '..convertPixels(' in header else ('generic-callee' if '..convertFormatValue(' in header else 'specialized-callee')
        (out/f'{mode}-{label}.ll').write_text(body)
        selcalls=[selectors.get(int(i),str(i)) for i in re.findall(r'call ptr @df.selector\([^\n]*ptr @df.seln.(\d+)',body)]
        gettercalls=re.findall(r'call i64 @([^ (]+\.Format\.[^ (]+)\(',body)
        bodies[label]={'symbol':symbol,'ir_bytes':len(body.encode()),'selector_sites':selcalls,'selector_count':len(selcalls),'format_getter_sites':gettercalls,'format_getter_count':len(gettercalls),'box_int_calls':len(re.findall(r'call i64 @df.box_int\(',body)),'box_double_calls':len(re.findall(r'call i64 @df.box_double\(',body)),'conditional_branches':len(re.findall(r'^\s*br i1 ',body,re.M)),'direct_clamp_calls':re.findall(r'call i64 @([^ (]+\.clamp)\(',body),'direct_integer_toInt_calls':len(re.findall(r'call i64 @df.dart\$3acore._IntegerImplementation.toInt\(',body)),'native_double_toInt_calls':len(re.findall(r'call i64 @dartforge_native_DartForge_double_toInt\(',body))}
    mapped=[];map_path=work/mode/'program.map'; map_bytes=map_path.read_bytes()
    for line in map_bytes.decode().splitlines():
        if any(name in line for name in ['..convertPixels','..convertFormatValue','_df.specialized.']):
            m=re.match(r'(0x[0-9A-Fa-f]+)\s+(0x[0-9A-Fa-f]+)\s+\[\s*(\d+)\]\s+(.*)',line)
            if m:mapped.append({'address':m[1],'bytes':int(m[2],16),'object':int(m[3]),'symbol':m[4]})
    binary=(work/mode/'program').read_bytes()
    summary[mode]={'ir_sha256':hashlib.sha256(data).hexdigest(),'ir_bytes':len(data),'map_sha256':hashlib.sha256(map_bytes).hexdigest(),'binary_sha256':hashlib.sha256(binary).hexdigest(),'binary_bytes':len(binary),'focused':bodies,'mapped_symbols':mapped}
summary['diagnosis']=['The ordinary caller targets the bounded specialized clone in ON IR; the generic color conversion body is absent after reachability pruning.','The ON final map retains the larger convertPixels caller but no standalone specialized callee symbol. This is consistent with LLVM LTO inlining/internalization; the clone has no separately attributable final interval.','Remaining specialized IR includes generic enum equality, lazy getter calls, guarded SDK clamp, boxed multiplication slow fallback and integer conversion paths. Preserving these effects/checks was intentional; no hot-path frequency is asserted.','Next focused investigation: certify selected SDK primitive clamp/compareTo semantics and constant bounds, preserving NaN ordering, signed zero, mixed int/double rounding and exact returned bound object. A guarded scalar pipeline could reduce temporary boxing and SDK dispatch while retaining fallback. Container element-type proof is a separate subsequent task.']
(out/'diagnosis.json').write_text(json.dumps(summary,indent=2)+'\n')
manifest={str(p.relative_to(out)):hashlib.sha256(p.read_bytes()).hexdigest() for p in out.iterdir() if p.is_file() and p.name!='hashes.json'}
(out/'hashes.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps(summary,indent=2))
