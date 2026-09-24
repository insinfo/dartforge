import 'dart:async';
void f(Null a) => Completer<int>().complete(a);
//                                          ^
// [diag.nullArgumentToNonNullType] 'Completer.complete' shouldn't be called with a 'null' argument for the non-nullable type argument 'int'.
