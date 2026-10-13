import 'dart:async';

void f(FutureOr<(int,)> x) {
  if (x case (int,) _) {}
}
