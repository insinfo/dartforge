import 'a.dart';
abstract class B extends A {}
//             ^
// [diag.subtypeOfBaseIsNotBaseFinalOrSealed] The type 'B' must be 'base', 'final' or 'sealed' because the supertype 'LinkedListEntry' is 'base'.
