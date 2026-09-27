// diverge-ddc: as mensagens dos erros de tipo e o runtimeType do mapa são os da web (dart:_rti, LinkedMap)
// V05: contratos além de uma soma — RTI de genéricos, `is`/`as` com
// argumentos de tipo, subtipagem de tipos de função, as mensagens dos
// erros de tipo, a ordem dos efeitos com exceção e `finally`, a ordenação
// inteira (a lista toda) e a preservação dos elementos e da ordem das
// coleções.

class Caixa<T> {
  final T valor;
  Caixa(this.valor);
}

T id<T>(T x) => x;

String qual<T>(T x) => '$T';

class Conta {
  int total = 0;
  void somar(int n) => total += n;
}

int dobro(int x) => x * 2;
num metade(num x) => x / 2;

String erro(void Function() f) {
  try {
    f();
    return 'sem erro';
  } on TypeError catch (e) {
    return 'TypeError: $e';
  } catch (e) {
    return '${e.runtimeType}';
  }
}

final efeitos = <String>[];

int comFinally(bool lancar) {
  try {
    efeitos.add('try');
    if (lancar) throw StateError('x');
    return 1;
  } catch (e) {
    efeitos.add('catch ${e.runtimeType}');
    return 2;
  } finally {
    efeitos.add('finally');
  }
}

int finallySobrepoe() {
  try {
    return 1;
  } finally {
    efeitos.add('sobrepõe');
  }
}

void main() {
  // RTI.
  print(<int>[].runtimeType);
  print(<String, List<int>>{}.runtimeType);
  print(Caixa(1).runtimeType);
  print(Caixa<num>(1).runtimeType);
  print(Caixa<List<Caixa<String>>>([]).runtimeType);
  print(<int?>[null].runtimeType);

  // is/as com argumentos de tipo.
  Object o = <int>[1, 2];
  print([o is List<int>, o is List<num>, o is List<String>, o is Iterable<Object>]);
  Object n = <num>[1];
  print([n is List<int>, n is List<num>]);
  print(Caixa<int>(3) is Caixa<num>);
  print(Caixa<num>(3) is Caixa<int>);
  print(erro(() => o as List<String>));
  print(erro(() => (1 as Object) as String));
  print(erro(() => (null as Object?) as int));

  // Covariância: gravar pelo tipo mais largo.
  List<num> nums = <int>[1];
  print(erro(() => nums.add(1.5)));
  print(nums);

  // Tipos de função.
  Object f = dobro;
  print([f is int Function(int), f is num Function(int), f is int Function(num), f is Object Function(Never)]);
  Object g = metade;
  print([g is num Function(int), g is int Function(num)]);
  int Function(int) instanciada = id;
  print(instanciada(4));
  print(instanciada is int Function(int));
  String Function(double) qualDouble = qual;
  print(qualDouble(1.5));
  print(qualDouble.runtimeType);
  dynamic conta = Conta();
  print(erro(() => conta.somar('dez')));
  conta.somar(10);
  print(conta.total);

  // Erros de tipo de chamadas dinâmicas e de retorno implícito.
  dynamic d = 'texto';
  print(erro(() {
    int x = d;
    print(x);
  }));
  dynamic fd = dobro;
  print(erro(() => fd('um')));

  // Ordem dos efeitos com exceção e finally.
  print(comFinally(false));
  print(comFinally(true));
  print(finallySobrepoe());
  print(efeitos);

  // Ordenação inteira: a lista toda, com repetidos, negativos e grandes.
  final xs = [5, -3, 1 << 50, 0, 5, -3, 1 << 40, -(1 << 40), 7];
  xs.sort();
  print(xs);
  final ys = [...xs]..sort((a, b) => b.compareTo(a));
  print(ys);
  final pares = [(2, 'b'), (1, 'z'), (2, 'a'), (1, 'y')];
  pares.sort((a, b) => a.$1 != b.$1 ? a.$1.compareTo(b.$1) : a.$2.compareTo(b.$2));
  print(pares);

  // Preservação dos elementos e da ordem.
  final s = <int>{3, 1, 2};
  s.add(1);
  s.remove(3);
  s.add(3);
  print(s);
  final m = <String, int>{'c': 1, 'a': 2, 'b': 3};
  m.remove('a');
  m['a'] = 4;
  m['c'] = 5;
  print(m);
  final l = [1, 2, 3, 4, 5, 6]..removeWhere((x) => x.isEven);
  print(l);
  final r = [1, 2, 3, 4, 5, 6]..retainWhere((x) => x > 2);
  print(r);
  print([for (final e in m.entries) '${e.key}=${e.value}']);
}
