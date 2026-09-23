import 'dart:async';
void f(Completer<int> c) {
  c..complete();
//   ^^^^^^^^^^
// [diag.nullArgumentToNonNullType] 'Completer.complete' shouldn't be called with a 'null' argument for the non-nullable type argument 'int'.
}
