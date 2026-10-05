# Gera crates/runtime/efeitos.tsv: a tabela de efeitos por extern (E1.1).
# Primeira versão = o que o emissor supõe hoje (EXTERNS de llvm/externs.rs);
# o que ele não declara é conservador. `roda_dart` vem da análise do grafo de
# chamadas (scripts/dados/efeitos-runtime/efeitos.tsv, gerado por
# scripts/analisar-efeitos-runtime.py), só onde a marca já
# é conservadora (roda_dart = 1 implica coleta = 1 e lanca = 1).
import re, os
R = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), 'crates').replace(os.sep, '/') + '/'
src = R + 'runtime/src/'
build = open(R + 'runtime/build.rs', encoding='utf-8').read()
frag = re.search(r'const FRAGMENTOS: &\[&str\] = &\[(.*?)\n\];', build, re.S).group(1)
fragmentos = re.findall(r'^\s*"(\w+)",', frag, re.M)
nomes = []
for f in fragmentos:
    linhas = open(src + f + '.rs', encoding='utf-8').read().replace('\r\n', '\n').split('\n')
    i = 0
    while i < len(linhas):
        if linhas[i].strip() == '#[unsafe(no_mangle)]':
            j = i + 1
            while j < len(linhas) and linhas[j].strip().startswith('#['):
                j += 1
            s = linhas[j].strip()
            m = re.match(r'pub (?:unsafe )?extern "C" fn (\w+)', s)
            assert m, (f, s)
            if m.group(1) != 'main':
                nomes.append(m.group(1))
            i = j
        i += 1
assert len(nomes) == len(set(nomes))

ext = open(R + 'emit_native/src/llvm/externs.rs', encoding='utf-8').read()
marcas = {}
for m in re.finditer(r'decl: "declare [^"]*?@(\w+)\([^"]*",\s*(?://[^\n]*\n\s*)*efeitos: ([^\n]*)', ext):
    nome, e = m.group(1), m.group(2).strip().rstrip(',')
    if e == 'CONSERVADOR':
        v = (1, 1, 0)
    elif e == 'ALOCA_SEM_LANCAR':
        v = (1, 0, 0)
    else:
        k = re.match(r'Efeitos \{ aloca: (true|false), lanca: (true|false), chama_dart: (true|false) \}', e)
        assert k, (nome, e)
        v = tuple(int(x == 'true') for x in k.groups())
    marcas[nome] = v
n_decl = len(re.findall(r'decl: "declare ', ext))
print('externs declaradas:', n_decl, 'lidas:', len(marcas))
assert n_decl == len(marcas)

analise = {}
p = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'dados', 'efeitos-runtime', 'efeitos.tsv')
if os.path.exists(p):
    for l in open(p, encoding='utf-8').read().split('\n')[1:]:
        c = l.split('\t')
        if len(c) >= 5:
            analise[c[0]] = c[4] not in ('0', 'tabela', '')

fora = [n for n in marcas if n not in set(nomes)]
print('declaradas pelo emissor e ausentes do runtime:', fora)
saida = ['# A tabela de efeitos das externs do runtime (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §13.8).',
         '# Uma linha por função `#[unsafe(no_mangle)]` dos fragmentos, em ordem alfabética:',
         '#   coleta    pode alocar no heap (logo, coletar)',
         '#   lanca     pode deixar uma exceção pendente',
         '#   roda_dart pode chamar código Dart gerado (implica coleta = 1 e lanca = 1)',
         '# O `build.rs` do runtime confere a lista contra os fragmentos e gera a tabela que o',
         '# emissor lê (`dartforge_runtime::efeitos`). Uma marca só passa de 1 para 0 junto com a',
         '# execução que a confere (`DARTFORGE_EFEITOS=conferir`).',
         '# nome\tcoleta\tlanca\troda_dart']
cont = {}
for n in sorted(nomes):
    a, l, d = marcas.get(n, (1, 1, 0))
    if a and l and analise.get(n, False):
        d = 1
    if d and not (a and l):
        d = 0
    cont[(a, l, d)] = cont.get((a, l, d), 0) + 1
    saida.append('%s\t%d\t%d\t%d' % (n, a, l, d))
open(R + 'runtime/efeitos.tsv', 'w', encoding='utf-8', newline='\n').write('\n'.join(saida) + '\n')
print(len(nomes), 'externs;', cont)
# as marcas de chama_dart que o emissor já tinha
print('chama_dart no emissor:', [n for n, v in marcas.items() if v[2]])
