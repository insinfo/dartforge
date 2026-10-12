// D9 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): a exceção Dart que
// sai de uma função que o runtime chamou. `Function.apply` chama o
// closure pelo runtime (`dartforge_nativo_Function_apply`, pela porta `dart_r3`); o
// `then` que lança roda numa microtarefa do laço de eventos. Com as
// exceções por tabelas, a porta é o que impede a exceção de atravessar os
// quadros Rust.
//
// Saída: 925

class Erro implements Exception {
  final int n;
  Erro(this.n);
}

int lanca(int x) {
  // Aloca antes de lançar.
  final s = 'v$x';
  if (x.isOdd) throw Erro(x + s.length);
  return x;
}

Future<void> main() async {
  var total = 0;
  for (var k = 0; k < 40; k++) {
    try {
      total += Function.apply(lanca, [k]) as int;
    } on Erro catch (e) {
      total += e.n;
    }
  }
  for (var k = 0; k < 10; k++) {
    final r = await Future.value(k).then((v) => lanca(v)).catchError((Object e) => (e as Erro).n * 2);
    total += r;
  }
  print(total);
}
