import 'dart:ffi';
int f(int x) => x;
void main() {
  Pointer.fromFunction<Unresolved Function()>(f, 0);
//                     ^^^^^^^^^^
// [diag.undefinedClass] Undefined class 'Unresolved'.
//                     ^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'InvalidType Function()' given to 'fromFunction' must be a valid 'dart:ffi' native function type.
}
