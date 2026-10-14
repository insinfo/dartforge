"""Extract actual SDK receiver proof and retained convert chain from paired maps."""
from pathlib import Path
import hashlib,json,re,shutil
ROOT=Path.cwd();OUT=ROOT/'target/canonical-cache-sizes';DEST=ROOT/'bench/results/2026-10-14-native-canonical-cache/sizes'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
results={}
for profile in ['off','on']:
    folder=OUT/profile;linkmap=(folder/'program.map').read_text();match=re.search(r'^\[\s*\d+\]\s+(.*?/sdk/[^/]+/convert\.o)$',linkmap,re.M)
    assert match,'SDK convert object provenance not in actual map'
    summary=Path(match[1]).with_suffix('.poda').resolve();lines=summary.read_text().splitlines();names=[line[2:] for line in lines if line.startswith('N\t')];caller='df.dart$3aconvert.AsciiCodec.encode$c';node=names.index(caller)
    selectors={int(parts[1]):parts[2] for line in lines if (parts:=line.split('\t'))[0]=='S'}
    watched={'tc:convert','c:convert'};exact=[];unrestricted=[];definition=None
    for line in lines:
        parts=line.split('\t')
        if parts[0]=='E' and int(parts[1])==node and selectors.get(int(parts[2])) in watched:
            exact.append({'selector':selectors[int(parts[2])],'CID':int(parts[3]),'raw':line})
        if parts[0]=='D' and int(parts[1])==node:
            definition=line;unrestricted=[selectors[int(value)] for value in parts[3].split() if selectors.get(int(value)) in watched]
    class_tables=[]
    for line in lines:
        parts=line.split('\t')
        if parts[0]=='T' and any(entry['CID']==int(parts[2]) for entry in exact):
            class_tables.append({'table':names[int(parts[1])],'CID':int(parts[2]),'raw':line})
    why=folder/'retention.tsv';rows=[]
    for line in why.read_text().splitlines():
        if 'PixelUint8.convert$tc' in line or caller in line or ('tc:convert' in line and 'Pixel' in line):rows.append(line)
    item={'SDK_summary':str(summary),'SDK_summary_sha256':sha(summary),'SDK_object_sha256':sha(summary.with_suffix('.o')),'caller':caller,'definition':definition,'exact_receiver_records':exact,'unrestricted_convert_selectors':unrestricted,'exact_class_tables':class_tables,'all_exact_receiver_record_count':sum(line.startswith('E\t')for line in lines),'WHY_sha256':sha(why),'watched_retention_rows':rows,'scope':'Actual cached SDK .poda paired with linked convert object; E records express physical CID union. WHY cause rows describe selected retention cause, not exhaustive paths.'}
    results[profile]=item
    DEST.mkdir(parents=True,exist_ok=True);(DEST/(profile+'-retention-excerpt.tsv')).write_text('\n'.join(rows)+'\n')
assert results['off']['unrestricted_convert_selectors'],results['off']
assert not results['on']['unrestricted_convert_selectors'],results['on']
assert {entry['selector'] for entry in results['on']['exact_receiver_records']}=={'tc:convert','c:convert'},results['on']
assert all('AsciiEncoder' in entry['table'] for entry in results['on']['exact_class_tables']),results['on']
(DEST/'receiver-retention.json').write_text(json.dumps(results,indent=2)+'\n')
print({profile:{'exact':item['exact_receiver_records'],'unrestricted':item['unrestricted_convert_selectors'],'E_count':item['all_exact_receiver_record_count']}for profile,item in results.items()})
