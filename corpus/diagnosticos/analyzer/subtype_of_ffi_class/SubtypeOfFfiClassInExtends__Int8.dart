import 'dart:ffi';
class C extends Int8 {}
//              ^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Int8' can't be extended outside of its library because it's a final class.
