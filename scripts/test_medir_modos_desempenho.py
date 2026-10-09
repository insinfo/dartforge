"""Verifica que falhas intermitentes não viram uma comparação válida."""
import contextlib
import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SCRIPT = Path(__file__).with_name("medir-modos-desempenho.py")
spec = importlib.util.spec_from_file_location("medidor", SCRIPT)
medidor = importlib.util.module_from_spec(spec)
spec.loader.exec_module(medidor)


def saida(resultado="42", codigo=0, nucleos=True):
    dados = {"soma": (20, resultado)} if nucleos else {}
    return codigo, dados, f"soma: 10 20 20 us | {resultado}\n", ""


class Medicao(unittest.TestCase):
    def rodada(self, execucoes, compilacao=True):
        # O teste simula dois modos e duas repetições do mesmo programa,
        # passando pelo main inteiro e pelos arquivos de saída do medidor.
        raiz = SCRIPT.parent.parent / "target" / "tmp-test-medidor"
        raiz.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=raiz) as pasta:
            pasta = Path(pasta)
            (pasta / "caso.dart").write_text("", encoding="utf-8")
            modos = [("A0", "sombra", "checagem", "tracing"),
                     ("ARC", "sombra", "checagem", "arc")]
            exe = str(pasta / "caso.exe") if compilacao else None
            with patch.object(medidor, "BENCH", str(pasta)), \
                 patch.object(medidor, "MODOS", modos), \
                 patch.object(medidor, "compilar", return_value=(exe, 0)), \
                 patch.object(medidor, "rodar", side_effect=execucoes), \
                 patch("sys.argv", [str(SCRIPT), str(pasta), "--sem-dart", "--repeticoes", "2"]), \
                 contextlib.redirect_stdout(io.StringIO()):
                codigo = medidor.main()
            amostras = [json.loads(linha) for linha in (pasta / "amostras.jsonl").read_text(encoding="utf-8").splitlines()]
            return codigo, (pasta / "resultado.md").read_text(encoding="utf-8"), amostras

    def test_preserva_todas_as_execucoes_e_ordem_alternada(self):
        codigo, relatorio, amostras = self.rodada([saida()] * 4)
        self.assertEqual(codigo, 0)
        self.assertIn("Resultados iguais", relatorio)
        self.assertEqual([r["modo"] for r in amostras[1:]], ["A0", "ARC", "ARC", "A0"])
        self.assertEqual([r["repeticao"] for r in amostras[1:]], [1, 1, 2, 2])
        self.assertTrue(all("10 20 20" in r["stdout"] for r in amostras[1:]))

    def test_falhas_anteriores_nao_somem_na_ultima_repeticao(self):
        for falha in (saida("43"), saida(codigo=1), saida(nucleos=False),
                      subprocess.TimeoutExpired("caso", 1800, output=b"parcial"), OSError("indisponível")):
            with self.subTest(falha=falha):
                codigo, relatorio, amostras = self.rodada([saida(), falha, saida(), saida()])
                self.assertEqual(codigo, 1)
                self.assertIn("Rodada inválida", relatorio)
                self.assertEqual(len(amostras), 5)
                if isinstance(falha, subprocess.TimeoutExpired):
                    self.assertEqual(amostras[2]["stdout"], "parcial")

    def test_compilacao_falha_invalida_rodada(self):
        codigo, relatorio, amostras = self.rodada([], compilacao=False)
        self.assertEqual(codigo, 1)
        self.assertIn("compilação falhou", relatorio)
        self.assertEqual(len(amostras), 1)


if __name__ == "__main__":
    unittest.main()
