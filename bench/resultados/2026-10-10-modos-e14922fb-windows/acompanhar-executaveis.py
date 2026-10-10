import hashlib
import json
from pathlib import Path
import re
import time

out = Path(__file__).resolve().parent
snapshots = {}
pattern = re.compile(r'^\s+(\w+) (A0|ARC|dart): ok \(', re.MULTILINE)
while len(snapshots) < 27:
    log = (out / 'gate.log').read_text(encoding='utf-8', errors='replace')
    for program, mode in pattern.findall(log):
        name = f'{program}-{mode}.exe'
        if name in snapshots:
            continue
        binary = out / name
        digest = hashlib.sha256(binary.read_bytes()).hexdigest()
        after_log = (out / 'gate.log').read_text(encoding='utf-8', errors='replace')
        sample_path = out / 'amostras.jsonl'
        completed = max(0, len(sample_path.read_text(encoding='utf-8').splitlines()) - 1) if sample_path.exists() else 0
        snapshots[name] = {'nome': name, 'sha256': digest,
                           'fase2_iniciada': '# Fase 2:' in after_log,
                           'execucoes_concluidas_no_snapshot': completed,
                           'compilacao_confirmada_no_log': True}
        result = {'executaveis': list(snapshots.values()),
                  'observacao': 'Cada hash foi capturado após a compilação confirmada; a auditoria confere se precedeu a primeira execução do arquivo.'}
        (out / 'executaveis-acompanhamento.json').write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8', newline='\n')
        print(f'{len(snapshots)}/27: {name}', flush=True)
    time.sleep(0.2)
