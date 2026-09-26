// Regressão: com muito dado vivo perto do teto do heap, o gatilho da coleta
// tem histerese (antes, passar da metade do teto coletava em toda alocação
// e o programa parava de andar); e o vetor de um literal de lista num laço
// nasce no bloco de entrada (antes, um `alloca` por volta estourava a pilha).
void main() {
  // ~ muitos objetos vivos (acima da metade do teto) e temporários em volta.
  final vivos = <List<int>>[];
  for (var i = 0; i < 9000; i++) {
    vivos.add(List<int>.filled(400, i));
  }
  var soma = 0;
  for (var i = 0; i < 300000; i++) {
    final t = <int>[i, i + 1, i + 2];
    soma += t[1];
  }
  print('${vivos.length} $soma');
}
