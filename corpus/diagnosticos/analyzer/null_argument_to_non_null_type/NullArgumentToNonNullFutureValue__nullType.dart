void foo(Null a) => Future<int>.value(a);
//                                    ^
// [diag.nullArgumentToNonNullType] 'Future.value' shouldn't be called with a 'null' argument for the non-nullable type argument 'int'.
