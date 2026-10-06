r"""Gera `crates/analise/src/lints/regexp_unicode_g.rs`: as tabelas Unicode que o
analisador de expressões regulares da VM do Dart 3.6.2 consulta no ICU 74.2
(Unicode 15.1): `ID_Start`/`ID_Continue` (nomes de grupos de captura) e os nomes
aceitos em `\p{...}` com `unicode: true` (`AddPropertyClassRange` de
`runtime/vm/regexp_parser.cc`).

Uso: python scripts/gerar-regexp-unicode.py [diretório do UCD 15.1.0]
(padrão: E:/dftemp/ucd-15.1.0; os arquivos que faltam são baixados de
https://www.unicode.org/Public/15.1.0/ucd/).
"""
import io
import os
import re
import sys
import urllib.request

UCD = sys.argv[1] if len(sys.argv) > 1 else 'E:/dftemp/ucd-15.1.0'
SAIDA = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', 'crates', 'analise', 'src', 'lints', 'regexp_unicode_g.rs')
ARQUIVOS = ['PropertyAliases.txt', 'PropertyValueAliases.txt', 'Scripts.txt', 'ScriptExtensions.txt', 'DerivedCoreProperties.txt']

# As propriedades binárias de `IsSupportedBinaryProperty`, pelo nome curto do UCD.
BINARIAS = [
    'Alpha', 'AHex', 'Bidi_C', 'Bidi_M', 'CI', 'Cased', 'CWCF', 'CWCM', 'CWL', 'CWKCF', 'CWT', 'CWU', 'Dash', 'DI', 'Dep',
    'Dia', 'Emoji', 'EComp', 'EBase', 'EMod', 'EPres', 'ExtPict', 'Ext', 'Gr_Base', 'Gr_Ext', 'Hex', 'IDC', 'IDS', 'Ideo',
    'IDSB', 'IDST', 'Join_C', 'LOE', 'Lower', 'Math', 'NChar', 'Pat_Syn', 'Pat_WS', 'QMark', 'Radical', 'RI', 'STerm', 'SD',
    'Term', 'UIdeo', 'Upper', 'VS', 'WSpace', 'XIDC', 'XIDS',
]


def ler(nome):
    caminho = os.path.join(UCD, nome)
    if not os.path.exists(caminho):
        os.makedirs(UCD, exist_ok=True)
        urllib.request.urlretrieve('https://www.unicode.org/Public/15.1.0/ucd/' + nome, caminho)
    return io.open(caminho, encoding='utf-8').read()


def linhas(texto):
    for l in texto.splitlines():
        l = l.split('#', 1)[0].strip()
        if l:
            yield [c.strip() for c in l.split(';')]


def faixas(texto, propriedade):
    v = []
    for c in linhas(texto):
        if len(c) >= 2 and c[1] == propriedade:
            a, _, b = c[0].partition('..')
            v.append((int(a, 16), int(b or a, 16)))
    v.sort()
    juntas = []
    for a, b in v:
        if juntas and a <= juntas[-1][1] + 1:
            juntas[-1] = (juntas[-1][0], max(juntas[-1][1], b))
        else:
            juntas.append((a, b))
    return juntas


def main():
    for a in ARQUIVOS:
        ler(a)
    pa = list(linhas(ler('PropertyAliases.txt')))
    pva = list(linhas(ler('PropertyValueAliases.txt')))
    # Os nomes de cada propriedade binária suportada (curto, longo e extras).
    binarias = []
    for curto in BINARIAS:
        achadas = [c for c in pa if c[0] == curto]
        assert len(achadas) == 1, curto
        binarias.extend(achadas[0])
    # Os valores da categoria geral (com os grupos L, LC, M, N, P, S, Z, C).
    categorias = []
    for c in pva:
        if c[0] == 'gc':
            categorias.extend(c[1:])
    # As escritas: um valor só vale se o conjunto dele não é vazio.
    scripts = ler('Scripts.txt')
    com_sc = {'Unknown'}
    for c in linhas(scripts):
        com_sc.add(c[1])
    curto_do_longo = {}
    nomes_da_escrita = []
    for c in pva:
        if c[0] == 'sc':
            curto_do_longo[c[2]] = c[1]
            nomes_da_escrita.append(c[1:])
    com_sc_curto = {curto_do_longo[n] for n in com_sc}
    com_scx_curto = set(com_sc_curto)
    for c in linhas(ler('ScriptExtensions.txt')):
        com_scx_curto.update(c[1].split())
    sc = [n for nomes in nomes_da_escrita if nomes[0] in com_sc_curto for n in nomes]
    scx = [n for nomes in nomes_da_escrita if nomes[0] in com_scx_curto for n in nomes]
    dcp = ler('DerivedCoreProperties.txt')
    id_start = faixas(dcp, 'ID_Start')
    id_continue = faixas(dcp, 'ID_Continue')

    def lista(nome, doc, v):
        return '/// %s\npub static %s: [&str; %d] = [%s];\n' % (doc, nome, len(v), ', '.join('"%s"' % x for x in sorted(set(v))))

    def tabela(nome, doc, v):
        corpo = ''.join('    (0x%X, 0x%X),\n' % x for x in v)
        return '/// %s\npub static %s: [(u32, u32); %d] = [\n%s];\n' % (doc, nome, len(v), corpo)

    out = '// GERADO por `scripts/gerar-regexp-unicode.py` (frente INFRA-LINTS). NÃO EDITE.\n'
    out += '// Fonte: o UCD 15.1.0 (o Unicode do ICU 74.2 da VM do Dart 3.6.2).\n\n'
    # Os tamanhos são os de nomes distintos.
    out += lista('CATEGORIAS_GERAIS', 'Os nomes (exatos) dos valores de `General_Category`, com os grupos.', categorias).replace('[&str; %d]' % len(categorias), '[&str; %d]' % len(set(categorias)))
    out += '\n' + lista('PROPRIEDADES_BINARIAS', 'Os nomes das propriedades binárias de `IsSupportedBinaryProperty`.', binarias).replace('[&str; %d]' % len(binarias), '[&str; %d]' % len(set(binarias)))
    out += '\n' + lista('ESCRITAS', 'Os nomes das escritas com algum caractere em `Script`.', sc).replace('[&str; %d]' % len(sc), '[&str; %d]' % len(set(sc)))
    out += '\n' + lista('ESCRITAS_ESTENDIDAS', 'Os nomes das escritas com algum caractere em `Script_Extensions`.', scx).replace('[&str; %d]' % len(scx), '[&str; %d]' % len(set(scx)))
    out += '\n' + tabela('ID_START', 'As faixas de `ID_Start`, ordenadas e disjuntas.', id_start)
    out += '\n' + tabela('ID_CONTINUE', 'As faixas de `ID_Continue`, ordenadas e disjuntas.', id_continue)
    io.open(SAIDA, 'w', encoding='utf-8', newline='\n').write(out)
    print('ok', len(set(categorias)), len(set(binarias)), len(set(sc)), len(set(scx)), len(id_start), len(id_continue))


main()
