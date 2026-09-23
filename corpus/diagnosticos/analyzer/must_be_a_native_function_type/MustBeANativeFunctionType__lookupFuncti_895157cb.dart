import 'dart:ffi';
typedef S = Pointer<Void> Function(Pointer<Void>);
typedef F = Pointer<Void> Function(Pointer<Void>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
