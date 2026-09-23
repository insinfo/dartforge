import 'dart:ffi';
class C extends Int16 {}
//              ^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Int16' can't be extended outside of its library because it's a final class.
