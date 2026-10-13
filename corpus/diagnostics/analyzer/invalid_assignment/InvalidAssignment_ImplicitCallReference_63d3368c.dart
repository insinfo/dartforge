import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<T Function<T>(T)> f = C();
