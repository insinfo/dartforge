void f(Future<int?> future) {
  future.catchError((e, st) {});
//                          ^
// [diag.bodyMightCompleteNormallyCatchError] This 'onError' handler must return a value assignable to 'int?', but ends without returning a value.
}
