// Depois de um `StackOverflowError` capturado, a pilha de raízes do coletor
// continua certa: a recursão que estoura segura objetos em cada quadro, e o
// programa aloca muito depois (com `--gc-stress`, uma coleta a cada
// alocação). A conferência da pilha volta antes de encadear o quadro de
// raízes da função (docs/NATIVO-PROJETOS-REAIS.md, C22).

class No {
  final int valor;
  final No? proximo;
  No(this.valor, this.proximo);
}

int profundidade = 0;

// Cada quadro segura uma referência (`aqui`); a alocação é rara (uma a
// cada mil níveis), para o `--gc-stress` — que coleta antes de cada
// alocação, percorrendo todos os quadros — não ficar quadrático numa pilha
// de 8 MB.
No desce(No? atual) {
  profundidade++;
  final aqui = profundidade % 1000 == 0 ? No(profundidade, atual) : atual;
  final r = desce(aqui);
  return r.valor > 0 ? r : aqui!;
}

int soma(No? n) {
  var s = 0;
  while (n != null) {
    s += n.valor;
    n = n.proximo;
  }
  return s;
}

void main() {
  final antes = No(7, No(35, null));
  for (var rodada = 0; rodada < 3; rodada++) {
    profundidade = 0;
    try {
      desce(null);
    } on StackOverflowError {
      print('rodada $rodada: estourou depois de mais de mil níveis: ${profundidade > 1000}');
    }
    // Muita alocação depois do estouro: listas, mapas, textos.
    var lista = <No>[];
    for (var i = 0; i < 20000; i++) {
      lista.add(No(i, i.isEven ? antes : null));
    }
    final mapa = <String, int>{};
    for (var i = 0; i < 5000; i++) {
      mapa['k$i'] = lista[i].valor;
    }
    print(lista.length + mapa.length + soma(antes) + mapa['k4999']!);
    lista = [];
  }
  print(soma(antes));
}
