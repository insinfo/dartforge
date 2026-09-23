import 'dart:ffi';
class C extends Float {}
//              ^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Float' can't be extended outside of its library because it's a final class.
