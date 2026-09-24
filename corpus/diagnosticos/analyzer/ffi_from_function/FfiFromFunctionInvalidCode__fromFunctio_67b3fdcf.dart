import 'dart:ffi';
int f(int x) => x;
void main() {
  Pointer.fromFunction<Int32 Function(Int32), Int8>(f, 0);
//                    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
// [diag.wrongNumberOfTypeArgumentsElement] The method 'fromFunction' is declared with 1 type parameters, but 2 type arguments are given.
//                     ^^^^^^^^^^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'dynamic' given to 'fromFunction' must be a valid 'dart:ffi' native function type.
}
