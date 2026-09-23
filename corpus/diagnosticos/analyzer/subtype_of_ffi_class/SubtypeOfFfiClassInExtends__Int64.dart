import 'dart:ffi';
class C extends Int64 {}
//              ^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Int64' can't be extended outside of its library because it's a final class.
