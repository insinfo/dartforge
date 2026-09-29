#!/usr/bin/env python3
"""Regenera os oráculos dos casos de perfil do motor de build.

Uso:
    python3 scripts/corpus-perfis.py <caso> --dart <dart> [--pub-cache <dir>]

`<caso>` é um diretório com `pubspec.lock` (as versões fixadas) e
`configuracoes.json`: a lista das execuções do `build_runner` oficial,
`[{"nome": "padrao", "argumentos": []}, ...]`. Para cada uma o script apaga
`.dart_tool/build` e as saídas `build_to: source` da execução anterior, roda
`dart run build_runner build <argumentos>` e grava em `oraculos/<nome>/`:

    manifesto.json   versões, argumentos e as saídas (asset, build_to, sha256)
    cache/<pkg>/...  o que ficou em .dart_tool/build/generated
    source/...       os arquivos que o build criou na árvore do pacote

O `dart` é o SDK que o lock pede (3.6.2 para o build_runner 2.4.15, 3.13 para
o 2.16.1); a VM só produz o oráculo. `pub get --enforce-lockfile` roda antes.
"""
import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

INFRA = [
    "analyzer", "build", "build_config", "build_resolvers", "build_runner",
    "build_runner_core", "dart_style", "glob",
]


def versoes(caso: Path) -> dict:
    """Versões do lock dos pacotes de infraestrutura (ordem alfabética)."""
    atual, saida = None, {}
    for linha in (caso / "pubspec.lock").read_text(encoding="utf-8").splitlines():
        if linha.startswith("  ") and not linha.startswith("    ") and linha.endswith(":"):
            atual = linha.strip()[:-1]
        elif linha.startswith("    version:") and atual in INFRA:
            saida[atual] = linha.split('"')[1]
    return dict(sorted(saida.items()))


def arquivos(raiz: Path) -> set:
    """Arquivos da árvore do pacote, fora de `.dart_tool`, `build` e `oraculos`."""
    v = set()
    for dirpath, dirnames, filenames in os.walk(raiz):
        rel = Path(dirpath).relative_to(raiz)
        dirnames[:] = [
            d for d in dirnames
            if not (rel == Path(".") and d in (".dart_tool", "build", "oraculos"))
            and d != ".dart_tool"
        ]
        for f in filenames:
            v.add((rel / f).as_posix())
    return v


def sha256(p: Path) -> str:
    return hashlib.sha256(p.read_bytes()).hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("caso", type=Path)
    ap.add_argument("--dart", required=True)
    ap.add_argument("--pub-cache")
    a = ap.parse_args()
    caso = a.caso.resolve()
    env = dict(os.environ)
    if a.pub_cache:
        env["PUB_CACHE"] = a.pub_cache
    versao_dart = subprocess.run(
        [a.dart, "--version"], capture_output=True, text=True, env=env
    )
    dart = (versao_dart.stdout + versao_dart.stderr).split()[3]
    subprocess.run([a.dart, "pub", "get", "--enforce-lockfile"], cwd=caso, env=env, check=True)
    configuracoes = json.loads((caso / "configuracoes.json").read_text(encoding="utf-8"))
    nome_raiz = next(
        l.split(":", 1)[1].strip()
        for l in (caso / "pubspec.yaml").read_text(encoding="utf-8").splitlines()
        if l.startswith("name:")
    )
    antes = arquivos(caso)
    for cfg in configuracoes:
        shutil.rmtree(caso / ".dart_tool" / "build", ignore_errors=True)
        r = subprocess.run(
            [a.dart, "run", "build_runner", "build", *cfg["argumentos"]],
            cwd=caso, env=env, capture_output=True, text=True,
        )
        if r.returncode != 0:
            print(r.stdout, r.stderr, sep="\n", file=sys.stderr)
            print(f"{cfg['nome']}: build_runner falhou", file=sys.stderr)
            return 1
        destino = caso / "oraculos" / cfg["nome"]
        shutil.rmtree(destino, ignore_errors=True)
        saidas = []
        gerado = caso / ".dart_tool" / "build" / "generated"
        for dirpath, _, filenames in os.walk(gerado):
            for f in filenames:
                p = Path(dirpath) / f
                rel = p.relative_to(gerado).as_posix()
                pacote, caminho = rel.split("/", 1)
                alvo = destino / "cache" / rel
                alvo.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(p, alvo)
                saidas.append({"asset": f"{pacote}|{caminho}", "build_to": "cache", "sha256": sha256(p)})
        for rel in sorted(arquivos(caso) - antes):
            p = caso / rel
            alvo = destino / "source" / rel
            alvo.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(p, alvo)
            saidas.append({"asset": f"{nome_raiz}|{rel}", "build_to": "source", "sha256": sha256(p)})
            p.unlink()
        saidas.sort(key=lambda s: s["asset"].encode("utf-8"))
        manifesto = {
            "dart": dart,
            "pacotes": versoes(caso),
            "argumentos": cfg["argumentos"],
            "saidas": saidas,
            "excluidos": 0,
            "executavel": False,
        }
        linhas = [
            "{",
            f'  "dart": {json.dumps(manifesto["dart"])},',
            '  "pacotes": ' + json.dumps(manifesto["pacotes"], ensure_ascii=False) + ",",
            '  "argumentos": ' + json.dumps(manifesto["argumentos"], ensure_ascii=False) + ",",
            '  "saidas": [',
            ",\n".join(
                "    " + json.dumps(s, ensure_ascii=False) for s in saidas
            ),
            "  ],",
            '  "excluidos": 0,',
            '  "executavel": false',
            "}",
        ]
        destino.mkdir(parents=True, exist_ok=True)
        (destino / "manifesto.json").write_text("\n".join(linhas) + "\n", encoding="utf-8")
        print(f"{cfg['nome']}: {len(saidas)} saídas")
    shutil.rmtree(caso / ".dart_tool" / "build", ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
