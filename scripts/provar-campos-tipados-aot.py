"""Prova fonte Dart → AOT para campos tipados, com ARC opt-in e tracing padrão."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dartforge", type=Path, required=True,
                        help="CLI já compilada com a fonte que se deseja verificar")
    parser.add_argument("--saida", type=Path, required=True)
    args = parser.parse_args()
    raiz = Path(__file__).resolve().parents[1]
    cli = args.dartforge.resolve()
    saida = args.saida.resolve()
    saida.mkdir(parents=True, exist_ok=True)
    fixture = raiz / "crates/emit_native/tests/fixtures/campos_tipados_arc.dart"
    layout = raiz / "crates/runtime/src/layout.rs"
    limite = int(re.search(r"CAMPOS_EM_LINHA: usize = (\d+)",
                           layout.read_text(encoding="utf-8")).group(1))
    fonte = fixture.read_text(encoding="utf-8")
    # A fixture tem seis campos de Base, um do mixin e trinta int de Folha.
    # Insere padding para que extensa/extenso/externo ultrapassem o inline.
    if limite < 37:
        raise RuntimeError("limite inline menor que o prefixo da fixture")
    padding = "".join(f"  int reserva{i} = 0;\n" for i in range(limite - 37))
    fontes = {"inline": fonte, "extensao": fonte.replace(
        "  double extensa = 3.5;", padding + "  double extensa = 3.5;")}
    ambiente = dict(os.environ)
    ambiente.pop("DARTFORGE_MEMORIA", None)
    ambiente.update(TEMP=str(saida), TMP=str(saida))
    execucao = dict(ambiente, DARTFORGE_GC_STRESS="1",
                    DARTFORGE_ARC_CONFERIR="1", DARTFORGE_ARC_BERCARIO="0")
    (saida / "evidencia.json").write_text('{"status": "em_execucao"}\n', encoding="utf-8")
    evidencias = []
    for nome, texto in fontes.items():
        entrada = saida / f"{nome}.dart"
        entrada.write_text(texto, encoding="utf-8")
        for arc in [False, True]:
            for otimizar in [False, True]:
                caso = f"{nome}-{'arc' if arc else 'padrao'}-{'o2' if otimizar else 'o0'}"
                exe = saida / (caso + (".exe" if os.name == "nt" else ".bin"))
                comando = [str(cli), "compile-native", str(entrada), "-o", str(exe)]
                if arc:
                    comando.append("--memoria=arc")
                if otimizar:
                    comando.append("--optimize")
                build = subprocess.run(comando, cwd=raiz, env=ambiente, capture_output=True)
                (saida / f"{caso}-build.stdout").write_bytes(build.stdout)
                (saida / f"{caso}-build.stderr").write_bytes(build.stderr)
                if build.returncode:
                    raise RuntimeError(f"{caso}: compilação {build.returncode}: "
                                       + build.stderr.decode(errors="replace"))
                resultado = subprocess.run([str(exe)], cwd=saida, env=execucao, capture_output=True)
                (saida / f"{caso}.stdout").write_bytes(resultado.stdout)
                (saida / f"{caso}.stderr").write_bytes(resultado.stderr)
                (saida / f"{caso}.exit").write_text(str(resultado.returncode), encoding="ascii")
                if resultado.returncode != 0:
                    raise RuntimeError((caso, resultado.returncode, resultado.stderr))
                if resultado.stdout != b"campos tipados: ok\n":
                    raise RuntimeError((caso, resultado.stdout))
                evidencias.append(dict(caso=caso, comando=comando, fonte_sha256=sha(entrada),
                                       exe_sha256=sha(exe), stdout_sha256=sha(saida / f"{caso}.stdout")))
                print(f"{caso}: aprovado", flush=True)
    fontes_verificadas = [fixture, layout, Path(__file__),
        raiz / "crates/emit_native/src/lower/membros.rs",
        raiz / "crates/emit_native/src/llvm/mod.rs",
        raiz / "crates/emit_native/src/otimizar/escape.rs"]
    manifesto = dict(status="aprovado", base=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=raiz).decode().strip(),
        cli_sha256=sha(cli), limite_inline=limite,
        fontes={str(p.relative_to(raiz)): sha(p) for p in fontes_verificadas}, casos=evidencias)
    (saida / "evidencia.json").write_text(json.dumps(manifesto, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
