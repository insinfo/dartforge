"""Remove funções Rust de um arquivo, com a documentação e os atributos delas.

Uso: python scripts/remover-funcao.py <arquivo.rs> <nome> [<nome> ...]
Acha `fn <nome>` (com qualquer visibilidade), recua sobre as linhas de `///`
e `#[...]` imediatamente acima e apaga até a chave que fecha o corpo.
Strings e comentários dentro do corpo não são interpretados: serve para o
código do repositório, que não tem chaves desbalanceadas em literais nas
funções mortas.
"""
import io
import re
import sys

caminho = sys.argv[1]
texto = io.open(caminho, encoding='utf-8').read()
for nome in sys.argv[2:]:
    m = re.search(r'^[ \t]*(pub(\([a-z]+\))? )?(const )?(async )?fn ' + re.escape(nome) + r'\b', texto, re.M)
    if not m:
        print('não achei', nome)
        continue
    ini = m.start()
    # Recua sobre doc e atributos.
    linhas_antes = texto[:ini].split('\n')
    while len(linhas_antes) > 1 and re.match(r'^\s*(///|#\[)', linhas_antes[-2]):
        linhas_antes.pop(-2)
    ini = len('\n'.join(linhas_antes[:-1])) + (1 if len(linhas_antes) > 1 else 0)
    abre = texto.index('{', m.end())
    prof = 0
    i = abre
    while i < len(texto):
        c = texto[i]
        if c == '{':
            prof += 1
        elif c == '}':
            prof -= 1
            if prof == 0:
                break
        i += 1
    fim = i + 1
    # Come a quebra de linha e uma linha em branco seguinte.
    if texto[fim:fim + 1] == '\n':
        fim += 1
    if texto[fim:fim + 1] == '\n' and texto[ini - 2:ini] == '\n\n':
        fim += 1
    texto = texto[:ini] + texto[fim:]
    print('removi', nome)
io.open(caminho, 'w', encoding='utf-8', newline='\n').write(texto)
