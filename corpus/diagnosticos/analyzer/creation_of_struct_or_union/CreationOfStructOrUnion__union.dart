import 'dart:ffi';

final class A extends Union {
  @Int32()
  external int a;
}

void f() {
  A();
//^
// [diag.creationOfStructOrUnion] Subclasses of 'Struct' and 'Union' are backed by native memory, and can't be instantiated by a generative constructor.
}
