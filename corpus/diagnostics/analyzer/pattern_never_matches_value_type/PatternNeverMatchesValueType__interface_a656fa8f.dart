import 'dart:async';

final class A {}
class B {}

void f(FutureOr<A> x) {
  if (x case B _) {}
}
