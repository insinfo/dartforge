import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<Function> f = C();
