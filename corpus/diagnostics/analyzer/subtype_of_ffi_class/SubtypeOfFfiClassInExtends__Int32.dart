import 'dart:ffi';
class C extends Int32 {}
//              ^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Int32' can't be extended outside of its library because it's a final class.
