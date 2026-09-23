void foo() => Future<int>.value(null);
//                              ^^^^
// [diag.nullArgumentToNonNullType] 'Future.value' shouldn't be called with a 'null' argument for the non-nullable type argument 'int'.
