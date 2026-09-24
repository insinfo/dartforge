import 'dart:async';

final class A {}

void f(FutureOr<A> x) {
  if (x case A _) {}
}
