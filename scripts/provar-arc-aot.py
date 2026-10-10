"""Executa a matriz AOT ARC/tracing com controles negativos e artefatos brutos.

Não aceita falha de compilação como resultado negativo. Os controles precisam
terminar por trap; erro de carregamento, abort do runtime ou saída inesperada
reprovam a prova. O manifesto conserva comandos, versões e hashes dos arquivos.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import tempfile


PROVAS = {
    "arc_keepalive_campos": ("prova-keepalive-campos", False, [
        ("normal", None), ("normal", "sem-retencao"), ("normal", "sem-liberar"),
    ]),
    "arc_keepalive_pending": ("prova-keepalive-pending", True, [
        ("normal", None), ("erro", None), ("normal", "sem-retencao"),
        ("normal", "sem-liberar"), ("erro", "copia-no-erro"),
    ]),
    "arc_cleanup_laco": ("prova-cleanup-laco", True, [
        ("normal", None), ("erro", None), ("normal", "sem-drop-normal"),
        ("erro", "sem-drop-erro"),
    ]),
}


def informacao(comando):
    return subprocess.check_output(comando, text=True, encoding="utf-8").strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("exemplo", choices=PROVAS)
    parser.add_argument("--debug", action="store_true", help="compilar o gerador sem --release")
    parser.add_argument("--features", help="features do gerador, como llvm-embutido")
    args = parser.parse_args()
    diretorio, tem_caminho, casos = PROVAS[args.exemplo]
    root = Path(__file__).resolve().parent.parent
    destino = root / "target" / diretorio
    destino.mkdir(parents=True, exist_ok=True)
    ambiente = os.environ.copy()
    ambiente.update({
        "DARTFORGE_ARC_CONFERIR": "1",
        "DARTFORGE_ARC_BERCARIO": "0",
        "DARTFORGE_GC_STRESS": "1",
    })
    # Traps intencionais não precisam gerar dumps de memória no runner Unix.
    if os.name != "nt":
        import resource
        resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    manifesto = {
        "fonte": informacao(["git", "-C", str(root), "rev-parse", "HEAD"]),
        "exemplo": args.exemplo,
        "exemplo_sha256": hashlib.sha256((root / "crates/emit_native/examples" / (args.exemplo + ".rs")).read_bytes()).hexdigest(),
        "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "estado_git_fontes": informacao(["git", "-C", str(root), "status", "--porcelain", "--",
            "crates/emit_native", "crates/runtime", "scripts/provar-arc-aot.py"]),
        "rustc": informacao(["rustc", "--version"]),
        "ambiente": {k: ambiente[k] for k in (
            "DARTFORGE_ARC_CONFERIR", "DARTFORGE_ARC_BERCARIO", "DARTFORGE_GC_STRESS")},
        "execucoes": [],
    }
    manifesto_path = destino / "evidencia.json"
    target = (root / "target").resolve()
    if not target.is_relative_to(root):
        raise RuntimeError("diretório de temporários fora do repositório")
    temporario = tempfile.TemporaryDirectory(prefix="tmp-arc-aot-", dir=target)
    ambiente["TEMP"] = ambiente["TMP"] = temporario.name
    try:
        for nivel in (0, 2):
            for modo in ("arc", "tracing"):
                for caminho, controle in casos:
                    stem = f"{modo}-O{nivel}-{caminho}"
                    if controle:
                        stem += "-" + controle
                    exe = destino / (stem + (".exe" if os.name == "nt" else ""))
                    comando = ["cargo", "run", "--locked"]
                    if not args.debug:
                        comando.append("--release")
                    comando += ["-p", "dartforge-emit-native"]
                    if args.features:
                        comando += ["--features", args.features]
                    comando += ["--example", args.exemplo, "--", str(exe), f"--memoria={modo}", str(nivel)]
                    if tem_caminho:
                        comando.append(caminho)
                    if controle:
                        comando.append(controle)
                    registro = {"modo": modo, "nivel": nivel, "caminho": caminho,
                        "controle": controle, "geracao": comando, "arquivos": []}
                    manifesto["execucoes"].append(registro)
                    log = destino / f"{stem}.build.log"
                    with log.open("wb") as out:
                        geracao = subprocess.run(comando, cwd=root, env=ambiente, stdout=out, stderr=subprocess.STDOUT)
                    registro["exit_geracao"] = geracao.returncode
                    if geracao.returncode:
                        raise RuntimeError(f"compilação falhou: {stem}; consulte {log}")
                    stdout = destino / f"{stem}.stdout"
                    stderr = destino / f"{stem}.stderr"
                    with stdout.open("wb") as out, stderr.open("wb") as err:
                        processo = subprocess.run([str(exe)], cwd=root, env=ambiente, stdout=out, stderr=err)
                    code = processo.returncode
                    registro["exit"] = code
                    (destino / f"{stem}.exit").write_bytes(f"{code}\n".encode("ascii"))
                    if controle:
                        traps = {0xC000001D, -1073741795} if os.name == "nt" else {-int(signal.SIGILL), -int(signal.SIGTRAP)}
                        if code not in traps or stdout.read_bytes() != b"":
                            raise RuntimeError(f"controle negativo não terminou por trap: {stem} ({code})")
                    elif code != 0 or stdout.read_bytes() != b"1\n":
                        raise RuntimeError(f"prova falhou: {stem} ({code}); consulte {stdout} e {stderr}")
                    for arquivo in (log, exe.with_suffix(".ll"), stdout, stderr, destino / f"{stem}.exit"):
                        dados = arquivo.read_bytes()
                        registro["arquivos"].append({"nome": arquivo.name, "bytes": len(dados),
                            "sha256": hashlib.sha256(dados).hexdigest()})
                    print(f"{args.exemplo}: {stem}: {'trap' if controle else 'passou'}", flush=True)
        manifesto["resultado"] = "sucesso"
    except Exception as erro:
        manifesto["resultado"] = "falha"
        manifesto["erro"] = str(erro)
        raise
    finally:
        try:
            manifesto_path.write_text(json.dumps(manifesto, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
        finally:
            temporario.cleanup()


if __name__ == "__main__":
    main()
