// Closures genéricas no backend nativo: funções locais e expressões de função
// com parâmetros de tipo próprios recebem os argumentos de tipo da chamada
// (escritos ou inferidos) e os juntam aos de quem as criou; chamadas
// dinâmicas sem argumentos de tipo usam os limites. Inclui o `typeName<T>`
// do `package_config` (`0 is T`, `'' is T`) que o analyzer usa.
String nomeDoTipo(Object? v) {
  String typeName<T>() {
    if (0 is T) return 'int';
    if ('' is T) return 'string';
    if (const <Object?>[] is T) return 'array';
    return 'object';
  }

  T? checar<T>(Object? x) {
    if (x is T) return x;
    return null;
  }

  return '${typeName<int>()} ${typeName<String>()} ${typeName<List<Object?>>()} '
      '${typeName<bool>()} ${checar<int>(v)} ${checar<String>(v)}';
}

class Caixa<E> {
  final E valor;
  Caixa(this.valor);

  // A closure genérica dentro de um método de classe genérica vê o `E` da
  // classe e o próprio `T`.
  List<String> par<R>(R r) {
    String f<T>(T t) => '$E/$R/$T:${t is E}:${r is T}';
    return [f<int>(1), f<String>('a'), f(valor)];
  }
}

// Dentro de uma função genérica: a tupla da closure é a de fora mais a dela.
List<String> fora<A>(A a) {
  String dentro<B>(B b) {
    String maisDentro<C>() => '$A,$B,$C';
    return maisDentro<double>();
  }

  final lista = <A>[a];
  return [dentro<bool>(true), dentro('x'), '${lista.runtimeType}'];
}

T identidade<T>(T x) => x;

void main() {
  print(nomeDoTipo(3));
  print(nomeDoTipo('a'));

  final f = <T>(Object? x) => x is T;
  print([f<int>(1), f<int>('x'), f<String>('x'), f<num>(2.5)]);

  // Inferidos pelos argumentos e pelo contexto.
  T primeiro<T>(List<T> l) => l.first;
  final int i = primeiro([7, 8]);
  print('$i ${primeiro(<String>['s']).runtimeType}');
  List<T> repetir<T>(T x, int n) => List<T>.filled(n, x);
  print(repetir(1.5, 2).runtimeType);
  print(repetir<Object>('o', 1).runtimeType);

  print(Caixa<int>(4).par<String>('r'));
  print(fora<int>(1));

  // Pelo valor: tear-off genérico e closure chamados com e sem tipos.
  T Function<T>(T) g = identidade;
  print([g<int>(1).runtimeType, g('s').runtimeType]);
  dynamic d = <T extends num>() => T;
  print(d());
  print(d<int>());
  dynamic h = <T>() => T;
  print(h());
  print(h<String>());

  // Cast e literal com o tipo da closure.
  List<T> unir<T>(Object a, Object b) => [a as T, b as T];
  print(unir<int>(1, 2).runtimeType);
  try {
    unir<int>(1, 'x');
  } on TypeError catch (_) {
    print('TypeError');
  }

  // Closure genérica async.
  Future<T> depois<T>(T x) async => x;
  depois<String>('assíncrono').then((v) => print('$v ${v.runtimeType}'));
}
