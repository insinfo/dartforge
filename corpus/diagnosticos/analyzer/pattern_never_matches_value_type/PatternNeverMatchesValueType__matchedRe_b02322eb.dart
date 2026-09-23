import 'dart:async';

void f((int,) x) {
  if (x case FutureOr<(int,)> _) {}
}
