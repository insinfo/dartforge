import 'dart:ffi';
class C extends Uint64 {}
//              ^^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Uint64' can't be extended outside of its library because it's a final class.
