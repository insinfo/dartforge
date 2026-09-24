import 'dart:ffi';
typedef T = Int8 Function(Int8);
String f(int i) => i.toString();
void g() {
  Pointer.fromFunction<T>(f, 5);
//                        ^
// [diag.mustBeASubtype] The type 'String Function(int)' must be a subtype of 'T' for 'fromFunction'.
}
