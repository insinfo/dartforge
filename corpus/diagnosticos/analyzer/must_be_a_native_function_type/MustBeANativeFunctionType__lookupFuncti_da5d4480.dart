import 'dart:ffi';
typedef S = Void Function(Pointer<NativeFunction<Int8 Function()>>);
typedef F = void Function(Pointer<NativeFunction<Int8 Function()>>);
void f(DynamicLibrary lib) {
  lib.lookupFunction<S, F>('g');
}
