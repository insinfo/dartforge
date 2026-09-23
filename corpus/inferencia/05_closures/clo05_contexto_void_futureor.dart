// R-CLO-05: contexto de retorno void; contexto FutureOr<Function>/Function.
import 'dart:async';

void main() {
  void Function() a = /*@*/() => 1;
  FutureOr<int> Function() b = /*@*/() => 1;
  FutureOr<int Function(int)> c = /*@*/(int x) => x;
  int Function(int)? d = /*@*/(x) => x;
  void Function(int) e = /*@*/(x) {};
  print([a, b, c, d, e]);
}
