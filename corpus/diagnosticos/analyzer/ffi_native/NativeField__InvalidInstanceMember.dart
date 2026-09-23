import 'dart:ffi';

class Foo {
  @Native<IntPtr>()
  external int field;
//             ^^^^^
// [diag.nativeFieldNotStatic] Native fields must be static.
}
