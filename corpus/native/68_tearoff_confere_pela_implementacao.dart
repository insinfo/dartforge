// O tear-off de um método é o da implementação que o receptor executa: os
// argumentos da chamada são conferidos pelos parâmetros dela, não pelos do
// membro estático. `Def<E>.hash(Object? e)` implementa `Eq<E>.hash(E e)`;
// tirado de um `Def<Never>` (o `const DefaultEquality<Never>()` padrão do
// `package:collection`), aceita qualquer argumento. Era o `type 'String' is
// not a subtype of type 'Never' of 'e'` do freezed no executor nativo de
// builders (`SetEquality<String>().equals` passa `_elementEquality.hash` ao
// `HashMap`).
import 'dart:collection';

abstract class Eq<E> {
  bool equals(E e1, E e2);
  int hash(E e);
  bool isValidKey(Object? o);
}

class Def<E> implements Eq<E> {
  const Def();
  @override
  bool equals(Object? e1, Object? e2) => e1 == e2;
  @override
  int hash(Object? e) => e.hashCode & 7;
  @override
  bool isValidKey(Object? o) => true;
}

class Estrito<E> implements Eq<E> {
  const Estrito();
  @override
  bool equals(E e1, E e2) => e1 == e2;
  @override
  int hash(E e) => 2;
  @override
  bool isValidKey(Object? o) => o is E;
}

class SetEq<E> {
  final Eq<E> _el;
  const SetEq([this._el = const Def<Never>()]);

  bool equals(Set<E> a, Set<E> b) {
    final contas = HashMap<E, int>(equals: _el.equals, hashCode: _el.hash, isValidKey: _el.isValidKey);
    for (final e in a) {
      contas[e] = (contas[e] ?? 0) + 1;
    }
    for (final e in b) {
      final c = contas[e];
      if (c == null || c == 0) return false;
      contas[e] = c - 1;
    }
    return a.length == b.length;
  }
}

abstract class Forma {
  String rotulo(int n, {String prefixo = '?'});
  void anotar(List<String> saida, String s);
  T primeiro<T>(List<T> l);
  String _privado(int x);
  String privado(int x) => _privado(x);
}

class Circulo extends Forma {
  @override
  String rotulo(num n, {Object? prefixo = '?'}) => 'circulo $prefixo$n';
  @override
  void anotar(List<Object?> saida, Object? s) => saida.add('c:$s');
  @override
  T primeiro<T>(List<T> l) => l.first;
  @override
  String _privado(num x) => 'c$x';
}

class Quadrado extends Forma {
  @override
  String rotulo(int n, {String prefixo = '!'}) => 'quadrado $prefixo$n';
  @override
  void anotar(List<String> saida, String s) => saida.add('q:$s');
  @override
  T primeiro<T>(List<T> l) => l.last;
  @override
  String _privado(int x) => 'q$x';
}

void tenta(String rotulo, Object? Function() f) {
  try {
    print('$rotulo: ${f()}');
  } on TypeError {
    print('$rotulo: TypeError');
  } on NoSuchMethodError {
    print('$rotulo: NoSuchMethodError');
  }
}

void main() {
  // O caso do freezed.
  print(const SetEq<String>().equals({'a', 'b'}, {'b', 'a'}));
  print(const SetEq<String>().equals({'a'}, {'c'}));
  print(const SetEq<String>(Estrito<String>()).equals({'x'}, {'x'}));

  final Eq<String> d = const Def<Never>();
  final Eq<String> s = const Estrito<Never>();
  final dh = d.hash, sh = s.hash, de = d.equals;
  tenta('d.hash', () => dh('x'));
  tenta('d.equals', () => de('x', 'x'));
  tenta('s.hash', () => sh('x'));
  final Function fd = dh, fs = sh;
  tenta('fd(7)', () => fd(7));
  tenta('fs(7)', () => fs(7));
  tenta('fd()', () => fd());

  // Sobrescritas que alargam os parâmetros, nomeados e posicionais.
  for (final Forma f in [Circulo(), Quadrado()]) {
    final r = f.rotulo;
    final a = f.anotar;
    final p = f.primeiro;
    final Function rd = r, ad = a;
    final saida = <String>[];
    tenta('rotulo', () => r(3));
    tenta('rotulo nomeado', () => r(4, prefixo: '#'));
    tenta('rotulo 1.5', () => rd(1.5));
    tenta('rotulo prefixo 9', () => rd(2, prefixo: 9));
    tenta('anotar', () {
      a(saida, 'v');
      return saida;
    });
    tenta('anotar dinamico', () => ad(<Object?>[], 5));
    tenta('primeiro', () => p<int>([1, 2, 3]));
    final String Function(List<String>) pi = f.primeiro;
    tenta('primeiro instanciado', () => pi(['a', 'b']));
    final pv = f.privado;
    tenta('privado', () => pv(8));
  }

  // Membro do SDK com a covariância das classes genéricas.
  List<Object> lista = <int>[1];
  final add = lista.add;
  tenta('add 2', () {
    add(2);
    return lista;
  });
  tenta('add x', () {
    add('x');
    return lista;
  });
}
