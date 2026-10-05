// D10 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): uma closure que
// captura um objeto e é a única dona dele, com `await` no meio e coleta
// durante a suspensão.

class Caixa {
  final String s;
  Caixa(this.s);
}

Future<int> espera(int i) async {
  final c = Caixa('c$i');
  final f = () => c.s.length;
  await Future<void>.delayed(Duration.zero);
  final lixo = <Caixa>[];
  for (var k = 0; k < 200; k++) {
    lixo.add(Caixa('$k'));
  }
  return f() + lixo.length;
}

Future<void> main() async {
  var total = 0;
  for (var i = 0; i < 20; i++) {
    total += await espera(i);
  }
  print(total);
}
