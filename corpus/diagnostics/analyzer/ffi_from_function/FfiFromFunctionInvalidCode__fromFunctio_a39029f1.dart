import 'dart:ffi';
int f(int x) => x;
void main() {
  Pointer.fromFunction(f);
//        ^^^^^^^^^^^^
// [diag.mustBeANativeFunctionType] The type 'Function' given to 'fromFunction' must be a valid 'dart:ffi' native function type.
}
