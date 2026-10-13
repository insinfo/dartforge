// R-CLO-07: flatten e o tipo de `await`.
import 'dart:async';

Future<void> f<T extends Future<int>>(
    FutureOr<int> a, Future<int>? b, T c, Future<Future<int>> d, int e) async {
  var p = /*@*/await a;
  var q = /*@*/await b;
  var r = /*@*/await c;
  var s = /*@*/await d;
  var t = /*@*/await e;
  print([p, q, r, s, t]);
}

void main() {}
