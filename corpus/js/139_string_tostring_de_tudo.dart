// toString e interpolação de todo tipo: num, bool, null, String, coleções, record, enum, classes, Symbol, Type, Duration, DateTime, Uri.
enum Naipe { copas, ouros }

enum Moeda {
  real('R\$'),
  euro('€');

  final String simbolo;
  const Moeda(this.simbolo);
  @override
  String toString() => 'Moeda.$name($simbolo)';
}

class SemToString {
  final int x = 1;
}

class ComToString {
  final int x;
  ComToString(this.x);
  @override
  String toString() => 'ComToString#$x';
}

class Generica<T> {
  final T v;
  Generica(this.v);
}

class Aninhada {
  final ComToString interno;
  final List<Object?> lista;
  Aninhada(this.interno, this.lista);
  @override
  String toString() => 'Aninhada($interno, $lista)';
}

mixin M {}

class ComMixin with M {}

void main() {
  void mostra(Object? o) => print('${o.toString()} | $o | ${o}');
  mostra(42);
  mostra(-7);
  mostra(0);
  mostra(2.5);
  mostra(-0.125);
  mostra(1 / 3);
  mostra(double.nan);
  mostra(double.infinity);
  mostra(true);
  mostra(false);
  mostra(null);
  mostra('texto');
  mostra('');
  mostra('com "aspas" e \'simples\'');
  mostra('multi\nlinha');
  mostra([1, 2, 3]);
  mostra(<int>[]);
  mostra([[1], [2, [3]]]);
  mostra(['a', null, 2.5, true]);
  mostra({'a': 1, 'b': [2]});
  mostra(<String, int>{});
  mostra({1: {2: {3: 4}}});
  mostra({1, 2, 3});
  mostra(<int>{});
  mostra({[1], {2}});
  mostra((1, 2));
  mostra((a: 1, b: 'x'));
  mostra((1, nome: 'n', 2.5));
  mostra(((1, 2), (3,)));
  mostra(Naipe.copas);
  mostra(Naipe.values);
  mostra(Moeda.real);
  mostra(Moeda.values);
  mostra(Naipe.ouros.name);
  mostra(SemToString());
  mostra(ComToString(9));
  mostra(Aninhada(ComToString(1), [ComToString(2), Naipe.copas, (3, 4), null]));
  mostra([ComToString(1), ComToString(2)]);
  mostra({ComToString(1): ComToString(2)});
  mostra({'k': SemToString()}.toString().contains("Instance of 'SemToString'"));
  mostra(Generica<int>(1).toString() == "Instance of 'Generica<int>'");
  mostra(ComMixin().toString() == "Instance of 'ComMixin'");
  mostra(Object().toString());
  mostra(#foo);
  mostra(#foo.bar);
  mostra(#foo == Symbol('foo'));
  mostra(Symbol('x y'));
  mostra(SemToString);
  mostra(ComToString);
  mostra(Naipe);
  mostra(Aninhada);
  mostra(ComMixin);
  mostra(SemToString().runtimeType);
  mostra(Generica<String>('a').runtimeType);
  mostra(Generica<Naipe>(Naipe.copas).runtimeType);
  mostra(Generica<ComToString>(ComToString(1)).runtimeType);
  mostra(Naipe.copas.runtimeType);
  mostra((1, 2).runtimeType == (3, 4).runtimeType);
  mostra(Duration.zero);
  mostra(Duration(hours: 1, minutes: 2, seconds: 3, milliseconds: 4, microseconds: 5));
  mostra(Duration(days: 2));
  mostra(-Duration(seconds: 90));
  mostra(Duration(hours: 100));
  mostra(DateTime.utc(2024, 2, 29));
  mostra(DateTime.utc(2024, 2, 29, 23, 59, 59, 999));
  mostra(DateTime.utc(1970));
  mostra(DateTime.utc(2024, 2, 29).toIso8601String());
  mostra(Uri.parse('https://h.com:8/p/q?a=1#f'));
  mostra(Uri.parse('mailto:x@y.z'));
  mostra(Uri(scheme: 'http', host: 'h', path: '/a b'));
  mostra(Uri.parse('http://h.com/').pathSegments);
  mostra(StringBuffer('buf')..write('!'));
  mostra(StringBuffer());
  mostra('abc'.runes);
  mostra('abc'.codeUnits);
  mostra([1, 2, 3].map((e) => e * 2));
  mostra([1, 2, 3].where((e) => e.isOdd));
  mostra([1, 2, 3].reversed);
  mostra([1, 2, 3].skip(1));
  mostra([1, 2, 3].take(2));
  mostra(Iterable.generate(3));
  mostra(Iterable.empty());
  mostra({'a': 1}.keys);
  mostra({'a': 1}.values);
  mostra({'a': 1}.entries);
  mostra(MapEntry('k', 'v'));
  mostra(MapEntry(1, [2]));
  mostra([1, 2].asMap());
  mostra(List.filled(3, 'x'));
  mostra(List.generate(4, (i) => i * i));
  mostra(List<int?>.filled(2, null));
  mostra(List.unmodifiable([1]));
  mostra(Set.unmodifiable({1}));
  mostra(Map.unmodifiable({1: 2}));
  mostra(Exception('msg'));
  mostra(FormatException('fmt'));
  mostra(FormatException('fmt', 'fonte'));
  mostra(FormatException('fmt', 'fonte', 2));
  mostra(StateError('estado'));
  mostra(ArgumentError('arg'));
  mostra(ArgumentError.value(5, 'nome', 'msg'));
  mostra(ArgumentError.notNull('p'));
  mostra(RangeError.value(5, 'i', 'fora'));
  mostra(UnsupportedError('nao'));
  mostra(UnimplementedError('ainda'));
  mostra(ConcurrentModificationError());
  mostra(AssertionError('assert'));
  mostra(Exception());
  mostra(Exception(null));
  mostra(Exception(42));
  mostra(Exception([1, 2]));
  mostra(Exception(ComToString(3)));
  mostra(1.toString() + 2.toString());
  mostra('${1}' + '${2}');
  mostra([1, 'a', null, true, 2.5, [1], {1: 2}, {3}, (4,), Naipe.copas, ComToString(5)]);
  mostra({'lista': [1, {'m': (1, 2)}], 'enum': Naipe.ouros, 'nulo': null});
  mostra(['a\nb', 'c\td']);
  mostra(['x', ['y', ['z']]].toString().length);
  mostra([1, 2, 3].toString().split(', '));
  mostra({1: 'a'}.toString().length);
  mostra(true.toString().length + null.toString().length);
  mostra(Object);
  mostra(int);
  mostra(String);
  mostra(bool);
  mostra(Null);
  mostra(Naipe.copas.toString() == 'Naipe.copas');
  mostra((#a).toString().length);
}
