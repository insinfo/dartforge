import 'dart:ffi';
void f(int i) => i * 2;
void g() {
  NativeCallable<Void Function(Double)>.listener(f);
//                                               ^
// [diag.mustBeASubtype] The type 'void Function(int)' must be a subtype of 'Void Function(Double)' for 'NativeCallable'.
}
