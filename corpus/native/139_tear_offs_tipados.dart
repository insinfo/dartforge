// Tear-offs de funções de topo e estáticas chamados pelo tipo de função
// (corpo tipado `$tt`, sem a entrada uniforme), pela entrada uniforme
// (`dynamic`, `Function.apply`) e comparados por `identical`. A saída tem de
// ser a da VM, inclusive o `TypeError` da chamada dinâmica com tipo errado.

int dobro(int x) => x * 2;
double metade(double x) => x / 2;
bool nega(bool b) => !b;
String junta(String a, int n) => a * n;
void anota(int x) => log.add('anota $x');
int? talvez(int? x) => x == null ? null : x + 1;
num soma(num a, num b) => a + b;
Future<int> depois(int x) async => x + 1;
int lanca(int x) => x > 2 ? throw StateError('grande: $x') : x;

final log = <String>[];

class Util {
  static int triplo(int x) => x * 3;
}

int aplica(int Function(int) f, int v) => f(v);
double aplicaD(double Function(double) f, double v) => f(v);
Object? aplicaO(Object? Function(int) f, int v) => f(v);

void main() async {
  // Chamadas pelo tipo de função.
  print(aplica(dobro, 21));
  print(aplica(Util.triplo, 5));
  print(aplicaD(metade, 3.0));
  final bool Function(bool) n = nega;
  print(n(true));
  final String Function(String, int) j = junta;
  print(j('ab', 3));
  final void Function(int) a = anota;
  a(7);
  print(log);
  final int? Function(int?) t = talvez;
  print('${t(null)} ${t(4)}');
  final num Function(num, num) s = soma;
  print('${s(1, 2.5)} ${s(2, 3)}');
  final Future<int> Function(int) d = depois;
  print(await d(9));
  // Um supertipo do tipo da função: outra representação do retorno.
  print(aplicaO(dobro, 4));
  final Object Function(int) o = Util.triplo;
  print(o(2));
  // Identidade do tear-off canônico, antes e depois das chamadas.
  print(identical(dobro, dobro));
  final int Function(int) x1 = dobro, x2 = dobro;
  print(identical(x1, x2));
  print(x1 == x2);
  // Pela entrada uniforme: `dynamic` e `Function.apply`.
  final dynamic dyn = dobro;
  print(dyn(8));
  print(Function.apply(dobro, [10]));
  print(Function.apply(junta, ['x', 2]));
  try {
    dyn('texto');
  } on TypeError catch (e) {
    print('TypeError: ${e.runtimeType}');
  }
  try {
    dyn(1, 2);
  } on NoSuchMethodError {
    print('NoSuchMethodError');
  }
  // Exceção que sai do corpo tipado.
  final int Function(int) l = lanca;
  for (final v in [1, 2, 3]) {
    try {
      print(l(v));
    } on StateError catch (e) {
      print(e.message);
    }
  }
  // Em laço, como no despacho por tabela de funções.
  final fs = <int Function(int)>[dobro, Util.triplo, (v) => v + 100];
  var acc = 0;
  for (var i = 0; i < 1000; i++) {
    acc += fs[i % 3](i);
  }
  print(acc);
  print(dobro.runtimeType);
  print(dobro is int Function(int));
  print(dobro is Object Function(Never));
}
