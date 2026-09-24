import 'dart:ffi';
class C extends Uint8 {}
//              ^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Uint8' can't be extended outside of its library because it's a final class.
