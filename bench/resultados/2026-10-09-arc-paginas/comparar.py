"""Comparação dirigida com executáveis congelados antes/depois."""
import ctypes
import importlib.util
import json
from pathlib import Path
import statistics

raiz = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("medidor", raiz / "scripts/medir-modos-desempenho.py")
medidor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(medidor)
k = ctypes.windll.kernel32
k.GetCurrentProcess.restype = ctypes.c_void_p
assert k.SetProcessAffinityMask(ctypes.c_void_p(k.GetCurrentProcess()), ctypes.c_size_t(4))
nomes = ["A0-antes", "A0-depois", "ARC-antes", "ARC-depois"]
exes = {
    nome: raiz / ("target/tmp-modos-faixas" if nome.endswith("antes") else "target/tmp-arc-paginas")
          / ("objetos_escapam-" + nome.split("-")[0] + ".exe")
    for nome in nomes
}
medidas, resultados, presentes = {}, {}, {}
with (Path(__file__).parent / "amostras.jsonl").open("w", encoding="utf-8") as arquivo:
    for rep in range(5):
        for nome in nomes[rep % 4:] + nomes[:rep % 4]:
            codigo, nucleos, stdout, stderr = medidor.rodar(str(exes[nome]))
            arquivo.write(json.dumps(dict(variante=nome, repeticao=rep+1, codigo=codigo,
                                         nucleos=nucleos, stdout=stdout, stderr=stderr)) + "\n")
            arquivo.flush()
            assert codigo == 0 and nucleos, (nome, codigo, stderr)
            presentes[nome, rep] = set(nucleos)
            for nucleo, (t, resultado) in nucleos.items():
                assert resultados.setdefault(nucleo, resultado) == resultado
                medidas.setdefault((nome, nucleo), []).append(t)
        print(f"repetição {rep+1}/5", flush=True)
assert set(resultados) == {"arvores", "lista_ligada"}
assert all(n == set(resultados) for n in presentes.values())
linhas = []
for modo in ["A0", "ARC"]:
    for nucleo in sorted(resultados):
        antes, depois = [statistics.median(medidas[modo + "-" + v, nucleo]) for v in ["antes", "depois"]]
        linhas.append(f"{modo}/{nucleo}: {antes/1000:.3f} -> {depois/1000:.3f} ms; depois/antes={depois/antes:.5f}")
texto = "\n".join(linhas)
(Path(__file__).parent / "resultado.txt").write_text(texto + "\n", encoding="utf-8")
print(texto)
