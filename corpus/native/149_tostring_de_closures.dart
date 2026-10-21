// O toString de uma closure é o da VM (Closure::ToCString): a assinatura e,
// num tear-off de função, método, estático ou construtor, `from Function
// 'nome':[ static].`; dentro de record, lista, mapa e interpolação. Antes
// saía `Instance of '_Closure'`.
class C {
  int m(int x) => x;
  static String s() => '';
  C();
  C.named(int a);
  T g<T>(T x) => x;
}
T id<T>(T x) => x;
extension E on int { int twice() => this * 2; }
void main() {
  int local(String s) => 1;
  print(local);
  print(C().m);
  print(C.s);
  print(C.new);
  print(C.named);
  print(id);
  int Function(int) i = id;
  print(i);
  print(id<String>);
  print(C().g);
  print(1.twice);
  print(print);
  print(<T>(T x) => x);
  print(([int? a, int b = 2]) => 0);
  print(({required int a, String? b}) => 0);
  print(() async {});
  print([1].map);
  dentro();
}

void dentro() {
  var f = (a, b) => 1;
  print((f, 42));
  print([f]);
  print({'k': f});
  print('$f');
}
