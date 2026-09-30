// O `P<i>` avaliado com cache no ponto de uso (`llvm/mod.rs`,
// `emitir_rti_avaliar_em_cache`) e a memória do `rti_definir` das coleções
// do runtime (`tipos.rs`). O mesmo ponto de uso vê receptores com outros
// argumentos de tipo, outras classes (método herdado), subclasses que fixam
// o argumento, objetos sem tipo gravado (argumentos `dynamic`) e objetos
// da mesma classe com metadados diferentes; e as coleções literais de um método genérico
// (`<E>[]`, `<K, V>{}`, `<E>{}`) com vários `E`, conferidas por `is`,
// `runtimeType` e pela covariância do `add`. Também dentro de
// `Isolate.run` (cada isolado tem a sua área e o seu universo de tipos) e
// com muitos objetos, para a coleta. A saída tem de ser a da VM.
import 'dart:isolate';

class Caixa<T> {
  T valor;
  Caixa(this.valor);
  // `T` no ponto de uso: `P0` avaliado a partir de `this`.
  bool aceita(Object? x) => x is T;
  List<T> lista() => <T>[valor];
  Map<String, T> mapa() => <String, T>{'v': valor};
  Set<T> conjunto() => <T>{valor};
  String get tipo => '$T';
  Type get tipoRt => T;
}

class Par<A, B> extends Caixa<A> {
  B outro;
  Par(A a, this.outro) : super(a);
  bool aceitaB(Object? x) => x is B;
  List<B> listaB() => <B>[outro];
  String get tipos => '$A/$B';
}

class CaixaDeInt extends Caixa<int> {
  CaixaDeInt(int v) : super(v);
}

class CaixaCrua extends Caixa {
  CaixaCrua(Object? v) : super(v);
}

class Pilha<E> {
  final List<E> itens = <E>[];
  void empilhar(E e) => itens.add(e);
  E desempilhar() => itens.removeLast();
  bool contem(Object? x) => x is E && itens.contains(x);
  List<List<E>> aninhada() => <List<E>>[<E>[...itens]];
}

String descrever(Caixa<Object?> c, Object? x) =>
    '${c.tipo} aceita ${x.runtimeType}: ${c.aceita(x)}; lista ${c.lista().runtimeType}; '
    'mapa ${c.mapa().runtimeType}; conjunto ${c.conjunto().runtimeType}; Type ${c.tipoRt}';

List<String> rodada(int semente) {
  final saida = <String>[];
  final caixas = <Caixa<Object?>>[
    Caixa<int>(semente),
    Caixa<String>('s$semente'),
    Caixa<num>(semente + 0.5),
    Caixa<int?>(null),
    Caixa<List<int>>([semente]),
    CaixaDeInt(semente * 2),
    CaixaCrua('cru'),
    Par<int, String>(semente, 'b'),
    Par<String, int>('a', semente),
    Caixa<Object>(Object()),
    Caixa(semente), // inferido: Caixa<int>
  ];
  final valores = <Object?>[1, 'x', 2.5, null, <int>[1], <String>['a'], true];
  // O mesmo ponto de uso (`aceita`, `lista`…) vê todas as combinações,
  // em duas ordens: a entrada do cache muda de chave a cada receptor.
  for (final c in caixas) {
    for (final v in valores) {
      saida.add(descrever(c, v));
    }
  }
  for (final c in caixas.reversed) {
    saida.add('${c.tipo} ${c.aceita(semente)} ${c.aceita('s')} ${c.aceita(null)}');
  }
  for (final c in caixas.whereType<Par<Object?, Object?>>()) {
    saida.add('${c.tipos} ${c.aceitaB(1)} ${c.aceitaB('b')} ${c.listaB().runtimeType}');
  }
  // Covariância pela coleção criada no método genérico.
  final l = Caixa<int>(1).lista();
  try {
    (l as List<Object?>).add('não int');
    saida.add('add aceitou');
  } on TypeError catch (e) {
    saida.add('TypeError: ${e.runtimeType}');
  }
  final m = Caixa<String>('v').mapa();
  try {
    (m as Map<String, Object?>)['k'] = 3;
    saida.add('[]= aceitou');
  } on TypeError catch (e) {
    saida.add('TypeError: ${e.runtimeType}');
  }
  saida.add('${l is List<int>} ${l is List<num>} ${l is List<String>} ${m is Map<String, String>}');
  // Pilhas com vários `E`: o `<E>[]` do campo e o aninhado.
  final pi = Pilha<int>()..empilhar(semente)..empilhar(2);
  final ps = Pilha<String>()..empilhar('a');
  final pd = Pilha<double>()..empilhar(1.5);
  final pn = Pilha();
  pn.empilhar('qualquer');
  for (final p in <Pilha<Object?>>[pi, ps, pd, pn]) {
    saida.add('${p.itens.runtimeType} ${p.aninhada().runtimeType} ${p.contem(2)} ${p.contem('a')} '
        '${p.itens is List<int>} ${p.aninhada() is List<List<num>>}');
  }
  saida.add('${pi.desempilhar()} ${ps.desempilhar()}');
  return saida;
}

void main() async {
  final a = rodada(7);
  a.forEach(print);
  // Muitos objetos: a coleta passa com o cache cheio.
  var aceitos = 0;
  for (var i = 0; i < 20000; i++) {
    final c = i.isEven ? Caixa<int>(i) : Caixa<String>('$i');
    if (c.aceita(i)) aceitos++;
    if (c.lista() is List<int>) aceitos++;
  }
  print('aceitos $aceitos');
  // Outro isolado: outra área e outro universo; o resultado é o mesmo.
  final b = await Isolate.run(() => rodada(7));
  print('isolado igual: ${a.join('|') == b.join('|')}');
  final c = await Isolate.run(() => rodada(8));
  print(c.first);
  print(c.last);
}
