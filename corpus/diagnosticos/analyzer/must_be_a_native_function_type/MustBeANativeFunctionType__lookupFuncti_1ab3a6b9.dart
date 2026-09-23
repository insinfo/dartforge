import 'dart:ffi';
typedef S = Void Function(Pointer);
typedef F = void Function(Pointer);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
