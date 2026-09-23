"""Grava os tipos esperados do corpus de conformidade da inferência
(`corpus/inferencia/`) com o oráculo (`oraculo.dart`, package:analyzer).

    python tools/oraculo_tipos/gravar_corpus.py --packages <package_config.json> [corpus/inferencia]

`<package_config.json>` é qualquer um que resolva `package:analyzer` 6.11.0 e
`package:path` (o do `new_sali/frontend`, por exemplo): o oráculo roda UM
processo `dart` para o corpus inteiro. Para cada `X.dart` escreve
`X.esperado.tsv`, uma linha por expressão:

    linha  coluna  offset  comprimento  nó  tipo  elemento  marca  trecho

`offset`/`comprimento` são unidades UTF-16 (as do analyzer); `linha`/`coluna`
começam em 1. `marca` é 1 quando a expressão começa logo depois de um
comentário `/*@*/` — as expressões que o programa quer verificar; as outras
também são verdade do oráculo. Ver docs/INFERENCIA-ESPECIFICACAO.md, §0.
"""
import os
import subprocess
import sys
from collections import defaultdict

MARCA = '/*@*/'


def main():
    args = sys.argv[1:]
    if '--packages' not in args:
        sys.exit('uso: gravar_corpus.py --packages <package_config.json> [raiz]')
    i = args.index('--packages')
    pacotes = args[i + 1]
    del args[i:i + 2]
    aqui = os.path.dirname(os.path.abspath(__file__))
    repo = os.path.dirname(os.path.dirname(aqui))
    raiz = os.path.abspath(args[0] if args else os.path.join(repo, 'corpus', 'inferencia'))
    progs = sorted(
        os.path.join(d, f).replace('\\', '/')
        for d, _, fs in os.walk(raiz) for f in fs if f.endswith('.dart'))
    tmp = os.environ.get('TEMP') or os.environ.get('TMP') or raiz
    lista = os.path.join(tmp, 'corpus_inferencia_arquivos.txt')
    saida = os.path.join(tmp, 'corpus_inferencia_oraculo.tsv')
    with open(lista, 'w', encoding='utf-8') as f:
        f.write('\n'.join(progs) + '\n')
    subprocess.run(['dart', '--packages=' + pacotes, os.path.join(aqui, 'oraculo.dart'),
                    raiz, lista, saida], check=True)
    por = defaultdict(list)
    with open(saida, encoding='utf-8') as f:
        for linha in f:
            p = linha.rstrip('\n').split('\t')
            if linha.startswith('#'):
                sys.exit('oráculo falhou: ' + linha)
            if len(p) >= 6:
                por[p[0].lower()].append(p)
    for prog in progs:
        texto = open(prog, encoding='utf-8', newline='').read()
        u16 = texto.encode('utf-16-le')
        linhas = []
        for (_, ini, comp, no, tipo, el) in por.get(prog.lower(), []):
            ini, comp = int(ini), int(comp)
            antes = u16[:2 * ini].decode('utf-16-le')
            lin = antes.count('\n') + 1
            col = len(antes) - (antes.rfind('\n') + 1) + 1
            marca = 1 if antes.rstrip(' ').endswith(MARCA) else 0
            trecho = u16[2 * ini:2 * (ini + min(comp, 60))].decode('utf-16-le')
            trecho = ' '.join(trecho.split())
            linhas.append((ini, -comp, f'{lin}\t{col}\t{ini}\t{comp}\t{no}\t{tipo}\t{el}\t{marca}\t{trecho}'))
        linhas.sort()
        rel = os.path.relpath(prog, raiz).replace('\\', '/')
        with open(prog[:-5] + '.esperado.tsv', 'w', encoding='utf-8', newline='\n') as f:
            f.write(f'# {rel} — gravado pelo oráculo (package:analyzer 6.11.0, Dart 3.6.2)\n')
            f.write('# linha\tcoluna\toffset\tcomprimento\tnó\ttipo\telemento\tmarca\ttrecho\n')
            for _, _, l in linhas:
                f.write(l + '\n')
    print(f'{len(progs)} programas gravados em {raiz}')


if __name__ == '__main__':
    main()
