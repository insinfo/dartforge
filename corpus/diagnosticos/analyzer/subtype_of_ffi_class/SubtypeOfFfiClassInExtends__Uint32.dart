import 'dart:ffi';
class C extends Uint32 {}
//              ^^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Uint32' can't be extended outside of its library because it's a final class.
