"""Gera crates/lsp/src/especies_g.rs a partir das tabelas de
docs/LSP-ESPECIFICACAO.md: §13.7.2 (`DartFixKind`) e a de `DartAssistKind`
(§13.8): o id, a prioridade e a mensagem de cada espécie, ordenadas pelo id
para busca binária. Ids repetidos ficam com a primeira linha (a ordem da
fonte do Dart).

Uso: python scripts/gerar-especies-de-acao.py
"""
import io
import os
import re

RAIZ = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DOC = os.path.join(RAIZ, 'docs', 'LSP-ESPECIFICACAO.md')
SAIDA = os.path.join(RAIZ, 'crates', 'lsp', 'src', 'especies_g.rs')

LINHA = re.compile(r'^\| `([A-Z_0-9]+)` \| `(dart\.(?:fix|assist)\.[^`]+)` \| (\d+)[^|]*\| `(.*)` \| \d+ \|\s*$')


def rust(s):
    return '"' + s.replace('\\', '\\\\').replace('"', '\\"') + '"'


def main():
    texto = io.open(DOC, encoding='utf-8').read()
    vistos = {}
    for linha in texto.splitlines():
        m = LINHA.match(linha)
        if not m:
            continue
        _, ident, pri, msg = m.groups()
        # Célula com crases dentro: `` `` texto `` ``.
        if msg.startswith('` ') and msg.endswith(' `'):
            msg = msg[2:-2]
        if ident not in vistos:
            vistos[ident] = (int(pri), msg)
    itens = sorted(vistos.items())
    with io.open(SAIDA, 'w', encoding='utf-8', newline='\n') as f:
        f.write('//! Gerado por scripts/gerar-especies-de-acao.py a partir de\n')
        f.write('//! docs/LSP-ESPECIFICACAO.md (§13.7.2 e a tabela de `DartAssistKind`).\n')
        f.write('//! Não editar à mão.\n\n')
        f.write('/// (id, prioridade, mensagem), em ordem de id.\n')
        f.write('pub(crate) const ESPECIES: &[(&str, u16, &str)] = &[\n')
        for ident, (pri, msg) in itens:
            f.write(f'    ({rust(ident)}, {pri}, {rust(msg)}),\n')
        f.write('];\n')
    print(len(itens), 'espécies')


if __name__ == '__main__':
    main()
