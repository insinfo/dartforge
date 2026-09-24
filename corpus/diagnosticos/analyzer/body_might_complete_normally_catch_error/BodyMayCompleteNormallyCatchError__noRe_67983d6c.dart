import 'dart:async';
void f(Future<FutureOr<void>> future) {
  future.catchError((e, st) {});
}
