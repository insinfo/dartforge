// Extension methods: em String/int/List<T>, métodos/getters/setters/operadores, nomeada e anônima, chamada explícita Ext(x).m(), on int?, genérica, static, conflito por nome, this.
extension StringExt on String {
  String get invertida => split('').reversed.join();
  bool get ehPalindromo => this == invertida;
  String repete(int n) => List.filled(n, this).join();
  String operator -(String outro) => replaceAll(outro, '');
  static String juntar(List<String> xs) => xs.join('+');
  static const separador = '|';
}

extension IntExt on int {
  int get quadrado => this * this;
  bool get ehPrimo {
    if (this < 2) return false;
    for (var i = 2; i * i <= this; i++) {
      if (this % i == 0) return false;
    }
    return true;
  }

  Iterable<int> ate(int fim) sync* {
    for (var i = this; i <= fim; i++) {
      yield i;
    }
  }

  int vezes(int Function(int) f) {
    var acc = 0;
    for (var i = 0; i < this; i++) {
      acc += f(i);
    }
    return acc;
  }
}

extension ListExt<T> on List<T> {
  T? get segundo => length > 1 ? this[1] : null;
  List<T> get semPrimeiro => skip(1).toList();
  set primeiro(T v) => this[0] = v;
  List<List<T>> pares() {
    final r = <List<T>>[];
    for (var i = 0; i + 1 < length; i += 2) {
      r.add([this[i], this[i + 1]]);
    }
    return r;
  }

  List<T> operator *(int n) => [for (var i = 0; i < n; i++) ...this];
}

extension on int? {
  int get ouZero => this ?? 0;
  bool get ehNulo => this == null;
}

extension on double {
  String get fixo => toStringAsFixed(2);
}

extension A on String {
  String get rotulo => 'A:$this';
  String get soA => 'só em A';
}

extension B on String {
  String get rotulo => 'B:$this';
  String get soB => 'só em B';
}

class Caixa {
  final int v;
  Caixa(this.v);
}

extension CaixaExt on Caixa {
  Caixa mais(int n) => Caixa(v + n);
  Caixa get eu => this;
  String toStringExt() => 'Caixa($v)';
  int operator [](int i) => v * i;
  Caixa operator -() => Caixa(-v);
}

void main() {
  print('abc'.invertida);
  print('arara'.ehPalindromo);
  print('abc'.ehPalindromo);
  print('ab'.repete(3));
  print('banana' - 'a');
  print(StringExt.juntar(['a', 'b', 'c']));
  print(StringExt.separador);
  print(StringExt('explícito').invertida);
  print('--');
  print(7.quadrado);
  print([1, 2, 7, 9, 11].map((n) => n.ehPrimo).toList());
  print(3.ate(6).toList());
  print(4.vezes((i) => i * 2));
  print((2 + 3).quadrado);
  print(IntExt(5).quadrado);
  print('--');
  final xs = [10, 20, 30, 40, 50];
  print(xs.segundo);
  print(<int>[].segundo);
  print(xs.semPrimeiro);
  xs.primeiro = 99;
  print(xs);
  print(xs.pares());
  print(['a', 'b'] * 3);
  print(ListExt<int>(xs).segundo);
  print(['s'].segundo == null);
  print('--');
  int? n;
  print(n.ouZero);
  print(n.ehNulo);
  n = 5;
  print(n.ouZero);
  print(n.ehNulo);
  print(7.ouZero);
  print(3.14159.fixo);
  print('--');
  print(A('x').rotulo);
  print(B('x').rotulo);
  print('x'.soA);
  print('x'.soB);
  print('--');
  final c = Caixa(1);
  print(c.mais(2).mais(3).toStringExt());
  print(identical(c.eu, c));
  print(c[7]);
  print((-c).v);
  print(CaixaExt(c).mais(10).v);
  final dynamic d = 'dinâmico';
  print((d as String).invertida);
  print('fim');
}
