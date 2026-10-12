// Comparativo entre linguagens (RELATORIO-COMPARATIVO-NATIVO.md): os mesmos
// quatro núcleos em cada uma; `scripts/comparar-linguagens.py` compila e mede.
class No {
  final No? esq, dir;
  No(this.esq, this.dir);
  int conta() => 1 + (esq?.conta() ?? 0) + (dir?.conta() ?? 0);
}
No arvore(int d) => d == 0 ? No(null, null) : No(arvore(d - 1), arvore(d - 1));
int arvores(int maxD) {
  final longa = arvore(maxD); var total = 0;
  for (var d = 4; d <= maxD; d += 2) { final it = 1 << (maxD - d + 4); for (var i = 0; i < it; i++) total += arvore(d).conta(); }
  return total + longa.conta();
}
int fib(int n) => n < 2 ? n : fib(n - 1) + fib(n - 2);
int numerico(int n) { var s = 0; for (var i = 0; i < n; i++) { s = (s + i * i) % 1000003; } return s; }
int textos(int n) {
  final m = <String, int>{};
  for (var i = 0; i < n; i++) { final k = 'k${i % 5000}'; m[k] = (m[k] ?? 0) + i; }
  var s = 0; m.forEach((_, v) { s += v % 1000; }); return s;
}
void medir(String nome, int Function() f) {
  var r = 0; final t = <int>[];
  for (var i = 0; i < 4; i++) { final sw = Stopwatch()..start(); r = f(); t.add(sw.elapsedMilliseconds); }
  print('$nome: ${t.join(' ')} ms | $r');
}
void main() {
  medir('fib', () => fib(32));
  medir('arvores', () => arvores(14));
  medir('numerico', () => numerico(30000000));
  medir('textos', () => textos(1000000));
}
