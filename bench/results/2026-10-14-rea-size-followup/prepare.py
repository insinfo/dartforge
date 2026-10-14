import pathlib, json, hashlib, re
root=pathlib.Path('/Volumes/ssd_dock/MyRustProjects/dartforge')
out=root/'bench/results/2026-10-14-rea-size-followup'
work=root/'target/rea-size-followup'
binary=root/'target/selector-getter-production/file-explorer/program'
b=binary.read_bytes()
assert hashlib.sha256(b).hexdigest()=='c3f1c6e90bde7f440c03cf1f542d5a934e4d3f99d98ed5e5430c01763d6abad1'
items=[('color',0x101067A18,0x5a04),('ascii-encode',0x101642028,0x188),('ascii-getter',0x101642788,0x6c)]
for label,addr,size in items:
 raw=work/(label+'.bin');raw.write_bytes(b[addr-0x100000000:addr-0x100000000+size])
 cfg={'label':'forge-'+label,'path':str(raw),'open_options':{'format':'raw-binary','architecture':'arm64','entry_point':hex(addr)},'procedures':[{'label':label,'address':hex(addr)}]}
 (out/(label+'-plan.json')).write_text(json.dumps(cfg,indent=2))
 (out/(label+'-origin.json')).write_text(json.dumps({'container':str(binary),'container_sha256':hashlib.sha256(b).hexdigest(),'offset':addr-0x100000000,'address':hex(addr),'bytes':size,'raw_sha256':hashlib.sha256(raw.read_bytes()).hexdigest(),'scope':'Exact mapped ARM64 function bytes; external data and call targets absent, no complete callgraph or runtime cost claim.'},indent=2))
cfg={'label':'official-color','path':str(root/'target/rea-native-analysis/official-color-raw.bin'),'open_options':{'format':'raw-binary','architecture':'arm64','entry_point':'0x3566c8'},'procedures':[{'label':'color','address':'0x3566c8'}]}
(out/'official-plan.json').write_text(json.dumps(cfg,indent=2))
