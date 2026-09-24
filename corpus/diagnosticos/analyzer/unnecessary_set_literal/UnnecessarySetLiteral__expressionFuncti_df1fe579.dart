import 'dart:async';

void g(FutureOr<Object> Function() fun) {}

void f() {
  g(() async => {1});
}
