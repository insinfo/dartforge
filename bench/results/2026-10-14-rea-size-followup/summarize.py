import pathlib,json,re,hashlib,subprocess
root=pathlib.Path('/Volumes/ssd_dock/MyRustProjects/dartforge');out=pathlib.Path(__file__).resolve().parent
symbols=[]
for l in (root/'target/selector-getter-production/file-explorer/program.map').read_text().splitlines():
 m=re.match(r'(0x[0-9a-fA-F]+)\s+(0x[0-9a-fA-F]+)\s+\[.*?\]\s+(.*)',l)
 if m:symbols.append((int(m[1],16),int(m[2],16),m[3]))
stats=[]
for folder,label,size in [('forge-color','color',23044),('official-color','color',8452),('forge-ascii-encode','ascii-encode',392)]:
 data=json.loads((out/folder/(label+'-procedure_assembly.json')).read_text())['result']['structuredContent']
 asm=data['normalized_result'];(out/folder/(label+'.s')).write_text(asm+'\n')
 pseudo=json.loads((out/folder/(label+'-procedure_pseudo_code.json')).read_text())['result']['structuredContent']['normalized_result'];(out/folder/(label+'.c')).write_text(pseudo+'\n')
 rows=[l for l in asm.splitlines() if re.match(r'0x[0-9a-f]+:',l)]
 calls=[]
 for l in rows:
  m=re.search(r': bl (0x[0-9a-f]+)$',l)
  if m:
   addr=int(m[1],16);matches=[s[2] for s in symbols if s[0]<=addr<s[0]+s[1]] if folder.startswith('forge') else []
   calls.append({'instruction':l,'mapped_symbols':matches})
 (out/folder/'calls-mapped.json').write_text(json.dumps(calls,indent=2))
 stats.append({'target':folder,'symbol_bytes':size,'decoded_bytes':len(rows)*4,'instructions':len(rows),'complete_symbol_coverage':len(rows)*4==size,'direct_BL_sites':len(calls),'indirect_BLR_sites':sum(': blr ' in l for l in rows),'MRS_sites':sum(': mrs ' in l for l in rows)})
# Preserve only relevant SDK ABI body, not the complete 300 MB IR.
text=(root/'target/selector-getter-production/file-explorer/.df_tmp/program.ll').read_text()
for name in ['AsciiCodec.encode$c','AsciiCodec.encoder$g','convertFormatValue']:
 matches=[]
 for m in re.finditer(r'^define .*?^}',text,re.M|re.S):
  header=m[0].splitlines()[0]
  if name in header:matches.append(m[0])
 (out/(name.replace('$','-')+'.ll')).write_text('\n\n'.join(matches)+'\n')
old=root/'target/numeric-sdk-cache-final-production/file-explorer/program.map'
oldsizes=[]
if old.exists():
 oldsizes=[l for l in old.read_text().splitlines() if 'convertFormatValue' in l]
validation={'scope':'Static local REA 6.5/Ghidra 12.1.4 exact mapped ARM64 function evidence, no timings. Raw slices do not contain external helpers/data; pseudocode is heuristic, not recovered Dart semantics.','current_binary':{'sha256':hashlib.sha256((root/'target/selector-getter-production/file-explorer/program').read_bytes()).hexdigest(),'bytes':(root/'target/selector-getter-production/file-explorer/program').stat().st_size,'input_manifest_sha256':'cb9bdc8c2495d25e3154c4ca8cd841506fd4ed57725331c5180be2d07f901995'},'upstream_binary':{'sha256':hashlib.sha256((root/'target/current-native-size/official/program').read_bytes()).hexdigest(),'bytes':(root/'target/current-native-size/official/program').stat().st_size,'snapshot_and_function_origin':'../2026-10-14-rea-native-investigation/rea-evidence/official-color-raw-origin.json'},'statistics':stats,'previous_wave_color_map':oldsizes,'no_performance_or_this_wave_size_gain_claim':True}
(out/'validation.json').write_text(json.dumps(validation,indent=2))
