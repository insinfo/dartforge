// `const []`/`const {}` como valor padrão de um parâmetro cujo tipo usa a
// variável de tipo da classe (`{Iterable<T> m = const []}`, o `Router<T>`
// do angel3): uma constante não depende de `T`, e o contexto vale pelo
// fecho menor — o literal é `const <Never>[]`, subtipo de `Iterable<T>`
// para todo `T`. O nativo fazia `List<dynamic>` e a conferência do
// parâmetro falhava (docs/NATIVO-PROJETOS-REAIS.md, C12).

typedef H = int Function(int);

class R<T> {
  void on(String p, {Iterable<T> m = const []}) {
    print('${m.runtimeType} ${m is Iterable<T>} ${m is List<Never>}');
    aceitar(m);
  }

  void aceitar(Iterable<T> m) => print('aceitou ${m.length}');

  Set<T> s([Set<T> x = const {}]) => x;
  Map<String, T> mp({Map<String, T> x = const {}}) => x;
}

class S extends R<H> {}

void main() {
  R<H>().on('x');
  R<int>().on('y');
  S().on('z');
  print(R<String>().s().runtimeType);
  print(R<String>().mp().runtimeType);
  const vazia = <Never>[];
  print(identical(vazia, const <Never>[]));
}
