// `const [...]` como argumento de um construtor de fábrica genérico: o
// contexto `Iterable<T>` traz a variável da fábrica sendo inferida, que não
// é variável livre do literal (fechá-la dava `Stream<Never>`; o
// `distinct_test` do rxdart; docs/NATIVO-PROJETOS-REAIS.md, C12).

class Caixa<T> {
  final List<T> itens;
  Caixa._(this.itens);
  factory Caixa.de(Iterable<T> xs) => Caixa._(xs.toList());
}

Future<void> main() async {
  const esperado = 1;
  final s = Stream.fromIterable(const [esperado, esperado]);
  print(s.runtimeType);
  print(await s.distinct().toList());
  print(await Stream.fromIterable(const ['h', 'i']).join('+'));
  final c = Caixa.de(const [1, 2]);
  print(c.itens.runtimeType);
  print(Caixa.de(const <String>[]).itens.runtimeType);
}
