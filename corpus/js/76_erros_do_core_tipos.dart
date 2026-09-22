// erros do core: só `e is X` para RangeError, StateError, ArgumentError, FormatException, TypeError, NoSuchMethodError, etc.
class Vazia {}

void main() {
  // RangeError: índice de lista
  try {
    [1, 2, 3][5];
  } catch (e) {
    print('índice: ${e is RangeError} ${e is ArgumentError} ${e is Error}');
  }
  try {
    [1][-1];
  } catch (e) {
    print('índice negativo: ${e is RangeError}');
  }
  try {
    'abc'[10];
  } catch (e) {
    print('índice de string: ${e is RangeError}');
  }
  try {
    'abc'.substring(2, 1);
  } catch (e) {
    print('substring: ${e is RangeError}');
  }
  try {
    throw RangeError.range(15, 0, 10, 'valor');
  } catch (e) {
    print('RangeError.range: ${e is RangeError} ${(e as RangeError).name} ${e.invalidValue} ${e.start} ${e.end}');
  }
  try {
    [1, 2].sublist(1, 5);
  } catch (e) {
    print('sublist: ${e is RangeError}');
  }
  try {
    [1, 2].removeAt(2);
  } catch (e) {
    print('removeAt: ${e is RangeError}');
  }

  // StateError
  try {
    <int>[].first;
  } catch (e) {
    print('first vazio: ${e is StateError}');
  }
  try {
    <int>[].last;
  } catch (e) {
    print('last vazio: ${e is StateError}');
  }
  try {
    [1, 2].single;
  } catch (e) {
    print('single: ${e is StateError}');
  }
  try {
    [1, 2].firstWhere((x) => x > 5);
  } catch (e) {
    print('firstWhere: ${e is StateError}');
  }
  try {
    <int>[].reduce((a, b) => a + b);
  } catch (e) {
    print('reduce: ${e is StateError}');
  }

  // ArgumentError lançado por mim
  try {
    throw ArgumentError.value(-1, 'idade', 'deve ser positiva');
  } catch (e) {
    final a = e as ArgumentError;
    print('ArgumentError.value: ${a.name} ${a.invalidValue} ${a.message}');
  }
  try {
    throw ArgumentError('simples');
  } catch (e) {
    print('ArgumentError: ${(e as ArgumentError).message} ${e.name}');
  }
  try {
    throw ArgumentError.notNull('param');
  } catch (e) {
    print('notNull: ${(e as ArgumentError).name} ${e.message}');
  }
  try {
    <String, int>{'a': 1}.update('b', (v) => v);
  } catch (e) {
    print('Map.update: ${e is ArgumentError}');
  }

  // FormatException
  try {
    int.parse('abc');
  } catch (e) {
    print('int.parse: ${e is FormatException} ${e is Exception}');
  }
  try {
    double.parse('x1');
  } catch (e) {
    print('double.parse: ${e is FormatException}');
  }
  try {
    int.parse('');
  } catch (e) {
    print('int.parse vazio: ${e is FormatException}');
  }
  print(int.tryParse('abc'));
  print(int.tryParse('42'));
  try {
    throw FormatException('minha', 'fonte', 2);
  } catch (e) {
    final f = e as FormatException;
    print('FormatException: ${f.message} ${f.source} ${f.offset}');
  }

  // TypeError via as errado com dynamic
  dynamic d = 'texto';
  try {
    d as int;
  } catch (e) {
    print('as errado: ${e is TypeError} ${e is Error}');
  }
  try {
    int x = d;
    print(x);
  } catch (e) {
    print('atribuição dinâmica: ${e is TypeError}');
  }
  try {
    final List<int> l = [];
    l.add(d);
  } catch (e) {
    print('add dinâmico: ${e is TypeError}');
  }
  Object o = 1;
  try {
    o as String;
  } catch (e) {
    print('as em Object: ${e is TypeError}');
  }

  // NoSuchMethodError via dynamic
  try {
    d.metodoInexistente();
  } catch (e) {
    print('método inexistente: ${e is NoSuchMethodError}');
  }
  try {
    d.propriedade;
  } catch (e) {
    print('getter inexistente: ${e is NoSuchMethodError}');
  }
  dynamic v = Vazia();
  try {
    v.nada = 1;
  } catch (e) {
    print('setter inexistente: ${e is NoSuchMethodError}');
  }
  try {
    v(1);
  } catch (e) {
    print('chamar não-função: ${e is NoSuchMethodError}');
  }
  dynamic nulo;
  try {
    nulo.foo();
  } catch (e) {
    print('método em null: ${e is NoSuchMethodError}');
  }

  // UnsupportedError
  try {
    List.unmodifiable([1]).add(2);
  } catch (e) {
    print('unmodifiable: ${e is UnsupportedError}');
  }
  try {
    const [1].add(2);
  } catch (e) {
    print('const list add: ${e is UnsupportedError}');
  }
  try {
    const {'a': 1}['b'] = 2;
  } catch (e) {
    print('const map []=: ${e is UnsupportedError}');
  }
  try {
    List.filled(1, 0).add(1);
  } catch (e) {
    print('filled add: ${e is UnsupportedError}');
  }

  // ConcurrentModificationError
  try {
    final m = {'a': 1, 'b': 2};
    for (final k in m.keys) {
      m.remove(k);
    }
  } catch (e) {
    print('modificar map em for-in: ${e is ConcurrentModificationError}');
  }
  try {
    final s = {1, 2};
    for (final x in s) {
      s.add(x + 10);
    }
  } catch (e) {
    print('modificar set em for-in: ${e is ConcurrentModificationError}');
  }

  // UnimplementedError
  try {
    throw UnimplementedError('ainda não');
  } catch (e) {
    print('UnimplementedError: ${e is UnimplementedError} ${e is UnsupportedError} ${(e as UnimplementedError).message}');
  }

  // divisão inteira por zero: o tipo da exceção difere entre VM e web
  try {
    print(1 ~/ 0);
  } catch (e) {
    print('~/ 0 lançou');
  }

  // hierarquia: todos são Error, exceto FormatException
  print(RangeError('r') is Error);
  print(StateError('s') is Error);
  print(ArgumentError('a') is Error);
  print(UnsupportedError('u') is Error);
  print(FormatException('f') is Error);
  print(FormatException('f') is Exception);
  print(RangeError('r') is ArgumentError);
  print(UnimplementedError('u') is UnsupportedError);
  print('fim');
}
