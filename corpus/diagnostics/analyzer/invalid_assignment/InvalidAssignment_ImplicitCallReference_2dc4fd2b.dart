import 'dart:async';
class C {
  T call<T>(T t) => t;
}

FutureOr<int Function(int)> f = C();
