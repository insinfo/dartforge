// R-MEM-11: tear-off implícito de `call` como argumento genérico e
// atribuição de função genérica a tipo de função genérico.
class C {
  int call() => 1;
}

T f<T>(T Function() g) => g();
T id<T>(T x) => x;
void main() {
  var a = /*@*/f(C());
  S Function<S>(S) g = /*@*/id;
  print([a, g]);
}
