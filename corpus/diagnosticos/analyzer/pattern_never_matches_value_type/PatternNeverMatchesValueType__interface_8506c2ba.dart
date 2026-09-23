import 'dart:async';

final class A {}

void f(A x) {
  if (x case FutureOr<A> _) {}
}
