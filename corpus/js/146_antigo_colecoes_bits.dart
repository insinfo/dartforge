// Convertido de tests/conformance/cases/colecoes_bits.dart (fixture antigo do corpus de conformidade).
// diverge-ddc: operadores de bits com operandos negativos devolvem 32 bits sem sinal na web
// Operadores de bits onde a VM e o alvo web divergem: `int` na web e um
// double de 64 bits, e `&`, `|`, `^`, `~`, `<<`, `>>` e `>>>` trabalham
// sobre 32 bits sem sinal. A saida esperada deste fixture e a de
// `dart compile js -O2`; ver docs/COLECOES-OPERADORES.md.
void main() {
  print(~0);
  print(~5);
  print(-1 | 0);
  print(-2 ^ 0);
  print(-8 >> 1);
  print(-1 >>> 28);
  print(-1 >>> 0);
  print(-1 << 1);
  print(1 << 31);
  print(1 << 32);
  print(1 << 40);
  print(8 >> 32);
  print(-8 >> 40);
  print(2147483647 + 1);
}
