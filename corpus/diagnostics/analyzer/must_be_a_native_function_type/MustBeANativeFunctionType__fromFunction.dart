import 'dart:ffi';
int f(int i) => i * 2;
class C<T extends Function> {
  void g() {
    Pointer.fromFunction<T>(f);
//                       ^
// [diag.mustBeANativeFunctionType] The type 'T' given to 'fromFunction' must be a valid 'dart:ffi' native function type.
  }
}
