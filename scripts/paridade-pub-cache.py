#!/usr/bin/env python3
"""Prepara os pacotes do pub-cache como projetos reais do placar de paridade.

Para cada pacote do pub-cache (`hosted/pub.dev`), a última versão; ficam de
fora os que dependem do Flutter e os de linguagem anterior a 2.12 (o Dart 3
não os analisa com null safety). De cada um, `lib/` e o `pubspec.yaml` vão
para `<destino>/pacotes/<nome>` (sem `analysis_options.yaml`: os lints não são
da paridade), com um `package_config.json` que resolve todos os escolhidos
pelas cópias. O registro `projetos.json` do corpus alternativo
`<destino>/corpus` lista os pacotes para o `dartforge-paridade projetos`:

    python scripts/paridade-pub-cache.py E:/dftemp/analise/pub
    dartforge-paridade projetos --oraculo --corpus E:/dftemp/analise/pub/corpus

(o `--oraculo` roda o `dart analyze` 3.6.2 de cada pacote; sem ele, o
registro gravado é reaproveitado). Nada é escrito no pub-cache.
"""

import json
import os
import re
import shutil
import sys


def pub_cache():
    base = os.environ.get("PUB_CACHE") or os.path.join(os.environ.get("LOCALAPPDATA", ""), "Pub", "Cache")
    return os.path.join(base, "hosted", "pub.dev")


def chave_versao(v):
    """Ordem de versão semântica suficiente para escolher a última estável."""
    base, _, pre = v.partition("-")
    nums = [int(x) if x.isdigit() else 0 for x in re.split(r"[.+]", base)[:3]]
    # Estável vence pré-lançamento da mesma base.
    return (nums, 0 if pre else 1, pre)


def ler_pubspec(caminho):
    try:
        return open(caminho, encoding="utf-8", errors="replace").read()
    except OSError:
        return ""


def versao_de_linguagem(pubspec):
    m = re.search(r"^\s*sdk:\s*['\"]?\s*(?:>=|\^)\s*(\d+)\.(\d+)", pubspec, re.M)
    if not m:
        return None
    return int(m.group(1)), int(m.group(2))


def main():
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    destino = os.path.abspath(sys.argv[1])
    origem = pub_cache()
    ultimas = {}
    for d in os.listdir(origem):
        nome, sep, versao = d.partition("-")
        if not sep or not os.path.isdir(os.path.join(origem, d, "lib")):
            continue
        if nome not in ultimas or chave_versao(versao) > chave_versao(ultimas[nome]):
            ultimas[nome] = versao
    escolhidos = {}
    for nome, versao in sorted(ultimas.items()):
        raiz = os.path.join(origem, f"{nome}-{versao}")
        pubspec = ler_pubspec(os.path.join(raiz, "pubspec.yaml"))
        if re.search(r"^\s*(flutter|flutter_test)\s*:\s*\n\s*sdk:\s*flutter", pubspec, re.M):
            continue
        lv = versao_de_linguagem(pubspec)
        if lv is None or lv < (2, 12):
            continue
        escolhidos[nome] = (raiz, lv)
    pacotes = os.path.join(destino, "pacotes")
    shutil.rmtree(pacotes, ignore_errors=True)
    config = {
        "configVersion": 2,
        "generator": "paridade-pub-cache",
        "packages": [
            {
                "name": nome,
                "rootUri": "file:///" + os.path.join(pacotes, nome).replace("\\", "/"),
                "packageUri": "lib/",
                "languageVersion": f"{lv[0]}.{lv[1]}",
            }
            for nome, (_, lv) in sorted(escolhidos.items())
        ],
    }
    registro = []
    for nome, (raiz, _) in sorted(escolhidos.items()):
        alvo = os.path.join(pacotes, nome)
        shutil.copytree(os.path.join(raiz, "lib"), os.path.join(alvo, "lib"))
        shutil.copy(os.path.join(raiz, "pubspec.yaml"), os.path.join(alvo, "pubspec.yaml"))
        os.makedirs(os.path.join(alvo, ".dart_tool"), exist_ok=True)
        with open(os.path.join(alvo, ".dart_tool", "package_config.json"), "w", encoding="utf-8") as f:
            json.dump(config, f, indent=2)
        registro.append([{"nome": nome, "caminho": alvo.replace("\\", "/"), "sdk": "3.6.2"}, []])
    corpus = os.path.join(destino, "corpus")
    os.makedirs(corpus, exist_ok=True)
    antigo = os.path.join(corpus, "projetos.json")
    if os.path.exists(antigo):
        # Mantém o oráculo gravado dos pacotes que continuam na lista.
        gravados = {p["nome"]: (p, regs) for p, regs in json.load(open(antigo, encoding="utf-8"))}
        registro = [list(gravados.get(p["nome"], (p, regs))) for p, regs in registro]
    with open(antigo, "w", encoding="utf-8") as f:
        json.dump(registro, f, indent=1)
    print(f"{len(escolhidos)} pacotes em {pacotes}; registro em {antigo}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
