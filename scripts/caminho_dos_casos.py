"""Troca, nos documentos de especificação, o caminho antigo dos casos dirigidos
(pasta temporária fora do repositório) pelo caminho versionado
`corpus/especificacao`. Rodar da raiz do repositório:

    python scripts/caminho_dos_casos.py
"""
import glob
import io

B = chr(92)
ANTIGO = 'E:' + B + 'dftemp' + B + 'analise' + B + 'spec-r4' + B + 'casos'
NOVO = 'corpus' + B + 'especificacao'

total = 0
for caminho in glob.glob('docs/*.md'):
    texto = io.open(caminho, 'r', encoding='utf-8', newline='').read()
    n = texto.count(ANTIGO)
    if n:
        io.open(caminho, 'w', encoding='utf-8', newline='').write(texto.replace(ANTIGO, NOVO))
        print(caminho, n)
        total += n
print('trocas:', total)
