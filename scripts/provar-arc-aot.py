"""Executa a matriz AOT ARC/tracing com controles negativos e artefatos brutos.

Não aceita falha de compilação como resultado negativo. Os controles precisam
terminar por trap; erro de carregamento, abort do runtime ou saída inesperada
reprovam a prova. O controle layout-obsoleto exige o abort específico da guarda
física e sua mensagem, pois a ABI não permite desenrolar o pânico para o Dart.
O manifesto conserva comandos, versões e hashes dos arquivos.
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
    "arc_literal_listas": ("prova-literal-listas", False, [
        ("normal", None), ("normal", "sem-owner"), ("normal", "sem-liberar"),
    ]),
    "arc_armazenamento_listas": ("prova-armazenamento-listas", False, [
        ("normal", None), ("normal", "sem-retencao"), ("normal", "sem-liberar"),
        ("--crescer", None), ("--crescer", "sem-retencao"), ("--crescer", "sem-liberar"),
    ]),
    "arc_caixas_simd": ("prova-caixas-simd", False, [("normal", None), ("normal", "sem-liberar")]),
    "arc_retorno_tipado_escalar": ("prova-retorno-tipado-escalar", False, [
        (tipo, controle) for tipo in ("--int", "--double", "--bool")
        for controle in (None, "sem-liberar-contexto")
    ]),
    "arc_porta_nativa_owned": ("prova-porta-nativa-owned", True, [
        (modo, controle) for modo in ("checagem", "tabelas")
        for controle in (None, "sem-registro", "sem-fechamento")
    ]),
    "arc_retorno_da_fonte": ("prova-retorno-da-fonte", False, [
        ("normal", None), ("normal", "sem-retencao"), ("normal", "sem-drop-retorno"),
        ("normal", "sem-drop-intermediario"),
    ]),
    "arc_instancia_zerada": ("prova-instancia-zerada", False, [
        ("normal", None), ("normal", "sem-owner"),
        ("normal", "sem-tabela"), ("normal", "sem-drop"),
        ("normal", "layout-obsoleto"),
    ]),
    "arc_campos_locais": ("prova-campos-locais", False, [
        ("normal", None), ("normal", "sem-retencao"),
        ("normal", "sem-drop-objeto"), ("normal", "sem-drop-retorno"),
    ]),
    "arc_alocacao_campos": ("prova-alocacao-campos", False, [
        ("normal", None), ("normal", "sem-drop-filho"), ("normal", "sem-owner-objeto"),
        ("--quadro", None), ("--quadro", "sem-drop-filho"), ("--quadro", "sem-owner-objeto"),
        ("--quadro", "sem-retencao-leitura"),
        ("--tipada", None), ("--tipada", "sem-drop-filho"), ("--tipada", "sem-owner-objeto"),
        ("--tipada", "sem-liberar-contexto"),
        ("--tipada-erro", None), ("--tipada-erro", "sem-liberar-contexto"),
    ]),
    "arc_keepalive_campos": ("prova-keepalive-campos", False, [
        ("normal", None), ("normal", "sem-retencao"), ("normal", "sem-liberar"),
        ("--vizinho", None), ("--vizinho", "sem-retencao"), ("--vizinho", "sem-liberar"),
        ("--argumento", None), ("--argumento", "sem-retencao"), ("--argumento", "sem-liberar"),
        ("--captura-vizinho", None), ("--captura-vizinho", "sem-retencao"), ("--captura-vizinho", "sem-liberar"),
        ("--anulavel", None), ("--anulavel", "sem-retencao"), ("--anulavel", "sem-liberar"),
        ("--misto", None), ("--misto", "sem-retencao"), ("--misto", "sem-liberar"),
        ("--ciclo", None), ("--ciclo", "sem-retencao"), ("--ciclo", "sem-liberar"),
        ("--celula", None), ("--celula", "sem-retencao"), ("--celula", "sem-liberar"),
        ("--contexto", None), ("--contexto", "sem-retencao"), ("--contexto", "sem-liberar"),
        ("--global", None), ("--global", "sem-retencao"), ("--global", "sem-liberar"),
        ("--late", None), ("--late", "sem-retencao"), ("--late", "sem-liberar"),
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
    if args.exemplo == "arc_retorno_da_fonte":
        sdk_lib = ambiente.get("DARTFORGE_SDK_LIB")
        if not sdk_lib:
            raise RuntimeError("a prova da fonte exige DARTFORGE_SDK_LIB")
        manifesto["sdk_lib"] = str(Path(sdk_lib).resolve())
        manifesto["sdk_versao"] = (Path(sdk_lib).parent / "version").read_text(encoding="utf-8").strip()
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
                    if tem_caminho or caminho.startswith("--"):
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
                    if args.exemplo == "arc_instancia_zerada" and controle == "layout-obsoleto":
                        aborts = {0xC0000409, -1073740791} if os.name == "nt" else {-int(signal.SIGABRT)}
                        diagnostico = stderr.read_bytes()
                        if code not in aborts or stdout.read_bytes() != b"" or not all(
                            trecho in diagnostico for trecho in (
                                b"layout escalar ARC incompat", b"classe/tamanho fora dos layouts Ref")):
                            raise RuntimeError(f"guarda de layout não recusou pelo motivo esperado: {stem} ({code})")
                        registro["rejeicao"] = "guarda de layout físico"
                    elif controle:
                        traps = {0xC000001D, -1073741795} if os.name == "nt" else {-int(signal.SIGILL), -int(signal.SIGTRAP)}
                        if code not in traps or stdout.read_bytes() != b"":
                            raise RuntimeError(f"controle negativo não terminou por trap: {stem} ({code})")
                    elif code != 0 or stdout.read_bytes() != b"1\n":
                        raise RuntimeError(f"prova falhou: {stem} ({code}); consulte {stdout} e {stderr}")
                    arquivos = [log, exe.with_suffix(".ll"), stdout, stderr, destino / f"{stem}.exit"]
                    if args.exemplo == "arc_retorno_da_fonte":
                        arquivos.append(exe.with_suffix(".dart"))
                    for arquivo in arquivos:
                        dados = arquivo.read_bytes()
                        registro["arquivos"].append({"nome": arquivo.name, "bytes": len(dados),
                            "sha256": hashlib.sha256(dados).hexdigest()})
                    resultado = "guarda de layout" if registro.get("rejeicao") else "trap" if controle else "passou"
                    print(f"{args.exemplo}: {stem}: {resultado}", flush=True)
        manifesto["resultado"] = "sucesso"
    except Exception as erro:
        manifesto["resultado"] = "falha"
        manifesto["erro"] = str(erro)
        raise
    finally:
        try:
            # `open(newline=)` em vez de `write_text(newline=)`, que exige Python 3.10.
            with manifesto_path.open("w", encoding="utf-8", newline="\n") as saida:
                saida.write(json.dumps(manifesto, ensure_ascii=False, indent=2) + "\n")
        finally:
            temporario.cleanup()


if __name__ == "__main__":
    main()
