// R-GEN-08: restrições com FutureOr: `int <: FutureOr<T>` dá T :> int;
// `Future<int> <: FutureOr<T>` dá T :> int.
import 'dart:async';

T f<T>(FutureOr<T> x) => throw 0;
void main() {
  var a = /*@*/f(1);
  var b = /*@*/f(Future.value(1));
  var c = /*@*/f(<int>[]);
  print([a, b, c]);
}
