import 'dart:ffi';
typedef T = Int8 Function(Int8);
int f(int i) => i * 2;
void g() {
  Pointer.fromFunction<T>(f, '');
//                           ^^
// [diag.mustBeASubtype] The type 'String' must be a subtype of 'Int8' for 'fromFunction'.
}
