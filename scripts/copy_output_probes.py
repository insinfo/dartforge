"""Copia as sondas s1-s7 de E:/dftemp/analise/spec-infra/sonda para
corpus/analyze-output (docs/ANALYZER-ESPECIFICACAO-INFRA.md, III.1, item 4).

Cada sonda vira um caso: `project/` (a pasta da sonda), `ARGS` (`.`) e os
três `EXPECTED.<formato>.txt`, com o caminho absoluto da sonda trocado por
`<raiz>` (nas grafias com `\\\\`, `\\` e `/`). O `EXPECTED.code` sai na
próxima gravação (`dartforge-parity output --write`).
"""

import os
import shutil
import sys

ORIGEM = sys.argv[1] if len(sys.argv) > 1 else 'E:/dftemp/analise/spec-infra/sonda'
DESTINO = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'corpus', 'analyze-output')


def normalizar(texto, raiz):
    a = os.path.abspath(raiz)
    for forma in (a.replace('\\', '\\\\'), a, a.replace('\\', '/')):
        texto = texto.replace(forma, '<raiz>')
    return texto.replace('\r\n', '\n')


for nome in sorted(os.listdir(ORIGEM)):
    pasta = os.path.join(ORIGEM, nome)
    if not os.path.isdir(pasta):
        continue
    caso = os.path.join(DESTINO, nome)
    projeto = os.path.join(caso, 'project')
    if os.path.exists(projeto):
        shutil.rmtree(projeto)
    shutil.copytree(pasta, projeto)
    with open(os.path.join(caso, 'ARGS'), 'w', encoding='utf-8', newline='\n') as f:
        f.write('.\n')
    for formato in ('default', 'json', 'machine'):
        origem = os.path.join(ORIGEM, f'{nome}-{formato}.txt')
        if not os.path.exists(origem):
            continue
        with open(origem, encoding='utf-8') as f:
            texto = normalizar(f.read(), pasta)
        with open(os.path.join(caso, f'EXPECTED.{formato}.txt'), 'w', encoding='utf-8', newline='\n') as f:
            f.write(texto)
    print('caso', nome)
