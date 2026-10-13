import 'dart:async';

void f(Future<(int,)> x) {
  if (x case FutureOr<(int,)> _) {}
}
