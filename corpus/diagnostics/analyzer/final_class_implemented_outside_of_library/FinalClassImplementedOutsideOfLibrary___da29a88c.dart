import 'a.dart';
final class B implements A {
//                       ^
// [diag.finalClassImplementedOutsideOfLibrary] The class 'MapEntry' can't be implemented outside of its library because it's a final class.
  int get key => 0;
  int get value => 1;
}
