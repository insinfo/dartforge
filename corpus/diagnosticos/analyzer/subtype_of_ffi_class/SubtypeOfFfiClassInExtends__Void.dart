import 'dart:ffi';
class C extends Void {}
//              ^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Void' can't be extended outside of its library because it's a final class.
