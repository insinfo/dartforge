// D14 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): no modo mapas, o
// local cujo endereço escapa (a variável mutável capturada por uma função
// local) continua no quadro residual da pilha-sombra (§3.7).

class C {
  final int v;
  C(this.v);
}

void main() {
  var x = C(0);
  void troca(int i) {
    x = C(x.v + i);
  }

  for (var i = 0; i < 1000; i++) {
    troca(i);
    final t = 'q$i';
    if (t.isEmpty) print(t);
  }
  print(x.v);
}
