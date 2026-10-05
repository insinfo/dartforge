// D5 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): o slot lido antes de
// escrito. A referência definida só num ramo, com uma chamada que coleta
// antes da definição; o `phi` com a entrada "indefinida" (o `null` do ramo
// que não define); e a função `async` com `await` antes da definição.
//
// Saída: 1729 140

String aloca(int i) => 'x$i';

int ramo(int i) {
  String? s;
  // Coleta com `s` ainda sem valor.
  final antes = aloca(i);
  if (i.isEven) {
    s = aloca(i + 1);
  }
  // Coleta com `s` definido só num dos ramos.
  final depois = aloca(i + 2);
  return antes.length + depois.length + (s?.length ?? 0);
}

Future<int> assincrona(int i) async {
  await null;
  // Definido depois do `await`: o quadro da continuação já existia.
  final s = aloca(i);
  await null;
  return s.length;
}

Future<void> main() async {
  var total = 0;
  for (var i = 0; i < 200; i++) {
    total += ramo(i);
  }
  var a = 0;
  for (var i = 0; i < 50; i++) {
    a += await assincrona(i);
  }
  print('$total $a');
}
