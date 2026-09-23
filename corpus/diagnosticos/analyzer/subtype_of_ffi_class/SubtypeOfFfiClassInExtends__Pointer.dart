import 'dart:ffi';
class C extends Pointer {
//              ^^^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Pointer' can't be extended outside of its library because it's a final class.
  external factory C();
}
