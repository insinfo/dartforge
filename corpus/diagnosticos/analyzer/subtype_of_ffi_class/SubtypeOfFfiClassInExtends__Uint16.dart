import 'dart:ffi';
class C extends Uint16 {}
//              ^^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Uint16' can't be extended outside of its library because it's a final class.
