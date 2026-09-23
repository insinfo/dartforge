import 'dart:ffi';
int f(int x) => x;
void main() {
  Pointer.fromFunction<Int32 Function(Int32)>(f, exceptional: 1);
//                                               ^^^^^^^^^^^
// [diag.undefinedNamedParameter] The named parameter 'exceptional' isn't defined.
}
