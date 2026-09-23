// %before-language-feature: class-modifiers
import 'dart:ffi';
class C extends Double {}
//              ^^^^^^
// [diag.finalClassExtendedOutsideOfLibrary] The class 'Double' can't be extended outside of its library because it's a final class.
