import 'a.dart';
class B extends A {
//    ^
// [diag.subtypeOfFinalIsNotBaseFinalOrSealed] The type 'B' must be 'base', 'final' or 'sealed' because the supertype 'MapEntry' is 'final'.
  int get key => 0;
  int get value => 1;
}
