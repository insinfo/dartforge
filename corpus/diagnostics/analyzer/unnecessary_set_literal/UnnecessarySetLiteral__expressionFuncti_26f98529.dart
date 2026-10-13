import 'dart:async';

void g(FutureOr Function() fun) {}

void f() {
  g(() async => {1});
}
