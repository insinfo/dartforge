import 'a.dart';
abstract class B implements A {}
//             ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed] The type 'B' must be 'base', 'final' or 'sealed' because the supertype 'LinkedListEntry' is 'base'.
//                          ^
// [diag.baseClassImplementedOutsideOfLibrary] The class 'LinkedListEntry' can't be implemented outside of its library because it's a base class.
